use std::io::{ErrorKind, Read};

pub struct MpscReaderFromReceiver {
    pos: usize,
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    current_buf: Vec<u8>,
    current_buf_pos: usize,
}

impl MpscReaderFromReceiver {
    pub fn new(receiver: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            pos: 0,
            receiver,
            current_buf: Vec::new(),
            current_buf_pos: 0,
        }
    }
}

impl Read for MpscReaderFromReceiver {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let starting_pos = self.pos;
        if self.current_buf.is_empty() || self.current_buf_pos >= self.current_buf.len() {
            self.current_buf = self.receiver.recv().map_err(|err| {
                std::io::Error::new(ErrorKind::InvalidData, format!("Receive error {:?}", err))
            })?;
            self.current_buf_pos = 0;
        }
        let min_val = std::cmp::min(self.current_buf.len() - self.current_buf_pos, buf.len());

        let src_slice = &self.current_buf[self.current_buf_pos..(self.current_buf_pos + min_val)];
        buf[0..min_val].copy_from_slice(src_slice);
        self.current_buf_pos += min_val;
        self.pos += min_val;

        log::debug!(
            "Chunk read: starting_pos: {} / length: {}",
            starting_pos,
            self.pos - starting_pos
        );
        return Ok(min_val);
    }
}
