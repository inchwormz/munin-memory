use rusqlite::{params, Connection};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(profile: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_munin"))
        .args(args)
        .env("MUNIN_DATA_DIR", profile.join("data"))
        .env("MUNIN_CONFIG_DIR", profile.join("config"))
        .env("MUNIN_SESSION_HOME", profile.join("sessions"))
        .env("MUNIN_INSTALL_HOME", profile.join("install"))
        .output()
        .unwrap_or_else(|error| panic!("failed to run munin {args:?}: {error}"))
}

fn run_with_data_root(profile: &Path, data_root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_munin"))
        .args(args)
        .env("MUNIN_DATA_DIR", data_root)
        .env("MUNIN_CONFIG_DIR", profile.join("config"))
        .env("MUNIN_SESSION_HOME", profile.join("sessions"))
        .env("MUNIN_INSTALL_HOME", profile.join("install"))
        .output()
        .unwrap_or_else(|error| panic!("failed to run munin {args:?}: {error}"))
}

fn json(output: &Output, args: &[&str]) -> Value {
    assert!(
        output.status.success(),
        "munin {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "munin {args:?} returned invalid JSON: {error}\nstdout:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

#[test]
fn privacy_lifecycle_exports_retains_and_forgets_only_owned_data() {
    let temp = tempfile::tempdir().expect("temp root");
    let profile = temp.path().join("profile");
    fs::create_dir_all(profile.join("sessions")).expect("session dir");
    let source_sentinel = profile.join("sessions/source-transcript-sentinel.jsonl");
    fs::write(&source_sentinel, "raw source stays outside Munin ownership")
        .expect("source sentinel");

    let doctor = run(&profile, &["doctor", "--format", "json"]);
    assert_eq!(doctor.status.code(), Some(2), "initialize schema");
    seed_commands(&profile.join("data/history.db"));

    let inspect_args = ["privacy", "inspect", "--format", "json"];
    let inspect = json(&run(&profile, &inspect_args), &inspect_args);
    assert_eq!(inspect["schema"], "munin.privacy.inspect.v1");
    assert_eq!(inspect["retention_days"], 90);
    assert_eq!(inspect["database"]["exists"], true);
    assert!(inspect["owned_bytes"].as_u64().unwrap() > 0);
    assert_eq!(inspect["source_transcripts_included"], false);

    let export_dir = temp.path().join("export");
    let export_text = export_dir.to_string_lossy().to_string();
    let export_args = [
        "privacy",
        "export",
        "--output",
        export_text.as_str(),
        "--format",
        "json",
    ];
    let export = json(&run(&profile, &export_args), &export_args);
    assert_eq!(export["schema"], "munin.privacy.export.v1");
    assert_eq!(export["database_snapshot"], "data/history.db");
    assert!(!export["files"].as_array().unwrap().is_empty());
    assert!(export_dir.join("manifest.json").exists());
    assert!(export_dir.join("data/history.db").exists());
    assert!(!export_dir.join("sessions").exists());
    verify_manifest_hashes(&export_dir);

    let preview_args = ["privacy", "retention", "--days", "30", "--format", "json"];
    let preview = json(&run(&profile, &preview_args), &preview_args);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["eligible"], 1);
    assert_eq!(count_commands(&profile, None), 3);
    assert!(!profile.join("config/config.toml").exists());

    let apply_args = [
        "privacy",
        "retention",
        "--days",
        "30",
        "--apply",
        "--format",
        "json",
    ];
    let applied = json(&run(&profile, &apply_args), &apply_args);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["removed"], 1);
    assert_eq!(count_commands(&profile, None), 2);
    let config = fs::read_to_string(profile.join("config/config.toml")).expect("saved config");
    assert!(config.contains("history_days = 30"));

    let bad_forget_args = [
        "privacy",
        "forget",
        "--project",
        "C:/alpha",
        "--confirm",
        "wrong",
        "--format",
        "json",
    ];
    let rejected = run(&profile, &bad_forget_args);
    assert!(!rejected.status.success());
    assert_eq!(count_commands(&profile, Some("C:/alpha")), 1);

    let project_forget_args = [
        "privacy",
        "forget",
        "--project",
        "C:/alpha",
        "--confirm",
        "FORGET-MUNIN",
        "--format",
        "json",
    ];
    let project_forget = json(&run(&profile, &project_forget_args), &project_forget_args);
    assert_eq!(project_forget["schema"], "munin.privacy.forget.v1");
    assert_eq!(project_forget["scope"], "project");
    assert_eq!(project_forget["removed"], 1);
    assert_eq!(count_commands(&profile, Some("C:/alpha")), 0);
    assert_eq!(count_commands(&profile, Some("C:/beta")), 1);

    let full_forget_args = [
        "privacy",
        "forget",
        "--all",
        "--confirm",
        "FORGET-MUNIN",
        "--format",
        "json",
    ];
    let full_forget = json(&run(&profile, &full_forget_args), &full_forget_args);
    assert_eq!(full_forget["scope"], "all");
    assert!(full_forget["removed_files"].as_u64().unwrap() > 0);
    assert!(!profile.join("data").exists());
    assert!(!profile.join("config").exists());
    assert_eq!(
        fs::read_to_string(&source_sentinel).expect("source sentinel survives"),
        "raw source stays outside Munin ownership"
    );
    assert!(export_dir.join("manifest.json").exists());
}

#[test]
fn privacy_forget_refuses_the_current_workspace_as_an_owned_root() {
    let temp = tempfile::tempdir().expect("temp root");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sentinel = workspace.join("Cargo.toml");
    let before = fs::read(&sentinel).expect("workspace sentinel");
    let args = [
        "privacy",
        "forget",
        "--all",
        "--confirm",
        "FORGET-MUNIN",
        "--format",
        "json",
    ];
    let output = run_with_data_root(temp.path(), workspace, &args);

    assert!(!output.status.success(), "dangerous root must fail closed");
    assert!(String::from_utf8_lossy(&output.stderr).contains("current workspace"));
    assert_eq!(fs::read(&sentinel).expect("workspace survives"), before);
}

fn seed_commands(db_path: &Path) {
    let conn = Connection::open(db_path).expect("open history db");
    let insert = "INSERT INTO commands (
        timestamp, original_cmd, context_cmd, input_tokens, output_tokens,
        saved_tokens, savings_pct, exec_time_ms, project_path
    ) VALUES (?1, ?2, ?2, 1, 1, 0, 0.0, 1, ?3)";
    conn.execute(insert, params!["2025-01-01T00:00:00Z", "old", "C:/alpha"])
        .expect("old command");
    conn.execute(
        insert,
        params![chrono::Utc::now().to_rfc3339(), "alpha", "C:/alpha"],
    )
    .expect("alpha command");
    conn.execute(
        insert,
        params![chrono::Utc::now().to_rfc3339(), "beta", "C:/beta"],
    )
    .expect("beta command");
}

fn count_commands(profile: &Path, project: Option<&str>) -> i64 {
    let conn = Connection::open(profile.join("data/history.db")).expect("open history db");
    match project {
        Some(project) => conn
            .query_row(
                "SELECT COUNT(*) FROM commands WHERE project_path = ?1",
                [project],
                |row| row.get(0),
            )
            .expect("project count"),
        None => conn
            .query_row("SELECT COUNT(*) FROM commands", [], |row| row.get(0))
            .expect("command count"),
    }
}

fn verify_manifest_hashes(export_dir: &Path) {
    let manifest: Value = serde_json::from_slice(
        &fs::read(export_dir.join("manifest.json")).expect("manifest bytes"),
    )
    .expect("manifest json");
    for file in manifest["files"].as_array().expect("manifest files") {
        let relative = file["path"].as_str().expect("file path");
        let bytes = fs::read(export_dir.join(PathBuf::from(relative))).expect("exported file");
        use sha2::{Digest, Sha256};
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), file["sha256"]);
    }
}
