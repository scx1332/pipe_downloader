mod frontend;
mod options;

use actix_web::web::Data;
use actix_web::{web, App, HttpRequest, HttpServer, Responder};
use std::sync::{Arc, Mutex};

use crate::options::CliOptions;
use pipe_downloader_lib::{PipeDownloader, PipeDownloaderOptions};

use crate::frontend::frontend_serve;
use crate::frontend::redirect_to_frontend;
use serde_json::json;

use std::time::Duration;

use actix_web::dev::ServerHandle;
use structopt::StructOpt;
use tokio::signal;

#[derive(Clone)]
pub struct ServerData {
    pub pipe_downloader: Arc<Mutex<PipeDownloader>>,
}

pub async fn config(_req: HttpRequest, _server_data: Data<Box<ServerData>>) -> impl Responder {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    web::Json(json!({"config": {"version": VERSION}}))
}
async fn progress_endpoint(
    _req: HttpRequest,
    server_data: Data<Box<ServerData>>,
) -> impl Responder {
    let pd = server_data.pipe_downloader.lock().unwrap();
    web::Json(json!({ "progress": pd.get_progress() }))
}

struct StopHandle {
    inner: std::sync::Mutex<ServerHandle>,
}

impl StopHandle {
    pub fn new(handle: ServerHandle) -> Self {
        StopHandle {
            inner: std::sync::Mutex::new(handle),
        }
    }

    /// Sends stop signal through contained server handle.
    pub(crate) fn stop(&self, graceful: bool) {
        #[allow(clippy::let_underscore_future)]
        let _ = self.inner.lock().unwrap().stop(graceful);
    }
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
        ignore_symlinks: opt.ignore_symlinks,
        ignore_directory_exists: opt.force,
    }
    .start_download(&opt.url, opt.output_dir)
    .await?;

    let server_data = Data::new(Box::new(ServerData {
        pipe_downloader: Arc::new(Mutex::new(pd)),
    }));
    let server_data_cloned = server_data.clone();

    let (srv, stop_handle) = if !opt.frontend {
        (None, None)
    } else {
        let srv = HttpServer::new(move || {
            let cors = if opt.add_cors {
                actix_cors::Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600)
            } else {
                actix_cors::Cors::default()
            };

            let api_scope = web::scope("/api")
                .wrap(cors)
                .app_data(server_data_cloned.clone())
                .route("/progress", web::get().to(progress_endpoint))
                .route("/config", web::get().to(config));

            App::new()
                .route("/", web::get().to(redirect_to_frontend))
                .route("/frontend", web::get().to(redirect_to_frontend))
                .route("/frontend/{_:.*}", web::get().to(frontend_serve))
                .service(api_scope)
        })
        .workers(1)
        .bind((opt.listen_addr.clone(), opt.listen_port))
        .map_err(anyhow::Error::from)?
        .run();
        let st_handle = StopHandle::new(srv.handle());
        (Some(srv), Some(st_handle))
    };

    //let mut signals = Signals::new(&[SIGINT])?;

    let sp_thread = tokio::spawn(async move {
        let current_time = std::time::Instant::now();
        let mut requested_kill = false;
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
                if pd.is_finished() {
                    break;
                }
            }
            // test pause
            // if elapsed.as_secs() > 30 {
            //     pd.pause_download();
            //     break;
            // }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                },
                _ = signal::ctrl_c() => {
                    println!("Kill signal received");
                    let pd = server_data.pipe_downloader.lock().unwrap();
                    pd.signal_stop();
                    requested_kill = true;

                },
            };
        }
        let elapsed = current_time.elapsed();
        println!("Unpack finished in: {elapsed:?}");
        if let Some(stop_handle) = stop_handle {
            if !requested_kill {
                println!("Waiting after finish: {} sec", opt.wait_after_finish_sec);
                std::thread::sleep(Duration::from_secs(opt.wait_after_finish_sec));
                stop_handle.stop(true);
            } else {
                stop_handle.stop(false);
            }
        }
    });

    if let Some(srv) = srv {
        println!(
            "Frontend started at http://{}:{}",
            opt.listen_addr, opt.listen_port
        );
        //Await actix_web server if it was started
        srv.await.map_err(anyhow::Error::from)?;
    }
    match sp_thread.await {
        Ok(_) => {
            log::info!("Finished");
        }
        Err(e) => {
            log::error!("Error: {:?}", e);
        }
    }
    Ok(())
}
