use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const HOOK_RECEIPT_SCHEMA: &str = "munin.agent-hook-receipt.v1";
const STATUS_SCHEMA: &str = "munin.agent-hook-status.v1";
const FRESH_FOR_SECONDS: i64 = 300;
const LEASE_STALE_AFTER_SECONDS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum HookFormat {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
pub struct SessionStartReceipt {
    pub schema: &'static str,
    pub status: String,
    pub generated_at: String,
    pub last_refresh_at: Option<String>,
    pub age_seconds: Option<u64>,
    pub refresh_queued: bool,
    pub lease_active: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WorkerStatus {
    schema: String,
    status: String,
    updated_at: String,
    last_refresh_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RefreshLease {
    started_at: String,
    pid: u32,
}

pub fn session_start() -> Result<SessionStartReceipt> {
    let root = hook_root()?;
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let now = Utc::now();
    let status_path = root.join("status.json");
    let lease_path = root.join("refresh.lease.json");
    let previous = read_worker_status(&status_path)?;
    let (last_refresh_at, age_seconds) = freshness(&previous, now);
    let is_fresh = age_seconds.is_some_and(|age| age <= FRESH_FOR_SECONDS as u64);
    let mut warning = previous
        .as_ref()
        .and_then(|status| status.last_error.clone());
    let mut lease_active = false;

    if lease_path.exists() {
        match read_lease(&lease_path) {
            Ok(lease) if lease_age_seconds(&lease, now) <= LEASE_STALE_AFTER_SECONDS => {
                lease_active = true;
            }
            Ok(_) => {
                fs::remove_file(&lease_path)
                    .with_context(|| format!("failed to recover stale {}", lease_path.display()))?;
                warning = Some("recovered stale refresh lease".to_string());
            }
            Err(error) => {
                fs::remove_file(&lease_path).with_context(|| {
                    format!("failed to remove invalid {}", lease_path.display())
                })?;
                warning = Some(format!("recovered invalid refresh lease: {error}"));
            }
        }
    }

    let mut refresh_queued = false;
    if !is_fresh && !lease_active {
        acquire_lease(&lease_path, now)?;
        match spawn_refresh_worker() {
            Ok(()) => {
                refresh_queued = true;
                lease_active = true;
            }
            Err(error) => {
                let _ = fs::remove_file(&lease_path);
                return Err(error);
            }
        }
    }

    let status = if previous
        .as_ref()
        .is_some_and(|state| state.status == "fail")
    {
        "refresh-failed"
    } else if is_fresh {
        "current"
    } else if refresh_queued {
        "refresh-queued"
    } else if lease_active {
        "refresh-running"
    } else {
        "stale"
    };
    Ok(SessionStartReceipt {
        schema: HOOK_RECEIPT_SCHEMA,
        status: status.to_string(),
        generated_at: now.to_rfc3339(),
        last_refresh_at,
        age_seconds,
        refresh_queued,
        lease_active,
        warning,
    })
}

pub fn render_session_start(receipt: &SessionStartReceipt, format: HookFormat) -> Result<()> {
    match format {
        HookFormat::Json => println!("{}", serde_json::to_string_pretty(receipt)?),
        HookFormat::Text => {
            println!(
                "Munin memory refresh: {} (queued={}, age={}s)",
                receipt.status,
                receipt.refresh_queued,
                receipt
                    .age_seconds
                    .map(|age| age.to_string())
                    .as_deref()
                    .unwrap_or("unknown")
            );
            if let Some(warning) = &receipt.warning {
                eprintln!("warning: {warning}");
            }
        }
    }
    Ok(())
}

pub fn refresh_worker() -> Result<()> {
    let root = hook_root()?;
    fs::create_dir_all(&root)?;
    let status_path = root.join("status.json");
    let lease_path = root.join("refresh.lease.json");
    let previous = read_worker_status(&status_path)?;
    let running = WorkerStatus {
        schema: STATUS_SCHEMA.to_string(),
        status: "running".to_string(),
        updated_at: Utc::now().to_rfc3339(),
        last_refresh_at: previous
            .as_ref()
            .and_then(|status| status.last_refresh_at.clone()),
        last_error: None,
    };
    write_json_atomically(&status_path, &running)?;

    let refresh_result = run_incremental_refresh();
    let finished = Utc::now();
    let final_status = match &refresh_result {
        Ok(()) => WorkerStatus {
            schema: STATUS_SCHEMA.to_string(),
            status: "pass".to_string(),
            updated_at: finished.to_rfc3339(),
            last_refresh_at: Some(finished.to_rfc3339()),
            last_error: None,
        },
        Err(error) => WorkerStatus {
            schema: STATUS_SCHEMA.to_string(),
            status: "fail".to_string(),
            updated_at: finished.to_rfc3339(),
            last_refresh_at: previous
                .as_ref()
                .and_then(|status| status.last_refresh_at.clone()),
            last_error: Some(format!("{error:#}")),
        },
    };
    let status_write = write_json_atomically(&status_path, &final_status);
    let lease_remove = if lease_path.exists() {
        fs::remove_file(&lease_path)
            .with_context(|| format!("failed to clear {}", lease_path.display()))
    } else {
        Ok(())
    };
    status_write?;
    lease_remove?;
    refresh_result
}

fn run_incremental_refresh() -> Result<()> {
    if std::env::var_os("MUNIN_HOOK_WORKER_TEST_FAIL").is_some() {
        bail!("forced hook worker failure");
    }
    let executable = std::env::current_exe().context("failed to locate Munin executable")?;
    let output = Command::new(executable)
        .args(["memory-os", "ingest", "--check-now", "--format", "json"])
        .stdin(Stdio::null())
        .output()
        .context("failed to start incremental memory refresh")?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "incremental memory refresh exited {:?}: {}",
        output.status.code(),
        stderr.trim()
    )
}

fn spawn_refresh_worker() -> Result<()> {
    let executable = std::env::current_exe().context("failed to locate Munin executable")?;
    let mut command = Command::new(executable);
    command
        .args(["hook", "refresh-worker"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .context("failed to queue detached Munin refresh worker")?;
    Ok(())
}

fn hook_root() -> Result<PathBuf> {
    Ok(crate::core::config::context_data_dir()?.join("agent-hook"))
}

fn acquire_lease(path: &Path, now: DateTime<Utc>) -> Result<()> {
    let lease = RefreshLease {
        started_at: now.to_rfc3339(),
        pid: std::process::id(),
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to acquire refresh lease {}", path.display()))?;
    file.write_all(&serde_json::to_vec(&lease)?)?;
    file.sync_all()?;
    Ok(())
}

fn read_lease(path: &Path) -> Result<RefreshLease> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn lease_age_seconds(lease: &RefreshLease, now: DateTime<Utc>) -> i64 {
    DateTime::parse_from_rfc3339(&lease.started_at)
        .map(|started| (now - started.with_timezone(&Utc)).num_seconds().max(0))
        .unwrap_or(i64::MAX)
}

fn read_worker_status(path: &Path) -> Result<Option<WorkerStatus>> {
    if !path.exists() {
        return Ok(None);
    }
    let status: WorkerStatus = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    if status.schema != STATUS_SCHEMA {
        return Err(anyhow!("unsupported hook status schema {}", status.schema));
    }
    Ok(Some(status))
}

fn freshness(status: &Option<WorkerStatus>, now: DateTime<Utc>) -> (Option<String>, Option<u64>) {
    let last = status
        .as_ref()
        .and_then(|status| status.last_refresh_at.as_deref());
    let Some(last) = last else {
        return (None, None);
    };
    match DateTime::parse_from_rfc3339(last) {
        Ok(parsed) => {
            let age = (now - parsed.with_timezone(&Utc)).num_seconds().max(0) as u64;
            (Some(last.to_string()), Some(age))
        }
        Err(_) => (None, None),
    }
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&temp, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", temp.display()))?;
    fs::rename(&temp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}
