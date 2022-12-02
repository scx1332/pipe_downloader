use bzip2::write::BzEncoder;
use fake::Fake;
use flate2::write::GzEncoder;
use lz4::EncoderBuilder;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

pub async fn build_random_file(path: &Path, size: usize) -> io::Result<()> {
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
