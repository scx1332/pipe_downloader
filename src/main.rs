mod options;

use anyhow;
use pipe_downloader_lib::PipeDownloaderOptions;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use crate::options::CliOptions;
use structopt::StructOpt;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: CliOptions = CliOptions::from_args();

    let pd = PipeDownloaderOptions {
        chunk_size_decoder: opt.unpack_buffer,
        chunk_size_downloader: opt.download_buffer,
        max_download_speed: opt.limit_speed,
        force_no_chunks: opt.force_no_partial_content,
        download_threads: opt.download_threads,
    }
    .start_download(&opt.url, &opt.output_dir)?;

    let current_time = std::time::Instant::now();
    loop {
        if opt.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&pd.get_progress()).unwrap()
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
