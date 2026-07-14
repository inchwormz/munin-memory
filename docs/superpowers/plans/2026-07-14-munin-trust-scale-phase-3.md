# Munin Trust, Privacy, and Release-Candidate Plan

> **Execution rule:** Run this plan test-first, one bounded task at a time. Use isolated Munin and host profiles for every write/delete test. Never run export, retention, or forget against the real user profile. No push, tag, release, registry publish, marketplace submission, or real-profile install without John's action-time approval.

**Goal:** Make Munin a trustworthy first-class local agent tool: fast diagnostic reads, executable data-lifecycle controls, explicit security/incident policy, MSRV/advisory/provenance CI gates, and one approval-ready `0.6.0-rc.1` candidate receipt.

**Progress measure:** Gate 10 moves from unproven to proven when one isolated lifecycle passes inspect, export, retention, project forget, full forget, and artifact verification; CI contains executable clippy, advisory, MSRV, migration, package, and provenance gates. The real-corpus Doctor witness moves from **over 180 seconds / about 1.1 GB** to **under 2 seconds** without importing or mutating sessions. Gate 4 becomes candidate-ready when every local version surface agrees on `0.6.0-rc.1`; creating the tag remains an approval boundary.

**Pinned contracts:** Rust MSRV `1.78`; MCP `2025-11-25`; Codex CLI `0.144.3`; Claude Code `2.1.207`; local-only storage; `publish = false`.

## Architecture

```text
Claude / Codex
      |
      v
native plugin -> MCP read boundary -> compiled Memory OS
                                      |        |
                                      |        +-- fast Doctor (no import)
                                      |
                                      +-- privacy lifecycle boundary
                                          inspect | export | retention | forget

release candidate referee
  +-- format / clippy / tests / MSRV
  +-- migrations / package / provenance / advisories
  +-- native install / rollback / MCP retrieval / uninstall
  +-- isolated export / forget receipts
```

The deterministic lifecycle layer never guesses ownership. It operates only on resolved Munin-owned roots or exact `project_path` database rows. Destructive actions require an explicit confirmation token and emit before/after counts. Exports are local directories with a versioned manifest and SHA-256 hashes; SQLite is snapshotted consistently rather than copied while live.

## Task 1: Make Doctor truly fast and read-only

**Files:** `src/analytics/memory_os_cmd.rs`, `tests/cli_contract.rs`, `scripts/munin-release-check.ps1`.

**RED:** Add a process test that makes the refresh path observably slow/failing and requires both Doctor routes to return from compiled state without invoking it. Preserve the real witness: `munin doctor` exceeded 180 seconds and about 1.1 GB because `run_doctor` calls `refresh_memory_os_session_import_before_read()` before its intentionally fast report builder.

**GREEN:** Remove implicit import from Doctor only. Doctor reports freshness and tells the user to run `munin memory-os ingest --check-now`; it does not repair the thing it is diagnosing. Add an explicit two-second release-check timeout so regression fails closed instead of hanging CI.

**Done:** Both Doctor routes finish in under two seconds on the pinned real compiled corpus and the isolated no-refresh falsifier passes.

## Task 2: Add an executable privacy lifecycle

**Files:** create `src/privacy.rs`, create `tests/privacy_contract.rs`; modify `src/lib.rs`, `src/bin/munin.rs`, `src/core/tracking.rs`, `src/core/config.rs`, `Cargo.toml`.

**RED:** Process tests require absent commands to fail: `munin privacy inspect`, `export`, `retention`, and `forget`. Seed two projects plus an old and new record in an isolated profile.

**GREEN:** Add:

- `privacy inspect` — owned roots, database path/size, configured retention, file/byte counts, and sensitivity warning;
- `privacy export --output <dir>` — consistent SQLite snapshot plus Munin-owned data/config files, versioned manifest, SHA-256 hashes, and no raw source transcripts;
- `privacy retention --days <1..3650> [--apply]` — report-only by default; apply updates `tracking.history_days` and prunes timestamped rows with found/removed/kept receipts;
- `privacy forget --project <exact-path> --confirm FORGET-MUNIN` — deletes exact project rows from every project-scoped table;
- `privacy forget --all --confirm FORGET-MUNIN` — deletes only resolved Munin-owned data/config/database paths and refuses dangerous roots.

The existing automatic cleanup must use configured `tracking.history_days`, not the hard-coded 90-day constant.

**Done:** Export hashes verify; source and export differ only by the intentional snapshot layout; project forget removes only the selected project; full forget leaves source transcript fixtures and unrelated sentinel files byte-for-byte unchanged.

## Task 3: Publish the trust contract

**Files:** create `SECURITY.md`, `docs/security/threat-model.md`, `docs/security/incident-response.md`, `docs/privacy/data-lifecycle.md`; modify `README.md`, `INSTALL.md`, `CHANGELOG.md`, `tests/package_contract.rs`.

**RED:** Package/docs contract fails while disclosure channel, trust boundaries, retention/export/forget commands, backup sensitivity, and incident steps are absent.

**GREEN:** Document assets, actors, entry points, trust boundaries, secret/PII handling, local plugin/MCP risks, export encryption responsibility, default retention, exact deletion boundaries, disclosure expectations, severity/containment/eradication/recovery steps, and what telemetry Munin does not send.

**Done:** Every public privacy claim maps to an executable command or a clearly labelled operator responsibility; all policy files ship in the crate.

## Task 4: Make quality and supply-chain checks executable

**Files:** `.github/workflows/ci.yml`, `deny.toml`, `tests/package_contract.rs`, tracking migration tests; optionally create `scripts/munin-provenance-check.ps1`.

**RED:** Capture current output from `cargo clippy --all-targets -- -D warnings`, `cargo +1.78.0 check --locked --all-targets`, advisory check, migration-open test, and package/provenance check. A gate that cannot be made to fail gets a deliberate bad fixture before it is trusted.

**GREEN:** Add independent CI jobs for:

- clippy with warnings denied;
- Rust `1.78.0` check against the locked graph;
- RustSec advisories through the existing `cargo-deny` action;
- old-schema-to-current migration contract;
- package contents and one provenance receipt tying Cargo, binary, plugin, marketplace, and MCP versions to hashes.

Do not silence real advisories or warnings without a written reason and expiry condition.

**Done:** Local equivalents pass after the last change; workflow YAML has least-privilege permissions and does not require secrets for pull requests.

## Task 5: Cut an approval-ready candidate identity

**Files:** version surfaces in Cargo/plugin/marketplace/docs/tests; create `docs/release/v0.6.0-rc.1.md`; modify release/provenance checks.

**RED:** Identity check reports the current deliberate divergence: package/plugin `0.5.14`, GitHub `v0.5.13`, no `0.6` candidate.

**GREEN:** Set every local candidate surface to `0.6.0-rc.1`, keep `publish = false`, describe the published/GitHub versions separately, and generate a release receipt with commit SHA, binary/package hashes, host pins, test counts, package contents, lifecycle proof, and unresolved external checks. Do not create the tag.

**Done:** One identity checker returns PASS for the local candidate and explicitly reports `tag: not created; approval required`.

## Task 6: Fresh final gate

Run after the last code or documentation change on a quiet machine:

```powershell
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo +1.78.0 check --locked --all-targets
cargo test --locked --target x86_64-pc-windows-gnu -- --test-threads 1
cargo run --locked --target x86_64-pc-windows-gnu --bin munin -- install --check-resolvable
pwsh -NoProfile -File scripts/munin-agent-plugin-check.ps1 -MuninBin <fresh-binary>
pwsh -NoProfile -File scripts/munin-mcp-host-check.ps1 -MuninBin <fresh-binary>
pwsh -NoProfile -File scripts/munin-provenance-check.ps1 -MuninBin <fresh-binary>
cargo package --locked --allow-dirty
claude plugin validate --strict .
```

Also run the isolated privacy lifecycle receipt and the real-corpus fast Doctor timing. Record exact target directory, tool versions, elapsed times, passed/failed counts, before/after digests, hashes, and anything not run.

## Stop conditions

- Doctor still imports or exceeds two seconds twice: stop and re-investigate the call path.
- Export cannot prove a consistent SQLite snapshot: do not ship it.
- Forget touches an unrelated sentinel or cannot enumerate dropped rows: stop immediately.
- MSRV requires weakening `rust-version`: pin or replace the incompatible dependency instead.
- Advisory suppression lacks a concrete unaffected-path reason and review date: do not add it.
- Version identity requires a tag, push, release, registry publish, marketplace submission, real-profile install, or any deletion of real user data: stop for John's explicit approval.

