use actix_multipart::Multipart;

use actix_web::http::header::ContentDisposition;
use actix_web::{get, post, HttpRequest};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use async_stream::stream;
use bytes::Bytes;
use bytes::BytesMut;
use futures_core::stream::Stream;
use futures_util::StreamExt;

use std::iter::repeat_with;

use structopt::StructOpt;

fn semi_random_stream(size: usize) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    const BUF_SIZE: usize = 1024 * 1024;
    const RAND_SEED: u64 = 4673746563;
    let _random_bytes: Vec<u8> = vec![0; std::cmp::min(BUF_SIZE, size)];
    let rng = fastrand::Rng::with_seed(RAND_SEED);
    let random_bytes: Vec<u8> = repeat_with(|| rng.u8(..)).take(BUF_SIZE).collect();
    let random_bytes = Bytes::from(random_bytes);

    stream! {
        let mut bytes_sent = 0;
        loop {
            let _buffer = BytesMut::with_capacity(BUF_SIZE);
            if bytes_sent >= size {
                break;
            }
            let current_size = std::cmp::min(BUF_SIZE, size - bytes_sent);
            yield Ok(random_bytes.slice(0..current_size));
            bytes_sent += current_size;
        }
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    /// Listen address
    #[structopt(long, default_value = "127.0.0.1")]
    pub listen_addr: String,

    /// Listen port
    #[structopt(long, default_value = "5554")]
    pub listen_port: u16,
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().content_type("text/html")
        .body("<a href=\"download/10000000\">Download 10MB</a><a href=\"download/1000000000\">Download 1000MB</a>")
}

#[get("/download/{size}")]
async fn download(args: web::Path<usize>) -> impl Responder {
    let download_size = args.into_inner();
    HttpResponse::Ok()
        .content_type("application/octet-stream")
        .insert_header(ContentDisposition::attachment("download.bin"))
        .insert_header(("Content-Length", download_size))
        .streaming(semi_random_stream(download_size))
}

#[post("/upload")]
pub async fn upload(_req: HttpRequest, mut payload: Multipart) -> impl Responder {
    if let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(field) => field,
            Err(err) => {
                return HttpResponse::InternalServerError()
                    .body(format!("Error when getting file from payload {err}"))
            }
        };

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            match chunk {
                Ok(chunk) => {
                    println!("-- CHUNK: \n{:?}", std::str::from_utf8(&chunk));
                }
                Err(err) => {
                    return HttpResponse::InternalServerError()
                        .body(format!("Error when getting chunk from payload {err}"));
                }
            }
        }
        HttpResponse::Ok()
            .content_type("text/html")
            .body("Upload finished")
    } else {
        HttpResponse::InternalServerError().body("No upload file found")
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let opt: Opt = Opt::from_args();
    println!("Listening on {}:{}", opt.listen_addr, opt.listen_port,);

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(manual_hello))
            .service(download)
            .service(upload)
    })
    .bind((opt.listen_addr, opt.listen_port))?
    .run()
    .await
}
