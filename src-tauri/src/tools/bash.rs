use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};

use super::tool_utils::workspace::confirm_action;
use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "bash";

const DESCRIPTION: &str = "Run one shell command in the workspace and wait for it to finish. Exact blockedCommands never run. Exact allowedCommands run without a prompt. Destructive commands, real file redirects, and non-local network commands wait for the user. `2>&1` and redirects to `/dev/null` do not ask. curl or wget to localhost, 127.0.0.1, or ::1 does not ask. Do not start a dev server here. Use background for a process that must stay up until the turn ends. Do not use &, nohup, or disown. Do not list, read, search, write, edit, or delete files here. Use list_directory, read, grep, write, edit, create_folder, and delete. bash is for install, build, test, and git. Output is capped.";

const MAX_OUTPUT_CHARS: usize = 50_000;
const TIMEOUT: Duration = Duration::from_secs(30);

const DANGEROUS: &[&str] = &[
    "rm", "rmdir", "mv", "cp", "chmod", "chown", "chgrp", "sudo", "su", "doas", "dd", "mkfs",
    "shutdown", "reboot", "poweroff", "halt", "kill", "killall", "pkill", "curl", "wget", "nc",
    "ncat", "ssh", "scp", "rsync", "docker", "podman", "kubectl",
];

const GIT_SAFE: &[&str] = &[
    "status",
    "diff",
    "log",
    "show",
    "blame",
    "ls-files",
    "rev-parse",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Run,
    Allowed,
    Blocked,
    Confirm,
    Empty,
    Background,
}

pub struct BashTool;

impl Tool for BashTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "minLength": 1,
                        "description": "One shell command. The full string is matched against the allow and block lists."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(command) = args.get("command").and_then(Value::as_str) else {
            return error_outcome("bash requires a string `command`.");
        };
        match classify(command, &ctx.allowed_commands, &ctx.blocked_commands) {
            Decision::Empty => error_outcome("bash `command` is empty."),
            Decision::Blocked => error_outcome("bash refused: command is on the block list."),
            Decision::Background => error_outcome(
                "bash does not start background jobs. Do not use &, nohup, or disown.",
            ),
            Decision::Confirm => {
                error_outcome("bash: a dangerous command must run on the async dispatch path.")
            }
            Decision::Run | Decision::Allowed => run_command(ctx, command.trim()),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let Some(command) = args.get("command").and_then(Value::as_str) else {
        return error_outcome("bash requires a string `command`.");
    };
    let command = command.trim();
    match classify(command, &ctx.allowed_commands, &ctx.blocked_commands) {
        Decision::Empty => error_outcome("bash `command` is empty."),
        Decision::Blocked => error_outcome("bash refused: command is on the block list."),
        Decision::Background => {
            error_outcome("bash does not start background jobs. Do not use &, nohup, or disown.")
        }
        Decision::Confirm => {
            if shell_session_granted(ctx) {
                return run_command(ctx, command);
            }
            match confirm_action(
                ctx,
                "bash_confirm",
                "Shell",
                &format!("Run this command?\n{command}"),
            )
            .await
            {
                Ok(super::tool_utils::workspace::ConfirmChoice::Deny) => {
                    error_outcome("User denied the command.")
                }
                Ok(super::tool_utils::workspace::ConfirmChoice::Once) => run_command(ctx, command),
                Ok(super::tool_utils::workspace::ConfirmChoice::Session) => {
                    grant_shell_session(ctx.session_id.as_deref());
                    run_command(ctx, command)
                }
                Err(message) => error_outcome(&message),
            }
        }
        Decision::Run | Decision::Allowed => run_command(ctx, command),
    }
}

pub(crate) fn classify(command: &str, allowed: &[String], blocked: &[String]) -> Decision {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Decision::Empty;
    }
    if blocked.iter().any(|item| item == trimmed) {
        return Decision::Blocked;
    }
    if allowed.iter().any(|item| item == trimmed) {
        return Decision::Allowed;
    }
    if is_background_job(trimmed) {
        return Decision::Background;
    }
    if is_dangerous(trimmed) {
        Decision::Confirm
    } else {
        Decision::Run
    }
}

fn is_background_job(command: &str) -> bool {
    if has_single_ampersand(command) {
        return true;
    }
    for segment in split_segments(command) {
        let tokens = tokenize(segment);
        let Some(name) = command_name(&tokens) else {
            continue;
        };
        if name.eq_ignore_ascii_case("nohup") || name.eq_ignore_ascii_case("disown") {
            return true;
        }
    }
    false
}

fn has_single_ampersand(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut quote: Option<char> = None;
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(mark) = quote {
            if ch == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == '&' {
            let prev = if index > 0 { chars[index - 1] } else { '\0' };
            let next = chars.get(index + 1).copied().unwrap_or('\0');
            if prev != '&' && next != '&' && prev != '>' && next != '>' {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn is_dangerous(command: &str) -> bool {
    if has_unquoted(command, "$(") || command.contains('`') {
        return true;
    }
    if has_file_redirect(command) {
        return true;
    }
    if pipes_to_shell(command) {
        return true;
    }
    for segment in split_segments(command) {
        let tokens = tokenize(segment);
        let Some(name) = command_name(&tokens) else {
            continue;
        };
        if name.eq_ignore_ascii_case("curl") || name.eq_ignore_ascii_case("wget") {
            if !loopback_fetch(&tokens) {
                return true;
            }
            continue;
        }
        if DANGEROUS.iter().any(|item| name.eq_ignore_ascii_case(item)) {
            return true;
        }
        if name.eq_ignore_ascii_case("git") {
            let sub = tokens.get(1).map(|item| item.as_str()).unwrap_or("");
            if !GIT_SAFE.iter().any(|item| sub.eq_ignore_ascii_case(item)) {
                return true;
            }
        }
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "node" | "python" | "python3" | "perl" | "ruby" | "php"
        ) && tokens.iter().any(|token| token == "-e" || token == "-c")
        {
            return true;
        }
    }
    false
}

fn split_segments(command: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = command.as_bytes();
    let mut index = 0usize;
    let mut quote: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(mark) = quote {
            if byte == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'\'' || byte == b'"' {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if byte == b';' || byte == b'\n' || byte == b'|' || byte == b'&' {
            out.push(command[start..index].trim());
            index += 1;
            start = index;
            continue;
        }
        index += 1;
    }
    let tail = command[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out.into_iter().filter(|item| !item.is_empty()).collect()
}

fn tokenize(segment: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in segment.chars() {
        if let Some(mark) = quote {
            if ch == mark {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn command_name(tokens: &[String]) -> Option<String> {
    let mut index = 0usize;
    while index < tokens.len() && is_env_assignment(&tokens[index]) {
        index += 1;
    }
    tokens.get(index).map(|token| {
        token
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(token)
            .to_string()
    })
}

fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn has_unquoted(command: &str, needle: &str) -> bool {
    let mut quote: Option<char> = None;
    let chars: Vec<char> = command.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(mark) = quote {
            if ch == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if chars[index..].starts_with(&needle) {
            return true;
        }
        index += 1;
    }
    false
}

fn has_file_redirect(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut quote: Option<char> = None;
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(mark) = quote {
            if ch == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == '>' && !harmless_redirect(&chars[index..]) {
            return true;
        }
        if ch == '<' && chars.get(index + 1) == Some(&'<') {
            return true;
        }
        index += 1;
    }
    false
}

fn harmless_redirect(from_gt: &[char]) -> bool {
    let mut index = 1usize;
    if from_gt.get(index) == Some(&'>') {
        index += 1;
    }
    if from_gt.get(index) == Some(&'&') {
        index += 1;
        if from_gt.get(index).is_some_and(|ch| ch.is_ascii_digit()) {
            return true;
        }
    }
    while from_gt.get(index).is_some_and(|ch| ch.is_whitespace()) {
        index += 1;
    }
    let rest: String = from_gt[index..].iter().collect();
    let token = rest
        .split(|ch: char| ch.is_whitespace() || ch == ';' || ch == '|' || ch == '&')
        .next()
        .unwrap_or("");
    let token = token.trim_matches('"').trim_matches('\'');
    token == "/dev/null"
}

fn loopback_fetch(tokens: &[String]) -> bool {
    let mut urls = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        if token == "-o" || token == "--output" || token == "-O" {
            let dest = tokens.get(index + 1).map(String::as_str).unwrap_or("");
            if dest != "/dev/null" {
                return false;
            }
        }
        if token.starts_with("http://") || token.starts_with("https://") {
            urls.push(token.as_str());
        }
        index += 1;
    }
    !urls.is_empty() && urls.iter().all(|url| is_loopback_url(url))
}

fn is_loopback_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.trim_matches(|ch| ch == '[' || ch == ']');
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

fn pipes_to_shell(command: &str) -> bool {
    for segment in split_segments(command).into_iter().skip(1) {
        let tokens = tokenize(segment);
        if let Some(name) = command_name(&tokens) {
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "sh" | "bash" | "zsh" | "dash" | "fish" | "pwsh" | "powershell"
            ) {
                return true;
            }
        }
    }
    false
}

fn run_command(ctx: &ToolContext<'_>, command: &str) -> ToolOutcome {
    let Some(workspace) = ctx.workspace_path() else {
        return error_outcome("bash needs a workspace.");
    };
    let shell = if ctx.shell_program.trim().is_empty() {
        default_shell()
    } else {
        ctx.shell_program.trim().to_string()
    };
    let flag =
        if shell.to_ascii_lowercase().ends_with("cmd.exe") || shell.eq_ignore_ascii_case("cmd") {
            "/c"
        } else {
            "-c"
        };
    let mut child = match Command::new(&shell)
        .arg(flag)
        .arg(command)
        .current_dir(&workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return error_outcome(&format!("bash could not start `{shell}`: {error}"));
        }
    };
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_task = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = stdout_pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    });
    let stderr_task = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    });
    let started = std::time::Instant::now();
    let status = loop {
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_task.join();
            let _ = stderr_task.join();
            return error_outcome("bash timed out after 30s.");
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = stdout_task.join();
                let _ = stderr_task.join();
                return error_outcome(&format!("bash: {error}"));
            }
        }
    };
    let stdout = stdout_task.join().unwrap_or_default();
    let stderr = stderr_task.join().unwrap_or_default();
    let code = status.code().unwrap_or(-1);
    let output = clip(&format!(
        "{}{}",
        stdout,
        if stderr.is_empty() {
            String::new()
        } else {
            format!("\n{stderr}")
        }
    ));
    let label = if status.success() { "ok" } else { "error" };
    let text = toon_doc(&[
        ("status", ToonValue::Str(label)),
        ("exitCode", ToonValue::Int(code as i64)),
        ("output", ToonValue::Block(&output)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            status: Some(label.into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn default_shell() -> String {
    if cfg!(windows) {
        "cmd.exe".into()
    } else {
        "/bin/sh".into()
    }
}

fn clip(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars().take(MAX_OUTPUT_CHARS) {
        out.push(ch);
    }
    if text.chars().count() > MAX_OUTPUT_CHARS {
        out.push_str("\n... (truncated)");
    }
    out
}

fn error_outcome(message: &str) -> ToolOutcome {
    let text = toon_doc(&[
        ("status", ToonValue::Str("error")),
        ("error", ToonValue::Block(message)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            status: Some("error".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn shell_grants() -> &'static Mutex<std::collections::HashSet<String>> {
    static GRANTS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    GRANTS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

pub(crate) fn shell_session_granted(ctx: &ToolContext<'_>) -> bool {
    let Some(session_id) = ctx.session_id.as_deref().filter(|id| !id.is_empty()) else {
        return false;
    };
    shell_grants()
        .lock()
        .map(|grants| grants.contains(session_id))
        .unwrap_or(false)
}

pub fn grant_shell_session(session_id: Option<&str>) {
    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return;
    };
    if let Ok(mut grants) = shell_grants().lock() {
        grants.insert(session_id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_list_wins_over_allow_list() {
        let decision = classify("rm -rf /", &["rm -rf /".into()], &["rm -rf /".into()]);
        assert_eq!(decision, Decision::Blocked);
    }

    #[test]
    fn allow_list_skips_danger_check() {
        let decision = classify("rm -rf /tmp/x", &["rm -rf /tmp/x".into()], &[]);
        assert_eq!(decision, Decision::Allowed);
    }

    #[test]
    fn rm_and_redirects_need_confirm() {
        assert_eq!(classify("rm file", &[], &[]), Decision::Confirm);
        assert_eq!(classify("echo hi > out", &[], &[]), Decision::Confirm);
        assert_eq!(
            classify("npm run build 2>&1 | tail -20", &[], &[]),
            Decision::Run
        );
        assert_eq!(
            classify("ss -tlnp 2>/dev/null | head -20", &[], &[]),
            Decision::Run
        );
        assert_eq!(
            classify("curl -s -o /dev/null http://[::1]:5173/", &[], &[]),
            Decision::Run
        );
        assert_eq!(
            classify("curl https://example.com", &[], &[]),
            Decision::Confirm
        );
    }

    #[test]
    fn git_status_runs() {
        assert_eq!(classify("git status", &[], &[]), Decision::Run);
        assert_eq!(classify("git commit -m x", &[], &[]), Decision::Confirm);
    }

    #[test]
    fn background_jobs_are_refused() {
        assert_eq!(
            classify("npm run dev > /tmp/vite.log 2>&1 &", &[], &[]),
            Decision::Background
        );
        assert_eq!(
            classify("nohup npx vite preview --port 4173 &", &[], &[]),
            Decision::Background
        );
        assert_eq!(classify("echo a && echo b", &[], &[]), Decision::Run);
    }
}
