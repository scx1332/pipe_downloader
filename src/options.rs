use crate::PipeDownloader;
use std::path::Path;
use std::str::FromStr;

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
    pub fn apply_env(self) -> Self {
        let mut options = self;
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

    pub fn start_download(self, url: &str, target_path: &Path) -> anyhow::Result<PipeDownloader> {
        let mut pd = PipeDownloader::new(url, target_path, self);
        pd.start_download()?;
        Ok(pd)
    }
}
