use std::path::Path;
use std::process::{Command, Output};

fn munin(args: &[&str]) -> String {
    munin_from_dir(args, None)
}

fn munin_from_dir(args: &[&str], current_dir: Option<&std::path::Path>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_munin"));
    command.args(args);
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run munin {args:?}: {error}"));
    assert!(
        output.status.success(),
        "munin {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn munin_in_empty_profile(args: &[&str], profile: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_munin"));
    command
        .args(args)
        .env("MUNIN_DATA_DIR", profile.join("data"))
        .env("MUNIN_CONFIG_DIR", profile.join("config"))
        .env("MUNIN_SESSION_HOME", profile.join("sessions"))
        .env("MUNIN_INSTALL_HOME", profile.join("install"));
    command
        .output()
        .unwrap_or_else(|error| panic!("failed to run munin {args:?}: {error}"))
}

#[test]
fn resolve_routes_friction_question() {
    let output = munin(&[
        "resolve", "--format", "json", "what", "keeps", "going", "wrong",
    ]);
    assert!(output.contains("\"route\": \"friction\""));
    assert!(output.contains("munin friction --last 30d --format text"));
}

#[test]
fn generated_skills_and_resolver_targets_are_parseable() {
    let output = munin(&["install", "--check-resolvable"]);
    assert!(output.contains("resolver, skill, and fixture checks passed"));
}

#[test]
fn check_resolvable_does_not_depend_on_current_directory() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let output = munin_from_dir(&["install", "--check-resolvable"], Some(temp_dir.path()));
    assert!(output.contains("resolver, skill, and fixture checks passed"));
}

#[test]
fn doctor_failure_returns_exit_two() {
    for args in [
        &["doctor", "--format", "json"][..],
        &["memory-os", "doctor", "--format", "json"][..],
    ] {
        let profile = tempfile::tempdir().expect("temp profile");
        let output = munin_in_empty_profile(args, profile.path());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
            panic!(
                "munin {args:?} returned invalid JSON: {error}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });

        assert_eq!(report["overall_status"], "fail", "munin {args:?}");
        assert_eq!(
            output.status.code(),
            Some(2),
            "munin {args:?} should fail with status 2\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn doctor_reads_compiled_state_without_refreshing_session_import() {
    for args in [
        &["doctor", "--format", "json"][..],
        &["memory-os", "doctor", "--format", "json"][..],
    ] {
        let profile = tempfile::tempdir().expect("temp profile");
        let started = std::time::Instant::now();
        let output = munin_in_empty_profile(args, profile.path());
        let elapsed = started.elapsed();
        assert_eq!(output.status.code(), Some(2), "munin {args:?}");
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "munin {args:?} exceeded the fast Doctor ceiling: {elapsed:?}"
        );
        for refresh_state in [
            "memory_os_session_onboarding.json",
            "memory_os_session_onboarding.lock.sqlite",
        ] {
            assert!(
                !profile.path().join("data").join(refresh_state).exists(),
                "munin {args:?} implicitly refreshed session import and wrote {refresh_state}"
            );
        }
    }
}
