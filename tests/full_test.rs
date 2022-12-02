use std::collections::HashMap;
use std::fs;
use std::fs::File;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use rand::{distributions::Alphanumeric, Rng};
use warp::Filter;

use sha256::try_digest;
use std::time::Duration;
use tokio::try_join;

use pipe_downloader_lib::PipeDownloaderOptions;
use pipe_utils::{build_random_file, bzip_compress, gzip_compress, lz4_compress, xz_compress};

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
        .run(SocketAddr::from_str(&format!("{}:{}", opt.listen_addr, opt.listen_port)).unwrap())
        .await
}

fn rand_str(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect::<String>()
}

#[tokio::test]
async fn test_download_and_unpack() {
    //let static_dir = "tmp/static".to_string();
    let static_dir = format!("tmp/static_{}", rand_str(10));

    let sd = Path::new(&static_dir);

    fs::create_dir_all(sd).unwrap();
    fs::create_dir_all(sd.join("subdir1")).unwrap();
    fs::create_dir_all(sd.join("subdir2")).unwrap();

    //fs::copy(sd.join("foo.txt"), sd.join("subdir1/foo.txt")).unwrap();

    let file = File::create(sd.join("foo.tar")).unwrap();
    let mut a = tar::Builder::new(file);

    let mut file_info_map = HashMap::<String, String>::new();
    for _i in 0..100 {
        let file_name_str = format!("foo_{}.txt", rand_str(15));
        let file_path = &sd.join(&file_name_str);
        build_random_file(file_path, rand::thread_rng().gen_range(20000..500000))
            .await
            .unwrap();
        let hex_digest = try_digest(file_path.as_path()).unwrap();
        println!("{} {}", hex_digest, &file_name_str);
        file_info_map.insert(file_name_str.clone(), hex_digest);
        a.append_file(&file_name_str, &mut File::open(file_path).unwrap())
            .unwrap();
    }
    let f1 = lz4_compress(sd.join("foo.tar"), sd.join("foo.tar.lz4"));
    let f2 = gzip_compress(sd.join("foo.tar"), sd.join("foo.tar.gz"));
    let f3 = bzip_compress(sd.join("foo.tar"), sd.join("foo.tar.bz2"));
    let f4 = xz_compress(sd.join("foo.tar"), sd.join("foo.tar.xz"));

    try_join!(f1, f2, f3, f4).unwrap();

    let opt = Opt {
        serve_dir: PathBuf::from(sd),
        listen_addr: String::from("127.0.0.1"),
        listen_port: 23752,
    };

    let move_opt = opt.clone();
    let tsk = tokio::task::spawn(async move {
        setup_server(&move_opt).await;
    });

    for compr in ["lz4", "gz", "bz2", "xz"].iter() {
        let pd = PipeDownloaderOptions {
            chunk_size_decoder: 10000000,
            chunk_size_downloader: 10000000,
            max_download_speed: None,
            force_no_chunks: false,
            download_threads: 10,
        }
        .start_download(
            format!(
                "http://{}:{}/static/foo.tar.{}",
                opt.listen_addr, opt.listen_port, compr
            )
            .as_str(),
            &sd.join(format!("output_{}", compr)),
        )
        .unwrap();

        let _current_time = std::time::Instant::now();
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
    }
    tsk.abort();

    fs::remove_dir_all(sd).unwrap();
}
