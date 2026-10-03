use tauri::AppHandle;

/// Facts the model otherwise discovers with `bash` and `list_directory`.
pub(crate) fn body(app: &AppHandle) -> String {
    let workspace = crate::pathutil::workspace_from_app(app);
    let config = crate::paths::config_dir(app).ok();
    let home = crate::paths::home_dir(app).ok();
    let mut lines = Vec::new();
    match workspace.as_ref() {
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
    if let Some(path) = config.as_ref() {
        lines.push(format!(
            "App config is `{}` (skills, agents, providers). That directory is not the workspace.",
            path.display()
        ));
    }
    lines.push(format!("Host: {}", os_pretty()));
    if let Some(path) = home.as_ref() {
        lines.push(format!("Home: {}", path.display()));
    }
    lines.push(format!("Shell: {}", shell_label(app)));
    lines.push(
        "Do not use `bash` or `list_directory` to discover the workspace, the home directory, or the OS."
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
