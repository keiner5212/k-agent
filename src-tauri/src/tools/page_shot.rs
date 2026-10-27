use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};

use super::{
    context_error, toon_doc, Tool, ToolContext, ToolDisplay, ToolOutcome, ToolSpec, ToonValue,
    TOOL_KIND_CONTEXT,
};
use crate::browser::parse_shot;

pub const NAME: &str = "page_shot";

const DESCRIPTION: &str = "Capture one viewport of a page that is already being served and return a PNG. One shot per review. Do not retry with another host, a taller window, or a new selector when the image is blank or unchanged. A blank image is a capture miss, not the page. Height is the window (max 1200), not the document. A URL hash scrolls that section into the window. Optional localStorage and sessionStorage are {key, value} lists applied on the page origin before the app script runs. Optional cookies are {name, value} plus optional domain, path, secure, and httpOnly, set before the first request. Default cookie domain is the URL host and must match it. Path defaults to /. Each list is at most 50 items. Each value is at most 8 KB. Start a server with background. It is killed when the turn ends. Do not use bash for that.";

pub struct PageShotTool;

impl Tool for PageShotTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME,
            description: DESCRIPTION,
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Page URL. http or https, including localhost." },
                    "width": { "type": "integer", "description": "Viewport width in pixels. Default 1280. Range 320-1600." },
                    "height": { "type": "integer", "description": "Viewport height in pixels. Default 720. Range 240-1200. This is the window, not the page length." },
                    "selector": { "type": "string", "description": "Optional CSS selector. When set, the image is only that element. Prefer one full viewport shot." },
                    "waitMs": { "type": "integer", "description": "Extra delay after the view has painted. Default 300. Max 5000." },
                    "localStorage": {
                        "type": "array",
                        "description": "Optional. Up to 50 {key, value} strings. Applied on the page origin before the app script runs. Each value max 8 KB.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["key", "value"]
                        }
                    },
                    "sessionStorage": {
                        "type": "array",
                        "description": "Optional. Same shape and timing as localStorage, for sessionStorage.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["key", "value"]
                        }
                    },
                    "cookies": {
                        "type": "array",
                        "description": "Optional. Up to 50 cookies, set before the first request. name and value are required. domain defaults to the URL host and must match it. path defaults to /. secure and httpOnly default false. Each value max 8 KB.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "value": { "type": "string" },
                                "domain": { "type": "string" },
                                "path": { "type": "string" },
                                "secure": { "type": "boolean" },
                                "httpOnly": { "type": "boolean" }
                            },
                            "required": ["name", "value"]
                        }
                    }
                },
                "required": ["url"]
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
        {
            return context_error(None, "page_shot requires a non-empty `url`.");
        }
        context_error(None, "page_shot must run on the async dispatch path.")
    }
}

pub async fn execute_async(arguments: &str, ctx: &ToolContext<'_>) -> ToolOutcome {
    let args: Value = serde_json::from_str(arguments).unwrap_or(Value::Null);
    let shot = match parse_shot(&args) {
        Ok(shot) => shot,
        Err(message) => return context_error(None, &message),
    };
    let Some(app) = ctx.app else {
        return context_error(None, "page_shot requires the desktop shell.");
    };
    let png = match crate::browser::capture(app, &shot).await {
        Ok(png) => png,
        Err(message) => return context_error(None, &message),
    };
    let encoded = BASE64.encode(&png);
    let selector_label = shot.selector.clone().unwrap_or_default();
    let local_count = shot.local_storage.len() as i64;
    let session_count = shot.session_storage.len() as i64;
    let cookie_count = shot.cookies.len() as i64;
    ToolOutcome {
        text: toon_doc(&[
            ("url", ToonValue::Str(&shot.url)),
            ("width", ToonValue::Int(shot.width as i64)),
            ("height", ToonValue::Int(shot.height as i64)),
            ("selector", ToonValue::Str(&selector_label)),
            ("localStorage", ToonValue::Int(local_count)),
            ("sessionStorage", ToonValue::Int(session_count)),
            ("cookies", ToonValue::Int(cookie_count)),
            ("bytes", ToonValue::Int(png.len() as i64)),
            ("image", ToonValue::Str("png attached")),
        ]),
        display: ToolDisplay {
            kind: TOOL_KIND_CONTEXT.to_string(),
            status: Some("ok".into()),
            image_data: Some(encoded),
            ..ToolDisplay::default()
        },
        snapshot: None,
        image_png: Some(png),
        file: None,
    }
}
