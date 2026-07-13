use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MARKETPLACE_NAME: &str = "munin-memory";
const PLUGIN_NAME: &str = "munin-memory";
const USER_FACING_SKILLS: &[&str] = &[
    "munin",
    "munin-brain",
    "munin-doctor",
    "munin-friction",
    "munin-hygiene",
    "munin-nudge",
    "munin-prove",
    "munin-recall",
    "munin-resume",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

#[test]
fn repository_marketplace_is_dual_host() {
    let root = repo_root();
    let codex_marketplace = read_json(&root.join(".agents/plugins/marketplace.json"));
    let claude_marketplace = read_json(&root.join(".claude-plugin/marketplace.json"));

    assert_eq!(codex_marketplace["name"], MARKETPLACE_NAME);
    assert_eq!(claude_marketplace["name"], MARKETPLACE_NAME);
    assert_eq!(
        codex_marketplace["plugins"][0]["source"]["path"],
        "./plugins/munin-memory"
    );
    assert_eq!(
        claude_marketplace["plugins"][0]["source"],
        "./plugins/munin-memory"
    );

    let plugin_root = root.join("plugins/munin-memory");
    let codex_plugin = read_json(&plugin_root.join(".codex-plugin/plugin.json"));
    let claude_plugin = read_json(&plugin_root.join(".claude-plugin/plugin.json"));
    for manifest in [&codex_plugin, &claude_plugin] {
        assert_eq!(manifest["name"], PLUGIN_NAME);
        assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(manifest["license"], "Apache-2.0");
        assert_eq!(manifest["skills"], "./skills/");
    }

    let actual_skills = fs::read_dir(plugin_root.join("skills"))
        .expect("plugin skills directory")
        .map(|entry| entry.expect("skill entry"))
        .filter(|entry| entry.file_type().expect("skill file type").is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<BTreeSet<_>>();
    let expected_skills = USER_FACING_SKILLS
        .iter()
        .map(|name| (*name).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_skills, expected_skills);
    for skill in USER_FACING_SKILLS {
        assert!(
            plugin_root
                .join("skills")
                .join(skill)
                .join("SKILL.md")
                .is_file(),
            "missing native plugin skill {skill}"
        );
    }
    assert!(
        !plugin_root.join("commands").exists(),
        "native plugin must not duplicate skills as flat Claude commands"
    );
    assert!(
        actual_skills
            .iter()
            .all(|name| !name.starts_with("munin-install")),
        "installer/demo skills do not belong in the runtime plugin"
    );

    let cargo_manifest = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    for packaged_path in [
        r#"/.agents/plugins/marketplace.json"#,
        r#"/.claude-plugin/marketplace.json"#,
        r#"/plugins/munin-memory/**"#,
    ] {
        assert!(
            cargo_manifest.contains(packaged_path),
            "Cargo package must include {packaged_path}"
        );
    }
}
