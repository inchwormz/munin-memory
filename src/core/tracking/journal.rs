use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{hash_text, Tracker};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryOsShadowEvent {
    pub event_id: String,
    pub stream_id: String,
    pub stream_revision: i64,
    pub expected_stream_revision: Option<i64>,
    pub tx_index: i64,
    pub event_kind: String,
    pub idempotency_key: String,
    pub idempotency_receipt_id: Option<String>,
    pub project_path: String,
    pub scope_json: String,
    pub actor_json: String,
    pub target_refs_json: String,
    pub payload_json: String,
    pub proof_refs_json: String,
    pub precondition_hash: Option<String>,
    pub result_hash: Option<String>,
    pub schema_fingerprint: String,
}

#[derive(Debug, Clone)]
pub(super) struct MemoryOsJournalEventRow {
    pub(super) journal_seq: i64,
    pub(super) event_id: String,
    pub(super) event_kind: String,
    pub(super) committed_at: String,
    pub(super) payload_json: String,
}

impl Tracker {
    fn next_memory_os_stream_revision(&self, stream_id: &str) -> Result<i64> {
        let revision = self.conn.query_row(
            "SELECT COALESCE(MAX(stream_revision), 0) + 1 FROM memory_os_journal_events WHERE stream_id = ?1",
            params![stream_id],
            |row| row.get(0),
        )?;
        Ok(revision)
    }

    pub(super) fn record_memory_os_shadow_event(&self, event: MemoryOsShadowEvent) -> Result<()> {
        let flags = crate::core::config::memory_os();
        if !(flags.journal_v1 && flags.dual_write_v1) {
            return Ok(());
        }

        self.insert_memory_os_shadow_event(event)
    }

    pub(super) fn record_memory_os_shadow_event_unchecked(
        &self,
        event: MemoryOsShadowEvent,
    ) -> Result<()> {
        self.insert_memory_os_shadow_event(event)
    }

    fn insert_memory_os_shadow_event(&self, event: MemoryOsShadowEvent) -> Result<()> {
        let payload_hash = hash_text(&event.payload_json);
        let existing: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT payload_hash, first_event_id FROM memory_os_idempotency_receipts WHERE idempotency_key = ?1",
                params![&event.idempotency_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((existing_hash, _existing_event_id)) = existing {
            if existing_hash == payload_hash {
                return Ok(());
            }
            return Err(anyhow!(
                "memory_os shadow event idempotency key reused with different payload"
            ));
        }

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let tx_result: Result<()> = (|| {
            let stream_revision = self.next_memory_os_stream_revision(&event.stream_id)?;
            self.conn.execute(
                "INSERT INTO memory_os_journal_events (
                    event_id, stream_id, stream_revision, expected_stream_revision, tx_index,
                    occurred_at, committed_at, event_kind, idempotency_key, idempotency_receipt_id,
                    project_path, scope_json, actor_json, target_refs_json, payload_json,
                    proof_refs_json, precondition_hash, result_hash, schema_fingerprint
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    &event.event_id,
                    &event.stream_id,
                    stream_revision,
                    event.expected_stream_revision,
                    event.tx_index,
                    Utc::now().to_rfc3339(),
                    Utc::now().to_rfc3339(),
                    &event.event_kind,
                    &event.idempotency_key,
                    &event.idempotency_receipt_id,
                    &event.project_path,
                    &event.scope_json,
                    &event.actor_json,
                    &event.target_refs_json,
                    &event.payload_json,
                    &event.proof_refs_json,
                    &event.precondition_hash,
                    &event.result_hash,
                    &event.schema_fingerprint,
                ],
            )?;
            self.conn.execute(
                "INSERT INTO memory_os_idempotency_receipts (idempotency_key, payload_hash, first_event_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    &event.idempotency_key,
                    payload_hash,
                    &event.event_id,
                    Utc::now().to_rfc3339(),
                ],
            )?;
            Ok(())
        })();

        if let Err(err) = tx_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(err);
        }
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    pub(super) fn memory_os_journal_seq_for_event(&self, event_id: &str) -> Result<Option<i64>> {
        self.conn
            .query_row(
                "SELECT journal_seq FROM memory_os_journal_events WHERE event_id = ?1",
                params![event_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn record_memory_os_projection_checkpoint(
        &self,
        projection_name: &str,
        project_path: &str,
        from_seq: i64,
        to_seq: i64,
        rebuild_kind: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO memory_os_projection_checkpoints (
                projection_name, project_path, from_seq, to_seq, rebuild_kind, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                projection_name,
                project_path,
                from_seq,
                to_seq,
                rebuild_kind,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
}
