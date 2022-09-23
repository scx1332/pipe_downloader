use anyhow;
use flate2::read::GzDecoder;
use log;
use lz4::{Decoder, EncoderBuilder};
use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, CONTENT_LENGTH, RANGE};
use reqwest::StatusCode;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::ptr::addr_of_mut;
use std::str::FromStr;
use std::sync::mpsc::channel;
use std::thread;
use tar::Archive;

struct PartialRangeIter {
    start: u64,
    end: u64,
    buffer_size: u32,
}

impl PartialRangeIter {
    pub fn new(start: u64, end: u64, buffer_size: u32) -> anyhow::Result<Self> {
        if buffer_size == 0 {
            anyhow::anyhow!("invalid buffer_size, give a value greater than zero.");
        }
        Ok(PartialRangeIter {
            start,
            end,
            buffer_size,
        })
    }
}

impl Iterator for PartialRangeIter {
    type Item = HeaderValue;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start > self.end {
            None
        } else {
            let prev_start = self.start;
            self.start += std::cmp::min(self.buffer_size as u64, self.end - self.start + 1);
            Some(
                HeaderValue::from_str(&format!("bytes={}-{}", prev_start, self.start - 1))
                    .expect("string provided by format!"),
            )
        }
    }
}

/*
fn decompress(source: &Path, destination: &Path) -> Result<()> {
    println!("Decompressing: {} -> {}", source.display(), destination.display());

    let mut decoder = Decoder::new(input_file)?;
    let mut output_file = File::create(destination)?;
    io::copy(&mut decoder, &mut output_file)?;

    Ok(())
}*/

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
        if self.pos > self.report_progress + 10000000 {
            if !self.progress_message.is_empty() {
                println!("{}: {}", self.progress_message, convert(self.pos as f64));
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
        anyhow::anyhow!("unexpected status code: {}", status);
    } else {
        log::info!("Chunk downloaded with status: {:?}", status);
    }
    let mut buf_vec: Vec<u8> = vec![];
    //let mut file = Cursor::new(buf_vec);
    //std::io::copy(&mut response, &mut file).unwrap();
    response.read_to_end(&mut buf_vec)?;

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
    send: std::sync::mpsc::Sender<Vec<u8>>,
) -> anyhow::Result<()> {
    let CHUNK_SIZE = 100000;
    let mut unpacked_size = 0;
    loop {
        let mut buf = vec![0u8; CHUNK_SIZE];
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

        println!("Unpacked_size: {}", convert(bytes_read as f64));
        buf.resize(bytes_read, 0);
        send.send(buf)?;
    }
    println!("Finishing thread 2");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    //let url = "http://mumbai-main.golem.network:14372/beacon.tar.lz4";
    let url = "https://github.com/golemfactory/ya-runtime-http-auth/releases/download/v0.1.0/ya-runtime-http-auth-linux-v0.1.0.tar.gz";
    const CHUNK_SIZE: usize = 100000;

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

    let mut output_file = File::create("download.bin")?;

    println!("starting download...");
    let (send, recv) = channel();
    /*for range in PartialRangeIter::new(0, length - 1, CHUNK_SIZE)? {
        println!("range {:?} / {}", range, length);
        let mut response = client.get(url).header(RANGE, range).send()?;

        let status = response.status();
        if !(status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT) {
            anyhow::anyhow!("unexpected status code: {}", status);
        }
        std::io::copy(&mut response, &mut output_file)?;
    }*/

    thread::spawn(move || {
        for i in 0..(length / CHUNK_SIZE + 1) {
            let max_length = std::cmp::min(CHUNK_SIZE, length - i * CHUNK_SIZE);
            if max_length == 0 {
                break;
            }
            let range = std::ops::Range {
                start: i * CHUNK_SIZE,
                end: i * CHUNK_SIZE + max_length,
            };
            let client = reqwest::blocking::Client::new();

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

    let (send2, recv2) = channel();

    thread::spawn(move || {
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

    let mut p2 = Pipe {
        pos: 0,
        receiver: recv2,
        current_buf: vec![],
        current_buf_pos: 0,
        report_progress: 0,
        progress_message: "Unpacking".to_string(),
    };

    let mut archive = Archive::new(p2);
    archive.unpack("download")?;
    println!("Finishing main thread");
    thread::sleep(std::time::Duration::from_secs(100));

    //while let Ok(current_buf) = recv.recv() {
    //std::io::copy(&mut p, &mut output_file)?;
    //}
    //let mut br = BufReader::new(p);
    //let content = response.text()?;
    //std::io::copy(&mut content.as_bytes(), &mut output_file)?;

    println!("Finished with success!");
    Ok(())
}
