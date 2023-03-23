use std::iter::repeat_with;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web::error::UrlencodedError::ContentType;
use actix_web::http::header::ContentDisposition;
use warp::http::Request;
use async_stream::stream;
use futures_core::stream::Stream;
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use bytes::Bytes;
use bytes::BytesMut;
use actix_web::get;
fn zero_to_three(size: usize) -> impl Stream<Item = Result<Bytes, std::io::Error>> {
    const BUF_SIZE: usize = 1*1024*1024;
    const RAND_SEED: u64 = 4673746563;
    let mut random_bytes: Vec<u8> = vec![0; std::cmp::min(BUF_SIZE, size)];
    let rng = fastrand::Rng::with_seed(RAND_SEED);
    let mut random_bytes: Vec<u8> = repeat_with(|| rng.u8(..)).take(BUF_SIZE).collect();
    let random_bytes = Bytes::from(random_bytes);

    stream! {
        let mut bytes_sent = 0;
        loop {
            let mut buffer = BytesMut::with_capacity(BUF_SIZE);
            if bytes_sent >= size {
                break;
            }
            let current_size = std::cmp::min(BUF_SIZE, size - bytes_sent);
//            fill_bytes(&mut buffer[0..current_size]);

            //slice make copy
            yield Ok(random_bytes.slice(0..current_size));
            bytes_sent += current_size;
        }
    }
}


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


async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().content_type("text/html")
        .body("<a href=\"download/10000000\">Download 10MB</a><a href=\"download/1000000000\">Download 1000MB</a>")
}



#[get("/download/{size}")]
async fn download(args: web::Path<usize>) -> impl Responder {
    HttpResponse::Ok().content_type("application/octet-stream")
        .insert_header(ContentDisposition::attachment("download.bin"))
        .streaming(zero_to_three(args.into_inner()))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let opt: Opt = Opt::from_args();
    println!(
        "Listening on {}:{}, serving static files: {}, http://{}:{}/static",
        opt.listen_addr,
        opt.listen_port,
        opt.serve_dir.display(),
        opt.listen_addr,
        opt.listen_port
    );

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(manual_hello))
            .service(download)
    })
        .bind((opt.listen_addr, opt.listen_port))?
        .run()
        .await
}


