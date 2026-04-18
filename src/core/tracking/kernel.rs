use anyhow::Result;
use rusqlite::params;
use std::collections::HashMap;

use super::{
    hash_text, journal::MemoryOsJournalEventRow, project_filter_params, project_repo_hint,
    resolved_project_path, Tracker,
};

#[derive(Debug, Clone)]
pub(super) struct MemoryOsOpenLoopIdentity {
    subject: Option<String>,
    section: String,
    source_event_id: String,
    packet_id: Option<String>,
    artifact_id: Option<String>,
    provenance: Vec<String>,
}

impl MemoryOsOpenLoopIdentity {
    pub(super) fn from_manual_claim(source_event_id: &str) -> Self {
        Self {
            subject: None,
            section: "validated_claim_leases".to_string(),
            source_event_id: source_event_id.to_string(),
            packet_id: None,
            artifact_id: None,
            provenance: vec!["claim-lease".to_string()],
        }
    }

    pub(super) fn from_packet_item(
        source_event_id: &str,
        packet_id: &str,
        item: &crate::core::memory_os::MemoryOsPacketSelection,
    ) -> Self {
        Self {
            subject: item.subject.clone(),
            section: item.section.clone(),
            source_event_id: source_event_id.to_string(),
            packet_id: Some(packet_id.to_string()),
            artifact_id: item.artifact_id.clone(),
            provenance: item.provenance.clone(),
        }
    }

    fn key(&self, summary: &str) -> String {
        if let Some(subject) = self
            .subject
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("subject:{}", subject.to_ascii_lowercase());
        }

        let packet_or_event = self.packet_id.as_deref().unwrap_or(&self.source_event_id);
        if !self.section.trim().is_empty() && !packet_or_event.trim().is_empty() {
            let mut parts = vec![
                "checkpoint".to_string(),
                self.section.trim().to_ascii_lowercase(),
                packet_or_event.trim().to_ascii_lowercase(),
            ];
            if let Some(artifact_id) = self
                .artifact_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                parts.push(format!("artifact:{}", artifact_id.to_ascii_lowercase()));
            }
            if !self.provenance.is_empty() {
                parts.push(format!(
                    "provenance:{}",
                    self.provenance
                        .iter()
                        .map(|value| value.trim().to_ascii_lowercase())
                        .filter(|value| !value.is_empty())
                        .collect::<Vec<_>>()
                        .join("|")
                ));
            }
            return parts.join(":");
        }

        format!("summary:{}", summary.trim().to_ascii_lowercase())
    }
}

impl Tracker {
    pub fn get_memory_os_project_snapshot(
        &self,
        project_path: Option<&str>,
    ) -> Result<crate::core::memory_os::MemoryOsProjectSnapshot> {
        let resolved_project_path = resolved_project_path(project_path);
        let (project_exact, project_glob) = project_filter_params(Some(&resolved_project_path));
        let repo_hint = project_repo_hint(&resolved_project_path);
        let journal_event_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_os_journal_events
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;
        let last_journal_seq: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(journal_seq) FROM memory_os_journal_events
                 WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
                params![project_exact, project_glob],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        let verification_result_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_os_verification_results
             WHERE (?1 IS NULL
                    OR json_extract(scope_json, '$.project_path') = ?1
                    OR json_extract(scope_json, '$.project_path') GLOB ?2
                    OR json_extract(scope_json, '$.repo_id') = ?3)",
            params![project_exact, project_glob, repo_hint],
            |row| row.get(0),
        )?;
        let trust_observation_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM memory_os_trust_observations
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT projection_name, from_seq, to_seq, rebuild_kind, created_at
             FROM memory_os_projection_checkpoints
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY created_at DESC
             LIMIT 10",
        )?;
        let projection_checkpoints = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(crate::core::memory_os::ProjectionCheckpointRef {
                    projection_name: row.get(0)?,
                    from_seq: row.get(1)?,
                    to_seq: row.get(2)?,
                    rebuild_kind: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(crate::core::memory_os::MemoryOsProjectSnapshot {
            project_path: resolved_project_path,
            journal_event_count,
            last_journal_seq,
            verification_result_count,
            trust_observation_count,
            projection_checkpoints,
        })
    }

    pub fn get_memory_os_project_kernel(
        &self,
        project_path: Option<&str>,
    ) -> Result<crate::core::memory_os::MemoryOsProjectKernel> {
        let resolved_project_path = resolved_project_path(project_path);
        let (project_exact, project_glob) = project_filter_params(Some(&resolved_project_path));
        let mut stmt = self.conn.prepare(
            "SELECT journal_seq, event_id, event_kind, committed_at, payload_json
             FROM memory_os_journal_events
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY journal_seq ASC",
        )?;
        let rows = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(MemoryOsJournalEventRow {
                    journal_seq: row.get(0)?,
                    event_id: row.get(1)?,
                    event_kind: row.get(2)?,
                    committed_at: row.get(3)?,
                    payload_json: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut claims = Vec::new();
        let mut open_loops = Vec::new();
        let mut open_loop_index = HashMap::new();
        let mut checkpoints = Vec::new();
        let mut last_journal_seq = None;

        for row in rows {
            last_journal_seq = Some(row.journal_seq);
            if row.event_kind == "legacy.claim-lease-created" {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&row.payload_json) {
                    let claim_type = payload
                        .get("claim_type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("decision");
                    let claim_text = payload
                        .get("claim_text")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string();
                    let confidence = payload
                        .get("confidence")
                        .and_then(|value| value.as_str())
                        .unwrap_or("medium")
                        .to_string();
                    let scope_key = payload
                        .get("scope_key")
                        .and_then(|value| value.as_str())
                        .map(|value| value.to_string());
                    let source_kind = payload
                        .get("source_kind")
                        .and_then(|value| value.as_str())
                        .unwrap_or("legacy-context")
                        .to_string();

                    if claim_type == "obligation" {
                        upsert_memory_os_open_loop(
                            &mut open_loops,
                            &mut open_loop_index,
                            claim_text,
                            "follow_up",
                            "open",
                            severity_from_confidence(&confidence),
                            MemoryOsOpenLoopIdentity::from_manual_claim(&row.event_id),
                            &row.event_id,
                            &row.committed_at,
                        );
                    } else {
                        claims.push(crate::core::memory_os::MemoryOsClaimProjection {
                            claim_id: format!("claim:{}", row.event_id),
                            source_event_id: row.event_id.clone(),
                            journal_seq: row.journal_seq,
                            observed_at: row.committed_at.clone(),
                            claim_kind: claim_type.to_string(),
                            claim_text,
                            confidence,
                            scope_key,
                            source_kind,
                        });
                    }
                }
                continue;
            }

            if row.event_kind.starts_with("legacy.packet-checkpoint.") {
                let capture = match serde_json::from_str::<
                    crate::core::memory_os::MemoryOsCheckpointCapture,
                >(&row.payload_json)
                {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                let open_loop_summaries = capture
                    .selected_items
                    .iter()
                    .filter(|item| item.section == "open_obligations")
                    .map(|item| item.summary.clone())
                    .collect::<Vec<_>>();
                let active_risks = capture
                    .selected_items
                    .iter()
                    .filter(|item| item.section == "current_failures")
                    .map(|item| item.summary.clone())
                    .collect::<Vec<_>>();
                let live_claim_summaries = capture
                    .selected_items
                    .iter()
                    .filter(|item| item.section == "validated_claim_leases")
                    .map(|item| item.summary.clone())
                    .collect::<Vec<_>>();

                for item in &capture.selected_items {
                    match item.section.as_str() {
                        "open_obligations" => upsert_memory_os_open_loop(
                            &mut open_loops,
                            &mut open_loop_index,
                            item.summary.clone(),
                            "follow_up",
                            "open",
                            "medium",
                            MemoryOsOpenLoopIdentity::from_packet_item(
                                &row.event_id,
                                &capture.packet_id,
                                item,
                            ),
                            &row.event_id,
                            &capture.generated_at,
                        ),
                        "current_failures" => upsert_memory_os_open_loop(
                            &mut open_loops,
                            &mut open_loop_index,
                            item.summary.clone(),
                            "bug",
                            "blocked",
                            "high",
                            MemoryOsOpenLoopIdentity::from_packet_item(
                                &row.event_id,
                                &capture.packet_id,
                                item,
                            ),
                            &row.event_id,
                            &capture.generated_at,
                        ),
                        _ => {}
                    }
                }

                checkpoints.push(crate::core::memory_os::MemoryOsCheckpointProjection {
                    checkpoint_id: format!("checkpoint:{}", capture.packet_id),
                    source_event_id: row.event_id.clone(),
                    journal_seq: row.journal_seq,
                    captured_at: capture.generated_at.clone(),
                    preset: capture.preset.clone(),
                    intent: capture.intent.clone(),
                    goal: capture.goal.clone(),
                    current_recommendation: capture.reentry.current_recommendation.clone(),
                    active_risks,
                    open_loop_summaries,
                    live_claim_summaries,
                    pager_manifest_hash: capture.pager_manifest_hash.clone(),
                    reentry: capture.reentry.clone(),
                });
            }
        }

        claims.sort_by(|left, right| right.observed_at.cmp(&left.observed_at));
        open_loops.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
        checkpoints.sort_by(|left, right| right.captured_at.cmp(&left.captured_at));

        Ok(crate::core::memory_os::MemoryOsProjectKernel {
            project_path: resolved_project_path,
            last_journal_seq,
            claims,
            open_loops,
            checkpoints,
        })
    }
}

fn severity_rank(value: &str) -> usize {
    match value {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    }
}

fn severity_from_confidence(confidence: &str) -> &'static str {
    match confidence {
        "high" => "high",
        "low" => "low",
        _ => "medium",
    }
}

pub(super) fn upsert_memory_os_open_loop(
    open_loops: &mut Vec<crate::core::memory_os::MemoryOsOpenLoopProjection>,
    index: &mut HashMap<String, usize>,
    summary: String,
    loop_kind: &str,
    status: &str,
    severity: &str,
    identity: MemoryOsOpenLoopIdentity,
    source_event_id: &str,
    seen_at: &str,
) {
    let key = identity.key(&summary);
    if let Some(position) = index.get(&key).copied() {
        let existing = &mut open_loops[position];
        if !existing
            .source_event_ids
            .iter()
            .any(|value| value == source_event_id)
        {
            existing.source_event_ids.push(source_event_id.to_string());
        }
        if severity_rank(severity) > severity_rank(&existing.severity) {
            existing.severity = severity.to_string();
        }
        if status == "blocked" {
            existing.status = status.to_string();
        }
        if seen_at > existing.last_seen_at.as_str() {
            existing.last_seen_at = seen_at.to_string();
        }
        return;
    }

    let position = open_loops.len();
    open_loops.push(crate::core::memory_os::MemoryOsOpenLoopProjection {
        open_loop_id: format!("open-loop-{}", hash_text(&key)),
        summary,
        loop_kind: loop_kind.to_string(),
        status: status.to_string(),
        severity: severity.to_string(),
        source_event_ids: vec![source_event_id.to_string()],
        last_seen_at: seen_at.to_string(),
    });
    index.insert(key, position);
}
