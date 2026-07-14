use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

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

    let codex_hooks =
        fs::read_to_string(plugin_root.join("hooks.json")).expect("Codex native plugin hooks");
    let claude_hooks = fs::read_to_string(plugin_root.join("hooks/hooks.json"))
        .expect("Claude native plugin hooks");
    let session_start =
        fs::read_to_string(plugin_root.join("scripts").join("munin-session-start.js"))
            .expect("shared session-start hook");
    for shipped_hook in [&codex_hooks, &claude_hooks, &session_start] {
        assert!(!shipped_hook.contains("spawnSync"));
        assert!(!shipped_hook.contains("timeout\": 120"));
        assert!(!shipped_hook.contains("memory-os ingest"));
    }
    assert!(session_start.contains("hook"));
    assert!(session_start.contains("session-start"));

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

struct FakeHosts {
    codex: PathBuf,
    claude: PathBuf,
    state: PathBuf,
}

fn write_fake_hosts(root: &Path) -> FakeHosts {
    let state = root.join("state");
    fs::create_dir_all(&state).expect("fake host state directory");

    #[cfg(windows)]
    let (codex, claude) = {
        let codex = root.join("codex.cmd");
        let claude = root.join("claude.cmd");
        fs::write(
            &codex,
            r#"@echo off
echo %*>> "%FAKE_HOST_STATE%\codex.log"
if "%1 %2 %3"=="plugin marketplace list" (
  if exist "%FAKE_HOST_STATE%\codex-marketplace" (echo {"marketplaces":[{"name":"munin-memory","root":"fake"}]}) else (echo {"marketplaces":[]})
  exit /b 0
)
if "%1 %2"=="plugin list" (
  if exist "%FAKE_HOST_STATE%\codex-plugin" (echo {"installed":[{"pluginId":"munin-memory@munin-memory","version":"0.5.14","installed":true,"enabled":true}],"available":[]}) else (echo {"installed":[],"available":[]})
  exit /b 0
)
if "%1 %2 %3"=="plugin marketplace add" (copy /Y NUL "%FAKE_HOST_STATE%\codex-marketplace" >NUL & echo {"name":"munin-memory"} & exit /b 0)
if "%1 %2"=="plugin add" (copy /Y NUL "%FAKE_HOST_STATE%\codex-plugin" >NUL & echo {"pluginId":"munin-memory@munin-memory","version":"0.5.14"} & exit /b 0)
if "%1 %2"=="plugin remove" (if exist "%FAKE_HOST_STATE%\codex-plugin" del "%FAKE_HOST_STATE%\codex-plugin" & echo {"removed":true} & exit /b 0)
if "%1 %2 %3"=="plugin marketplace remove" (if exist "%FAKE_HOST_STATE%\codex-marketplace" del "%FAKE_HOST_STATE%\codex-marketplace" & echo {"removed":true} & exit /b 0)
echo unexpected codex arguments: %* 1>&2
exit /b 9
"#,
        )
        .expect("write fake Codex host");
        fs::write(
            &claude,
            r#"@echo off
echo %*>> "%FAKE_HOST_STATE%\claude.log"
if "%1 %2 %3"=="plugin marketplace list" (
  if exist "%FAKE_HOST_STATE%\claude-marketplace" (echo [{"name":"munin-memory","source":"directory","path":"fake"}]) else (echo [])
  exit /b 0
)
if "%1 %2"=="plugin list" (
  if exist "%FAKE_HOST_STATE%\claude-plugin" (echo [{"id":"munin-memory@munin-memory","version":"0.5.14","scope":"user","enabled":true}]) else (echo [])
  exit /b 0
)
if "%1 %2"=="plugin validate" (echo Validation passed & exit /b 0)
if "%1 %2 %3"=="plugin marketplace add" (copy /Y NUL "%FAKE_HOST_STATE%\claude-marketplace" >NUL & echo Marketplace added & exit /b 0)
if "%1 %2"=="plugin install" (
  if "%FAKE_CLAUDE_FAIL_INSTALL%"=="1" (echo forced Claude install failure 1>&2 & exit /b 7)
  copy /Y NUL "%FAKE_HOST_STATE%\claude-plugin" >NUL & echo Plugin installed & exit /b 0
)
if "%1 %2"=="plugin uninstall" (if exist "%FAKE_HOST_STATE%\claude-plugin" del "%FAKE_HOST_STATE%\claude-plugin" & echo Plugin uninstalled & exit /b 0)
if "%1 %2 %3"=="plugin marketplace remove" (if exist "%FAKE_HOST_STATE%\claude-marketplace" del "%FAKE_HOST_STATE%\claude-marketplace" & echo Marketplace removed & exit /b 0)
echo unexpected claude arguments: %* 1>&2
exit /b 9
"#,
        )
        .expect("write fake Claude host");
        (codex, claude)
    };

    #[cfg(unix)]
    let (codex, claude) = {
        use std::os::unix::fs::PermissionsExt;
        let codex = root.join("codex");
        let claude = root.join("claude");
        fs::write(
            &codex,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_HOST_STATE/codex.log"
if [ "$1 $2 $3" = "plugin marketplace list" ]; then
  if [ -f "$FAKE_HOST_STATE/codex-marketplace" ]; then printf '%s\n' '{"marketplaces":[{"name":"munin-memory","root":"fake"}]}'; else printf '%s\n' '{"marketplaces":[]}'; fi
elif [ "$1 $2" = "plugin list" ]; then
  if [ -f "$FAKE_HOST_STATE/codex-plugin" ]; then printf '%s\n' '{"installed":[{"pluginId":"munin-memory@munin-memory","version":"0.5.14","installed":true,"enabled":true}],"available":[]}'; else printf '%s\n' '{"installed":[],"available":[]}'; fi
elif [ "$1 $2 $3" = "plugin marketplace add" ]; then touch "$FAKE_HOST_STATE/codex-marketplace"; printf '%s\n' '{"name":"munin-memory"}'
elif [ "$1 $2" = "plugin add" ]; then touch "$FAKE_HOST_STATE/codex-plugin"; printf '%s\n' '{"pluginId":"munin-memory@munin-memory","version":"0.5.14"}'
elif [ "$1 $2" = "plugin remove" ]; then rm -f "$FAKE_HOST_STATE/codex-plugin"; printf '%s\n' '{"removed":true}'
elif [ "$1 $2 $3" = "plugin marketplace remove" ]; then rm -f "$FAKE_HOST_STATE/codex-marketplace"; printf '%s\n' '{"removed":true}'
else printf 'unexpected codex arguments: %s\n' "$*" >&2; exit 9
fi
"#,
        )
        .expect("write fake Codex host");
        fs::write(
            &claude,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_HOST_STATE/claude.log"
if [ "$1 $2 $3" = "plugin marketplace list" ]; then
  if [ -f "$FAKE_HOST_STATE/claude-marketplace" ]; then printf '%s\n' '[{"name":"munin-memory","source":"directory","path":"fake"}]'; else printf '%s\n' '[]'; fi
elif [ "$1 $2" = "plugin list" ]; then
  if [ -f "$FAKE_HOST_STATE/claude-plugin" ]; then printf '%s\n' '[{"id":"munin-memory@munin-memory","version":"0.5.14","scope":"user","enabled":true}]'; else printf '%s\n' '[]'; fi
elif [ "$1 $2" = "plugin validate" ]; then printf '%s\n' 'Validation passed'
elif [ "$1 $2 $3" = "plugin marketplace add" ]; then touch "$FAKE_HOST_STATE/claude-marketplace"; printf '%s\n' 'Marketplace added'
elif [ "$1 $2" = "plugin install" ]; then
  if [ "$FAKE_CLAUDE_FAIL_INSTALL" = "1" ]; then printf '%s\n' 'forced Claude install failure' >&2; exit 7; fi
  touch "$FAKE_HOST_STATE/claude-plugin"; printf '%s\n' 'Plugin installed'
elif [ "$1 $2" = "plugin uninstall" ]; then rm -f "$FAKE_HOST_STATE/claude-plugin"; printf '%s\n' 'Plugin uninstalled'
elif [ "$1 $2 $3" = "plugin marketplace remove" ]; then rm -f "$FAKE_HOST_STATE/claude-marketplace"; printf '%s\n' 'Marketplace removed'
else printf 'unexpected claude arguments: %s\n' "$*" >&2; exit 9
fi
"#,
        )
        .expect("write fake Claude host");
        fs::set_permissions(&codex, fs::Permissions::from_mode(0o755)).expect("chmod Codex fake");
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).expect("chmod Claude fake");
        (codex, claude)
    };

    FakeHosts {
        codex,
        claude,
        state,
    }
}

fn run_lifecycle(profile: &Path, hosts: &FakeHosts, args: &[&str], fail_claude: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_munin"));
    command
        .args(args)
        .env("MUNIN_INSTALL_HOME", profile)
        .env("MUNIN_CODEX_BIN", &hosts.codex)
        .env("MUNIN_CLAUDE_BIN", &hosts.claude)
        .env("FAKE_HOST_STATE", &hosts.state);
    if fail_claude {
        command.env("FAKE_CLAUDE_FAIL_INSTALL", "1");
    }
    command.output().expect("run Munin lifecycle command")
}

fn assert_success(output: &Output, label: &str) -> Value {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{label} returned invalid JSON: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

#[test]
fn lifecycle_install_verify_uninstall_uses_native_hosts() {
    let profile = tempfile::tempdir().expect("isolated install profile");
    let hosts_root = tempfile::tempdir().expect("fake host root");
    let hosts = write_fake_hosts(hosts_root.path());

    for (flag, state_name) in [("--codex", "codex-plugin"), ("--claude", "claude-plugin")] {
        let install = assert_success(
            &run_lifecycle(
                profile.path(),
                &hosts,
                &["plugin", "install", flag, "--format", "json"],
                false,
            ),
            "plugin install",
        );
        assert_eq!(install["schema"], "munin.agent-plugin-receipt.v1");
        assert_eq!(install["status"], "pass");
        assert!(hosts.state.join(state_name).is_file());

        let repeat_install = assert_success(
            &run_lifecycle(
                profile.path(),
                &hosts,
                &["plugin", "install", flag, "--format", "json"],
                false,
            ),
            "repeat plugin install",
        );
        assert_eq!(repeat_install["status"], "pass");
        assert_eq!(repeat_install["bundle_created"], false);
        assert_eq!(repeat_install["hosts"][0]["created_marketplace"], false);
        assert_eq!(repeat_install["hosts"][0]["created_plugin"], false);

        let verify = assert_success(
            &run_lifecycle(
                profile.path(),
                &hosts,
                &["plugin", "verify", flag, "--format", "json"],
                false,
            ),
            "plugin verify",
        );
        assert_eq!(verify["status"], "pass");

        let uninstall = assert_success(
            &run_lifecycle(
                profile.path(),
                &hosts,
                &["plugin", "uninstall", flag, "--format", "json"],
                false,
            ),
            "plugin uninstall",
        );
        assert_eq!(uninstall["status"], "pass");
        assert!(!hosts.state.join(state_name).exists());
    }

    let codex_log = fs::read_to_string(hosts.state.join("codex.log")).expect("Codex argv log");
    let claude_log = fs::read_to_string(hosts.state.join("claude.log")).expect("Claude argv log");
    assert!(codex_log.contains("plugin marketplace add"));
    assert!(codex_log.contains("plugin add munin-memory@munin-memory"));
    assert!(claude_log.contains("plugin validate --strict"));
    assert!(claude_log.contains("plugin install munin-memory@munin-memory"));
}

#[test]
fn lifecycle_partial_failure_rolls_back_created_codex_state() {
    let profile = tempfile::tempdir().expect("isolated install profile");
    let hosts_root = tempfile::tempdir().expect("fake host root");
    let hosts = write_fake_hosts(hosts_root.path());

    let output = run_lifecycle(
        profile.path(),
        &hosts,
        &["plugin", "install", "--format", "json"],
        true,
    );
    assert!(
        !output.status.success(),
        "forced Claude failure must fail closed"
    );
    for state in [
        "codex-plugin",
        "codex-marketplace",
        "claude-plugin",
        "claude-marketplace",
    ] {
        assert!(
            !hosts.state.join(state).exists(),
            "rollback left fake state {state}"
        );
    }
    let codex_log = fs::read_to_string(hosts.state.join("codex.log")).expect("Codex argv log");
    assert!(codex_log.contains("plugin remove munin-memory@munin-memory"));
    assert!(codex_log.contains("plugin marketplace remove munin-memory"));
}

fn write_profile_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("profile file parent")).expect("create profile parent");
    fs::write(path, contents).expect("write profile fixture");
}

#[test]
fn migration_preserves_unrelated_state_and_backs_up_munin_files() {
    let profile = tempfile::tempdir().expect("isolated install profile");
    let hosts_root = tempfile::tempdir().expect("fake host root");
    let hosts = write_fake_hosts(hosts_root.path());
    write_profile_file(
        profile.path(),
        ".codex/skills/munin-recall/SKILL.md",
        "---\nname: munin-recall\n---\nRun munin recall.\n",
    );
    write_profile_file(
        profile.path(),
        ".codex/plugins/munin-memory/.codex-plugin/plugin.json",
        r#"{"name":"munin-memory","version":"0.5.14"}"#,
    );
    write_profile_file(
        profile.path(),
        ".claude/skills/munin-brain/SKILL.md",
        "---\nname: munin-brain\n---\nRun munin brain.\n",
    );
    write_profile_file(
        profile.path(),
        ".claude/skills/munin-custom/SKILL.md",
        "---\nname: munin-custom\n---\nUser-owned content.\n",
    );
    write_profile_file(
        profile.path(),
        ".claude/commands/munin-recall.md",
        "Invoke the Munin skill and run munin recall.\n",
    );
    write_profile_file(
        profile.path(),
        ".claude/hooks/munin-session-start.js",
        "spawnSync('munin', ['memory-os', 'ingest']);\n",
    );

    let unrelated_claude = serde_json::json!({
        "matcher": "startup",
        "hooks": [{
            "type": "command",
            "command": "node C:/tools/unrelated-claude.js",
            "timeout": 7
        }]
    });
    let claude_settings = serde_json::json!({
        "theme": "dark",
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "startup",
                    "hooks": [{
                        "type": "command",
                        "command": "node C:/profile/.claude/hooks/munin-session-start.js",
                        "timeout": 120
                    }]
                },
                unrelated_claude.clone()
            ]
        }
    });
    write_profile_file(
        profile.path(),
        ".claude/settings.json",
        &serde_json::to_string_pretty(&claude_settings).expect("Claude settings JSON"),
    );

    let unrelated_codex = serde_json::json!({
        "matcher": "startup",
        "hooks": [{
            "type": "command",
            "command": "python C:/tools/unrelated-codex.py",
            "timeout": 9
        }]
    });
    let codex_hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [
                {
                    "matcher": "startup|resume|clear",
                    "hooks": [{
                        "type": "command",
                        "command": "\"C:/bin/munin.exe\" memory-os ingest --check-now --format json",
                        "timeout": 120
                    }]
                },
                unrelated_codex.clone()
            ]
        }
    });
    write_profile_file(
        profile.path(),
        ".codex/hooks.json",
        &serde_json::to_string_pretty(&codex_hooks).expect("Codex hooks JSON"),
    );

    let install = assert_success(
        &run_lifecycle(
            profile.path(),
            &hosts,
            &["plugin", "install", "--format", "json"],
            false,
        ),
        "plugin install with migration",
    );
    let backup = PathBuf::from(
        install["migration"]["backup_path"]
            .as_str()
            .expect("migration backup path"),
    );
    for relative in [
        ".codex/skills/munin-recall/SKILL.md",
        ".codex/plugins/munin-memory/.codex-plugin/plugin.json",
        ".claude/skills/munin-brain/SKILL.md",
        ".claude/commands/munin-recall.md",
        ".claude/hooks/munin-session-start.js",
    ] {
        assert!(
            !profile.path().join(relative).exists(),
            "legacy path remains active: {relative}"
        );
        assert!(
            backup.join(relative).is_file(),
            "legacy path was not backed up: {relative}"
        );
    }
    assert!(
        profile
            .path()
            .join(".claude/skills/munin-custom/SKILL.md")
            .is_file(),
        "unknown Munin-named path must remain untouched"
    );
    assert!(
        install["migration"]["conflicts"]
            .as_array()
            .expect("migration conflicts")
            .iter()
            .any(|value| value.as_str() == Some(".claude/skills/munin-custom")),
        "unknown Munin-named path must be reported as a conflict"
    );

    let migrated_claude: Value = serde_json::from_str(
        &fs::read_to_string(profile.path().join(".claude/settings.json"))
            .expect("migrated Claude settings"),
    )
    .expect("parse migrated Claude settings");
    let migrated_codex: Value = serde_json::from_str(
        &fs::read_to_string(profile.path().join(".codex/hooks.json"))
            .expect("migrated Codex hooks"),
    )
    .expect("parse migrated Codex hooks");
    assert_eq!(migrated_claude["theme"], "dark");
    assert_eq!(
        migrated_claude["hooks"]["SessionStart"],
        serde_json::json!([unrelated_claude])
    );
    assert_eq!(
        migrated_codex["hooks"]["SessionStart"],
        serde_json::json!([unrelated_codex])
    );
    assert!(serde_json::to_string(&migrated_claude)
        .expect("serialize Claude settings")
        .contains("unrelated-claude.js"));
    assert!(serde_json::to_string(&migrated_codex)
        .expect("serialize Codex hooks")
        .contains("unrelated-codex.py"));
}

fn run_hook(profile: &Path, fail_worker: bool) -> (Output, Duration) {
    let started = Instant::now();
    let mut command = Command::new(env!("CARGO_BIN_EXE_munin"));
    command
        .args(["hook", "session-start", "--format", "json"])
        .env("MUNIN_DATA_DIR", profile.join("data"))
        .env("MUNIN_CONFIG_DIR", profile.join("config"))
        .env("MUNIN_SESSION_HOME", profile.join("sessions"));
    if fail_worker {
        command.env("MUNIN_HOOK_WORKER_TEST_FAIL", "1");
    }
    let output = command.output().expect("run session-start hook");
    (output, started.elapsed())
}

fn wait_for_worker_status(profile: &Path, expected: &str) -> Value {
    let path = profile.join("data/agent-hook/status.json");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(contents) = fs::read_to_string(&path) {
            let value: Value = serde_json::from_str(&contents).expect("worker status JSON");
            if value["status"] == expected {
                return value;
            }
        }
        assert!(
            Instant::now() < deadline,
            "worker did not report {expected}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn hook_session_start_is_bounded_and_failure_is_visible() {
    let profile = tempfile::tempdir().expect("isolated hook profile");
    let (output, elapsed) = run_hook(profile.path(), true);
    let receipt = assert_success(&output, "session-start hook");
    assert!(
        elapsed < Duration::from_secs(2),
        "session-start took {:?}",
        elapsed
    );
    assert_eq!(receipt["schema"], "munin.agent-hook-receipt.v1");
    assert_eq!(receipt["refresh_queued"], true);
    assert!(receipt.get("generated_at").is_some());
    assert!(receipt.get("last_refresh_at").is_some());
    assert!(receipt.get("age_seconds").is_some());

    let worker = wait_for_worker_status(profile.path(), "fail");
    assert!(worker["last_error"]
        .as_str()
        .expect("worker error")
        .contains("forced hook worker failure"));

    let (next_output, next_elapsed) = run_hook(profile.path(), true);
    let next = assert_success(&next_output, "session-start after worker failure");
    assert!(next_elapsed < Duration::from_secs(2));
    assert_eq!(next["status"], "refresh-failed");
    assert_eq!(next["last_refresh_at"], Value::Null);
    assert_eq!(next["age_seconds"], Value::Null);
}
