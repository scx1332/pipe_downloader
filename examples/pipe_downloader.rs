use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use pipe_downloader::{PipeDownloaderOptions};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Pipe downloader", about = "Fast multithreaded downloader for tar.lz4, tar.gz, tar.bz2 files")]
struct Opt {
    /// Url of tar.gz or tar.lz4 file
    #[structopt(long = "url")]
    url: String,

    /// Output directory
    #[structopt(short = "o", long = "output-dir", parse(from_os_str))]
    output_dir: PathBuf,

    /// Max bytes downloaded per seconds per one thread
    #[structopt(long = "limit-speed")]
    limit_speed: Option<usize>,

    /// Number of download threads/connections
    /// You can improve download speed by increasing this number,
    /// note that this will also increase memory usage
    #[structopt(short = "t", long = "download-threads", default_value = "4")]
    download_threads: usize,

    /// Size of download buffer in bytes, if memory is an issue, reduce this value
    /// If the download is slow, you can use smaller value and increase download threads.
    /// For the fast downloads buffer should be big to improve performance.
    #[structopt(long = "download-buffer", default_value = "30000000")]
    download_buffer: usize,

    /// Size of unpack buffer in bytes, better left unchanged
    #[structopt(long = "unpack-buffer", default_value = "10000000")]
    unpack_buffer: usize,

    /// Set output in json format (default is human readable)
    #[structopt(long = "json")]
    json: bool,

    /// For debugging purposes
    #[structopt(long = "run-after-finish")]
    run_after_finish: bool,

    /// Force only one connection (like when no partial content header is supported by header)
    #[structopt(long = "force-no-partial-content")]
    force_no_partial_content: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: Opt = Opt::from_args();

    let mut pd = PipeDownloaderOptions {
        chunk_size_decoder: opt.unpack_buffer,
        chunk_size_downloader: opt.download_buffer,
        max_download_speed: opt.limit_speed,
        force_no_chunks: opt.force_no_partial_content,
        download_threads: opt.download_threads,
    }
    .apply_env()
    .create_downloader(&opt.url, &opt.output_dir);
    pd.start_download()?;

    let current_time = std::time::Instant::now();
    loop {
        if opt.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&pd.get_progress_json()).unwrap()
            )
        } else {
            println!("{}", pd.get_progress_human_line());
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
