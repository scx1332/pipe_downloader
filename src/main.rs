mod pipe_downloader;

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
use std::sync::mpsc::sync_channel;
use std::thread;
use lazy_static::lazy_static;
use tar::Archive;
use std::sync::Mutex;
use std::time::Duration;
use crate::pipe_downloader::download;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    download()?;
    Ok(())
}
