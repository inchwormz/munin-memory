use crate::core::proactivity;
use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProactivityFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct ProactivityRunRequest {
    pub scope: Option<String>,
    pub provider: Option<crate::core::config::ProactivityProvider>,
    pub dry_run: bool,
    pub auto_spawn: bool,
    pub no_spawn: bool,
    pub format: ProactivityFormat,
}

#[derive(Debug, Clone)]
pub struct ProactivityScopeRequest {
    pub scope: Option<String>,
    pub format: ProactivityFormat,
}

#[derive(Debug, Clone)]
pub struct ProactivityScheduleInstallRequest {
    pub scope: Option<String>,
    pub provider: Option<crate::core::config::ProactivityProvider>,
    pub project_path: Option<PathBuf>,
    pub format: ProactivityFormat,
}

#[derive(Debug, Clone)]
pub struct ProactivityClaimRequest {
    pub job_id: String,
    pub format: ProactivityFormat,
}

#[derive(Debug, Clone)]
pub struct ProactivityApproveRequest {
    pub job_id: String,
    pub no_spawn: bool,
    pub format: ProactivityFormat,
}

#[derive(Debug, Clone)]
pub struct ProactivityCompleteRequest {
    pub job_id: String,
    pub status: proactivity::ProactivityTerminalStatus,
    pub summary: String,
    pub error: Option<String>,
    pub notes: Vec<String>,
    pub format: ProactivityFormat,
}

pub fn run(request: ProactivityRunRequest) -> Result<()> {
    let report = proactivity::run(&proactivity::ProactivityRunOptions {
        scope: request.scope,
        provider: request.provider,
        dry_run: request.dry_run,
        auto_spawn: request.auto_spawn,
        no_spawn: request.no_spawn,
    })?;
    render_response(&report, request.format)
}

pub fn sweep(request: ProactivityScopeRequest) -> Result<()> {
    let report = proactivity::sweep(&proactivity::ProactivityScopeOptions {
        scope: request.scope,
    })?;
    render_response(&report, request.format)
}

pub fn status(request: ProactivityScopeRequest) -> Result<()> {
    let report = proactivity::status(&proactivity::ProactivityScopeOptions {
        scope: request.scope,
    })?;
    render_response(&report, request.format)
}

pub fn schedule_install(request: ProactivityScheduleInstallRequest) -> Result<()> {
    let report = proactivity::install_schedule(&proactivity::ProactivityScheduleInstallOptions {
        scope: request.scope,
        provider: request.provider,
        project_path: request.project_path,
    })?;
    render_response(&report, request.format)
}

pub fn schedule_remove(request: ProactivityScopeRequest) -> Result<()> {
    let report = proactivity::remove_schedule(&proactivity::ProactivityScopeOptions {
        scope: request.scope,
    })?;
    render_response(&report, request.format)
}

pub fn claim(request: ProactivityClaimRequest) -> Result<()> {
    let report = proactivity::claim(&proactivity::ProactivityClaimOptions {
        job_id: request.job_id,
    })?;
    render_response(&report, request.format)
}

pub fn approve(request: ProactivityApproveRequest) -> Result<()> {
    let report = proactivity::approve(&proactivity::ProactivityApproveOptions {
        job_id: request.job_id,
        no_spawn: request.no_spawn,
    })?;
    render_response(&report, request.format)
}

pub fn complete(request: ProactivityCompleteRequest) -> Result<()> {
    let report = proactivity::complete(&proactivity::ProactivityCompleteOptions {
        job_id: request.job_id,
        status: request.status,
        summary: request.summary,
        error: request.error,
        notes: request.notes,
    })?;
    render_response(&report, request.format)
}

fn render_response<T: Serialize>(report: &T, format: ProactivityFormat) -> Result<()> {
    match format {
        ProactivityFormat::Text => println!("{}", serde_json::to_string_pretty(report)?),
        ProactivityFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}
