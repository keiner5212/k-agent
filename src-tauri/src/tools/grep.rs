use std::path::Path;
use std::sync::{Arc, Mutex};

use grep_regex::RegexMatcher;
use grep_searcher::{sinks::UTF8, SearcherBuilder};
use ignore::WalkBuilder;
use serde_json::{json, Value};

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "grep";

const DESCRIPTION: &str = "Search file contents with the ripgrep engine compiled into the app. pattern is a regex. path defaults to the workspace. glob limits files (for example *.rs). Uses the configured worker cores. Results are capped. No system rg binary.";

const MAX_MATCHES: usize = 100;
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
                        "description": "Optional glob, for example *.ts or *.{rs,toml}."
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
        let jobs = ctx.parallelism.clamp(1, 16);
        let workspace = ctx.workspace_path();
        match search(&resolved, workspace.as_deref(), pattern, glob, jobs) {
            Ok(found) => {
                let truncated = if found.truncated { "true" } else { "false" };
                ok_outcome(
                    pattern,
                    &found.lines.len().to_string(),
                    truncated,
                    &found.lines.join("\n"),
                )
            }
            Err(message) => error_outcome(&message),
        }
    }
}

struct SearchHit {
    lines: Vec<String>,
    truncated: bool,
}

fn search(
    root: &Path,
    workspace: Option<&Path>,
    pattern: &str,
    glob: &str,
    jobs: usize,
) -> Result<SearchHit, String> {
    let matcher = RegexMatcher::new(pattern).map_err(|error| format!("grep: {error}"))?;
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
        let mut overrides = ignore::overrides::OverrideBuilder::new(root);
        overrides
            .add("*")
            .map_err(|error| format!("grep: {error}"))?;
        overrides
            .add(&format!("!{glob}"))
            .map_err(|error| format!("grep: {error}"))?;
        walk.overrides(
            overrides
                .build()
                .map_err(|error| format!("grep: {error}"))?,
        );
    }
    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let truncated = Arc::new(Mutex::new(false));
    walk.build_parallel().run(|| {
        let matcher = matcher.clone();
        let hits = Arc::clone(&hits);
        let truncated = Arc::clone(&truncated);
        Box::new(move |result| {
            if hits.lock().map(|guard| guard.len()).unwrap_or(MAX_MATCHES) >= MAX_MATCHES {
                *truncated.lock().unwrap_or_else(|err| err.into_inner()) = true;
                return ignore::WalkState::Quit;
            }
            let Ok(entry) = result else {
                return ignore::WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                return ignore::WalkState::Continue;
            }
            let mut searcher = SearcherBuilder::new().line_number(true).build();
            let mut local = Vec::new();
            let searched = searcher.search_path(
                &matcher,
                entry.path(),
                UTF8(|line_number, line| {
                    if local.len() >= 20 {
                        return Ok(false);
                    }
                    let text = line.trim_end();
                    let text = trim_line(text);
                    let rel = display_path(entry.path(), workspace);
                    local.push(format!("{rel}:{line_number}:{text}"));
                    Ok(true)
                }),
            );
            if searched.is_err() {
                return ignore::WalkState::Continue;
            }
            let mut guard = hits.lock().unwrap_or_else(|err| err.into_inner());
            for line in local {
                if guard.len() >= MAX_MATCHES {
                    *truncated.lock().unwrap_or_else(|err| err.into_inner()) = true;
                    return ignore::WalkState::Quit;
                }
                guard.push(line);
            }
            ignore::WalkState::Continue
        })
    });
    let lines = hits.lock().unwrap_or_else(|err| err.into_inner()).clone();
    let truncated = *truncated.lock().unwrap_or_else(|err| err.into_inner());
    Ok(SearchHit { lines, truncated })
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

fn ok_outcome(pattern: &str, count: &str, truncated: &str, matches: &str) -> ToolOutcome {
    let count_num: i64 = count.parse().unwrap_or(0);
    let text = toon_doc(&[
        ("pattern", ToonValue::Str(pattern)),
        ("count", ToonValue::Int(count_num)),
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
