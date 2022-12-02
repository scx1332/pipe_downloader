use bzip2::write::BzEncoder;
use fake::Fake;
use flate2::write::GzEncoder;
use lz4::EncoderBuilder;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};




pub async fn process_in_to_out<F>(source: PathBuf, destination: PathBuf, process_fn: F) -> anyhow::Result<()>
where
    F: FnOnce(&mut File, File) -> anyhow::Result<()> + std::marker::Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let mut input_file = File::open(source)?;
        let output_file = File::create(destination)?;
        process_fn(&mut input_file, output_file)
    })
    .await
    .map_err(anyhow::Error::from)?
}

pub async fn lz4_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    process_in_to_out(source, destination, |input_file, output_file| {
        let mut encoder = EncoderBuilder::new()
            .level(4)
            .build(output_file)
            .map_err(anyhow::Error::from)?;
        std::io::copy(input_file, &mut encoder).map_err(anyhow::Error::from)?;
        let (_output, result) = encoder.finish();
        result.map_err(anyhow::Error::from)
    })
        .await
}

pub async fn gzip_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    process_in_to_out(source, destination, |input_file, output_file| {
        let mut encoder = GzEncoder::new(output_file, flate2::Compression::default());
        std::io::copy(input_file, &mut encoder).map_err(anyhow::Error::from)?;
        let _file = encoder.finish().map_err(anyhow::Error::from)?;
        Ok(())
    })
        .await
}

pub async fn bzip_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    process_in_to_out(source, destination, |input_file, output_file| {
        let mut encoder = BzEncoder::new(output_file, bzip2::Compression::default());
        std::io::copy(input_file, &mut encoder).map_err(anyhow::Error::from)?;
        let _res = encoder.finish().map_err(anyhow::Error::from)?;
        Ok(())
    })
        .await
}

pub async fn xz_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    process_in_to_out(source, destination, |input_file, output_file| {
        let mut encoder = xz2::write::XzEncoder::new(output_file, 5);
        std::io::copy(input_file, &mut encoder).map_err(anyhow::Error::from)?;
        let _res = encoder.finish().map_err(anyhow::Error::from)?;
        Ok(())
    })
        .await
}

pub async fn lz4_decompress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    process_in_to_out(source, destination, |input_file, output_file| {
        let mut encoder = EncoderBuilder::new()
            .level(4)
            .build(output_file)
            .map_err(anyhow::Error::from)?;
        std::io::copy(input_file, &mut encoder).map_err(anyhow::Error::from)?;
        let (_output, result) = encoder.finish();
        result.map_err(anyhow::Error::from)
    })
        .await
}


pub async fn build_random_name_file(path: &Path, size: usize) -> io::Result<()> {
    // using `faker` module with locales
    use fake::faker::name::raw::*;
    use fake::locales::*;

    let mut file = File::create(path)?;

    let mut str = "".to_string();
    for _i in 0..40000 {
        let _name: String = Name(EN).fake();
        str += format!(
            "{}, {}, {}",
            Name(EN).fake::<String>(),
            Name(JA_JP).fake::<String>(),
            Name(ZH_TW).fake::<String>()
        )
        .as_str();
        if str.len() > size {
            break;
        };
    }

    let mut bytes_left = size;
    loop {
        if bytes_left < str.as_bytes().len() {
            bytes_left -= file.write(&str.as_bytes()[0..bytes_left])?;
        } else {
            bytes_left -= file.write(str.as_bytes())?;
        }
        if bytes_left <= 0 {
            break;
        }
    }
    Ok(())
}

