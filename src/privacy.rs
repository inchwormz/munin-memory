//! Explicit local data lifecycle for Munin-owned state.

use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIRMATION: &str = "FORGET-MUNIN";

#[derive(Debug, Serialize)]
pub struct PrivacyDatabase {
    pub path: String,
    pub exists: bool,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct PrivacyInspectReceipt {
    pub schema: &'static str,
    pub generated_at: String,
    pub data_root: String,
    pub config_root: String,
    pub database: PrivacyDatabase,
    pub retention_days: u32,
    pub owned_files: usize,
    pub owned_bytes: u64,
    pub source_transcripts_included: bool,
    pub sensitivity: &'static str,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrivacyExportFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct PrivacyExportReceipt {
    pub schema: &'static str,
    pub generated_at: String,
    pub output: String,
    pub database_snapshot: String,
    pub source_transcripts_included: bool,
    pub sensitivity: &'static str,
    pub files: Vec<PrivacyExportFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PrivacyRetentionReceipt {
    pub schema: &'static str,
    pub generated_at: String,
    pub days: u32,
    pub cutoff: String,
    pub applied: bool,
    pub tables: usize,
    pub found: usize,
    pub eligible: usize,
    pub removed: usize,
    pub kept: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PrivacyForgetReceipt {
    pub schema: &'static str,
    pub generated_at: String,
    pub scope: &'static str,
    pub project_path: Option<String>,
    pub found: usize,
    pub removed: usize,
    pub removed_files: usize,
    pub removed_bytes: u64,
    pub affected_tables: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn inspect() -> Result<PrivacyInspectReceipt> {
    let data_root = crate::core::config::context_data_dir()?;
    let config_root = crate::core::config::context_config_dir()?;
    let database_path = crate::tracking::history_database_path()?;
    let config = crate::core::config::Config::load().unwrap_or_default();
    let roots = owned_roots(&data_root, &config_root, &database_path);
    let (owned_files, owned_bytes) = inventory_paths(&roots)?;
    let database_bytes = fs::metadata(&database_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    Ok(PrivacyInspectReceipt {
        schema: "munin.privacy.inspect.v1",
        generated_at: Utc::now().to_rfc3339(),
        data_root: display_path(&data_root),
        config_root: display_path(&config_root),
        database: PrivacyDatabase {
            path: display_path(&database_path),
            exists: database_path.exists(),
            bytes: database_bytes,
        },
        retention_days: config.tracking.history_days,
        owned_files,
        owned_bytes,
        source_transcripts_included: false,
        sensitivity: "local-private",
        warnings: vec![
            "Munin exports can contain private prompts, paths, and derived memory; protect the output like the source profile.".to_string(),
            "Claude and Codex source transcripts are read in place and are not owned or deleted by Munin.".to_string(),
        ],
    })
}

pub fn export(output: &Path) -> Result<PrivacyExportReceipt> {
    let output = absolute_path(output)?;
    if output.exists() {
        bail!("privacy export output already exists: {}", output.display());
    }
    let data_root = absolute_path(&crate::core::config::context_data_dir()?)?;
    let config_root = absolute_path(&crate::core::config::context_config_dir()?)?;
    let database_path = absolute_path(&crate::tracking::history_database_path()?)?;
    if output.starts_with(&data_root) || output.starts_with(&config_root) {
        bail!("privacy export output must be outside Munin-owned data/config roots");
    }

    fs::create_dir_all(output.join("data"))?;
    if data_root.exists() {
        copy_tree(&data_root, &output.join("data"), Some(&database_path))?;
    }
    if config_root.exists() {
        copy_tree(&config_root, &output.join("config"), None)?;
    }

    let database_snapshot = output.join("data/history.db");
    if database_path.exists() {
        if database_snapshot.exists() {
            fs::remove_file(&database_snapshot)?;
        }
        let conn = Connection::open(&database_path)
            .with_context(|| format!("failed to open {}", database_path.display()))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute(
            "VACUUM INTO ?1",
            params![database_snapshot.to_string_lossy().to_string()],
        )
        .context("failed to create consistent SQLite export snapshot")?;
    }

    let mut files = manifest_files(&output)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let receipt = PrivacyExportReceipt {
        schema: "munin.privacy.export.v1",
        generated_at: Utc::now().to_rfc3339(),
        output: display_path(&output),
        database_snapshot: if database_snapshot.exists() {
            "data/history.db".to_string()
        } else {
            String::new()
        },
        source_transcripts_included: false,
        sensitivity: "local-private",
        files,
        warnings: vec![
            "The export is not encrypted by Munin; store or transmit it only through an encrypted channel.".to_string(),
        ],
    };
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    Ok(receipt)
}

pub fn retention(days: u32, apply: bool) -> Result<PrivacyRetentionReceipt> {
    if !(1..=3650).contains(&days) {
        bail!("retention days must be between 1 and 3650");
    }
    let database_path = crate::tracking::history_database_path()?;
    let cutoff = Utc::now() - Duration::days(i64::from(days));
    let mut found = 0usize;
    let mut eligible = 0usize;
    let mut removed = 0usize;
    let mut kept = 0usize;
    let mut table_count = 0usize;

    if database_path.exists() {
        let mut conn = Connection::open(&database_path)?;
        let timestamped = timestamped_tables(&conn)?;
        table_count = timestamped.len();
        for (table, column) in &timestamped {
            let total = count_where(&conn, table, None, None)?;
            let old = count_where(&conn, table, Some(column), Some(&cutoff.to_rfc3339()))?;
            found += total;
            eligible += old;
        }
        if apply {
            let transaction = conn.transaction()?;
            for (table, column) in &timestamped {
                let sql = format!(
                    "DELETE FROM {} WHERE julianday({}) < julianday(?1)",
                    quote_identifier(table),
                    quote_identifier(column)
                );
                removed += transaction.execute(&sql, [cutoff.to_rfc3339()])?;
            }
            transaction.commit()?;
        }
        kept = found.saturating_sub(if apply { removed } else { eligible });
    }

    if apply {
        let mut config = crate::core::config::Config::load().unwrap_or_default();
        config.tracking.history_days = days;
        config.save()?;
    }
    Ok(PrivacyRetentionReceipt {
        schema: "munin.privacy.retention.v1",
        generated_at: Utc::now().to_rfc3339(),
        days,
        cutoff: cutoff.to_rfc3339(),
        applied: apply,
        tables: table_count,
        found,
        eligible,
        removed,
        kept,
        warnings: if apply {
            vec!["Retention removes compiled Munin rows only; source transcripts remain under their host's control.".to_string()]
        } else {
            vec![
                "Preview only; pass --apply to update config and delete eligible compiled rows."
                    .to_string(),
            ]
        },
    })
}

pub fn forget(
    project: Option<&str>,
    all: bool,
    confirmation: &str,
) -> Result<PrivacyForgetReceipt> {
    if confirmation != CONFIRMATION {
        bail!("forget requires --confirm {CONFIRMATION}");
    }
    match (
        project.map(str::trim).filter(|value| !value.is_empty()),
        all,
    ) {
        (Some(project), false) => forget_project(project),
        (None, true) => forget_all(),
        _ => bail!("choose exactly one of --project <exact-path> or --all"),
    }
}

pub fn inspect_text(receipt: &PrivacyInspectReceipt) -> String {
    format!(
        "Munin Privacy Inspect\n---------------------\nData: {}\nConfig: {}\nDatabase: {} ({} bytes)\nRetention: {} days\nOwned: {} files / {} bytes\nSource transcripts included: no",
        receipt.data_root,
        receipt.config_root,
        receipt.database.path,
        receipt.database.bytes,
        receipt.retention_days,
        receipt.owned_files,
        receipt.owned_bytes
    )
}

pub fn export_text(receipt: &PrivacyExportReceipt) -> String {
    format!(
        "Munin Privacy Export\n--------------------\nOutput: {}\nFiles: {}\nDatabase snapshot: {}\nSensitivity: {}",
        receipt.output,
        receipt.files.len(),
        receipt.database_snapshot,
        receipt.sensitivity
    )
}

pub fn retention_text(receipt: &PrivacyRetentionReceipt) -> String {
    format!(
        "Munin Privacy Retention\n-----------------------\nDays: {}\nApplied: {}\nFound: {}\nEligible: {}\nRemoved: {}\nKept: {}",
        receipt.days,
        receipt.applied,
        receipt.found,
        receipt.eligible,
        receipt.removed,
        receipt.kept
    )
}

pub fn forget_text(receipt: &PrivacyForgetReceipt) -> String {
    format!(
        "Munin Privacy Forget\n--------------------\nScope: {}\nFound: {}\nRemoved rows: {}\nRemoved files: {}\nRemoved bytes: {}",
        receipt.scope,
        receipt.found,
        receipt.removed,
        receipt.removed_files,
        receipt.removed_bytes
    )
}

fn forget_project(project: &str) -> Result<PrivacyForgetReceipt> {
    let database_path = crate::tracking::history_database_path()?;
    let mut affected_tables = Vec::new();
    let mut found = 0usize;
    let mut removed = 0usize;
    if database_path.exists() {
        let mut conn = Connection::open(&database_path)?;
        let tables = project_tables(&conn)?;
        for table in &tables {
            let sql = format!(
                "SELECT COUNT(*) FROM {} WHERE project_path = ?1",
                quote_identifier(table)
            );
            let count: i64 = conn.query_row(&sql, [project], |row| row.get(0))?;
            found += count.max(0) as usize;
        }
        let transaction = conn.transaction()?;
        for table in &tables {
            let sql = format!(
                "DELETE FROM {} WHERE project_path = ?1",
                quote_identifier(table)
            );
            let count = transaction.execute(&sql, [project])?;
            if count > 0 {
                affected_tables.push(table.clone());
                removed += count;
            }
        }
        transaction.commit()?;
    }
    Ok(PrivacyForgetReceipt {
        schema: "munin.privacy.forget.v1",
        generated_at: Utc::now().to_rfc3339(),
        scope: "project",
        project_path: Some(project.to_string()),
        found,
        removed,
        removed_files: 0,
        removed_bytes: 0,
        affected_tables,
        warnings: vec![
            "Project forget removes exact project_path rows from compiled Munin state; host source transcripts are unchanged.".to_string(),
        ],
    })
}

fn forget_all() -> Result<PrivacyForgetReceipt> {
    let data_root = absolute_path(&crate::core::config::context_data_dir()?)?;
    let config_root = absolute_path(&crate::core::config::context_config_dir()?)?;
    let database_path = absolute_path(&crate::tracking::history_database_path()?)?;
    let roots = owned_roots(&data_root, &config_root, &database_path);
    for root in &roots {
        ensure_safe_forget_path(root)?;
    }
    let (removed_files, removed_bytes) = inventory_paths(&roots)?;
    for root in roots.iter().rev() {
        if root.is_dir() {
            fs::remove_dir_all(root)
                .with_context(|| format!("failed to remove Munin directory {}", root.display()))?;
        } else if root.is_file() {
            fs::remove_file(root)
                .with_context(|| format!("failed to remove Munin file {}", root.display()))?;
        }
    }
    Ok(PrivacyForgetReceipt {
        schema: "munin.privacy.forget.v1",
        generated_at: Utc::now().to_rfc3339(),
        scope: "all",
        project_path: None,
        found: removed_files,
        removed: 0,
        removed_files,
        removed_bytes,
        affected_tables: Vec::new(),
        warnings: vec![
            "All resolved Munin-owned data/config paths were removed; Claude and Codex source transcripts were not touched.".to_string(),
        ],
    })
}

fn timestamped_tables(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for table in tables {
        let columns = table_columns(conn, &table)?;
        if let Some(column) = ["timestamp", "observed_at", "created_at", "generated_at"]
            .iter()
            .find(|candidate| columns.contains(**candidate))
        {
            result.push((table, (*column).to_string()));
        }
    }
    Ok(result)
}

fn project_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for table in tables {
        if table_columns(conn, &table)?.contains("project_path") {
            result.push(table);
        }
    }
    Ok(result)
}

fn table_columns(conn: &Connection, table: &str) -> Result<BTreeSet<String>> {
    let sql = format!("PRAGMA table_info({})", quote_identifier(table));
    let mut statement = conn.prepare(&sql)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    Ok(columns)
}

fn count_where(
    conn: &Connection,
    table: &str,
    timestamp_column: Option<&str>,
    cutoff: Option<&str>,
) -> Result<usize> {
    let count: i64 = if let (Some(column), Some(cutoff)) = (timestamp_column, cutoff) {
        let sql = format!(
            "SELECT COUNT(*) FROM {} WHERE julianday({}) < julianday(?1)",
            quote_identifier(table),
            quote_identifier(column)
        );
        conn.query_row(&sql, [cutoff], |row| row.get(0))?
    } else {
        let sql = format!("SELECT COUNT(*) FROM {}", quote_identifier(table));
        conn.query_row(&sql, [], |row| row.get(0))?
    };
    Ok(count.max(0) as usize)
}

fn owned_roots(data_root: &Path, config_root: &Path, database_path: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![data_root.to_path_buf(), config_root.to_path_buf()];
    if !database_path.starts_with(data_root) && !database_path.starts_with(config_root) {
        candidates.push(database_path.to_path_buf());
    }
    candidates.sort_by_key(|path| path.components().count());
    let mut roots = Vec::<PathBuf>::new();
    for candidate in candidates {
        if !roots.iter().any(|root| candidate.starts_with(root)) {
            roots.push(candidate);
        }
    }
    roots
}

fn inventory_paths(paths: &[PathBuf]) -> Result<(usize, u64)> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    for path in paths {
        if path.is_file() {
            files += 1;
            bytes += fs::metadata(path)?.len();
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).follow_links(false) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    files += 1;
                    bytes += entry.metadata()?.len();
                }
            }
        }
    }
    Ok((files, bytes))
}

fn copy_tree(source: &Path, destination: &Path, skip: Option<&Path>) -> Result<()> {
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if path == source {
            continue;
        }
        if let Some(skip) = skip {
            if path == skip || is_sqlite_transient_for(path, skip) {
                continue;
            }
        }
        let relative = path.strip_prefix(source)?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}

fn is_sqlite_transient_for(path: &Path, database_path: &Path) -> bool {
    let database = database_path.to_string_lossy();
    let candidate = path.to_string_lossy();
    candidate == format!("{database}-wal") || candidate == format!("{database}-shm")
}

fn manifest_files(root: &Path) -> Result<Vec<PrivacyExportFile>> {
    let mut result = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() == "manifest.json" {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let relative = entry.path().strip_prefix(root)?;
        result.push(PrivacyExportFile {
            path: relative.to_string_lossy().replace('\\', "/"),
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(bytes)),
        });
    }
    Ok(result)
}

fn ensure_safe_forget_path(path: &Path) -> Result<()> {
    let checked_path = if path.exists() {
        path.canonicalize()
            .with_context(|| format!("failed to resolve forget path {}", path.display()))?
    } else {
        path.to_path_buf()
    };
    if checked_path.as_os_str().is_empty()
        || checked_path.parent().is_none()
        || checked_path.file_name().is_none()
    {
        bail!("refusing dangerous forget path: {}", checked_path.display());
    }
    let current = absolute_path(&std::env::current_dir()?)?;
    let current = current.canonicalize().unwrap_or(current);
    if checked_path == current || current.starts_with(&checked_path) {
        bail!("refusing to forget the current workspace or one of its parents");
    }
    if let Some(home) = dirs::home_dir() {
        let home = absolute_path(&home)?;
        let home = home.canonicalize().unwrap_or(home);
        if checked_path == home || home.starts_with(&checked_path) {
            bail!("refusing to forget the home directory or one of its parents");
        }
    }
    Ok(())
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
