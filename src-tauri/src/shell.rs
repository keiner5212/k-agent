use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
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
const STREAM_FLUSH_BYTES: usize = 4096;

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

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ShellChunkKind {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellChunk {
    pub kind: ShellChunkKind,
    pub text: String,
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

fn take_utf8(pending: &mut Vec<u8>, incoming: &[u8]) -> String {
    if incoming.is_empty() && pending.is_empty() {
        return String::new();
    }
    pending.extend_from_slice(incoming);
    let mut out = String::new();
    loop {
        match std::str::from_utf8(pending) {
            Ok(s) => {
                out.push_str(s);
                pending.clear();
                break;
            }
            Err(err) => {
                let valid = err.valid_up_to();
                if valid > 0 {
                    let prefix = std::str::from_utf8(&pending[..valid]).unwrap_or("");
                    out.push_str(prefix);
                    pending.drain(..valid);
                }
                match err.error_len() {
                    Some(len) => {
                        let len = len.min(pending.len());
                        if len == 0 {
                            break;
                        }
                        out.push('\u{FFFD}');
                        pending.drain(..len);
                    }
                    None => break,
                }
            }
        }
    }
    out
}

fn finish_utf8(pending: &mut Vec<u8>) -> String {
    let mut out = take_utf8(pending, &[]);
    if !pending.is_empty() {
        out.push('\u{FFFD}');
        pending.clear();
    }
    out
}

fn flush_emit(
    on_chunk: Option<&Channel<ShellChunk>>,
    kind: ShellChunkKind,
    emit_buf: &mut String,
    force: bool,
) {
    if emit_buf.is_empty() {
        return;
    }
    if !force && emit_buf.len() < STREAM_FLUSH_BYTES {
        return;
    }
    if let Some(channel) = on_chunk {
        let _ = channel.send(ShellChunk {
            kind,
            text: std::mem::take(emit_buf),
        });
    } else {
        emit_buf.clear();
    }
}

fn push_stream_bytes(
    on_chunk: Option<&Channel<ShellChunk>>,
    kind: ShellChunkKind,
    utf8_pending: &mut Vec<u8>,
    emit_buf: &mut String,
    bytes: &[u8],
) {
    let Some(channel) = on_chunk else {
        return;
    };
    let text = take_utf8(utf8_pending, bytes);
    if text.is_empty() {
        return;
    }
    emit_buf.push_str(&text);
    flush_emit(Some(channel), kind, emit_buf, false);
}

fn finish_stream(
    on_chunk: Option<&Channel<ShellChunk>>,
    kind: ShellChunkKind,
    utf8_pending: &mut Vec<u8>,
    emit_buf: &mut String,
) {
    let Some(channel) = on_chunk else {
        return;
    };
    let rest = finish_utf8(utf8_pending);
    if !rest.is_empty() {
        emit_buf.push_str(&rest);
    }
    flush_emit(Some(channel), kind, emit_buf, true);
}

async fn read_streaming<R>(
    reader: &mut R,
    cap: usize,
    on_chunk: Option<Channel<ShellChunk>>,
    kind: ShellChunkKind,
) -> (String, bool)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = Vec::with_capacity(cap.min(8192));
    let mut chunk = [0u8; 4096];
    let mut utf8_pending = Vec::new();
    let mut emit_buf = String::new();
    let mut truncated = false;
    loop {
        match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                if truncated {
                    continue;
                }
                let take = if buf.len() + n > cap {
                    truncated = true;
                    cap.saturating_sub(buf.len())
                } else {
                    n
                };
                if take == 0 {
                    continue;
                }
                buf.extend_from_slice(&chunk[..take]);
                push_stream_bytes(
                    on_chunk.as_ref(),
                    kind,
                    &mut utf8_pending,
                    &mut emit_buf,
                    &chunk[..take],
                );
            }
            Err(_) => {
                if !truncated {
                    truncated = true;
                }
                break;
            }
        }
    }
    finish_stream(on_chunk.as_ref(), kind, &mut utf8_pending, &mut emit_buf);
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
    on_chunk: Channel<ShellChunk>,
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

    let stdout_chunk = on_chunk.clone();
    let stderr_chunk = on_chunk;
    let stdout_task = tokio::spawn(async move {
        read_streaming(
            &mut stdout_reader,
            cap,
            Some(stdout_chunk),
            ShellChunkKind::Stdout,
        )
        .await
    });
    let stderr_task = tokio::spawn(async move {
        read_streaming(
            &mut stderr_reader,
            cap,
            Some(stderr_chunk),
            ShellChunkKind::Stderr,
        )
        .await
    });

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
