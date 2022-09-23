mod pipe_downloader;

use anyhow;

use crate::pipe_downloader::download;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    download()?;
    Ok(())
}
