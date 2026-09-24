use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{sinks::UTF8, SearcherBuilder};
use ignore::WalkBuilder;
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "grep";

const DESCRIPTION: &str = "Search file contents with the ripgrep engine compiled into the app. pattern is a regex. path defaults to the workspace. glob is an include filter (*.rs, *.{ts,tsx}); a leading ! excludes. caseInsensitive matches either letter case. count is the total number of matches. matches is a sample: at most 20 lines per file and 100 lines overall. No system rg binary.";

const MAX_MATCHES: usize = 100;
const MAX_PER_FILE: usize = 20;
const MAX_LINE_CHARS: usize = 400;

pub struct GrepTool;

impl Tool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Ripgrep regex."
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory. Absolute or workspace-relative. Default: workspace root."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Include only paths matching this glob, for example *.ts or *.{rs,toml}. A leading ! excludes instead."
                    },
                    "caseInsensitive": {
                        "type": "boolean",
                        "description": "Match letters regardless of case. Default false."
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    fn execute(&self, args: &Value, ctx: &ToolContext<'_>) -> ToolOutcome {
        let Some(pattern) = args.get("pattern").and_then(Value::as_str) else {
            return error_outcome("grep requires a string `pattern`.");
        };
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return error_outcome("grep `pattern` is empty.");
        }
        let raw_path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let resolved =
            match crate::pathutil::resolve_tool_path(raw_path, ctx.workspace_path().as_deref()) {
                Ok(path) => path,
                Err(error) => return error_outcome(&error.to_string()),
            };
        if super::tool_utils::workspace::reject_if_unconfirmed(
            &resolved,
            ctx.workspace_path().as_deref(),
        ) {
            return error_outcome(
                "grep outside the workspace must run on the async dispatch path.",
            );
        }
        let glob = args
            .get("glob")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let case_insensitive = args
            .get("caseInsensitive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let jobs = ctx.parallelism.clamp(1, 16);
        let workspace = ctx.workspace_path();
        match search(
            &resolved,
            workspace.as_deref(),
            pattern,
            glob,
            case_insensitive,
            jobs,
        ) {
            Ok(found) => {
                let truncated = found.total > found.lines.len() as u64;
                ok_outcome(pattern, found.total, truncated, &found.lines.join("\n"))
            }
            Err(message) => error_outcome(&message),
        }
    }
}

#[derive(Debug)]
struct SearchHit {
    total: u64,
    lines: Vec<String>,
}

fn search(
    root: &Path,
    workspace: Option<&Path>,
    pattern: &str,
    glob: &str,
    case_insensitive: bool,
    jobs: usize,
) -> Result<SearchHit, String> {
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(case_insensitive)
        .build(pattern)
        .map_err(|error| format!("grep: {error}"))?;
    let mut walk = WalkBuilder::new(root);
    walk.threads(jobs)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !matches!(name.as_ref(), ".git" | "node_modules" | "target" | "dist")
        });
    if !glob.is_empty() {
        let base = if root.is_dir() {
            root
        } else {
            root.parent().unwrap_or(root)
        };
        let mut overrides = ignore::overrides::OverrideBuilder::new(base);
        overrides
            .add(glob)
            .map_err(|error| format!("grep: {error}"))?;
        walk.overrides(
            overrides
                .build()
                .map_err(|error| format!("grep: {error}"))?,
        );
    }
    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let total = Arc::new(AtomicU64::new(0));
    walk.build_parallel().run(|| {
        let matcher = matcher.clone();
        let hits = Arc::clone(&hits);
        let total = Arc::clone(&total);
        Box::new(move |result| {
            let Ok(entry) = result else {
                return ignore::WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                return ignore::WalkState::Continue;
            }
            let mut searcher = SearcherBuilder::new().line_number(true).build();
            let mut local = Vec::new();
            let mut file_hits = 0u64;
            let searched = searcher.search_path(
                &matcher,
                entry.path(),
                UTF8(|line_number, line| {
                    file_hits += 1;
                    if local.len() < MAX_PER_FILE {
                        let text = trim_line(line.trim_end());
                        let rel = display_path(entry.path(), workspace);
                        local.push(format!("{rel}:{line_number}:{text}"));
                    }
                    Ok(true)
                }),
            );
            if searched.is_err() {
                return ignore::WalkState::Continue;
            }
            total.fetch_add(file_hits, Ordering::Relaxed);
            if local.is_empty() {
                return ignore::WalkState::Continue;
            }
            let mut guard = hits.lock().unwrap_or_else(|err| err.into_inner());
            for line in local {
                if guard.len() >= MAX_MATCHES {
                    break;
                }
                guard.push(line);
            }
            ignore::WalkState::Continue
        })
    });
    let mut lines = hits.lock().unwrap_or_else(|err| err.into_inner()).clone();
    lines.sort();
    let total = total.load(Ordering::Relaxed);
    Ok(SearchHit { total, lines })
}

fn display_path(path: &Path, workspace: Option<&Path>) -> String {
    let relative = workspace
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let text = relative.to_string_lossy().replace('\\', "/");
    text.trim_start_matches("./").to_string()
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let raw = args.get("path").and_then(Value::as_str).unwrap_or(".");
    super::tool_utils::workspace::guard(ctx, raw, "Search", false, || GrepTool.execute(&args, ctx))
        .await
}

fn trim_line(line: &str) -> String {
    let mut out = String::new();
    for ch in line.chars().take(MAX_LINE_CHARS) {
        out.push(ch);
    }
    out
}

fn ok_outcome(pattern: &str, total: u64, truncated: bool, matches: &str) -> ToolOutcome {
    let count = i64::try_from(total).unwrap_or(i64::MAX);
    let truncated = if truncated { "true" } else { "false" };
    let text = toon_doc(&[
        ("pattern", ToonValue::Str(pattern)),
        ("count", ToonValue::Int(count)),
        ("truncated", ToonValue::Str(truncated)),
        ("matches", ToonValue::Block(matches)),
    ]);
    ToolOutcome {
        text,
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("ok".into()),
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
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("error".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("k-agent-grep-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn glob_includes_only_matching_files() {
        let dir = scratch();
        fs::write(dir.join("keep.md"), "TOKEN_TARGET\n").unwrap();
        fs::write(dir.join("skip.txt"), "TOKEN_TARGET\n").unwrap();
        let found = search(&dir, Some(&dir), "TOKEN_TARGET", "*.md", false, 2).unwrap();
        assert_eq!(found.total, 1, "{found:?}");
        assert!(found.lines.iter().all(|line| line.contains("keep.md")));
        assert!(found.lines.iter().all(|line| !line.contains("skip.txt")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn case_insensitive_is_opt_in() {
        let dir = scratch();
        fs::write(dir.join("a.txt"), "TOKEN_TARGET\n").unwrap();
        let miss = search(&dir, Some(&dir), "token_target", "", false, 1).unwrap();
        assert_eq!(miss.total, 0);
        let hit = search(&dir, Some(&dir), "token_target", "", true, 1).unwrap();
        assert_eq!(hit.total, 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn count_is_the_total_and_matches_are_a_sample() {
        let dir = scratch();
        let mut body = String::new();
        for _ in 0..150 {
            body.push_str("HIT\n");
        }
        fs::write(dir.join("a.txt"), body).unwrap();
        let found = search(&dir, Some(&dir), "HIT", "", false, 1).unwrap();
        assert_eq!(found.total, 150);
        assert_eq!(found.lines.len(), MAX_PER_FILE);
        let _ = fs::remove_dir_all(dir);
    }
}
