use flate2::read::GzDecoder;
use human_bytes::human_bytes;

use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use std::str::FromStr;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use lz4::Decoder as Lz4Decoder;

use crate::options::PipeDownloaderOptions;
use anyhow::anyhow;
use bzip2::read::BzDecoder;
use lz4_flex::frame::FrameDecoder;
use reqwest::blocking::Response;
use std::time::Duration;
use tar::Archive;

use crate::pipe_progress::InternalProgress;
use crate::pipe_wrapper::{DataChunk, MpscReaderFromReceiver};
use crate::PipeDownloaderProgress;

pub fn bytes_to_human(bytes: usize) -> String {
    human_bytes(bytes as f64)
}

pub fn resolve_url(
    download_url: String,
    pc: Arc<Mutex<InternalProgress>>,
) -> anyhow::Result<String> {
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
