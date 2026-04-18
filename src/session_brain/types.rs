use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionBrainProvider {
    Codex,
    Claude,
    Unknown,
}

impl SessionBrainProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainMeta {
    pub session_id: Option<String>,
    pub cwd: String,
    pub project_root: String,
    pub built_at: String,
    pub version: u32,
    pub provider: SessionBrainProvider,
    pub transcript_source_path: Option<String>,
    pub transcript_modified_at: Option<String>,
    pub source_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainMessage {
    pub role: String,
    pub provider: SessionBrainProvider,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub cwd: Option<String>,
    pub transcript_path: String,
    pub record_type: String,
    pub line_number: usize,
    pub text: String,
    pub source_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainSignal {
    pub summary: String,
    pub source: String,
    pub timestamp: Option<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainAgenda {
    pub current_goal: Option<String>,
    pub subgoals: Vec<String>,
    pub redirects: Vec<SessionBrainSignal>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainState {
    pub decisions: Vec<SessionBrainSignal>,
    pub findings: Vec<SessionBrainSignal>,
    pub blockers: Vec<SessionBrainSignal>,
    pub verified_facts: Vec<SessionBrainSignal>,
    pub rejected_options: Vec<SessionBrainSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainProjectContext {
    pub summary: Vec<String>,
    pub key_files: Vec<String>,
    pub codebase_map: String,
    pub project_memory: Option<Value>,
    pub priority_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainStrategyContext {
    pub summary: Vec<String>,
    pub source_paths: Vec<String>,
    pub planning_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainUserContext {
    pub brief: String,
    pub overview: String,
    pub profile: String,
    pub friction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainGuidance {
    pub retrieval_hints: Vec<String>,
    pub avoid: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrain {
    pub meta: SessionBrainMeta,
    pub messages: SessionBrainMessageGroup,
    pub agenda: SessionBrainAgenda,
    pub state: SessionBrainState,
    pub project: SessionBrainProjectContext,
    pub strategy: SessionBrainStrategyContext,
    pub user: SessionBrainUserContext,
    pub guidance: SessionBrainGuidance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBrainMessageGroup {
    pub user: Vec<SessionBrainMessage>,
    pub assistant: Vec<SessionBrainMessage>,
}
