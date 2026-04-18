use anyhow::Result;
use rusqlite::params;

use super::claim_leases::normalize_claim_text;
use super::{
    compact_display_text, memory_os_repo_label, memory_os_scope_params, resolved_project_path,
    ClaimLeaseConfidence, ClaimLeaseStatus, ClaimLeaseType, Tracker,
};

impl Tracker {
    pub fn get_memory_os_evidence_events(
        &self,
        scope: crate::core::memory_os::MemoryOsInspectionScope,
        project_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<crate::core::memory_os::MemoryOsEvidenceEventRecord>> {
        let (project_exact, project_glob, _) = memory_os_scope_params(scope, project_path);
        let limit = limit.max(1) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT event_id, event_kind, committed_at, project_path, payload_json, proof_refs_json
             FROM memory_os_journal_events
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
               AND (event_kind = 'legacy.claim-lease-created'
                    OR event_kind LIKE 'legacy.packet-checkpoint.%')
             ORDER BY journal_seq DESC
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![project_exact, project_glob, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut events = Vec::new();
        for (event_id, event_kind, committed_at, project_path, payload_json, proof_refs_json) in
            rows
        {
            let payload: serde_json::Value =
                serde_json::from_str(&payload_json).unwrap_or(serde_json::Value::Null);
            let (lane, derivation_kind, root_source_id, summary) =
                if event_kind == "legacy.claim-lease-created" {
                    let claim_text = payload
                        .get("claim_text")
                        .and_then(|value| value.as_str())
                        .unwrap_or("legacy claim lease")
                        .to_string();
                    (
                        "claim-lease".to_string(),
                        "distilled".to_string(),
                        event_id.clone(),
                        claim_text,
                    )
                } else if event_kind.starts_with("legacy.packet-checkpoint.") {
                    if let Ok(capture) = serde_json::from_value::<
                        crate::core::memory_os::MemoryOsCheckpointCapture,
                    >(payload.clone())
                    {
                        let root = capture.packet_id.clone();
                        let summary = capture
                            .reentry
                            .current_recommendation
                            .clone()
                            .or_else(|| {
                                capture
                                    .selected_items
                                    .first()
                                    .map(|item| item.summary.clone())
                            })
                            .unwrap_or_else(|| capture.intent.clone());
                        (
                            "checkpoint".to_string(),
                            "checkpointed".to_string(),
                            root,
                            summary,
                        )
                    } else {
                        (
                            "checkpoint".to_string(),
                            "checkpointed".to_string(),
                            event_id.clone(),
                            compact_display_text(&payload_json, 120),
                        )
                    }
                } else {
                    (
                        "journal".to_string(),
                        "raw".to_string(),
                        event_id.clone(),
                        compact_display_text(&payload_json, 120),
                    )
                };

            let supporting_refs =
                serde_json::from_str::<Vec<String>>(&proof_refs_json).unwrap_or_default();
            events.push(crate::core::memory_os::MemoryOsEvidenceEventRecord {
                evidence_id: format!("evidence:{event_id}"),
                lane,
                source_record_id: event_id.clone(),
                root_source_id,
                derivation_kind,
                project_path: project_path.clone(),
                event_kind,
                timestamp: committed_at,
                summary,
                scope_hints: vec![memory_os_repo_label(&project_path)],
                supporting_refs,
            });
        }
        Ok(events)
    }

    pub fn get_memory_os_promoted_assertions(
        &self,
        scope: crate::core::memory_os::MemoryOsInspectionScope,
        project_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<crate::core::memory_os::MemoryOsPromotedAssertionRecord>> {
        let statuses = [
            ClaimLeaseStatus::Live,
            ClaimLeaseStatus::Stale,
            ClaimLeaseStatus::Superseded,
        ];
        let records = match scope {
            crate::core::memory_os::MemoryOsInspectionScope::User => {
                self.get_claim_leases_filtered(limit.max(1), None, Some(&statuses))?
            }
            crate::core::memory_os::MemoryOsInspectionScope::Project => {
                let resolved = resolved_project_path(project_path);
                self.get_claim_leases_filtered(limit.max(1), Some(&resolved), Some(&statuses))?
            }
        };

        Ok(records
            .into_iter()
            .map(|record| {
                let (scope_label, scope_target) = if let Some(scope_key) = &record.scope_key {
                    ("workflow".to_string(), Some(scope_key.clone()))
                } else {
                    ("project".to_string(), Some(record.project_path.clone()))
                };
                let status = match record.status {
                    ClaimLeaseStatus::Live => "active",
                    ClaimLeaseStatus::Superseded => "superseded",
                    ClaimLeaseStatus::Stale => "contested",
                }
                .to_string();
                let stability = match record.claim_type {
                    ClaimLeaseType::Obligation => "active_state",
                    _ => "durable_preference",
                }
                .to_string();
                let category = match record.claim_type {
                    ClaimLeaseType::Decision => "decision",
                    ClaimLeaseType::Rejection => "rejection",
                    ClaimLeaseType::HypothesisTested => "hypothesis-tested",
                    ClaimLeaseType::Obligation => "obligation",
                    ClaimLeaseType::BenignAnomaly => "benign-anomaly",
                }
                .to_string();
                let confidence = match record.confidence {
                    ClaimLeaseConfidence::Low => "low",
                    ClaimLeaseConfidence::Medium => "medium",
                    ClaimLeaseConfidence::High => "high",
                }
                .to_string();
                let supporting_evidence =
                    serde_json::from_str::<Vec<String>>(&record.evidence_json)
                        .unwrap_or_else(|_| vec![compact_display_text(&record.evidence_json, 120)]);
                crate::core::memory_os::MemoryOsPromotedAssertionRecord {
                    assertion_id: format!("claim-lease:{}", record.id),
                    category,
                    statement: record.claim_text.clone(),
                    normalized_claim: normalize_claim_text(&record.claim_text),
                    scope: scope_label,
                    scope_target,
                    status,
                    promotion_basis: "legacy_claim_lease".to_string(),
                    confidence,
                    stability,
                    first_promoted_at: record.timestamp.to_rfc3339(),
                    last_confirmed_at: record.timestamp.to_rfc3339(),
                    review_after: record.review_after.clone(),
                    expires_at: record.expires_at.clone(),
                    last_reviewed_at: record.last_reviewed_at.clone(),
                    demotion_reason: record.demotion_reason.clone(),
                    supporting_evidence,
                }
            })
            .collect())
    }
}
