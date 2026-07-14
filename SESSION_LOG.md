# Munin 0.6 execution ledger

## Goal

Turn Munin from a file-copying CLI integration into a first-class local memory tool for Codex and Claude, with truthful release gates, native plugin discovery, typed MCP retrieval, and a documented privacy lifecycle.

## Source of truth

- Audit: `C:\Users\johnr\Documents\Codex\2026-07-14\goa\outputs\munin-memory-audit-2026-07-14.md`
- Starting commit: `7a0fa884da82214c056c7db532b803e413fff05d`
- Branch: `main`
- Remote: `origin https://github.com/inchwormz/munin-memory.git`

## Progress measure

**First-class gates proven: 7 / 10**

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
- **Task 3 RED:** native installation left direct Codex/Claude skills, commands, plugin files, and blocking hook entries active; no migration backup was reported.
- **Task 3 GREEN:** positively identified legacy files move to a timestamped `.munin/migration-backups` tree after native install succeeds. Original Claude settings and Codex hooks are backed up before only Munin-owned entries are pruned.
- **Preservation proof:** unrelated Claude/Codex hook entries and the Claude `theme` value survived. An unknown `munin-custom` skill remained untouched and appeared in the receipt conflict list.
- **Verification:** all four `agent_plugin_contract` tests passed on `x86_64-pc-windows-gnu`.
- **Task 4 RED:** `munin hook` did not exist, and the repository lacked both native plugin hook manifests and the shared script.
- **Task 4 GREEN:** `munin hook session-start` returns a versioned freshness receipt, acquires a short lease, queues a hidden worker, and exits. The worker runs incremental `memory-os ingest --check-now`, records pass/fail truth, preserves the last known successful refresh, and clears the lease.
- **Failure proof:** forced worker failure produced `munin.agent-hook-status.v1` with `status=fail`, `last_refresh_at=null`, and `last_error=forced hook worker failure`; the next session-start receipt surfaced `status=refresh-failed`.
- **Latency proof:** the pinned GNU debug binary returned the session-start receipt in 55 ms on the quiet local machine. The automated ceiling remains two seconds.
- **Shipped-hook proof:** Node syntax check, Claude strict validation, and repository assertions prove there is no `spawnSync`, 120-second timeout, or direct ingestion command in the plugin hook assets.
- **Cleanup:** removed 1,098 lines of unreachable direct installer/global-hook code and replaced the old generated-asset check with validation of the embedded native marketplace. `cargo check` is warning-free.
- **Verification:** all five `agent_plugin_contract` tests plus the generated-skill resolver contract passed on `x86_64-pc-windows-gnu`.
- **Task 5 RED:** the package contract failed because the real-host gate script was missing from both the repository and Cargo package include list.
- **Task 5 GREEN:** `scripts/munin-agent-plugin-check.ps1` creates isolated native profiles, proves install/list/details/verify, forces a partial failure, measures the hook, proves the worker, uninstalls, and compares native post-state to pre-state.
- **Real-host receipt:** Codex CLI `0.144.3` and Claude Code `2.1.207` both listed `munin-memory@munin-memory` version `0.5.14` enabled. Forced rollback exited `1`; the final session-start receipt returned in `125 ms`; worker status was `pass`; uninstall restored all four native list/marketplace surfaces to their pre-state.
- **Receipt root:** `C:\Users\johnr\AppData\Local\Munin\checks\munin-agent-plugin-check-20260714-122659-034-15312`.
- **Fresh Stage 1 suite:** clean target `C:\Users\johnr\Documents\Codex\2026-07-14\goa\work\munin-phase1-final-20260714`; `420 + 9 + 5 + 4 + 6 + 1 + 1 + 24 doctests = 470` passed, `0` failed, in `226` seconds. Production `cargo check` is warning-free; one pre-existing test-only dead-code warning remains in `core/artifacts.rs`.
- **Artifact gate:** `install --check-resolvable` passed `136` checks; `cargo package --locked --allow-dirty` assembled and verified `169` files, `2.1 MiB` uncompressed and `393.1 KiB` compressed; Claude strict validation passed.
- **Gate accounting:** gates 5–8 are now fully proven. Gate 4 remains deliberately uncounted until an approved `0.6` version/tag/artifact identity exists. Gates 9–10 are Stage 2 typed MCP and Stage 3 trust/scale work.
- **Not run:** GitHub-hosted CI was not run because nothing was pushed. No real user profile was changed. No tag, release, registry publish, marketplace submission, or deployment was performed.
- **Next:** commit Task 5, then write the Stage 2 typed MCP protocol plan from the six-tool contract in this ledger.

### 2026-07-14 — Phase 2 orientation

- **Verified repo state:** `main` is clean at `d2d0c702e3c0e51365ebf182c4269177658bae24`; `origin/main` remains `7a0fa884da82214c056c7db532b803e413fff05d` because nothing has been pushed.
- **Verified protocol pin:** MCP `2025-11-25` is the latest stable specification. Its stdio transport is newline-delimited UTF-8 JSON-RPC; stdout is protocol-only and stderr may carry diagnostics.
- **Verified host pins:** Codex CLI `0.144.3` exposes `codex mcp add/list/get/remove`; Claude Code `2.1.207` exposes the corresponding MCP lifecycle.
- **Verified plugin shape:** installed OpenAI Codex plugins point `mcpServers` at a plugin-local `.mcp.json`; installed Claude plugins load a plugin-root `.mcp.json` server map.
- **Decision:** implement the narrow synchronous stdio lifecycle with existing `serde`/`serde_json` instead of adding an async SDK/runtime. This preserves Rust `1.78` and keeps one compiled Munin binary as the only runtime dependency.
- **Decision:** the six tools are read-only adapters over existing typed read models. They must not run session import refresh, write tracking events, update strategy, or infer missing sensitivity as safe.
- **Premortem check:** the Stage 2 host gate first installs a broken executable declaration and must observe both hosts fail. A host check that cannot detect the broken server is rejected as false-green.
- **Plan:** `docs/superpowers/plans/2026-07-14-munin-typed-mcp-phase-2.md` plus the browser plan at the adjacent `.html` path.
- **Next:** Task 1 RED — run the black-box lifecycle test against the absent `munin mcp serve` command and prove the judge rejects silence.
- **Task 1 instrument correction:** the first GNU test attempt never reached Munin because `dlltool.exe` was absent from this shell's PATH. Pinning `C:\msys64\mingw64\bin` reached the actual subject; the instrument failure is not counted as product RED.
- **Task 1 RED:** both black-box tests failed against the current binary. `munin mcp serve` was an unrecognized subcommand, and malformed input produced process failure plus no JSON-RPC error response.
- **Task 1 GREEN:** `munin mcp serve` now implements synchronous newline-delimited JSON-RPC with initialize/version negotiation, initialized notifications, ping, tools/list, deterministic errors, and protocol-only stdout. It lists exactly six fixed read-only tools.
- **Compatibility proof:** the server advertises MCP `2025-11-25` and echoes a supported `2025-06-18` client version. Unsupported versions negotiate to the current server pin.
- **Verification:** `tests/mcp_contract.rs` passed `2 / 2`; the focused unit compatibility test passed `1 / 1`; the command completed with exit `0` after EOF.
- **Next:** commit Task 1, then seed the typed six-tool corpus and capture the expected Task 2 RED.
- **Task 2 RED:** the pinned isolated corpus reached `tools/call`, but `search` returned `isError=true` with `declared but not implemented yet`; the six-tool contract failed at the first adapter.
- **Task 2 mechanism:** added one versioned `munin.mcp.result.v1` envelope and thin read-only adapters for search, exact session summary, project state, friction, strategy status, and health. Existing typed read models remain authoritative.
- **Exactness:** session summary lookup matches exact `sessionId` plus optional source/project; it does not reuse fuzzy topic recall. Nested legacy structs are normalized to camelCase only at the MCP boundary, leaving existing CLI JSON unchanged.
- **Read-only correction:** strategy now has explicit `inspect_read_only` and `status_read_only` paths. They do not perform the existing empty-kernel rehydration write. MCP health also uses the read-only strategy inspect path.
- **Privacy behavior:** every result runs the existing trust scanner. Secret-shaped values are redacted before output, sensitivity is labeled, and a focused falsifier proved `api_key=...` becomes `[REDACTED]` with `classification=restricted`.
- **Coverage proof:** seeded two resolver matches and requested `limit=1`; the result reported at least `found=2`, `returned=1`, and `dropped=found-1`.
- **Fresh verification:** all four process-level MCP contract tests passed, including malformed protocol input and invalid tool arguments. The secret-redaction unit test passed. GNU `cargo check --locked` completed warning-free.
- **Mutation proof:** a recursive SHA-256 digest of the isolated profile (excluding SQLite transient WAL/SHM files) was identical before and after all six calls, including strategy and health.
- **Next:** commit Task 2, then falsify and implement native Codex/Claude MCP declarations in isolated profiles.
- **Task 3 host-shape correction:** Claude consumes a flat plugin-root `.mcp.json`, while Codex requires a wrapped `mcpServers` document referenced by its manifest. The bundle therefore carries `.mcp.json` for Claude and `.codex-mcp.json` for Codex; both launch `munin mcp serve`.
- **Task 3 RED:** both declarations first launched `munin-does-not-exist`. Native installation still looked successful, but Claude reported `Failed to connect`; Codex app-server listed `munin-memory` with `serverInfo=null` and `tools={}`. This proves install/list alone is a false-green judge.
- **Host-discovered bug:** the first connected Claude install reported `Duplicate hooks file detected` because its manifest explicitly referenced `hooks/hooks.json`, which Claude auto-loads. A repository contract reproduced the error; removing the duplicate manifest field left the installed plugin error-free.
- **Discovery compatibility bug:** Codex listed all six tools but warned that `resources/list` and `resources/templates/list` were unimplemented. Focused protocol tests failed first; the server now returns explicit empty resource surfaces and the warnings are absent.
- **Task 3 GREEN:** `scripts/munin-mcp-host-check.ps1` installs into fresh isolated profiles, substitutes a deliberately failing runtime, requires Claude to disconnect and Codex to expose zero tools, then restores the real binary. The real Codex app-server exposes exactly `friction, health, project_state, search, session_summary, strategy_status`; a native `mcpServer/tool/call` to `health` returns `munin.mcp.result.v1` with `isError=false`; Claude reports `Connected`.
- **Host pins and cleanup:** Codex CLI `0.144.3`, Claude Code `2.1.207`, and Munin `0.5.14` were exercised. Native uninstall restored all four plugin/marketplace list surfaces to their pre-state. Receipt root: `C:\Users\johnr\Documents\Codex\2026-07-14\goa\work\munin-mcp-host-checks\munin-mcp-host-check-20260714-125505-250-36112`.
- **Fresh focused verification:** all five MCP process tests passed; the dual-host repository contract passed; Claude strict marketplace validation passed; the permanent native MCP host check passed.
- **Next:** commit Task 3, then add package/docs/release coverage for the typed MCP surface.
- **Task 4 RED:** the package contract failed because `src/mcp.rs`, `src/mcp_tools.rs`, `tests/mcp_contract.rs`, and the native MCP host gate were absent from the crate include list; README and INSTALL still said typed MCP was deferred.
- **Task 4 GREEN:** crate metadata now packages the MCP protocol, adapters, process contract, host gate, and both native declarations. README, INSTALL, and CHANGELOG document the six tools, strict result envelope, privacy/coverage fields, host pins, and falsifiable host gate.
- **Release judge:** the default release check now performs a real JSON-RPC initialize/tools-list smoke and requires exactly six read-only tools. `-NativeMcpHosts` opts into the installed Claude/Codex falsifier and health call; the default prints an explicit SKIP receipt instead of implying host coverage.
- **Real release receipt:** in an isolated empty profile the actual binary reported 138 install checks from both working directories, `MCP protocol smoke` PASS, resolver PASS, memory health FAIL, promotion WARN, native hosts SKIP, and exit `1`. The judge remains correctly release-blocked.
- **Scale finding:** an accidental release-check run against the real user corpus left `munin doctor` active beyond three minutes at roughly 1.1 GB working set. The bounded run was stopped and is not a passing result. Stage 3 must diagnose this path before calling the health surface production-grade.
- **Package proof:** all seven package-contract tests passed. `cargo package --locked --allow-dirty` packaged and verified 175 files, 2.1 MiB uncompressed and 408.2 KiB compressed. The package list contains both MCP source files, `tests/mcp_contract.rs`, the host gate, and both plugin declarations.
- **Next:** commit Task 4, then start the Stage 3 privacy/lifecycle/trust/scale plan with the real-corpus Doctor performance failure as the first reproduced bug.

### 2026-07-14 — Phase 3 orientation

- **Verified scale RED:** the top-level Doctor route still calls `refresh_memory_os_session_import_before_read()` before a report builder explicitly designed as a fast compiled-state check. On the real corpus it exceeded 180 seconds, consumed about 1.1 GB, and was stopped; no pass is claimed.
- **Verified lifecycle gap:** the CLI has no export or forget surface. `tracking.history_days` is configurable but automatic cleanup still uses the hard-coded 90-day constant.
- **Verified policy gap:** the repository has no `SECURITY.md`, threat model, data lifecycle, or incident-response document.
- **Verified CI gap:** stable build/test, licenses, and Windows GNU exist; warnings-denied clippy, locked Rust 1.78, advisories, migration, and provenance gates do not.
- **Boundary inventory:** native host lifecycle (`src/agent_plugin.rs`) and MCP protocol/adapters (`src/mcp.rs`, `src/mcp_tools.rs`) are already separated. Stage 3 adds one privacy lifecycle boundary and does not perform speculative rewrites of the remaining large modules.
- **Plan:** `docs/superpowers/plans/2026-07-14-munin-trust-scale-phase-3.md`; browser plan verified HTTP 200 at the adjacent `.html` URL.
- **Next:** commit the Stage 3 plan, then add the Doctor no-refresh falsifier before changing the call path.
