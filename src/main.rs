mod pipe_downloader;
mod lz4_decoder;

use anyhow;
use std::thread;
use std::time::Duration;
use human_bytes::human_bytes;

use crate::pipe_downloader::{PipeDownloader, PipeDownloaderOptions};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut options = PipeDownloaderOptions::default();
    options.chunk_size_decoder = 1024 * 1024;
    options.chunk_size_downloader = 1024 * 1024;
    let mut pd = PipeDownloader::new(
        "http://mumbai-main.golem.network:14372/beacon.tar.lz4",
        "beacon",
        options
    );
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        let progress = pd.get_progress()?;
        println!(
            "downloaded: {}, unpacked: {}",
            human_bytes((progress.total_downloaded + progress.chunk_downloaded) as f64),
            human_bytes(progress.total_unpacked as f64)
        );
        if pd.is_finished() {
            break;
        }
        let elapsed = current_time.elapsed();
        if elapsed.as_secs() > 30 {
            pd.pause_download();
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    pd.wait_for_finish();
    Ok(())
}
