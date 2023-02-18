use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;

use std::io::Read;

use std::str::FromStr;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::options::PipeDownloaderOptions;
use anyhow::anyhow;

use reqwest::blocking::Response;
use std::time::Duration;

use crate::pipe_progress::{DownloadChunkProgress, InternalProgress};
use crate::pipe_utils::bytes_to_human;
use crate::pipe_wrapper::DataChunk;

fn download_chunk(
    chunk_no: usize,
    thread_no: usize,
    progress_context: Arc<Mutex<InternalProgress>>,
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

            if let Some(cc) = progress_context.current_chunks.get_mut(&chunk_no) {
                cc.downloaded += n;
            }
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

    Ok(buf_vec)
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
        .ok_or_else(|| anyhow::anyhow!("Content-Length header not found"))?
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

pub fn decode_loop<T: Read>(
    progress_context: Arc<Mutex<InternalProgress>>,
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
            chunk_no: 0,
            data: buf,
            range: unpacked_size - bytes_read..unpacked_size,
        };
        send.send(data_chunk)?;
    }
    log::info!("Finishing decode loop");
    Ok(())
}

pub fn download_loop(
    thread_no: usize,
    thread_count: usize,
    options: PipeDownloaderOptions,
    progress_context: Arc<Mutex<InternalProgress>>,
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

    let total_length = match response
        .headers()
        .get(CONTENT_LENGTH)
        .ok_or_else(|| anyhow!("response doesn't include the content length"))
    {
        Ok(length) => Some(
            usize::from_str(length.to_str()?)
                .map_err(|_| anyhow!("invalid Content-Length header"))?,
        ),
        Err(_) => {
            log::warn!("Content-Length header not found, continue download without knowledge about file size...");
            use_chunks = false;
            None
        }
    };

    progress_context.lock().unwrap().total_download_size = total_length;

    if total_length
        .map(|total_length| total_length == 0)
        .unwrap_or(false)
    {
        return Err(anyhow::anyhow!(
            "Content-Length is 0, empty files not supported"
        ));
    }

    if total_length
        .map(|total_length| total_length < 10000)
        .unwrap_or(false)
    {
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
    } else if thread_no != 0 {
        log::warn!("Thread number is set, but server does not support partial content, falling back to single request");
        return Ok(());
    } else {
        1
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

    let chunk_count = match total_length {
        Some(total_length) => (total_length - 1) / chunk_size + 1,
        None => 1,
    };
    let total_length = total_length.ok_or_else(|| anyhow!("Content length unknown"))?;

    {
        //first thread to fill this value (blocking other threads)
        let mut pc = progress_context.lock().unwrap();
        if pc.unfinished_chunks.is_empty() {
            pc.chunk_size = chunk_size;
            pc.total_chunks = chunk_count;
            pc.unfinished_chunks.reserve(chunk_count);
            for i in (0..chunk_count).rev() {
                pc.unfinished_chunks.push(i);
            }
            log::warn!("Unfinished chunks: {:?}", pc.unfinished_chunks);
        }
    }

    for chunk_no in 0..chunk_count {
        let max_length = std::cmp::min(chunk_size, total_length - chunk_no * chunk_size);
        if max_length == 0 {
            break;
        }
        //one thread for one range
        if chunk_no % thread_count != thread_no {
            continue;
        }
        loop {
            let smallest_unfinished = *progress_context
                .lock()
                .unwrap()
                .unfinished_chunks
                .last()
                .expect("Critical error unfinished chunks have to be here");
            if chunk_no - smallest_unfinished > thread_count {
                //log::warn!("Waiting for chunk {}", i);
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            break;
        }
        let range = std::ops::Range {
            start: chunk_no * chunk_size,
            end: chunk_no * chunk_size + max_length,
        };
        let client = reqwest::blocking::Client::new();
        {
            let mut progress = progress_context.lock().unwrap();
            progress.current_chunks.insert(
                chunk_no,
                DownloadChunkProgress {
                    downloaded: 0,
                    to_download: max_length,
                    unpacked: 0,
                    to_unpack: max_length,
                },
            );
        }

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
                    chunk_no,
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
                            chunk_no,
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
                        chunk_no,
                        range: range.clone(),
                        data: buf,
                    };
                    {
                        // search from right to left, because smallest chunk are on the right
                        // also remove element from right, because it is faster
                        let mut pc = progress_context.lock().unwrap();
                        let idx_to_remove = pc
                            .unfinished_chunks
                            .iter()
                            .rposition(|el| *el == chunk_no)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Critical error, chunk {} should be in unfinished chunks",
                                    chunk_no
                                )
                            });
                        assert!(chunk_no == pc.unfinished_chunks[idx_to_remove]);
                        log::warn!("Removing chunk {} at idx {}", chunk_no, idx_to_remove);
                        pc.unfinished_chunks.remove(idx_to_remove);

                        // remove from current chunks - it's easier, because there is only few of them
                        //pc.current_chunks.remove(&chunk_no);
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
