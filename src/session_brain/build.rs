use super::evidence::{build_session_focus, SessionEvidence, SessionFocus};
use super::guidance::build_guidance;
use super::messages::{load_context_snapshot_messages, read_current_session_messages};
use super::project::build_project_context;
use super::strategy::build_strategy_context;
use super::types::{
    SessionBrain, SessionBrainAgenda, SessionBrainMessageGroup, SessionBrainMeta,
    SessionBrainSignal, SessionBrainState, SessionBrainStrategyContext,
};
use super::user::build_user_context;
use crate::core::tracking::Tracker;
use crate::core::utils::{current_project_root_string, truncate};
use crate::core::worldview::{CompiledContext, FailureFact};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct SessionBrainBuildOptions {
    pub explicit_goal: Option<String>,
    pub allow_session_fallback: bool,
}

#[derive(Debug, Clone, Default)]
struct TaskPath {
    paths: Vec<String>,
    terms: Vec<String>,
}

impl TaskPath {
    fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.terms.is_empty()
    }

    fn add_hint(&mut self, hint: &str) {
        if looks_like_path_hint(hint) {
            let normalized = normalize_hint_path(hint);
            if !normalized.is_empty() && !self.paths.contains(&normalized) {
                self.paths.push(normalized);
            }
        }
        self.add_text(hint);
    }

    fn add_text(&mut self, text: &str) {
        for term in extract_match_terms(text) {
            if !self.terms.contains(&term) {
                self.terms.push(term);
            }
        }
    }

    fn matches_text(&self, text: &str) -> bool {
        let lowered = text.to_ascii_lowercase();
        if self
            .paths
            .iter()
            .any(|path| lowered.contains(&path.to_ascii_lowercase()))
        {
            return true;
        }
        let overlap = self
            .terms
            .iter()
            .filter(|term| lowered.contains(term.as_str()))
            .count();
        overlap >= required_overlap(self.terms.len())
    }
}

pub fn build_session_brain(
    tracker: &Tracker,
    compiled: &CompiledContext,
    failures: &[FailureFact],
    options: &SessionBrainBuildOptions,
) -> Result<SessionBrain> {
    let project_root = if compiled.project_path.is_empty() {
        PathBuf::from(current_project_root_string())
    } else {
        PathBuf::from(&compiled.project_path)
    };
    let messages = read_current_session_messages(&project_root, options.allow_session_fallback)?;
    let mut focus_user_messages = messages.user.clone();
    focus_user_messages.extend(load_context_snapshot_messages(
        &project_root,
        &messages.user,
    )?);
    let focus = build_session_focus(&focus_user_messages, &messages.assistant);
    let user = build_user_context(tracker);
    let strategy =
        build_strategy_context(&project_root).unwrap_or_else(|_| SessionBrainStrategyContext {
            summary: Vec::new(),
            source_paths: Vec::new(),
            planning_complete: false,
        });
    let guidance = build_guidance(&user);
    let agenda = build_agenda(compiled, &focus, &strategy.summary, options);
    let task_path = build_task_path(agenda.current_goal.as_deref(), &focus, compiled);
    let state = build_state(compiled, failures, &focus, &task_path);
    let project = build_project_context(
        &project_root,
        agenda.current_goal.as_deref(),
        &focus.ordered_task_hints(),
    )?;
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(SessionBrain {
        meta: SessionBrainMeta {
            session_id: messages.session_id.clone(),
            cwd,
            project_root: compiled.project_path.clone(),
            built_at: Utc::now().to_rfc3339(),
            version: 1,
            provider: messages.provider,
            transcript_source_path: messages.transcript_path.clone(),
            transcript_modified_at: messages.transcript_modified_at.clone(),
            source_status: messages.source_status.clone(),
        },
        messages: SessionBrainMessageGroup {
            user: messages.user,
            assistant: messages.assistant,
        },
        agenda,
        state,
        project,
        strategy,
        user,
        guidance,
    })
}

fn build_agenda(
    compiled: &CompiledContext,
    focus: &SessionFocus,
    strategy_summary: &[String],
    options: &SessionBrainBuildOptions,
) -> SessionBrainAgenda {
    let suppress_machine_fallback = focus.suppresses_machine_fallback();
    let current_goal = options
        .explicit_goal
        .as_ref()
        .map(|goal| truncate(goal, 200))
        .or_else(|| focus.preferred_live_goal())
        .or_else(|| {
            focus
                .next_move_candidates
                .iter()
                .find(|item| item.role == "user")
                .map(|item| item.summary.clone())
        })
        .or_else(|| {
            focus
                .next_move_candidates
                .iter()
                .find(|item| item.role == "assistant")
                .map(|item| item.summary.clone())
        })
        .or_else(|| {
            (!suppress_machine_fallback)
                .then(|| dependency_linked_obligation(compiled, &focus.ordered_task_hints()))
                .flatten()
        })
        .or_else(|| {
            (!suppress_machine_fallback)
                .then(|| strategy_summary.first().map(|line| truncate(line, 200)))
                .flatten()
        });

    let mut subgoals = focus
        .next_move_candidates
        .iter()
        .filter(|item| item.role == "user")
        .filter_map(|item| {
            (Some(item.summary.as_str()) != current_goal.as_deref()).then(|| item.summary.clone())
        })
        .take(4)
        .collect::<Vec<_>>();
    if subgoals.is_empty() {
        subgoals.extend(
            focus
                .current_ask_candidates
                .iter()
                .skip(1)
                .take(3)
                .map(|item| item.summary.clone()),
        );
    }
    dedupe_strings(&mut subgoals);

    let redirects = focus
        .redirects
        .iter()
        .take(5)
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();

    let mut next_actions = focus
        .next_move_candidates
        .iter()
        .filter(|item| item.role == "user")
        .map(|item| item.summary.clone())
        .collect::<Vec<_>>();
    next_actions.extend(
        focus
            .next_move_candidates
            .iter()
            .filter(|item| item.role == "assistant")
            .map(|item| item.summary.clone()),
    );
    if next_actions.is_empty() && !suppress_machine_fallback {
        next_actions.extend(matching_obligations(compiled, &focus.ordered_task_hints()));
    }
    if next_actions.is_empty() {
        next_actions.extend(
            strategy_summary
                .iter()
                .take(2)
                .map(|line| truncate(line, 160)),
        );
    }
    dedupe_strings(&mut next_actions);

    SessionBrainAgenda {
        current_goal,
        subgoals,
        redirects,
        next_actions,
    }
}

fn build_state(
    compiled: &CompiledContext,
    failures: &[FailureFact],
    focus: &SessionFocus,
    task_path: &TaskPath,
) -> SessionBrainState {
    let mut decisions = focus
        .decisions
        .iter()
        .take(4)
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();
    if decisions.len() < 4 {
        decisions.extend(
            compiled
                .live_claims
                .iter()
                .filter(|claim| claim.claim_type.eq_ignore_ascii_case("decision"))
                .filter(|claim| task_path.is_empty() || task_path.matches_text(&claim.claim))
                .take(4 - decisions.len())
                .map(|claim| SessionBrainSignal {
                    summary: truncate(&claim.claim, 180),
                    source: format!("claim:{}", claim.claim_type),
                    timestamp: Some(claim.observed_at.clone()),
                    evidence: claim.dependencies.iter().take(3).cloned().collect(),
                }),
        );
    }
    dedupe_signals(&mut decisions);

    let mut findings = focus
        .findings
        .iter()
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();
    findings.extend(
        focus
            .resolved_blockers
            .iter()
            .map(SessionEvidence::to_signal)
            .take(2),
    );
    findings.extend(
        non_blocking_failure_signals(failures, task_path)
            .into_iter()
            .take(2),
    );
    if findings.len() < 4 {
        findings.extend(
            compiled
                .current_state
                .iter()
                .filter(|fact| task_path.is_empty() || task_path.matches_text(&fact.summary))
                .take(4 - findings.len())
                .map(|fact| SessionBrainSignal {
                    summary: truncate(&fact.summary, 180),
                    source: fact.event_type.clone(),
                    timestamp: Some(fact.observed_at.clone()),
                    evidence: fact.artifact_id.iter().cloned().collect::<Vec<_>>(),
                }),
        );
    }
    dedupe_signals(&mut findings);

    let mut blockers = focus
        .blockers
        .iter()
        .filter(|item| task_path.is_empty() || task_path.matches_text(&item.summary))
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();
    blockers.extend(
        failures
            .iter()
            .filter(|failure| failure_is_blocker(failure, task_path))
            .filter(|failure| {
                !focus
                    .resolved_blockers
                    .iter()
                    .any(|item| item.matches_text(&failure.summary))
            })
            .take(4)
            .map(|failure| SessionBrainSignal {
                summary: truncate(&failure.summary, 180),
                source: failure.event_type.clone(),
                timestamp: Some(failure.observed_at.clone()),
                evidence: failure.details.iter().take(2).cloned().collect(),
            }),
    );
    dedupe_signals(&mut blockers);

    let mut verified_facts = focus
        .verified_facts
        .iter()
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();
    if verified_facts.len() < 3 {
        verified_facts.extend(
            compiled
                .current_state
                .iter()
                .filter(|fact| fact_is_verification(&fact.summary))
                .filter(|fact| task_path.is_empty() || task_path.matches_text(&fact.summary))
                .take(3 - verified_facts.len())
                .map(|fact| SessionBrainSignal {
                    summary: truncate(&fact.summary, 180),
                    source: "verified-state".to_string(),
                    timestamp: Some(fact.observed_at.clone()),
                    evidence: fact.artifact_id.iter().cloned().collect::<Vec<_>>(),
                }),
        );
    }
    dedupe_signals(&mut verified_facts);

    let mut rejected_options = focus
        .rejections
        .iter()
        .take(4)
        .map(SessionEvidence::to_signal)
        .collect::<Vec<_>>();
    dedupe_signals(&mut rejected_options);

    SessionBrainState {
        decisions,
        findings,
        blockers,
        verified_facts,
        rejected_options,
    }
}

fn build_task_path(
    current_goal: Option<&str>,
    focus: &SessionFocus,
    compiled: &CompiledContext,
) -> TaskPath {
    let mut task_path = TaskPath::default();
    let suppress_machine_fallback = focus.suppresses_machine_fallback();

    if let Some(goal) = current_goal {
        task_path.add_text(goal);
    }

    for hint in focus.ordered_task_hints().iter().take(12) {
        task_path.add_hint(hint);
    }

    if task_path.is_empty() {
        for item in focus.current_ask_candidates.iter().take(3) {
            task_path.add_text(&item.summary);
        }
    }

    if task_path.is_empty() {
        for item in focus
            .next_move_candidates
            .iter()
            .filter(|item| item.role == "assistant")
            .take(2)
        {
            task_path.add_text(&item.summary);
        }
    }

    if task_path.is_empty() && !suppress_machine_fallback {
        for claim in compiled.open_obligations.iter().take(2) {
            task_path.add_text(&claim.claim);
            for dependency in &claim.dependencies {
                task_path.add_text(dependency);
            }
        }
    }

    task_path
}

fn matching_obligations(compiled: &CompiledContext, task_hints: &[String]) -> Vec<String> {
    let hints = task_hints
        .iter()
        .map(|hint| hint.to_ascii_lowercase())
        .collect::<Vec<_>>();
    compiled
        .open_obligations
        .iter()
        .filter(|claim| {
            hints.is_empty()
                || hints.iter().any(|hint| {
                    claim.claim.to_ascii_lowercase().contains(hint)
                        || claim
                            .dependencies
                            .iter()
                            .any(|dependency| dependency.to_ascii_lowercase().contains(hint))
                })
        })
        .take(3)
        .map(|claim| truncate(&claim.claim, 160))
        .collect()
}

fn dependency_linked_obligation(
    compiled: &CompiledContext,
    task_hints: &[String],
) -> Option<String> {
    matching_obligations(compiled, task_hints)
        .into_iter()
        .next()
        .or_else(|| {
            compiled
                .open_obligations
                .first()
                .map(|claim| truncate(&claim.claim, 200))
        })
}

fn non_blocking_failure_signals(
    failures: &[FailureFact],
    task_path: &TaskPath,
) -> Vec<SessionBrainSignal> {
    failures
        .iter()
        .filter(|failure| !failure_is_blocker(failure, task_path))
        .filter(|failure| {
            let combined = failure_text(failure);
            task_path.is_empty()
                || task_path.matches_text(&combined)
                || combined.contains("warning")
        })
        .take(2)
        .map(|failure| SessionBrainSignal {
            summary: truncate(&failure.summary, 180),
            source: failure.event_type.clone(),
            timestamp: Some(failure.observed_at.clone()),
            evidence: failure.details.iter().take(2).cloned().collect(),
        })
        .collect()
}

fn failure_is_blocker(failure: &FailureFact, task_path: &TaskPath) -> bool {
    let combined = failure_text(failure);
    if !task_path.is_empty() && !task_path.matches_text(&combined) {
        return false;
    }
    looks_like_blocker(&combined) && !combined.contains("warning")
}

fn failure_text(failure: &FailureFact) -> String {
    format!("{}\n{}", failure.summary, failure.details.join("\n")).to_ascii_lowercase()
}

fn looks_like_blocker(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    let tokens = lowered
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let blocker_token = tokens.iter().any(|token| {
        matches!(
            *token,
            "blocked" | "blocker" | "error" | "failed" | "panic" | "unable" | "cannot"
        )
    });
    blocker_token
        && !tokens
            .iter()
            .any(|token| *token == "warning" || *token == "warnings")
}

fn fact_is_verification(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("passed")
        || lowered.contains("verified")
        || lowered.contains("confirmed")
        || lowered.contains("ok")
}

fn dedupe_strings(items: &mut Vec<String>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.clone()));
}

fn dedupe_signals(items: &mut Vec<SessionBrainSignal>) {
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert(item.summary.clone()));
}

fn required_overlap(term_count: usize) -> usize {
    if term_count <= 3 {
        1
    } else {
        2
    }
}

fn normalize_hint_path(hint: &str) -> String {
    hint.trim_matches(|ch: char| matches!(ch, '`' | '"' | '\'' | '(' | ')' | '[' | ']'))
        .trim_end_matches('.')
        .replace('\\', "/")
}

fn looks_like_path_hint(hint: &str) -> bool {
    let lowered = hint.to_ascii_lowercase();
    lowered.contains("src/")
        || lowered.contains("tests/")
        || lowered.contains('\\')
        || lowered.ends_with(".rs")
        || lowered.ends_with(".md")
        || lowered.ends_with(".toml")
        || lowered.ends_with(".json")
}

fn extract_match_terms(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .filter_map(|token| {
            let lowered = token.to_ascii_lowercase();
            let keep = lowered.len() >= 4
                && !matches!(
                    lowered.as_str(),
                    "with"
                        | "from"
                        | "that"
                        | "this"
                        | "into"
                        | "then"
                        | "only"
                        | "will"
                        | "keep"
                        | "make"
                        | "have"
                        | "does"
                        | "current"
                        | "next"
                );
            keep.then_some(lowered)
        })
        .filter(|token| seen.insert(token.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::types::{SessionBrainMessage, SessionBrainProvider};
    use super::*;
    use crate::core::worldview::{ArtifactHandle, ContextClaim, ContextFact};

    fn sample_context() -> CompiledContext {
        CompiledContext {
            generated_at: "2026-04-15T00:00:00Z".to_string(),
            project_path: "C:/repo".to_string(),
            goal: None,
            current_state: vec![ContextFact {
                observed_at: "2026-04-15T00:00:00Z".to_string(),
                event_type: "git-status".to_string(),
                subject: "git-status:C:/repo".to_string(),
                status: "changed".to_string(),
                summary: "repo has the active parser patch".to_string(),
                command_sig: "context git status".to_string(),
                artifact_id: None,
            }],
            live_claims: vec![ContextClaim {
                observed_at: "2026-04-15T00:00:00Z".to_string(),
                claim_type: "decision".to_string(),
                confidence: "high".to_string(),
                claim: "Keep Session Brain in the context runtime instead of an OMX-only lane."
                    .to_string(),
                rationale_capsule: None,
                dependencies: vec!["user-decision:session-brain".to_string()],
                evidence: vec![],
            }],
            open_obligations: vec![ContextClaim {
                observed_at: "2026-04-15T00:00:00Z".to_string(),
                claim_type: "obligation".to_string(),
                confidence: "high".to_string(),
                claim: "Resolve the current failing auth test.".to_string(),
                rationale_capsule: None,
                dependencies: vec!["worldview:cargo-test:C:/repo".to_string()],
                evidence: vec![],
            }],
            auto_obligation_count: 0,
            recent_changes: vec![ContextFact {
                observed_at: "2026-04-15T00:00:00Z".to_string(),
                event_type: "diff".to_string(),
                subject: "diff:session-brain".to_string(),
                status: "changed".to_string(),
                summary: "session-brain module added".to_string(),
                command_sig: "context diff".to_string(),
                artifact_id: None,
            }],
            recent_commands: vec![],
            recent_command_input_tokens: 0,
            artifact_handles: vec![ArtifactHandle {
                artifact_id: "@context/a_test".to_string(),
                reopen_hint: "context show @context/a_test".to_string(),
            }],
            prompt: String::new(),
        }
    }

    fn failure(summary: &str, details: &[&str]) -> FailureFact {
        FailureFact {
            observed_at: "2026-04-15T00:00:00Z".to_string(),
            event_type: "cargo-build".to_string(),
            subject: "cargo-build:C:/repo".to_string(),
            summary: summary.to_string(),
            details: details.iter().map(|item| item.to_string()).collect(),
            artifact_id: None,
        }
    }

    fn message(role: &str, text: &str) -> SessionBrainMessage {
        SessionBrainMessage {
            role: role.to_string(),
            provider: SessionBrainProvider::Codex,
            session_id: Some("sess-1".to_string()),
            timestamp: Some("2026-04-15T00:00:00Z".to_string()),
            cwd: Some("C:/repo".to_string()),
            transcript_path: "C:/repo/session.jsonl".to_string(),
            record_type: "fixture".to_string(),
            line_number: 1,
            text: text.to_string(),
            source_kind: "root".to_string(),
        }
    }

    #[test]
    fn agenda_prefers_transcript_ask_over_worldview_obligation() {
        let focus = build_session_focus(
            &[message(
                "user",
                "Task Statement\nFix the Session Brain content so it reflects the actual session.",
            )],
            &[],
        );

        let agenda = build_agenda(
            &sample_context(),
            &focus,
            &[],
            &SessionBrainBuildOptions {
                explicit_goal: None,
                allow_session_fallback: false,
            },
        );

        assert_eq!(
            agenda.current_goal.as_deref(),
            Some("Fix the Session Brain content so it reflects the actual session.")
        );
    }

    #[test]
    fn dissatisfaction_suppresses_worldview_fallback_without_replacing_live_ask() {
        let focus = build_session_focus(
            &[message(
                "user",
                "That is not what I asked you to do in this session. None of this represents what I asked for.",
            )],
            &[],
        );

        let agenda = build_agenda(
            &sample_context(),
            &focus,
            &[],
            &SessionBrainBuildOptions::default(),
        );

        assert!(agenda.current_goal.is_none());
        assert!(agenda.next_actions.is_empty());
    }

    #[test]
    fn blockers_require_current_task_path_match() {
        let focus = build_session_focus(
            &[message(
                "user",
                "Task Statement\nFix the Session Brain content so it reflects the actual session.\nLikely Codebase Touchpoints\n- src/session_brain/build.rs",
            )],
            &[],
        );
        let task_path = build_task_path(
            Some("Fix the Session Brain content so it reflects the actual session."),
            &focus,
            &sample_context(),
        );
        let state = build_state(
            &sample_context(),
            &[
                failure(
                    "cargo build: 0 errors, 23 warnings",
                    &["warning: unused replay_shells"],
                ),
                failure(
                    "cargo build: 1 failed",
                    &["error in src/session_brain/build.rs: missing field"],
                ),
            ],
            &focus,
            &task_path,
        );

        assert_eq!(state.blockers.len(), 1);
        assert!(state.blockers[0].summary.contains("1 failed"));
        assert!(state
            .findings
            .iter()
            .any(|item| item.summary.contains("23 warnings")));
    }

    #[test]
    fn build_task_path_prefers_live_intent_before_worldview_seed() {
        let focus = build_session_focus(
            &[message(
                "user",
                "Task Statement\nShow me the session brain command for this session.",
            )],
            &[],
        );

        let task_path = build_task_path(None, &focus, &sample_context());

        assert!(task_path.matches_text("Show me the session brain command for this session."));
        assert!(!task_path.matches_text("Resolve the current failing auth test."));
    }

    #[test]
    fn next_actions_prefer_transcript_moves_before_worldview() {
        let focus = build_session_focus(
            &[message(
                "user",
                "Approved Plan\n1. Add a transient SessionFocus / SessionEvidence layer inside src/session_brain/.\n2. Rebuild agenda precedence.",
            )],
            &[message(
                "assistant",
                "Next I'll update the renderers after the normalization layer lands.",
            )],
        );

        let agenda = build_agenda(
            &sample_context(),
            &focus,
            &[],
            &SessionBrainBuildOptions::default(),
        );

        assert_eq!(
            agenda.next_actions.first().map(String::as_str),
            Some("Next I'll update the renderers after the normalization layer lands.")
        );
        assert!(agenda
            .next_actions
            .iter()
            .any(|item| item.contains("renderers")));
        assert!(!agenda
            .next_actions
            .iter()
            .any(|item| item.contains("SessionFocus / SessionEvidence")));
        assert!(!agenda
            .next_actions
            .iter()
            .any(|item| item.contains("failing auth test")));
    }

    #[test]
    fn resolved_blocker_clear_suppresses_matching_failure_fact() {
        let focus = build_session_focus(
            &[message(
                "user",
                "Task Statement\nFix Session Brain freshness.\nKnown Facts / Evidence\n- cargo build failed in src/session_brain/build.rs",
            )],
            &[message(
                "assistant",
                "Verified cargo build passed after fixing src/session_brain/build.rs.",
            )],
        );
        let task_path = build_task_path(
            Some("Fix Session Brain freshness."),
            &focus,
            &sample_context(),
        );

        let state = build_state(
            &sample_context(),
            &[FailureFact {
                event_type: "cargo-build".to_string(),
                subject: "src/session_brain/build.rs".to_string(),
                summary: "cargo build failed in src/session_brain/build.rs".to_string(),
                details: vec!["blocking".to_string()],
                observed_at: "2026-04-15T00:00:00Z".to_string(),
                artifact_id: None,
            }],
            &focus,
            &task_path,
        );

        assert!(state.blockers.is_empty());
        assert!(state
            .verified_facts
            .iter()
            .any(|item| item.summary.contains("cargo build passed")));
    }
}
