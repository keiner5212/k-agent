use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::Manager;

use super::{
    toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue, TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "lsp";

const DESCRIPTION: &str = "\
Ask the language server for one file. Operations: goToDefinition, findReferences, hover, \
documentSymbol, workspaceSymbol, goToImplementation, prepareCallHierarchy, incomingCalls, outgoingCalls. \
filePath is absolute or workspace-relative. line and character are 1-based. \
workspaceSymbol also takes query. The server comes from Settings. If none is installed for the file, the error says so. \
Use this instead of guessing a definition from a grep sample.\
";

const OPERATIONS: &[&str] = &[
    "goToDefinition",
    "findReferences",
    "hover",
    "documentSymbol",
    "workspaceSymbol",
    "goToImplementation",
    "prepareCallHierarchy",
    "incomingCalls",
    "outgoingCalls",
];

pub struct LspTool;

impl Tool for LspTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": OPERATIONS,
                        "description": "LSP operation."
                    },
                    "filePath": {
                        "type": "string",
                        "description": "Absolute or workspace-relative file."
                    },
                    "line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "1-based line. Ignored by documentSymbol and workspaceSymbol."
                    },
                    "character": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "1-based character. Ignored by documentSymbol and workspaceSymbol."
                    },
                    "query": {
                        "type": "string",
                        "description": "workspaceSymbol filter. Empty lists symbols."
                    }
                },
                "required": ["operation", "filePath"]
            }),
        }
    }

    fn execute(&self, _args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        super::context_error(None, "lsp must run on the async path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let Some(operation) = args.get("operation").and_then(Value::as_str) else {
        return super::context_error(None, "lsp requires `operation`.");
    };
    if !OPERATIONS.contains(&operation) {
        return super::context_error(
            None,
            &format!("lsp operation must be one of: {}.", OPERATIONS.join(", ")),
        );
    }
    let Some(raw) = args.get("filePath").and_then(Value::as_str) else {
        return super::context_error(None, "lsp requires `filePath`.");
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return super::context_error(None, "lsp `filePath` is empty.");
    }
    let Some(app) = ctx.app else {
        return super::context_error(Some(raw), "lsp needs the desktop shell.");
    };
    let resolved = match crate::pathutil::resolve_tool_path(raw, ctx.workspace_path().as_deref()) {
        Ok(path) => path,
        Err(message) => return super::context_error(Some(raw), &message),
    };
    if super::tool_utils::workspace::reject_if_unconfirmed(
        &resolved,
        ctx.workspace_path().as_deref(),
    ) {
        let probe = super::tool_utils::workspace::guard(ctx, raw, "LSP", false, || {
            super::ToolOutcome::text("ok")
        })
        .await;
        if probe.text.contains("denied") {
            return probe;
        }
    }
    if !resolved.is_file() {
        return super::context_error(
            Some(raw),
            &format!("File not found: {}", resolved.display()),
        );
    }
    let line = args.get("line").and_then(Value::as_u64).unwrap_or(1).max(1);
    let character = args
        .get("character")
        .and_then(Value::as_u64)
        .unwrap_or(1)
        .max(1);
    let query = args.get("query").and_then(Value::as_str).unwrap_or("");
    let position = json!({
        "textDocument": { "uri": "" },
        "position": { "line": line - 1, "character": character - 1 }
    });
    let hub = app.state::<crate::lsp_client::LspHub>();
    let result = match operation {
        "goToDefinition" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/definition".into(),
                Some(position),
            )
            .await
        }
        "findReferences" => {
            let mut params = position;
            params["context"] = json!({ "includeDeclaration": true });
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/references".into(),
                Some(params),
            )
            .await
        }
        "hover" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/hover".into(),
                Some(position),
            )
            .await
        }
        "documentSymbol" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/documentSymbol".into(),
                Some(json!({ "textDocument": { "uri": "" } })),
            )
            .await
        }
        "workspaceSymbol" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "workspace/symbol".into(),
                Some(json!({ "query": query })),
            )
            .await
        }
        "goToImplementation" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/implementation".into(),
                Some(position),
            )
            .await
        }
        "prepareCallHierarchy" => {
            crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/prepareCallHierarchy".into(),
                Some(position),
            )
            .await
        }
        "incomingCalls" | "outgoingCalls" => {
            let prepared = crate::lsp_client::lsp_request(
                app.clone(),
                hub.clone(),
                resolved.display().to_string(),
                "textDocument/prepareCallHierarchy".into(),
                Some(position),
            )
            .await;
            match prepared {
                Ok(value) => {
                    let item = value
                        .as_array()
                        .and_then(|items| items.first().cloned())
                        .or_else(|| value.get("name").map(|_| value.clone()));
                    let Some(item) = item else {
                        return ok_outcome(operation, &ctx.relative_path(&resolved), "none");
                    };
                    let method = if operation == "incomingCalls" {
                        "callHierarchy/incomingCalls"
                    } else {
                        "callHierarchy/outgoingCalls"
                    };
                    crate::lsp_client::lsp_request(
                        app.clone(),
                        hub.clone(),
                        resolved.display().to_string(),
                        method.into(),
                        Some(json!({ "item": item })),
                    )
                    .await
                }
                Err(error) => Err(error),
            }
        }
        _ => return super::context_error(None, "lsp operation is unknown."),
    };
    match result {
        Ok(value) => {
            let body = render_result(ctx, &value);
            ok_outcome(operation, &ctx.relative_path(&resolved), &body)
        }
        Err(error) => super::context_error(Some(&ctx.relative_path(&resolved)), &error.to_string()),
    }
}

fn ok_outcome(operation: &str, path: &str, body: &str) -> ToolOutcome {
    ToolOutcome {
        text: toon_doc(&[
            ("operation", ToonValue::Str(operation)),
            ("path", ToonValue::Str(path)),
            ("result", ToonValue::Block(body)),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            path: Some(path.to_string()),
            status: Some("ok".into()),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: None,
        file: None,
    }
}

fn render_result(ctx: &ToolContext<'_>, value: &Value) -> String {
    if value.is_null() {
        return "none".into();
    }
    if let Some(text) = hover_text(value) {
        return clip(&text, 1200);
    }
    let mut lines = Vec::new();
    collect_locations(ctx, value, &mut lines);
    if lines.is_empty() {
        if let Some(items) = value.as_array() {
            for item in items.iter().take(40) {
                if let Some(name) = item.get("name").and_then(Value::as_str) {
                    let kind = item.get("kind").and_then(Value::as_u64).unwrap_or(0);
                    lines.push(format!("{name} kind={kind}"));
                }
            }
        }
    }
    if lines.is_empty() {
        return clip(&value.to_string(), 1200);
    }
    let truncated = lines.len() > 40;
    lines.truncate(40);
    if truncated {
        lines.push("... (truncated at 40)".into());
    }
    lines.join("\n")
}

fn hover_text(value: &Value) -> Option<String> {
    let contents = value.get("contents")?;
    Some(markup(contents))
}

fn markup(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    if let Some(text) = value.get("value").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .map(markup)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
    }
    String::new()
}

fn collect_locations(ctx: &ToolContext<'_>, value: &Value, out: &mut Vec<String>) {
    if let Some(uri) = value
        .get("uri")
        .or_else(|| value.get("targetUri"))
        .and_then(Value::as_str)
    {
        let range = value
            .get("range")
            .or_else(|| value.get("targetSelectionRange"))
            .or_else(|| value.get("targetRange"));
        out.push(format_location(ctx, uri, range));
        return;
    }
    if let Some(location) = value.get("location") {
        collect_locations(ctx, location, out);
        return;
    }
    if let Some(items) = value.as_array() {
        for item in items {
            collect_locations(ctx, item, out);
        }
    }
}

fn format_location(ctx: &ToolContext<'_>, uri: &str, range: Option<&Value>) -> String {
    let path = uri_path(ctx, uri);
    let line = range
        .and_then(|item| item.get("start"))
        .and_then(|item| item.get("line"))
        .and_then(Value::as_u64)
        .map(|line| line + 1)
        .unwrap_or(1);
    let character = range
        .and_then(|item| item.get("start"))
        .and_then(|item| item.get("character"))
        .and_then(Value::as_u64)
        .map(|character| character + 1)
        .unwrap_or(1);
    format!("{path}:{line}:{character}")
}

fn uri_path(ctx: &ToolContext<'_>, uri: &str) -> String {
    let raw = uri.strip_prefix("file://").unwrap_or(uri);
    let path = PathBuf::from(raw);
    ctx.relative_path(&path)
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let clipped: String = text.chars().take(max).collect();
    format!("{clipped}... (truncated)")
}
