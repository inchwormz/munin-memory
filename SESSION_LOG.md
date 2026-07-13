# Munin 0.6 execution ledger

## Goal

Turn Munin from a file-copying CLI integration into a first-class local memory tool for Codex and Claude, with truthful release gates, native plugin discovery, typed MCP retrieval, and a documented privacy lifecycle.

## Source of truth

- Audit: `C:\Users\johnr\Documents\Codex\2026-07-14\goa\outputs\munin-memory-audit-2026-07-14.md`
- Starting commit: `7a0fa884da82214c056c7db532b803e413fff05d`
- Branch: `main`
- Remote: `origin https://github.com/inchwormz/munin-memory.git`

## Progress measure

**First-class gates proven: 0 / 10**

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
