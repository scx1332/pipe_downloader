use crate::world_time::{init_world_time, world_time};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use structopt::StructOpt;
use sha2::{Sha256, Sha512, Digest};
use sha256::digest;
use sha2::digest::FixedOutput;
use serde::{Serialize, Deserialize};

pub const START_TEST_HEADER: [u8; 8] = [105,  93,  84, 170,  59, 220, 179, 253 ];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartTest {
    pub msg_header: [u8; 8],
    pub packet_size: u16,
    pub packet_count: usize,
    pub hash: [u8; 32],
}

impl StartTest {
    pub fn new(packet_size: u16, packet_count: usize) -> StartTest {
        let mut st = StartTest {
            msg_header: START_TEST_HEADER,
            packet_size,
            packet_count,
            hash: [0; 32],
        };
        st.self_digest();
        st
    }

    fn digest(&self) -> [u8; 32] {
        //just change the salt to make sure that program is not working with previous versions
        const SALT:[u8;10] = [12, 84, 233, 11, 110, 192, 211, 1, 88, 137];
        let mut s = Sha256::new();
        s.update(SALT);
        s.update(self.msg_header);
        s.update(self.packet_size.to_be_bytes());
        s.update(self.packet_count.to_be_bytes());
        s.finalize_fixed().into()
    }

    pub(crate) fn self_verify(&self) -> bool {
        self.digest() == self.hash
    }

    fn self_digest(&mut self) {
        let mut s = Sha256::new();
        self.hash = self.digest();
    }
}