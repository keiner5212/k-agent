use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use zeroize::Zeroize;

const MASTER_FILE: &str = "master.key";
const MAGIC: &[u8; 4] = b"KAG1";
const MASTER_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const SEAL_PREFIX: &str = "enc:v1:";
const MAX_PLAINTEXT: usize = 8_192;
const HKDF_SALT: &[u8] = b"k-agent.api-key.v1";
const HKDF_INFO: &[u8] = b"aes-256-gcm";

static CIPHER: OnceLock<Aes256Gcm> = OnceLock::new();

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("config directory is unavailable")]
    NoConfigDir,
    #[error("could not persist the local secret")]
    Persist,
    #[error("local secret file is missing")]
    Missing,
    #[error("local secret file is invalid")]
    Invalid,
    #[error("local secret file is unreadable")]
    Unreadable,
    #[error("API key is too large to store")]
    TooLarge,
    #[error("stored API key could not be decrypted")]
    Decrypt,
    #[error("could not encrypt API key")]
    Encrypt,
}

struct MasterKey([u8; MASTER_LEN]);

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub fn ensure_master_key(config_dir: &Path) -> Result<(), SecretError> {
    load_or_create_master(config_dir).map(|_| ())
}

pub fn cipher(config_dir: &Path) -> Result<&'static Aes256Gcm, SecretError> {
    init_cipher(config_dir)
}

pub fn seal(cipher: &Aes256Gcm, plaintext: &str) -> Result<String, SecretError> {
    if plaintext.len() > MAX_PLAINTEXT {
        return Err(SecretError::TooLarge);
    }
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| SecretError::Encrypt)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| SecretError::Encrypt)?;
    let mut packed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    packed.extend_from_slice(&nonce_bytes);
    packed.extend_from_slice(&ciphertext);
    nonce_bytes.zeroize();
    Ok(format!("{SEAL_PREFIX}{}", BASE64.encode(packed)))
}

pub fn open(cipher: &Aes256Gcm, stored: Option<&str>) -> Result<Option<String>, SecretError> {
    let Some(value) = stored.map(str::trim).filter(|item| !item.is_empty()) else {
        return Ok(None);
    };
    let payload = value
        .strip_prefix(SEAL_PREFIX)
        .ok_or(SecretError::Decrypt)?;
    Ok(Some(unseal(cipher, payload)?))
}

fn init_cipher(config_dir: &Path) -> Result<&'static Aes256Gcm, SecretError> {
    if let Some(existing) = CIPHER.get() {
        return Ok(existing);
    }
    let master = load_or_create_master(config_dir)?;
    let mut derived = derive_aes_key(&master.0);
    let built = Aes256Gcm::new_from_slice(&derived).map_err(|_| SecretError::Invalid)?;
    derived.zeroize();
    let _ = CIPHER.set(built);
    CIPHER.get().ok_or(SecretError::Invalid)
}

fn load_or_create_master(config_dir: &Path) -> Result<MasterKey, SecretError> {
    if config_dir.as_os_str().is_empty() {
        return Err(SecretError::NoConfigDir);
    }
    prepare_config_dir(config_dir)?;
    let path = config_dir.join(MASTER_FILE);
    match read_master(&path) {
        Ok(key) => Ok(key),
        Err(SecretError::Missing) => write_master(&path),
        Err(error) => Err(error),
    }
}

fn prepare_config_dir(config_dir: &Path) -> Result<(), SecretError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        match std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(config_dir)
        {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(SecretError::Persist),
        }
        let _ = fs::set_permissions(config_dir, fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(config_dir).map_err(|_| SecretError::Persist)?;
    }
    Ok(())
}

fn read_master(path: &Path) -> Result<MasterKey, SecretError> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(SecretError::Missing);
        }
        Err(_) => return Err(SecretError::Unreadable),
    };
    if data.len() != MAGIC.len() + MASTER_LEN || !data.starts_with(MAGIC) {
        return Err(SecretError::Invalid);
    }
    let mut key = [0u8; MASTER_LEN];
    key.copy_from_slice(&data[MAGIC.len()..]);
    Ok(MasterKey(key))
}

fn write_master(path: &Path) -> Result<MasterKey, SecretError> {
    let mut key = [0u8; MASTER_LEN];
    getrandom::getrandom(&mut key).map_err(|_| SecretError::Persist)?;
    let mut data = [0u8; MAGIC.len() + MASTER_LEN];
    data[..MAGIC.len()].copy_from_slice(MAGIC);
    data[MAGIC.len()..].copy_from_slice(&key);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&data).map_err(|_| SecretError::Persist)?;
            file.sync_all().map_err(|_| SecretError::Persist)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            key.zeroize();
            data.zeroize();
            return read_master(path);
        }
        Err(_) => {
            key.zeroize();
            data.zeroize();
            return Err(SecretError::Persist);
        }
    }
    data.zeroize();
    restrict_file(path);
    Ok(MasterKey(key))
}

fn restrict_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn derive_aes_key(master: &[u8; MASTER_LEN]) -> [u8; MASTER_LEN] {
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), master);
    let mut okm = [0u8; MASTER_LEN];
    hkdf.expand(HKDF_INFO, &mut okm)
        .expect("hkdf output length is valid");
    okm
}

fn unseal(cipher: &Aes256Gcm, payload: &str) -> Result<String, SecretError> {
    let packed = BASE64.decode(payload.trim()).map_err(|_| SecretError::Decrypt)?;
    if packed.len() < NONCE_LEN + 16 {
        return Err(SecretError::Decrypt);
    }
    let (nonce_bytes, ciphertext) = packed.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| SecretError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| SecretError::Decrypt)
}
