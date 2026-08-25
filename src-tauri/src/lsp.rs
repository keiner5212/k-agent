use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;

const BUNDLED: &str = include_str!("../catalog/language-servers.json");
const INSTALL_ROOT: &str = "lang_servers";
const NPM_INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
const GO_INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
const PIP_INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
const GITHUB_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Error)]
pub enum LspError {
    #[error("{0}")]
    Message(String),
    #[error("unknown language server: {0}")]
    Unknown(String),
    #[error("{0} is required. Install it before this language server.")]
    MissingRequire(String),
    #[error("language servers disabled")]
    Disabled,
    #[error("language server not installed: {0}")]
    NotInstalled(String),
    #[error("no language server for this file")]
    NoMatch,
    #[error("io error: {0}")]
    Io(String),
}

impl Serialize for LspError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InstallMethod {
    Npm,
    Go,
    Pip,
    Github,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServerInstall {
    pub method: InstallMethod,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub asset_contains: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServerSpec {
    pub id: String,
    pub name: String,
    pub language_ids: Vec<String>,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
    pub root_markers: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
    pub requires: Vec<String>,
    pub install: LanguageServerInstall,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServerRow {
    #[serde(flatten)]
    pub spec: LanguageServerSpec,
    pub installed: bool,
    pub command_path: Option<String>,
    pub missing_requires: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    servers: Vec<LanguageServerSpec>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn catalog() -> Result<&'static [LanguageServerSpec], LspError> {
    static CATALOG: OnceLock<Result<Vec<LanguageServerSpec>, String>> = OnceLock::new();
    match CATALOG.get_or_init(|| {
        serde_json::from_str::<CatalogFile>(BUNDLED)
            .map(|file| file.servers)
            .map_err(|error| error.to_string())
    }) {
        Ok(servers) => Ok(servers.as_slice()),
        Err(error) => Err(LspError::Message(format!(
            "language server catalog: {error}"
        ))),
    }
}

fn spec_by_id(id: &str) -> Result<&'static LanguageServerSpec, LspError> {
    catalog()?
        .iter()
        .find(|spec| spec.id == id)
        .ok_or_else(|| LspError::Unknown(id.to_string()))
}

fn install_root(app: &AppHandle) -> Result<PathBuf, LspError> {
    let home = app
        .path()
        .home_dir()
        .map_err(|error| LspError::Io(error.to_string()))?;
    Ok(home.join(crate::APP_CONFIG_DIR).join(INSTALL_ROOT))
}

pub(crate) fn install_dir(app: &AppHandle, spec: &LanguageServerSpec) -> Result<PathBuf, LspError> {
    let name = spec.install.dir.as_deref().unwrap_or(&spec.id);
    Ok(install_root(app)?.join(name))
}

fn platform_key() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    format!("{os}-{arch}")
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if let Some(found) = executable_in_dir(&dir, name) {
            return Some(found);
        }
    }
    None
}

fn executable_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let candidate = dir.join(name);
    if is_runnable(&candidate) {
        return Some(candidate);
    }
    #[cfg(windows)]
    {
        for ext in [".exe", ".cmd", ".bat"] {
            let with_ext = dir.join(format!("{name}{ext}"));
            if is_runnable(&with_ext) {
                return Some(with_ext);
            }
        }
    }
    None
}

fn is_runnable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn managed_command_path(dir: &Path, spec: &LanguageServerSpec) -> Option<PathBuf> {
    match spec.install.method {
        InstallMethod::Npm => {
            executable_in_dir(&dir.join("node_modules").join(".bin"), &spec.command)
        }
        InstallMethod::Go => executable_in_dir(&dir.join("bin"), &spec.command),
        InstallMethod::Pip => {
            let unix = dir.join("venv").join("bin");
            if let Some(found) = executable_in_dir(&unix, &spec.command) {
                return Some(found);
            }
            executable_in_dir(&dir.join("venv").join("Scripts"), &spec.command)
        }
        InstallMethod::Github => github_command_path(dir, spec),
    }
}

fn github_command_path(dir: &Path, spec: &LanguageServerSpec) -> Option<PathBuf> {
    if let Some(rel) = spec.install.bin_path.as_deref() {
        let nested = dir.join(rel);
        if is_runnable(&nested) {
            return Some(nested);
        }
    }
    executable_in_dir(dir, &spec.command)
        .or_else(|| find_named_file(dir, &spec.command).filter(|path| is_runnable(&path)))
}

fn resolve_command(dir: &Path, spec: &LanguageServerSpec) -> Option<PathBuf> {
    managed_command_path(dir, spec)
}

fn missing_requires(spec: &LanguageServerSpec) -> Vec<String> {
    spec.requires
        .iter()
        .filter(|tool| find_in_path(tool).is_none())
        .cloned()
        .collect()
}

fn row_for(app: &AppHandle, spec: &LanguageServerSpec) -> Result<LanguageServerRow, LspError> {
    let dir = install_dir(app, spec)?;
    let command_path = resolve_command(&dir, spec);
    let mut row_spec = spec.clone();
    if let Some(path) = command_path.as_ref() {
        row_spec.command = path.to_string_lossy().into_owned();
    }
    Ok(LanguageServerRow {
        spec: row_spec,
        installed: command_path.is_some(),
        command_path: command_path.map(|path| path.to_string_lossy().into_owned()),
        missing_requires: missing_requires(spec),
    })
}

fn filename_matches(pattern: &str, name: &str) -> bool {
    if pattern.ends_with(".*") {
        let prefix = &pattern[..pattern.len() - 2];
        return name.eq_ignore_ascii_case(prefix)
            || name
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix));
    }
    name.eq_ignore_ascii_case(pattern)
}

fn spec_matches_path(spec: &LanguageServerSpec, path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if spec
        .filenames
        .iter()
        .any(|pattern| filename_matches(pattern, name))
    {
        return true;
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()));
    match ext {
        Some(ext) => spec
            .extensions
            .iter()
            .any(|item| item.eq_ignore_ascii_case(&ext)),
        None => false,
    }
}

pub(crate) fn specs_for_path(path: &Path) -> Result<Vec<&'static LanguageServerSpec>, LspError> {
    Ok(catalog()?
        .iter()
        .filter(|spec| spec_matches_path(spec, path))
        .collect())
}

pub(crate) fn language_id_for(path: &Path, spec: &LanguageServerSpec) -> String {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mapped = match ext.as_str() {
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" | "mts" | "cts" => "javascript",
        "jsx" => "javascriptreact",
        "py" | "pyi" => "python",
        "md" | "markdown" => "markdown",
        "yml" => "yaml",
        "jsonc" => "jsonc",
        "htm" => "html",
        "scss" | "less" => "css",
        other => other,
    };
    if spec.language_ids.iter().any(|id| id == mapped) {
        return mapped.to_string();
    }
    spec.language_ids
        .first()
        .cloned()
        .unwrap_or_else(|| mapped.to_string())
}

pub(crate) fn find_root(file: &Path, markers: &[String]) -> PathBuf {
    let mut dir = file.parent().unwrap_or(file).to_path_buf();
    loop {
        for marker in markers {
            if dir.join(marker).exists() {
                return dir;
            }
        }
        if !dir.pop() {
            break;
        }
    }
    file.parent().unwrap_or(file).to_path_buf()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLanguageServer {
    #[serde(flatten)]
    pub row: LanguageServerRow,
    pub language_id: String,
    pub root: String,
}

pub(crate) fn resolve_for_path(
    app: &AppHandle,
    path: &Path,
) -> Result<Option<ResolvedLanguageServer>, LspError> {
    let specs = specs_for_path(path)?;
    if specs.is_empty() {
        return Ok(None);
    }
    let mut fallback: Option<ResolvedLanguageServer> = None;
    for spec in specs {
        let row = row_for(app, spec)?;
        let resolved = ResolvedLanguageServer {
            language_id: language_id_for(path, spec),
            root: find_root(path, &spec.root_markers)
                .to_string_lossy()
                .into_owned(),
            row,
        };
        if resolved.row.installed {
            return Ok(Some(resolved));
        }
        if fallback.is_none() {
            fallback = Some(resolved);
        }
    }
    Ok(fallback)
}

#[tauri::command]
pub async fn resolve_language_server(
    app: AppHandle,
    path: String,
) -> Result<Option<ResolvedLanguageServer>, LspError> {
    let file = PathBuf::from(path.trim());
    if file.as_os_str().is_empty() {
        return Ok(None);
    }
    resolve_for_path(&app, &file)
}

#[tauri::command]
pub async fn list_language_servers(app: AppHandle) -> Result<Vec<LanguageServerRow>, LspError> {
    let servers = catalog()?;
    let mut rows = Vec::with_capacity(servers.len());
    for spec in servers {
        rows.push(row_for(&app, spec)?);
    }
    Ok(rows)
}

#[tauri::command]
pub async fn install_language_server(
    app: AppHandle,
    id: String,
) -> Result<LanguageServerRow, LspError> {
    let spec = spec_by_id(&id)?;
    let missing = missing_requires(spec);
    if !missing.is_empty() {
        return Err(LspError::MissingRequire(missing.join(", ")));
    }
    let dir = install_dir(&app, spec)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    match spec.install.method {
        InstallMethod::Npm => install_npm(&dir, spec).await?,
        InstallMethod::Go => install_go(&dir, spec).await?,
        InstallMethod::Pip => install_pip(&dir, spec).await?,
        InstallMethod::Github => install_github(&dir, spec).await?,
    }
    row_for(&app, spec)
}

async fn install_npm(dir: &Path, spec: &LanguageServerSpec) -> Result<(), LspError> {
    if spec.install.packages.is_empty() {
        return Err(LspError::Message(format!(
            "{} has no npm packages",
            spec.id
        )));
    }
    let npm = find_in_path("npm").ok_or_else(|| LspError::MissingRequire("npm".into()))?;
    let mut args = vec![
        "install".to_string(),
        "--omit=dev".to_string(),
        "--no-fund".to_string(),
        "--no-audit".to_string(),
        "--prefix".to_string(),
        dir.to_string_lossy().into_owned(),
    ];
    args.extend(spec.install.packages.clone());
    run_cmd(&npm, &args, None, NPM_INSTALL_TIMEOUT).await
}

async fn install_go(dir: &Path, spec: &LanguageServerSpec) -> Result<(), LspError> {
    let module = spec
        .install
        .module
        .as_deref()
        .ok_or_else(|| LspError::Message(format!("{} has no go module", spec.id)))?;
    let go = find_in_path("go").ok_or_else(|| LspError::MissingRequire("go".into()))?;
    let bin = dir.join("bin");
    tokio::fs::create_dir_all(&bin)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    let gobin = bin.to_string_lossy().into_owned();
    run_cmd(
        &go,
        &["install".to_string(), module.to_string()],
        Some(&[("GOBIN", gobin.as_str())]),
        GO_INSTALL_TIMEOUT,
    )
    .await
}

async fn install_pip(dir: &Path, spec: &LanguageServerSpec) -> Result<(), LspError> {
    if spec.install.packages.is_empty() {
        return Err(LspError::Message(format!(
            "{} has no pip packages",
            spec.id
        )));
    }
    let python = find_in_path("python3")
        .or_else(|| find_in_path("python"))
        .ok_or_else(|| LspError::MissingRequire("python3".into()))?;
    let venv = dir.join("venv");
    run_cmd(
        &python,
        &[
            "-m".to_string(),
            "venv".to_string(),
            venv.to_string_lossy().into_owned(),
        ],
        None,
        PIP_INSTALL_TIMEOUT,
    )
    .await?;
    let pip = executable_in_dir(&venv.join("bin"), "pip")
        .or_else(|| executable_in_dir(&venv.join("Scripts"), "pip"))
        .ok_or_else(|| LspError::Message("venv pip missing".into()))?;
    let mut args = vec![
        "install".to_string(),
        "--disable-pip-version-check".to_string(),
    ];
    args.extend(spec.install.packages.clone());
    run_cmd(&pip, &args, None, PIP_INSTALL_TIMEOUT).await
}

async fn install_github(dir: &Path, spec: &LanguageServerSpec) -> Result<(), LspError> {
    let repo = spec
        .install
        .repo
        .as_deref()
        .ok_or_else(|| LspError::Message(format!("{} has no github repo", spec.id)))?;
    let needle = spec
        .install
        .asset_contains
        .get(&platform_key())
        .ok_or_else(|| {
            LspError::Message(format!("{} has no asset for {}", spec.id, platform_key()))
        })?;
    let client = reqwest::Client::builder()
        .timeout(GITHUB_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| LspError::Message(error.to_string()))?;
    let release: GithubRelease = client
        .get(format!(
            "https://api.github.com/repos/{repo}/releases/latest"
        ))
        .send()
        .await
        .map_err(|error| LspError::Message(error.to_string()))?
        .error_for_status()
        .map_err(|error| LspError::Message(error.to_string()))?
        .json()
        .await
        .map_err(|error| LspError::Message(error.to_string()))?;
    let asset = release
        .assets
        .iter()
        .find(|item| item.name.contains(needle))
        .ok_or_else(|| LspError::Message(format!("no github asset matching {needle}")))?;
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|error| LspError::Message(error.to_string()))?
        .error_for_status()
        .map_err(|error| LspError::Message(error.to_string()))?
        .bytes()
        .await
        .map_err(|error| LspError::Message(error.to_string()))?;
    let download_path = dir.join(&asset.name);
    tokio::fs::write(&download_path, &bytes)
        .await
        .map_err(|error| LspError::Io(error.to_string()))?;
    extract_asset(dir, &download_path, &asset.name, spec).await?;
    Ok(())
}

async fn extract_asset(
    dir: &Path,
    download: &Path,
    asset_name: &str,
    spec: &LanguageServerSpec,
) -> Result<(), LspError> {
    let dest = dir.join(&spec.command);
    let kind = archive_kind(asset_name, spec.install.archive.as_deref());
    match kind {
        "none" => {
            tokio::fs::copy(download, &dest)
                .await
                .map_err(|error| LspError::Io(error.to_string()))?;
            make_executable(&dest)?;
        }
        "gz" => {
            let gzip =
                find_in_path("gzip").ok_or_else(|| LspError::MissingRequire("gzip".into()))?;
            let output = tokio::process::Command::new(&gzip)
                .args(["-dc", &download.to_string_lossy()])
                .output()
                .await
                .map_err(|error| LspError::Io(error.to_string()))?;
            if !output.status.success() {
                return Err(LspError::Message(stderr_or_status("gzip", &output)));
            }
            tokio::fs::write(&dest, output.stdout)
                .await
                .map_err(|error| LspError::Io(error.to_string()))?;
            make_executable(&dest)?;
        }
        "zip" => {
            let unzip =
                find_in_path("unzip").ok_or_else(|| LspError::MissingRequire("unzip".into()))?;
            run_cmd(
                &unzip,
                &[
                    "-o".to_string(),
                    download.to_string_lossy().into_owned(),
                    "-d".to_string(),
                    dir.to_string_lossy().into_owned(),
                ],
                None,
                GITHUB_TIMEOUT,
            )
            .await?;
            let bin_rel = spec.install.bin_path.as_deref().unwrap_or(&spec.command);
            ensure_github_bin(dir, bin_rel, &dest)?;
        }
        "tar.gz" => {
            let tar = find_in_path("tar").ok_or_else(|| LspError::MissingRequire("tar".into()))?;
            run_cmd(
                &tar,
                &[
                    "-xzf".to_string(),
                    download.to_string_lossy().into_owned(),
                    "-C".to_string(),
                    dir.to_string_lossy().into_owned(),
                ],
                None,
                GITHUB_TIMEOUT,
            )
            .await?;
            let bin_rel = spec.install.bin_path.as_deref().unwrap_or(&spec.command);
            ensure_github_bin(dir, bin_rel, &dest)?;
        }
        other => return Err(LspError::Message(format!("unsupported archive: {other}"))),
    }
    Ok(())
}

fn archive_kind(asset_name: &str, fallback: Option<&str>) -> &'static str {
    if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
        return "tar.gz";
    }
    if asset_name.ends_with(".gz") {
        return "gz";
    }
    if asset_name.ends_with(".zip") {
        return "zip";
    }
    match fallback {
        Some("gz") => "gz",
        Some("zip") => "zip",
        Some("tar.gz") => "tar.gz",
        _ => "none",
    }
}

fn ensure_github_bin(dir: &Path, bin_rel: &str, dest: &Path) -> Result<(), LspError> {
    let nested = dir.join(bin_rel);
    let source = if is_runnable(&nested) {
        nested
    } else {
        find_named_file(
            dir,
            dest.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(bin_rel),
        )
        .filter(|path| is_runnable(path))
        .ok_or_else(|| LspError::Message(format!("extracted binary missing: {bin_rel}")))?
    };
    make_executable(&source)?;
    Ok(())
}

fn find_named_file(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|item| item.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

fn make_executable(path: &Path) -> Result<(), LspError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|error| LspError::Io(error.to_string()))?
            .permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms).map_err(|error| LspError::Io(error.to_string()))?;
    }
    let _ = path;
    Ok(())
}

async fn run_cmd(
    program: &Path,
    args: &[String],
    env: Option<&[(&str, &str)]>,
    timeout: Duration,
) -> Result<(), LspError> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    if let Some(pairs) = env {
        for (key, value) in pairs {
            cmd.env(key, value);
        }
    }
    let output = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| LspError::Message(format!("{} timed out", program.display())))?
        .map_err(|error| LspError::Io(error.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    Err(LspError::Message(stderr_or_status(
        &program.display().to_string(),
        &output,
    )))
}

fn stderr_or_status(name: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("{name} failed with status {}", output.status)
    } else {
        stderr
    }
}
