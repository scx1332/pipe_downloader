mod lz4_decoder;
mod pipe_downloader;

use anyhow;
use human_bytes::human_bytes;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::pipe_downloader::{PipeDownloader, PipeDownloaderOptions};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// Url of tar.gz or tar.lz4 file
    #[structopt(long = "url")]
    url: String,

    /// Output directory
    #[structopt(short = "o", long = "output-dir", parse(from_os_str))]
    output_dir: PathBuf,

    /// Size of download buffer in bytes
    #[structopt(long = "download-buffer", default_value = "30000000")]
    download_buffer: usize,

    /// Size of unpack buffer in bytes
    #[structopt(long = "unpack-buffer", default_value = "10000000")]
    unpack_buffer: usize,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: Opt = Opt::from_args();

    let mut options = PipeDownloaderOptions::default();
    options.chunk_size_decoder = opt.unpack_buffer;
    options.chunk_size_downloader = opt.download_buffer;
    let mut pd = PipeDownloader::new(&opt.url, &opt.output_dir, options);
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        let progress = pd.get_progress()?;
        println!(
            "downloaded: {} speed[current: {}/s total: {}/s], unpacked: {} [current: {}/s total: {}/s]",
            human_bytes((progress.total_downloaded + progress.chunk_downloaded) as f64),
            human_bytes(progress.progress_buckets_download.get_speed()),
            progress.get_download_speed_human(),
            human_bytes(progress.total_unpacked as f64),
            human_bytes(progress.progress_buckets_unpack.get_speed()),
            progress.get_unpack_speed_human(),
        );
        if pd.is_finished() {
            break;
        }

        // test pause
        // if elapsed.as_secs() > 30 {
        //     pd.pause_download();
        //     break;
        // }
        thread::sleep(Duration::from_millis(1000));
    }
    let elapsed = current_time.elapsed();
    pd.wait_for_finish();
    println!("Unpack finished in: {:?}", elapsed);
    Ok(())
}
