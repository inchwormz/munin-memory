use munin_memory::core::memory_os::{
    MemoryOsCheckpointCapture, MemoryOsCheckpointReentry, MemoryOsCheckpointTelemetry,
    MemoryOsPacketSelection,
};
use munin_memory::core::strategy::{self, StrategySetupOptions};
use munin_memory::tracking::Tracker;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn run_mcp(lines: &[Value], raw_tail: Option<&str>) -> Output {
    run_mcp_in_profile(lines, raw_tail, None)
}

fn run_mcp_in_profile(lines: &[Value], raw_tail: Option<&str>, profile: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_munin"));
    command
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(profile) = profile {
        command
            .env("MUNIN_DATA_DIR", profile.join("data"))
            .env("MUNIN_CONFIG_DIR", profile.join("config"))
            .env("MUNIN_SESSION_HOME", profile.join("sessions"))
            .env("MUNIN_INSTALL_HOME", profile.join("install"))
            .env("MUNIN_MEMORYOS_READ_MODEL_V1", "true");
    }
    let mut child = command.spawn().expect("start munin mcp server");

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
    let _guard = ENV_LOCK.lock().expect("environment lock");
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
        assert_eq!(
            tool["outputSchema"]["properties"]["schemaVersion"]["const"],
            "munin.mcp.result.v1"
        );
        assert_eq!(tool["outputSchema"]["additionalProperties"], false);
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }
}

#[test]
fn invalid_tool_arguments_are_visible_execution_errors() {
    let _guard = ENV_LOCK.lock().expect("environment lock");
    let profile = tempfile::tempdir().expect("profile");
    let output = run_mcp_in_profile(
        &[
            initialize_request(),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            tool_request(2, "search", json!({"query": "resolver", "limit": 0})),
        ],
        None,
        Some(profile.path()),
    );
    assert!(output.status.success());
    let messages = parse_protocol_lines(&output);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1]["result"]["isError"], true);
    assert!(messages[1]["result"]["content"][0]["text"]
        .as_str()
        .expect("error text")
        .contains("limit must be between 1 and 20"));
}

#[test]
fn malformed_json_is_a_protocol_error_not_silence_or_console_text() {
    let _guard = ENV_LOCK.lock().expect("environment lock");
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

#[test]
fn unsupported_resource_discovery_is_an_empty_surface_not_a_host_warning() {
    let _guard = ENV_LOCK.lock().expect("environment lock");
    let output = run_mcp(
        &[
            initialize_request(),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "resources/templates/list", "params": {}}),
        ],
        None,
    );
    assert!(output.status.success());
    let messages = parse_protocol_lines(&output);
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["result"], json!({"resources": []}));
    assert_eq!(messages[2]["result"], json!({"resourceTemplates": []}));
}

#[test]
fn all_six_tools_return_typed_bounded_read_only_results() {
    let _guard = ENV_LOCK.lock().expect("environment lock");
    let profile = tempfile::tempdir().expect("profile");
    seed_contract_corpus(profile.path());
    let digest_before = profile_digest(profile.path());

    let requests = vec![
        initialize_request(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        tool_request(10, "search", json!({"query": "resolver", "limit": 1})),
        tool_request(
            11,
            "session_summary",
            json!({"sessionId": "session-123", "source": "codex", "projectPath": "C:/repo"}),
        ),
        tool_request(
            12,
            "project_state",
            json!({"projectPath": "C:/repo", "limit": 10}),
        ),
        tool_request(
            13,
            "friction",
            json!({"scope": "project", "projectPath": "C:/repo", "limit": 10}),
        ),
        tool_request(14, "strategy_status", json!({"scope": "phase2"})),
        tool_request(
            15,
            "health",
            json!({"scope": "project", "projectPath": "C:/repo"}),
        ),
    ];
    let output = run_mcp_in_profile(&requests, None, Some(profile.path()));
    assert!(
        output.status.success(),
        "server failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let messages = parse_protocol_lines(&output);
    assert_eq!(messages.len(), 7, "initialize plus six tool responses");

    let expected_tools = [
        "search",
        "session_summary",
        "project_state",
        "friction",
        "strategy_status",
        "health",
    ];
    for (message, expected_tool) in messages.iter().skip(1).zip(expected_tools) {
        let result = &message["result"];
        assert_eq!(result["isError"], false, "{expected_tool} failed: {result}");
        let structured = &result["structuredContent"];
        assert_eq!(structured["schemaVersion"], "munin.mcp.result.v1");
        assert_eq!(structured["tool"], expected_tool);
        assert!(structured["generatedAt"].is_string());
        assert!(structured["provenance"].is_array());
        assert!(structured["freshness"]["status"].is_string());
        assert!(structured["sensitivity"]["classification"].is_string());
        assert!(structured["sensitivity"]["scanCompleted"].is_boolean());
        assert!(structured["warnings"].is_array());
        assert!(structured["receipt"]["found"].is_u64());
        assert!(structured["receipt"]["returned"].is_u64());
        assert!(structured["receipt"]["dropped"].is_u64());
        assert!(structured["data"].is_object());
        let text = result["content"][0]["text"].as_str().expect("text content");
        let text_value: Value = serde_json::from_str(text).expect("text mirrors JSON");
        assert_eq!(&text_value, structured);
    }

    let search_receipt = &messages[1]["result"]["structuredContent"]["receipt"];
    assert!(search_receipt["found"].as_u64().unwrap() >= 2);
    assert_eq!(search_receipt["returned"], 1);
    assert_eq!(
        search_receipt["dropped"].as_u64().unwrap(),
        search_receipt["found"].as_u64().unwrap() - 1
    );
    assert_eq!(
        messages[2]["result"]["structuredContent"]["data"]["summary"]["sessionId"], "session-123",
        "session response: {}",
        messages[2]
    );
    let health = &messages[6]["result"]["structuredContent"]["data"]["health"];
    assert_eq!(health["status"], "partial");
    assert!(health["checkpointCount"].as_u64().unwrap() > 0);
    assert!(health.get("signalQuality").is_none());

    let digest_after = profile_digest(profile.path());
    assert_eq!(digest_after, digest_before, "MCP reads mutated the corpus");
}

fn initialize_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {"name": "munin-contract", "version": "1"}
        }
    })
}

fn tool_request(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    })
}

fn seed_contract_corpus(profile: &Path) {
    let data_dir = profile.join("data");
    let config_dir = profile.join("config");
    fs::create_dir_all(&data_dir).expect("data dir");
    fs::create_dir_all(&config_dir).expect("config dir");
    std::env::set_var("MUNIN_DATA_DIR", &data_dir);
    std::env::set_var("MUNIN_CONFIG_DIR", &config_dir);
    std::env::set_var("MUNIN_MEMORYOS_READ_MODEL_V1", "true");

    let tracker = Tracker::new().expect("tracker");
    let generated_at = chrono::Utc::now().to_rfc3339();
    let capture = MemoryOsCheckpointCapture {
        packet_id: "phase2-contract-packet".to_string(),
        generated_at: generated_at.clone(),
        preset: "continue".to_string(),
        intent: "continue".to_string(),
        profile: "session-onboarding".to_string(),
        goal: Some("Ship the typed resolver memory boundary".to_string()),
        budget: 1600,
        estimated_tokens: 240,
        estimated_source_tokens: 480,
        pager_manifest_hash: "phase2-contract-hash".to_string(),
        recall_mode: "off".to_string(),
        recall_used: false,
        recall_reason: "contract seed".to_string(),
        telemetry: MemoryOsCheckpointTelemetry {
            current_fact_count: 2,
            recent_change_count: 1,
            live_claim_count: 1,
            open_obligation_count: 1,
            artifact_handle_count: 0,
            failure_count: 0,
        },
        selected_items: vec![
            MemoryOsPacketSelection {
                section: "current_facts".to_string(),
                kind: "decision".to_string(),
                summary: "Resolver uses compiled memory rather than raw transcripts.".to_string(),
                token_estimate: 12,
                score: 1500,
                artifact_id: None,
                subject: Some("resolver".to_string()),
                provenance: vec!["contract:resolver-decision".to_string()],
            },
            MemoryOsPacketSelection {
                section: "open_obligations".to_string(),
                kind: "open-loop".to_string(),
                summary: "Verify the resolver through both native host declarations.".to_string(),
                token_estimate: 12,
                score: 1400,
                artifact_id: None,
                subject: Some("resolver-host-proof".to_string()),
                provenance: vec!["contract:resolver-host-proof".to_string()],
            },
            MemoryOsPacketSelection {
                section: "session_summary".to_string(),
                kind: "session-summary".to_string(),
                summary: [
                    "- Session summary: codex session `session-123` for `C:/repo` started 2026-07-14T00:00:00Z.",
                    "- User asked first: expose typed resolver memory to both coding agents.",
                    "- Latest user ask: prove the same resolver contract through both hosts.",
                    "- Work performed: 4 command(s) captured, 3 succeeded, 1 failed, 0 had unknown outcome.",
                    "- Important handoff: verify resolver output before enabling the release gate.",
                ]
                .join("\n"),
                token_estimate: 80,
                score: 1800,
                artifact_id: Some("session-summary:codex:session-123".to_string()),
                subject: Some("session-summary:codex:session-123".to_string()),
                provenance: vec![
                    "session:codex".to_string(),
                    "session-id:session-123".to_string(),
                ],
            },
        ],
        exclusions: vec![],
        reentry: MemoryOsCheckpointReentry {
            recommended_command: "cargo test --test mcp_contract".to_string(),
            current_recommendation: Some("Run the resolver host contract".to_string()),
            first_question: "Does the resolver preserve provenance?".to_string(),
            first_verification: "cargo test --test mcp_contract".to_string(),
        },
    };
    tracker
        .record_memory_os_packet_checkpoint_for_project("C:/repo", &capture)
        .expect("checkpoint");
    drop(tracker);

    strategy::setup(&StrategySetupOptions {
        scope: "phase2".to_string(),
        import_path: None,
        bootstrap_claude: false,
        template: true,
    })
    .expect("strategy scope");

    std::env::remove_var("MUNIN_MEMORYOS_READ_MODEL_V1");
    std::env::remove_var("MUNIN_CONFIG_DIR");
    std::env::remove_var("MUNIN_DATA_DIR");
}

fn profile_digest(root: &Path) -> String {
    let mut paths = Vec::<PathBuf>::new();
    collect_files(root, &mut paths);
    paths.sort();
    let mut hasher = Sha256::new();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.ends_with("-wal") || name.ends_with("-shm") {
            continue;
        }
        hasher.update(
            path.strip_prefix(root)
                .expect("relative")
                .to_string_lossy()
                .as_bytes(),
        );
        hasher.update(fs::read(&path).expect("digest file"));
    }
    format!("{:x}", hasher.finalize())
}

fn collect_files(root: &Path, paths: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }
    for entry in fs::read_dir(root).expect("read profile") {
        let entry = entry.expect("profile entry");
        if entry.file_type().expect("entry type").is_dir() {
            collect_files(&entry.path(), paths);
        } else {
            paths.push(entry.path());
        }
    }
}
