use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryHygieneOptions {
    pub root: PathBuf,
    pub write: bool,
    pub include_codex: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryHygieneReport {
    pub generated_at: String,
    pub root: String,
    pub write_applied: bool,
    pub files_scanned: Vec<MemoryFileSummary>,
    pub duplicate_groups: Vec<MemoryDuplicateGroup>,
    pub planned_removals: Vec<MemoryPruneRemoval>,
    pub backups: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryFileSummary {
    pub path: String,
    pub store_kind: String,
    pub guidance_units: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryDuplicateGroup {
    pub normalized: String,
    pub occurrences: Vec<MemoryDuplicateOccurrence>,
    pub auto_prunable: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryDuplicateOccurrence {
    pub path: String,
    pub line_number: usize,
    pub text: String,
    pub store_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPruneRemoval {
    pub path: String,
    pub line_number: usize,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
struct MemoryUnit {
    path: PathBuf,
    line_number: usize,
    text: String,
    normalized: String,
    store_kind: String,
}

pub fn run(options: &MemoryHygieneOptions) -> Result<MemoryHygieneReport> {
    let root = options
        .root
        .canonicalize()
        .unwrap_or_else(|_| options.root.clone());
    let files = discover_memory_files(&root, options.include_codex)?;
    let mut units = Vec::new();
    let mut summaries = Vec::new();
    let mut warnings = Vec::new();

    for path in files {
        let store_kind = store_kind(&path);
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read memory file {}", path.display()))?;
        let file_units = extract_units(&path, &store_kind, &content);
        summaries.push(MemoryFileSummary {
            path: display_path(&path),
            store_kind,
            guidance_units: file_units.len(),
        });
        units.extend(file_units);
    }

    let mut groups_by_key: BTreeMap<String, Vec<MemoryUnit>> = BTreeMap::new();
    for unit in units {
        groups_by_key
            .entry(unit.normalized.clone())
            .or_default()
            .push(unit);
    }

    let mut duplicate_groups = Vec::new();
    let mut planned_removals = Vec::new();
    for (normalized, mut occurrences) in groups_by_key {
        if occurrences.len() < 2 {
            continue;
        }
        occurrences.sort_by(|left, right| {
            memory_priority(left)
                .cmp(&memory_priority(right))
                .then(left.path.cmp(&right.path))
                .then(left.line_number.cmp(&right.line_number))
        });
        let auto_prunable = same_auto_prune_family(&occurrences);
        let reason = if auto_prunable {
            "exact duplicate inside the same memory family".to_string()
        } else {
            "cross-agent duplicate; report only because CLAUDE.md and AGENTS.md may both need the same rule"
                .to_string()
        };
        if auto_prunable {
            for occurrence in occurrences.iter().skip(1) {
                planned_removals.push(MemoryPruneRemoval {
                    path: display_path(&occurrence.path),
                    line_number: occurrence.line_number,
                    text: occurrence.text.clone(),
                    reason: reason.clone(),
                });
            }
        }
        duplicate_groups.push(MemoryDuplicateGroup {
            normalized,
            occurrences: occurrences
                .iter()
                .map(|unit| MemoryDuplicateOccurrence {
                    path: display_path(&unit.path),
                    line_number: unit.line_number,
                    text: unit.text.clone(),
                    store_kind: unit.store_kind.clone(),
                })
                .collect(),
            auto_prunable,
            reason,
        });
    }

    let mut backups = Vec::new();
    if options.write {
        backups = apply_removals(&planned_removals)?;
    } else if !planned_removals.is_empty() {
        warnings.push(
            "dry run only; pass --write to apply exact duplicate removals with backups".to_string(),
        );
    }
    if duplicate_groups.iter().any(|group| !group.auto_prunable) {
        warnings.push("cross-agent duplicates were reported but not auto-pruned".to_string());
    }

    Ok(MemoryHygieneReport {
        generated_at: Utc::now().to_rfc3339(),
        root: display_path(&root),
        write_applied: options.write,
        files_scanned: summaries,
        duplicate_groups,
        planned_removals,
        backups,
        warnings,
    })
}

fn discover_memory_files(root: &Path, include_codex: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    discover_memory_files_inner(root, root, include_codex, &mut files)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn discover_memory_files_inner(
    root: &Path,
    dir: &Path,
    include_codex: bool,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "target" | "node_modules" | ".worktrees"
            ) {
                continue;
            }
            if !include_codex && name == ".codex" {
                continue;
            }
            if path
                .strip_prefix(root)
                .ok()
                .map(|p| p.components().count())
                .unwrap_or(0)
                <= 4
            {
                discover_memory_files_inner(root, &path, include_codex, files)?;
            }
            continue;
        }
        if is_memory_filename(&name) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_memory_filename(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "agents.md" | "claude.md" | "context.md" | ".claude.md" | ".claude.local.md"
    )
}

fn extract_units(path: &Path, store_kind: &str, content: &str) -> Vec<MemoryUnit> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let text = line.trim();
            let normalized = normalize_guidance(text)?;
            Some(MemoryUnit {
                path: path.to_path_buf(),
                line_number: index + 1,
                text: text.to_string(),
                normalized,
                store_kind: store_kind.to_string(),
            })
        })
        .collect()
}

fn normalize_guidance(text: &str) -> Option<String> {
    let trimmed = text
        .trim()
        .trim_start_matches("- ")
        .trim_start_matches("* ")
        .trim_start_matches("> ")
        .trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("```")
        || trimmed.len() < 28
    {
        return None;
    }
    let mut out = String::new();
    let mut last_space = false;
    for ch in trimmed.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    let normalized = out.trim().to_string();
    if normalized.len() < 24 {
        None
    } else {
        Some(normalized)
    }
}

fn store_kind(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match name.as_str() {
        "agents.md" => "agents".to_string(),
        "claude.md" | ".claude.md" | ".claude.local.md" => "claude".to_string(),
        "context.md" => "context".to_string(),
        _ => "memory".to_string(),
    }
}

fn memory_priority(unit: &MemoryUnit) -> (i32, usize) {
    let kind_priority = match unit.store_kind.as_str() {
        "agents" => 0,
        "claude" => 1,
        "context" => 2,
        _ => 3,
    };
    (kind_priority, unit.path.components().count())
}

fn same_auto_prune_family(units: &[MemoryUnit]) -> bool {
    units
        .first()
        .map(|first| units.iter().all(|unit| unit.path == first.path))
        .unwrap_or(false)
}

fn apply_removals(removals: &[MemoryPruneRemoval]) -> Result<Vec<String>> {
    let mut by_path: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for removal in removals {
        by_path
            .entry(removal.path.clone())
            .or_default()
            .insert(removal.line_number);
    }

    let mut backups = Vec::new();
    for (path, line_numbers) in by_path {
        let path_buf = PathBuf::from(&path);
        let content = fs::read_to_string(&path_buf)
            .with_context(|| format!("failed to read {}", path_buf.display()))?;
        let backup = format!("{}.munin-bak", path);
        fs::write(&backup, &content).with_context(|| format!("failed to write backup {backup}"))?;
        let mut next = Vec::new();
        for (index, line) in content.lines().enumerate() {
            if !line_numbers.contains(&(index + 1)) {
                next.push(line);
            }
        }
        fs::write(&path_buf, format!("{}\n", next.join("\n")))
            .with_context(|| format!("failed to write {}", path_buf.display()))?;
        backups.push(backup);
    }
    Ok(backups)
}

fn display_path(path: &Path) -> String {
    crate::core::utils::normalize_windows_path_string(path.to_string_lossy().as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_marks_cross_agent_duplicates_report_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path().join("AGENTS.md"),
            "- Always run tests before claiming completion.\n",
        )
        .expect("agents");
        fs::write(
            temp.path().join("CLAUDE.md"),
            "- Always run tests before claiming completion.\n",
        )
        .expect("claude");

        let report = run(&MemoryHygieneOptions {
            root: temp.path().to_path_buf(),
            write: false,
            include_codex: false,
        })
        .expect("report");

        assert_eq!(report.duplicate_groups.len(), 1);
        assert!(!report.duplicate_groups[0].auto_prunable);
        assert!(report.planned_removals.is_empty());
    }

    #[test]
    fn write_prunes_exact_duplicate_inside_same_family_with_backup() {
        let temp = tempfile::tempdir().expect("tempdir");
        let agents = temp.path().join("AGENTS.md");
        fs::write(
            &agents,
            "- Always run tests before claiming completion.\n- Always run tests before claiming completion.\n- Keep responses concise.\n",
        )
        .expect("agents");

        let report = run(&MemoryHygieneOptions {
            root: temp.path().to_path_buf(),
            write: true,
            include_codex: false,
        })
        .expect("report");

        assert_eq!(report.planned_removals.len(), 1);
        assert_eq!(report.backups.len(), 1);
        let updated = fs::read_to_string(&agents).expect("updated");
        assert_eq!(
            updated
                .matches("Always run tests before claiming completion.")
                .count(),
            1
        );
    }

    #[test]
    fn same_kind_duplicates_across_files_are_report_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("nested")).expect("nested");
        fs::write(
            temp.path().join("CLAUDE.md"),
            "- Always run tests before claiming completion.\n",
        )
        .expect("root claude");
        fs::write(
            temp.path().join("nested").join("CLAUDE.md"),
            "- Always run tests before claiming completion.\n",
        )
        .expect("nested claude");

        let report = run(&MemoryHygieneOptions {
            root: temp.path().to_path_buf(),
            write: false,
            include_codex: false,
        })
        .expect("report");

        assert_eq!(report.duplicate_groups.len(), 1);
        assert!(!report.duplicate_groups[0].auto_prunable);
        assert!(report.planned_removals.is_empty());
    }
}
