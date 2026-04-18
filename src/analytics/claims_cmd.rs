//! Explicit validated claim lease management.

use crate::core::tracking::{
    ClaimLeaseConfidence, ClaimLeaseDependency, ClaimLeaseDependencyKind, ClaimLeaseStatus,
    ClaimLeaseType, Tracker, UserDecisionRecord,
};
use crate::core::worldview;
use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimOutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Serialize)]
struct ClaimListItem {
    id: i64,
    timestamp: String,
    claim_type: String,
    confidence: String,
    status: String,
    claim: String,
    rationale_capsule: Option<String>,
    scope_key: Option<String>,
    dependencies: Vec<String>,
    evidence: Vec<String>,
    source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimSuggestion {
    claim_type: String,
    confidence: String,
    claim: String,
    rationale: String,
    scope_key: Option<String>,
    worldview_dependencies: Vec<String>,
    artifact_dependencies: Vec<String>,
    evidence: Vec<String>,
}

pub fn add(
    claim_type: ClaimLeaseType,
    claim: &str,
    rationale_capsule: Option<&str>,
    confidence: ClaimLeaseConfidence,
    scope_key: Option<&str>,
    worldview_dependencies: &[String],
    artifact_dependencies: &[String],
    user_decisions: &[String],
    evidence: &[String],
) -> Result<()> {
    let mut dependencies = Vec::new();
    for subject in worldview_dependencies {
        dependencies.push(ClaimLeaseDependency {
            kind: ClaimLeaseDependencyKind::WorldviewSubject,
            key: subject.clone(),
            fingerprint: None,
        });
    }
    for artifact in artifact_dependencies {
        dependencies.push(ClaimLeaseDependency {
            kind: ClaimLeaseDependencyKind::Artifact,
            key: artifact.clone(),
            fingerprint: None,
        });
    }
    for decision in user_decisions {
        dependencies.push(ClaimLeaseDependency {
            kind: ClaimLeaseDependencyKind::UserDecision,
            key: decision.clone(),
            fingerprint: None,
        });
    }

    if dependencies.is_empty() {
        return Err(anyhow!(
            "claim leases need at least one --worldview, --artifact, or --user-decision dependency"
        ));
    }

    let mut evidence_items = evidence.to_vec();
    for artifact in artifact_dependencies {
        if !evidence_items.iter().any(|item| item == artifact) {
            evidence_items.push(artifact.clone());
        }
    }

    let tracker = Tracker::new()?;
    let claim_id = tracker.create_claim_lease(
        claim_type,
        claim,
        rationale_capsule,
        confidence,
        scope_key,
        &dependencies,
        &serde_json::to_string(&evidence_items)?,
        "context claim add",
    )?;

    println!(
        "Created claim lease #{} [{}:{}] {}",
        claim_id, claim_type, confidence, claim
    );
    Ok(())
}

pub fn list(limit: usize, include_all_statuses: bool, format: ClaimOutputFormat) -> Result<()> {
    let tracker = Tracker::new()?;
    let project_path = current_project_path_string();
    tracker.refresh_claim_lease_statuses(Some(&project_path))?;
    let statuses = if include_all_statuses {
        None
    } else {
        Some(&[ClaimLeaseStatus::Live][..])
    };
    let claims = tracker.get_claim_leases_filtered(limit, Some(&project_path), statuses)?;
    let items = claims
        .into_iter()
        .map(|claim| ClaimListItem {
            id: claim.id,
            timestamp: claim.timestamp.to_rfc3339(),
            claim_type: claim.claim_type.to_string(),
            confidence: claim.confidence.to_string(),
            status: claim.status.to_string(),
            claim: claim.claim_text,
            rationale_capsule: claim.rationale_capsule,
            scope_key: claim.scope_key,
            dependencies: claim
                .dependencies
                .into_iter()
                .map(|dependency| match dependency.kind {
                    ClaimLeaseDependencyKind::WorldviewSubject => {
                        format!("worldview:{}", dependency.key)
                    }
                    ClaimLeaseDependencyKind::Artifact => format!("artifact:{}", dependency.key),
                    ClaimLeaseDependencyKind::UserDecision => {
                        format!("user-decision:{}", dependency.key)
                    }
                })
                .collect(),
            evidence: serde_json::from_str(&claim.evidence_json).unwrap_or_default(),
            source_kind: claim.source_kind,
        })
        .collect::<Vec<_>>();

    match format {
        ClaimOutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        ClaimOutputFormat::Text => {
            println!("{}", render_claims_text(&items));
        }
    }

    Ok(())
}

pub fn supersede(id: i64) -> Result<()> {
    let tracker = Tracker::new()?;
    let project_path = current_project_path_string();
    if tracker.supersede_claim_lease_for_project(&project_path, id)? {
        println!("Superseded claim lease #{}", id);
        Ok(())
    } else {
        Err(anyhow!(
            "claim lease #{} was not found in the current project",
            id
        ))
    }
}

pub fn set_user_decision(key: &str, value: &str) -> Result<()> {
    let tracker = Tracker::new()?;
    let id = tracker.set_user_decision(key, value)?;
    println!("Recorded user decision #{} [{}] {}", id, key, value);
    Ok(())
}

pub fn list_user_decisions(limit: usize, format: ClaimOutputFormat) -> Result<()> {
    let tracker = Tracker::new()?;
    let project_path = current_project_path_string();
    let decisions = tracker.get_user_decisions_filtered(limit, Some(&project_path))?;
    match format {
        ClaimOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &decisions
                        .iter()
                        .map(|decision| serde_json::json!({
                            "key": decision.key,
                            "value": decision.value_text,
                            "fingerprint": decision.fingerprint,
                            "updated_at": decision.updated_at.to_rfc3339(),
                        }))
                        .collect::<Vec<_>>()
                )?
            );
        }
        ClaimOutputFormat::Text => {
            println!("{}", render_user_decisions_text(&decisions));
        }
    }
    Ok(())
}

pub fn suggest(limit: usize, apply: bool, format: ClaimOutputFormat) -> Result<()> {
    let failures = worldview::collect_failures(limit)?;
    let suggestions = failures
        .into_iter()
        .take(limit)
        .map(|failure| {
            let mut evidence = failure.details.clone();
            let mut artifact_dependencies = Vec::new();
            if let Some(artifact_id) = &failure.artifact_id {
                artifact_dependencies.push(artifact_id.clone());
                if !evidence.iter().any(|item| item == artifact_id) {
                    evidence.push(artifact_id.clone());
                }
            }
            ClaimSuggestion {
                claim_type: ClaimLeaseType::Obligation.to_string(),
                confidence: ClaimLeaseConfidence::Medium.to_string(),
                claim: format!("Resolve active failure: {}", failure.summary),
                rationale: format!(
                    "Current worldview reports an active {} failure. Capture it as an obligation once ownership/scope is agreed.",
                    failure.event_type
                ),
                scope_key: Some(format!("failure:{}", failure.event_type)),
                worldview_dependencies: vec![failure.subject],
                artifact_dependencies,
                evidence,
            }
        })
        .collect::<Vec<_>>();

    if apply {
        return apply_suggestions(&suggestions);
    }

    match format {
        ClaimOutputFormat::Json => println!("{}", serde_json::to_string_pretty(&suggestions)?),
        ClaimOutputFormat::Text => println!("{}", render_suggestions_text(&suggestions)),
    }
    Ok(())
}

fn render_claims_text(items: &[ClaimListItem]) -> String {
    let mut lines = vec!["Claim Leases".to_string(), "------------".to_string()];
    if items.is_empty() {
        lines.push("No claim leases for this project.".to_string());
        return lines.join("\n");
    }

    for item in items {
        lines.push(format!(
            "- #{} [{}][{}:{}] {}",
            item.id, item.status, item.claim_type, item.confidence, item.claim
        ));
        if let Some(scope_key) = &item.scope_key {
            lines.push(format!("  scope: {}", scope_key));
        }
        if !item.dependencies.is_empty() {
            lines.push(format!("  deps: {}", item.dependencies.join(", ")));
        }
        if !item.evidence.is_empty() {
            lines.push(format!("  evidence: {}", item.evidence.join(", ")));
        }
        if let Some(rationale_capsule) = &item.rationale_capsule {
            lines.push(format!("  rationale: {}", rationale_capsule));
        }
    }

    lines.join("\n")
}

fn render_user_decisions_text(items: &[UserDecisionRecord]) -> String {
    let mut lines = vec!["User Decisions".to_string(), "--------------".to_string()];
    if items.is_empty() {
        lines.push("No user decisions for this project.".to_string());
        return lines.join("\n");
    }

    for item in items {
        lines.push(format!("- [{}] {}", item.key, item.value_text));
    }

    lines.join("\n")
}

fn render_suggestions_text(items: &[ClaimSuggestion]) -> String {
    let mut lines = vec![
        "Claim Suggestions".to_string(),
        "-----------------".to_string(),
    ];
    if items.is_empty() {
        lines.push("No obvious claim suggestions for this project.".to_string());
        return lines.join("\n");
    }

    for item in items {
        lines.push(format!(
            "- [{}:{}] {}",
            item.claim_type, item.confidence, item.claim
        ));
        lines.push(format!("  rationale: {}", item.rationale));
        if let Some(scope_key) = &item.scope_key {
            lines.push(format!("  scope: {}", scope_key));
        }
        if !item.worldview_dependencies.is_empty() {
            lines.push(format!(
                "  worldview deps: {}",
                item.worldview_dependencies.join(", ")
            ));
        }
        if !item.artifact_dependencies.is_empty() {
            lines.push(format!(
                "  artifact deps: {}",
                item.artifact_dependencies.join(", ")
            ));
        }
        if !item.evidence.is_empty() {
            lines.push(format!("  evidence: {}", item.evidence.join(", ")));
        }
    }

    lines.join("\n")
}

fn apply_suggestions(items: &[ClaimSuggestion]) -> Result<()> {
    let tracker = Tracker::new()?;
    let project_path = current_project_path_string();
    tracker.refresh_claim_lease_statuses(Some(&project_path))?;
    let existing = tracker.get_claim_leases_filtered(
        200,
        Some(&project_path),
        Some(&[ClaimLeaseStatus::Live]),
    )?;

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    for item in items {
        let worldview_dependencies = item
            .worldview_dependencies
            .iter()
            .map(|subject| ClaimLeaseDependency {
                kind: ClaimLeaseDependencyKind::WorldviewSubject,
                key: subject.clone(),
                fingerprint: None,
            })
            .collect::<Vec<_>>();
        let artifact_dependencies = item
            .artifact_dependencies
            .iter()
            .map(|artifact| ClaimLeaseDependency {
                kind: ClaimLeaseDependencyKind::Artifact,
                key: artifact.clone(),
                fingerprint: None,
            })
            .collect::<Vec<_>>();
        let dependencies = worldview_dependencies
            .iter()
            .chain(artifact_dependencies.iter())
            .cloned()
            .collect::<Vec<_>>();

        if has_matching_live_claim(&existing, item, &dependencies) {
            skipped.push(item.claim.clone());
            continue;
        }

        let id = tracker.create_claim_lease(
            ClaimLeaseType::Obligation,
            &item.claim,
            Some(&item.rationale),
            ClaimLeaseConfidence::Medium,
            item.scope_key.as_deref(),
            &dependencies,
            &serde_json::to_string(&item.evidence)?,
            "context claim suggest --apply",
        )?;
        created.push((id, item.claim.clone()));
    }

    println!("Applied claim suggestions");
    println!("------------------------");
    println!("Created: {}", created.len());
    for (id, claim) in &created {
        println!("- #{} {}", id, claim);
    }
    println!("Skipped existing: {}", skipped.len());
    for claim in &skipped {
        println!("- {}", claim);
    }

    Ok(())
}

fn has_matching_live_claim(
    existing: &[crate::core::tracking::ClaimLeaseRecord],
    suggestion: &ClaimSuggestion,
    dependencies: &[ClaimLeaseDependency],
) -> bool {
    existing.iter().any(|record| {
        record.claim_text == suggestion.claim
            && record.scope_key.as_deref() == suggestion.scope_key.as_deref()
            && record.dependencies == dependencies
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::tracking::{
        ClaimLeaseConfidence, ClaimLeaseDependency, ClaimLeaseDependencyKind, ClaimLeaseType,
    };

    #[test]
    fn render_suggestions_shows_artifact_dependencies() {
        let text = render_suggestions_text(&[ClaimSuggestion {
            claim_type: "obligation".to_string(),
            confidence: "medium".to_string(),
            claim: "Resolve active failure: cargo test failed".to_string(),
            rationale: "Need follow-up".to_string(),
            scope_key: Some("failure:cargo-test".to_string()),
            worldview_dependencies: vec!["cargo-test:C:/repo".to_string()],
            artifact_dependencies: vec!["@context/a_deadbeef".to_string()],
            evidence: vec!["@context/a_deadbeef".to_string()],
        }]);

        assert!(text.contains("artifact deps: @context/a_deadbeef"));
        assert!(text.contains("scope: failure:cargo-test"));
    }

    #[test]
    fn matching_live_claim_detects_duplicate_dependency_set() {
        let existing = crate::core::tracking::ClaimLeaseRecord {
            id: 1,
            timestamp: chrono::Utc::now(),
            project_path: "C:/repo".to_string(),
            claim_type: ClaimLeaseType::Obligation,
            claim_text: "Resolve active failure: cargo test failed".to_string(),
            rationale_capsule: None,
            confidence: ClaimLeaseConfidence::Medium,
            status: crate::core::tracking::ClaimLeaseStatus::Live,
            scope_key: Some("failure:cargo-test".to_string()),
            dependencies: vec![
                ClaimLeaseDependency {
                    kind: ClaimLeaseDependencyKind::WorldviewSubject,
                    key: "cargo-test:C:/repo".to_string(),
                    fingerprint: None,
                },
                ClaimLeaseDependency {
                    kind: ClaimLeaseDependencyKind::Artifact,
                    key: "@context/a_deadbeef".to_string(),
                    fingerprint: None,
                },
            ],
            dependency_fingerprint: String::new(),
            evidence_json: "[]".to_string(),
            source_kind: "test".to_string(),
            review_after: None,
            expires_at: None,
            last_reviewed_at: None,
            demotion_reason: None,
        };

        let suggestion = ClaimSuggestion {
            claim_type: "obligation".to_string(),
            confidence: "medium".to_string(),
            claim: "Resolve active failure: cargo test failed".to_string(),
            rationale: "Need follow-up".to_string(),
            scope_key: Some("failure:cargo-test".to_string()),
            worldview_dependencies: vec!["cargo-test:C:/repo".to_string()],
            artifact_dependencies: vec!["@context/a_deadbeef".to_string()],
            evidence: vec!["@context/a_deadbeef".to_string()],
        };

        assert!(has_matching_live_claim(
            &[existing],
            &suggestion,
            &[
                ClaimLeaseDependency {
                    kind: ClaimLeaseDependencyKind::WorldviewSubject,
                    key: "cargo-test:C:/repo".to_string(),
                    fingerprint: None,
                },
                ClaimLeaseDependency {
                    kind: ClaimLeaseDependencyKind::Artifact,
                    key: "@context/a_deadbeef".to_string(),
                    fingerprint: None,
                },
            ]
        ));
    }
}

fn current_project_path_string() -> String {
    crate::core::utils::current_project_root_string()
}
