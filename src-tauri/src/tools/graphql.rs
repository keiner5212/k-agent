use serde_json::{json, Value};

use super::tool_utils::http_call::{self, HttpCall};
use super::{
    context_error, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_CONTEXT,
};

pub const NAME: &str = "graphql";

const DESCRIPTION: &str = "Send one GraphQL request. POST by default, with query, optional variables, operation name, and headers. GET is allowed for queries and does not ask. POST and any other method wait for Deny, Accept this time, or Accept for this chat.";

pub struct GraphqlTool;

impl Tool for GraphqlTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "GraphQL endpoint. http or https, including localhost." },
                    "query": { "type": "string", "description": "GraphQL query or mutation document." },
                    "variables": { "type": "object", "description": "Optional variables object." },
                    "operationName": { "type": "string", "description": "Optional operation name." },
                    "headers": { "type": "object", "description": "Optional header map." },
                    "method": { "type": "string", "description": "POST (default) or GET. Non-GET waits for confirmation." },
                    "timeoutMs": { "type": "integer", "description": "Timeout in milliseconds. Default 20000, max 60000." }
                },
                "required": ["url", "query"]
            }),
        }
    }

    fn execute(&self, args: &Value, _ctx: &ToolContext<'_>) -> ToolOutcome {
        if args
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
            || args
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return context_error(None, "graphql requires non-empty `url` and `query`.");
        }
        context_error(None, "graphql must run on the async dispatch path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let url = args.get("url").and_then(Value::as_str).unwrap_or("").trim();
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if url.is_empty() || query.is_empty() {
        return context_error(None, "graphql requires non-empty `url` and `query`.");
    }
    let method = match http_call::normalize_method(
        args.get("method").and_then(Value::as_str).unwrap_or("POST"),
    ) {
        Ok(method) => method,
        Err(message) => return context_error(None, &message),
    };
    if let Err(message) = super::tool_utils::workspace::ensure_http_write(ctx, &method).await {
        return context_error(None, &message);
    }
    let headers = match http_call::pairs_from_object(args.get("headers"), "headers") {
        Ok(mut headers) => {
            if method != "GET"
                && method != "HEAD"
                && !headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            {
                headers.push(("content-type".into(), "application/json".into()));
            }
            headers
        }
        Err(message) => return context_error(None, &message),
    };
    let operation_name = args
        .get("operationName")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let variables = args
        .get("variables")
        .cloned()
        .filter(|value| !value.is_null());
    let timeout_ms = http_call::timeout_ms(args.get("timeoutMs").and_then(Value::as_u64));
    let call = if method == "GET" || method == "HEAD" {
        let mut query_pairs = vec![("query".to_string(), query.to_string())];
        if let Some(name) = &operation_name {
            query_pairs.push(("operationName".into(), name.clone()));
        }
        if let Some(variables) = &variables {
            query_pairs.push(("variables".into(), variables.to_string()));
        }
        HttpCall {
            method,
            url: url.to_string(),
            headers,
            query: query_pairs,
            body: None,
            timeout_ms,
        }
    } else {
        let mut payload = serde_json::Map::new();
        payload.insert("query".into(), Value::String(query.to_string()));
        if let Some(variables) = variables {
            payload.insert("variables".into(), variables);
        }
        if let Some(name) = operation_name {
            payload.insert("operationName".into(), Value::String(name));
        }
        HttpCall {
            method,
            url: url.to_string(),
            headers,
            query: Vec::new(),
            body: Some(Value::Object(payload).to_string()),
            timeout_ms,
        }
    };
    let method = call.method.clone();
    match http_call::send(call).await {
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
        },
        Err(message) => context_error(None, &message),
    }
}
