# Munin Native Plugin Lifecycle Phase 1 Implementation Plan

> **Execution:** Follow this plan task by task with red/green tests and a fresh host-level check after the final change.

**Goal:** Make Munin a plugin that current Codex and Claude actually discover, validate, install, verify, and uninstall, while replacing blocking profile hooks with a bounded freshness-aware refresh trigger.

**Architecture:** Ship one tracked dual-host marketplace in the repository. Both hosts install the same `plugins/munin-memory` payload through their native plugin managers, using host-specific marketplace and plugin manifests. The Munin binary embeds that tracked payload for offline/local installation, materializes it into a Munin-owned versioned directory, and delegates install/remove to `codex plugin` and `claude plugin`. A transaction receipt records what Munin created so partial installs can roll back without touching pre-existing plugins or unrelated settings.

**Product decision:** The native plugin contains the umbrella Munin skill plus the eight user-facing memory skills. It does not install seven installer/demo skills, duplicate Claude command files, duplicate Codex skill trees, or mutate global hook settings. Typed MCP tools arrive in Phase 2.

**Current host pins for local proof:** Codex CLI `0.144.3`; Claude Code `2.1.207`; plugin version remains the current Cargo version until the eventual `0.6` release candidate is assembled.

---

## Task 1: Add one real dual-host marketplace

**Files:**

- Create: `.agents/plugins/marketplace.json`
- Create: `.claude-plugin/marketplace.json`
- Create: `plugins/munin-memory/.codex-plugin/plugin.json`
- Create: `plugins/munin-memory/.claude-plugin/plugin.json`
- Create: `plugins/munin-memory/skills/*/SKILL.md`
- Create: `tests/agent_plugin_contract.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add the failing repository-contract test**

Assert that both marketplace manifests exist, point at `./plugins/munin-memory`, use marketplace name `munin-memory`, and that both plugin manifests agree with `Cargo.toml` on name/version. Assert that the payload contains exactly the umbrella plus the eight user-facing memory skills and contains no installer/demo skills or flat `commands/` duplicates.

- [ ] **Step 2: Run RED**

```powershell
cargo test --locked --test agent_plugin_contract repository_marketplace_is_dual_host -- --nocapture
```

Expected: missing marketplace files.

- [ ] **Step 3: Add the tracked payload**

Use current host-native layouts:

```text
.agents/plugins/marketplace.json
.claude-plugin/marketplace.json
plugins/munin-memory/
  .codex-plugin/plugin.json
  .claude-plugin/plugin.json
  skills/<name>/SKILL.md
```

Codex manifest declares `skills: "./skills/"`. Claude manifest declares the same skill root and later its host-specific hook file. Both manifests include author, repository, Apache-2.0 license, and a concise interface description.

- [ ] **Step 4: Validate with both real hosts**

```powershell
claude plugin validate --strict .
$env:CODEX_HOME = "<empty-isolated-codex-home>"
codex plugin marketplace add . --json
codex plugin add munin-memory@munin-memory --json
codex plugin list --json
```

Expected: Claude strict validation passes; Codex lists `munin-memory@munin-memory` enabled at the Cargo version.

- [ ] **Step 5: Commit**

```powershell
git add .agents/plugins/marketplace.json .claude-plugin/marketplace.json plugins/munin-memory tests/agent_plugin_contract.rs Cargo.toml
git commit -m "feat: ship native Munin plugin marketplace"
```

## Task 2: Add a transactional host lifecycle

**Files:**

- Create: `src/agent_plugin.rs`
- Modify: `src/lib.rs`
- Modify: `src/bin/munin.rs`
- Modify: `tests/agent_plugin_contract.rs`

- [ ] **Step 1: Add a black-box false-install test**

Run the current installer in an isolated `MUNIN_INSTALL_HOME`, then query both real host lists. Preserve the already-proven red expectation as a regression witness: files exist, but neither host lists Munin.

- [ ] **Step 2: Add failing lifecycle contract tests**

Add process-level tests for:

- `munin plugin install --codex --format json`
- `munin plugin install --claude --format json`
- `munin plugin verify --codex --format json`
- `munin plugin verify --claude --format json`
- `munin plugin uninstall --codex --format json`
- `munin plugin uninstall --claude --format json`

Use fake host executables for deterministic CI. Each fake records argv and returns the same JSON shapes produced by the pinned real CLIs. RED is the missing `plugin` command surface.

- [ ] **Step 3: Add the new command surface**

Introduce:

```text
munin plugin install [--codex|--claude] [--dry-run] [--format text|json]
munin plugin verify [--codex|--claude] [--format text|json]
munin plugin uninstall [--codex|--claude] [--dry-run] [--format text|json]
```

Keep `munin install --check-resolvable` as the compatibility contract. Existing `munin install --codex|--claude` forwards to the new native install path for the 0.6 transition.

- [ ] **Step 4: Materialize the tracked bundle**

Embed the tracked marketplace files in `agent_plugin.rs` and materialize the exact tree under:

```text
<install-home>/.munin/plugin-marketplaces/<version>/
```

Write through a temporary sibling, validate hashes and manifests, then rename into place. A quiet repeat install is idempotent.

- [ ] **Step 5: Delegate to host CLIs**

Codex transaction:

```text
codex plugin marketplace add <bundle> --json
codex plugin add munin-memory@munin-memory --json
codex plugin list --json
```

Claude transaction:

```text
claude plugin validate --strict <bundle>
claude plugin marketplace add <bundle> --scope user
claude plugin install munin-memory@munin-memory --scope user
claude plugin list --json
```

Resolve the host binaries before any write. When `MUNIN_INSTALL_HOME` is set, also set isolated `CODEX_HOME` and `CLAUDE_CONFIG_DIR` for child processes.

- [ ] **Step 6: Write an ownership/transaction receipt**

Store a versioned JSON receipt in the Munin-owned install root. It records:

- schema and plugin version;
- bundle path and file hashes;
- per-host pre-existing marketplace/plugin state;
- commands attempted, exit status, and installed identity;
- rollback actions and final verification.

It never copies host credentials, full configs, or unrelated plugin state.

- [ ] **Step 7: Fail closed and roll back partial installs**

If Claude fails after Codex succeeds, remove only the Codex plugin/marketplace created by this transaction and restore the prior receipt/bundle. A fake-host test must force this branch and prove both profiles return to their exact pre-state.

- [ ] **Step 8: Verify and commit**

```powershell
cargo test --locked --test agent_plugin_contract lifecycle -- --nocapture
cargo test --locked install_ -- --nocapture
git add src/agent_plugin.rs src/lib.rs src/bin/munin.rs tests/agent_plugin_contract.rs
git commit -m "feat: manage Munin through host plugin lifecycles"
```

## Task 3: Remove duplicate profile integration safely

**Files:**

- Modify: `src/bin/munin.rs`
- Modify: `src/agent_plugin.rs`
- Modify: `tests/agent_plugin_contract.rs`

- [ ] **Step 1: Add a failing migration test**

Seed an isolated profile with:

- current direct Munin skills/commands;
- a direct Codex plugin folder;
- Munin entries plus unrelated entries in Claude settings and Codex hooks.

After native install, assert that unrelated entries are byte-for-byte unchanged and old Munin files are moved to a timestamped Munin-owned backup. Do not delete them.

- [ ] **Step 2: Stop writing duplicate surfaces**

Delete the code path that writes:

- `~/.codex/skills/munin*` and `~/.codex/plugins/munin-memory` directly;
- `~/.claude/skills/munin*` and duplicate flat commands directly;
- global Claude/Codex hook entries.

The only active surface after migration is the host-installed plugin cache.

- [ ] **Step 3: Preserve reversible migration**

Move only paths and hook entries positively identified as Munin-owned into the transaction backup. Unknown files or commands are left untouched and reported as conflicts.

- [ ] **Step 4: Verify and commit**

```powershell
cargo test --locked --test agent_plugin_contract migration_preserves_unrelated_state -- --nocapture
git add src/bin/munin.rs src/agent_plugin.rs tests/agent_plugin_contract.rs
git commit -m "fix: remove duplicate profile integration"
```

## Task 4: Replace blocking ingestion hooks with a bounded trigger

**Files:**

- Create: `src/agent_hook.rs`
- Create: `plugins/munin-memory/scripts/munin-session-start.js`
- Create: `plugins/munin-memory/hooks.json`
- Create: `plugins/munin-memory/hooks/hooks.json`
- Modify: `plugins/munin-memory/.claude-plugin/plugin.json`
- Modify: `src/lib.rs`
- Modify: `src/bin/munin.rs`
- Modify: `tests/agent_plugin_contract.rs`

- [ ] **Step 1: Add the failing latency/freshness test**

In an isolated empty session home, run `munin hook session-start --format json`. Assert it returns a versioned receipt with `status`, `generated_at`, `last_refresh_at`, `age_seconds`, and `refresh_queued`. The automated ceiling is two seconds to avoid loaded-runner flakiness; the local release gate is under one second on a quiet machine.

- [ ] **Step 2: Add the bounded hook command**

`munin hook session-start` acquires a short lease, starts a detached `munin hook refresh-worker`, and returns immediately. The worker runs the existing incremental `memory-os ingest --check-now` path, writes success/failure/freshness status, and clears the lease. A stale lease fails visibly and can be recovered; deterministic code never invents freshness.

- [ ] **Step 3: Add host-specific plugin hook configs**

- Codex discovers root `hooks.json` and invokes the plugin script by relative path.
- Claude discovers `hooks/hooks.json` and invokes it through `${CLAUDE_PLUGIN_ROOT}`.
- Both use a two-second host timeout; the script never invokes full ingestion synchronously.

- [ ] **Step 4: Prove the old trap is gone**

Assert no shipped hook contains `spawnSync` for ingestion, `timeout: 120`, or a direct `memory-os ingest` command. Force worker failure and prove the next receipt reports it rather than silently returning healthy.

- [ ] **Step 5: Verify and commit**

```powershell
cargo test --locked --test agent_plugin_contract hook_ -- --nocapture
git add src/agent_hook.rs src/lib.rs src/bin/munin.rs plugins/munin-memory tests/agent_plugin_contract.rs
git commit -m "fix: make agent refresh hooks nonblocking"
```

## Task 5: Prove install, verify, rollback, and uninstall in both real hosts

**Files:**

- Create: `scripts/munin-agent-plugin-check.ps1`
- Modify: `tests/package_contract.rs`
- Modify: `README.md`
- Modify: `INSTALL.md`
- Modify: `CHANGELOG.md`
- Modify: `SESSION_LOG.md`

- [ ] **Step 1: Add the isolated real-host check**

The script creates fresh Codex and Claude profiles, captures pre-state, runs Munin install/verify, queries native host lists/details, runs uninstall, and compares post-state to pre-state. It emits counts and paths; any missing CLI is a fail, not a skip, in the local release gate.

- [ ] **Step 2: Run strict validation and lifecycle proof**

```powershell
claude plugin validate --strict .
./scripts/munin-agent-plugin-check.ps1 -MuninBin <fresh-binary>
```

Done means:

- Codex lists `munin-memory@munin-memory`, enabled, at the expected version.
- Claude lists the same identity, and `claude plugin details` reports the expected skill/hook inventory.
- `munin plugin verify` passes for both.
- forced partial failure rolls back.
- uninstall returns both profiles to their exact pre-state.
- the session-start receipt returns in under one second on the quiet pinned machine.

- [ ] **Step 3: Update public install truth**

Document native marketplace identity, supported CLI versions used for proof, offline/local materialization, verify/uninstall commands, receipt location, and the fact that Phase 2 will add typed MCP tools.

- [ ] **Step 4: Run the fresh Stage 1 gate**

```powershell
cargo fmt --check
cargo build --locked --bin munin
cargo test --locked -- --test-threads 1
cargo run --locked --bin munin -- install --check-resolvable
cargo package --locked --allow-dirty
claude plugin validate --strict .
./scripts/munin-agent-plugin-check.ps1 -MuninBin <fresh-binary>
```

- [ ] **Step 5: Record the checkpoint**

Advance gates 5–8 only for facts the host-level receipt proves. Record exact versions, counts, timings, SHAs, anything not run, and the Phase 2 typed MCP starting point.

## Plan self-review

- The architecture follows both hosts' current marketplace model instead of inventing another installer format.
- One tracked payload is the source of truth for GitHub distribution and binary-embedded local installation.
- Host managers own plugin caches; Munin owns only its materialized marketplace and receipt.
- Rollback is bounded to state created by the current transaction.
- The plugin is intentionally small: memory skills now, typed MCP tools next.
- No actual user profile, release, tag, push, or marketplace submission is part of this phase.
