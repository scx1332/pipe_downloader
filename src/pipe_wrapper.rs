use std::io::{ErrorKind, Read};

pub struct DataChunk {
    pub data: Vec<u8>,
    pub range: std::ops::Range<usize>,
}

pub struct MpscReaderFromReceiver {
    pos: usize,
    receiver: std::sync::mpsc::Receiver<DataChunk>,
    current_buf: Vec<u8>,
    current_buf_pos: usize,
    chunk_waiting_list: Vec<DataChunk>,
    debug: bool,
}

impl MpscReaderFromReceiver {
    pub fn new(receiver: std::sync::mpsc::Receiver<DataChunk>, debug: bool) -> Self {
        Self {
            pos: 0,
            receiver,
            current_buf: Vec::new(),
            current_buf_pos: 0,
            chunk_waiting_list: Vec::new(),
            debug,
        }
    }
}

impl Read for MpscReaderFromReceiver {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let starting_pos = self.pos;
        if self.current_buf.is_empty() || self.current_buf_pos >= self.current_buf.len() {
            let mut found_idx = None;
            for (idx, chunk) in self.chunk_waiting_list.iter().enumerate() {
                if chunk.range.start == self.pos {
                    if self.debug {
                        log::warn!("Found compatible chunk from waiting list {}", self.pos);
                    }
                    found_idx = Some(idx);
                    break;
                }
            }
            if let Some(found_idx) = found_idx {
                let dt = self.chunk_waiting_list.swap_remove(found_idx);
                self.current_buf = dt.data;
                self.current_buf_pos = 0;
            }
            loop {
                let new_chunk = self.receiver.recv().map_err(|err| {
                    std::io::Error::new(ErrorKind::InvalidData, format!("Receive error {:?}", err))
                })?;
                if new_chunk.range.start == self.pos {
                    if self.debug {
                        log::warn!("Found compatible chunk {}", self.pos);
                    }
                    self.current_buf = new_chunk.data;
                    self.current_buf_pos = 0;
                    break;
                } else {
                    if self.debug {
                        log::warn!("Found incompatible chunk, adding to waiting list {}", self.pos);
                    }
                    self.chunk_waiting_list.push(new_chunk);
                }
            }

        }
        let min_val = std::cmp::min(self.current_buf.len() - self.current_buf_pos, buf.len());

        let src_slice = &self.current_buf[self.current_buf_pos..(self.current_buf_pos + min_val)];
        buf[0..min_val].copy_from_slice(src_slice);
        self.current_buf_pos += min_val;
        self.pos += min_val;

        log::trace!(
            "Chunk read: starting_pos: {} / length: {}",
            starting_pos,
            self.pos - starting_pos
        );
        return Ok(min_val);
    }
}
