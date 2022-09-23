mod pipe_downloader;

use anyhow;
use std::thread;
use std::time::Duration;

use crate::pipe_downloader::{convert_bytes_to_human, PipeDownloader};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut pd = PipeDownloader::new(
        "http://mumbai-main.golem.network:14372/beacon.tar.lz4",
        "beacon",
    );
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        let progress = pd.get_progress()?;
        println!(
            "downloaded: {}, unpacked: {}",
            convert_bytes_to_human(progress.total_downloaded + progress.chunk_downloaded),
            convert_bytes_to_human(progress.total_unpacked)
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
