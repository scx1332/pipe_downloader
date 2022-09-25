use anyhow;
use flate2::read::GzDecoder;
use log;

use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use std::str::FromStr;
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::lz4_decoder::Lz4Decoder;
use human_bytes::human_bytes;
use std::time::Duration;
use tar::Archive;

#[derive(Debug, Clone)]
pub struct ProgressContext {
    pub start_time: std::time::Instant,
    pub total_downloaded: usize,
    pub chunk_downloaded: usize,
    pub total_unpacked: usize,
    pub stop_requested: bool,
    pub paused: bool,
}

impl Default for ProgressContext {
    fn default() -> ProgressContext {
        ProgressContext {
            start_time: std::time::Instant::now(),
            total_downloaded: 0,
            chunk_downloaded: 0,
            total_unpacked: 0,
            stop_requested: false,
            paused: false,
        }
    }
}

impl ProgressContext {
    pub fn get_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
    pub fn get_download_speed(&self) -> f64 {
        let elapsed = self.get_elapsed();
        if elapsed.as_secs() == 0 {
            return 0.0;
        }
        (self.total_downloaded + self.chunk_downloaded) as f64 / elapsed.as_secs() as f64
    }
    pub fn get_download_speed_human(&self) -> String {
        human_bytes(self.get_download_speed())
    }
    pub fn get_unpack_speed(&self) -> f64 {
        let elapsed = self.get_elapsed();
        if elapsed.as_secs() == 0 {
            return 0.0;
        }
        (self.total_unpacked) as f64 / elapsed.as_secs() as f64
    }
    pub fn get_unpack_speed_human(&self) -> String {
        human_bytes(self.get_unpack_speed())
    }

}

#[derive(Debug, Clone)]
pub struct PipeDownloaderOptions {
    pub chunk_size_downloader: usize,
    pub chunk_size_decoder: usize,
}

impl Default for PipeDownloaderOptions {
    fn default() -> Self {
        Self {
            chunk_size_downloader: 30_000_000,
            chunk_size_decoder: 10_000_000,
        }
    }
}

pub struct PipeDownloader {
    url: String,
    progress_context: Arc<Mutex<ProgressContext>>,
    options: PipeDownloaderOptions,
    download_started: bool,
    target_path: PathBuf,
    t1: Option<thread::JoinHandle<()>>,
    t2: Option<thread::JoinHandle<()>>,
    t3: Option<thread::JoinHandle<()>>,
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
            t1: None,
            t2: None,
            t3: None,
            options: pipe_downloader_options,
        }
    }
}

struct Pipe {
    pos: usize,
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    current_buf: Vec<u8>,
    current_buf_pos: usize,
    report_progress: usize,
    progress_message: String,
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos > self.report_progress + 100000 {
            if !self.progress_message.is_empty() {
                log::debug!(
                    "{}: {}",
                    self.progress_message,
                    human_bytes(self.pos as f64)
                );
            }
            self.report_progress = self.pos;
        }
        let starting_pos = self.pos;
        if self.current_buf.is_empty() || self.current_buf_pos >= self.current_buf.len() {
            let res = self.receiver.recv();
            if res.is_err() {
                return Ok(0);
            }
            self.current_buf = res.unwrap();
            self.current_buf_pos = 0;
        }
        let min_val = std::cmp::min(self.current_buf.len() - self.current_buf_pos, buf.len());
        for i in 0..min_val {
            buf[i] = self.current_buf[self.current_buf_pos];
            self.current_buf_pos += 1;
            self.pos += 1;
        }
        log::debug!(
            "Chunk read: starting_pos: {} / length: {}",
            starting_pos,
            self.pos - starting_pos
        );
        return Ok(min_val);
    }
}

fn download_chunk(
    progress_context: Arc<Mutex<ProgressContext>>,
    url: &str,
    client: &reqwest::blocking::Client,
    range: &std::ops::Range<usize>,
) -> anyhow::Result<Vec<u8>> {
    log::debug!(
        "Downloading chunk: range {:?} / {}",
        range,
        range.end - range.start
    );

    let header = format!("bytes={}-{}", range.start, range.end - 1);
    let mut response = client.get(url).header("Range", header).send()?;

    let status = response.status();
    let content_length = response
        .headers()
        .get("Content-Length")
        .ok_or(anyhow::anyhow!("Content-Length header not found"))?
        .to_str()?;
    let content_length = usize::from_str(content_length)?;

    if !(status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT) {
        return Err(anyhow::anyhow!("unexpected status code: {}", status));
    } else {
        log::info!("Chunk downloaded with status: {:?}", status);
    }
    let mut buf_vec: Vec<u8> = Vec::with_capacity(content_length);

    let mut buf = vec![0; 1024 * 1024];
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        buf_vec.extend_from_slice(&buf[..n]);
        {
            let mut progress_context = progress_context.lock().unwrap();
            progress_context.chunk_downloaded += n;
            if progress_context.paused {
                return Err(anyhow::anyhow!("Download paused"));
            }
            if progress_context.stop_requested {
                return Err(anyhow::anyhow!("Stop requested"));
            }
        }
    }
    if buf_vec.len() != content_length {
        return Err(anyhow::anyhow!(
            "unexpected content length: {}",
            buf_vec.len()
        ));
    }

    assert_eq!(content_length, range.end - range.start);
    assert_eq!(buf_vec.len(), range.end - range.start);

    log::debug!(
        "Chunk downloaded: range {:?} / {}",
        range,
        range.end - range.start
    );

    return Ok(buf_vec);
}

fn decode_loop<T: Read>(
    progress_context: Arc<Mutex<ProgressContext>>,
    options: &PipeDownloaderOptions,
    decoder: &mut T,
    send: std::sync::mpsc::SyncSender<Vec<u8>>,
) -> anyhow::Result<()> {
    let mut unpacked_size = 0;
    loop {
        let mut buf = vec![0u8; options.chunk_size_decoder];
        let bytes_read = match decoder.read(&mut buf) {
            Ok(bytes_read) => bytes_read,
            Err(err) => {
                log::error!("Error while reading from lz4 decoder {:?}", err);
                break;
            }
        };
        if bytes_read == 0 {
            break;
        }
        unpacked_size += bytes_read;
        {
            let mut progress = progress_context.lock().unwrap();
            progress.total_unpacked = unpacked_size;
            if progress.stop_requested {
                break;
            }
        }

        log::debug!(
            "Decode loop, Unpacked size: {}",
            human_bytes(unpacked_size as f64)
        );
        buf.resize(bytes_read, 0);
        send.send(buf)?;
    }
    log::info!("Finishing decode loop");
    Ok(())
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
        if let Some(t3) = self.t3.as_ref() {
            t3.is_finished()
        } else {
            false
        }
    }

    pub fn get_progress(self: &PipeDownloader) -> anyhow::Result<ProgressContext> {
        let pc = self
            .progress_context
            .lock()
            .expect("Failed to lock progress context");
        let pc = pc.clone();
        return Ok(pc);
    }

    pub fn start_download(self: &mut PipeDownloader) -> anyhow::Result<()> {
        if self.download_started {
            return Err(anyhow::anyhow!("Download already started"));
        }
        self.progress_context.lock().expect("Failed to obtain lock").start_time = std::time::Instant::now();
        self.download_started = true;
        let url = self.url.clone();
        //let url = "https://github.com/golemfactory/ya-runtime-http-auth/releases/download/v0.1.0/ya-runtime-http-auth-linux-v0.1.0.tar.gz";

        let client = reqwest::blocking::Client::new();
        let response = client.head(&url).send()?;
        let length = response
            .headers()
            .get(CONTENT_LENGTH)
            .ok_or("response doesn't include the content length")
            .unwrap();
        let length = usize::from_str(length.to_str()?)
            .map_err(|_| "invalid Content-Length header")
            .unwrap();

        log::info!("starting download...");
        let (send_download_chunks, receive_download_chunks) = sync_channel(1);

        let pc = self.progress_context.clone();
        let download_url = url.clone();
        let options = self.options.clone();
        self.t1 = Some(thread::spawn(move || {
            let chunk_size = options.chunk_size_downloader;

            'range_loop: for i in 0..(length / chunk_size + 1) {
                let max_length = std::cmp::min(chunk_size, length - i * chunk_size);
                if max_length == 0 {
                    break;
                }
                let range = std::ops::Range {
                    start: i * chunk_size,
                    end: i * chunk_size + max_length,
                };
                let client = reqwest::blocking::Client::new();

                loop {
                    let progress = { pc.lock().unwrap().clone() };
                    if progress.stop_requested {
                        break 'range_loop;
                    }
                    if progress.paused {
                        log::info!("Download still paused...");
                        thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                    match download_chunk(pc.clone(), &download_url, &client, &range) {
                        Ok(buf) => {
                            {
                                let mut progress = pc.lock().unwrap();
                                progress.total_downloaded += progress.chunk_downloaded;
                                progress.chunk_downloaded = 0;
                                if progress.stop_requested {
                                    break 'range_loop;
                                }
                            }
                            if let Err(err) = send_download_chunks.send(buf) {
                                log::error!("Error while sending chunk: {:?}", err);
                                let mut progress = pc.lock().unwrap();
                                progress.stop_requested = true;
                                break 'range_loop;
                            }
                            break;
                        }
                        Err(err) => {
                            let progress = {
                                let mut progress = pc.lock().unwrap();
                                progress.chunk_downloaded = 0;
                                progress.clone()
                            };
                            if progress.stop_requested {
                                break 'range_loop;
                            }
                            if progress.paused {
                                log::info!("Download paused, trying again");
                            } else {
                                log::warn!(
                                    "Error while downloading chunk, trying again: {:?}",
                                    err
                                );
                            }
                            thread::sleep(Duration::from_secs(5));
                        }
                    }
                }
            }
            println!("Finishing thread 1");
        }));
        let mut p = Pipe {
            pos: 0,
            receiver: receive_download_chunks,
            current_buf: vec![],
            current_buf_pos: 0,
            report_progress: 0,
            progress_message: "Downloading".to_string(),
        };

        let (send_unpack_chunks, receive_unpack_chunks) = sync_channel(1);

        let pc = self.progress_context.clone();
        let download_url = url.clone();
        let options = self.options.clone();
        self.t2 = Some(thread::spawn(move || {
            if download_url.ends_with(".gz") {
                let mut gz = GzDecoder::new(&mut p);
                decode_loop(pc.clone(), &options, &mut gz, send_unpack_chunks).unwrap();
            } else if download_url.ends_with(".lz4") {
                let mut lz4 = Lz4Decoder::new(&mut p).unwrap();
                decode_loop(pc.clone(), &options, &mut lz4, send_unpack_chunks).unwrap();
            } else {
                panic!("Unknown file type");
            };
        }));

        let mut p2 = Pipe {
            pos: 0,
            receiver: receive_unpack_chunks,
            current_buf: vec![],
            current_buf_pos: 0,
            report_progress: 0,
            progress_message: "Unpacking".to_string(),
        };

        let target_path = self.target_path.clone();
        if url.contains(".tar.") {
            self.t3 = Some(thread::spawn(move || {
                let mut archive = Archive::new(p2);
                match archive.unpack(target_path) {
                    Ok(_) => {
                        log::info!("Successfully unpacked")
                    }
                    Err(err) => {
                        log::error!("Error while unpacking {:?}", err);
                    }
                }
            }));
        } else {
            let mut output_file = File::create(&target_path)?;
            self.t3 = Some(thread::spawn(move || {
                match std::io::copy(&mut p2, &mut output_file) {
                    Ok(_) => {
                        log::info!("Successfully written file {:?}", target_path);
                    }
                    Err(err) => {
                        log::error!("Error while writing {:?}", err);
                    }
                };
            }));
        }

        Ok(())
    }
    pub fn wait_for_finish(self: &mut PipeDownloader) {
        self.t1.take().unwrap().join().unwrap();
        self.t2.take().unwrap().join().unwrap();
    }
}
