use flate2::read::GzDecoder;
use log;

use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use std::str::FromStr;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use crate::lz4_decoder::Lz4Decoder;

use anyhow::anyhow;
use bzip2::read::BzDecoder;
use lz4_flex::frame::FrameDecoder;
use reqwest::blocking::Response;
use std::time::Duration;
use tar::Archive;

use crate::pipe_progress::ProgressContext;
use crate::pipe_utils::bytes_to_human;
use crate::pipe_wrapper::{DataChunk, MpscReaderFromReceiver};

#[derive(Debug, Clone)]
pub struct PipeDownloaderOptions {
    pub chunk_size_downloader: usize,
    pub chunk_size_decoder: usize,
    pub max_download_speed: Option<usize>,
    pub force_no_chunks: bool,
    pub download_threads: usize,
}

impl Default for PipeDownloaderOptions {
    fn default() -> Self {
        Self {
            chunk_size_downloader: 30_000_000,
            chunk_size_decoder: 10_000_000,
            max_download_speed: None,
            force_no_chunks: false,
            download_threads: 2,
        }
    }
}

impl PipeDownloaderOptions {
    pub fn from_env() -> Self {
        let mut options = Self::default();
        if let Ok(download_buffer) = std::env::var("PIPE_DOWNLOADER_DOWNLOAD_BUFFER") {
            options.chunk_size_downloader = usize::from_str(&download_buffer).unwrap();
        }
        if let Ok(decoder_buffer) = std::env::var("PIPE_DOWNLOADER_DECODER_BUFFER") {
            options.chunk_size_decoder = usize::from_str(&decoder_buffer).unwrap();
        }
        if let Ok(download_threads) = std::env::var("PIPE_DOWNLOADER_DOWNLOAD_THREADS") {
            options.download_threads = usize::from_str(&download_threads).unwrap();
        }
        options
    }
}

pub struct PipeDownloader {
    url: String,
    progress_context: Arc<Mutex<ProgressContext>>,
    options: PipeDownloaderOptions,
    download_started: bool,
    target_path: PathBuf,
    thread_last_stage: Option<thread::JoinHandle<()>>,
}

impl PipeDownloader {
    pub fn new(
        url: &str,
        target_path: &PathBuf,
        pipe_downloader_options: PipeDownloaderOptions,
    ) -> Self {
        Self {
            url: url.to_string(),
            progress_context: Arc::new(Mutex::new(ProgressContext::default())),
            download_started: false,
            target_path: target_path.clone(),
            thread_last_stage: None,
            options: pipe_downloader_options,
        }
    }
}

fn download_chunk(
    thread_no: usize,
    progress_context: Arc<Mutex<ProgressContext>>,
    range: &std::ops::Range<usize>,
    max_speed: Option<usize>,
    response: &mut reqwest::blocking::Response,
) -> anyhow::Result<Vec<u8>> {
    let mut buf_vec: Vec<u8> = Vec::with_capacity(range.end - range.start);

    let mut buf = vec![0; 1024 * 1024];
    let mut total_downloaded: usize = 0;
    let start_time = std::time::Instant::now();
    loop {
        let left_to_download = (range.end - range.start) - total_downloaded;
        let max_buf_size = std::cmp::min(buf.len(), left_to_download);
        if max_buf_size == 0 {
            break;
        }
        let n = response.read(&mut buf[..max_buf_size])?;
        if n == 0 {
            return Err(anyhow!("Unexpected end of file"));
        }
        total_downloaded += n;

        buf_vec.extend_from_slice(&buf[..n]);
        {
            let mut progress_context = progress_context.lock().unwrap();
            if let Some(cd) = progress_context.chunk_downloaded.get_mut(thread_no) {
                *cd += n;
            }
            progress_context.progress_buckets_download.add_bytes(n);
            if progress_context.paused {
                return Err(anyhow::anyhow!("Download paused"));
            }
            if progress_context.stop_requested {
                return Err(anyhow::anyhow!("Stop requested"));
            }
        }
        //Speed throttling is not perfect by any means, but it's good enough for now
        if let Some(max_speed) = max_speed {
            let should_take_time =
                Duration::from_secs_f64(total_downloaded as f64 / max_speed as f64);
            log::debug!("Should take time: {:?}", should_take_time);
            loop {
                let elapsed = std::time::Instant::now().duration_since(start_time);
                if should_take_time > elapsed {
                    std::thread::sleep(Duration::from_millis(1));
                } else {
                    break;
                }
            }
        }
    }
    if buf_vec.len() != range.end - range.start {
        return Err(anyhow::anyhow!(
            "unexpected content length: {}",
            buf_vec.len()
        ));
    }

    log::debug!(
        "Chunk downloaded: range {:?} / {}",
        range,
        range.end - range.start
    );

    return Ok(buf_vec);
}

fn request_chunk(
    url: &str,
    client: &reqwest::blocking::Client,
    range: &std::ops::Range<usize>,
) -> anyhow::Result<Response> {
    log::debug!(
        "Downloading chunk: range {:?} / {}",
        range,
        range.end - range.start
    );

    let header = format!("bytes={}-{}", range.start, range.end - 1);
    let response = client.get(url).header("Range", header).send()?;

    let status = response.status();
    let content_length = response
        .headers()
        .get("Content-Length")
        .ok_or(anyhow::anyhow!("Content-Length header not found"))?
        .to_str()?;
    let content_length = usize::from_str(content_length)?;

    if status == StatusCode::OK && range.start != 0 {
        return Err(anyhow::anyhow!(
            "Seems like server does not support partial content: {}",
            status
        ));
    }
    if status != StatusCode::PARTIAL_CONTENT && range.start != 0 {
        return Err(anyhow::anyhow!("unexpected status code: {}", status));
    } else {
        log::info!(
            "Received status: {:?}, starting downloading chunk data...",
            status
        );
    }
    if content_length != range.end - range.start {
        return Err(anyhow::anyhow!(
            "unexpected content length: {}",
            content_length
        ));
    }
    Ok(response)
}

fn decode_loop<T: Read>(
    progress_context: Arc<Mutex<ProgressContext>>,
    options: &PipeDownloaderOptions,
    decoder: &mut T,
    send: std::sync::mpsc::SyncSender<DataChunk>,
) -> anyhow::Result<()> {
    let mut unpacked_size = 0;
    loop {
        let mut buf = vec![0u8; options.chunk_size_decoder];
        let bytes_read = match decoder.read(&mut buf) {
            Ok(bytes_read) => bytes_read,
            Err(err) => {
                log::error!("Error while reading from decoder {:?}", err);
                return Err(anyhow::anyhow!(
                    "Error while reading from decoder {:?}",
                    err
                ));
            }
        };
        if bytes_read == 0 {
            break;
        }
        unpacked_size += bytes_read;
        {
            let mut progress = progress_context.lock().unwrap();
            progress.total_unpacked = unpacked_size;
            progress.progress_buckets_unpack.add_bytes(bytes_read);
            if progress.stop_requested {
                break;
            }
        }

        log::debug!(
            "Decode loop, Unpacked size: {}",
            bytes_to_human(unpacked_size)
        );
        buf.resize(bytes_read, 0);
        let data_chunk = DataChunk {
            data: buf,
            range: unpacked_size - bytes_read..unpacked_size,
        };
        send.send(data_chunk)?;
    }
    log::info!("Finishing decode loop");
    Ok(())
}

fn download_loop(
    thread_no: usize,
    thread_count: usize,
    options: PipeDownloaderOptions,
    progress_context: Arc<Mutex<ProgressContext>>,
    send_download_chunks: SyncSender<DataChunk>,
    download_url: &str,
) -> anyhow::Result<()> {
    let mut use_chunks = !options.force_no_chunks;
    let client = reqwest::blocking::Client::new();

    //.link extension means that the files point to the file that we need to download
    let download_url = if download_url.ends_with(".link") {
        let url = client.get(download_url).send()?.text()?.trim().to_string();
        if url.is_empty() {
            return Err(anyhow::anyhow!("Empty url from link"));
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(anyhow::anyhow!(
                "Wrong url, should start from http:// or https://"
            ));
        }
        log::info!("Got download address from link: {}", url);
        url
    } else {
        download_url.to_string()
    };
    progress_context.lock().unwrap().download_url = Some(download_url.clone());

    let response = client.head(&download_url).send()?;
    let length = response
        .headers()
        .get(CONTENT_LENGTH)
        .ok_or(anyhow!("response doesn't include the content length"))?;
    let total_length =
        usize::from_str(length.to_str()?).map_err(|_| anyhow!("invalid Content-Length header"))?;

    progress_context.lock().unwrap().total_download_size = Some(total_length);
    if total_length == 0 {
        return Err(anyhow::anyhow!(
            "Content-Length is 0, empty files not supported"
        ));
    }

    if total_length < 10000 {
        log::info!("Single request mode");
        use_chunks = false;
    }
    //check if server supports partial content
    if use_chunks {
        let header = format!("bytes={}-{}", 1000, 2000);
        let response = client.head(&download_url).header("Range", header).send()?;
        if response.status() != StatusCode::PARTIAL_CONTENT {
            log::warn!("Server does not support partial content, falling back to single request. No retries after connection error will be possible");
            use_chunks = false;
        }
    }
    let thread_count = if use_chunks {
        thread_count
    } else {
        if thread_no != 0 {
            log::warn!("Thread number is set, but server does not support partial content, falling back to single request");
            return Ok(());
        } else {
            1
        }
    };

    progress_context
        .lock()
        .unwrap()
        .chunk_downloaded
        .resize(thread_count, 0);
    let mut download_response = if !use_chunks {
        Some(client.get(&download_url).send()?)
    } else {
        None
    };

    let chunk_size = options.chunk_size_downloader;

    {
        //first thread to fill this value (blocking other threads)
        let mut pc = progress_context.lock().unwrap();
        if pc.unfinished_chunks.is_empty() {
            for i in 0..((total_length - 1) / chunk_size + 1) {
                pc.unfinished_chunks.push(i);
            }
            pc.unfinished_chunks.reverse();
            log::warn!("Unfinished chunks: {:?}", pc.unfinished_chunks);
        }
    }

    for i in 0..((total_length - 1) / chunk_size + 1) {
        let max_length = std::cmp::min(chunk_size, total_length - i * chunk_size);
        if max_length == 0 {
            break;
        }
        //one thread for one range
        if i % thread_count != thread_no {
            continue;
        }
        loop {
            let smallest_unfinished = *progress_context
                .lock()
                .unwrap()
                .unfinished_chunks
                .last()
                .expect("Critical error unfinished chunks have to be here");
            if i - smallest_unfinished > thread_count {
                //log::warn!("Waiting for chunk {}", i);
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            break;
        }
        let range = std::ops::Range {
            start: i * chunk_size,
            end: i * chunk_size + max_length,
        };
        let client = reqwest::blocking::Client::new();

        loop {
            let progress = { progress_context.lock().unwrap().clone() };
            if progress.stop_requested {
                return Err(anyhow::anyhow!("Stop requested"));
            }
            if progress.paused {
                log::info!("Download still paused...");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            if thread_count > 1 {
                //unfortunately we can't reuse response, when using more threads
                download_response = None;
            }

            let result = if let Some(download_response) = &mut download_response {
                download_chunk(
                    thread_no,
                    progress_context.clone(),
                    &range,
                    options.max_download_speed,
                    download_response,
                )
            } else {
                // recreate response if last one was closed
                let new_range = if thread_count == 1 {
                    //reuse connection if only one thread
                    std::ops::Range {
                        start: range.start,
                        end: total_length,
                    }
                } else {
                    range.clone()
                };

                match request_chunk(&download_url, &client, &new_range) {
                    Ok(mut new_response) => {
                        let res = download_chunk(
                            thread_no,
                            progress_context.clone(),
                            &range,
                            options.max_download_speed,
                            &mut new_response,
                        );
                        download_response = Some(new_response);
                        res
                    }
                    Err(err) => {
                        log::error!("Error while requesting chunk: {:?}", err);
                        Err(err)
                    }
                }
            };
            match result {
                Ok(buf) => {
                    {
                        let mut progress = progress_context.lock().unwrap();
                        progress.total_downloaded += progress.chunk_downloaded[thread_no];
                        progress.chunk_downloaded[thread_no] = 0;
                        if progress.stop_requested {
                            return Err(anyhow::anyhow!("Stop requested"));
                        }
                    }
                    let dc = DataChunk {
                        range: range.clone(),
                        data: buf,
                    };
                    {
                        // search from right to left, because smallest chunk are on the right
                        // also remove element from right, because it is faster
                        let mut pc = progress_context.lock().unwrap();
                        let idx_to_remove =
                            pc.unfinished_chunks.iter().rposition(|el| *el == i).expect(
                                format!("Critical error unfinished chunks have to be here {}", i)
                                    .as_str(),
                            );
                        assert!(i == pc.unfinished_chunks[idx_to_remove]);
                        log::warn!("Removing chunk {} at idx {}", i, idx_to_remove);
                        pc.unfinished_chunks.remove(idx_to_remove);
                    }
                    if let Err(err) = send_download_chunks.send(dc) {
                        log::error!("Error while sending chunk: {:?}", err);
                        return Err(anyhow::anyhow!("Error while sending chunk: {:?}", err));
                    }
                    break;
                }
                Err(err) => {
                    //reset response to force reconnection
                    download_response = None;
                    let progress = {
                        let mut progress = progress_context.lock().unwrap();
                        progress.chunk_downloaded[thread_no] = 0;
                        progress.clone()
                    };
                    if progress.stop_requested {
                        return Err(anyhow::anyhow!("Stop requested"));
                    }
                    if !use_chunks {
                        return Err(anyhow::anyhow!("Error while downloading: {:?}", err));
                    }
                    if progress.paused {
                        log::info!("Download paused, trying again");
                    } else {
                        log::warn!("Error while downloading chunk, trying again: {:?}", err);
                    }
                    thread::sleep(Duration::from_secs(5));
                }
            }
        }
    }
    Ok(())
}

fn resolve_url(download_url: String, pc: Arc<Mutex<ProgressContext>>) -> anyhow::Result<String> {
    if download_url.ends_with(".link") {
        loop {
            if let Some(url) = pc.lock().unwrap().download_url.clone() {
                break Ok(url);
            }
            if pc.lock().unwrap().stop_requested {
                return Err(anyhow::anyhow!("Stop requested"));
            }
            thread::sleep(Duration::from_secs(1));
        }
    } else {
        Ok(download_url)
    }
}

impl PipeDownloader {
    #[allow(unused)]
    pub fn signal_stop(self: &PipeDownloader) {
        let mut pc = self
            .progress_context
            .lock()
            .expect("Failed to lock progress context");
        pc.stop_requested = true;
    }

    #[allow(unused)]
    pub fn pause_download(self: &PipeDownloader) {
        let mut pc = self
            .progress_context
            .lock()
            .expect("Failed to lock progress context");
        pc.paused = true;
    }

    #[allow(unused)]
    pub fn resume_download(self: &PipeDownloader) {
        let mut pc = self
            .progress_context
            .lock()
            .expect("Failed to lock progress context");
        pc.paused = false;
    }

    pub fn is_finished(self: &PipeDownloader) -> bool {
        if let Some(thread_last_stage) = self.thread_last_stage.as_ref() {
            thread_last_stage.is_finished()
        } else {
            false
        }
    }

    fn get_progress_guard(self: &PipeDownloader) -> MutexGuard<ProgressContext> {
        self.progress_context
            .lock()
            .expect("Failed to lock progress context")
    }

    #[allow(unused)]
    pub fn get_progress(self: &PipeDownloader) -> ProgressContext {
        self.get_progress_guard().clone()
    }

    pub fn get_progress_json(self: &PipeDownloader) -> serde_json::Value {
        self.get_progress_guard().to_json()
    }

    pub fn get_progress_human_line(self: &PipeDownloader) -> String {
        let progress = self.get_progress_guard();

        let eta_string = if let Some(eta) = progress.get_time_left_sec() {
            let seconds = eta % 60;
            let minutes = (eta / 60) % 60;
            let hours = (eta / 60) / 60;
            format!("ETA: {:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            "ETA: unknown".to_string()
        };
        let percent_string = if let Some(total_length) = progress.total_download_size {
            format!(
                "[{:.2}%]",
                ((progress.total_downloaded + progress.chunk_downloaded.iter().sum::<usize>())
                    as f64
                    / total_length as f64)
                    * 100.0
            )
        } else {
            "".to_string()
        };

        format!(
            "Downloaded: {} [{}/s now: {}/s], Unpack: {} [{}/s now: {}/s] - {} {}",
            bytes_to_human(
                progress.total_downloaded + progress.chunk_downloaded.iter().sum::<usize>()
            ),
            bytes_to_human(progress.get_download_speed()),
            bytes_to_human(progress.progress_buckets_download.get_speed()),
            bytes_to_human(progress.total_unpacked),
            bytes_to_human(progress.get_unpack_speed()),
            bytes_to_human(progress.progress_buckets_unpack.get_speed()),
            eta_string,
            percent_string
        )
    }

    #[allow(unused)]
    pub fn is_started(self: &PipeDownloader) -> bool {
        return self.download_started;
    }

    pub fn start_download(self: &mut PipeDownloader) -> anyhow::Result<()> {
        if self.download_started {
            return Err(anyhow::anyhow!("Download already started"));
        }
        self.progress_context
            .lock()
            .expect("Failed to obtain lock")
            .start_time = chrono::Utc::now();
        self.download_started = true;
        let url = self.url.clone();
        //let url = "https://github.com/golemfactory/ya-runtime-http-auth/releases/download/v0.1.0/ya-runtime-http-auth-linux-v0.1.0.tar.gz";

        log::info!("starting download...");
        let (send_download_chunks, receive_download_chunks) = sync_channel(1);

        let download_thread_count = self.options.download_threads;
        let mut threads = Vec::new();
        for thread_no in 0..download_thread_count {
            let pc = self.progress_context.clone();
            let download_url = url.clone();
            let options = self.options.clone();
            let send = send_download_chunks.clone();
            threads.push(thread::spawn(move || {
                match download_loop(
                    thread_no,
                    download_thread_count,
                    options,
                    pc.clone(),
                    send,
                    &download_url,
                ) {
                    Ok(_) => {
                        log::info!("Download loop finished, finishing thread");
                    }
                    Err(err) => {
                        log::error!("Error in download loop: {:?}, finishing thread", err);
                        //stop other threads as well
                        pc.lock().unwrap().stop_requested = true;
                        pc.lock().unwrap().error_message_download = Some(err.to_string());
                    }
                }
            }));
        }

        let mut p = MpscReaderFromReceiver::new(receive_download_chunks, true);

        let (send_unpack_chunks, receive_unpack_chunks) = sync_channel::<DataChunk>(1);

        let pc = self.progress_context.clone();
        let download_url = url.clone();
        let options = self.options.clone();
        let t2 = thread::spawn(move || {
            let download_url = match resolve_url(download_url, pc.clone()) {
                Ok(url) => url,
                Err(err) => {
                    pc.lock().unwrap().error_message_unpack = Some(err.to_string());
                    return;
                }
            };

            let res = if download_url.ends_with(".gz") {
                let mut gz = GzDecoder::new(&mut p);
                decode_loop(pc.clone(), &options, &mut gz, send_unpack_chunks)
            } else if download_url.ends_with(".lz4-rust") {
                //rust implementation is slower - you can test that renaming source file from .lz4 to .lz4-rust
                let mut lz4 = FrameDecoder::new(&mut p);
                decode_loop(pc.clone(), &options, &mut lz4, send_unpack_chunks)
            } else if download_url.ends_with(".lz4") {
                let mut lz4 = Lz4Decoder::new(&mut p).unwrap();
                decode_loop(pc.clone(), &options, &mut lz4, send_unpack_chunks)
            } else if download_url.ends_with(".bz2") {
                let mut bz2 = BzDecoder::new(&mut p);
                decode_loop(pc.clone(), &options, &mut bz2, send_unpack_chunks)
            } else {
                panic!("Unknown file type");
            };
            if let Err(err) = res {
                log::error!("Error in decode loop: {:?}, finishing thread", err);
                //stop other threads as well
                pc.lock().unwrap().stop_requested = true;
                pc.lock().unwrap().error_message_unpack = Some(err.to_string());
            };
            log::info!("Decode loop finished, finishing thread");
        });

        let mut p2 = MpscReaderFromReceiver::new(receive_unpack_chunks, false);

        let target_path = self.target_path.clone();
        let download_url = url.clone();

        let pc = self.progress_context.clone();
        self.thread_last_stage = Some(thread::spawn(move || {
            let download_url = match resolve_url(download_url, pc.clone()) {
                Ok(url) => url,
                Err(err) => {
                    pc.lock().unwrap().error_message = Some(err.to_string());
                    return;
                }
            };

            let res = if download_url.contains(".tar.") {
                let mut archive = Archive::new(p2);
                match archive.unpack(target_path) {
                    Ok(_) => {
                        log::info!("Successfully unpacked");
                        Ok(())
                    }
                    Err(err) => {
                        log::error!("Error while unpacking {:?}", err);
                        Err(err)
                    }
                }
            } else {
                let mut output_file = File::create(&target_path).unwrap();
                match std::io::copy(&mut p2, &mut output_file) {
                    Ok(_) => {
                        log::info!("Successfully written file {:?}", target_path);
                        Ok(())
                    }
                    Err(err) => {
                        log::error!("Error while writing {:?}", err);
                        Err(err)
                    }
                }
            };
            match res {
                Ok(_) => {
                    pc.lock().unwrap().stop_requested = true;
                    for t1 in threads {
                        t1.join().unwrap();
                    }
                    t2.join().unwrap();
                    pc.lock().unwrap().finish_time = Some(chrono::Utc::now());
                }
                Err(err) => {
                    pc.lock().unwrap().error_message = Some(format!("{:?}", err));
                    pc.lock().unwrap().stop_requested = true;
                    for t1 in threads {
                        t1.join().unwrap();
                    }
                    t2.join().unwrap();
                    pc.lock().unwrap().error_time = Some(chrono::Utc::now());
                }
            }
        }));

        Ok(())
    }
}
