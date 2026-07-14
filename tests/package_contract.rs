#[test]
fn cargo_toml_marks_crate_apache_licensed() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == r#"license = "Apache-2.0""#),
        "Cargo.toml must declare the Apache-2.0 license"
    );
}

#[test]
fn release_posture_is_truthful() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == "publish = false"),
        "publishing must stay disabled until the release is explicitly approved"
    );

    let readme = std::fs::read_to_string("README.md").expect("README.md");
    assert!(readme.contains("Published crate: `0.5.14`."));
    assert!(readme.contains("Latest GitHub release: `v0.5.13`."));

    let install = std::fs::read_to_string("INSTALL.md").expect("INSTALL.md");
    assert!(!install.contains("Current testing build: `v0.5.1`."));
    assert!(install.contains("Repository package version: `0.5.14`."));

    let changelog = std::fs::read_to_string("CHANGELOG.md").expect("CHANGELOG.md");
    assert!(changelog.contains("## [Unreleased]"));
}

#[test]
fn readme_names_open_source_license() {
    let readme = std::fs::read_to_string("README.md").expect("README.md");
    assert!(readme.contains("Apache 2.0"));
}

#[test]
fn release_check_script_is_packaged_with_readme() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let script_path = "/scripts/munin-release-check.ps1";
    assert!(
        manifest.contains(script_path),
        "Cargo.toml include list must package {script_path}"
    );
    assert!(
        std::path::Path::new("scripts/munin-release-check.ps1").exists(),
        "README documents scripts/munin-release-check.ps1, so it must exist"
    );
}

#[test]
fn agent_plugin_check_script_is_packaged() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let script_path = "/scripts/munin-agent-plugin-check.ps1";
    assert!(
        manifest.contains(script_path),
        "Cargo.toml include list must package {script_path}"
    );
    assert!(
        std::path::Path::new("scripts/munin-agent-plugin-check.ps1").exists(),
        "native plugin release gate must exist"
    );
}

#[test]
fn typed_mcp_distribution_is_packaged_and_documented() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    for path in [
        "/src/mcp.rs",
        "/src/mcp_tools.rs",
        "/tests/mcp_contract.rs",
        "/scripts/munin-mcp-host-check.ps1",
    ] {
        assert!(
            manifest.contains(path),
            "Cargo.toml include list must package {path}"
        );
    }

    let readme = std::fs::read_to_string("README.md").expect("README.md");
    assert!(readme.contains("## Typed MCP Memory Tools"));
    assert!(readme.contains("scripts\\munin-mcp-host-check.ps1"));

    let install = std::fs::read_to_string("INSTALL.md").expect("INSTALL.md");
    assert!(!install.contains("Typed MCP memory tools are planned"));
    assert!(install.contains("Six read-only MCP tools"));

    let release =
        std::fs::read_to_string("scripts/munin-release-check.ps1").expect("release check");
    assert!(release.contains("MCP protocol smoke"));
}

#[test]
fn privacy_lifecycle_implementation_and_contract_are_packaged() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    for path in ["/src/privacy.rs", "/tests/privacy_contract.rs"] {
        assert!(
            manifest.contains(path),
            "Cargo.toml include list must package {path}"
        );
    }
}

#[test]
fn public_trust_contract_is_complete_and_packaged() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    let required = [
        ("SECURITY.md", "/SECURITY.md"),
        (
            "docs/security/threat-model.md",
            "/docs/security/threat-model.md",
        ),
        (
            "docs/security/incident-response.md",
            "/docs/security/incident-response.md",
        ),
        (
            "docs/privacy/data-lifecycle.md",
            "/docs/privacy/data-lifecycle.md",
        ),
    ];
    for (file, package_path) in required {
        assert!(std::path::Path::new(file).exists(), "missing {file}");
        assert!(
            manifest.contains(package_path),
            "Cargo.toml include list must package {package_path}"
        );
    }

    let security = std::fs::read_to_string("SECURITY.md").expect("SECURITY.md");
    assert!(security.contains("Report a vulnerability"));
    assert!(security.contains("Do not include real transcripts or secrets"));

    let threat = std::fs::read_to_string("docs/security/threat-model.md").expect("threat model");
    for phrase in [
        "Assets",
        "Actors",
        "Trust boundaries",
        "Plugin and MCP risks",
    ] {
        assert!(threat.contains(phrase), "threat model missing {phrase}");
    }

    let incident =
        std::fs::read_to_string("docs/security/incident-response.md").expect("incident response");
    for phrase in ["Severity", "Containment", "Eradication", "Recovery"] {
        assert!(
            incident.contains(phrase),
            "incident response missing {phrase}"
        );
    }

    let lifecycle =
        std::fs::read_to_string("docs/privacy/data-lifecycle.md").expect("data lifecycle");
    for phrase in [
        "90 days",
        "munin privacy inspect",
        "munin privacy export",
        "munin privacy retention",
        "munin privacy forget",
        "FORGET-MUNIN",
        "not encrypted",
        "does not send telemetry",
    ] {
        assert!(
            lifecycle.contains(phrase),
            "data lifecycle missing {phrase}"
        );
    }
}

#[test]
fn provenance_gate_rejects_a_mismatched_binary_then_accepts_the_real_binary() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(manifest.contains("/scripts/munin-provenance-check.ps1"));

    let temp = tempfile::tempdir().expect("temp dir");
    let fake = temp.path().join("fake-munin.ps1");
    std::fs::write(
        &fake,
        r#"$command = $args -join " "
if ($command -eq "--version") {
    Write-Output "munin 9.9.9"
    exit 0
}
if ($command -eq "mcp serve") {
    Write-Output '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","serverInfo":{"name":"munin-memory","version":"9.9.9"}}}'
    exit 0
}
exit 2
"#,
    )
    .expect("fake Munin");

    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = repo.join("scripts/munin-provenance-check.ps1");
    let run = |binary: &std::path::Path| {
        std::process::Command::new("pwsh")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .arg("-MuninBin")
            .arg(binary)
            .arg("-RepoRoot")
            .arg(repo)
            .output()
            .expect("run provenance gate")
    };

    let rejected = run(&fake);
    assert_eq!(rejected.status.code(), Some(1));
    let rejected_output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&rejected.stdout),
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(
        rejected_output.contains("Status: FAIL")
            || rejected_output.contains("version surfaces differ"),
        "provenance falsifier failed without a version diagnosis:\n{rejected_output}"
    );

    let accepted = run(std::path::Path::new(env!("CARGO_BIN_EXE_munin")));
    assert!(
        accepted.status.success(),
        "real provenance gate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&accepted.stdout),
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(String::from_utf8_lossy(&accepted.stdout).contains("Artifacts hashed: 8"));
}

#[test]
fn quality_and_supply_chain_ci_gates_are_executable() {
    let workflow = std::fs::read_to_string(".github/workflows/ci.yml").expect("CI workflow");
    for required in [
        "permissions:\n  contents: read",
        "cargo clippy --locked --all-targets -- -D warnings",
        "dtolnay/rust-toolchain@1.78.0",
        "cargo check --locked --all-targets",
        "command: check licenses advisories",
        "test_old_commands_schema_migrates_to_current_without_losing_rows -- --exact",
        "cargo package --locked",
        "munin-provenance-check.ps1",
    ] {
        assert!(workflow.contains(required), "CI gate missing: {required}");
    }

    let deny = std::fs::read_to_string("deny.toml").expect("deny.toml");
    assert!(deny.contains("[advisories]"));
    assert!(deny.contains("ignore = []"));

    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(manifest.contains("/.cargo/config.toml"));
    assert!(manifest.contains("/scripts/munin-provenance-check.ps1"));
}

#[test]
fn release_check_fails_closed_on_warnings() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let fake_munin = temp_dir.path().join("fake-munin.ps1");
    std::fs::write(
        &fake_munin,
        r#"$CliArgs = @($args)
$ProtocolInput = @($input)
$command = $CliArgs -join " "
if ($command -eq "install --check-resolvable") {
    Write-Output "resolver, skill, and fixture checks passed"
    exit 0
}
if ($command -eq "mcp serve") {
    Write-Output '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"munin-memory","version":"0.5.14"}}}'
    Write-Output '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search","annotations":{"readOnlyHint":true}},{"name":"session_summary","annotations":{"readOnlyHint":true}},{"name":"project_state","annotations":{"readOnlyHint":true}},{"name":"friction","annotations":{"readOnlyHint":true}},{"name":"strategy_status","annotations":{"readOnlyHint":true}},{"name":"health","annotations":{"readOnlyHint":true}}]}}'
    exit 0
}
if ($CliArgs[0] -eq "doctor") {
    Write-Output "Status: warn"
    exit 0
}
if ($CliArgs[0] -eq "prove") {
    Write-Output "Decision: blocked"
    exit 0
}
if ($CliArgs[0] -eq "resolve") {
    Write-Output "Route: friction"
    exit 0
}
Write-Error "unexpected fake Munin arguments: $command"
exit 2
"#,
    )
    .expect("write fake Munin script");

    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_script = repo_root.join("scripts/munin-release-check.ps1");
    let output = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(release_script)
        .arg("-MuninBin")
        .arg(&fake_munin)
        .arg("-RepoRoot")
        .arg(repo_root)
        .output()
        .expect("run release check");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert_eq!(
        output.status.code(),
        Some(1),
        "release warnings must fail closed\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("fix the WARN items"),
        "release warning must name the required action\nstdout:\n{stdout}"
    );
}
