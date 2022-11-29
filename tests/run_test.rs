use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use warp::Filter;

#[derive(Debug, Clone)]
struct Opt {
    /// Localization of static files
    pub serve_dir: PathBuf,

    /// Listen address
    pub listen_addr: String,

    /// Listen port
    pub listen_port: u16,
}

async fn setup_server(opt: &Opt) {


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
        .run(SocketAddr::from_str(&format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap()).await
}

#[tokio::test]
async fn test_something_async() {
    
    let opt = Opt {
        serve_dir: PathBuf::from("tests/static"),
        listen_addr: String::from("127.0.0.1"),
        listen_port: 23752,
    };

    let move_opt = opt.clone();
    let tsk = tokio::task::spawn(
        async move {
            setup_server(&move_opt).await;
        }
    );



    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    tsk.abort();


}