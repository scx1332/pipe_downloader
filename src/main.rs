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
use std::thread;
use std::time::Duration;

use actix_web::dev::ServerHandle;
use structopt::StructOpt;

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

#[derive(Default)]
struct StopHandle {
    inner: std::sync::Mutex<Option<ServerHandle>>,
}

impl StopHandle {
    /// Sets the server handle to stop.
    pub(crate) fn register(&self, handle: ServerHandle) {
        *self.inner.lock().unwrap() = Some(handle);
    }

    /// Sends stop signal through contained server handle.
    pub(crate) fn stop(&self, graceful: bool) {
        #[allow(clippy::let_underscore_future)]
        let _ = self.inner.lock().unwrap().as_ref().unwrap().stop(graceful);
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
    }
    .start_download(&opt.url, &opt.output_dir)?;

    let server_data = Data::new(Box::new(ServerData {
        pipe_downloader: Arc::new(Mutex::new(pd)),
    }));
    let server_data_cloned = server_data.clone();

    let stop_handle = web::Data::new(StopHandle::default());
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

        return App::new()
            .route("/", web::get().to(redirect_to_frontend))
            .route("/frontend", web::get().to(redirect_to_frontend))
            .route("/frontend/{_:.*}", web::get().to(frontend_serve))
            .service(api_scope);
    })
    .workers(1)
    .bind((opt.listen_addr, opt.listen_port))
    .map_err(anyhow::Error::from)?
    .run();

    stop_handle.register(srv.handle());

    let _ = thread::spawn(move || {
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
                if pd.is_finished() {
                    break;
                }
            }
            // test pause
            // if elapsed.as_secs() > 30 {
            //     pd.pause_download();
            //     break;
            // }

            //
            thread::sleep(Duration::from_millis(1000));
        }
        let elapsed = current_time.elapsed();
        println!("Unpack finished in: {:?}", elapsed);
        println!("Waiting after finish: {} sec", opt.wait_after_finish_sec);
        std::thread::sleep(Duration::from_secs(opt.wait_after_finish_sec));
        stop_handle.stop(true);
    });

    srv.await.map_err(anyhow::Error::from)?;

    Ok(())
}
