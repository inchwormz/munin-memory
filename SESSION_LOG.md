# Munin 0.6 execution ledger

## Goal

Turn Munin from a file-copying CLI integration into a first-class local memory tool for Codex and Claude, with truthful release gates, native plugin discovery, typed MCP retrieval, and a documented privacy lifecycle.

## Source of truth

- Audit: `C:\Users\johnr\Documents\Codex\2026-07-14\goa\outputs\munin-memory-audit-2026-07-14.md`
- Starting commit: `7a0fa884da82214c056c7db532b803e413fff05d`
- Branch: `main`
- Remote: `origin https://github.com/inchwormz/munin-memory.git`

## Progress measure

**First-class gates proven: 3 / 10**

1. Windows GNU build and tests pass.
2. Doctor process exit matches its report status.
3. Release warnings fail closed.
4. Version, docs, package policy, tag candidate, and release artifacts agree.
5. Codex recognizes and validates Munin as an installed plugin.
6. Claude recognizes and validates Munin as an installed plugin.
7. Install verification, uninstall, and rollback pass in isolated profiles.
8. Agent hooks return within one second and expose freshness instead of hiding failures.
9. Six read-only MCP tools pass one host-independent contract suite.
10. Privacy lifecycle and release-quality CI gates are documented and executable.

The done condition is **10 / 10**, a clean full suite, and an approval-ready release candidate. Publishing, GitHub releases, and crates.io changes require John’s explicit approval at action time.

## Stage map

### Stage 0 — Restore release truth

Deliverables:

- Target-aware Windows linker flags and a Windows GNU CI lane.
- Doctor exit codes `0=pass`, `1=warn`, `2=fail` for both command routes.
- Release script exits non-zero for WARN or FAIL.
- `publish = false` restored until release approval.
- Public version state describes the real 0.5.13 GitHub / 0.5.14 crate divergence without pretending it is resolved.
- Authenticated historical CI log retrieval attempted and its result recorded.

Verification artifact: `docs/superpowers/plans/2026-07-14-munin-first-class-agent-tool-phase-0.md` plus fresh red/green command output.

### Stage 1 — Native plugin lifecycle

Deliverables:

- Valid Codex marketplace plugin.
- Valid Claude marketplace plugin.
- Host-aware `install --verify`, ownership manifest, uninstall, and rollback.
- Hooks changed from blocking full ingestion to bounded incremental refresh.

Verification artifact: two isolated profiles where each host lists the expected Munin version, validates it, retrieves memory, and returns to its exact pre-install state after uninstall.

### Stage 2 — Typed local memory tools

Deliverables:

- Versioned protocol boundary.
- Local stdio MCP server.
- `search`, `session_summary`, `project_state`, `friction`, `strategy_status`, and `health` tools.
- Provenance, freshness, sensitivity, warnings, and found/returned/dropped receipts in every result.

Verification artifact: one black-box suite run against the same pinned corpus through both Codex and Claude plugin configurations.

### Stage 3 — Trust and scale

Deliverables:

- Threat model, security policy, retention/export/forget controls, and incident process.
- Clippy, advisories, MSRV, migrations, package contents, and provenance gates.
- Only the stable core/protocol/MCP/host boundaries extracted from large modules.

Verification artifact: release-candidate receipt proving install, inspect, retrieve, upgrade, export, forget, rollback, and uninstall.

## Checkpoints

### 2026-07-14 — Orientation

- **Verified:** local `main` is clean and equals `origin/main` at `7a0fa884...`.
- **Verified:** there is no prior repo session log.
- **Verified:** Phase 0 root causes remain present in current code.
- **Decision:** execute inline on the explicitly requested main checkout; do not create a worktree.
- **Decision:** split the audit roadmap into independently verifiable phase plans. Phase 0 is detailed first because later plugin/MCP design must build on truthful process contracts.
- **Next:** execute the Phase 0 TDD plan.

### 2026-07-14 — Phase 0 checkpoint

Source commits, in order:

- `bb41958` — `docs: map Munin first-class execution`
- `52f6554` — `fix: support both Windows Rust linkers`
- `41fe2a3` — `fix: make doctor exit codes match health`
- `5aeb0e0` — `fix: fail releases closed on warnings`
- `43fd635` — `docs: make Munin release status explicit`

Verified red/green receipts:

- **Windows GNU RED:** isolated `cargo build --locked --target x86_64-pc-windows-gnu --bin munin` failed because GNU `ld.exe` received `/STACK:8388608` and reported `cannot find /STACK:8388608`.
- **Windows GNU GREEN:** the exact build passed in 5.28 seconds after target-aware flags; the pre-check full GNU suite then passed 468 tests in 205.8 seconds.
- **Doctor RED:** an isolated empty profile rendered JSON `overall_status: fail` but both command routes returned process code `0`.
- **Doctor GREEN:** both `munin doctor` and `munin memory-os doctor` now return `2` for the same report. The stable mapping test proves `pass=0`, `warn=1`, `fail=2`, and unknown status is an error.
- **Release judge RED:** the real PowerShell judge received parseable WARN results and returned `0`.
- **Release judge GREEN:** the same fake-WARN path returns `1`. A real isolated unhealthy profile also returns `1`, after both 132-check install probes pass.
- **Package truth GREEN:** `cargo package --locked --allow-dirty` assembled and verified 149 files, 2.0 MiB uncompressed and 378.7 KiB compressed. `publish = false` is restored; README and INSTALL explicitly distinguish crates.io `0.5.14` from GitHub `v0.5.13`.

Fresh final gate after the last change:

```text
target: x86_64-pc-windows-gnu
target dir: C:\Users\johnr\Documents\Codex\2026-07-14\goa\work\munin-phase0-final-target
elapsed: 235.4 seconds
tests: 420 + 17 + 4 + 5 + 1 + 1 + 24 doctests = 472 passed, 0 failed
install check-resolvable: 132 resolver, skill, and fixture checks passed
```

Final release falsifier:

```text
[PASS] Binary
[PASS] Install contract from repo (132 checks)
[PASS] Install contract from temp folder (132 checks)
[FAIL] Memory health
[WARN] Promotion proof
[PASS] Resolver routing
exit: 1
```

Gate accounting:

- Gates 1–3 are fully proven locally.
- Gate 4 is **partially repaired, not counted**. Current metadata, docs, package policy, and package contents are truthful, but an eventual `0.6` candidate still needs one version/tag/artifact identity. Phase 0 deliberately did not manufacture or publish a tag.
- Exact historical GitHub failure logs remain blocked: `gh auth status` reports no authenticated GitHub host. The May `0.5.14` CI cause is still unproven.

Not rerun:

- GitHub-hosted Windows MSVC, Linux, macOS, and the new Windows GNU workflow job were not run because this branch has not been pushed.
- No tag, GitHub release, crates.io publish, or production install was created.

Next: write and browser-host the Stage 1 native plugin lifecycle plan, starting from current Codex and Claude plugin specifications and isolated-profile discovery tests.

### 2026-07-14 — Phase 1 orientation

- **Verified false install:** current `munin install --force` wrote 76 files into an isolated profile, including a Codex `plugin.json`; both `codex plugin list --json` and `claude plugin list --json` contained zero Munin plugins.
- **Verified host contracts:** local pins are Codex CLI `0.144.3` and Claude Code `2.1.207`. Codex exposes native marketplace/add/list/remove commands. Claude exposes strict validate, marketplace, install/list/details/eval/uninstall commands.
- **Verified dual-host probe:** one scratch marketplace with `.agents/plugins/marketplace.json`, `.claude-plugin/marketplace.json`, two host manifests, and one shared skill installed successfully into both isolated host caches. Codex reported the plugin enabled at `0.5.14`; Claude reported one skill and about 18 always-on tokens.
- **Verified cleanup:** native plugin removal plus marketplace removal returned both isolated profiles to zero installed plugins. Repeat installation was idempotent in both hosts.
- **Root cause:** Munin writes plausible plugin-shaped files directly into profile directories but never registers a marketplace or asks either host's plugin manager to install them.
- **Decision:** one tracked dual-host marketplace is the source of truth; the binary embeds/materializes it for local installation and delegates lifecycle state to the host managers.
- **Decision:** native payload contains the umbrella plus eight memory skills, not installer/demo skills or duplicate Claude command files.
- **Plan:** `docs/superpowers/plans/2026-07-14-munin-native-plugin-lifecycle-phase-1.md`.
- **Task 1 RED:** the repository contract test failed because `.agents/plugins/marketplace.json` did not exist.
- **Task 1 GREEN:** the tracked dual-host marketplace contains exactly the nine user-facing memory skills; `cargo fmt --check` and the targeted GNU contract test pass; `claude plugin validate --strict .` passes.
- **Real-host proof:** isolated Codex `0.144.3` and Claude Code `2.1.207` profiles each installed and listed `munin-memory@munin-memory` version `0.5.14` as enabled. Native uninstall plus marketplace removal returned both profiles to zero Munin plugins.
- **Task 2 RED:** both process tests failed because `munin plugin` did not exist. The pre-plan isolated witness also proved the legacy direct writer produced 76 files while both native host lists remained empty.
- **Task 2 GREEN:** `munin plugin install|verify|uninstall` delegates to the selected native host manager, materializes a hash-checked versioned marketplace, writes schema-versioned receipts, and keeps legacy `munin install` as a forwarding command.
- **Rollback proof:** a fake Claude install failure after a successful Codex install returned non-zero and removed only the Codex plugin/marketplace and Claude marketplace created by that transaction. The isolated host state returned to four absent state markers.
- **Idempotence proof:** a repeat install reported `bundle_created=false`, `created_marketplace=false`, and `created_plugin=false`.
- **Verification:** the two lifecycle tests and the filtered installer suite passed on `x86_64-pc-windows-gnu`. The compiler now reports the old direct-write installer as dead code; Task 3 removes it and adds reversible migration.
- **Next:** commit Task 2, then add the migration-preservation test before deleting the duplicate installer path.
