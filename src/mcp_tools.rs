//! Typed, read-only adapters between Munin read models and MCP tool results.

use crate::analytics::memory_os_cmd;
use crate::core::memory_os::MemoryOsInspectionScope;
use crate::core::strategy::{self, StrategyReadOptions};
use crate::core::tracking::{scan_memory_os_trust_payload, MemoryOsTrustFindingKind, Tracker};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;
const STRATEGY_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpResultEnvelope {
    schema_version: &'static str,
    tool: &'static str,
    generated_at: String,
    provenance: Vec<McpProvenance>,
    freshness: McpFreshness,
    sensitivity: McpSensitivity,
    warnings: Vec<String>,
    receipt: McpReceipt,
    data: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpProvenance {
    source_kind: String,
    source_ref: String,
    observed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpFreshness {
    status: String,
    source_updated_at: Option<String>,
    age_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpSensitivity {
    classification: String,
    scan_completed: bool,
    contains_secret: bool,
    contains_pii: bool,
    must_not_packetize: bool,
    finding_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpReceipt {
    found: usize,
    returned: usize,
    dropped: usize,
    limit: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SearchInput {
    query: String,
    scope: Option<String>,
    project_path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionSummaryInput {
    session_id: String,
    source: Option<String>,
    project_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectStateInput {
    project_path: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrictionInput {
    scope: Option<String>,
    project_path: Option<String>,
    limit: Option<usize>,
    #[serde(default)]
    include_resolved: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StrategyStatusInput {
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HealthInput {
    scope: Option<String>,
    project_path: Option<String>,
}

pub fn call_tool(name: &str, arguments: &Value) -> Result<Value> {
    match name {
        "search" => search(parse(arguments)?),
        "session_summary" => session_summary(parse(arguments)?),
        "project_state" => project_state(parse(arguments)?),
        "friction" => friction(parse(arguments)?),
        "strategy_status" => strategy_status(parse(arguments)?),
        "health" => health(parse(arguments)?),
        _ => Err(anyhow!("unknown Munin MCP tool `{name}`")),
    }
}

fn search(input: SearchInput) -> Result<Value> {
    let query = input.query.trim();
    if query.is_empty() {
        return Err(anyhow!("query must not be empty"));
    }
    let limit = validate_limit(input.limit)?;
    let scope = parse_scope(input.scope.as_deref(), input.project_path.as_deref())?;
    let tracker = Tracker::new().context("failed to open Munin history")?;
    let report =
        tracker.get_memory_os_recall_report(scope, input.project_path.as_deref(), query)?;
    let found = report.matches.len();
    let matches = report.matches.into_iter().take(limit).collect::<Vec<_>>();
    let provenance = matches
        .iter()
        .map(|item| McpProvenance {
            source_kind: item.source_kind.clone(),
            source_ref: item.source_ref.clone(),
            observed_at: None,
        })
        .collect();
    let mut warnings = Vec::new();
    if let Some(reason) = report.no_match_reason {
        warnings.push(reason);
    }
    envelope(
        "search",
        provenance,
        warnings,
        found,
        matches.len(),
        limit,
        json!({"query": report.query, "scope": scope, "matches": matches}),
    )
}

fn session_summary(input: SessionSummaryInput) -> Result<Value> {
    let session_id = input.session_id.trim();
    if session_id.is_empty() {
        return Err(anyhow!("sessionId must not be empty"));
    }
    if let Some(source) = input.source.as_deref() {
        if source != "codex" && source != "claude" {
            return Err(anyhow!("source must be `codex` or `claude`"));
        }
    }
    let tracker = Tracker::new().context("failed to open Munin history")?;
    let matches = tracker.get_memory_os_session_summary_matches(
        input.project_path.as_deref(),
        session_id,
        input.source.as_deref(),
    )?;
    let found = matches.len();
    let selected = matches.into_iter().next();
    let provenance = selected
        .as_ref()
        .map(|item| McpProvenance {
            source_kind: "session-summary".to_string(),
            source_ref: item.source_ref.clone(),
            observed_at: Some(item.captured_at.clone()),
        })
        .into_iter()
        .collect();
    let mut warnings = Vec::new();
    if selected.is_none() {
        warnings.push("No compiled summary matched the exact session identity.".to_string());
    }
    let summary = selected.map(|item| item.summary);
    envelope(
        "session_summary",
        provenance,
        warnings,
        found,
        usize::from(summary.is_some()),
        1,
        json!({"summary": summary}),
    )
}

fn project_state(input: ProjectStateInput) -> Result<Value> {
    let limit = validate_limit(input.limit)?;
    let tracker = Tracker::new().context("failed to open Munin history")?;
    let kernel = tracker.get_memory_os_project_kernel(input.project_path.as_deref())?;
    let mut claims = Vec::new();
    for claim in &kernel.claims {
        claims.push((
            json!({"kind": "claim", "value": claim}),
            McpProvenance {
                source_kind: claim.source_kind.clone(),
                source_ref: claim.source_event_id.clone(),
                observed_at: Some(claim.observed_at.clone()),
            },
        ));
    }
    let mut open_loops = Vec::new();
    for open_loop in &kernel.open_loops {
        open_loops.push((
            json!({"kind": "openLoop", "value": open_loop}),
            McpProvenance {
                source_kind: "open-loop".to_string(),
                source_ref: open_loop.open_loop_id.clone(),
                observed_at: Some(open_loop.last_seen_at.clone()),
            },
        ));
    }
    let mut checkpoints = Vec::new();
    for checkpoint in &kernel.checkpoints {
        checkpoints.push((
            json!({"kind": "checkpoint", "value": checkpoint}),
            McpProvenance {
                source_kind: "checkpoint".to_string(),
                source_ref: checkpoint.source_event_id.clone(),
                observed_at: Some(checkpoint.captured_at.clone()),
            },
        ));
    }
    let mut ranked = prioritize_project_state(open_loops, claims, checkpoints);
    let found = ranked.len();
    ranked.truncate(limit);
    let (items, provenance): (Vec<_>, Vec<_>) = ranked.into_iter().unzip();
    envelope(
        "project_state",
        provenance,
        Vec::new(),
        found,
        items.len(),
        limit,
        json!({
            "projectPath": kernel.project_path,
            "lastJournalSeq": kernel.last_journal_seq,
            "items": items
        }),
    )
}

fn prioritize_project_state(
    mut open_loops: Vec<(Value, McpProvenance)>,
    mut claims: Vec<(Value, McpProvenance)>,
    mut checkpoints: Vec<(Value, McpProvenance)>,
) -> Vec<(Value, McpProvenance)> {
    // Give the first page one action, one decision, and one checkpoint when
    // available. A long claim history must not hide the current blocker.
    let mut ranked = Vec::new();
    for category in [&mut open_loops, &mut claims, &mut checkpoints] {
        if !category.is_empty() {
            ranked.push(category.remove(0));
        }
    }
    ranked.extend(open_loops);
    ranked.extend(claims);
    ranked.extend(checkpoints);
    ranked
}

fn friction(input: FrictionInput) -> Result<Value> {
    let limit = validate_limit(input.limit)?;
    let scope = parse_scope(input.scope.as_deref(), input.project_path.as_deref())?;
    let tracker = Tracker::new().context("failed to open Munin history")?;
    let report = tracker.get_memory_os_friction_report(scope, input.project_path.as_deref())?;
    let mut fixes = report
        .top_fixes
        .into_iter()
        .chain(report.new_unproven_friction)
        .filter(|fix| input.include_resolved || !is_resolved_status(&fix.status))
        .collect::<Vec<_>>();
    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.score));
    let found = fixes.len();
    fixes.truncate(limit);
    let provenance = fixes
        .iter()
        .map(|fix| McpProvenance {
            source_kind: "friction-fix".to_string(),
            source_ref: fix.fix_id.clone(),
            observed_at: fix.last_signal_at.clone(),
        })
        .collect();
    envelope(
        "friction",
        provenance,
        Vec::new(),
        found,
        fixes.len(),
        limit,
        json!({"scope": scope, "fixes": fixes}),
    )
}

fn strategy_status(input: StrategyStatusInput) -> Result<Value> {
    let scope = input
        .scope
        .filter(|scope| !scope.trim().is_empty())
        .unwrap_or_else(strategy::default_strategy_scope_hint);
    let mut report = strategy::status_read_only(&StrategyReadOptions {
        scope: scope.clone(),
    })?;
    let found = report.items.len();
    report.items.truncate(STRATEGY_LIMIT);
    let provenance = vec![McpProvenance {
        source_kind: "strategy-registry".to_string(),
        source_ref: report.registry.artifact_path.display().to_string(),
        observed_at: Some(report.registry.imported_at.clone()),
    }];
    envelope(
        "strategy_status",
        provenance,
        report.warnings.clone(),
        found,
        report.items.len(),
        STRATEGY_LIMIT,
        json!({"status": report}),
    )
}

fn health(input: HealthInput) -> Result<Value> {
    let scope = parse_scope(input.scope.as_deref(), input.project_path.as_deref())?;
    let tracker = Tracker::new().context("failed to open Munin history")?;
    let report =
        memory_os_cmd::build_doctor_report(&tracker, scope, input.project_path.as_deref())?;
    let found = report.signal_quality.len() + report.top_pipeline_problems.len();
    let provenance = vec![McpProvenance {
        source_kind: "memory-os-onboarding".to_string(),
        source_ref: "compiled-onboarding-state".to_string(),
        observed_at: report.corpus.latest_import_completed_at.clone(),
    }];
    let mut warnings = Vec::new();
    if report.overall_status != "pass" {
        warnings.push(format!("Memory OS health is {}.", report.overall_status));
    }
    envelope(
        "health",
        provenance,
        warnings,
        found,
        found,
        found,
        json!({"health": report}),
    )
}

fn envelope(
    tool: &'static str,
    provenance: Vec<McpProvenance>,
    mut warnings: Vec<String>,
    found: usize,
    returned: usize,
    limit: usize,
    mut data: Value,
) -> Result<Value> {
    camelize_object_keys(&mut data);
    let freshness = freshness(&provenance);
    if freshness.status == "unknown" {
        warnings.push("Source observation time is unavailable; freshness is unknown.".to_string());
    }

    let serialized = serde_json::to_string(&data)?;
    let scan = scan_memory_os_trust_payload(&serialized);
    let secret_excerpts = scan
        .findings
        .iter()
        .filter(|finding| finding.kind == MemoryOsTrustFindingKind::Secret)
        .map(|finding| finding.match_excerpt.clone())
        .collect::<Vec<_>>();
    if !secret_excerpts.is_empty() {
        redact_strings(&mut data, &secret_excerpts);
        warnings.push(format!(
            "{} secret-shaped value(s) were redacted from tool output.",
            secret_excerpts.len()
        ));
    }
    warnings.sort();
    warnings.dedup();
    let classification = if scan.contains_secret || scan.must_not_packetize {
        "restricted"
    } else if scan.contains_pii {
        "personal"
    } else {
        "local-private"
    };

    Ok(serde_json::to_value(McpResultEnvelope {
        schema_version: "munin.mcp.result.v1",
        tool,
        generated_at: Utc::now().to_rfc3339(),
        provenance,
        freshness,
        sensitivity: McpSensitivity {
            classification: classification.to_string(),
            scan_completed: true,
            contains_secret: scan.contains_secret,
            contains_pii: scan.contains_pii,
            must_not_packetize: scan.must_not_packetize,
            finding_count: scan.findings.len(),
        },
        warnings,
        receipt: McpReceipt {
            found,
            returned,
            dropped: found.saturating_sub(returned),
            limit,
        },
        data,
    })?)
}

fn camelize_object_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let old = std::mem::take(map);
            for (key, mut child) in old {
                camelize_object_keys(&mut child);
                map.insert(snake_to_camel(&key), child);
            }
        }
        Value::Array(items) => {
            for item in items {
                camelize_object_keys(item);
            }
        }
        _ => {}
    }
}

fn snake_to_camel(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut uppercase = false;
    for ch in value.chars() {
        if ch == '_' {
            uppercase = true;
        } else if uppercase {
            output.extend(ch.to_uppercase());
            uppercase = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn freshness(provenance: &[McpProvenance]) -> McpFreshness {
    let latest = provenance
        .iter()
        .filter_map(|item| item.observed_at.as_deref())
        .filter_map(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .max();
    let Some(latest) = latest else {
        return McpFreshness {
            status: "unknown".to_string(),
            source_updated_at: None,
            age_seconds: None,
        };
    };
    let age = Utc::now()
        .signed_duration_since(latest)
        .num_seconds()
        .max(0);
    McpFreshness {
        status: if age <= 7 * 24 * 60 * 60 {
            "fresh"
        } else {
            "stale"
        }
        .to_string(),
        source_updated_at: Some(latest.to_rfc3339()),
        age_seconds: Some(age),
    }
}

fn parse<T: DeserializeOwned>(arguments: &Value) -> Result<T> {
    serde_json::from_value(arguments.clone()).map_err(|error| anyhow!("invalid arguments: {error}"))
}

fn parse_scope(value: Option<&str>, project_path: Option<&str>) -> Result<MemoryOsInspectionScope> {
    match value.unwrap_or(if project_path.is_some() {
        "project"
    } else {
        "user"
    }) {
        "user" => Ok(MemoryOsInspectionScope::User),
        "project" => Ok(MemoryOsInspectionScope::Project),
        other => Err(anyhow!(
            "scope must be `user` or `project`, received `{other}`"
        )),
    }
}

fn validate_limit(value: Option<usize>) -> Result<usize> {
    let limit = value.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(anyhow!("limit must be between 1 and {MAX_LIMIT}"));
    }
    Ok(limit)
}

fn is_resolved_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "fixed" | "retired" | "resolved" | "completed"
    )
}

fn redact_strings(value: &mut Value, excerpts: &[String]) {
    match value {
        Value::String(text) => {
            for excerpt in excerpts {
                *text = text.replace(excerpt, "[REDACTED]");
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_strings(item, excerpts);
            }
        }
        Value::Object(map) => {
            for value in map.values_mut() {
                redact_strings(value, excerpts);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_blocker_survives_a_page_of_historical_claims() {
        let item = |kind: &str, id: usize| {
            (
                json!({"kind": kind, "id": id}),
                McpProvenance {
                    source_kind: kind.to_string(),
                    source_ref: format!("{kind}:{id}"),
                    observed_at: None,
                },
            )
        };
        let claims = (0..7).map(|id| item("claim", id)).collect();
        let ranked = prioritize_project_state(
            vec![item("openLoop", 1)],
            claims,
            vec![item("checkpoint", 1)],
        );
        let first_page = ranked
            .iter()
            .take(5)
            .map(|entry| &entry.0)
            .collect::<Vec<_>>();
        assert!(first_page.iter().any(|value| value["kind"] == "openLoop"));
        assert!(first_page.iter().any(|value| value["kind"] == "claim"));
        assert!(first_page.iter().any(|value| value["kind"] == "checkpoint"));
    }

    #[test]
    fn secret_values_are_redacted_and_labeled() {
        let output = envelope(
            "search",
            Vec::new(),
            Vec::new(),
            1,
            1,
            1,
            json!({"value": "api_key=abcdefghijklmnopqrstuv"}),
        )
        .expect("envelope");
        assert_eq!(output["sensitivity"]["classification"], "restricted");
        assert_eq!(output["sensitivity"]["containsSecret"], true);
        assert_eq!(output["data"]["value"], "[REDACTED]");
    }
}
