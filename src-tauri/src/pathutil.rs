use std::io;
use std::path::{Path, PathBuf};

pub fn canonicalize_path(path: &Path) -> io::Result<PathBuf> {
    dunce::canonicalize(path)
}
