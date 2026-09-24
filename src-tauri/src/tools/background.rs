use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::tool_utils::workspace::confirm_action;
use super::{
    bash, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_ACTION,
};

pub const NAME: &str = "background";

const DESCRIPTION: &str = "Start one command in the workspace and keep it running until this turn ends, then kill it. Use this for a dev server or preview. Do not use bash for that. Do not append &, nohup, or disown. The command is the process itself, for example `npm run dev`. Returns the pid and output from the first 1200ms.";

const STARTUP_WAIT: Duration = Duration::from_millis(1200);
const OUTPUT_CAP: usize = 8_000;

pub struct BackgroundTool;

pub struct TurnSlot {
    procs: Mutex<Vec<RunningProc>>,
}

struct RunningProc {
    child: Child,
    output: Arc<Mutex<String>>,
    readers: Vec<JoinHandle<()>>,
}

enum Launch {
    Finished { code: i32, output: String },
    Running(RunningProc),
}

impl TurnSlot {
    pub fn start() -> Arc<Self> {
        Arc::new(Self {
            procs: Mutex::new(Vec::new()),
        })
    }

    fn track(&self, proc: RunningProc) {
        match self.procs.lock() {
            Ok(mut procs) => procs.push(proc),
            Err(poisoned) => poisoned.into_inner().push(proc),
        }
    }
}

impl Drop for TurnSlot {
    fn drop(&mut self) {
        let procs = match self.procs.lock() {
            Ok(mut procs) => std::mem::take(&mut *procs),
            Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
        };
        for proc in procs {
            drop(proc);
        }
    }
}

impl Drop for RunningProc {
    fn drop(&mut self) {
        kill_group(&mut self.child);
        let _ = self.child.wait();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

impl Tool for BackgroundTool {
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
                        "description": "One command to keep running until the turn ends. No &, nohup, or disown."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        if args
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            return error_outcome("background requires a string `command`.");
        }
        error_outcome("background must run on the async dispatch path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let Some(command) = args.get("command").and_then(Value::as_str) else {
        return error_outcome("background requires a string `command`.");
    };
    let command = command.trim();
    if let Err(outcome) = authorize(command, ctx).await {
        return outcome;
    }
    let Some(workspace) = ctx.workspace_path() else {
        return error_outcome("background needs a workspace.");
    };
    let (shell, flag) = shell_invocation(ctx);
    let launch = match launch(&shell, flag, command, &workspace, STARTUP_WAIT) {
        Ok(launch) => launch,
        Err(message) => return error_outcome(&message),
    };
    match launch {
        Launch::Finished { code, output } => finished_outcome(code, &output),
        Launch::Running(proc) => {
            let Some(turn) = ctx.turn.clone() else {
                let pid = proc.child.id();
                drop(proc);
                return error_outcome(&format!(
                    "background requires an active chat turn. Stopped pid {pid}."
                ));
            };
            let pid = proc.child.id();
            let output = snapshot(&proc.output);
            turn.track(proc);
            running_outcome(pid, &output)
        }
    }
}

async fn authorize(command: &str, ctx: &ToolContext<'_>) -> Result<(), ToolOutcome> {
    match bash::classify(command, &ctx.allowed_commands, &ctx.blocked_commands) {
        bash::Decision::Empty => Err(error_outcome("background `command` is empty.")),
        bash::Decision::Blocked => Err(error_outcome(
            "background refused: command is on the block list.",
        )),
        bash::Decision::Background => Err(error_outcome(
            "background keeps the process until the turn ends. Do not use &, nohup, or disown.",
        )),
        bash::Decision::Run | bash::Decision::Allowed => Ok(()),
        bash::Decision::Confirm => {
            if bash::shell_session_granted(ctx) {
                return Ok(());
            }
            match confirm_action(
                ctx,
                "background_confirm",
                "Background",
                &format!("Keep this process until the turn ends?\n{command}"),
            )
            .await
            {
                Ok(super::tool_utils::workspace::ConfirmChoice::Deny) => {
                    Err(error_outcome("User denied the command."))
                }
                Ok(super::tool_utils::workspace::ConfirmChoice::Once) => Ok(()),
                Ok(super::tool_utils::workspace::ConfirmChoice::Session) => {
                    grant_shell_session_from(ctx);
                    Ok(())
                }
                Err(message) => Err(error_outcome(&message)),
            }
        }
    }
}

fn shell_invocation(ctx: &ToolContext<'_>) -> (String, &'static str) {
    let shell = if ctx.shell_program.trim().is_empty() {
        if cfg!(windows) {
            "cmd.exe".to_string()
        } else {
            "/bin/sh".to_string()
        }
    } else {
        ctx.shell_program.trim().to_string()
    };
    let flag =
        if shell.to_ascii_lowercase().ends_with("cmd.exe") || shell.eq_ignore_ascii_case("cmd") {
            "/c"
        } else {
            "-c"
        };
    (shell, flag)
}

fn launch(
    shell: &str,
    flag: &str,
    command: &str,
    workspace: &std::path::Path,
    startup: Duration,
) -> Result<Launch, String> {
    let mut cmd = Command::new(shell);
    cmd.arg(flag)
        .arg(command)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .map_err(|error| format!("background could not start `{shell}`: {error}"))?;
    let output = Arc::new(Mutex::new(String::new()));
    let mut readers = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        readers.push(drain(pipe, output.clone()));
    }
    if let Some(pipe) = child.stderr.take() {
        readers.push(drain(pipe, output.clone()));
    }
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                for reader in readers {
                    let _ = reader.join();
                }
                return Ok(Launch::Finished {
                    code: status.code().unwrap_or(-1),
                    output: snapshot(&output),
                });
            }
            Ok(None) => {}
            Err(error) => return Err(format!("background: {error}")),
        }
        if started.elapsed() >= startup {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(Launch::Running(RunningProc {
        child,
        output,
        readers,
    }))
}

fn drain(mut pipe: impl Read + Send + 'static, output: Arc<Mutex<String>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]);
                    append_capped(&output, &text);
                }
            }
        }
    })
}

fn append_capped(output: &Mutex<String>, text: &str) {
    let mut guard = match output.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.len() >= OUTPUT_CAP {
        return;
    }
    let room = OUTPUT_CAP - guard.len();
    let take = text.chars().take(room).collect::<String>();
    guard.push_str(&take);
}

fn snapshot(output: &Mutex<String>) -> String {
    match output.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn running_outcome(pid: u32, output: &str) -> ToolOutcome {
    let text = toon_doc(&[
        ("status", ToonValue::Str("running")),
        ("pid", ToonValue::Int(pid as i64)),
        ("output", ToonValue::Block(output)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_ACTION.to_string(),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn finished_outcome(code: i32, output: &str) -> ToolOutcome {
    let label = if code == 0 { "ok" } else { "error" };
    let text = toon_doc(&[
        ("status", ToonValue::Str(label)),
        ("exitCode", ToonValue::Int(code as i64)),
        ("output", ToonValue::Block(output)),
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

fn grant_shell_session_from(ctx: &ToolContext<'_>) {
    super::bash::grant_shell_session(ctx.session_id.as_deref());
}

fn kill_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
        }
        std::thread::sleep(Duration::from_millis(50));
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &child.id().to_string()])
            .status();
    }
    let _ = child.kill();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn drop_kills_the_process_group() {
        let dir = std::env::temp_dir();
        let launch = launch(
            "/bin/sh",
            "-c",
            "exec sleep 30",
            &dir,
            Duration::from_millis(200),
        )
        .expect("spawn");
        let Launch::Running(proc) = launch else {
            panic!("sleep should still be running");
        };
        let pid = proc.child.id();
        let turn = TurnSlot::start();
        turn.track(proc);
        drop(turn);
        assert!(!process_alive(pid));
    }

    fn process_alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}
