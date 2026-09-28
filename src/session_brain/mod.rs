mod build;
mod evidence;
mod guidance;
mod messages;
pub(crate) mod project;
mod strategy;
mod types;
mod user;

pub use build::{build_session_brain, SessionBrainBuildOptions};
#[allow(unused_imports)]
pub use types::{
    SessionBrain, SessionBrainAgenda, SessionBrainGuidance, SessionBrainMessage,
    SessionBrainMessageGroup, SessionBrainMeta, SessionBrainProjectContext, SessionBrainProvider,
    SessionBrainSignal, SessionBrainState, SessionBrainStrategyContext, SessionBrainUserContext,
};

use crate::core::tracking::Tracker;
use crate::core::utils::{current_project_root_string, truncate};
use crate::core::worldview;
use crate::runtime_context::{packet_from_session_brain, render_packet, RuntimeContextRenderMode};
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionBrainRenderMode {
    Text,
    Json,
    Prompt,
}

impl SessionBrainRenderMode {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "prompt" => Ok(Self::Prompt),
            other => Err(anyhow!(
                "unsupported format '{}' (expected text, json, or prompt)",
                other
            )),
        }
    }
}

pub fn run_inspect_current(format: &str, _verbose: u8) -> Result<()> {
    crate::analytics::session_backfill::refresh_memory_os_session_import_before_read()?;
    let brain = build_current_session_brain()?;
    println!(
        "{}",
        render_session_brain(&brain, SessionBrainRenderMode::parse(format)?)?
    );
    Ok(())
}

pub fn build_current_session_brain() -> Result<SessionBrain> {
    let tracker = Tracker::new()?;
    let project_path = current_project_root_string();
    let (compiled, failures) = worldview::compile_context_packet_source_with_tracker(
        &tracker,
        &project_path,
        None,
        12,
        8,
        6,
    )?;
    let compiled = build::SessionBrainCompiledInput::from(&compiled);
    let brain = build_session_brain(
        &tracker,
        &compiled,
        &failures,
        &SessionBrainBuildOptions {
            explicit_goal: None,
            allow_session_fallback: true,
        },
    )?;
    Ok(brain)
}

pub fn current_source_status() -> Option<String> {
    let project_path = current_project_root_string();
    let project_root = std::path::Path::new(&project_path);
    messages::read_current_session_messages(project_root, true)
        .ok()
        .map(|messages| messages.source_status)
}

pub fn render_session_brain(brain: &SessionBrain, mode: SessionBrainRenderMode) -> Result<String> {
    Ok(match mode {
        SessionBrainRenderMode::Text => render_text(brain),
        SessionBrainRenderMode::Json => render_packet(
            &packet_from_session_brain(brain),
            RuntimeContextRenderMode::Json,
        )?,
        SessionBrainRenderMode::Prompt => render_prompt(brain),
    })
}

fn render_text(brain: &SessionBrain) -> String {
    if brain.meta.source_status != "live" {
        let packet = packet_from_session_brain(brain);
        return render_packet(&packet, RuntimeContextRenderMode::Text)
            .unwrap_or_else(|_| "Runtime context render failed.".to_string());
    }

    let mut lines = vec![
        format!("Session Brain [{}]", brain.meta.provider.as_str()),
        format!("project: {}", brain.meta.project_root),
    ];
    if let Some(session_id) = brain.meta.session_id.as_deref() {
        lines.push(format!("session: {}", session_id));
    }
    lines.push(format!("session source: {}", brain.meta.source_status));
    if brain.meta.source_status != "live" && brain.meta.source_status != "none" {
        lines.push(format!(
            "freshness warning: Session Brain is reading {} context, not the live window.",
            brain.meta.source_status
        ));
    }
    if let Some(goal) = brain.agenda.current_goal.as_deref() {
        lines.push(format!("current ask: {}", goal));
    }
    if !brain.agenda.redirects.is_empty() {
        lines.push("redirects:".to_string());
        for redirect in brain.agenda.redirects.iter().take(3) {
            lines.push(format!("- {}", redirect.summary));
        }
    }
    if !brain.state.decisions.is_empty() {
        lines.push("decisions:".to_string());
        for decision in brain.state.decisions.iter().take(3) {
            lines.push(format!("- {}", decision.summary));
        }
    }
    if !brain.state.findings.is_empty() {
        lines.push("findings:".to_string());
        for finding in brain.state.findings.iter().take(3) {
            lines.push(format!("- {}", finding.summary));
        }
    }
    if !brain.state.blockers.is_empty() {
        lines.push("blockers:".to_string());
        for blocker in brain.state.blockers.iter().take(3) {
            lines.push(format!("- {}", blocker.summary));
        }
    }
    if !brain.project.summary.is_empty() {
        lines.push("project capsule:".to_string());
        for line in brain.project.summary.iter().take(4) {
            lines.push(format!("- {}", line));
        }
    }
    lines.push("strategy:".to_string());
    if brain.strategy.summary.is_empty() {
        lines.push("- no active strategy artifact matched this project".to_string());
    } else {
        for line in brain.strategy.summary.iter().take(3) {
            lines.push(format!("- {}", line));
        }
    }
    lines.push("user operating model:".to_string());
    if !brain.user.brief.is_empty() {
        lines.push(format!("- brief: {}", brain.user.brief));
    }
    if !brain.user.overview.is_empty() {
        lines.push(format!("- overview: {}", brain.user.overview));
    }
    if !brain.user.profile.is_empty() {
        lines.push(format!("- profile: {}", brain.user.profile));
    }
    if !brain.user.friction.is_empty() {
        lines.push(format!("- friction: {}", brain.user.friction));
    }
    if !brain.agenda.next_actions.is_empty() {
        lines.push("next actions:".to_string());
        for action in brain.agenda.next_actions.iter().take(3) {
            lines.push(format!("- {}", action));
        }
    }
    lines.join("\n")
}

fn render_prompt(brain: &SessionBrain) -> String {
    if brain.meta.source_status != "live" {
        let packet = packet_from_session_brain(brain);
        return render_packet(&packet, RuntimeContextRenderMode::Prompt)
            .unwrap_or_else(|_| "<runtime_context_v1 error=\"render_failed\" />".to_string());
    }

    let mut lines = vec![format!(
        "<session_brain provider=\"{}\" session_id=\"{}\" built_at=\"{}\" source_status=\"{}\">",
        brain.meta.provider.as_str(),
        brain.meta.session_id.as_deref().unwrap_or("none"),
        brain.meta.built_at,
        brain.meta.source_status
    )];
    lines.push("<agenda>".to_string());
    if let Some(goal) = brain.agenda.current_goal.as_deref() {
        lines.push(format!("- current ask: {}", goal));
    }
    if let Some(latest) = brain
        .messages
        .user
        .last()
        .map(|message| message.text.trim())
    {
        if !latest.is_empty()
            && !brain
                .agenda
                .current_goal
                .as_deref()
                .is_some_and(|goal| latest == goal || latest.ends_with(goal))
        {
            lines.push(format!(
                "- latest user direction: {}",
                truncate(latest, 700)
            ));
        }
    }
    if let Some(previous) = brain
        .messages
        .user
        .iter()
        .rev()
        .skip(1)
        .map(|message| message.text.trim())
        .find(|text| {
            text.len() >= 30
                && !text.starts_with('#')
                && !text.starts_with('<')
                && !brain
                    .agenda
                    .current_goal
                    .as_deref()
                    .is_some_and(|goal| goal.contains(text) || text.ends_with(goal))
        })
    {
        lines.push(format!(
            "- recent user requirement: {}",
            truncate(previous, 500)
        ));
    }
    for subgoal in brain.agenda.subgoals.iter().take(4) {
        lines.push(format!("- subgoal: {}", subgoal));
    }
    for redirect in brain.agenda.redirects.iter().take(3) {
        lines.push(format!("- redirect: {}", redirect.summary));
    }
    for action in brain.agenda.next_actions.iter().take(4) {
        lines.push(format!("- next: {}", action));
    }
    lines.push("</agenda>".to_string());

    let state_start = lines.len();
    lines.push("<state>".to_string());
    for signal in brain.state.decisions.iter().take(3) {
        lines.push(format!("- decision: {}", signal.summary));
    }
    for signal in brain.state.findings.iter().take(3) {
        lines.push(format!("- finding: {}", signal.summary));
    }
    for signal in brain.state.blockers.iter().take(3) {
        lines.push(format!("- blocker: {}", signal.summary));
    }
    for signal in brain.state.verified_facts.iter().take(2) {
        lines.push(format!("- verified: {}", signal.summary));
    }
    for signal in brain.state.rejected_options.iter().take(2) {
        lines.push(format!("- rejected: {}", signal.summary));
    }
    if lines.len() == state_start + 1 {
        lines.pop();
    } else {
        lines.push("</state>".to_string());
    }

    let project_start = lines.len();
    lines.push("<project>".to_string());
    for line in brain.project.summary.iter().take(5) {
        if !line.starts_with("Active ask:") {
            lines.push(format!("- {}", line));
        }
    }
    if !brain.project.priority_notes.is_empty() {
        lines.push(format!(
            "- priority notes: {}",
            truncate(&brain.project.priority_notes, 220)
        ));
    }
    if lines.len() == project_start + 1 {
        lines.pop();
    } else {
        lines.push("</project>".to_string());
    }

    if !brain.strategy.summary.is_empty() {
        lines.push("<strategy>".to_string());
        for line in brain.strategy.summary.iter().take(5) {
            lines.push(format!("- {}", line));
        }
        lines.push("</strategy>".to_string());
    }

    let user_start = lines.len();
    lines.push("<user_operating_model>".to_string());
    if !brain.user.brief.is_empty() {
        lines.push(format!("- brief: {}", brain.user.brief));
    }
    if !brain.user.overview.is_empty() {
        lines.push(format!("- overview: {}", brain.user.overview));
    }
    if !brain.user.profile.is_empty() {
        lines.push(format!("- profile: {}", brain.user.profile));
    }
    if lines.len() == user_start + 1 {
        lines.pop();
    } else {
        lines.push("</user_operating_model>".to_string());
    }

    lines.push("</session_brain>".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_brain() -> SessionBrain {
        SessionBrain {
            meta: SessionBrainMeta {
                session_id: Some("sess-1".to_string()),
                cwd: "C:/repo".to_string(),
                project_root: "C:/repo".to_string(),
                built_at: "2026-04-15T00:00:00Z".to_string(),
                version: 1,
                provider: SessionBrainProvider::Codex,
                transcript_source_path: Some("C:/repo/session.jsonl".to_string()),
                transcript_modified_at: Some("2026-04-15T00:00:00Z".to_string()),
                source_status: "live".to_string(),
            },
            messages: SessionBrainMessageGroup {
                user: (0..8)
                    .map(|index| SessionBrainMessage {
                        role: "user".to_string(),
                        provider: SessionBrainProvider::Codex,
                        session_id: Some("sess-1".to_string()),
                        timestamp: Some("2026-04-15T00:00:00Z".to_string()),
                        cwd: Some("C:/repo".to_string()),
                        transcript_path: "C:/repo/session.jsonl".to_string(),
                        record_type: "fixture".to_string(),
                        line_number: index + 1,
                        text: format!("user message {}", index),
                        source_kind: "root".to_string(),
                    })
                    .collect(),
                assistant: (0..5)
                    .map(|index| SessionBrainMessage {
                        role: "assistant".to_string(),
                        provider: SessionBrainProvider::Codex,
                        session_id: Some("sess-1".to_string()),
                        timestamp: Some("2026-04-15T00:00:00Z".to_string()),
                        cwd: Some("C:/repo".to_string()),
                        transcript_path: "C:/repo/session.jsonl".to_string(),
                        record_type: "fixture".to_string(),
                        line_number: index + 1,
                        text: format!("assistant message {}", index),
                        source_kind: "root".to_string(),
                    })
                    .collect(),
            },
            agenda: SessionBrainAgenda {
                current_goal: Some("Fix session brain semantics".to_string()),
                subgoals: vec!["Rebuild transcript precedence".to_string()],
                redirects: vec![SessionBrainSignal {
                    summary: "Actually use the real session ask first.".to_string(),
                    source: "user-message".to_string(),
                    timestamp: None,
                    evidence: vec![],
                }],
                next_actions: vec!["Update renderers".to_string()],
            },
            state: SessionBrainState {
                decisions: vec![SessionBrainSignal {
                    summary: "Keep transcript evidence ahead of worldview noise.".to_string(),
                    source: "user-message".to_string(),
                    timestamp: None,
                    evidence: vec![],
                }],
                findings: vec![SessionBrainSignal {
                    summary: "Current output misses the active ask.".to_string(),
                    source: "assistant-message".to_string(),
                    timestamp: None,
                    evidence: vec![],
                }],
                blockers: vec![SessionBrainSignal {
                    summary: "session_brain build is currently broken".to_string(),
                    source: "cargo-build".to_string(),
                    timestamp: None,
                    evidence: vec![],
                }],
                verified_facts: Vec::new(),
                rejected_options: Vec::new(),
            },
            project: SessionBrainProjectContext {
                summary: vec![
                    "context: Local CLI proxy and context compiler".to_string(),
                    "Active ask: Fix session brain semantics".to_string(),
                ],
                key_files: vec!["src/session_brain/build.rs".to_string()],
                codebase_map: "top-level: src".to_string(),
                project_memory: None,
                priority_notes: String::new(),
            },
            strategy: SessionBrainStrategyContext {
                summary: vec!["Continuity: Session brain fix is active.".to_string()],
                source_paths: Vec::new(),
                planning_complete: true,
            },
            user: SessionBrainUserContext {
                brief: "Execution-heavy workflow".to_string(),
                overview: String::new(),
                profile: "Prefers current-session signals before broad search.".to_string(),
                friction: "Avoid treating old goals as current.".to_string(),
            },
            guidance: SessionBrainGuidance {
                retrieval_hints: vec!["Start from current-session messages.".to_string()],
                avoid: vec!["Raw transcript dumps.".to_string()],
            },
        }
    }

    #[test]
    fn text_renderer_includes_strategy_and_user_sections() {
        let rendered = render_text(&sample_brain());

        assert!(rendered.contains("strategy:"));
        assert!(rendered.contains("user operating model:"));
        assert!(rendered.contains("current ask: Fix session brain semantics"));
    }

    #[test]
    fn prompt_renderer_keeps_latest_direction_without_dumping_transcript() {
        let rendered = render_prompt(&sample_brain());

        assert!(rendered.contains("- current ask: Fix session brain semantics"));
        assert!(rendered.contains("<project>"));
        assert!(rendered.contains("<strategy>"));
        assert!(rendered.contains("<user_operating_model>"));
        assert!(!rendered.contains("<messages>"));
        assert!(rendered.contains("latest user direction: user message 7"));
        assert!(!rendered.contains("user message 0"));
        assert!(!rendered.contains("assistant message"));
        assert!(!rendered.contains("C:/repo/session.jsonl"));
        assert!(!rendered.contains("<guidance>"));
    }

    #[test]
    fn prompt_renderer_retains_recent_requirement_before_a_deadline() {
        let mut brain = sample_brain();
        let template = brain.messages.user[0].clone();
        brain.messages.user = [
            "# AGENTS.md instructions\n<INSTRUCTIONS>Ignore this as a goal</INSTRUCTIONS>",
            "Keep the remembered context fresh and remove old frustrations.",
            "Please finish this work within 45 minutes.",
        ]
        .into_iter()
        .map(|text| SessionBrainMessage {
            text: text.to_string(),
            ..template.clone()
        })
        .collect();
        let rendered = render_prompt(&brain);
        assert!(rendered.contains("latest user direction: Please finish"));
        assert!(rendered.contains("recent user requirement: Keep the remembered context fresh"));
        assert!(!rendered.contains("AGENTS.md instructions"));
    }

    #[test]
    fn json_renderer_uses_public_runtime_packet_without_raw_transcript() {
        let rendered =
            render_session_brain(&sample_brain(), SessionBrainRenderMode::Json).expect("render");

        assert!(rendered.contains("\"version\""));
        assert!(!rendered.contains("C:/repo/session.jsonl"));
        assert!(!rendered.contains("user message"));
        assert!(!rendered.contains("transcriptSourcePath"));
    }

    #[test]
    fn non_live_prompt_renderer_redirects_to_runtime_context_packet() {
        let mut brain = sample_brain();
        brain.meta.source_status = "fallback-latest".to_string();

        let rendered = render_prompt(&brain);

        assert!(rendered.contains("<runtime_context_v1"));
        assert!(rendered.contains("<redirect>"));
        assert!(rendered.contains("munin resume --format prompt"));
        assert!(!rendered.contains("<session_brain"));
        assert!(!rendered.contains("- current ask: Fix session brain semantics"));
    }
}
