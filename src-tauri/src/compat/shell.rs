use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Shell program plus the arguments that make it run one command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSpec {
    pub program: String,
    pub prefix_args: Vec<String>,
}

/// PowerShell is the Windows default: cmd.exe has no quoting rules an agent can rely on.
pub fn default_shell() -> ShellSpec {
    if cfg!(windows) {
        ShellSpec {
            program: "powershell.exe".to_string(),
            prefix_args: vec!["-NoProfile".to_string(), "-Command".to_string()],
        }
    } else {
        ShellSpec {
            program: "/bin/sh".to_string(),
            prefix_args: vec!["-c".to_string()],
        }
    }
}

/// Flag that makes the shell run one command string.
pub fn shell_flag(shell: &str) -> &'static str {
    match shell_stem(shell).as_str() {
        "cmd" => "/c",
        "powershell" | "pwsh" => "-Command",
        _ => "-c",
    }
}

pub fn shell_stem(shell: &str) -> String {
    let base = shell.rsplit(['/', '\\']).next().unwrap_or(shell);
    let stem = base.strip_suffix(".exe").unwrap_or(base);
    stem.to_ascii_lowercase()
}

/// cmd and PowerShell have no login profile to read.
pub fn shell_reads_profile(shell: &str) -> bool {
    !matches!(shell_stem(shell).as_str(), "cmd" | "powershell" | "pwsh")
}

/// `sh` has no profile of its own. Probe the user shell so PATH includes node and the rest.
pub fn probe_shell_for(shell: &str) -> String {
    let user = std::env::var_os("SHELL").map(|value| value.to_string_lossy().into_owned());
    pick_probe_shell(shell, user.as_deref())
}

fn pick_probe_shell(shell: &str, user_shell: Option<&str>) -> String {
    if !matches!(shell_stem(shell).as_str(), "sh" | "dash") {
        return shell.to_string();
    }
    if let Some(program) = user_shell.map(str::trim).filter(|value| !value.is_empty()) {
        let stem = shell_stem(program);
        if matches!(stem.as_str(), "bash" | "zsh" | "fish") && Path::new(program).is_file() {
            return program.to_string();
        }
    }
    for candidate in login_shell_candidates() {
        if Path::new(candidate).is_file() {
            return candidate.to_string();
        }
    }
    shell.to_string()
}

pub fn login_shell_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["C:\\Program Files\\Git\\bin\\bash.exe"]
    } else {
        &["/bin/bash", "/usr/bin/bash", "/bin/zsh"]
    }
}

/// Command that prints the login shell environment behind a marker.
pub fn env_probe_command() -> &'static str {
    if cfg!(target_os = "linux") {
        // A NUL inside one printf format ends that format. The marker is three writes.
        "command printf '\\0'; command printf '%s' '__KAGENT_ENV__'; command printf '\\0'; command env -0"
    } else {
        // BSD env has no -0 flag. Records are newline separated.
        "command printf '__KAGENT_ENV__\\n'; command env"
    }
}

pub fn env_probe_marker() -> &'static [u8] {
    if cfg!(target_os = "linux") {
        b"\0__KAGENT_ENV__\0"
    } else {
        b"__KAGENT_ENV__\n"
    }
}

pub fn env_probe_separator() -> u8 {
    if cfg!(target_os = "linux") {
        0
    } else {
        b'\n'
    }
}

/// Parse the environment dump that the probe command prints.
pub fn parse_env_dump(bytes: &[u8]) -> Result<Vec<(OsString, OsString)>, String> {
    let marker = env_probe_marker();
    let Some(start) = find_bytes(bytes, marker) else {
        return Err("login shell returned no environment".into());
    };
    let rest = &bytes[start + marker.len()..];
    let separator = env_probe_separator();
    let mut vars = Vec::new();
    let mut saw_path = false;
    for record in rest.split(|byte| *byte == separator) {
        if record.is_empty() {
            continue;
        }
        let Some(eq) = record.iter().position(|byte| *byte == b'=') else {
            continue;
        };
        if eq == 0 {
            continue;
        }
        let key = &record[..eq];
        let value = &record[eq + 1..];
        if is_path_key(key) && !value.is_empty() {
            saw_path = true;
        }
        vars.push((os_from_bytes(key), os_from_bytes(value)));
    }
    if !saw_path {
        return Err("login shell returned no PATH".into());
    }
    Ok(vars)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub fn is_path_key(key: &[u8]) -> bool {
    key.eq_ignore_ascii_case(b"PATH")
}

pub fn os_from_bytes(bytes: &[u8]) -> OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(bytes.to_vec())
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(bytes).into_owned().into()
    }
}

/// Separator of the PATH variable on this host.
pub fn path_separator() -> u8 {
    if cfg!(windows) {
        b';'
    } else {
        b':'
    }
}

/// Where a command sends its discarded output.
pub fn null_device() -> &'static str {
    if cfg!(windows) {
        "NUL"
    } else {
        "/dev/null"
    }
}

/// Commands that destroy data or take the machine down.
const UNIX_DANGEROUS: &[&str] = &[
    "rm", "rmdir", "mv", "cp", "chmod", "chown", "chgrp", "sudo", "su", "doas", "dd", "mkfs",
    "shutdown", "reboot", "poweroff", "halt", "kill", "killall", "pkill", "curl", "wget", "nc",
    "ncat", "ssh", "scp", "rsync", "docker", "podman", "kubectl",
];

/// The names that only exist on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
const WINDOWS_DANGEROUS: &[&str] = &[
    "del",
    "erase",
    "rd",
    "format",
    "diskpart",
    "taskkill",
    "reg",
    "cipher",
    "takeown",
    "icacls",
    "attrib",
    "bcdedit",
    "vssadmin",
    "wmic",
    "manage-bde",
];

/// The commands that need a confirmation on this host.
pub fn dangerous_commands() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        static MERGED: OnceLock<Vec<&'static str>> = OnceLock::new();
        MERGED.get_or_init(|| {
            UNIX_DANGEROUS
                .iter()
                .chain(WINDOWS_DANGEROUS.iter())
                .copied()
                .collect()
        })
    }
    #[cfg(not(windows))]
    {
        UNIX_DANGEROUS
    }
}

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const ENV_STDOUT_CAP: u64 = 2 * 1024 * 1024;
const ENV_STDERR_CAP: u64 = 8 * 1024;

pub struct LoginEnv {
    pub vars: Vec<(OsString, OsString)>,
    pub fallback: Option<String>,
}

fn login_envs() -> &'static Mutex<HashMap<String, Arc<LoginEnv>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<LoginEnv>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_login_envs() -> std::sync::MutexGuard<'static, HashMap<String, Arc<LoginEnv>>> {
    login_envs().lock().unwrap_or_else(|err| err.into_inner())
}

pub fn login_env(shell: &str) -> Arc<LoginEnv> {
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
    let sep = path_separator();
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
    let mut command = Command::new(shell);
    command
        .args(["-l", "-i", "-c", env_probe_command()])
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
        if started.elapsed() > PROBE_TIMEOUT {
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
    match parse_env_dump(&stdout) {
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

pub fn apply_login_env(command: &mut Command, env: &LoginEnv) {
    command.env_clear();
    for (key, value) in &env.vars {
        if key.as_encoded_bytes() == b"BASH_ENV" {
            continue;
        }
        command.env(key, value);
    }
}

/// PATH the user shell would use. The desktop process PATH misses profile installs.
pub fn command_search_path() -> OsString {
    let env = login_env(&default_shell().program);
    last_path_value(&env.vars)
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| std::env::var_os("PATH"))
        .unwrap_or_default()
}

pub fn is_dangerous_command(name: &str) -> bool {
    let base = base_command_name(name);
    dangerous_commands()
        .iter()
        .any(|item| base.eq_ignore_ascii_case(item))
}

/// Command name without the directory. Windows also drops the extension of a
/// program, because `del.exe` is the `del` of the list of dangerous commands.
pub fn base_command_name(token: &str) -> String {
    let base = token.rsplit(['/', '\\']).next().unwrap_or(token);
    #[cfg(windows)]
    {
        let stem = Path::new(base)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(base);
        stem.to_string()
    }
    #[cfg(not(windows))]
    {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn shell_flags_match_the_shell_kind() {
        assert_eq!(shell_flag("/bin/zsh"), "-c");
        assert_eq!(shell_flag("/bin/sh"), "-c");
        assert_eq!(shell_flag("cmd.exe"), "/c");
        assert_eq!(shell_flag("C:\\Windows\\System32\\cmd.exe"), "/c");
        assert_eq!(shell_flag("powershell.exe"), "-Command");
        assert_eq!(shell_flag("pwsh"), "-Command");
    }

    #[test]
    fn default_shell_runs_one_command_string() {
        let spec = default_shell();
        assert!(!spec.program.is_empty());
        assert!(!spec.prefix_args.is_empty());
    }

    #[test]
    fn dangerous_commands_follow_the_host() {
        assert!(is_dangerous_command("rm"));
        assert!(!is_dangerous_command("cargo"));
        if cfg!(windows) {
            assert!(is_dangerous_command("del"));
            assert!(is_dangerous_command("C:\\Windows\\System32\\format.com"));
        } else {
            // The names of Windows are not commands of this host.
            assert!(!is_dangerous_command("del"));
            assert!(!is_dangerous_command("format"));
        }
    }

    #[test]
    fn base_command_name_drops_the_directory() {
        assert_eq!(base_command_name("/usr/bin/ls"), "ls");
        assert_eq!(base_command_name("ls"), "ls");
        if cfg!(windows) {
            assert_eq!(
                base_command_name("C:\\Windows\\System32\\where.exe"),
                "where"
            );
        } else {
            // The name of a program of this host keeps its extension.
            assert_eq!(base_command_name("/usr/bin/ls.sh"), "ls.sh");
        }
    }

    #[test]
    fn env_dump_needs_a_path() {
        let marker = env_probe_marker();
        let sep = [env_probe_separator()];
        let mut dump = marker.to_vec();
        dump.extend_from_slice(b"PATH=/bin");
        dump.extend_from_slice(&sep);
        dump.extend_from_slice(b"HOME=/root");
        dump.extend_from_slice(&sep);
        let vars = parse_env_dump(&dump).expect("dump parses");
        assert_eq!(vars.len(), 2);

        let mut no_path = marker.to_vec();
        no_path.extend_from_slice(b"HOME=/root");
        no_path.extend_from_slice(&sep);
        assert!(parse_env_dump(&no_path).is_err());
    }

    #[test]
    fn sh_probe_prefers_the_user_shell() {
        if Path::new("/bin/bash").is_file() {
            assert_eq!(pick_probe_shell("/bin/sh", Some("/bin/bash")), "/bin/bash");
            assert_eq!(
                pick_probe_shell("/bin/sh", Some("/no/such/shell")),
                "/bin/bash"
            );
        }
        assert_eq!(pick_probe_shell("/bin/zsh", Some("/bin/bash")), "/bin/zsh");
    }

    #[test]
    fn probe_command_round_trips_path() {
        let shell = if Path::new("/bin/bash").is_file() {
            "/bin/bash"
        } else if Path::new("/bin/sh").is_file() {
            "/bin/sh"
        } else {
            return;
        };
        let output = std::process::Command::new(shell)
            .args(["-c", env_probe_command()])
            .output()
            .expect("probe");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let vars = parse_env_dump(&output.stdout).expect("parse");
        assert!(vars
            .iter()
            .any(|(key, value)| { is_path_key(key.as_encoded_bytes()) && !value.is_empty() }));
    }
}
