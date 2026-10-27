use std::ffi::OsString;
use std::path::Path;
#[cfg(windows)]
use std::sync::OnceLock;

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

/// `sh` has no profile of its own, so a real shell is used for the probe.
pub fn probe_shell_for(shell: &str) -> String {
    if !matches!(shell_stem(shell).as_str(), "sh" | "dash") {
        return shell.to_string();
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
        // GNU env can separate records with NUL, so values keep their newlines.
        "command printf '\\0__KAGENT_ENV__\\0'; command env -0"
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
        let mut dump = marker.to_vec();
        dump.extend_from_slice(b"PATH=/bin\0HOME=/root\0");
        let vars = parse_env_dump(&dump).expect("dump parses");
        assert_eq!(vars.len(), 2);

        let mut no_path = marker.to_vec();
        no_path.extend_from_slice(b"HOME=/root\0");
        assert!(parse_env_dump(&no_path).is_err());
    }
}
