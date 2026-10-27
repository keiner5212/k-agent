use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Program and arguments unchanged. Batch files use `command` instead.
pub fn resolve(program: &str, args: &[String]) -> (OsString, Vec<OsString>) {
    (
        OsString::from(program),
        args.iter().map(OsString::from).collect(),
    )
}

/// A command the host can start. Batch files go through `cmd /C`.
pub fn command(program: &str, args: &[String]) -> std::process::Command {
    #[cfg(windows)]
    {
        return windows_command(program, args);
    }
    #[cfg(not(windows))]
    {
        let (program, args) = resolve(program, args);
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        cmd
    }
}

#[cfg(windows)]
fn windows_command(program: &str, args: &[String]) -> std::process::Command {
    use std::os::windows::process::CommandExt;

    let Some(full) = find_in_path(program) else {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        return cmd;
    };
    if !is_script(&full) {
        let mut cmd = std::process::Command::new(full);
        cmd.args(args);
        return cmd;
    }
    let processor = std::env::var_os("ComSpec").unwrap_or_else(|| OsString::from("cmd.exe"));
    let mut cmd = std::process::Command::new(processor);
    // /S strips the first and last quote. The trailing space keeps that last
    // quote from being one that belongs to an argument.
    cmd.arg("/D")
        .arg("/S")
        .arg("/C")
        .raw_arg(cmd_c_argument(&full.to_string_lossy(), args));
    cmd
}

/// `cmd /S /C` argument for a batch file. The outer quotes are sacrificial.
pub fn cmd_c_argument(script: &str, args: &[String]) -> String {
    let mut line = String::from("call ");
    line.push_str(&quote_cmd_argument(script));
    for arg in args {
        line.push(' ');
        line.push_str(&quote_cmd_argument(arg));
    }
    format!("\"{line} \"")
}

fn quote_cmd_argument(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    let special = value.chars().any(|ch| {
        matches!(
            ch,
            ' ' | '\t' | '"' | '&' | '|' | '<' | '>' | '^' | '%' | '(' | ')'
        )
    });
    if !special {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Find a program on this host.
pub fn find_in_path(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 {
        return Some(candidate.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        let direct = dir.join(program);
        if direct.is_file() {
            return Some(direct);
        }
        for extension in crate::compat::paths::executable_extensions() {
            let with_extension = dir.join(format!("{program}{extension}"));
            if with_extension.is_file() {
                return Some(with_extension);
            }
        }
    }
    None
}

/// Whether the file is a script that needs the command processor.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_script(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        extension == "cmd" || extension == "bat"
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_c_argument_keeps_a_path_with_spaces() {
        let line = cmd_c_argument(
            r"C:\Program Files\tool.cmd",
            &["one".to_string(), "two words".to_string()],
        );
        assert_eq!(
            line,
            "\"call \"C:\\Program Files\\tool.cmd\" one \"two words\" \""
        );
    }

    #[test]
    fn resolve_keeps_the_program_on_unix() {
        let (program, args) = resolve("ls", &["-la".to_string()]);
        assert_eq!(program, OsString::from("ls"));
        assert_eq!(args, vec![OsString::from("-la")]);
    }

    #[test]
    fn find_in_path_locates_the_shell() {
        let found = find_in_path("sh");
        assert!(found.is_some(), "sh must be on PATH");
    }
}
