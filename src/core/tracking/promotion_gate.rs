use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::Tracker;

const MEMORY_OS_PROMOTION_REQUIRED_SYSTEM: &str = "proposed-kernel";
const MEMORY_OS_PROMOTION_REQUIRED_SPLITS: &[&str] = &["test-private", "adversarial-private"];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryOsVerificationStatus {
    Verified,
    Rejected,
    Inconclusive,
}

impl std::fmt::Display for MemoryOsVerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Inconclusive => "inconclusive",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsVerificationResultInput {
    pub verification_result_id: String,
    pub proof_id: String,
    pub scope_json: String,
    pub verifier_id: String,
    pub verifier_version: String,
    pub trusted_root_id: Option<String>,
    pub trusted_producer_ids: Vec<String>,
    pub materials_hashes: Vec<String>,
    pub products_hashes: Vec<String>,
    pub verification_time: String,
    pub result: MemoryOsVerificationStatus,
    pub reason: Option<String>,
    pub attestation_kind: String,
}

impl Tracker {
    pub fn record_memory_os_verification_result(
        &self,
        input: &MemoryOsVerificationResultInput,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO memory_os_verification_results (
                verification_result_id, proof_id, scope_json, verifier_id, verifier_version,
                trusted_root_id, trusted_producer_ids_json, materials_hashes_json, products_hashes_json,
                verification_time, result, reason, attestation_kind
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                input.verification_result_id,
                input.proof_id,
                input.scope_json,
                input.verifier_id,
                input.verifier_version,
                input.trusted_root_id,
                serde_json::to_string(&input.trusted_producer_ids)?,
                serde_json::to_string(&input.materials_hashes)?,
                serde_json::to_string(&input.products_hashes)?,
                input.verification_time,
                input.result.to_string(),
                input.reason,
                input.attestation_kind,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_memory_os_promotion_report(
        &self,
    ) -> Result<crate::core::memory_os::MemoryOsPromotionReport> {
        let flags = crate::core::config::memory_os();
        let mut stmt = self.conn.prepare(
            "SELECT verification_result_id, scope_json, result, reason, verification_time
             FROM memory_os_verification_results
             ORDER BY verification_time DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;

        let mut matching_result_count = 0usize;
        let mut contaminated_result_count = 0usize;
        let mut latest_matching_result = None;
        let mut latest_by_required_split: BTreeMap<
            String,
            crate::core::memory_os::MemoryOsPromotionResultRecord,
        > = BTreeMap::new();

        for row in rows {
            let (verification_result_id, scope_json, result, reason, verification_time) = row?;
            let Ok(scope) = serde_json::from_str::<serde_json::Value>(&scope_json) else {
                continue;
            };
            let system = scope
                .get("system")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let split = scope
                .get("split")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if system != MEMORY_OS_PROMOTION_REQUIRED_SYSTEM
                || !MEMORY_OS_PROMOTION_REQUIRED_SPLITS.contains(&split)
            {
                continue;
            }

            let independent = scope
                .get("independent")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                || scope.get("proof_tier").and_then(|value| value.as_str()) == Some("independent");
            let contamination_free = scope
                .get("contamination_free")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
                && !scope
                    .get("contaminated")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
            if !contamination_free {
                contaminated_result_count += 1;
            }

            matching_result_count += 1;
            let record = crate::core::memory_os::MemoryOsPromotionResultRecord {
                verification_result_id,
                root: scope
                    .get("root")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                split: split.to_string(),
                system: system.to_string(),
                result,
                reason,
                verification_time,
                independent,
                contamination_free,
            };
            if latest_matching_result.is_none() {
                latest_matching_result = Some(record.clone());
            }
            latest_by_required_split
                .entry(split.to_string())
                .or_insert(record);
        }

        let required_results = MEMORY_OS_PROMOTION_REQUIRED_SPLITS
            .iter()
            .filter_map(|split| latest_by_required_split.get(*split).cloned())
            .collect::<Vec<_>>();
        let missing_required_splits = MEMORY_OS_PROMOTION_REQUIRED_SPLITS
            .iter()
            .filter(|&&split| !latest_by_required_split.contains_key(split))
            .map(|split| (*split).to_string())
            .collect::<Vec<_>>();
        let failed_required_splits = required_results
            .iter()
            .filter(|row| row.result != "verified")
            .map(|row| row.split.clone())
            .collect::<Vec<_>>();
        let contaminated_required_splits = required_results
            .iter()
            .filter(|row| !row.contamination_free)
            .map(|row| row.split.clone())
            .collect::<Vec<_>>();
        let non_independent_required_splits = required_results
            .iter()
            .filter(|row| !row.independent)
            .map(|row| row.split.clone())
            .collect::<Vec<_>>();
        let independent_proof_set_verified = missing_required_splits.is_empty()
            && failed_required_splits.is_empty()
            && contaminated_required_splits.is_empty()
            && non_independent_required_splits.is_empty();

        let eligible = if !flags.strict_promotion_v1 {
            true
        } else {
            independent_proof_set_verified
        };
        let resume_cutover_ready = flags.read_model_v1 && flags.resume_v1 && eligible;
        let handoff_cutover_ready = flags.read_model_v1 && flags.handoff_v1 && eligible;
        let required_split_label = MEMORY_OS_PROMOTION_REQUIRED_SPLITS.join("+");
        let decision_summary = if !flags.read_model_v1 {
            "Memory OS read model is disabled, so resume and handoff stay on the packet path."
                .to_string()
        } else if !flags.resume_v1 && !flags.handoff_v1 {
            "Resume and handoff cutover flags are disabled, so the promotion proof is advisory only."
                .to_string()
        } else if !flags.strict_promotion_v1 {
            "Strict promotion gate is disabled, so replay proof is advisory only.".to_string()
        } else if independent_proof_set_verified {
            format!(
                "Strict promotion gate passed: independent {} / {} replay proof set is verified and contamination-free.",
                MEMORY_OS_PROMOTION_REQUIRED_SYSTEM, required_split_label
            )
        } else if !missing_required_splits.is_empty() {
            format!(
                "Strict promotion gate is blocking cutover: missing independent {} proof for {}.",
                MEMORY_OS_PROMOTION_REQUIRED_SYSTEM,
                missing_required_splits.join(", ")
            )
        } else if !non_independent_required_splits.is_empty() {
            format!(
                "Strict promotion gate is blocking cutover: proof for {} is not marked independent.",
                non_independent_required_splits.join(", ")
            )
        } else if !contaminated_required_splits.is_empty() {
            format!(
                "Strict promotion gate is blocking cutover: proof for {} is missing contamination-free attestation.",
                contaminated_required_splits.join(", ")
            )
        } else if !failed_required_splits.is_empty() {
            format!(
                "Strict promotion gate is blocking cutover: proof for {} is not verified.",
                failed_required_splits.join(", ")
            )
        } else {
            format!(
                "Strict promotion gate is blocking cutover: no {} / {} replay proof set is recorded yet.",
                MEMORY_OS_PROMOTION_REQUIRED_SYSTEM, required_split_label
            )
        };

        Ok(crate::core::memory_os::MemoryOsPromotionReport {
            generated_at: Utc::now().to_rfc3339(),
            read_model_enabled: flags.read_model_v1,
            resume_enabled: flags.resume_v1,
            handoff_enabled: flags.handoff_v1,
            strict_gate_enabled: flags.strict_promotion_v1,
            eligible,
            resume_cutover_ready,
            handoff_cutover_ready,
            required_split: required_split_label,
            required_system: MEMORY_OS_PROMOTION_REQUIRED_SYSTEM.to_string(),
            matching_result_count,
            missing_required_splits,
            contaminated_result_count,
            required_results,
            decision_summary,
            latest_matching_result,
        })
    }
}
