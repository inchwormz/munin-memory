use super::types::SessionBrainStrategyContext;
use crate::core::config::{Config, StrategyScopeConfig};
use crate::core::strategy::{self, StrategyReadOptions};
use crate::core::utils::{detect_project_root, normalize_windows_path_string};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn build_strategy_context(project_root: &Path) -> Result<SessionBrainStrategyContext> {
    let config = Config::load().context("failed to load config.toml")?;
    let Some(scope_id) = resolve_relevant_scope(&config, project_root) else {
        return Ok(SessionBrainStrategyContext {
            summary: Vec::new(),
            source_paths: Vec::new(),
            planning_complete: false,
        });
    };

    let status = strategy::status(&StrategyReadOptions {
        scope: scope_id.clone(),
    })?;
    let recommend = strategy::recommend(&StrategyReadOptions { scope: scope_id }).ok();

    let mut summary = Vec::new();
    if status.continuity.active {
        if let Some(detail) = status.continuity.summary.as_deref() {
            summary.push(format!("Continuity: {detail}"));
        } else {
            summary.push("Continuity is active for this project.".to_string());
        }
    }
    for item in status.items.iter().take(3) {
        summary.push(format!(
            "{} [{}]: {}",
            item.item_kind, item.status, item.title
        ));
    }
    if let Some(report) = recommend {
        for nudge in report.nudges.iter().take(2) {
            summary.push(format!("Next: {}", nudge.task));
        }
    }

    let mut source_paths = BTreeSet::new();
    source_paths.insert(normalize_windows_path_string(
        status.registry.artifact_path.to_string_lossy().as_ref(),
    ));
    source_paths.insert(normalize_windows_path_string(
        status.registry.metrics_path.to_string_lossy().as_ref(),
    ));
    source_paths.insert(normalize_windows_path_string(
        status.registry.storage_dir.to_string_lossy().as_ref(),
    ));
    if let Some(path) = status.registry.continuity_project_path.as_ref() {
        source_paths.insert(normalize_windows_path_string(
            path.to_string_lossy().as_ref(),
        ));
    }
    for path in &status.registry.signal_paths {
        source_paths.insert(normalize_windows_path_string(
            path.to_string_lossy().as_ref(),
        ));
    }

    Ok(SessionBrainStrategyContext {
        summary,
        source_paths: source_paths.into_iter().collect(),
        planning_complete: !status.items.is_empty(),
    })
}

fn resolve_relevant_scope(config: &Config, project_root: &Path) -> Option<String> {
    let project_root = normalized_project_root(project_root);
    config.strategy.scopes.iter().find_map(|(scope_id, scope)| {
        if !scope.enabled {
            return None;
        }
        scope_matches_project(scope, &project_root).then(|| scope_id.clone())
    })
}

fn scope_matches_project(scope: &StrategyScopeConfig, project_root: &str) -> bool {
    if let Some(path) = scope.continuity_project_path.as_deref() {
        return normalized_project_root(path) == project_root;
    }
    if let Some(path) = scope.artifact_path.as_deref() {
        return path_in_project(path, project_root);
    }
    false
}

fn path_in_project(path: &Path, project_root: &str) -> bool {
    let normalized = normalize_windows_path_string(path.to_string_lossy().as_ref());
    normalized.starts_with(project_root)
}

fn normalized_project_root(path: &Path) -> String {
    normalize_windows_path_string(detect_project_root(path).to_string_lossy().as_ref())
}

#[allow(dead_code)]
fn as_path_buf(path: &Path) -> PathBuf {
    path.to_path_buf()
}
