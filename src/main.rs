mod pipe_downloader;

use std::thread;
use std::time::Duration;
use anyhow;

use crate::pipe_downloader::{convert, PipeDownloader};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut pd = PipeDownloader::new("http://mumbai-main.golem.network:14372/beacon.tar.lz4");
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        let progress = pd.get_progress()?;
        println!(
            "downloaded: {}, unpacked: {}",
            convert((progress.total_downloaded + progress.chunk_downloaded) as f64),
            convert(progress.total_unpacked as f64)
        );
        if pd.is_finished() {
            break;
        }
        let elapsed = current_time.elapsed();
        if elapsed.as_secs() > 30 {
            //pd.signal_stop();
            //break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    pd.wait_for_finish();
    Ok(())
}
