use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::LocalWorkspace;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_TIMEOUT_MS: u64 = 10 * 60_000;
const MAX_OUTPUT_BYTES_CAP: usize = 8 * 1024 * 1024;
const PIPE_DRAIN_MS: u64 = 1_500;
const KILL_WAIT_MS: u64 = 400;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunShellInput {
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub max_output_bytes: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunShellResult {
    pub command: String,
    pub cwd: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
    pub started_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("empty command")]
    EmptyCommand,
    #[error("workspace root is not set")]
    MissingWorkspace,
    #[error("working directory does not exist: {0}")]
    CwdMissing(String),
    #[error("working directory escapes workspace root")]
    CwdEscape,
    #[error("io error: {0}")]
    Io(String),
}

impl Serialize for ShellError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        crate::serialize_error(self, serializer)
    }
}

fn clamp_timeout(value: Option<u64>) -> u64 {
    match value {
        Some(0) | None => DEFAULT_TIMEOUT_MS,
        Some(v) => v.min(MAX_TIMEOUT_MS),
    }
}

fn clamp_output_cap(value: Option<usize>) -> usize {
    match value {
        None => DEFAULT_MAX_OUTPUT_BYTES,
        Some(v) if v == 0 => DEFAULT_MAX_OUTPUT_BYTES,
        Some(v) => v.min(MAX_OUTPUT_BYTES_CAP),
    }
}

fn resolve_cwd(cwd: Option<&str>, workspace_root: &Path) -> Result<PathBuf, ShellError> {
    match cwd.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(workspace_root.to_path_buf()),
        Some(raw) => {
            let candidate = {
                let path = Path::new(raw);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    workspace_root.join(path)
                }
            };
            let resolved = dunce::canonicalize(&candidate)
                .map_err(|e| ShellError::CwdMissing(format!("{raw}: {e}")))?;
            if !resolved.starts_with(workspace_root) {
                return Err(ShellError::CwdEscape);
            }
            Ok(resolved)
        }
    }
}

#[cfg(unix)]
fn shell_program() -> &'static str {
    "/bin/sh"
}

#[cfg(unix)]
fn shell_arg() -> &'static str {
    "-c"
}

#[cfg(windows)]
fn shell_program() -> &'static str {
    "cmd.exe"
}

#[cfg(windows)]
fn shell_arg() -> &'static str {
    "/C"
}

async fn read_capped<R>(reader: &mut R, cap: usize) -> (String, bool)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(cap.min(8192));
    let mut chunk = [0u8; 4096];
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                if truncated {
                    continue;
                }
                if buf.len() + n > cap {
                    let remaining = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..remaining]);
                    truncated = true;
                    continue;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(_) => {
                if !truncated {
                    truncated = true;
                }
                break;
            }
        }
    }
    let s = String::from_utf8_lossy(&buf).into_owned();
    (s, truncated)
}

#[cfg(unix)]
extern "C" {
    fn setsid() -> i32;
}

#[cfg(unix)]
fn kill_tree(child: &tokio::process::Child) {
    if let Some(pid) = child.id() {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

#[cfg(not(unix))]
fn kill_tree(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
}

fn result_from_pipes(
    command: String,
    cwd: String,
    stdout: (String, bool),
    stderr: (String, bool),
    exit_code: Option<i32>,
    timed_out: bool,
    started: Instant,
    started_at_ms: u64,
) -> RunShellResult {
    RunShellResult {
        command,
        cwd,
        stdout: stdout.0,
        stderr: stderr.0,
        exit_code,
        timed_out,
        stdout_truncated: stdout.1,
        stderr_truncated: stderr.1,
        duration_ms: started.elapsed().as_millis() as u64,
        started_at_ms,
    }
}

fn pipes_or_empty(
    stdout: Result<(String, bool), tokio::task::JoinError>,
    stderr: Result<(String, bool), tokio::task::JoinError>,
) -> ((String, bool), (String, bool)) {
    (
        stdout.unwrap_or_else(|_| (String::new(), true)),
        stderr.unwrap_or_else(|_| (String::new(), true)),
    )
}

async fn join_pipes(
    stdout_task: JoinHandle<(String, bool)>,
    stderr_task: JoinHandle<(String, bool)>,
) -> ((String, bool), (String, bool)) {
    let (stdout, stderr) = tokio::join!(stdout_task, stderr_task);
    pipes_or_empty(stdout, stderr)
}

async fn collect_pipes_after_kill(
    stdout_task: JoinHandle<(String, bool)>,
    stderr_task: JoinHandle<(String, bool)>,
) -> ((String, bool), (String, bool)) {
    let stdout_abort = stdout_task.abort_handle();
    let stderr_abort = stderr_task.abort_handle();
    match timeout(Duration::from_millis(PIPE_DRAIN_MS), async {
        tokio::join!(stdout_task, stderr_task)
    })
    .await
    {
        Ok((stdout, stderr)) => pipes_or_empty(stdout, stderr),
        Err(_) => {
            stdout_abort.abort();
            stderr_abort.abort();
            ((String::new(), true), (String::new(), true))
        }
    }
}

#[tauri::command]
pub async fn run_shell_command(
    state: State<'_, LocalWorkspace>,
    input: RunShellInput,
) -> Result<RunShellResult, ShellError> {
    let command = input.command.trim();
    if command.is_empty() {
        return Err(ShellError::EmptyCommand);
    }

    let workspace_root = state
        .cloned_path()
        .ok_or(ShellError::MissingWorkspace)
        .and_then(|path| {
            dunce::canonicalize(&path)
                .map_err(|e| ShellError::CwdMissing(format!("workspace_root: {e}")))
        })?;

    let cwd = resolve_cwd(input.cwd.as_deref(), &workspace_root)?;
    let timeout_ms = clamp_timeout(input.timeout_ms);
    let cap = clamp_output_cap(input.max_output_bytes);

    let started = Instant::now();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut cmd = Command::new(shell_program());
    cmd.arg(shell_arg())
        .arg(command)
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.as_std_mut().pre_exec(|| {
                if setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| ShellError::Io(format!("spawn: {e}")))?;

    let mut stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| ShellError::Io("stdout pipe missing".into()))?;
    let mut stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| ShellError::Io("stderr pipe missing".into()))?;

    let stdout_task = tokio::spawn(async move { read_capped(&mut stdout_reader, cap).await });
    let stderr_task = tokio::spawn(async move { read_capped(&mut stderr_reader, cap).await });

    let exit_status = match timeout(Duration::from_millis(timeout_ms), child.wait()).await {
        Ok(result) => result.map_err(|e| ShellError::Io(format!("wait: {e}")))?,
        Err(_) => {
            kill_tree(&mut child);
            let _ = child.start_kill();
            let _ = timeout(Duration::from_millis(KILL_WAIT_MS), child.wait()).await;
            let (stdout, stderr) = collect_pipes_after_kill(stdout_task, stderr_task).await;
            return Ok(result_from_pipes(
                command.to_string(),
                cwd.to_string_lossy().into_owned(),
                stdout,
                stderr,
                None,
                true,
                started,
                started_at_ms,
            ));
        }
    };

    let (stdout, stderr) = join_pipes(stdout_task, stderr_task).await;
    Ok(result_from_pipes(
        command.to_string(),
        cwd.to_string_lossy().into_owned(),
        stdout,
        stderr,
        exit_status.code(),
        false,
        started,
        started_at_ms,
    ))
}
