use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;

use super::{
    current_project_path_string, hash_text, memory_os_scope_params, parse_rfc3339_to_utc,
    MemoryOsShadowEvent, Tracker,
};

#[derive(Debug, Clone)]
pub(super) struct MemoryOsCheckpointEnvelope {
    pub(super) project_path: String,
    pub(super) captured_at: DateTime<Utc>,
    pub(super) capture: crate::core::memory_os::MemoryOsCheckpointCapture,
}

impl Tracker {
    /// A small, conservative user model for live agent context. The full
    /// profile scans shell history; this reads only recent semantic captures.
    pub fn get_memory_os_user_preferences_fast(
        &self,
        project_path: &str,
    ) -> Result<Vec<crate::core::memory_os::MemoryOsNarrativeFinding>> {
        let mut stmt = self.conn.prepare(
            "SELECT project_path, payload_json
             FROM memory_os_journal_events
             WHERE event_kind LIKE 'legacy.packet-checkpoint.%'
               AND payload_json LIKE '%user_work_style%'
             ORDER BY json_extract(payload_json, '$.generated_at') DESC, journal_seq DESC
             LIMIT 512",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut findings = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            let (source_project, payload) = row?;
            let Ok(capture) =
                serde_json::from_str::<crate::core::memory_os::MemoryOsCheckpointCapture>(&payload)
            else {
                continue;
            };
            for item in capture.selected_items {
                if item.section != "user_work_style"
                    || !stable_preference(&item.summary, &source_project, project_path)
                {
                    continue;
                }
                let normalized = item.summary.trim().to_ascii_lowercase();
                if !seen.insert(normalized) {
                    continue;
                }
                findings.push(crate::core::memory_os::MemoryOsNarrativeFinding {
                    title: "Working preference".to_string(),
                    summary: item.summary,
                    evidence: vec![
                        format!("checkpoint:{}", capture.packet_id),
                        format!("observed:{}", capture.generated_at),
                        format!("project:{source_project}"),
                    ],
                });
                if findings.len() == 3 {
                    return Ok(findings);
                }
            }
        }
        Ok(findings)
    }

    pub fn get_memory_os_recent_project_request_fast(
        &self,
        project_path: &str,
    ) -> Result<Option<crate::core::memory_os::MemoryOsNarrativeFinding>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload_json
             FROM memory_os_journal_events
             WHERE event_kind LIKE 'legacy.packet-checkpoint.%'
               AND project_path = ?1
             ORDER BY json_extract(payload_json, '$.generated_at') DESC, journal_seq DESC
             LIMIT 512",
        )?;
        let rows = stmt.query_map([project_path], |row| row.get::<_, String>(0))?;
        for row in rows {
            let payload = row?;
            let Ok(capture) =
                serde_json::from_str::<crate::core::memory_os::MemoryOsCheckpointCapture>(&payload)
            else {
                continue;
            };
            if capture.profile != "session-onboarding" {
                continue;
            }
            if current_session_capture(&capture.packet_id) {
                continue;
            }
            if !recent_session_capture(&capture.generated_at) {
                continue;
            }
            let Some(goal) = capture.goal.as_deref().map(str::trim) else {
                continue;
            };
            if goal.is_empty()
                || goal.starts_with('#')
                || goal.starts_with('<')
                || delegated_agent_prompt(goal)
            {
                continue;
            }
            return Ok(Some(crate::core::memory_os::MemoryOsNarrativeFinding {
                title: "Last recorded request (completion unknown)".to_string(),
                summary: crate::core::utils::truncate(goal, 240),
                evidence: vec![
                    format!("checkpoint:{}", capture.packet_id),
                    format!("observed:{}", capture.generated_at),
                ],
            }));
        }
        Ok(None)
    }

    pub fn get_memory_os_recent_project_outcome_fast(
        &self,
        project_path: &str,
    ) -> Result<Option<crate::core::memory_os::MemoryOsNarrativeFinding>> {
        let mut stmt = self.conn.prepare(
            "SELECT payload_json
             FROM memory_os_journal_events
             WHERE event_kind LIKE 'legacy.packet-checkpoint.%'
               AND project_path = ?1
             ORDER BY json_extract(payload_json, '$.generated_at') DESC, journal_seq DESC
             LIMIT 512",
        )?;
        let rows = stmt.query_map([project_path], |row| row.get::<_, String>(0))?;
        for row in rows {
            let payload = row?;
            let Ok(capture) =
                serde_json::from_str::<crate::core::memory_os::MemoryOsCheckpointCapture>(&payload)
            else {
                continue;
            };
            if capture.profile != "session-onboarding" {
                continue;
            }
            if current_session_capture(&capture.packet_id) {
                continue;
            }
            if !recent_session_capture(&capture.generated_at)
                || capture.goal.as_deref().is_some_and(delegated_agent_prompt)
            {
                continue;
            }
            let Some(outcome) = capture
                .selected_items
                .iter()
                .find(|item| item.kind == "agent-reported-outcome")
            else {
                continue;
            };
            return Ok(Some(crate::core::memory_os::MemoryOsNarrativeFinding {
                title: "Agent reported result (unverified)".to_string(),
                summary: crate::core::utils::truncate(&outcome.summary, 240),
                evidence: vec![
                    format!("checkpoint:{}", capture.packet_id),
                    format!("observed:{}", capture.generated_at),
                ],
            }));
        }
        Ok(None)
    }

    pub fn record_memory_os_packet_checkpoint(
        &self,
        input: &crate::core::memory_os::MemoryOsCheckpointCapture,
    ) -> Result<()> {
        let project_path = current_project_path_string();
        self.record_memory_os_packet_checkpoint_for_project(&project_path, input)
    }

    pub fn record_memory_os_packet_checkpoint_for_project(
        &self,
        project_path: &str,
        input: &crate::core::memory_os::MemoryOsCheckpointCapture,
    ) -> Result<()> {
        let flags = crate::core::config::memory_os();
        let cutover_checkpoint_write =
            flags.journal_v1 && flags.dual_write_v1 && flags.checkpoint_v1;
        let onboarding_read_model_write =
            flags.read_model_v1 && input.profile == "session-onboarding";
        if !(cutover_checkpoint_write || onboarding_read_model_write) {
            return Ok(());
        }

        let payload_json = serde_json::to_string(input)?;
        let target_refs = input
            .selected_items
            .iter()
            .filter_map(|item| item.subject.clone())
            .collect::<Vec<_>>();
        let payload_hash = hash_text(&payload_json);
        let payload_suffix = payload_hash.chars().take(12).collect::<String>();
        let event_id = if onboarding_read_model_write && !cutover_checkpoint_write {
            format!(
                "legacy-packet-{}-{}-{}",
                input.preset.as_str(),
                input.packet_id.as_str(),
                payload_suffix
            )
        } else {
            format!(
                "legacy-packet-{}-{}",
                input.preset.as_str(),
                input.packet_id.as_str()
            )
        };
        let idempotency_key = if onboarding_read_model_write && !cutover_checkpoint_write {
            format!(
                "legacy.packet:{}:{}",
                input.packet_id.as_str(),
                payload_suffix
            )
        } else {
            format!("legacy.packet:{}", input.packet_id.as_str())
        };

        let event = MemoryOsShadowEvent {
            event_id: event_id.clone(),
            stream_id: format!("legacy.packet:{}:{}", project_path, input.preset.as_str()),
            stream_revision: 0,
            expected_stream_revision: None,
            tx_index: 0,
            event_kind: format!("legacy.packet-checkpoint.{}", input.preset.as_str()),
            idempotency_key,
            idempotency_receipt_id: None,
            project_path: project_path.to_string(),
            scope_json: serde_json::json!({
                "repo_id": project_path,
                "branch_id": "",
                "worktree_id": "",
                "task_id": serde_json::Value::Null,
                "objective_id": serde_json::Value::Null,
                "session_id": serde_json::Value::Null,
                "agent_id": serde_json::Value::Null,
                "runtime_profile": &input.profile,
                "os_profile": std::env::consts::OS,
                "valid_from": &input.generated_at,
                "valid_until": serde_json::Value::Null
            })
            .to_string(),
            actor_json: serde_json::json!({
                "actor_id": "munin",
                "actor_kind": "system",
                "origin_agent_id": serde_json::Value::Null,
                "trust_domain": "local_core"
            })
            .to_string(),
            target_refs_json: serde_json::to_string(&target_refs)?,
            payload_json,
            proof_refs_json: "[]".to_string(),
            precondition_hash: None,
            result_hash: Some(input.pager_manifest_hash.clone()),
            schema_fingerprint: "memoryos-packet-checkpoint-v1".into(),
        };
        if cutover_checkpoint_write {
            self.record_memory_os_shadow_event(event)?;
        } else {
            self.record_memory_os_shadow_event_unchecked(event)?;
        }

        if let Some(journal_seq) = self.memory_os_journal_seq_for_event(&event_id)? {
            let _ = self.record_memory_os_projection_checkpoint(
                "packet-checkpoints",
                project_path,
                journal_seq,
                journal_seq,
                "append",
            );
        }

        if flags.action_v1 && input.profile != "session-onboarding" {
            let trigger_item = input
                .selected_items
                .iter()
                .find(|item| item.section == "open_obligations")
                .or_else(|| {
                    input
                        .selected_items
                        .iter()
                        .find(|item| item.section == "current_failures")
                })
                .or_else(|| {
                    input
                        .selected_items
                        .iter()
                        .find(|item| item.section == "validated_claim_leases")
                })
                .or_else(|| input.selected_items.first());
            let cue = crate::core::memory_os::MemoryOsActionCue {
                cue_kind: "checkpoint-reentry".to_string(),
                packet_preset: Some(input.preset.clone()),
                intent: Some(input.intent.clone()),
                override_type: None,
                correction_shape: None,
                trigger_section: trigger_item.map(|item| item.section.clone()),
                trigger_subject: trigger_item.and_then(|item| item.subject.clone()),
                trigger_summary: trigger_item.map(|item| item.summary.clone()),
            };
            let action = crate::core::memory_os::MemoryOsAction {
                action_kind: "run_command".to_string(),
                command_sig: Some(input.reentry.recommended_command.clone()),
                recommendation: input.reentry.current_recommendation.clone(),
            };
            self.record_memory_os_action_observation(
                project_path,
                "checkpoint-reentry",
                Some(&event_id),
                &cue,
                &action,
                &input.packet_id,
                &input.generated_at,
            )?;
        }

        Ok(())
    }

    pub(super) fn load_memory_os_checkpoint_captures(
        &self,
        scope: crate::core::memory_os::MemoryOsInspectionScope,
        project_path: Option<&str>,
    ) -> Result<Vec<MemoryOsCheckpointEnvelope>> {
        let (project_exact, project_glob, _) = memory_os_scope_params(scope, project_path);
        let mut stmt = self.conn.prepare(
            "SELECT project_path, committed_at, payload_json
             FROM memory_os_journal_events
             WHERE event_kind LIKE 'legacy.packet-checkpoint.%'
               AND (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY committed_at DESC, journal_seq DESC",
        )?;
        let rows = stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut checkpoints = Vec::new();
        for (project_path, committed_at, payload_json) in rows {
            let Ok(capture) = serde_json::from_str::<
                crate::core::memory_os::MemoryOsCheckpointCapture,
            >(&payload_json) else {
                continue;
            };
            checkpoints.push(MemoryOsCheckpointEnvelope {
                project_path,
                captured_at: parse_rfc3339_to_utc(&committed_at),
                capture,
            });
        }
        Ok(checkpoints)
    }
}

fn current_session_capture(packet_id: &str) -> bool {
    std::env::var("CODEX_THREAD_ID")
        .ok()
        .is_some_and(|id| packet_id.ends_with(&id))
}

fn recent_session_capture(generated_at: &str) -> bool {
    DateTime::parse_from_rfc3339(generated_at)
        .ok()
        .map(|time| time.with_timezone(&Utc))
        .is_some_and(|time| time >= Utc::now() - chrono::Duration::days(45))
}

pub(super) fn delegated_agent_prompt(text: &str) -> bool {
    let lowered = text.trim().to_ascii_lowercase();
    if lowered.starts_with("continue working toward the active thread goal") {
        return true;
    }
    if lowered.starts_with("leader sweep")
        || lowered.starts_with("lane sweep")
        || lowered.contains("autonomous-ship section 0")
    {
        return true;
    }
    (lowered.starts_with("you are a ") || lowered.starts_with("you are an "))
        && [
            "agent",
            "reviewer",
            "worker",
            "lead",
            "tester",
            "researcher",
            "auditor",
        ]
        .iter()
        .any(|role| lowered.get(..120).unwrap_or(&lowered).contains(role))
}

fn stable_preference(text: &str, _source_project: &str, _current_project: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.len() > 240 || normalized.contains('\n') {
        return false;
    }
    let global = [
        "i prefer ",
        "i'd prefer ",
        "i would prefer ",
        "i always want ",
        "i want you to always ",
    ]
    .iter()
    .any(|prefix| normalized.starts_with(prefix));
    global
}

#[cfg(test)]
mod startup_tests {
    use super::{delegated_agent_prompt, stable_preference, Tracker};
    use crate::core::memory_os::{
        MemoryOsCheckpointCapture, MemoryOsCheckpointReentry, MemoryOsCheckpointTelemetry,
        MemoryOsPacketSelection,
    };
    use rusqlite::params;

    use chrono::Utc;

    #[test]
    fn only_explicit_applicable_preferences_enter_live_context() {
        assert!(stable_preference(
            "I prefer short answers with evidence.",
            "C:/other",
            "C:/repo"
        ));
        assert!(stable_preference(
            "I prefer isolated worktrees for risky changes.",
            "C:/other",
            "C:/repo"
        ));
        assert!(!stable_preference(
            "I don't want this design file editable.",
            "C:/repo",
            "C:/repo"
        ));
        assert!(!stable_preference(
            "I don't want this design file editable.",
            "C:/other",
            "C:/repo"
        ));
        assert!(!stable_preference(
            "Review the exact commit before approval.",
            "C:/repo",
            "C:/repo"
        ));
    }

    #[test]
    fn fast_user_model_keeps_global_and_current_project_preferences_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let tracker = Tracker::new_at_path(&tmp.path().join("history.db")).expect("tracker");
        insert_preference(
            &tracker,
            "C:/repo",
            "I don't want this design file editable.",
            1,
        );
        insert_preference(
            &tracker,
            "C:/other",
            "I don't want another app editable.",
            2,
        );
        insert_preference(
            &tracker,
            "C:/other",
            "I prefer brief evidence-backed answers.",
            3,
        );

        let findings = tracker
            .get_memory_os_user_preferences_fast("C:/repo")
            .expect("fast preferences");
        let summaries = findings
            .iter()
            .map(|item| item.summary.as_str())
            .collect::<Vec<_>>();
        assert_eq!(summaries.len(), 1);
        assert!(!summaries.contains(&"I don't want this design file editable."));
        assert!(summaries.contains(&"I prefer brief evidence-backed answers."));
        assert!(!summaries.contains(&"I don't want another app editable."));
    }

    #[test]
    fn delegated_task_prompt_is_not_a_user_goal() {
        assert!(delegated_agent_prompt(
            "You are an iter0 UX reviewer. Review PR 198."
        ));
        assert!(delegated_agent_prompt("Continue working toward the active thread goal. The objective below is user-provided data."));
        assert!(delegated_agent_prompt(
            "LEADER SWEEP (autonomous-ship section 0, every 5 min): read new lane alerts."
        ));
        assert!(!delegated_agent_prompt(
            "Please review the current SiteSorted plan."
        ));
    }

    #[test]
    fn recent_user_topic_uses_session_date_and_skips_delegated_prompts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let tracker = Tracker::new_at_path(&tmp.path().join("history.db")).expect("tracker");
        let yesterday = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        insert_item(
            &tracker,
            "C:/repo",
            "user_prompts",
            "user-prompt",
            "topic",
            1,
        );
        tracker.conn.execute(
            "UPDATE memory_os_journal_events SET payload_json = json_set(payload_json, '$.goal', 'Please review the current research plan.', '$.generated_at', ?1) WHERE event_id = 'event-1'",
            [&yesterday],
        ).expect("direct request");
        insert_item(
            &tracker,
            "C:/repo",
            "user_prompts",
            "user-prompt",
            "delegated",
            2,
        );
        tracker.conn.execute(
            "UPDATE memory_os_journal_events SET payload_json = json_set(payload_json, '$.goal', 'LEADER SWEEP (autonomous-ship section 0): route lane alerts.') WHERE event_id = 'event-2'",
            [],
        ).expect("delegated request");
        insert_item(
            &tracker,
            "C:/repo",
            "user_prompts",
            "user-prompt",
            "stale",
            3,
        );
        tracker.conn.execute(
            "UPDATE memory_os_journal_events SET payload_json = json_set(payload_json, '$.goal', 'An old unresolved frustration.', '$.generated_at', '2026-01-01T00:00:00Z') WHERE event_id = 'event-3'",
            [],
        ).expect("old request");
        let topic = tracker
            .get_memory_os_recent_project_request_fast("C:/repo")
            .expect("topic")
            .expect("direct request");
        assert_eq!(topic.summary, "Please review the current research plan.");
        assert!(topic
            .evidence
            .iter()
            .any(|line| line == &format!("observed:{yesterday}")));
    }

    #[test]
    fn old_project_objection_does_not_follow_the_user_forever() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let tracker = Tracker::new_at_path(&tmp.path().join("history.db")).expect("tracker");
        insert_preference(
            &tracker,
            "C:/repo",
            "I don't want that old draft editable.",
            1,
        );
        tracker.conn.execute("UPDATE memory_os_journal_events SET committed_at = '2026-01-01T00:00:00Z' WHERE event_id = 'event-1'", []).expect("age preference");
        insert_preference(&tracker, "C:/other", "I prefer concise reports.", 2);
        let findings = tracker
            .get_memory_os_user_preferences_fast("C:/repo")
            .expect("preferences");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].summary, "I prefer concise reports.");
    }

    #[test]
    fn recent_agent_result_is_scoped_and_labeled_unverified() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let tracker = Tracker::new_at_path(&tmp.path().join("history.db")).expect("tracker");
        insert_item(
            &tracker,
            "C:/other",
            "session_outcome",
            "agent-reported-outcome",
            "The unrelated project was deployed.",
            1,
        );
        insert_item(
            &tracker,
            "C:/repo",
            "session_outcome",
            "agent-reported-outcome",
            "The requested parser was built locally.",
            2,
        );
        let result = tracker
            .get_memory_os_recent_project_outcome_fast("C:/repo")
            .expect("read outcome")
            .expect("matching outcome");
        assert_eq!(result.title, "Agent reported result (unverified)");
        assert_eq!(result.summary, "The requested parser was built locally.");
        assert!(tracker
            .get_memory_os_recent_project_outcome_fast("C:/missing")
            .expect("read missing project")
            .is_none());
    }

    fn insert_preference(tracker: &Tracker, project: &str, summary: &str, id: usize) {
        insert_item(
            tracker,
            project,
            "user_work_style",
            "working-preference",
            summary,
            id,
        );
    }

    fn insert_item(
        tracker: &Tracker,
        project: &str,
        section: &str,
        kind: &str,
        summary: &str,
        id: usize,
    ) {
        let capture = MemoryOsCheckpointCapture {
            packet_id: format!("test-{id}"),
            generated_at: "2026-09-28T00:00:00Z".to_string(),
            preset: "resume".to_string(),
            intent: "diagnose".to_string(),
            profile: "session-onboarding".to_string(),
            goal: None,
            budget: 1600,
            estimated_tokens: 0,
            estimated_source_tokens: 0,
            pager_manifest_hash: "hash".to_string(),
            recall_mode: "off".to_string(),
            recall_used: false,
            recall_reason: "test".to_string(),
            telemetry: MemoryOsCheckpointTelemetry {
                current_fact_count: 0,
                recent_change_count: 0,
                live_claim_count: 0,
                open_obligation_count: 0,
                artifact_handle_count: 0,
                failure_count: 0,
            },
            selected_items: vec![MemoryOsPacketSelection {
                section: section.to_string(),
                kind: kind.to_string(),
                summary: summary.to_string(),
                token_estimate: 10,
                score: 90,
                artifact_id: None,
                subject: None,
                provenance: vec!["session:test".to_string()],
            }],
            exclusions: Vec::new(),
            reentry: MemoryOsCheckpointReentry {
                recommended_command: String::new(),
                current_recommendation: None,
                first_question: String::new(),
                first_verification: String::new(),
            },
        };
        tracker
            .conn
            .execute(
                "INSERT INTO memory_os_journal_events (
                event_id, stream_id, stream_revision, tx_index, occurred_at, committed_at,
                event_kind, idempotency_key, project_path, scope_json, actor_json,
                target_refs_json, payload_json, proof_refs_json, schema_fingerprint
             ) VALUES (?1, ?2, 1, 0, ?3, ?3, 'legacy.packet-checkpoint.test', ?1,
                       ?4, '{}', '{}', '[]', ?5, '[]', 'test')",
                params![
                    format!("event-{id}"),
                    format!("stream-{id}"),
                    capture.generated_at,
                    project,
                    serde_json::to_string(&capture).expect("payload"),
                ],
            )
            .expect("seed checkpoint");
    }
}
