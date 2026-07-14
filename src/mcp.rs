//! Minimal, synchronous Model Context Protocol stdio boundary.
//!
//! Munin exposes a fixed set of local, read-only tools. Keeping the transport
//! narrow avoids an async runtime and preserves the crate's Rust 1.78 MSRV.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub fn serve_stdio() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve(stdin.lock(), stdout.lock())
}

pub fn serve<R: BufRead, W: Write>(reader: R, mut writer: W) -> Result<()> {
    for line in reader.lines() {
        let line = line.context("failed to read MCP stdio request")?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle_message(&message),
            Err(error) => Some(error_response(
                Value::Null,
                -32700,
                "Parse error",
                Some(json!({"detail": error.to_string()})),
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut writer, &response)
                .context("failed to write MCP stdio response")?;
            writer
                .write_all(b"\n")
                .context("failed to delimit MCP stdio response")?;
            writer.flush().context("failed to flush MCP response")?;
        }
    }
    Ok(())
}

fn handle_message(message: &Value) -> Option<Value> {
    let Some(object) = message.as_object() else {
        return Some(error_response(Value::Null, -32600, "Invalid Request", None));
    };
    let id = object.get("id").cloned();
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return id.map(|id| error_response(id, -32600, "Invalid Request", None));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return id.map(|id| error_response(id, -32600, "Invalid Request", None));
    };

    if id.is_none() {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    match method {
        "initialize" => Some(success_response(id, initialize_result(message))),
        "ping" => Some(success_response(id, json!({}))),
        "tools/list" => Some(success_response(id, json!({"tools": tool_definitions()}))),
        "tools/call" => Some(tool_call(id, message)),
        "resources/list" => Some(success_response(id, json!({"resources": []}))),
        "resources/templates/list" => Some(success_response(id, json!({"resourceTemplates": []}))),
        _ => Some(error_response(
            id,
            -32601,
            "Method not found",
            Some(json!({"method": method})),
        )),
    }
}

fn initialize_result(message: &Value) -> Value {
    let requested = message
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str);
    let protocol_version = requested
        .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {"tools": {}},
        "serverInfo": {
            "name": "munin-memory",
            "title": "Munin Memory",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Local, evidence-backed memory tools for coding agents"
        },
        "instructions": "Munin tools read local compiled memory. Results include provenance, freshness, sensitivity, warnings, and coverage receipts."
    })
}

fn tool_call(id: Value, message: &Value) -> Value {
    let Some(name) = message.pointer("/params/name").and_then(Value::as_str) else {
        return error_response(id, -32602, "Invalid params", None);
    };
    if !tool_names().contains(&name) {
        return error_response(id, -32602, "Unknown tool", Some(json!({"name": name})));
    }
    let arguments = message
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    match crate::mcp_tools::call_tool(name, &arguments) {
        Ok(structured) => {
            let text = serde_json::to_string(&structured)
                .unwrap_or_else(|_| "{\"error\":\"failed to serialize result\"}".to_string());
            success_response(
                id,
                json!({
                    "content": [{"type": "text", "text": text}],
                    "structuredContent": structured,
                    "isError": false
                }),
            )
        }
        Err(error) => success_response(
            id,
            json!({
                "content": [{"type": "text", "text": error.to_string()}],
                "isError": true
            }),
        ),
    }
}

fn tool_names() -> [&'static str; 6] {
    [
        "search",
        "session_summary",
        "project_state",
        "friction",
        "strategy_status",
        "health",
    ]
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        tool_definition(
            "search",
            "Search compiled memory",
            "Search ranked, evidence-backed local memory without opening raw transcripts.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "minLength": 1, "description": "Topic or decision to recall"},
                    "scope": scope_schema(),
                    "projectPath": {"type": "string"},
                    "limit": limit_schema()
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "session_summary",
            "Read an exact session summary",
            "Return the compiled five-bullet summary for one exact session identity.",
            json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "string", "minLength": 1},
                    "source": {"type": "string", "enum": ["codex", "claude"]},
                    "projectPath": {"type": "string"}
                },
                "required": ["sessionId"],
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "project_state",
            "Read project state",
            "Return compiled claims, open loops, checkpoints, and current next actions for a project.",
            json!({
                "type": "object",
                "properties": {
                    "projectPath": {"type": "string"},
                    "limit": limit_schema()
                },
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "friction",
            "Read repeated friction",
            "Return active repeated failure patterns and permanent-fix pointers.",
            json!({
                "type": "object",
                "properties": {
                    "scope": scope_schema(),
                    "projectPath": {"type": "string"},
                    "limit": limit_schema(),
                    "includeResolved": {"type": "boolean", "default": false}
                },
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "strategy_status",
            "Read strategy status",
            "Return goals, KPI and initiative status, evidence freshness, and instrumentation warnings.",
            json!({
                "type": "object",
                "properties": {"scope": {"type": "string", "minLength": 1}},
                "additionalProperties": false
            }),
        ),
        tool_definition(
            "health",
            "Read Memory OS health",
            "Return fast compiled-substrate health without triggering an import refresh.",
            json!({
                "type": "object",
                "properties": {
                    "scope": scope_schema(),
                    "projectPath": {"type": "string"}
                },
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool_definition(name: &str, title: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": result_schema(),
        "annotations": {
            "title": title,
            "readOnlyHint": true,
            "destructiveHint": false,
            "idempotentHint": true,
            "openWorldHint": false
        }
    })
}

fn result_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "schemaVersion": {"type": "string", "const": "munin.mcp.result.v1"},
            "tool": {"type": "string"},
            "generatedAt": {"type": "string", "format": "date-time"},
            "provenance": {"type": "array", "items": {"type": "object"}},
            "freshness": {"type": "object"},
            "sensitivity": {"type": "object"},
            "warnings": {"type": "array", "items": {"type": "string"}},
            "receipt": {"type": "object"},
            "data": {"type": "object"}
        },
        "required": [
            "schemaVersion", "tool", "generatedAt", "provenance", "freshness",
            "sensitivity", "warnings", "receipt", "data"
        ],
        "additionalProperties": false
    })
}

fn scope_schema() -> Value {
    json!({"type": "string", "enum": ["user", "project"], "default": "user"})
}

fn limit_schema() -> Value {
    json!({"type": "integer", "minimum": 1, "maximum": 20, "default": 5})
}

fn success_response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut error = json!({"code": code, "message": message});
    if let Some(data) = data {
        error["data"] = data;
    }
    json!({"jsonrpc": "2.0", "id": id, "error": error})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn older_supported_protocol_version_is_echoed() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
            "\n"
        );
        let mut output = Vec::new();
        serve(Cursor::new(input), &mut output).expect("serve");
        let response: Value = serde_json::from_slice(&output).expect("response");
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    }
}
