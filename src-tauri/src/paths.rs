use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub const APP_CONFIG_DIR: &str = ".k-agent";

pub fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().home_dir().map_err(|error| error.to_string())
}

pub fn config_root(home: &Path) -> PathBuf {
    home.join(APP_CONFIG_DIR)
}

pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_root(&home_dir(app)?))
}

pub fn config_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join(name))
}

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|error| error.to_string())
}

pub fn app_data_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join(name))
}

pub fn tool_cache_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let path = app_data_dir(app)?.join("cache").join(name);
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    Ok(path)
}

pub fn set_user_private(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}
