use super::types::SessionBrainProjectContext;
use crate::core::utils::normalize_windows_path_string;
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn build_project_context(
    project_root: &Path,
    current_goal: Option<&str>,
    task_hints: &[String],
) -> Result<SessionBrainProjectContext> {
    let project_memory = read_project_memory(project_root)?;
    let priority_notes = read_priority_notes(project_root).unwrap_or_default();
    let description = read_project_description(project_root)?;
    let mut summary = Vec::new();

    if let Some(description) = description.clone() {
        summary.push(description);
    }
    summary.extend(summarize_project_memory(project_memory.as_ref()));
    if let Some(active_area) = build_active_task_area(project_root, current_goal, task_hints) {
        summary.push(active_area);
    }
    if summary.is_empty() {
        summary.push("Project capsule fallback: repo description unavailable.".to_string());
    }
    summary.truncate(6);

    let key_files = discover_key_files(project_root, task_hints);
    let codebase_map = build_codebase_map(project_root)?;

    Ok(SessionBrainProjectContext {
        summary,
        key_files,
        codebase_map,
        project_memory,
        priority_notes,
    })
}

fn read_project_description(project_root: &Path) -> Result<Option<String>> {
    let cargo_toml = project_root.join("Cargo.toml");
    if cargo_toml.exists() {
        let content = fs::read_to_string(&cargo_toml)
            .with_context(|| format!("failed to read {}", cargo_toml.display()))?;
        let parsed = content.parse::<toml::Value>().ok();
        if let Some(description) = parsed
            .as_ref()
            .and_then(|value| value.get("package"))
            .and_then(|value| value.get("description"))
            .and_then(|value| value.as_str())
        {
            let name = parsed
                .as_ref()
                .and_then(|value| value.get("package"))
                .and_then(|value| value.get("name"))
                .and_then(|value| value.as_str())
                .unwrap_or("project");
            return Ok(Some(format!("{name}: {description}")));
        }
    }

    let package_json = project_root.join("package.json");
    if package_json.exists() {
        let content = fs::read_to_string(&package_json)
            .with_context(|| format!("failed to read {}", package_json.display()))?;
        let parsed: Value = serde_json::from_str(&content).unwrap_or(Value::Null);
        if let Some(description) = parsed.get("description").and_then(Value::as_str) {
            let name = parsed
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("project");
            return Ok(Some(format!("{name}: {description}")));
        }
    }

    let readme = project_root.join("README.md");
    if readme.exists() {
        let content = fs::read_to_string(&readme)
            .with_context(|| format!("failed to read {}", readme.display()))?;
        let paragraph = content
            .split("\n\n")
            .map(str::trim)
            .find(|chunk| !chunk.is_empty() && !chunk.starts_with('#'));
        if let Some(paragraph) = paragraph {
            return Ok(Some(paragraph.replace('\n', " ")));
        }
    }

    Ok(None)
}

fn read_project_memory(project_root: &Path) -> Result<Option<Value>> {
    let candidates = [
        project_root.join(".codex").join("project-memory.json"),
        project_root.join("project-memory.json"),
    ];

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let content = fs::read_to_string(&candidate)
            .with_context(|| format!("failed to read {}", candidate.display()))?;
        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            return Ok(Some(parsed));
        }
    }

    Ok(None)
}

fn summarize_project_memory(project_memory: Option<&Value>) -> Vec<String> {
    let Some(project_memory) = project_memory else {
        return Vec::new();
    };

    let mut summary = Vec::new();
    for key in ["techStack", "build", "conventions", "structure"] {
        if let Some(value) = project_memory.get(key).and_then(Value::as_str) {
            summary.push(format!("{key}: {value}"));
        }
    }
    summary
}

fn read_priority_notes(project_root: &Path) -> Result<String> {
    let path = project_root.join(".codex").join("notepad.md");
    if !path.exists() {
        return Ok(String::new());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(extract_markdown_section(&content, "PRIORITY"))
}

fn extract_markdown_section(content: &str, header: &str) -> String {
    let marker = format!("## {header}");
    let Some(start) = content.find(&marker) else {
        return String::new();
    };
    let body = &content[start + marker.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    body[..end].trim().to_string()
}

fn build_active_task_area(
    project_root: &Path,
    current_goal: Option<&str>,
    task_hints: &[String],
) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(goal) = current_goal {
        lines.push(format!("Active ask: {}", goal));
    }

    let hinted_files = hinted_file_paths(project_root, task_hints)
        .into_iter()
        .filter(|path| path.starts_with("src/") || path.starts_with("tests/"))
        .take(4)
        .collect::<Vec<_>>();
    if !hinted_files.is_empty() {
        lines.push(format!("Active task area: {}", hinted_files.join(", ")));
    }

    (!lines.is_empty()).then(|| lines.join(" | "))
}

fn discover_key_files(project_root: &Path, task_hints: &[String]) -> Vec<String> {
    let mut key_files = Vec::new();

    for hinted in hinted_file_paths(project_root, task_hints) {
        push_unique(&mut key_files, hinted);
    }

    let static_candidates = [
        "README.md",
        "Cargo.toml",
        "package.json",
        "AGENTS.md",
        "CONTEXT.md",
        "src/main.rs",
        "src/bin/munin.rs",
        "src/lib.rs",
        "src/analytics/memory_os_cmd.rs",
        "src/core/memory_hygiene.rs",
        "src/core/resolver.rs",
    ];

    for relative in static_candidates {
        let path = project_root.join(relative);
        if path.exists() {
            push_unique(&mut key_files, relative_to_root(project_root, &path));
        }
    }

    if key_files.len() < 8 {
        let src_dir = project_root.join("src");
        if let Ok(entries) = fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                push_unique(&mut key_files, relative_to_root(project_root, &path));
                if key_files.len() >= 8 {
                    break;
                }
            }
        }
    }

    key_files.truncate(8);
    key_files
}

fn hinted_file_paths(project_root: &Path, task_hints: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    for hint in task_hints {
        if !looks_like_file_path(hint) {
            continue;
        }
        let candidate = project_root.join(hint);
        if candidate.is_file() {
            push_unique(&mut files, relative_to_root(project_root, &candidate));
            continue;
        }
        if candidate.is_dir() {
            collect_dir_files(project_root, &candidate, &mut files);
        }
    }
    files
}

fn collect_dir_files(project_root: &Path, dir: &Path, files: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        let mut sorted = entries
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        sorted.sort();
        for path in sorted {
            if !path.is_file() {
                continue;
            }
            push_unique(files, relative_to_root(project_root, &path));
            if files.len() >= 4 {
                break;
            }
        }
    }
}

fn build_codebase_map(project_root: &Path) -> Result<String> {
    let mut lines = Vec::new();
    let top_level = fs::read_dir(project_root)
        .with_context(|| format!("failed to read {}", project_root.display()))?
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    let top_level_names = top_level
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .take(10)
        .collect::<Vec<_>>();
    lines.push(format!("top-level: {}", top_level_names.join(", ")));

    let src_dir = project_root.join("src");
    if src_dir.exists() {
        let src_entries = fs::read_dir(&src_dir)
            .with_context(|| format!("failed to read {}", src_dir.display()))?
            .flatten()
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        let src_names = src_entries
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .take(10)
            .collect::<Vec<_>>();
        lines.push(format!("src: {}", src_names.join(", ")));
    }

    let hooks_dir = project_root.join("hooks");
    if hooks_dir.exists() {
        lines.push("hooks/: runtime integration assets".to_string());
    }

    Ok(lines.join("\n"))
}

fn looks_like_file_path(hint: &str) -> bool {
    let lowered = hint.to_ascii_lowercase();
    lowered.contains("src/")
        || lowered.contains("tests/")
        || lowered.ends_with(".rs")
        || lowered.ends_with(".md")
        || lowered.ends_with(".toml")
        || lowered.ends_with(".json")
}

fn relative_to_root(project_root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    normalize_windows_path_string(relative.to_string_lossy().as_ref())
}

fn push_unique(target: &mut Vec<String>, value: String) {
    if !target.contains(&value) {
        target.push(value);
    }
}

#[allow(dead_code)]
fn join_relative(project_root: &Path, relative: &str) -> PathBuf {
    project_root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_hints_rank_relevant_files_ahead_of_static_defaults() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project = build_project_context(
            &root,
            Some("Fix session brain ranking"),
            &[
                "src/session_brain/build.rs".to_string(),
                "src/session_brain/project.rs".to_string(),
            ],
        )
        .expect("project context");

        assert_eq!(
            project.key_files.first().map(String::as_str),
            Some("src/session_brain/build.rs")
        );
        assert_eq!(
            project.key_files.get(1).map(String::as_str),
            Some("src/session_brain/project.rs")
        );
        assert!(project
            .summary
            .iter()
            .any(|line| line.contains("Active ask: Fix session brain ranking")));
    }
}
