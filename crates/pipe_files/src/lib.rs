use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use lz4::EncoderBuilder;
use bzip2::write::BzEncoder;
use flate2::GzBuilder;
use fake::{Fake};
use flate2::write::GzEncoder;

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
        }
        else {
            bytes_left -= file.write(str.as_bytes())?;
        }
        if bytes_left <= 0 {
            break;
        }
    }
    Ok(())
}

pub async fn lz4_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move ||
        {
            println!(
                "Compressing: {} -> {}",
                source.display(),
                destination.display()
            );

            let mut input_file = std::fs::File::open(source).unwrap();
            let output_file = std::fs::File::create(destination).unwrap();
            let mut encoder = EncoderBuilder::new().level(4).build(output_file).unwrap();
            std::io::copy(&mut input_file, &mut encoder).unwrap();
            let (_output, result) = encoder.finish();
            result.map_err(anyhow::Error::from)
        }).await.map_err(anyhow::Error::from)?
}

pub async fn gzip_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move ||
        {
            println!(
                "Compressing: {} -> {}",
                source.display(),
                destination.display()
            );

            let mut input_file = std::fs::File::open(source).unwrap();
            let output_file = std::fs::File::create(destination).unwrap();
            let mut encoder = GzEncoder::new(output_file, flate2::Compression::default());
            std::io::copy(&mut input_file, &mut encoder).unwrap();
            let _res = encoder.finish()?;
            Ok(())
        }).await.map_err(anyhow::Error::from)?
}

pub async fn bzip_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move ||
        {
            println!(
                "Compressing: {} -> {}",
                source.display(),
                destination.display()
            );

            let mut input_file = std::fs::File::open(source).unwrap();
            let output_file = std::fs::File::create(destination).unwrap();
            let mut encoder = BzEncoder::new(output_file, bzip2::Compression::default());
            std::io::copy(&mut input_file, &mut encoder).unwrap();
            let _res = encoder.finish()?;
            Ok(())
        }).await.map_err(anyhow::Error::from)?
}

pub async fn xz_compress(source: PathBuf, destination: PathBuf) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move ||
        {
            println!(
                "Compressing: {} -> {}",
                source.display(),
                destination.display()
            );

            let mut input_file = std::fs::File::open(source).unwrap();
            let output_file = std::fs::File::create(destination).unwrap();
            let mut encoder = xz2::write::XzEncoder::new(output_file, 5);
            std::io::copy(&mut input_file, &mut encoder).unwrap();
            let _res = encoder.finish()?;
            Ok(())
        }).await.map_err(anyhow::Error::from)?
}