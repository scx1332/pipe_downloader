mod world_time;

use crate::world_time::{init_world_time, world_time};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use structopt::StructOpt;

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
}

fn receive_udp(sock: UdpSocket, stats: Arc<Mutex<ServerStats>>) -> std::io::Result<()> {
    const SMALL_BUF: usize = 90000;
    let mut buf = Box::new(vec![0; SMALL_BUF]);
    let mut local_stats = ServerStats {
        bytes_received: 0,
        packets_received: 0,
    };
    let mut last_update = std::time::Instant::now();
    loop {
        let (len, _addr) = sock.recv_from(&mut buf)?;
        local_stats.bytes_received += len as u64;
        local_stats.packets_received += 1;
        if last_update.elapsed().as_secs_f64() > 0.1 {
            //update value behind lock only sometimes to improve perf
            *stats.lock().unwrap() = local_stats;
            last_update = std::time::Instant::now();
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let opt: Opt = Opt::from_args();

    init_world_time();
    let world_time = world_time();
    println!("World time without fix: {}", chrono::Utc::now());
    println!("World time: {}", world_time.utc_time());
    println!("World time without fix: {}", chrono::Utc::now());

    if opt.is_server {
        println!("Listening on {}:{}", opt.listen_addr, opt.listen_port,);
        let sock = UdpSocket::bind(format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap();
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
        }
    } else {
        println!("Connecting to {}:{}", opt.connect_addr, opt.connect_port);
        let sock = UdpSocket::bind(format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap();
        let mut buf = Box::new(vec![0; 1100]);
        for i in 0..buf.len() {
            buf[i] = i as u8;
        }
        loop {
            let _len = sock.send_to(&buf, format!("{}:{}", opt.connect_addr, opt.connect_port))?;
        }
    }
}
