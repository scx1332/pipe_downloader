use chrono::{Duration, Utc};
use serde_json::json;
use std::ops::Div;

#[derive(Debug, Clone)]
pub struct ProgressHistoryEntry {
    time: chrono::DateTime<Utc>,
    bytes: usize,
}
#[derive(Debug, Clone)]
pub struct ProgressHistory {
    progress_entries: Vec<ProgressHistoryEntry>,
    max_entries: usize,
    keep_time: Duration,
}
impl Default for ProgressHistory {
    fn default() -> Self {
        Self {
            progress_entries: Vec::new(),
            max_entries: 10,
            keep_time: Duration::seconds(1),
        }
    }
}
impl ProgressHistory {
    pub fn new() -> ProgressHistory {
        ProgressHistory {
            progress_entries: vec![],
            max_entries: 50,
            keep_time: Duration::seconds(10),
        }
    }

    pub fn get_speed(&self) -> usize {
        //log::warn!("First enty from {}", self.progress_entries.get(0).map(|entry| std::time::Instant::now() - entry.time).unwrap_or());
        let current_time = chrono::Utc::now();
        let mut last_time = current_time - self.keep_time;
        let mut total: usize = 0;
        //let now = std::time::Instant::now();
        for entry in self.progress_entries.iter().rev() {
            if current_time - entry.time > self.keep_time {
                break;
            }
            total += entry.bytes;
            last_time = entry.time;
        }
        let elapsed_secs = (chrono::Utc::now() - last_time).num_milliseconds() as f64 / 1000.0;
        log::trace!("Progress entries count {}", self.progress_entries.len());
        log::trace!("Last entry: {}", elapsed_secs);

        (total as f64 / elapsed_secs).round() as usize
    }

    pub fn add_bytes(self: &mut Self, bytes: usize) {
        let current_time = chrono::Utc::now();
        if let Some(last_entry) = self.progress_entries.last_mut() {
            if current_time - last_entry.time < self.keep_time.div(self.max_entries as i32) {
                last_entry.bytes += bytes;
                return;
            }
        }
        self.progress_entries.push(ProgressHistoryEntry {
            time: current_time,
            bytes: bytes,
        });

        //this should be removed max one time
        assert!(self.progress_entries.len() <= self.max_entries + 1);
        while self.progress_entries.len() > self.max_entries {
            //log::warn!("ProgressHistory: max_entries reached");
            self.progress_entries.remove(0);
        }

        //remove old entries
        while let Some(first) = self.progress_entries.first() {
            if current_time - first.time > self.keep_time {
                self.progress_entries.remove(0);
            } else {
                break;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressContext {
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub total_downloaded: usize,
    pub total_download_size: Option<usize>,
    pub chunk_downloaded: usize,
    pub total_unpacked: usize,
    pub total_unpack_size: Option<usize>,
    pub stop_requested: bool,
    pub paused: bool,
    pub progress_buckets_download: ProgressHistory,
    pub progress_buckets_unpack: ProgressHistory,
    pub finish_time: Option<chrono::DateTime<chrono::Utc>>,
    pub error_time: Option<chrono::DateTime<chrono::Utc>>,
    pub error_message_download: Option<String>,
    pub error_message_unpack: Option<String>,
    pub error_message: Option<String>,
}

impl Default for ProgressContext {
    fn default() -> ProgressContext {
        ProgressContext {
            start_time: chrono::Utc::now(),
            total_download_size: None,
            total_downloaded: 0,
            chunk_downloaded: 0,
            total_unpacked: 0,
            total_unpack_size: None,
            stop_requested: false,
            paused: false,
            progress_buckets_download: ProgressHistory::new(),
            progress_buckets_unpack: ProgressHistory::new(),
            finish_time: None,
            error_time: None,
            error_message: None,
            error_message_download: None,
            error_message_unpack: None,
        }
    }
}

impl ProgressContext {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "startTime": self.start_time.to_rfc3339(),
            "downloaded": self.total_downloaded + self.chunk_downloaded,
            "unpacked": self.total_unpacked,
            "stopRequested": self.stop_requested,
            "paused": self.paused,
            "elapsedTime": self.get_elapsed().num_milliseconds() as f64 / 1000.0,
            "estimatedTimeLeft": self.get_time_left_sec(),
            "finishTime": self.finish_time.map(|ft| ft.to_rfc3339()),
            "currentDownloadSpeed": self.progress_buckets_download.get_speed(),
            "currentUnpackSpeed": self.progress_buckets_unpack.get_speed(),
            "errorMessage": self.error_message,
            "errorMessageDownload": self.error_message_download,
            "errorMessageUnpack": self.error_message_unpack,
            "totalUnpackSize": self.total_unpack_size,
            "totalDownloadSize": self.total_download_size,
        })
    }

    pub fn get_elapsed(&self) -> chrono::Duration {
        self.finish_time.unwrap_or(chrono::Utc::now()) - self.start_time
    }

    pub fn get_time_left_sec(&self) -> Option<u64> {
        if self.finish_time.is_some() {
            return Some(0);
        }
        let download_speed = self.get_download_speed();
        if download_speed < 100 {
            return None;
        }
        if let Some(total_download_size) = self.total_download_size {
            let seconds_left =
                (total_download_size - self.total_downloaded - self.chunk_downloaded)
                    / download_speed;
            return Some(seconds_left as u64);
        }
        None
    }

    pub fn get_download_speed(&self) -> usize {
        if self.finish_time.is_some() {
            return 0;
        }
        let elapsed = self.get_elapsed();
        if elapsed.num_milliseconds() == 0 {
            return 0;
        }
        let res_f64 = (self.total_downloaded + self.chunk_downloaded) as f64
            / (elapsed.num_milliseconds() as f64 / 1000.0);
        res_f64.round() as usize
    }
    pub fn get_unpack_speed(&self) -> usize {
        if self.finish_time.is_some() {
            return 0;
        }
        let elapsed = self.get_elapsed();
        if elapsed.num_milliseconds() == 0 {
            return 0;
        }
        let speed_f64 = (self.total_unpacked) as f64 / (elapsed.num_milliseconds() as f64 / 1000.0);
        speed_f64.round() as usize
    }
}
