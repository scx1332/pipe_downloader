#![allow(clippy::redundant_closure)]
#[deny(missing_docs)]

mod options;
mod pipe_downloader;
mod pipe_engine;
mod pipe_progress;
mod pipe_utils;
mod pipe_wrapper;
mod tsutils;

pub use options::PipeDownloaderOptions;
pub use crate::pipe_downloader::PipeDownloader;
pub use pipe_progress::PipeDownloaderProgress;
