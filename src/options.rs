use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Pipe downloader",
    about = "Fast multithreaded downloader for tar.lz4, tar.gz, tar.bz2 files"
)]
pub struct CliOptions {
    /// Url of tar.gz or tar.lz4 file
    #[structopt(long = "url")]
    pub url: String,

    /// Output directory
    #[structopt(short = "o", long = "output-dir", parse(from_os_str))]
    pub output_dir: PathBuf,

    /// Max bytes downloaded per seconds per one thread
    #[structopt(long = "limit-speed")]
    pub limit_speed: Option<usize>,

    /// Number of download threads/connections
    /// You can improve download speed by increasing this number,
    /// note that this will also increase memory usage
    #[structopt(short = "t", long = "download-threads", default_value = "4")]
    pub download_threads: usize,

    /// Size of download buffer in bytes, if memory is an issue, reduce this value
    /// If the download is slow, you can use smaller value and increase download threads.
    /// For the fast downloads buffer should be big to improve performance.
    #[structopt(long = "download-buffer", default_value = "30000000")]
    pub download_buffer: usize,

    /// Size of unpack buffer in bytes, better left unchanged
    #[structopt(long = "unpack-buffer", default_value = "10000000")]
    pub unpack_buffer: usize,

    /// Set output in json format (default is human readable)
    #[structopt(long = "json")]
    pub json: bool,

    /// For server
    #[structopt(long = "wait-after-finish-sec", default_value = "10")]
    pub wait_after_finish_sec: u64,

    /// Force only one connection (like when no partial content header is supported by header)
    #[structopt(long = "force-no-partial-content")]
    pub force_no_partial_content: bool,

    /// No web UI/backend
    #[structopt(long = "cli-only")]
    pub cli_only: bool,

    /// Listen address
    #[structopt(long, default_value = "127.0.0.1")]
    pub listen_addr: String,

    /// Listen port
    #[structopt(long, default_value = "15100")]
    pub listen_port: u16,

    #[structopt(long = "add-cors")]
    pub add_cors: bool,
}
