use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};

use super::tool_utils::workspace::confirm_action;
use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_ACTION,
};

pub const NAME: &str = "bash";

const DESCRIPTION: &str = "Run one shell command in the workspace and wait for it to finish. `description` is required on every call, including safe, allowed, and blocked commands: one short sentence, what this command does and why. Exact blockedCommands never run. Exact allowedCommands run without a prompt. Destructive commands, real file redirects, and non-local network commands wait for the user. `2>&1` and redirects to `/dev/null` do not ask. curl to localhost, 127.0.0.1, or ::1 does not ask for a local read (silent, head, fail, timeout, header, literal body, or output to /dev/null). Any other curl flag, a config file, a proxy, an upload, a redirect, or a second command waits for the user. Do not start a dev server here. Use background for a process that must stay up until the turn ends. Do not kill a pid returned by background. That process is stopped when the turn ends. Do not use &, nohup, or disown. Do not list, read, search, write, edit, or delete files here. Use glob, read, grep, write, edit, create_folder, and delete. bash is for install, build, test, and git. Output is capped.";

const MAX_OUTPUT_CHARS: usize = 50_000;
const TIMEOUT: Duration = Duration::from_secs(30);

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
                        "description": "One shell command."
                    },
                    "description": {
                        "type": "string",
                        "minLength": 1,
                        "description": "One short sentence: what this command does and why. Required on every call, including safe, allowed, and blocked commands."
                    }
                },
                "required": ["command", "description"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let (command, _description) = match bash_args(args) {
            Ok(pair) => pair,
            Err(outcome) => return *outcome,
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
    let (command, description) = match bash_args(&args) {
        Ok(pair) => pair,
        Err(outcome) => return *outcome,
    };
    let command = command.trim();
    if let Some(message) = background_kill_refusal(command, ctx) {
        return error_outcome(&message);
    }
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
                &confirm_question(description, command),
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

fn bash_args(args: &Value) -> Result<(&str, &str), Box<ToolOutcome>> {
    let Some(command) = args.get("command").and_then(Value::as_str) else {
        return Err(Box::new(error_outcome("bash requires a string `command`.")));
    };
    let Some(description) = args.get("description").and_then(Value::as_str) else {
        return Err(Box::new(error_outcome("bash: `description` is required.")));
    };
    if description.trim().is_empty() {
        return Err(Box::new(error_outcome("bash: `description` is empty.")));
    }
    Ok((command, description.trim()))
}

fn confirm_question(description: &str, command: &str) -> String {
    let summary = description.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{summary}\n{command}")
}

fn background_kill_refusal(command: &str, ctx: &ToolContext<'_>) -> Option<String> {
    let turn = ctx.turn.as_ref()?;
    for segment in split_segments(command) {
        let tokens = tokenize(&segment);
        let Some(name) = command_name(&tokens) else {
            continue;
        };
        if !name.eq_ignore_ascii_case("kill") && !name.eq_ignore_ascii_case("pkill") {
            continue;
        }
        for token in &tokens {
            if token.starts_with('-') {
                continue;
            }
            let Ok(pid) = token.parse::<u32>() else {
                continue;
            };
            if turn.owns_pid(pid) {
                return Some(format!(
                    "bash refused: pid {pid} belongs to background. It is killed when the turn ends. Do not kill it."
                ));
            }
        }
    }
    None
}

const CLASSIFY_DEPTH: u32 = 32;

pub(crate) fn classify(command: &str, allowed: &[String], blocked: &[String]) -> Decision {
    classify_at(command, allowed, blocked, 0)
}

fn classify_at(command: &str, allowed: &[String], blocked: &[String], depth: u32) -> Decision {
    if depth > CLASSIFY_DEPTH {
        return Decision::Confirm;
    }
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Decision::Empty;
    }
    if command_blocked(trimmed, blocked, depth) {
        return Decision::Blocked;
    }
    if allowed.iter().any(|item| item == trimmed) {
        return Decision::Allowed;
    }
    if is_background_job(trimmed) {
        return Decision::Background;
    }
    let (segments, nested) = compound_segments(trimmed);
    if !segments.is_empty()
        && segments
            .iter()
            .all(|segment| allowed.iter().any(|item| item == segment))
    {
        return Decision::Allowed;
    }
    if pipes_to_shell(trimmed)
        || segments
            .iter()
            .any(|segment| segment_outer_dangerous(segment, trimmed))
    {
        return Decision::Confirm;
    }
    for body in nested_bodies(&segments, &nested) {
        if body.trim().is_empty() || body.trim() == trimmed {
            continue;
        }
        match classify_at(&body, allowed, blocked, depth + 1) {
            Decision::Blocked => return Decision::Blocked,
            Decision::Background => return Decision::Background,
            Decision::Confirm => return Decision::Confirm,
            Decision::Empty | Decision::Run | Decision::Allowed => {}
        }
    }
    Decision::Run
}

fn command_blocked(command: &str, blocked: &[String], depth: u32) -> bool {
    if depth > CLASSIFY_DEPTH || blocked.is_empty() {
        return false;
    }
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }
    if blocked.iter().any(|item| item == trimmed) {
        return true;
    }
    let (segments, nested) = compound_segments(trimmed);
    if segments
        .iter()
        .any(|segment| blocked.iter().any(|item| item == segment))
    {
        return true;
    }
    for body in nested_bodies(&segments, &nested) {
        if body.trim().is_empty() || body.trim() == trimmed {
            continue;
        }
        if command_blocked(&body, blocked, depth + 1) {
            return true;
        }
    }
    false
}

fn nested_bodies(segments: &[String], nested: &[String]) -> Vec<String> {
    let mut out = nested.to_vec();
    for segment in segments {
        if let Some(script) = shell_c_script(&tokenize(segment)) {
            out.push(script);
        }
    }
    out
}

fn segment_outer_dangerous(segment: &str, full: &str) -> bool {
    if has_file_redirect(segment) {
        return true;
    }
    let tokens = tokenize(segment);
    let Some(name) = command_name(&tokens) else {
        return false;
    };
    if name.eq_ignore_ascii_case("curl") {
        return !safe_curl_command(full);
    }
    if crate::compat::shell::is_dangerous_command(&name) {
        return true;
    }
    if name.eq_ignore_ascii_case("git") {
        let sub = tokens.get(1).map(|item| item.as_str()).unwrap_or("");
        return !GIT_SAFE.iter().any(|item| sub.eq_ignore_ascii_case(item));
    }
    matches!(
        name.to_ascii_lowercase().as_str(),
        "node" | "python" | "python3" | "perl" | "ruby" | "php"
    ) && tokens.iter().any(|token| token == "-e" || token == "-c")
}

fn is_background_job(command: &str) -> bool {
    if has_single_ampersand(command) {
        return true;
    }
    for segment in split_segments(command) {
        let tokens = tokenize(&segment);
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

enum ShellPart {
    Segment(String),
    Sep(String),
}

fn split_segments(command: &str) -> Vec<String> {
    compound_segments(command).0
}

fn compound_segments(command: &str) -> (Vec<String>, Vec<String>) {
    let (parts, nested) = scan_compound(command);
    let segments = parts
        .into_iter()
        .filter_map(|part| match part {
            ShellPart::Segment(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            ShellPart::Sep(_) => None,
        })
        .collect();
    (segments, nested)
}

fn scan_compound(command: &str) -> (Vec<ShellPart>, Vec<String>) {
    let chars: Vec<char> = command.chars().collect();
    let mut parts = Vec::new();
    let mut nested = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut quote: Option<char> = None;
    while index < chars.len() {
        if consume_meta(&chars, &mut index, &mut quote, Some(&mut nested)) {
            continue;
        }
        if let Some(len) = separator_len(&chars, index) {
            parts.push(ShellPart::Segment(slice_chars(&chars, start, index)));
            parts.push(ShellPart::Sep(slice_chars(&chars, index, index + len)));
            index += len;
            start = index;
            continue;
        }
        index += 1;
    }
    parts.push(ShellPart::Segment(slice_chars(&chars, start, chars.len())));
    (parts, nested)
}

fn slice_chars(chars: &[char], start: usize, end: usize) -> String {
    chars[start..end].iter().collect()
}

fn separator_len(chars: &[char], index: usize) -> Option<usize> {
    match chars.get(index).copied()? {
        ';' | '\n' => Some(1),
        '|' => {
            if chars.get(index + 1) == Some(&'|') || chars.get(index + 1) == Some(&'&') {
                Some(2)
            } else {
                Some(1)
            }
        }
        '&' => {
            if chars.get(index + 1) == Some(&'&') {
                return Some(2);
            }
            let prev = if index > 0 { chars[index - 1] } else { '\0' };
            let next = chars.get(index + 1).copied().unwrap_or('\0');
            if prev == '>' || next == '>' {
                None
            } else {
                Some(1)
            }
        }
        _ => None,
    }
}

fn consume_meta(
    chars: &[char],
    index: &mut usize,
    quote: &mut Option<char>,
    nested: Option<&mut Vec<String>>,
) -> bool {
    let at = *index;
    if at >= chars.len() {
        return false;
    }
    let ch = chars[at];
    if let Some(mark) = *quote {
        if mark == '\'' {
            if ch == '\'' {
                *quote = None;
            }
            *index += 1;
            return true;
        }
        if ch == '\\' && at + 1 < chars.len() {
            *index += 2;
            return true;
        }
        if ch == '"' {
            *quote = None;
            *index += 1;
            return true;
        }
        if ch == '`' {
            push_subst(chars, at, index, nested, true);
            return true;
        }
        if ch == '$' && chars.get(at + 1) == Some(&'(') {
            push_subst(chars, at, index, nested, false);
            return true;
        }
        *index += 1;
        return true;
    }
    if ch == '\\' && at + 1 < chars.len() {
        *index += 2;
        return true;
    }
    if ch == '\'' || ch == '"' {
        *quote = Some(ch);
        *index += 1;
        return true;
    }
    if ch == '`' {
        push_subst(chars, at, index, nested, true);
        return true;
    }
    if ch == '$' && chars.get(at + 1) == Some(&'(') {
        push_subst(chars, at, index, nested, false);
        return true;
    }
    false
}

fn push_subst(
    chars: &[char],
    at: usize,
    index: &mut usize,
    nested: Option<&mut Vec<String>>,
    backtick: bool,
) {
    let (body, next) = if backtick {
        take_backtick(chars, at)
    } else {
        take_dollar(chars, at)
    };
    if let Some(nested) = nested {
        let trimmed = body.trim();
        if !trimmed.is_empty() {
            nested.push(trimmed.to_string());
        }
    }
    *index = next;
}

fn take_dollar(chars: &[char], at: usize) -> (String, usize) {
    let mut index = at + 2;
    let start = index.min(chars.len());
    let mut depth = 1i32;
    let mut quote: Option<char> = None;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(mark) = quote {
            if mark == '"' && ch == '\\' && index + 1 < chars.len() {
                index += 2;
                continue;
            }
            if ch == mark {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '\\' && index + 1 < chars.len() {
            index += 2;
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if ch == '(' {
            depth += 1;
            index += 1;
            continue;
        }
        if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return (slice_chars(chars, start, index), index + 1);
            }
            index += 1;
            continue;
        }
        index += 1;
    }
    (slice_chars(chars, start, chars.len()), chars.len())
}

fn take_backtick(chars: &[char], at: usize) -> (String, usize) {
    let mut index = at + 1;
    let start = index.min(chars.len());
    while index < chars.len() {
        if chars[index] == '\\' && index + 1 < chars.len() {
            index += 2;
            continue;
        }
        if chars[index] == '`' {
            return (slice_chars(chars, start, index), index + 1);
        }
        index += 1;
    }
    (slice_chars(chars, start, chars.len()), chars.len())
}

fn shell_c_script(tokens: &[String]) -> Option<String> {
    let mut index = 0usize;
    while index < tokens.len() && is_env_assignment(&tokens[index]) {
        index += 1;
    }
    let name = tokens.get(index)?;
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    if !matches!(base.to_ascii_lowercase().as_str(), "sh" | "bash" | "zsh") {
        return None;
    }
    index += 1;
    while index < tokens.len() {
        let token = &tokens[index];
        if is_shell_c_flag(token) {
            let script = tokens.get(index + 1)?.trim();
            if script.is_empty() {
                return None;
            }
            return Some(script.to_string());
        }
        if token.starts_with('-') {
            index += 1;
            continue;
        }
        return None;
    }
    None
}

fn is_shell_c_flag(token: &str) -> bool {
    let Some(flags) = token.strip_prefix('-') else {
        return false;
    };
    if flags.is_empty() || flags.starts_with('-') {
        return false;
    }
    let mut saw_c = false;
    for ch in flags.chars() {
        match ch {
            'c' => saw_c = true,
            'l' | 'i' => {}
            _ => return false,
        }
    }
    saw_c
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
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
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
    tokens
        .get(index)
        .map(|token| crate::compat::shell::base_command_name(token))
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

fn has_file_redirect(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut quote: Option<char> = None;
    let mut index = 0usize;
    while index < chars.len() {
        if consume_meta(&chars, &mut index, &mut quote, None) {
            continue;
        }
        let ch = chars[index];
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
    token == crate::compat::shell::null_device()
}

fn is_loopback_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let host = host.trim_matches(|ch| ch == '[' || ch == ']');
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

pub(crate) struct RunPrep {
    pub command: String,
    pub clear_proxy: bool,
}

pub(crate) fn prepare_auto_run(command: &str) -> RunPrep {
    if safe_curl_command(command) {
        RunPrep {
            command: inject_curl_guards(command),
            clear_proxy: true,
        }
    } else {
        RunPrep {
            command: command.to_string(),
            clear_proxy: false,
        }
    }
}

fn safe_curl_command(command: &str) -> bool {
    let segments = split_segments(command);
    !segments.is_empty() && segments.iter().all(|segment| segment_is_safe_curl(segment))
}

fn segment_is_safe_curl(segment: &str) -> bool {
    let trimmed = segment.trim();
    let Some(word) = trimmed.split_whitespace().next() else {
        return false;
    };
    if word.contains('\'') || word.contains('"') {
        return false;
    }
    let base = word.rsplit(['/', '\\']).next().unwrap_or(word);
    if !base.eq_ignore_ascii_case("curl") {
        return false;
    }
    curl_invocation_safe(&tokenize(trimmed))
}

fn curl_invocation_safe(tokens: &[String]) -> bool {
    if tokens.iter().any(|token| is_env_assignment(token)) {
        return false;
    }
    let Some(name) = tokens.first() else {
        return false;
    };
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    if !base.eq_ignore_ascii_case("curl") {
        return false;
    }
    let mut index = 1usize;
    let mut urls = 0usize;
    while index < tokens.len() {
        let token = &tokens[index];
        if token == "--" {
            index += 1;
            while index < tokens.len() {
                if !is_loopback_url(&tokens[index]) {
                    return false;
                }
                urls += 1;
                index += 1;
            }
            break;
        }
        if let Some(rest) = token.strip_prefix("--") {
            if !take_curl_long(tokens, rest, &mut index, &mut urls) {
                return false;
            }
            continue;
        }
        if token.starts_with('-') {
            let Some(next) = take_curl_short(tokens, index) else {
                return false;
            };
            index = next;
            continue;
        }
        if is_loopback_url(token) {
            urls += 1;
            index += 1;
            continue;
        }
        return false;
    }
    urls > 0
}

fn take_curl_long(tokens: &[String], rest: &str, index: &mut usize, urls: &mut usize) -> bool {
    if rest.is_empty() {
        return false;
    }
    let (flag, inline) = match rest.split_once('=') {
        Some((flag, value)) => (flag, Some(value.to_string())),
        None => (rest, None),
    };
    if curl_long_bool(flag) {
        if inline.is_some() {
            return false;
        }
        *index += 1;
        return true;
    }
    if !curl_long_takes_value(flag) {
        return false;
    }
    let value = if let Some(value) = inline {
        value
    } else {
        let Some(next) = tokens.get(*index + 1) else {
            return false;
        };
        *index += 1;
        next.clone()
    };
    if !curl_long_value_ok(flag, &value) {
        return false;
    }
    if flag == "url" {
        *urls += 1;
    }
    *index += 1;
    true
}

fn take_curl_short(tokens: &[String], index: usize) -> Option<usize> {
    let flags = tokens.get(index)?.strip_prefix('-')?;
    if flags.is_empty() || flags.starts_with('-') {
        return None;
    }
    let chars: Vec<char> = flags.chars().collect();
    let mut cursor = 0usize;
    while cursor < chars.len() {
        let flag = chars[cursor];
        if curl_short_bool(flag) {
            cursor += 1;
            continue;
        }
        if !curl_short_takes_value(flag) {
            return None;
        }
        let inline: String = chars[cursor + 1..].iter().collect();
        let (value, advance) = if inline.is_empty() {
            (tokens.get(index + 1)?.clone(), 2)
        } else {
            (inline, 1)
        };
        if !curl_short_value_ok(flag, &value) {
            return None;
        }
        return Some(index + advance);
    }
    Some(index + 1)
}

fn curl_long_bool(flag: &str) -> bool {
    matches!(
        flag,
        "silent"
            | "show-error"
            | "fail"
            | "head"
            | "globoff"
            | "insecure"
            | "verbose"
            | "include"
            | "disable"
            | "compressed"
            | "path-as-is"
            | "ipv4"
            | "ipv6"
            | "get"
    )
}

fn curl_long_takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "output"
            | "max-time"
            | "connect-timeout"
            | "write-out"
            | "header"
            | "user-agent"
            | "request"
            | "referer"
            | "data"
            | "data-raw"
            | "data-binary"
            | "data-urlencode"
            | "json"
            | "url"
    )
}

fn curl_long_value_ok(flag: &str, value: &str) -> bool {
    match flag {
        "output" => value == crate::compat::shell::null_device(),
        "max-time" | "connect-timeout" => is_timeout_value(value),
        "request" => is_http_method(value),
        "url" => is_loopback_url(value),
        "data-urlencode" => !value.contains('@'),
        "data" | "data-raw" | "data-binary" | "json" => !value.starts_with('@'),
        "write-out" | "header" | "user-agent" | "referer" => !value.starts_with('@'),
        _ => false,
    }
}

fn curl_short_bool(flag: char) -> bool {
    matches!(
        flag,
        's' | 'S' | 'f' | 'I' | 'g' | 'k' | 'v' | 'i' | 'q' | 'G'
    )
}

fn curl_short_takes_value(flag: char) -> bool {
    matches!(flag, 'o' | 'm' | 'w' | 'H' | 'A' | 'X' | 'e' | 'd')
}

fn curl_short_value_ok(flag: char, value: &str) -> bool {
    match flag {
        'o' => value == crate::compat::shell::null_device(),
        'm' => is_timeout_value(value),
        'X' => is_http_method(value),
        'd' => !value.starts_with('@'),
        'w' | 'H' | 'A' | 'e' => !value.starts_with('@'),
        _ => false,
    }
}

fn is_timeout_value(value: &str) -> bool {
    let mut saw_digit = false;
    let mut saw_dot = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            continue;
        }
        return false;
    }
    saw_digit
}

fn is_http_method(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
    )
}

fn inject_curl_guards(command: &str) -> String {
    let (parts, _) = scan_compound(command);
    let mut out = String::new();
    for part in parts {
        match part {
            ShellPart::Segment(text) => out.push_str(&inject_curl_segment(&text)),
            ShellPart::Sep(text) => out.push_str(&text),
        }
    }
    out
}

fn inject_curl_segment(segment: &str) -> String {
    let trimmed_start = segment.len() - segment.trim_start().len();
    let body = &segment[trimmed_start..];
    let Some(end) = curl_word_end(body) else {
        return segment.to_string();
    };
    let mut out = String::new();
    out.push_str(&segment[..trimmed_start]);
    out.push_str(&body[..end]);
    // -q skips curlrc, -g stops bracket globs, and proxy env would leave the machine.
    out.push_str(" -q -g");
    out.push_str(&body[end..]);
    out
}

fn curl_word_end(body: &str) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index == 0 {
        return None;
    }
    let word = &body[..index];
    let base = word.rsplit(['/', '\\']).next().unwrap_or(word);
    if base.eq_ignore_ascii_case("curl") {
        Some(index)
    } else {
        None
    }
}

fn pipes_to_shell(command: &str) -> bool {
    for segment in split_segments(command).into_iter().skip(1) {
        let tokens = tokenize(&segment);
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

const ENV_STDOUT_CAP: u64 = 2 * 1024 * 1024;
const ENV_STDERR_CAP: u64 = 8 * 1024;

struct LoginEnv {
    vars: Vec<(OsString, OsString)>,
    fallback: Option<String>,
}

fn resolved_shell(program: &str) -> String {
    let trimmed = program.trim();
    if trimmed.is_empty() {
        default_shell()
    } else {
        trimmed.to_string()
    }
}

fn shell_flag(shell: &str) -> &'static str {
    crate::compat::shell::shell_flag(shell)
}

fn shell_reads_profile(shell: &str) -> bool {
    crate::compat::shell::shell_reads_profile(shell)
}

fn probe_shell_for(shell: &str) -> String {
    crate::compat::shell::probe_shell_for(shell)
}

fn login_envs() -> &'static Mutex<HashMap<String, Arc<LoginEnv>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<LoginEnv>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_login_envs() -> std::sync::MutexGuard<'static, HashMap<String, Arc<LoginEnv>>> {
    login_envs().lock().unwrap_or_else(|err| err.into_inner())
}

fn login_env(shell: &str) -> Arc<LoginEnv> {
    if let Some(hit) = lock_login_envs().get(shell).cloned() {
        return hit;
    }
    let loaded = Arc::new(load_login_env(shell));
    lock_login_envs()
        .entry(shell.to_string())
        .or_insert_with(|| Arc::clone(&loaded))
        .clone()
}

fn load_login_env(shell: &str) -> LoginEnv {
    if !shell_reads_profile(shell) {
        return LoginEnv {
            vars: process_env(),
            fallback: None,
        };
    }
    match probe_login_env(&probe_shell_for(shell)) {
        Ok(vars) => LoginEnv {
            vars: merge_probe_env(vars),
            fallback: None,
        },
        Err(reason) => LoginEnv {
            vars: process_env(),
            fallback: Some(format!(
                "bash: login shell startup failed: {reason}. Using the desktop process environment."
            )),
        },
    }
}

fn merge_probe_env(probed: Vec<(OsString, OsString)>) -> Vec<(OsString, OsString)> {
    let process = process_env();
    let mut merged = probed;
    merge_path(&mut merged, &process);
    for (key, value) in process {
        if merged.iter().any(|(existing, _)| existing == &key) {
            continue;
        }
        merged.push((key, value));
    }
    merged
}

fn merge_path(merged: &mut Vec<(OsString, OsString)>, process: &[(OsString, OsString)]) {
    let Some(process_path) = last_path_value(process) else {
        return;
    };
    let Some(slot) = last_path_index(merged) else {
        merged.push((OsString::from("PATH"), process_path.clone()));
        return;
    };
    merged[slot].1 = join_missing_path(&merged[slot].1, process_path);
}

fn last_path_index(vars: &[(OsString, OsString)]) -> Option<usize> {
    vars.iter()
        .enumerate()
        .rev()
        .find(|(_, (key, _))| is_path_key(key.as_encoded_bytes()))
        .map(|(index, _)| index)
}

fn last_path_value(vars: &[(OsString, OsString)]) -> Option<&OsString> {
    last_path_index(vars).map(|index| &vars[index].1)
}

fn join_missing_path(primary: &OsString, extra: &OsString) -> OsString {
    let sep = crate::compat::shell::path_separator();
    let mut out = primary.as_encoded_bytes().to_vec();
    for component in extra.as_encoded_bytes().split(|byte| *byte == sep) {
        if component.is_empty() || path_has_component(&out, component, sep) {
            continue;
        }
        if !out.is_empty() && !out.ends_with(&[sep]) {
            out.push(sep);
        }
        out.extend_from_slice(component);
    }
    os_from_bytes(&out)
}

fn path_has_component(path: &[u8], component: &[u8], sep: u8) -> bool {
    path.split(|byte| *byte == sep)
        .any(|item| item == component)
}

fn process_env() -> Vec<(OsString, OsString)> {
    std::env::vars_os().collect()
}

fn probe_login_env(shell: &str) -> Result<Vec<(OsString, OsString)>, String> {
    // Probe is separate so the user command is not executed inside the profile.
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", crate::compat::shell::env_probe_command()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = crate::compat::process::spawn_protected(&mut command)
        .map_err(|error| format!("could not start `{shell}`: {error}"))?;
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_task = std::thread::spawn(move || match stdout_pipe {
        Some(pipe) => read_capped(pipe, ENV_STDOUT_CAP),
        None => Vec::new(),
    });
    let stderr_task = std::thread::spawn(move || match stderr_pipe {
        Some(pipe) => read_capped(pipe, ENV_STDERR_CAP),
        None => Vec::new(),
    });
    let started = std::time::Instant::now();
    let status = loop {
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_task.join();
            let _ = stderr_task.join();
            return Err("timed out after 30s".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = stdout_task.join();
                let _ = stderr_task.join();
                return Err(error.to_string());
            }
        }
    };
    let stdout = stdout_task.join().unwrap_or_default();
    let stderr = stderr_task.join().unwrap_or_default();
    match parse_env_block(&stdout) {
        Ok(vars) => Ok(vars),
        Err(reason) if status.success() => Err(reason),
        Err(_) => Err(format!(
            "probe exited {}{}",
            status.code().unwrap_or(-1),
            stderr_suffix(&stderr)
        )),
    }
}

fn read_capped(pipe: impl Read, cap: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut limited = pipe.take(cap);
    let _ = limited.read_to_end(&mut buf);
    buf
}

fn parse_env_block(bytes: &[u8]) -> Result<Vec<(OsString, OsString)>, String> {
    crate::compat::shell::parse_env_dump(bytes)
}

fn is_path_key(key: &[u8]) -> bool {
    crate::compat::shell::is_path_key(key)
}

fn os_from_bytes(bytes: &[u8]) -> OsString {
    crate::compat::shell::os_from_bytes(bytes)
}

fn stderr_suffix(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed = collapsed.trim();
    if collapsed.is_empty() {
        return String::new();
    }
    let clipped: String = collapsed.chars().take(180).collect();
    format!(": {clipped}")
}

fn apply_login_env(command: &mut Command, env: &LoginEnv) {
    command.env_clear();
    for (key, value) in &env.vars {
        if key.as_encoded_bytes() == b"BASH_ENV" {
            continue;
        }
        command.env(key, value);
    }
}

pub(crate) fn install_command_env(command: &mut Command, shell: &str, clear_proxy: bool) {
    let login = login_env(shell);
    apply_login_env(command, &login);
    if clear_proxy {
        clear_proxy_env(command);
    }
}

fn clear_proxy_env(command: &mut Command) {
    for key in [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "ftp_proxy",
        "socks_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "FTP_PROXY",
        "SOCKS_PROXY",
    ] {
        command.env_remove(key);
    }
    command.env("NO_PROXY", "*");
    command.env("no_proxy", "*");
}

fn with_login_note(message: &str, note: Option<&str>) -> String {
    match note {
        Some(note) => format!("{note} {message}"),
        None => message.to_string(),
    }
}

fn run_command(ctx: &ToolContext<'_>, command: &str) -> ToolOutcome {
    let Some(workspace) = ctx.workspace_path() else {
        return error_outcome("bash needs a workspace.");
    };
    let prep = prepare_auto_run(command);
    let shell = resolved_shell(&ctx.shell_program);
    let flag = shell_flag(&shell);
    let login = login_env(&shell);
    let mut process = Command::new(&shell);
    process
        .arg(flag)
        .arg(&prep.command)
        .current_dir(&workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    apply_login_env(&mut process, &login);
    if prep.clear_proxy {
        clear_proxy_env(&mut process);
    }
    let mut child = match crate::compat::process::spawn_protected(&mut process) {
        Ok(child) => child,
        Err(error) => {
            return error_outcome(&with_login_note(
                &format!("bash could not start `{shell}`: {error}"),
                login.fallback.as_deref(),
            ));
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
            return error_outcome(&with_login_note(
                "bash timed out after 30s.",
                login.fallback.as_deref(),
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => {
                let _ = child.kill();
                let _ = stdout_task.join();
                let _ = stderr_task.join();
                return error_outcome(&with_login_note(
                    &format!("bash: {error}"),
                    login.fallback.as_deref(),
                ));
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
    let note = login.fallback.as_deref().filter(|_| !status.success());
    let text = match note {
        Some(note) => toon_doc(&[
            ("status", ToonValue::Str(label)),
            ("exitCode", ToonValue::Int(code as i64)),
            ("output", ToonValue::Block(&output)),
            ("error", ToonValue::Block(note)),
        ]),
        None => toon_doc(&[
            ("status", ToonValue::Str(label)),
            ("exitCode", ToonValue::Int(code as i64)),
            ("output", ToonValue::Block(&output)),
        ]),
    };
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
    crate::compat::shell::default_shell().program
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
    fn curl_leaving_the_machine_needs_confirm() {
        assert_eq!(
            classify("curl -L http://localhost:8080/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify(
                "curl --resolve localhost:80:example.com http://localhost/",
                &[],
                &[]
            ),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl ftp://example.com/file", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("wget http://localhost:3000/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify(
                "curl --resolve=127.0.0.1:80:1.2.3.4 http://127.0.0.1/",
                &[],
                &[]
            ),
            Decision::Confirm
        );
        assert_eq!(
            classify(
                "curl --connect-to=127.0.0.1:80:evil.example:80 http://127.0.0.1/",
                &[],
                &[]
            ),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl -x evil.example:8080 http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl --proxy evil.example:8080 http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl -K /tmp/cfg http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl --output=/tmp/x http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl -o/tmp/x http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl -d @/etc/passwd http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl -T /etc/passwd http://127.0.0.1/", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify(
                "http_proxy=http://evil:8080 curl http://127.0.0.1/",
                &[],
                &[]
            ),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl http://127.0.0.1/ && echo hi", &[], &[]),
            Decision::Confirm
        );
        assert_eq!(
            classify("curl --output=/dev/null http://127.0.0.1/", &[], &[]),
            Decision::Run
        );
        assert_eq!(
            classify("curl -sS --max-time 2 http://127.0.0.1:9/", &[], &[]),
            Decision::Run
        );
        assert_eq!(
            classify("curl -d '{\"a\":1}' http://127.0.0.1/api", &[], &[]),
            Decision::Run
        );
        let prep = prepare_auto_run("curl -s http://127.0.0.1/");
        assert_eq!(prep.command, "curl -q -g -s http://127.0.0.1/");
        assert!(prep.clear_proxy);
        let plain = prepare_auto_run("echo hi");
        assert_eq!(plain.command, "echo hi");
        assert!(!plain.clear_proxy);
    }

    #[test]
    fn git_status_runs() {
        assert_eq!(classify("git status", &[], &[]), Decision::Run);
        assert_eq!(classify("git commit -m x", &[], &[]), Decision::Confirm);
    }

    #[test]
    fn description_is_required_on_every_call() {
        let ctx = ToolContext::for_test(std::path::PathBuf::from("."), 1);
        let missing = BashTool.execute(&json!({ "command": "rm -rf foo" }), &ctx);
        assert!(
            missing.text.contains("bash: `description` is required."),
            "{}",
            missing.text
        );
        let blank = BashTool.execute(
            &json!({ "command": "git status", "description": " \n\t " }),
            &ctx,
        );
        assert!(
            blank.text.contains("bash: `description` is empty."),
            "{}",
            blank.text
        );
        let described = BashTool.execute(
            &json!({ "command": "rm -rf foo", "description": "git status && echo ok" }),
            &ctx,
        );
        assert!(
            described
                .text
                .contains("dangerous command must run on the async dispatch path"),
            "{}",
            described.text
        );
    }

    #[test]
    fn compound_segments_are_classified() {
        let cases: &[(&str, &[&str], &[&str], Decision)] = &[
            ("git status && git diff", &[], &[], Decision::Run),
            ("git status && rm -rf foo", &[], &[], Decision::Confirm),
            ("bash -c \"rm -rf foo\"", &[], &[], Decision::Confirm),
            ("bash -c\"rm -rf foo\"", &[], &[], Decision::Confirm),
            ("sh -c \"rm -rf foo\"", &[], &[], Decision::Confirm),
            ("zsh -lc \"rm -rf foo\"", &[], &[], Decision::Confirm),
            ("bash -lc \"rm -rf foo\"", &[], &[], Decision::Confirm),
            ("echo \"rm -rf foo\"", &[], &[], Decision::Run),
            ("echo \"a; rm -rf foo\"", &[], &[], Decision::Run),
            ("echo 'a; rm -rf foo'", &[], &[], Decision::Run),
            ("cmd >/dev/null", &[], &[], Decision::Run),
            ("cmd > out.txt", &[], &[], Decision::Confirm),
            ("echo $(rm -rf foo)", &[], &[], Decision::Confirm),
            ("echo \"$(rm -rf foo)\"", &[], &[], Decision::Confirm),
            ("echo '$(rm -rf foo)'", &[], &[], Decision::Run),
            ("echo `rm -rf foo`", &[], &[], Decision::Confirm),
            ("git status || git diff", &[], &[], Decision::Run),
            ("git status || rm -rf foo", &[], &[], Decision::Confirm),
            (
                "git status && git diff",
                &["git status", "git diff"],
                &[],
                Decision::Allowed,
            ),
            (
                "git status && git diff",
                &["git status && git diff"],
                &[],
                Decision::Allowed,
            ),
            (
                "git status && git diff",
                &["git status"],
                &[],
                Decision::Run,
            ),
            (
                "git status && rm -rf foo",
                &["git status"],
                &["rm -rf foo"],
                Decision::Blocked,
            ),
            (
                "bash -c \"rm -rf foo\"",
                &[],
                &["rm -rf foo"],
                Decision::Blocked,
            ),
        ];
        for (command, allow, block, decision) in cases {
            let allowed: Vec<String> = allow.iter().map(|item| (*item).to_string()).collect();
            let blocked: Vec<String> = block.iter().map(|item| (*item).to_string()).collect();
            assert_eq!(
                classify(command, &allowed, &blocked),
                *decision,
                "{command}"
            );
        }
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

    #[cfg(unix)]
    #[test]
    fn login_profile_path_finds_stub_and_missing_command_fails() {
        let root = temp_workspace("profile");
        let bin_dir = root.join("bin");
        let home = root.join("home");
        let workspace = root.join("ws");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        write_exe(
            &bin_dir.join("only-in-profile"),
            "#!/bin/sh\nprintf 'profile-ok\\n'\n",
        );
        let bin = bin_dir.display().to_string();
        let profile = format!("export PATH='{bin}':\"$PATH\"\n");
        std::fs::write(home.join(".bash_profile"), &profile).unwrap();
        std::fs::write(home.join(".bashrc"), &profile).unwrap();
        std::fs::write(home.join(".profile"), &profile).unwrap();
        let shell = root.join("shell");
        write_exe(
            &shell,
            &format!(
                "#!/bin/sh\nexport HOME='{}'\nexport ZDOTDIR=\"$HOME\"\nexec /bin/bash \"$@\"\n",
                home.display()
            ),
        );
        let mut ctx = ToolContext::for_test(workspace, 1);
        ctx.shell_program = shell.display().to_string();

        let found = run_command(&ctx, "only-in-profile");
        assert_eq!(
            found.display.status.as_deref(),
            Some("ok"),
            "{}",
            found.text
        );
        assert!(found.text.contains("profile-ok"), "{}", found.text);
        assert!(
            !found.text.contains("login shell startup failed"),
            "{}",
            found.text
        );

        let missing = run_command(&ctx, "k-agent-missing-bin-xyz");
        assert_eq!(
            missing.display.status.as_deref(),
            Some("error"),
            "{}",
            missing.text
        );
        assert!(missing.text.contains("exitCode: 127"), "{}", missing.text);
    }

    #[cfg(unix)]
    #[test]
    fn login_profile_failure_reports_fallback() {
        let root = temp_workspace("fallback");
        let workspace = root.join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let shell = root.join("shell");
        write_exe(
            &shell,
            "#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    *__KAGENT_ENV__*) exit 3 ;;\n  esac\ndone\ncmd=\nprev=\nfor arg in \"$@\"; do\n  if [ \"$prev\" = \"-c\" ]; then\n    cmd=$arg\n  fi\n  prev=$arg\ndone\nexec /bin/sh -c \"$cmd\"\n",
        );
        let mut ctx = ToolContext::for_test(workspace, 1);
        ctx.shell_program = shell.display().to_string();
        let outcome = run_command(&ctx, "printf ok");
        assert_eq!(
            outcome.display.status.as_deref(),
            Some("ok"),
            "{}",
            outcome.text
        );
        assert!(outcome.text.contains("exitCode: 0"), "{}", outcome.text);
        assert!(
            !outcome.text.contains("login shell startup failed"),
            "{}",
            outcome.text
        );

        let missing = run_command(&ctx, "k-agent-missing-bin-xyz");
        assert_eq!(
            missing.display.status.as_deref(),
            Some("error"),
            "{}",
            missing.text
        );
        assert!(
            missing.text.contains("bash: login shell startup failed:"),
            "{}",
            missing.text
        );
        assert!(
            missing
                .text
                .contains("Using the desktop process environment."),
            "{}",
            missing.text
        );
    }

    #[cfg(unix)]
    fn temp_workspace(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "k-agent-bash-{label}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    fn write_exe(path: &std::path::Path, body: &str) {
        std::fs::write(path, body).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}
