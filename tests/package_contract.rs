#[test]
fn cargo_toml_marks_crate_local_only() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == "publish = false"),
        "Cargo.toml must keep Munin local-only until a release process is approved"
    );
}

#[test]
fn readme_does_not_claim_crates_io_install() {
    let readme = std::fs::read_to_string("README.md").expect("README.md");
    assert!(!readme.contains("cargo install munin-memory"));
    assert!(!readme.to_lowercase().contains("crates.io"));
}
