use std::cell::Cell;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::super::ask_user::{ask_user_wait, AskUserOption, AskUserQuestion};
use super::super::{action_error, context_error, ToolContext, ToolOutcome};

thread_local! {
    static CONFIRMED: Cell<bool> = const { Cell::new(false) };
}

pub fn is_outside_workspace(path: &Path, workspace: Option<&Path>) -> bool {
    let Some(root) = workspace else {
        return true;
    };
    !canonical_or_normalize(path).starts_with(canonical_or_normalize(root))
}

pub fn reject_if_unconfirmed(path: &Path, workspace: Option<&Path>) -> bool {
    is_outside_workspace(path, workspace) && !CONFIRMED.with(Cell::get)
}

pub fn with_confirmed<T>(run: impl FnOnce() -> T) -> T {
    CONFIRMED.with(|flag| {
        let previous = flag.get();
        flag.set(true);
        let outcome = run();
        flag.set(previous);
        outcome
    })
}

pub async fn guard(
    ctx: &ToolContext<'_>,
    raw: &str,
    verb: &str,
    action: bool,
    run: impl FnOnce() -> ToolOutcome,
) -> ToolOutcome {
    let workspace = ctx.workspace_path();
    let resolved = match crate::pathutil::resolve_tool_path(raw, workspace.as_deref()) {
        Ok(path) => path,
        Err(_) => return run(),
    };
    if !is_outside_workspace(&resolved, workspace.as_deref()) {
        return run();
    }
    if session_granted(ctx.session_id.as_deref()) {
        return with_confirmed(run);
    }
    let rel = ctx.relative_path(&resolved);
    match confirm_outside(ctx, verb, &resolved, &rel).await {
        OutsideChoice::Deny => {
            let message = "User denied access outside the workspace.";
            if action {
                action_error(&rel, message)
            } else {
                context_error(Some(&rel), message)
            }
        }
        OutsideChoice::Once => with_confirmed(run),
        OutsideChoice::Session => {
            grant_session(ctx.session_id.as_deref());
            with_confirmed(run)
        }
    }
}

enum OutsideChoice {
    Deny,
    Once,
    Session,
}

fn session_grants() -> &'static Mutex<HashSet<String>> {
    static GRANTS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    GRANTS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn session_granted(session_id: Option<&str>) -> bool {
    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return false;
    };
    session_grants()
        .lock()
        .map(|grants| grants.contains(session_id))
        .unwrap_or(false)
}

fn grant_session(session_id: Option<&str>) {
    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return;
    };
    if let Ok(mut grants) = session_grants().lock() {
        grants.insert(session_id.to_string());
    }
}

async fn confirm_outside(
    ctx: &ToolContext<'_>,
    verb: &str,
    resolved: &Path,
    rel: &str,
) -> OutsideChoice {
    let questions = vec![AskUserQuestion {
        id: "outside_confirm".to_string(),
        header: "Outside workspace".to_string(),
        question: format!(
            "`{rel}` is outside the workspace. {verb} `{}`?",
            resolved.display()
        ),
        options: vec![
            AskUserOption {
                label: "Deny".to_string(),
                description: Some("Stop. Do not touch this path.".to_string()),
                preview: None,
            },
            AskUserOption {
                label: "Accept this time".to_string(),
                description: Some("Allow this action only.".to_string()),
                preview: None,
            },
            AskUserOption {
                label: "Accept for this chat".to_string(),
                description: Some(
                    "Allow outside-workspace access for the rest of this chat.".to_string(),
                ),
                preview: None,
            },
        ],
        multi_select: false,
        allow_free_text: false,
    }];
    let call_id = format!("outside_confirm::{}", ctx.call_id);
    let answer = ask_user_wait(ctx, &call_id, &questions).await;
    let entry = answer
        .iter()
        .find(|entry| entry.question_id == "outside_confirm");
    let Some(entry) = entry else {
        return OutsideChoice::Deny;
    };
    if entry.skipped {
        return OutsideChoice::Deny;
    }
    if entry
        .selected
        .iter()
        .any(|label| label == "Accept for this chat")
    {
        return OutsideChoice::Session;
    }
    if entry
        .selected
        .iter()
        .any(|label| label == "Accept this time")
    {
        return OutsideChoice::Once;
    }
    OutsideChoice::Deny
}

fn canonical_or_normalize(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| normalize(path))
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_grant_persists_for_that_chat_only() {
        grant_session(Some("chat-a"));
        assert!(session_granted(Some("chat-a")));
        assert!(!session_granted(Some("chat-b")));
        assert!(!session_granted(None));
    }
}
