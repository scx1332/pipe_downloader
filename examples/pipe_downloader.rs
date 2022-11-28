use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use pipe_downloader::{PipeDownloader, PipeDownloaderOptions};

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "Options for pipe downloader")]
struct Opt {
    /// Url of tar.gz or tar.lz4 file
    #[structopt(long = "url")]
    url: String,

    /// Output directory
    #[structopt(short = "o", long = "output-dir", parse(from_os_str))]
    output_dir: PathBuf,

    /// Max bytes downloaded per seconds
    #[structopt(long = "limit-speed")]
    limit_speed: Option<usize>,

    /// Number of download threads/connections
    #[structopt(long = "download-threads", default_value = "2")]
    download_threads: usize,

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

    /// For debugging purposes
    #[structopt(long = "force-no-partial-content")]
    force_no_partial_content: bool,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: Opt = Opt::from_args();

    let mut options = PipeDownloaderOptions::from_env();
    options.chunk_size_decoder = opt.unpack_buffer;
    options.chunk_size_downloader = opt.download_buffer;
    options.max_download_speed = opt.limit_speed;
    options.force_no_chunks = opt.force_no_partial_content;
    options.download_threads = opt.download_threads;
    let mut pd = PipeDownloader::new(&opt.url, &opt.output_dir, options);
    pd.start_download()?;
    let current_time = std::time::Instant::now();
    loop {
        if opt.json_output {
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
