#[test]
fn cargo_toml_marks_crate_apache_licensed() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == r#"license = "Apache-2.0""#),
        "Cargo.toml must declare the Apache-2.0 license"
    );
    assert!(
        !manifest
            .lines()
            .any(|line| line.trim() == "publish = false"),
        "Munin should be publishable during the 0.5 customer-testing cutover"
    );
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
fn release_check_fails_closed_on_warnings() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let fake_munin = temp_dir.path().join("fake-munin.ps1");
    std::fs::write(
        &fake_munin,
        r#"param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CliArgs)
$command = $CliArgs -join " "
if ($command -eq "install --check-resolvable") {
    Write-Output "resolver, skill, and fixture checks passed"
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
