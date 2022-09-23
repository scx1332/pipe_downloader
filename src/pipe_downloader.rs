use anyhow;
use flate2::read::GzDecoder;
use log;

use reqwest::header::CONTENT_LENGTH;
use reqwest::StatusCode;
use std::fs::File;
use std::io::Read;

use lazy_static::lazy_static;
use std::str::FromStr;
use std::sync::mpsc::sync_channel;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tar::Archive;

const CHUNK_SIZE_DOWNLOADER: usize = 30 * 1000 * 1000;
const CHUNK_SIZE_DECODER: usize = 10 * 1000 * 1000;

struct ProgressContext {
    total_downloaded: usize,
    total_unpacked: usize,
}

lazy_static! {
    static ref PROGRESS: Mutex<ProgressContext> = Mutex::new(ProgressContext {
        total_downloaded: 0,
        total_unpacked: 0,
    });
}

struct Pipe {
    pos: usize,
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    current_buf: Vec<u8>,
    current_buf_pos: usize,
    report_progress: usize,
    progress_message: String,
}

pub fn convert(num: f64) -> String {
    let negative = if num.is_sign_positive() { "" } else { "-" };
    let num = num.abs();
    let units = ["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    if num < 1_f64 {
        return format!("{}{} {}", negative, num, "B");
    }
    let delimiter = 1000_f64;
    let exponent = std::cmp::min(
        (num.ln() / delimiter.ln()).floor() as i32,
        (units.len() - 1) as i32,
    );
    let pretty_bytes = format!("{:.2}", num / delimiter.powi(exponent))
        .parse::<f64>()
        .unwrap()
        * 1_f64;
    let unit = units[exponent as usize];
    format!("{}{} {}", negative, pretty_bytes, unit)
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos > self.report_progress + 100000 {
            if !self.progress_message.is_empty() {
                log::debug!("{}: {}", self.progress_message, convert(self.pos as f64));
            }
            self.report_progress = self.pos;
        }
        let starting_pos = self.pos;
        if self.current_buf.is_empty() || self.current_buf_pos >= self.current_buf.len() {
            let res = self.receiver.recv();
            if res.is_err() {
                return Ok(0);
            }
            self.current_buf = res.unwrap();
            self.current_buf_pos = 0;
        }
        let min_val = std::cmp::min(self.current_buf.len() - self.current_buf_pos, buf.len());
        for i in 0..min_val {
            buf[i] = self.current_buf[self.current_buf_pos];
            self.current_buf_pos += 1;
            self.pos += 1;
        }
        log::debug!(
            "Chunk read: starting_pos: {} / length: {}",
            starting_pos,
            self.pos - starting_pos
        );
        return Ok(min_val);
    }
}

fn download_chunk(
    url: &str,
    client: reqwest::blocking::Client,
    range: std::ops::Range<usize>,
) -> anyhow::Result<Vec<u8>> {
    log::debug!(
        "Downloading chunk: range {:?} / {}",
        range,
        range.end - range.start
    );

    let header = format!("bytes={}-{}", range.start, range.end - 1);
    let mut response = client.get(url).header("Range", header).send()?;

    let status = response.status();
    let content_length = response
        .headers()
        .get("Content-Length")
        .ok_or(anyhow::anyhow!("Content-Length header not found"))?
        .to_str()?;
    let content_length = usize::from_str(content_length)?;

    if !(status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT) {
        return Err(anyhow::anyhow!("unexpected status code: {}", status));
    } else {
        log::info!("Chunk downloaded with status: {:?}", status);
    }
    let mut buf_vec: Vec<u8> = Vec::with_capacity(content_length);
    //let mut file = Cursor::new(buf_vec);
    //std::io::copy(&mut response, &mut file).unwrap();
    response.read_to_end(&mut buf_vec)?;

    assert!(content_length == range.end - range.start);
    assert!(buf_vec.len() == range.end - range.start);
    log::debug!(
        "Chunk downloaded: range {:?} / {}",
        range,
        range.end - range.start
    );

    return Ok(buf_vec);
}

fn decode_loop<T: Read>(
    decoder: &mut T,
    send: std::sync::mpsc::SyncSender<Vec<u8>>,
) -> anyhow::Result<()> {
    let mut unpacked_size = 0;
    loop {
        let mut buf = vec![0u8; CHUNK_SIZE_DECODER];
        let bytes_read = match decoder.read(&mut buf) {
            Ok(bytes_read) => bytes_read,
            Err(err) => {
                log::error!("Error while reading from lz4 decoder {:?}", err);
                break;
            }
        };
        if bytes_read == 0 {
            break;
        }
        unpacked_size += bytes_read;
        {
            let mut progress = PROGRESS.lock().unwrap();
            progress.total_unpacked = unpacked_size;
        }

        log::debug!(
            "Decode loop, Unpacked size: {}",
            convert(unpacked_size as f64)
        );
        buf.resize(bytes_read, 0);
        send.send(buf)?;
    }
    log::info!("Finishing decode loop");
    Ok(())
}

pub fn download() -> anyhow::Result<()> {
    let url = "http://mumbai-main.golem.network:14372/beacon.tar.lz4";
    //let url = "https://github.com/golemfactory/ya-runtime-http-auth/releases/download/v0.1.0/ya-runtime-http-auth-linux-v0.1.0.tar.gz";

    let client = reqwest::blocking::Client::new();
    let response = client.head(url).send()?;
    let length = response
        .headers()
        .get(CONTENT_LENGTH)
        .ok_or("response doesn't include the content length")
        .unwrap();
    let length = usize::from_str(length.to_str()?)
        .map_err(|_| "invalid Content-Length header")
        .unwrap();

    let _output_file = File::create("download.bin")?;

    log::info!("starting download...");
    let (send, recv) = sync_channel(1);
    /*for range in PartialRangeIter::new(0, length - 1, CHUNK_SIZE)? {
        println!("range {:?} / {}", range, length);
        let mut response = client.get(url).header(RANGE, range).send()?;

        let status = response.status();
        if !(status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT) {
            anyhow::anyhow!("unexpected status code: {}", status);
        }
        std::io::copy(&mut response, &mut output_file)?;
    }*/

    let t1 = thread::spawn(move || {
        let chunk_size = CHUNK_SIZE_DOWNLOADER;
        for i in 0..(length / chunk_size + 1) {
            let max_length = std::cmp::min(chunk_size, length - i * chunk_size);
            if max_length == 0 {
                break;
            }
            let range = std::ops::Range {
                start: i * chunk_size,
                end: i * chunk_size + max_length,
            };
            let client = reqwest::blocking::Client::new();

            {
                let mut progress = PROGRESS.lock().unwrap();
                progress.total_downloaded = i * chunk_size;
            }
            let current_buf = download_chunk(url, client, range).unwrap();
            send.send(current_buf).unwrap();
        }
        println!("Finishing thread 1");
    });
    let mut p = Pipe {
        pos: 0,
        receiver: recv,
        current_buf: vec![],
        current_buf_pos: 0,
        report_progress: 0,
        progress_message: "Downloading".to_string(),
    };

    let (send2, recv2) = sync_channel(1);

    let t2 = thread::spawn(move || {
        /*let decoder = if url.ends_with(".gz") {
            GzDecoder::new(&mut p)
        } else if url.ends_with(".lz4") {*/
        // lz4::Decoder::new(&mut p).unwrap();
        /*        } else {
            panic!("Unknown file type");
        };*/
        if url.ends_with(".gz") {
            let mut gz = GzDecoder::new(&mut p);
            decode_loop(&mut gz, send2).unwrap();
        } else if url.ends_with(".lz4") {
            let mut lz4 = lz4::Decoder::new(&mut p).unwrap();
            decode_loop(&mut lz4, send2).unwrap();
        } else {
            panic!("Unknown file type");
        };
    });

    let p2 = Pipe {
        pos: 0,
        receiver: recv2,
        current_buf: vec![],
        current_buf_pos: 0,
        report_progress: 0,
        progress_message: "Unpacking".to_string(),
    };

    //let mut br = BufReader::new(p2);
    //std::io::copy(&mut br, &mut output_file)?;
    let t3 = thread::spawn(move || {
        let mut archive = Archive::new(p2);
        archive.unpack("download").unwrap();
    });
    loop {
        {
            let progress = PROGRESS.lock().unwrap();
            println!(
                "downloaded: {}, unpacked: {}",
                convert(progress.total_downloaded as f64),
                convert(progress.total_unpacked as f64)
            );
            if t3.is_finished() {
                break;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    t1.join().unwrap();
    t2.join().unwrap();

    //while let Ok(current_buf) = recv.recv() {
    //std::io::copy(&mut p, &mut output_file)?;
    //}
    //let mut br = BufReader::new(p);
    //let content = response.text()?;
    //std::io::copy(&mut content.as_bytes(), &mut output_file)?;

    println!("Finished with success!");
    Ok(())
}
