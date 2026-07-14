use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_mcp(lines: &[Value], raw_tail: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_munin"))
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start munin mcp server");

    {
        let stdin = child.stdin.as_mut().expect("mcp stdin");
        for line in lines {
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(line).expect("request json")
            )
            .expect("write mcp request");
        }
        if let Some(raw) = raw_tail {
            writeln!(stdin, "{raw}").expect("write raw request");
        }
    }

    child.wait_with_output().expect("collect mcp output")
}

fn parse_protocol_lines(output: &Output) -> Vec<Value> {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    stdout
        .lines()
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| {
                panic!("stdout contained non-JSON protocol text `{line}`: {error}")
            })
        })
        .collect()
}

#[test]
fn stdio_lifecycle_lists_exactly_six_read_only_tools() {
    let output = run_mcp(
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "munin-contract", "version": "1"}
                }
            }),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {}}),
        ],
        None,
    );

    assert!(
        output.status.success(),
        "server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = parse_protocol_lines(&output);
    assert_eq!(
        messages.len(),
        3,
        "expected initialize, ping, and tool list"
    );
    assert_eq!(messages[0]["id"], 1);
    assert_eq!(messages[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(messages[0]["result"]["capabilities"]["tools"], json!({}));
    assert_eq!(
        messages[1],
        json!({"jsonrpc": "2.0", "id": 2, "result": {}})
    );

    let tools = messages[2]["result"]["tools"]
        .as_array()
        .expect("tools array");
    let names = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "search",
            "session_summary",
            "project_state",
            "friction",
            "strategy_status",
            "health"
        ]
    );
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }
}

#[test]
fn malformed_json_is_a_protocol_error_not_silence_or_console_text() {
    let output = run_mcp(&[], Some("{not-json"));
    assert!(
        output.status.success(),
        "server should survive malformed input"
    );
    let messages = parse_protocol_lines(&output);
    assert_eq!(messages.len(), 1, "malformed input needs one response");
    assert_eq!(messages[0]["jsonrpc"], "2.0");
    assert!(messages[0]["id"].is_null());
    assert_eq!(messages[0]["error"]["code"], -32700);
}
