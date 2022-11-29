use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use warp::Filter;

#[derive(StructOpt, Debug)]
struct Opt {
    /// Localization of static files
    #[structopt(long, default_value = "static")]
    pub serve_dir: PathBuf,

    /// Listen address
    #[structopt(long, default_value = "127.0.0.1")]
    pub listen_addr: String,

    /// Listen port
    #[structopt(long, default_value = "3003")]
    pub listen_port: u16,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opt: Opt = Opt::from_args();

    let route = warp::path("static").and(warp::fs::dir(opt.serve_dir.clone()));

    println!(
        "Listening on {}:{}, serving static files: {}, {}//{}:{}/static",
        opt.listen_addr,
        opt.listen_port,
        opt.serve_dir.display(),
        "http:",
        opt.listen_addr,
        opt.listen_port
    );
    warp::serve(route)
        .run(SocketAddr::from_str(&format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap())
        .await;
}
