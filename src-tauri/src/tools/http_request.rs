use serde_json::{json, Value};

use super::tool_utils::http_call::{self, HttpCall};
use super::{
    context_error, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "http_request";

const DESCRIPTION: &str = "Send one HTTP request. Accepts any http or https URL, including localhost, plus method, headers, query, and body. GET and HEAD run immediately. Any other method waits for Deny, Accept this time, or Accept for this chat.";

pub struct HttpRequestTool;

impl Tool for HttpRequestTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Absolute http or https URL, including localhost." },
                    "method": { "type": "string", "description": "GET, HEAD, POST, PUT, PATCH, DELETE, or OPTIONS. Default GET." },
                    "headers": { "type": "object", "description": "Optional header map. Values are strings." },
                    "query": { "type": "object", "description": "Optional query parameter map appended to the URL." },
                    "body": { "type": "string", "description": "Optional raw body. Ignored for GET and HEAD." },
                    "timeoutMs": { "type": "integer", "description": "Timeout in milliseconds. Default 20000, max 60000." }
                },
                "required": ["url"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        match args.get("url").and_then(Value::as_str) {
            Some(url) if !url.trim().is_empty() => {
                context_error(None, "http_request must run on the async dispatch path.")
            }
            _ => context_error(None, "http_request requires a non-empty `url`."),
        }
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let url = args.get("url").and_then(Value::as_str).unwrap_or("").trim();
    if url.is_empty() {
        return context_error(None, "http_request requires a non-empty `url`.");
    }
    let method = match http_call::normalize_method(
        args.get("method").and_then(Value::as_str).unwrap_or("GET"),
    ) {
        Ok(method) => method,
        Err(message) => return context_error(None, &message),
    };
    if let Err(message) = super::tool_utils::workspace::ensure_http_write(ctx, &method).await {
        return context_error(None, &message);
    }
    let headers = match http_call::pairs_from_object(args.get("headers"), "headers") {
        Ok(headers) => headers,
        Err(message) => return context_error(None, &message),
    };
    let query = match http_call::pairs_from_object(args.get("query"), "query") {
        Ok(query) => query,
        Err(message) => return context_error(None, &message),
    };
    let body = args.get("body").and_then(Value::as_str).map(str::to_string);
    let timeout_ms = http_call::timeout_ms(args.get("timeoutMs").and_then(Value::as_u64));
    match http_call::send(HttpCall {
        method: method.clone(),
        url: url.to_string(),
        headers,
        query,
        body,
        timeout_ms,
    })
    .await
    {
        Ok(result) => ToolOutcome {
            text: toon_doc(&[
                ("url", ToonValue::Str(&result.final_url)),
                ("method", ToonValue::Str(&method)),
                ("status", ToonValue::Int(result.status as i64)),
                (
                    "truncated",
                    ToonValue::Int(if result.truncated { 1 } else { 0 }),
                ),
                ("headers", ToonValue::Block(&result.headers)),
                ("body", ToonValue::Block(&result.body)),
            ]),
            display: ToolDisplay {
                kind: TOOL_KIND_CONTEXT.to_string(),
                status: Some("ok".into()),
                ..ToolDisplay::default()
            },
            snapshot: None,
            image_png: None,
            file: None,
        },
        Err(message) => context_error(None, &message),
    }
}
