use reqwest::header;

fn main() {
    let client = reqwest::blocking::ClientBuilder::new();

    let download_url =
        "https://snapshot-download.polygon.technology/heimdall-mumbai-2023-02-14.tar.gz";

    let mut headers = header::HeaderMap::new();
    headers.insert("Accept", header::HeaderValue::from_static("*/*"));
    headers.insert(
        "User-Agent",
        header::HeaderValue::from_static("pipe_downloader/7.68.0"),
    );
    let client = client.default_headers(headers).build().unwrap();
    let response = client.get(download_url).send().unwrap();

    for header in response.headers() {
        println!("{:?}", header);
    }
}
