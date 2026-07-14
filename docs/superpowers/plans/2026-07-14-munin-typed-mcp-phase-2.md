# Munin Typed MCP Phase 2 Implementation Plan

> **Execution rule:** Run this plan test-first, one task at a time. Preserve the Rust 1.78 MSRV, use isolated Munin/host profiles, and do not touch the real user profile.

**Goal:** Expose Munin's compiled local memory to Codex and Claude as six typed, read-only MCP tools with explicit provenance, freshness, sensitivity, warnings, and coverage receipts.

**Progress measure:** Gate 9 moves from unproven to proven when one black-box corpus passes the same six-tool contract through the raw stdio server, the installed Codex plugin declaration, and the installed Claude plugin declaration. Current first-class score: **7 / 10**. Target after this phase: **8 / 10**.

**Pinned contracts:** MCP `2025-11-25`; Codex CLI `0.144.3`; Claude Code `2.1.207`; Rust MSRV `1.78`.

## Premortem

The most likely failure is that a host appears to install the plugin but never starts the MCP server because its manifest shape or executable resolution is wrong. Test that assumption first: install a deliberately broken server declaration in isolated profiles and require both hosts to report a connection failure before accepting any green host result.

The next likely failure is stdout contamination. MCP stdio stdout is protocol-only. Every diagnostic belongs on stderr, and a black-box test must fail if any non-JSON line appears.

The data-risk failure is an unbounded or ambiguous result. Every tool must cap output, label sensitivity, include the source timestamp, and report `found`, `returned`, and `dropped` counts. A result without those fields is invalid even if its content is useful.

## Architecture

```text
Codex plugin / Claude plugin
            |
            | newline-delimited JSON-RPC 2.0 over stdio
            v
       munin mcp serve
            |
            +-- protocol lifecycle and six JSON Schemas
            |
            +-- typed read-only adapters
                    |
                    +-- Tracker recall / session summaries
                    +-- project kernel / Session Brain
                    +-- friction reports
                    +-- strategy status
                    +-- fast doctor health
```

The server is a narrow protocol boundary, not a second memory engine. Existing typed read models remain authoritative. The MCP adapter may filter and cap results, but it must not ingest sessions, mutate strategy, update timestamps, or write tracking events.

No MCP SDK is added in this phase. Munin needs only the stable stdio lifecycle and tools surface; implementing that narrow boundary with existing `serde`/`serde_json` preserves the declared MSRV and avoids an async runtime. The black-box suite is the referee for protocol correctness.

## Shared result contract

Every successful tool returns both MCP text content and identical `structuredContent` conforming to `munin.mcp.result.v1`:

```json
{
  "schemaVersion": "munin.mcp.result.v1",
  "tool": "search",
  "generatedAt": "RFC3339",
  "provenance": [{"sourceKind":"checkpoint","sourceRef":"checkpoint:...","observedAt":"RFC3339 or null"}],
  "freshness": {"status":"fresh|stale|unknown","sourceUpdatedAt":"RFC3339 or null","ageSeconds":0},
  "sensitivity": {"classification":"local-private","containsSecret":false,"containsPii":false,"mustNotPacketize":false},
  "warnings": [],
  "receipt": {"found":0,"returned":0,"dropped":0,"limit":20},
  "data": {}
}
```

Fail closed: if sensitivity cannot be established, report `classification: unknown` and a warning. Never claim `containsSecret: false` merely because no scan ran.

## Tool contract

1. `search`
   - Inputs: required `query`; optional `scope`, `projectPath`, `limit` (1-20).
   - Source: compiled recall matches.
   - Output: ranked matches with evidence and source references.
2. `session_summary`
   - Inputs: required `sessionId`; optional `source`, `projectPath`.
   - Source: exact structured session-summary checkpoint matches.
   - Output: the five-bullet summary plus quality metadata; never fuzzy-match a different session.
3. `project_state`
   - Inputs: optional `projectPath`; optional `limit` (1-20).
   - Source: project kernel plus current Session Brain only when the requested project is the live project.
   - Output: claims, open loops, checkpoints, next actions, and source freshness.
4. `friction`
   - Inputs: optional `scope`, `projectPath`, `limit` (1-20), `includeResolved` (default false).
   - Source: typed friction report.
   - Output: active/monitoring fixes first; resolved history only on request.
5. `strategy_status`
   - Inputs: optional `scope`; default is Munin's configured strategy scope.
   - Source: typed strategy status report.
   - Output: kernel counts, KPI/initiative status, warnings, and evidence freshness.
6. `health`
   - Inputs: optional `scope`, `projectPath`.
   - Source: fast doctor builder without import refresh.
   - Output: overall pass/warn/fail, corpus state, checks, pipeline problems, and permanent fix.

## Task 1: Protocol falsifier and stdio lifecycle

**Files:**

- Create: `src/mcp.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/munin.rs`
- Create: `tests/mcp_contract.rs`

**RED:** Start the current binary with `munin mcp serve`; require `initialize`, `notifications/initialized`, `ping`, and `tools/list`. It must fail because the command does not exist. Feed a deliberately malformed request and prove the harness rejects silence/non-JSON output.

**GREEN:** Add a synchronous newline-delimited JSON-RPC loop. Support MCP `2025-11-25`, echo a supported older client version, advertise only stable tools, answer `ping`, ignore valid notifications, return standard JSON-RPC errors, and keep stdout protocol-only.

**Verify:** The black-box lifecycle test lists exactly six tools, every input schema is an object with `additionalProperties: false`, every tool is annotated read-only, and malformed input gets a deterministic error.

**Commit:** `feat: add Munin MCP protocol boundary`

## Task 2: Typed read-only adapters

**Files:**

- Create: `src/mcp_tools.rs`
- Modify: `src/mcp.rs`
- Modify: `src/lib.rs`
- Modify: `src/core/tracking/recall.rs`
- Modify: `src/analytics/memory_os_cmd.rs`
- Modify: `tests/mcp_contract.rs`

**RED:** Seed one isolated history database and strategy store with known search evidence, one exact session summary, project kernel state, friction, strategy, and doctor data. Call all six tools. Require version/provenance/freshness/sensitivity/warnings/receipt fields and deliberately set `limit=1` against two results to prove `found=2`, `returned=1`, `dropped=1`. The suite must fail because the tools are not callable.

**GREEN:** Add typed input models, exact validation, stable output models, and thin adapters over existing read models. Expose a no-refresh doctor builder and exact session-summary lookup rather than parsing CLI text. Run the existing trust scanner before assigning sensitivity; an incomplete scan reports unknown rather than safe.

**Verify:** All six calls succeed against the same pinned corpus, output schema validation passes, invalid inputs return `isError: true`, and a before/after digest proves the database and strategy files were not changed.

**Commit:** `feat: expose six typed Munin memory tools`

## Task 3: Native host declarations and false-green proof

**Files:**

- Create: `plugins/munin-memory/.mcp.json`
- Modify: `plugins/munin-memory/.codex-plugin/plugin.json`
- Modify: `plugins/munin-memory/.claude-plugin/plugin.json`
- Modify: `src/agent_plugin.rs`
- Modify: `tests/agent_plugin_contract.rs`
- Create: `scripts/munin-mcp-host-check.ps1`

**RED:** Install a deliberately broken `munin-does-not-exist mcp serve` declaration into isolated Codex and Claude profiles. Require each host to recognize the server but report it unhealthy. If either host silently passes, the host judge is false-green and implementation stops.

**GREEN:** Ship the dual host declaration using plugin-root-relative configuration and the installed `munin` executable. Codex's manifest points `mcpServers` at `./.mcp.json`; Claude loads the same plugin-local server declaration. Extend install verification receipts with MCP server identity and health.

**Verify:** In fresh isolated profiles, both hosts install the plugin, enumerate the Munin MCP server, list exactly six tools, call the pinned `health` tool, uninstall, and return to byte-equivalent pre-state. No real profile is changed.

**Commit:** `feat: connect native plugins to Munin MCP`

## Task 4: Package and operator contract

**Files:**

- Modify: `README.md`
- Modify: `INSTALL.md`
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `tests/package_contract.rs`
- Modify: `scripts/munin-release-check.ps1`

**RED:** Require the package and release contract to contain the MCP server source, host declaration, six-tool contract test, and host check script. It must fail before the include list and release judge are updated.

**GREEN:** Document the six read-only tools, local-only data path, sensitivity labels, host install behavior, and troubleshooting. Add the MCP host check to release gates in report-only mode for Stage 2; Stage 3 decides when it becomes fail-closed across CI platforms.

**Verify:** `cargo package --locked --allow-dirty`, package content assertions, resolver checks, strict Claude validation, and both host lifecycle checks pass.

**Commit:** `docs: prove typed MCP integration`

## Task 5: Fresh phase gate

Run after the final change:

```powershell
cargo fmt --check
cargo check --locked --target x86_64-pc-windows-gnu
cargo test --locked --target x86_64-pc-windows-gnu
pwsh -NoProfile -File scripts/munin-mcp-host-check.ps1
pwsh -NoProfile -File scripts/munin-agent-plugin-check.ps1
cargo run --locked --target x86_64-pc-windows-gnu --bin munin -- install --check-resolvable
cargo package --locked --allow-dirty
claude plugin validate --strict .
```

Record exact versions, target directory, elapsed time, passed/failed counts, host tool lists, falsifier result, read-only digest result, package contents, and anything not run. Then update `SESSION_LOG.md` and gate 9 to proven only if every subject passed after the last change.

## Stop conditions

- Same protocol failure signature twice: stop editing and replace the theory.
- A host cannot be made to fail with a broken executable: do not trust its green result.
- Any tool mutates the pinned corpus: stop; the adapter is not read-only.
- Current data lacks provenance or sensitivity evidence: return explicit unknown plus warning; never fabricate a safe label.
- No release, tag, registry publish, marketplace submission, real-profile install, or push without John's approval.
