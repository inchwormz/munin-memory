# Munin First-Class Agent Tool Phase 0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Munin's release false-greens and produce an approval-ready, truthfully versioned release candidate without publishing it.

**Architecture:** Keep the existing CLI structure. Make the build script select linker arguments from Cargo's target OS/environment, return Doctor's computed health status to the existing `run_cli() -> Result<i32>` exit path, and make the PowerShell release judge fail closed. Add black-box regression tests at the process/script boundary and a dedicated Windows GNU CI lane.

**Tech Stack:** Rust 2021, Cargo, PowerShell 7, GitHub Actions, MSYS2 MinGW-w64.

---

### Task 1: Prove and repair Windows GNU linking

**Files:**
- Modify: `build.rs`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Reproduce the current linker failure**

Run on this Windows GNU host:

```powershell
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\target-phase0-gnu"
cargo build --locked --target x86_64-pc-windows-gnu --bin munin
```

Expected RED: GNU `ld` reports `cannot find /STACK:8388608`.

- [ ] **Step 2: Make the build script target-aware**

Replace the OS-only branch in `build.rs` with:

```rust
use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => println!("cargo:rustc-link-arg=/STACK:8388608"),
        Ok("gnu") => println!("cargo:rustc-link-arg=-Wl,--stack,8388608"),
        Ok(target_env) => panic!("unsupported Windows target environment `{target_env}`"),
        Err(error) => panic!("Cargo did not provide CARGO_CFG_TARGET_ENV: {error}"),
    }
}
```

- [ ] **Step 3: Verify the original reproduction is green**

Run the exact Step 1 command again.

Expected GREEN: `Finished dev profile`; no `/STACK` linker error.

- [ ] **Step 4: Add a Windows GNU CI lane**

Add a `windows-gnu` job that installs the `x86_64-pc-windows-gnu` target plus MinGW-w64, runs `cargo build --locked --target ... --bin munin`, and runs the test suite for the same target. Keep the existing Windows MSVC lane.

- [ ] **Step 5: Verify formatting and the focused build**

```powershell
cargo fmt --check
cargo build --locked --target x86_64-pc-windows-gnu --bin munin
```

- [ ] **Step 6: Commit the verified unit**

```powershell
git add build.rs .github/workflows/ci.yml
git commit -m "fix: support both Windows Rust linkers"
```

### Task 2: Make Doctor's process status truthful

**Files:**
- Modify: `tests/cli_contract.rs`
- Modify: `src/analytics/memory_os_cmd.rs`
- Modify: `src/bin/munin.rs`

- [ ] **Step 1: Add a black-box failing test**

Add a helper that returns `std::process::Output`, then add a test that runs `munin doctor --format json` and `munin memory-os doctor --format json` with `MUNIN_DATA_DIR`, `MUNIN_CONFIG_DIR`, `MUNIN_SESSION_HOME`, and `MUNIN_INSTALL_HOME` pointed at an empty temporary profile. For each route, parse stdout and assert:

```rust
assert_eq!(report["overall_status"], "fail");
assert_eq!(output.status.code(), Some(2));
```

- [ ] **Step 2: Run the focused test and record RED**

```powershell
cargo test --locked --test cli_contract doctor_failure_returns_exit_two -- --nocapture
```

Expected RED: the JSON status is `fail`, but the actual code is `Some(0)`.

- [ ] **Step 3: Return a typed status from the report layer**

In `memory_os_cmd.rs`, add:

```rust
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MemoryOsDoctorStatus {
    Pass,
    Warn,
    Fail,
}

impl MemoryOsDoctorStatus {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Pass => 0,
            Self::Warn => 1,
            Self::Fail => 2,
        }
    }
}
```

Change `run_doctor` to return `Result<MemoryOsDoctorStatus>` after rendering, mapped directly from `report.overall_status`. Unknown report status is an error, not a guessed success.

- [ ] **Step 4: Preserve the status in both CLI routes**

In top-level `Commands::Doctor`, return `doctor_status.exit_code()` after any release-only checks. In `run_memory_os`, return early from the Doctor match arm with the same code. Other commands continue returning zero.

- [ ] **Step 5: Run focused and unit tests**

```powershell
cargo test --locked --test cli_contract doctor_failure_returns_exit_two -- --nocapture
cargo test --locked run_doctor_rejects_bad_format -- --nocapture
```

Expected GREEN: both Doctor routes return 2 for a JSON `fail`; invalid format still errors.

- [ ] **Step 6: Commit the verified unit**

```powershell
git add tests/cli_contract.rs src/analytics/memory_os_cmd.rs src/bin/munin.rs
git commit -m "fix: make doctor exit codes match health"
```

### Task 3: Make release warnings fail closed

**Files:**
- Modify: `tests/package_contract.rs`
- Modify: `scripts/munin-release-check.ps1`

- [ ] **Step 1: Add a process-level failing test**

Create a temporary fake `munin.ps1` from the Rust test. It must return parseable success for install/resolve, `Status: warn` for Doctor, and a blocked promotion proof. Invoke the real release script with PowerShell and assert:

```rust
assert_eq!(output.status.code(), Some(1));
assert!(stdout.contains("fix the WARN items"));
```

- [ ] **Step 2: Run the focused test and record RED**

```powershell
cargo test --locked --test package_contract release_check_fails_closed_on_warnings -- --nocapture
```

Expected RED: current script exits `Some(0)`.

- [ ] **Step 3: Change the WARN branch only**

Use:

```powershell
if ($results.Status -contains "WARN") {
    Write-Output "Next action: fix the WARN items before trusting this release."
    exit 1
}
```

- [ ] **Step 4: Run the process-level test and real script**

```powershell
cargo test --locked --test package_contract release_check_fails_closed_on_warnings -- --nocapture
./scripts/munin-release-check.ps1 -MuninBin ./target/debug/munin.exe
```

Expected GREEN: the fake warning test exits 1. The real script also exits non-zero while local memory health/promotion proof are not release-ready.

- [ ] **Step 5: Commit the verified unit**

```powershell
git add tests/package_contract.rs scripts/munin-release-check.ps1
git commit -m "fix: fail releases closed on warnings"
```

### Task 4: Restore truthful package and version posture

**Files:**
- Modify: `tests/package_contract.rs`
- Modify: `Cargo.toml`
- Modify: `README.md`
- Modify: `INSTALL.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add failing package-policy tests**

Assert that Cargo metadata contains `publish = false`, INSTALL no longer calls `0.5.1` current, and the README explicitly distinguishes the published crate version from the latest GitHub release.

- [ ] **Step 2: Run and record RED**

```powershell
cargo test --locked --test package_contract release_posture_is_truthful -- --nocapture
```

- [ ] **Step 3: Restore the repository policy**

Add `publish = false` under `[package]`. Do not bump to 0.5.15 and do not create a tag.

- [ ] **Step 4: Align public text without inventing a release**

Document the exact state: crate 0.5.14 exists; latest tagged GitHub release is 0.5.13; the next release candidate is unreleased. Add an `[Unreleased]` changelog section for the verified Phase 0 fixes.

- [ ] **Step 5: Verify package policy and docs**

```powershell
cargo test --locked --test package_contract release_posture_is_truthful -- --nocapture
cargo package --locked --allow-dirty
```

The package command may refuse publication because `publish=false`; it must still prove the package contents can be assembled or produce a clear policy refusal.

- [ ] **Step 6: Commit the verified unit**

```powershell
git add tests/package_contract.rs Cargo.toml README.md INSTALL.md CHANGELOG.md
git commit -m "docs: make Munin release status explicit"
```

### Task 5: Run the complete Phase 0 gate and record the checkpoint

**Files:**
- Modify: `SESSION_LOG.md`

- [ ] **Step 1: Run fresh verification after the last change**

```powershell
cargo fmt --check
cargo build --locked --bin munin
cargo test --locked -- --test-threads 1
cargo run --locked --bin munin -- install --check-resolvable
```

- [ ] **Step 2: Run Windows GNU verification from a quiet, isolated target**

```powershell
$env:PATH = "C:\msys64\mingw64\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\target-phase0-gnu"
cargo test --locked --target x86_64-pc-windows-gnu -- --test-threads 1
```

- [ ] **Step 3: Run release falsifiers**

Run the fake-WARN test and real release script. Confirm the fake warning gate fails for the expected reason. The real release script is allowed to remain red until memory health and promotion evidence are genuinely ready.

- [ ] **Step 4: Recheck source identity and scope**

```powershell
git status --short
git log --oneline --decorate -5
git diff HEAD~4 --stat
```

- [ ] **Step 5: Update the ledger**

Record exact commands, outputs, new SHAs, the gate count, unresolved historical CI-log access, and the next Stage 1 task.

## Plan self-review

- Spec coverage: all Phase 0 audit findings have a task and an observable done condition.
- Scope: publishing is deliberately excluded because it requires action-time approval.
- TDD: Doctor, release WARN behavior, and package policy all begin with tests that fail on current code; the linker fix begins with the actual failing cross-target build.
- Placeholders: none. Stage 1 through Stage 3 remain in `SESSION_LOG.md` and will receive separate detailed plans after Phase 0 contracts are green.
