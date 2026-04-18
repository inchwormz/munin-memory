use serde::{Deserialize, Serialize};

use crate::core::access_layer::intent_rules::{IntentRule, INTENT_RULES};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveReport {
    pub query: String,
    pub route: String,
    pub command: String,
    pub reason: String,
}

pub fn resolve(query: &str) -> ResolveReport {
    resolve_with_source_status(query, None)
}

pub fn resolve_with_source_status(query: &str, source_status: Option<&str>) -> ResolveReport {
    let trimmed = query.trim();
    let lowered = trimmed.to_lowercase();
    let rule = INTENT_RULES
        .iter()
        .find(|rule| rule_matches(rule, &lowered))
        .unwrap_or_else(|| {
            crate::core::access_layer::intent_rules::intent_by_route("recall")
                .expect("recall intent must exist")
        });

    let (route, command, reason) = if let Some(live_fallback) = rule.live_fallback {
        if source_status == Some("live") {
            (rule.route, rule.primary_command, rule.reason)
        } else {
            (
                live_fallback.fallback_route,
                live_fallback.fallback_command,
                live_fallback.fallback_reason,
            )
        }
    } else {
        (rule.route, rule.primary_command, rule.reason)
    };

    ResolveReport {
        query: trimmed.to_string(),
        route: route.to_string(),
        command: render_query_command(command, trimmed),
        reason: reason.to_string(),
    }
}

pub fn known_resolver_commands() -> Vec<&'static str> {
    INTENT_RULES
        .iter()
        .flat_map(|rule| {
            std::iter::once(rule.primary_command).chain(
                rule.fallback_command
                    .iter()
                    .filter(|fallback| fallback.command.starts_with("munin "))
                    .map(|fallback| fallback.command),
            )
        })
        .collect()
}

fn rule_matches(rule: &IntentRule, lowered_query: &str) -> bool {
    let positive = rule
        .trigger_phrases
        .iter()
        .any(|phrase| lowered_query.contains(phrase));
    let negative = rule
        .negative_triggers
        .iter()
        .any(|phrase| lowered_query.contains(phrase));
    positive && !negative
}

fn render_query_command(command: &str, query: &str) -> String {
    command.replace("<query>", query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_routes_friction_questions_to_friction() {
        let report = resolve("what keeps going wrong with codex?");
        assert_eq!(report.route, "friction");
    }

    #[test]
    fn resolve_routes_memory_questions_to_recall() {
        let report = resolve("what did we decide about resolver?");
        assert_eq!(report.route, "recall");
    }

    #[test]
    fn resolve_routes_continuity_to_brain_only_when_live() {
        let live = resolve_with_source_status("what was I doing?", Some("live"));
        assert_eq!(live.route, "brain");
        let fallback = resolve_with_source_status("what was I doing?", Some("fallback-latest"));
        assert_eq!(fallback.route, "resume");
        let current_fallback =
            resolve_with_source_status("current session?", Some("fallback-latest"));
        assert_eq!(current_fallback.route, "resume");
    }
}
