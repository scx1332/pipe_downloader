use anyhow::anyhow;
use reqwest::header::{HeaderMap, HeaderValue};
use std::io::Read;
use std::str::FromStr;

fn main() -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::new();
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    let mut headers = HeaderMap::new();
    headers.insert(
        "User-Agent",
        HeaderValue::from_str(&format!("pipe_downloader/{VERSION}")).unwrap(),
    );

    let download_url = "https://snapshot-download.polygon.technology/erigon-mumbai-archive-snapshot-2023-02-14.tar.gz";

    let mut response = client.get(download_url).headers(headers).send()?;

    let status = response.status();
    let content_length = response
        .headers()
        .get("Content-Length")
        .ok_or_else(|| anyhow::anyhow!("Content-Length header not found"))?
        .to_str()?;

    let mut buf = vec![0; 30 * 1024 * 1024];
    let content_length = usize::from_str(content_length)?;
    let mut total_downloaded: usize = 0;
    let start_time = std::time::Instant::now();
    let mut buf_vec: Vec<u8> = Vec::with_capacity(30 * 1024 * 1024);

    let mut buf = vec![0; 1024 * 1024];

    loop {
        let left_to_download = content_length - total_downloaded;
        let max_buf_size = std::cmp::min(buf.len(), left_to_download);
        if max_buf_size == 0 {
            break;
        }
        let n = response.read(&mut buf[..max_buf_size])?;
        buf_vec.extend_from_slice(&buf[..n]);
        if n == 0 {
            return Err(anyhow!("Unexpected end of file"));
        }
        total_downloaded += n;
        if buf_vec.len() > 20 * 1024 * 1024 {
            println!("{} MB downloaded", total_downloaded / 1024 / 1024);
            buf_vec.clear();
        }
    }
    Ok(())
}
