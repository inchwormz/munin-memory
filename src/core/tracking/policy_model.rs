#![allow(dead_code)]

use anyhow::Result;
use rusqlite::{params, params_from_iter, ToSql};
use serde::{Deserialize, Serialize};

use super::Tracker;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsAccessRule {
    pub access_rule_id: String,
    pub subject_predicate: String,
    pub object_predicate: String,
    pub environment_predicate: String,
    pub action: String,
    pub effect: String,
    pub priority: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsPolicyModelInput {
    pub policy_model_id: String,
    pub version: String,
    pub description: String,
    pub created_at: String,
    pub rules: Vec<MemoryOsAccessRule>,
}

impl Tracker {
    pub fn upsert_memory_os_policy_model(&self, input: &MemoryOsPolicyModelInput) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let tx_result: Result<()> = (|| {
            self.conn.execute(
                "INSERT INTO memory_os_policy_models (policy_model_id, version, description, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(policy_model_id) DO UPDATE SET
                    version = excluded.version,
                    description = excluded.description,
                    created_at = excluded.created_at",
                params![
                    input.policy_model_id,
                    input.version,
                    input.description,
                    input.created_at,
                ],
            )?;

            for rule in &input.rules {
                self.conn.execute(
                    "INSERT INTO memory_os_access_rules (
                        access_rule_id, policy_model_id, subject_predicate, object_predicate,
                        environment_predicate, action, effect, priority
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    ON CONFLICT(access_rule_id) DO UPDATE SET
                        policy_model_id = excluded.policy_model_id,
                        subject_predicate = excluded.subject_predicate,
                        object_predicate = excluded.object_predicate,
                        environment_predicate = excluded.environment_predicate,
                        action = excluded.action,
                        effect = excluded.effect,
                        priority = excluded.priority",
                    params![
                        rule.access_rule_id,
                        input.policy_model_id,
                        rule.subject_predicate,
                        rule.object_predicate,
                        rule.environment_predicate,
                        rule.action,
                        rule.effect,
                        rule.priority,
                    ],
                )?;
            }

            if input.rules.is_empty() {
                self.conn.execute(
                    "DELETE FROM memory_os_access_rules WHERE policy_model_id = ?1",
                    params![input.policy_model_id],
                )?;
            } else {
                let placeholders = std::iter::repeat("?")
                    .take(input.rules.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "DELETE FROM memory_os_access_rules
                     WHERE policy_model_id = ?1
                       AND access_rule_id NOT IN ({placeholders})"
                );
                let mut sql_params: Vec<&dyn ToSql> = Vec::with_capacity(input.rules.len() + 1);
                sql_params.push(&input.policy_model_id);
                for rule in &input.rules {
                    sql_params.push(&rule.access_rule_id);
                }
                self.conn.execute(&sql, params_from_iter(sql_params))?;
            }

            Ok(())
        })();

        if let Err(err) = tx_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(err);
        }
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }
}
