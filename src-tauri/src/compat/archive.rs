use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Archive shapes the language server installer can receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gz,
    Zip,
    TarGz,
    TarXz,
}

/// Archive kind from the label the language server catalog uses.
pub fn kind_from_label(label: &str) -> Option<Kind> {
    match label {
        "gz" => Some(Kind::Gz),
        "zip" => Some(Kind::Zip),
        "tar.gz" => Some(Kind::TarGz),
        "tar.xz" => Some(Kind::TarXz),
        _ => None,
    }
}

/// Unpack an archive into `dest` without asking the host for tools.
pub fn extract(kind: Kind, archive: &Path, dest: &Path) -> Result<(), String> {
    match kind {
        Kind::Gz => Err("a plain gzip file is decompressed into a single file".into()),
        Kind::Zip => extract_zip(archive, dest),
        Kind::TarGz => extract_tar(archive, dest, TarCompression::Gzip),
        Kind::TarXz => extract_tar(archive, dest, TarCompression::Xz),
    }
}

/// Decompress a single gzip file into `dest`.
pub fn decompress_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| error.to_string())?;
    let mut decoder = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut out = File::create(dest).map_err(|error| error.to_string())?;
    std::io::copy(&mut decoder, &mut out).map_err(|error| error.to_string())?;
    Ok(())
}

enum TarCompression {
    Gzip,
    Xz,
}

fn extract_tar(archive: &Path, dest: &Path, compression: TarCompression) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| error.to_string())?;
    match compression {
        TarCompression::Gzip => {
            let mut decoder = flate2::read::GzDecoder::new(BufReader::new(file));
            unpack_tar(&mut decoder, dest)
        }
        TarCompression::Xz => {
            // xz has no streaming reader here, so the tar lands in a file first.
            let plain = archive.with_extension("xz.plain");
            {
                let mut out = File::create(&plain).map_err(|error| error.to_string())?;
                let mut input = BufReader::new(file);
                lzma_rs::xz_decompress(&mut input, &mut out).map_err(|error| error.to_string())?;
            }
            let plain_file = File::open(&plain).map_err(|error| error.to_string())?;
            let result = unpack_tar(&mut BufReader::new(plain_file), dest);
            let _ = std::fs::remove_file(&plain);
            result
        }
    }
}

fn unpack_tar<R: Read>(source: &mut R, dest: &Path) -> Result<(), String> {
    let mut tar = tar::Archive::new(source);
    let entries = tar.entries().map_err(|error| error.to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|error| error.to_string())?;
        entry.unpack_in(dest).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| error.to_string())?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|error| error.to_string())?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| error.to_string())?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut target = File::create(&out).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut target).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_follows_the_catalog_label() {
        assert_eq!(kind_from_label("gz"), Some(Kind::Gz));
        assert_eq!(kind_from_label("zip"), Some(Kind::Zip));
        assert_eq!(kind_from_label("tar.gz"), Some(Kind::TarGz));
        assert_eq!(kind_from_label("tar.xz"), Some(Kind::TarXz));
        assert_eq!(kind_from_label("none"), None);
    }
}
