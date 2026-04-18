use super::types::SessionBrainUserContext;
use crate::core::memory_os::{
    MemoryOsFrictionReport, MemoryOsInspectionScope, MemoryOsNarrativeFinding,
    MemoryOsOverviewReport, MemoryOsProfileReport,
};
use crate::core::tracking::Tracker;
use crate::core::utils::truncate;

pub fn build_user_context(tracker: &Tracker) -> SessionBrainUserContext {
    let scope = MemoryOsInspectionScope::User;
    let overview = tracker.get_memory_os_overview_report(scope, None).ok();
    let profile = tracker.get_memory_os_profile_report(scope, None).ok();
    let friction = tracker.get_memory_os_friction_report(scope, None).ok();

    SessionBrainUserContext {
        brief: build_brief_line(overview.as_ref(), profile.as_ref()),
        overview: build_overview_line(overview.as_ref()),
        profile: build_profile_line(profile.as_ref()),
        friction: build_friction_line(friction.as_ref()),
    }
}

fn build_brief_line(
    overview: Option<&MemoryOsOverviewReport>,
    profile: Option<&MemoryOsProfileReport>,
) -> String {
    let mut parts = Vec::new();
    if let Some(overview) = overview {
        parts.extend(overview.active_work.iter().take(2).map(finding_to_line));
    }
    if let Some(profile) = profile {
        parts.extend(profile.operating_style.iter().take(1).map(finding_to_line));
    }
    truncate(&parts.join(" | "), 320)
}

fn build_overview_line(overview: Option<&MemoryOsOverviewReport>) -> String {
    let Some(overview) = overview else {
        return String::new();
    };
    let mut parts = overview
        .active_work
        .iter()
        .take(3)
        .map(finding_to_line)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts.extend(overview.top_projects.iter().take(2).map(|project| {
            format!(
                "{}: {} sessions / {} shell executions",
                project.repo_label, project.sessions, project.shell_executions
            )
        }));
    }
    truncate(&parts.join(" | "), 360)
}

fn build_profile_line(profile: Option<&MemoryOsProfileReport>) -> String {
    let Some(profile) = profile else {
        return String::new();
    };
    let parts = profile
        .operating_style
        .iter()
        .chain(profile.preferences.iter())
        .take(4)
        .map(finding_to_line)
        .collect::<Vec<_>>();
    truncate(&parts.join(" | "), 360)
}

fn build_friction_line(friction: Option<&MemoryOsFrictionReport>) -> String {
    let Some(friction) = friction else {
        return String::new();
    };
    let mut parts = friction
        .behavior_changes
        .iter()
        .take(2)
        .map(|change| format!("{}: {}", change.target_agent, change.change))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts.extend(
            friction
                .likely_misunderstandings
                .iter()
                .take(2)
                .map(|item| format!("{}: {}", item.label, item.summary)),
        );
    }
    truncate(&parts.join(" | "), 360)
}

fn finding_to_line(finding: &MemoryOsNarrativeFinding) -> String {
    format!("{}: {}", finding.title, finding.summary)
}
