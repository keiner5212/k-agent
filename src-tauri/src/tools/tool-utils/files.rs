use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Write the file whole or not at all.
///
/// The text goes to a file of its own next to the target and the target is
/// replaced with a rename, so a failure never leaves a half written file. The
/// mode of the file that is replaced is kept.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    // Two writes of the same file can overlap. The temp name is unique.
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_file_name(format!(
        ".{name}.{}.{unique}.k-agent-write",
        std::process::id()
    ));
    if let Err(error) = fs::write(&temp, content) {
        let _ = fs::remove_file(&temp);
        return Err(error.to_string());
    }
    if let Ok(meta) = fs::metadata(path) {
        let _ = fs::set_permissions(&temp, meta.permissions());
    }
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_atomic_keeps_the_mode_of_the_file() {
        let dir = std::env::temp_dir().join(format!("k-agent-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("script.sh");
        std::fs::write(&path, "old").unwrap();
        let before = std::fs::metadata(&path).unwrap().permissions().mode();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        let after = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(after & 0o7777, before & 0o7777);
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains("k-agent-write"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
