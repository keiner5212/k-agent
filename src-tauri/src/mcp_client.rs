use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::mcp_servers::{McpServer, McpServerError, McpToolSummary, McpTransport};

const MCP_PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
const MCP_SESSION_HEADER: &str = "mcp-session-id";

#[derive(Debug, Deserialize)]
struct ToolsListPage {
    #[serde(default)]
    tools: Vec<McpToolSummary>,
    #[serde(default, rename = "nextCursor")]
    next_cursor: Option<String>,
}

pub async fn probe_tools(server: &McpServer) -> Result<Vec<McpToolSummary>, McpServerError> {
    match server.transport {
        McpTransport::Stdio => probe_stdio(server).await,
        McpTransport::Http => probe_http(server).await,
        McpTransport::Sse => Err(McpServerError::ProbeUnsupported),
    }
}

async fn probe_stdio(server: &McpServer) -> Result<Vec<McpToolSummary>, McpServerError> {
    let command = server
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(McpServerError::CommandRequired)?;

    let mut child = Command::new(command);
    child
        .args(&server.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(cwd) = server.cwd.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        child.current_dir(cwd);
    }
    for (key, value) in server.probe_env() {
        child.env(key, value);
    }

    let mut child = child
        .spawn()
        .map_err(|error| McpServerError::ProbeFailed(error.to_string()))?;
    let stdin = child.stdin.take().ok_or_else(|| McpServerError::ProbeFailed("stdin unavailable".into()))?;
    let stdout = child.stdout.take().ok_or_else(|| McpServerError::ProbeFailed("stdout unavailable".into()))?;

    let probe = async {
        let mut stdin = stdin;
        let mut reader = BufReader::new(stdout);
        rpc_request_stdio(
            &mut reader,
            &mut stdin,
            1,
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "k-agent",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        )
        .await?;
        write_message_stdio(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            }),
        )
        .await?;
        let result = rpc_request_stdio(&mut reader, &mut stdin, 2, "tools/list", json!({})).await?;
        Ok(normalize_tools(parse_tools_page(result)?.tools))
    };

    let result = timeout(MCP_PROBE_TIMEOUT, probe).await;
    let _ = child.kill().await;
    match result {
        Ok(Ok(tools)) => Ok(tools),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(McpServerError::ProbeFailed("timed out".into())),
    }
}

async fn probe_http(server: &McpServer) -> Result<Vec<McpToolSummary>, McpServerError> {
    let url = server
        .url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(McpServerError::UrlRequired)?;
    let client = http_client()?;
    let mut session_id: Option<String> = None;

    http_rpc(
        &client,
        url,
        server,
        &mut session_id,
        1,
        "initialize",
        json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": "k-agent",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }),
    )
    .await?;
    http_notification(&client, url, server, &mut session_id, "notifications/initialized").await?;

    let mut tools = Vec::new();
    let mut cursor: Option<String> = None;
    let mut request_id = 2u64;
    loop {
        let params = match cursor.as_deref() {
            Some(value) if !value.is_empty() => json!({ "cursor": value }),
            _ => json!({}),
        };
        let page = parse_tools_page(
            http_rpc(
                &client,
                url,
                server,
                &mut session_id,
                request_id,
                "tools/list",
                params,
            )
            .await?,
        )?;
        request_id += 1;
        tools.extend(page.tools);
        let next = page
            .next_cursor
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if next.is_none() {
            break;
        }
        cursor = next;
    }

    Ok(normalize_tools(tools))
}

fn http_client() -> Result<Client, McpServerError> {
    Client::builder()
        .timeout(MCP_PROBE_TIMEOUT)
        .user_agent(concat!("k-agent/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| McpServerError::ProbeFailed(error.to_string()))
}

async fn http_rpc(
    client: &Client,
    url: &str,
    server: &McpServer,
    session_id: &mut Option<String>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, McpServerError> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let response = send_http_request(client, url, server, session_id, &body).await?;
    extract_rpc_result(&response, id)
}

async fn http_notification(
    client: &Client,
    url: &str,
    server: &McpServer,
    session_id: &mut Option<String>,
    method: &str,
) -> Result<(), McpServerError> {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": {},
    });
    let _ = send_http_request(client, url, server, session_id, &body).await?;
    Ok(())
}

async fn send_http_request(
    client: &Client,
    url: &str,
    server: &McpServer,
    session_id: &mut Option<String>,
    body: &Value,
) -> Result<Value, McpServerError> {
    let mut request = client
        .post(url)
        .header(ACCEPT, "application/json, text/event-stream")
        .header(CONTENT_TYPE, "application/json")
        .json(body);
    for (key, value) in server.probe_headers() {
        request = request.header(key, value);
    }
    if let Some(id) = session_id.as_deref() {
        request = request.header(MCP_SESSION_HEADER, id);
    }

    let response = request
        .send()
        .await
        .map_err(|error| McpServerError::ProbeFailed(error.to_string()))?;
    if let Some(next) = response
        .headers()
        .get(MCP_SESSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        *session_id = Some(next.to_string());
    }

    let status = response.status();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| McpServerError::ProbeFailed(error.to_string()))?;
    if !status.is_success() {
        let detail = String::from_utf8_lossy(&bytes).trim().to_string();
        let message = if detail.is_empty() {
            format!("http {status}")
        } else {
            format!("http {status}: {detail}")
        };
        return Err(McpServerError::ProbeFailed(message));
    }
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    parse_http_body(&content_type, &bytes)
}

fn parse_http_body(content_type: &str, bytes: &[u8]) -> Result<Value, McpServerError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| McpServerError::Parse(error.to_string()))?
        .trim();
    if text.is_empty() {
        return Err(McpServerError::ProbeFailed("empty http response".into()));
    }
    if content_type.contains("text/event-stream") || text.starts_with("event:") || text.contains("\ndata:") {
        return parse_sse_payload(text);
    }
    serde_json::from_str(text).map_err(|error| McpServerError::Parse(error.to_string()))
}

fn parse_sse_payload(text: &str) -> Result<Value, McpServerError> {
    let mut last_message = None;
    for line in text.lines() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() {
            continue;
        }
        let message: Value =
            serde_json::from_str(data).map_err(|error| McpServerError::Parse(error.to_string()))?;
        last_message = Some(message);
    }
    last_message.ok_or_else(|| McpServerError::ProbeFailed("sse response missing data".into()))
}

fn extract_rpc_result(message: &Value, id: u64) -> Result<Value, McpServerError> {
    if message.is_null() {
        return Err(McpServerError::ProbeFailed("empty http response".into()));
    }
    let message_id = message
        .get("id")
        .and_then(json_id)
        .ok_or_else(|| McpServerError::ProbeFailed("response missing id".into()))?;
    if message_id != id {
        return Err(McpServerError::ProbeFailed("unexpected response id".into()));
    }
    if let Some(error) = message.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("mcp request failed");
        return Err(McpServerError::ProbeFailed(message.to_string()));
    }
    message
        .get("result")
        .cloned()
        .ok_or_else(|| McpServerError::ProbeFailed("response missing result".into()))
}

async fn rpc_request_stdio(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    stdin: &mut tokio::process::ChildStdin,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, McpServerError> {
    write_message_stdio(
        stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }),
    )
    .await?;
    wait_for_response_stdio(reader, id).await
}

async fn write_message_stdio(
    stdin: &mut tokio::process::ChildStdin,
    body: &Value,
) -> Result<(), McpServerError> {
    let encoded = serde_json::to_string(body).map_err(|error| McpServerError::Parse(error.to_string()))?;
    stdin
        .write_all(encoded.as_bytes())
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))?;
    stdin
        .flush()
        .await
        .map_err(|error| McpServerError::Io(error.to_string()))
}

async fn wait_for_response_stdio(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    id: u64,
) -> Result<Value, McpServerError> {
    loop {
        let message = read_message_stdio(reader).await?;
        if message.get("method").is_some() && message.get("id").is_none() {
            continue;
        }
        return extract_rpc_result(&message, id);
    }
}

async fn read_message_stdio(
    reader: &mut BufReader<tokio::process::ChildStdout>,
) -> Result<Value, McpServerError> {
    loop {
        let mut line = String::new();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|error| McpServerError::Io(error.to_string()))?;
        if read == 0 {
            return Err(McpServerError::ProbeFailed("server closed stdout".into()));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return serde_json::from_str(trimmed).map_err(|error| McpServerError::Parse(error.to_string()));
    }
}

fn json_id(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .or_else(|| number.as_i64().and_then(|n| u64::try_from(n).ok())),
        Value::String(raw) => raw.parse().ok(),
        _ => None,
    }
}

fn parse_tools_page(result: Value) -> Result<ToolsListPage, McpServerError> {
    serde_json::from_value(result).map_err(|error| McpServerError::Parse(error.to_string()))
}

fn normalize_tools(tools: Vec<McpToolSummary>) -> Vec<McpToolSummary> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for tool in tools {
        let name = tool.name.trim();
        if name.is_empty() || seen.contains_key(name) {
            continue;
        }
        seen.insert(name.to_string(), ());
        out.push(McpToolSummary {
            name: name.to_string(),
            description: tool
                .description
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        });
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    out
}
