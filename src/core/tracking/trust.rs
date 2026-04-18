use anyhow::Result;
use chrono::Utc;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::{current_project_path_string, hash_text, memory_os_scope_params, Tracker};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryOsTrustDecision {
    Allow,
    Deny,
    Review,
}

impl std::fmt::Display for MemoryOsTrustDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Review => "review",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsTrustObservationInput {
    pub observation_id: String,
    pub target_kind: String,
    pub target_ref: String,
    pub action_kind: String,
    pub decision: MemoryOsTrustDecision,
    pub reason_json: String,
    pub read_seq_cut: Option<i64>,
    pub policy_model_id: Option<String>,
    pub sensitivity_class: String,
    pub contains_secret: bool,
    pub contains_pii: bool,
    pub must_not_packetize: bool,
    pub taint_state: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryOsTrustFindingKind {
    Secret,
    Pii,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsTrustFinding {
    pub detector_id: String,
    pub kind: MemoryOsTrustFindingKind,
    pub severity: String,
    pub match_excerpt: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsTrustScanSummary {
    pub contains_secret: bool,
    pub contains_pii: bool,
    pub must_not_packetize: bool,
    pub findings: Vec<MemoryOsTrustFinding>,
}

fn secret_patterns() -> &'static Vec<(Regex, &'static str)> {
    static SECRET_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    SECRET_PATTERNS.get_or_init(|| {
        vec![
            (
                Regex::new(
                    r#"(?i)\b(password|passwd|secret|api[_-]?key|access[_-]?token)\b(?:['"])?\s*[:=]\s*['"]?[A-Za-z0-9_./+=-]{12,}"#,
                )
                .expect("secret assignment regex"),
                "high",
            ),
            (
                Regex::new(r"\bgh[pousr]_[A-Za-z0-9_]{20,}\b").expect("github token regex"),
                "critical",
            ),
            (
                Regex::new(r"\bsk-[A-Za-z0-9]{20,}\b").expect("openai key regex"),
                "critical",
            ),
        ]
    })
}

fn email_pattern() -> &'static Regex {
    static EMAIL_PATTERN: OnceLock<Regex> = OnceLock::new();
    EMAIL_PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").expect("email regex")
    })
}

pub fn scan_memory_os_trust_payload(text: &str) -> MemoryOsTrustScanSummary {
    let mut summary = MemoryOsTrustScanSummary::default();

    for (pattern, severity) in secret_patterns() {
        for mat in pattern.find_iter(text) {
            summary.contains_secret = true;
            summary.must_not_packetize = true;
            summary.findings.push(MemoryOsTrustFinding {
                detector_id: "memoryos-secret-scan-v1".into(),
                kind: MemoryOsTrustFindingKind::Secret,
                severity: (*severity).into(),
                match_excerpt: mat.as_str().to_string(),
            });
        }
    }

    for mat in email_pattern().find_iter(text) {
        summary.contains_pii = true;
        summary.findings.push(MemoryOsTrustFinding {
            detector_id: "memoryos-pii-scan-v1".into(),
            kind: MemoryOsTrustFindingKind::Pii,
            severity: "medium".into(),
            match_excerpt: mat.as_str().to_string(),
        });
    }

    summary
}

impl Tracker {
    #[allow(dead_code)]
    pub fn record_memory_os_trust_observation(
        &self,
        input: &MemoryOsTrustObservationInput,
    ) -> Result<i64> {
        let project_path = current_project_path_string();
        self.record_memory_os_trust_observation_for_project(&project_path, input)
    }

    pub(crate) fn record_memory_os_trust_observation_for_project(
        &self,
        project_path: &str,
        input: &MemoryOsTrustObservationInput,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO memory_os_trust_observations (
                observation_id, project_path, target_kind, target_ref, action_kind, decision,
                reason_json, read_seq_cut, policy_model_id, sensitivity_class,
                contains_secret, contains_pii, must_not_packetize, taint_state, observed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                input.observation_id,
                project_path,
                input.target_kind,
                input.target_ref,
                input.action_kind,
                input.decision.to_string(),
                input.reason_json,
                input.read_seq_cut,
                input.policy_model_id,
                input.sensitivity_class,
                if input.contains_secret { 1 } else { 0 },
                if input.contains_pii { 1 } else { 0 },
                if input.must_not_packetize { 1 } else { 0 },
                input.taint_state,
                input.observed_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_memory_os_trust_report(
        &self,
        scope: crate::core::memory_os::MemoryOsInspectionScope,
        project_path: Option<&str>,
    ) -> Result<crate::core::memory_os::MemoryOsTrustReport> {
        let (project_exact, project_glob, _) = memory_os_scope_params(scope, project_path);

        let (observation_count, must_not_packetize_count, secret_count, pii_count): (
            i64,
            i64,
            i64,
            i64,
        ) = self.conn.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(must_not_packetize), 0),
                COALESCE(SUM(contains_secret), 0),
                COALESCE(SUM(contains_pii), 0)
             FROM memory_os_trust_observations
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)",
            params![project_exact, project_glob],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let mut target_stmt = self.conn.prepare(
            "SELECT
                target_kind,
                COUNT(*),
                COALESCE(SUM(must_not_packetize), 0),
                COALESCE(SUM(contains_secret), 0),
                COALESCE(SUM(contains_pii), 0),
                MAX(observed_at)
             FROM memory_os_trust_observations
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             GROUP BY target_kind
             ORDER BY COUNT(*) DESC, target_kind ASC",
        )?;
        let by_target = target_stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(crate::core::memory_os::MemoryOsTrustTargetSummary {
                    target_kind: row.get(0)?,
                    observation_count: row.get::<_, i64>(1)? as usize,
                    must_not_packetize_count: row.get::<_, i64>(2)? as usize,
                    secret_count: row.get::<_, i64>(3)? as usize,
                    pii_count: row.get::<_, i64>(4)? as usize,
                    latest_observed_at: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut recent_stmt = self.conn.prepare(
            "SELECT
                observation_id, project_path, target_kind, target_ref, action_kind, decision,
                reason_json, read_seq_cut, policy_model_id, sensitivity_class,
                contains_secret, contains_pii, must_not_packetize, taint_state, observed_at
             FROM memory_os_trust_observations
             WHERE (?1 IS NULL OR project_path = ?1 OR project_path GLOB ?2)
             ORDER BY observed_at DESC
             LIMIT 20",
        )?;
        let recent_observations = recent_stmt
            .query_map(params![project_exact, project_glob], |row| {
                Ok(crate::core::memory_os::MemoryOsTrustObservationRecord {
                    observation_id: row.get(0)?,
                    project_path: row.get(1)?,
                    target_kind: row.get(2)?,
                    target_ref: row.get(3)?,
                    action_kind: row.get(4)?,
                    decision: row.get(5)?,
                    reason_json: row.get(6)?,
                    read_seq_cut: row.get(7)?,
                    policy_model_id: row.get(8)?,
                    sensitivity_class: row.get(9)?,
                    contains_secret: row.get::<_, i64>(10)? != 0,
                    contains_pii: row.get::<_, i64>(11)? != 0,
                    must_not_packetize: row.get::<_, i64>(12)? != 0,
                    taint_state: row.get(13)?,
                    observed_at: row.get(14)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(crate::core::memory_os::MemoryOsTrustReport {
            generated_at: Utc::now().to_rfc3339(),
            scope,
            observation_count: observation_count as usize,
            must_not_packetize_count: must_not_packetize_count as usize,
            secret_count: secret_count as usize,
            pii_count: pii_count as usize,
            by_target,
            recent_observations,
        })
    }

    pub(super) fn observe_memory_os_payload_for_project(
        &self,
        project_path: &str,
        target_kind: &str,
        target_ref: &str,
        action_kind: &str,
        payload_text: &str,
    ) -> Result<()> {
        let flags = crate::core::config::memory_os();
        if !flags.trust_v1 {
            return Ok(());
        }

        let summary = scan_memory_os_trust_payload(payload_text);
        if summary.findings.is_empty() {
            return Ok(());
        }

        let observed_at = Utc::now().to_rfc3339();
        let reason_json = serde_json::to_string(&summary.findings)?;
        let decision = if summary.must_not_packetize {
            MemoryOsTrustDecision::Review
        } else {
            MemoryOsTrustDecision::Allow
        };
        let input = MemoryOsTrustObservationInput {
            observation_id: hash_text(&format!(
                "trust-observation:{}:{}:{}:{}",
                project_path, target_kind, target_ref, observed_at
            )),
            target_kind: target_kind.to_string(),
            target_ref: target_ref.to_string(),
            action_kind: action_kind.to_string(),
            decision,
            reason_json,
            read_seq_cut: None,
            policy_model_id: None,
            sensitivity_class: if summary.contains_secret {
                "secret".into()
            } else if summary.contains_pii {
                "confidential".into()
            } else {
                "internal".into()
            },
            contains_secret: summary.contains_secret,
            contains_pii: summary.contains_pii,
            must_not_packetize: summary.must_not_packetize,
            taint_state: if summary.must_not_packetize {
                "quoted_untrusted".into()
            } else {
                "clean".into()
            },
            observed_at,
        };
        let _ = self.record_memory_os_trust_observation_for_project(project_path, &input);
        Ok(())
    }
}
