use std::{fs, thread};
use std::fs::File;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use fake::{Dummy, Fake, Faker};
use rand::rngs::StdRng;
use rand::SeedableRng;
use warp::Filter;
use rand::{distributions::Alphanumeric, Rng};
use std::io::Write;
use std::env;
use std::io::{self, Result};
use std::time::Duration;

use lz4::{Decoder, EncoderBuilder};
use pipe_downloader_lib::PipeDownloaderOptions;

#[derive(Debug, Dummy)]
pub struct Foo {
    #[dummy(faker = "1000..2000")]
    order_id: usize,
    customer: String,
    paid: bool,
}

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

fn compress(source: &Path, destination: &Path) {
    println!("Compressing: {} -> {}", source.display(), destination.display());

    let mut input_file = File::open(source).unwrap();
    let output_file = File::create(destination).unwrap();
    let mut encoder = EncoderBuilder::new()
        .level(4)
        .build(output_file).unwrap();
    io::copy(&mut input_file, &mut encoder).unwrap();
    let (_output, result) = encoder.finish();
    result.unwrap();
}

#[tokio::test]
async fn test_something_async() {
    let static_dir = "tmp/static".to_string();
    let sd = Path::new(&static_dir);
    /*let static_dir = format!("tmp/static_{}", rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect::<String>());*/

    fs::create_dir_all(sd).unwrap();
    fs::create_dir_all(sd.join("subdir1")).unwrap();
    fs::create_dir_all(sd.join("subdir2")).unwrap();

    // using `faker` module with locales
    use fake::faker::name::raw::*;
    use fake::locales::*;

    let mut file = File::create(static_dir.clone() + "/foo.txt").unwrap();

    let mut str = "".to_string();
    for i in 0..40000 {
        let name: String = Name(EN).fake();
        str += format!("{}, {}, {}", Name(EN).fake::<String>(), Name(JA_JP).fake::<String>(), Name(ZH_TW).fake::<String>()).as_str();
    }

    for i in 0..5 {
        file.write(str.as_bytes()).unwrap();
    }

    fs::copy(sd.join("foo.txt"), sd.join("subdir1/foo.txt")).unwrap();

    let file = File::create(sd.join("foo.tar")).unwrap();
    let mut a = tar::Builder::new(file);
    for i in 0..100 {
        a.append_file(format!("foo_{}.txt", i), &mut File::open(sd.join("foo.txt")).unwrap()).unwrap();
    }
    compress(sd.join("foo.tar").as_path(), sd.join("foo.tar.lz4").as_path());

    let name: String = Name(EN).fake();
    println!("name {:?}", name);

    let name: String = Name(ZH_TW).fake();
    println!("name {:?}", name);

    let opt = Opt {
        serve_dir: PathBuf::from(sd),
        listen_addr: String::from("127.0.0.1"),
        listen_port: 23752,
    };

    let move_opt = opt.clone();
    let tsk = tokio::task::spawn(
        async move {
            setup_server(&move_opt).await;
        }
    );


    let pd = PipeDownloaderOptions {
        chunk_size_decoder: 30_000_000,
        chunk_size_downloader: 10_000_000,
        max_download_speed: None,
        force_no_chunks: false,
        download_threads: 10,
    }
        .start_download(format!("http://{}:{}/static/foo.tar.lz4", opt.listen_addr, opt.listen_port).as_str(), &sd.join("output")).unwrap();

    let current_time = std::time::Instant::now();
    loop {

        println!("{}", pd.get_progress_human_line());
        if pd.is_finished() {
            break;
        }
        // test pause
        // if elapsed.as_secs() > 30 {
        //     pd.pause_download();
        //     break;
        // }

        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    tsk.abort();
}