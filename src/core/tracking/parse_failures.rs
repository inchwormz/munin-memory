use anyhow::Result;
use chrono::Utc;
use rusqlite::params;

use super::Tracker;

/// Individual parse failure record.
#[derive(Clone, Debug)]
pub struct ParseFailureRecord {
    pub timestamp: String,
    pub raw_command: String,
    #[allow(dead_code)]
    pub error_message: String,
    pub fallback_succeeded: bool,
}

/// Aggregated parse failure summary.
#[derive(Debug)]
pub struct ParseFailureSummary {
    pub total: usize,
    pub recovered: usize,
    pub unrecovered: usize,
    pub recovery_rate: f64,
    pub top_commands: Vec<(String, usize)>,
    pub top_unrecovered_commands: Vec<(String, usize)>,
    pub recent_unrecovered: Vec<ParseFailureRecord>,
}

impl Tracker {
    /// Record a parse failure for analytics.
    pub fn record_parse_failure(
        &self,
        raw_command: &str,
        error_message: &str,
        fallback_succeeded: bool,
    ) -> Result<()> {
        if should_suppress_parse_failure_record(raw_command, fallback_succeeded) {
            return Ok(());
        }

        self.conn.execute(
            "INSERT INTO parse_failures (timestamp, raw_command, error_message, fallback_succeeded)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                Utc::now().to_rfc3339(),
                raw_command,
                error_message,
                fallback_succeeded as i32,
            ],
        )?;
        self.cleanup_old_if_due()?;
        Ok(())
    }

    /// Get parse failure summary for `context gain --failures`.
    pub fn get_parse_failure_summary(&self) -> Result<ParseFailureSummary> {
        let mut stmt = self.conn.prepare(
            "SELECT timestamp, raw_command, error_message, fallback_succeeded
             FROM parse_failures
             ORDER BY timestamp DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ParseFailureRecord {
                timestamp: row.get(0)?,
                raw_command: row.get(1)?,
                error_message: row.get(2)?,
                fallback_succeeded: row.get::<_, i32>(3)? != 0,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            let record = row?;
            if !should_suppress_parse_failure_record(&record.raw_command, record.fallback_succeeded)
            {
                records.push(record);
            }
        }

        let total = records.len();
        let succeeded = records
            .iter()
            .filter(|record| record.fallback_succeeded)
            .count();
        let unrecovered = total.saturating_sub(succeeded);

        let recovery_rate = if total > 0 {
            (succeeded as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for record in &records {
            *counts.entry(record.raw_command.clone()).or_insert(0) += 1;
        }

        let mut top_commands = counts.into_iter().collect::<Vec<_>>();
        top_commands.sort_by(|(left_cmd, left_count), (right_cmd, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_cmd.cmp(right_cmd))
        });
        top_commands.truncate(10);

        let unrecovered_records = records
            .iter()
            .filter(|record| !record.fallback_succeeded)
            .cloned()
            .collect::<Vec<_>>();
        let mut unrecovered_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for record in &unrecovered_records {
            *unrecovered_counts
                .entry(record.raw_command.clone())
                .or_insert(0) += 1;
        }

        let mut top_unrecovered_commands = unrecovered_counts.into_iter().collect::<Vec<_>>();
        top_unrecovered_commands.sort_by(|(left_cmd, left_count), (right_cmd, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_cmd.cmp(right_cmd))
        });
        top_unrecovered_commands.truncate(10);
        let recent_unrecovered = unrecovered_records.into_iter().take(10).collect();

        Ok(ParseFailureSummary {
            total,
            recovered: succeeded,
            unrecovered,
            recovery_rate,
            top_commands,
            top_unrecovered_commands,
            recent_unrecovered,
        })
    }
}

/// Record a parse failure without ever crashing.
/// Silently ignores all errors - used in the fallback path.
pub fn record_parse_failure_silent(raw_command: &str, error_message: &str, succeeded: bool) {
    if let Ok(tracker) = Tracker::new() {
        let _ = tracker.record_parse_failure(raw_command, error_message, succeeded);
    }
}

fn should_suppress_parse_failure_record(raw_command: &str, fallback_succeeded: bool) -> bool {
    if !fallback_succeeded {
        return false;
    }
    is_assistant_housekeeping_command(raw_command)
}

pub(super) fn is_assistant_housekeeping_command(raw_command: &str) -> bool {
    let normalized = raw_command.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("/.claude/statusline.js")
        || normalized.contains("/.claude/hooks/")
        || normalized.contains("/.claude/scripts/localhost-registry.js")
        || normalized.starts_with("bash -c file=\"$claude_file_path\";")
        || normalized.starts_with("bash -c if echo \"$claude_file_path\"")
}
