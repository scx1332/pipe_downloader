mod options;

use std::sync::{Arc, Mutex};
use actix_web::web::{Bytes, Data};
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder, Scope};

use crate::options::CliOptions;
use pipe_downloader_lib::{PipeDownloader, PipeDownloaderOptions};
use std::thread;
use std::time::Duration;
use serde_json::json;
use structopt::StructOpt;
use serde::Serialize;

#[derive(Clone)]
pub struct ServerData {
    pub pipe_downloader: Arc<Mutex<PipeDownloader>>,
}

async fn progress_endpoint(req: HttpRequest, server_data: Data<Box<ServerData>>) -> impl Responder {
    let pd = server_data.pipe_downloader.lock().unwrap();
    web::Json(json!({ "progress": pd.get_progress() }))
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt: CliOptions = CliOptions::from_args();

    let pd = PipeDownloaderOptions {
        chunk_size_decoder: opt.unpack_buffer,
        chunk_size_downloader: opt.download_buffer,
        max_download_speed: opt.limit_speed,
        force_no_chunks: opt.force_no_partial_content,
        download_threads: opt.download_threads,
    }
    .start_download(&opt.url, &opt.output_dir)?;




    let server_data = Data::new(Box::new(ServerData {
        pipe_downloader: Arc::new(Mutex::new(pd
        )),
    }));
    let server_data_cloned = server_data.clone();

    HttpServer::new(move || {
        App::new()
            .app_data(server_data_cloned.clone())
            .route("/", web::get().to(HttpResponse::Ok))
            .route("/progress", web::get().to(progress_endpoint))
    })
        .workers(1)
        .bind((opt.listen_addr, opt.listen_port))
        .map_err(anyhow::Error::from)?
        .run()
        .await
        .map_err(anyhow::Error::from)?;

    let current_time = std::time::Instant::now();
    loop {
        {
            let pd = server_data.pipe_downloader.lock().unwrap();
            if opt.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&pd.get_progress()).unwrap()
                )
            } else {
                println!("{}", pd.get_progress_human_line());
            }
            if !opt.run_after_finish && pd.is_finished() {
                break;
            }
        }
        // test pause
        // if elapsed.as_secs() > 30 {
        //     pd.pause_download();
        //     break;
        // }

        thread::sleep(Duration::from_millis(1000));
    }
    let elapsed = current_time.elapsed();
    println!("Unpack finished in: {:?}", elapsed);
    Ok(())
}
