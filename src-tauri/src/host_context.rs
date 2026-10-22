use std::path::Path;

use tauri::AppHandle;

/// Facts the model otherwise discovers with `bash` and `glob`.
pub(crate) fn body(app: &AppHandle) -> String {
    let workspace = crate::pathutil::workspace_from_app(app);
    let home = crate::paths::home_dir(app).ok();
    render(workspace.as_deref(), home.as_deref(), &shell_label(app))
}

fn render(workspace: Option<&Path>, home: Option<&Path>, shell: &str) -> String {
    let mut lines = Vec::new();
    match workspace {
        Some(path) => {
            lines.push(format!("Workspace: {}", path.display()));
            lines.push(
                "`bash` starts in this directory. Relative tool paths and `.` resolve here.".into(),
            );
        }
        None => {
            lines.push("Workspace: not set".into());
            lines.push(
                "Set a workspace before relative paths. `bash` cannot run without one.".into(),
            );
        }
    }
    lines.push(format!("Host: {}", os_pretty()));
    if let Some(path) = home {
        lines.push(format!("Home: {}", path.display()));
    }
    lines.push(format!("Shell: {shell}"));
    lines.push(
        "Do not use `bash` or `glob` to discover the workspace, the home directory, or the OS."
            .into(),
    );
    lines.push("Do not scan `/` or the home directory to find the project.".into());
    lines.join("\n")
}

fn shell_label(app: &AppHandle) -> String {
    let configured = crate::load_ui_settings(app)
        .as_ref()
        .and_then(|settings| settings.get("shellProgram"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if let Some(shell) = configured {
        return shell;
    }
    if cfg!(windows) {
        "cmd.exe".into()
    } else {
        "/bin/sh".into()
    }
}

fn os_pretty() -> String {
    let fallback = format!("{} {}", std::env::consts::OS, std::env::consts::ARCH);
    #[cfg(target_os = "linux")]
    {
        if let Some(name) = linux_pretty_name() {
            return format!(
                "{name} ({}/{})",
                std::env::consts::OS,
                std::env::consts::ARCH
            );
        }
    }
    fallback
}

#[cfg(target_os = "linux")]
fn linux_pretty_name() -> Option<String> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("PRETTY_NAME=") else {
            continue;
        };
        let name = rest.trim().trim_matches('"').trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

#[tauri::command]
pub(crate) fn host_context(app: AppHandle) -> String {
    body(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omits_app_config_location() {
        let text = render(
            Some(Path::new("/home/ada/src/k-agent")),
            Some(Path::new("/home/ada")),
            "/bin/zsh",
        );
        assert!(!text.contains("App config"));
        assert!(!text.contains(".k-agent"));
        assert!(text.contains("Workspace: /home/ada/src/k-agent"));
        assert!(text.contains("Host: "));
        assert!(text.contains("Home: /home/ada"));
        assert!(text.contains("Shell: /bin/zsh"));
    }

    #[test]
    fn workspace_missing_keeps_the_rest() {
        let text = render(None, Some(Path::new("/home/ada")), "/bin/zsh");
        assert!(text.contains("Workspace: not set"));
        assert!(text.contains("Home: /home/ada"));
        assert!(text.contains("Shell: /bin/zsh"));
        assert!(!text.contains("App config"));
    }
}
