use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const ABSENT: &str = "absent";

#[derive(Clone)]
pub struct NotedFile {
    pub path: String,
    pub absolute: PathBuf,
    pub before_absent: bool,
    pub before_bytes: Vec<u8>,
}

pub struct SealedFile {
    pub path: String,
    pub before_hash: String,
    pub after_hash: String,
}

pub struct Sealed {
    pub checkpoint_id: String,
    pub files: Vec<SealedFile>,
}

pub fn remember(files: &Mutex<Vec<NotedFile>>, absolute: &Path, rel: &str) {
    let path = normalize_path(rel);
    if path.is_empty() || path.split('/').any(|part| part == "..") {
        return;
    }
    let mut guard = match files.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.iter().any(|item| item.path == path) {
        return;
    }
    if absolute.is_dir() {
        return;
    }
    if !absolute.exists() {
        guard.push(NotedFile {
            path,
            absolute: absolute.to_path_buf(),
            before_absent: true,
            before_bytes: Vec::new(),
        });
        return;
    }
    let Ok(before_bytes) = fs::read(absolute) else {
        return;
    };
    guard.push(NotedFile {
        path,
        absolute: absolute.to_path_buf(),
        before_absent: false,
        before_bytes,
    });
}

pub fn seal(session_dir: &Path, noted: Vec<NotedFile>) -> Result<Option<Sealed>, String> {
    if noted.is_empty() {
        return Ok(None);
    }
    let checkpoint_id = Uuid::new_v4().to_string();
    let dir = session_dir.join("checkpoints").join(&checkpoint_id);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    for item in noted {
        let Ok((after_absent, after_bytes)) = read_after(&item.absolute) else {
            continue;
        };
        let key = path_key(&item.path);
        if let Err(error) = write_sides(
            &dir,
            &key,
            item.before_absent,
            &item.before_bytes,
            after_absent,
            &after_bytes,
        ) {
            let _ = fs::remove_dir_all(&dir);
            return Err(error);
        }
        files.push(SealedFile {
            path: item.path,
            before_hash: state_hash(item.before_absent, &item.before_bytes),
            after_hash: state_hash(after_absent, &after_bytes),
        });
    }
    if files.is_empty() {
        let _ = fs::remove_dir_all(&dir);
        return Ok(None);
    }
    Ok(Some(Sealed {
        checkpoint_id,
        files,
    }))
}

pub fn restore_at(
    session_dir: &Path,
    absolute: &Path,
    checkpoint_id: &str,
    rel_path: &str,
    side: &str,
    expect_hash: &str,
    restore_hash: &str,
) -> Result<(), String> {
    let side = side.trim().to_ascii_lowercase();
    if side != "before" && side != "after" {
        return Err("side must be before or after".into());
    }
    if !safe_checkpoint_id(checkpoint_id) {
        return Err("invalid checkpoint id".into());
    }
    let path = normalize_path(rel_path);
    if path.is_empty() {
        return Err("invalid path".into());
    }
    if disk_state(absolute)? != expect_hash {
        return Err("file changed".into());
    }
    if restore_hash == ABSENT {
        if absolute.exists() {
            fs::remove_file(absolute).map_err(|error| error.to_string())?;
        }
        return Ok(());
    }
    let stored = session_dir
        .join("checkpoints")
        .join(checkpoint_id)
        .join(format!("{}.{side}", path_key(&path)));
    let bytes = fs::read(&stored).map_err(|error| error.to_string())?;
    if hash_bytes(&bytes) != restore_hash {
        return Err("checkpoint bytes do not match".into());
    }
    if let Some(parent) = absolute.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
    }
    fs::write(absolute, bytes).map_err(|error| error.to_string())
}

fn write_sides(
    dir: &Path,
    key: &str,
    before_absent: bool,
    before_bytes: &[u8],
    after_absent: bool,
    after_bytes: &[u8],
) -> Result<(), String> {
    if !before_absent {
        fs::write(dir.join(format!("{key}.before")), before_bytes)
            .map_err(|error| error.to_string())?;
    }
    if !after_absent {
        fs::write(dir.join(format!("{key}.after")), after_bytes)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn read_after(absolute: &Path) -> Result<(bool, Vec<u8>), String> {
    if absolute.is_file() {
        let bytes = fs::read(absolute).map_err(|error| error.to_string())?;
        return Ok((false, bytes));
    }
    if absolute.exists() {
        return Err("touched path is not a file".into());
    }
    Ok((true, Vec::new()))
}

fn disk_state(absolute: &Path) -> Result<String, String> {
    if !absolute.exists() {
        return Ok(ABSENT.to_string());
    }
    if !absolute.is_file() {
        return Err("file changed".into());
    }
    let bytes = fs::read(absolute).map_err(|error| error.to_string())?;
    Ok(hash_bytes(&bytes))
}

fn state_hash(absent: bool, bytes: &[u8]) -> String {
    if absent {
        ABSENT.to_string()
    } else {
        hash_bytes(bytes)
    }
}

fn normalize_path(rel: &str) -> String {
    rel.replace('\\', "/")
}

fn path_key(path: &str) -> String {
    hash_bytes(path.as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn safe_checkpoint_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("checkpoint-tests")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn noted(absolute: &Path, rel: &str) -> Vec<NotedFile> {
        let files = Mutex::new(Vec::new());
        remember(&files, absolute, rel);
        remember(&files, absolute, rel);
        files.into_inner().unwrap()
    }

    #[test]
    fn create_keeps_absent_then_final_bytes() {
        let root = scratch();
        let session = root.join("session");
        let file = root.join("new.txt");
        let mut files = noted(&file, "new.txt");
        assert!(files[0].before_absent);
        fs::write(&file, "v1").unwrap();
        fs::write(&file, "v2").unwrap();
        let sealed = seal(&session, std::mem::take(&mut files)).unwrap().unwrap();
        assert_eq!(sealed.files.len(), 1);
        assert_eq!(sealed.files[0].before_hash, ABSENT);
        assert_eq!(sealed.files[0].after_hash, hash_bytes(b"v2"));
        assert!(!session
            .join("checkpoints")
            .join(&sealed.checkpoint_id)
            .join(format!("{}.before", path_key("new.txt")))
            .exists());

        fs::write(&file, "other").unwrap();
        let blocked = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "new.txt",
            "before",
            &sealed.files[0].after_hash,
            ABSENT,
        );
        assert!(blocked.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"other");

        fs::write(&file, "v2").unwrap();
        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "new.txt",
            "before",
            &sealed.files[0].after_hash,
            ABSENT,
        )
        .unwrap();
        assert!(!file.exists());

        fs::write(&file, "nope").unwrap();
        let redo_blocked = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "new.txt",
            "after",
            ABSENT,
            &sealed.files[0].after_hash,
        );
        assert!(redo_blocked.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"nope");

        fs::remove_file(&file).unwrap();
        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "new.txt",
            "after",
            ABSENT,
            &sealed.files[0].after_hash,
        )
        .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"v2");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn edit_saves_first_before_and_final_after() {
        let root = scratch();
        let session = root.join("session");
        let file = root.join("note.txt");
        fs::write(&file, "a").unwrap();
        let hold = Mutex::new(Vec::new());
        remember(&hold, &file, "note.txt");
        fs::write(&file, "b").unwrap();
        remember(&hold, &file, "note.txt");
        fs::write(&file, "c").unwrap();
        let noted = hold.into_inner().unwrap();
        assert_eq!(noted.len(), 1);
        assert_eq!(noted[0].before_bytes, b"a");
        let sealed = seal(&session, noted).unwrap().unwrap();
        assert_eq!(sealed.files[0].before_hash, hash_bytes(b"a"));
        assert_eq!(sealed.files[0].after_hash, hash_bytes(b"c"));
        let dir = session.join("checkpoints").join(&sealed.checkpoint_id);
        let key = path_key("note.txt");
        assert_eq!(fs::read(dir.join(format!("{key}.before"))).unwrap(), b"a");
        assert_eq!(fs::read(dir.join(format!("{key}.after"))).unwrap(), b"c");

        let missed = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "note.txt",
            "before",
            &hash_bytes(b"nope"),
            &sealed.files[0].before_hash,
        );
        assert!(missed.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"c");

        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "note.txt",
            "before",
            &sealed.files[0].after_hash,
            &sealed.files[0].before_hash,
        )
        .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"a");

        fs::write(&file, "drift").unwrap();
        let redo_blocked = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "note.txt",
            "after",
            &sealed.files[0].before_hash,
            &sealed.files[0].after_hash,
        );
        assert!(redo_blocked.is_err());
        assert_eq!(fs::read(&file).unwrap(), b"drift");

        fs::write(&file, "a").unwrap();
        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "note.txt",
            "after",
            &sealed.files[0].before_hash,
            &sealed.files[0].after_hash,
        )
        .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"c");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_restores_only_when_absence_matches() {
        let root = scratch();
        let session = root.join("session");
        let file = root.join("gone.txt");
        fs::write(&file, "gone").unwrap();
        let noted = noted(&file, "gone.txt");
        fs::remove_file(&file).unwrap();
        let sealed = seal(&session, noted).unwrap().unwrap();
        assert_eq!(sealed.files[0].before_hash, hash_bytes(b"gone"));
        assert_eq!(sealed.files[0].after_hash, ABSENT);
        assert!(!session
            .join("checkpoints")
            .join(&sealed.checkpoint_id)
            .join(format!("{}.after", path_key("gone.txt")))
            .exists());

        fs::write(&file, "gone").unwrap();
        let blocked = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "gone.txt",
            "before",
            ABSENT,
            &sealed.files[0].before_hash,
        );
        assert!(blocked.is_err());
        assert!(file.exists());

        fs::remove_file(&file).unwrap();
        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "gone.txt",
            "before",
            ABSENT,
            &sealed.files[0].before_hash,
        )
        .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"gone");

        restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "gone.txt",
            "after",
            &sealed.files[0].before_hash,
            ABSENT,
        )
        .unwrap();
        assert!(!file.exists());

        let redo_blocked = restore_at(
            &session,
            &file,
            &sealed.checkpoint_id,
            "gone.txt",
            "after",
            &sealed.files[0].before_hash,
            ABSENT,
        );
        assert!(redo_blocked.is_err());
        assert!(!file.exists());
        let _ = fs::remove_dir_all(&root);
    }
}
