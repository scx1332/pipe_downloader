mod lz4_decoder;
mod pipe_downloader;
mod pipe_progress;
mod pipe_utils;
mod pipe_wrapper;

use anyhow;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::pipe_downloader::{PipeDownloader, PipeDownloaderOptions};

use crate::pipe_utils::bytes_to_human;
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

    /// Max bytes downloaded per seconde
    #[structopt(long = "limit-speed")]
    limit_speed: Option<usize>,

    /// Size of download buffer in bytes
    #[structopt(long = "download-buffer", default_value = "30000000")]
    download_buffer: usize,

    /// Size of unpack buffer in bytes
    #[structopt(long = "unpack-buffer", default_value = "10000000")]
    unpack_buffer: usize,

    /// Set output in json format
    #[structopt(long = "json-output")]
    json_output: bool,

    /// For debugging purposes
    #[structopt(long = "run-after-finish")]
    run_after_finish: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: Opt = Opt::from_args();

    let mut options = PipeDownloaderOptions::default();
    options.chunk_size_decoder = opt.unpack_buffer;
    options.chunk_size_downloader = opt.download_buffer;
    options.max_download_speed = opt.limit_speed;
    let mut pd = PipeDownloader::new(&opt.url, &opt.output_dir, options);
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        let progress = pd.get_progress()?;
        if opt.json_output {
            println!("{}", serde_json::to_string_pretty(&progress ).unwrap());
        } else {
            println!(
                "downloaded: {} speed[current: {}/s total: {}/s], unpacked: {} [current: {}/s total: {}/s]",
                bytes_to_human(progress.total_downloaded + progress.chunk_downloaded),
                bytes_to_human(progress.progress_buckets_download.get_speed()),
                progress.get_download_speed_human(),
                bytes_to_human(progress.total_unpacked),
                bytes_to_human(progress.progress_buckets_unpack.get_speed()),
                progress.get_unpack_speed_human(),
            );
        }
        if !opt.run_after_finish && pd.is_finished() {
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
    println!("Unpack finished in: {:?}", elapsed);
    Ok(())
}
