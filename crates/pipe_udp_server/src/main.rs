mod world_time;
mod commands;

use std::env;
use crate::world_time::{init_world_time, world_time};
use std::net::{SocketAddr, UdpSocket};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use structopt::StructOpt;
use sha2::{Sha256, Sha512, Digest};
use sha256::digest;
use sha2::digest::FixedOutput;
use serde::{Serialize, Deserialize};
use crate::commands::{START_TEST_HEADER, StartTest};

#[derive(StructOpt, Debug)]
struct Opt {
    /// Is server
    #[structopt(long)]
    pub is_server: bool,

    /// Listen address
    #[structopt(long, default_value = "127.0.0.1")]
    pub listen_addr: String,

    /// Listen port
    #[structopt(long, default_value = "11500")]
    pub listen_port: u16,

    /// Connect address
    #[structopt(long, default_value = "127.0.0.1")]
    pub connect_addr: String,

    /// Connect port
    #[structopt(long, default_value = "11501")]
    pub connect_port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ServerStats {
    pub bytes_received: u64,
    pub packets_received: u64,
    pub packet_count: u64,
}


fn test_send_loop(packet_size: u16, packet_count: usize, sock: UdpSocket, addr: std::net::SocketAddr) {
    const RATE_CHECKS_PER_SEC: usize = 20;
    let mut buf = vec![0; packet_size as usize];

    log::info!("Starting send loop on {:?}", sock.local_addr().unwrap());
    let mut packet_no: usize = 0;
    let packet_rate = 1000;
    let mut last_update = std::time::Instant::now();
    let mut packets_sent = 0;

    loop {
        if last_update.elapsed().as_secs_f64() > 1.0 / RATE_CHECKS_PER_SEC as f64 {
            last_update = std::time::Instant::now();
            packets_sent = 0;
        }
        if packets_sent >= packet_rate / RATE_CHECKS_PER_SEC {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        packet_no += 1;
        buf[0..8].copy_from_slice(&packet_no.to_be_bytes());


        sock.send_to(&buf, addr).unwrap();
        packets_sent += 1;

    }
}

fn receive_udp(sock: UdpSocket, stats: Arc<Mutex<ServerStats>>) -> std::io::Result<()> {
    const SMALL_BUF: usize = 90000;
    let mut buf = Box::new(vec![0; SMALL_BUF]);
    let mut local_stats = ServerStats {
        bytes_received: 0,
        packets_received: 0,
        packet_count: 0,
    };
    log::info!("Starting receive loop on {:?}", sock.local_addr()?);
    let mut last_update = std::time::Instant::now();
    loop {
        let (len, addr) = sock.recv_from(&mut buf)?;

        if buf[0..8] == START_TEST_HEADER {
            match bincode::deserialize::<StartTest>(&buf[8..len]) {
                Ok(start_test) => {
                    if start_test.self_verify() {
                        log::info!("Starting test: {:?} with addr {}", start_test, addr);
                        let sock_clone = sock.try_clone().unwrap();
                        std::thread::spawn(move||{
                            test_send_loop(
                                start_test.packet_size,
                                start_test.packet_count,
                                sock_clone,
                                addr,
                            );
                        });
                        continue;
                    } else {
                        log::error!("Start test verification failed");
                    }
                }
                Err(e) => {
                    log::error!("Error deserializing start test: {}", e);
                }
            }
        }



        local_stats.bytes_received += len as u64;
        local_stats.packets_received += 1;
        let packet_count = usize::from_be_bytes(buf[0..8].try_into().unwrap()) as u64;
        if local_stats.packet_count < packet_count {
            local_stats.packet_count = packet_count;
        }

        //{
            //update value behind lock only sometimes to improve perf
            *stats.lock().unwrap() = local_stats;
            last_update = std::time::Instant::now();
        //}
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let opt: Opt = Opt::from_args();
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }
    env_logger::init();

    init_world_time();
    let world_time = world_time();
    log::info!("World time without fix: {}", chrono::Utc::now());
    log::info!("World time: {}", world_time.utc_time());
    log::info!("World time without fix: {}", chrono::Utc::now());


    //if opt.is_server {
    log::info!("Listening on {}:{}", opt.listen_addr, opt.listen_port,);
    let sock = UdpSocket::bind(format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap();
    let server_stats = Arc::new(Mutex::new(ServerStats {
        bytes_received: 0,
        packets_received: 0,
        packet_count: 0,
    }));

    let server_stats_ = server_stats.clone();
    let sock_ = sock.try_clone().unwrap();
    let _thread = std::thread::spawn(move || match receive_udp(sock_, server_stats_) {
        Ok(_) => log::info!("UDP received thread end"),
        Err(e) => log::error!("UDP error: {}", e),
    });

    if !opt.is_server {
        let mut start_test = StartTest::new(1100, 10000);
        let mut buf = START_TEST_HEADER.to_vec();
        buf.extend(bincode::serialize(&start_test).unwrap());

        let addr = format!("{}:{}", opt.connect_addr, opt.connect_port);
        let addr = SocketAddr::from_str(&addr).unwrap();
        sock.send_to(buf.as_slice(), &addr).unwrap();

        log::info!("Starting test: {:?} with addr {}", start_test, &addr);
        let sock_clone = sock.try_clone().unwrap();
        std::thread::spawn(move||{
            test_send_loop(
                start_test.packet_size,
                start_test.packet_count,
                sock_clone,
                addr,
            );
        });
    }
    let mut last_stats = ServerStats {
        bytes_received: 0,
        packets_received: 0,
        packet_count: 0,
    };
    let mut pre_last_update = std::time::Instant::now();
    let mut last_update = std::time::Instant::now();
    loop {
        let stats = *server_stats.lock().unwrap();
        if stats != last_stats {
            println!("Bytes received: {}", stats.bytes_received);
            //bytes per second
            println!(
                "Bytes per second: {} MB/s",
                (stats.bytes_received - last_stats.bytes_received) as f64
                    / (last_update - pre_last_update).as_secs_f64()
                    / 1024.0
                    / 1024.0
            );
            println!(
                "Packets per second: {}",
                (stats.packets_received - last_stats.packets_received) as f64
                    / (last_update - pre_last_update).as_secs_f64()
            );
            println!(
                "Packets missing: {}",stats.packet_count as i64 - stats.packets_received as i64
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        pre_last_update = last_update;
        last_update = std::time::Instant::now();
        last_stats = stats;
    }

   /* } else {
        println!("Connecting to {}:{}", opt.connect_addr, opt.connect_port);
        let sock = UdpSocket::bind(format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap();
        let mut buf = Box::new(vec![0; 1100]);

        let mut start_test = StartTest::new(1100, 10000);
        let mut buf = START_TEST_HEADER.to_vec();
        buf.extend(bincode::serialize(&start_test).unwrap());

        sock.send_to(buf.as_slice(), format!("{}:{}", opt.connect_addr, opt.connect_port))?;

        let sock_clone = sock.try_clone().unwrap();
        let server_stats = Arc::new(Mutex::new(ServerStats {
            bytes_received: 0,
            packets_received: 0,
        }));
        let server_stats_ = server_stats.clone();
        let _thread = std::thread::spawn(move || match receive_udp(sock, server_stats_) {
            Ok(_) => println!("UDP received"),
            Err(e) => println!("UDP error: {}", e),
        });

        let mut last_stats = ServerStats {
            bytes_received: 0,
            packets_received: 0,
        };
        let mut pre_last_update = std::time::Instant::now();
        let mut last_update = std::time::Instant::now();
        loop {
            let stats = *server_stats.lock().unwrap();

            if stats != last_stats {
                println!("Bytes received: {}", stats.bytes_received);
                //bytes per second
                println!(
                    "Bytes per second: {} MB/s",
                    (stats.bytes_received - last_stats.bytes_received) as f64
                        / (last_update - pre_last_update).as_secs_f64()
                        / 1024.0
                        / 1024.0
                );
                println!(
                    "Packets per second: {}",
                    (stats.packets_received - last_stats.packets_received) as f64
                        / (last_update - pre_last_update).as_secs_f64()
                );
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            pre_last_update = last_update;
            last_update = std::time::Instant::now();
            last_stats = stats;
        }*/
        /*for i in 0..buf.len() {
            buf[i] = i as u8;
        }
        loop {
            let _len = sock.send_to(&buf, format!("{}:{}", opt.connect_addr, opt.connect_port))?;
        }*/

    Ok(())
}