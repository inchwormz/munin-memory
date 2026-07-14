# Munin

Local memory for Claude Code and Codex.

Munin ingests Claude and Codex sessions and compiles them into local Memory OS
surfaces for live continuity, strategy, and recall.

It exposes that memory back to your agent through CLI commands and one native
Claude/Codex plugin. It is designed for developers who want agents to
remember active work, repeated mistakes, strategic priorities, and unfinished
tasks without sending that memory to a hosted service.

Published crate: `0.5.14`.
Latest GitHub release: `v0.5.13`.
The next release candidate is unreleased. Crate publishing is disabled in this
checkout until that release is explicitly approved.

[![CI](https://github.com/inchwormz/munin-memory/actions/workflows/ci.yml/badge.svg)](https://github.com/inchwormz/munin-memory/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/munin-memory.svg)](https://crates.io/crates/munin-memory)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/inchwormz/munin-memory?style=social)](https://github.com/inchwormz/munin-memory)

Munin is open source under the Apache 2.0 license.

## Runtime Ownership

Munin is the product owner for runtime context. End users should not need the
`context` tool installed in order to use Munin's Session Brain, startup brief,
strategy surfaces, or Claude/Codex integration assets.

Architecture docs:

- [Runtime Context Ownership](docs/architecture/runtime-context-ownership.md)
- [Context Strangler Migration](docs/architecture/context-strangler-migration.md)

## What Munin Does

<img width="996" height="795" alt="image" src="https://github.com/user-attachments/assets/5e2d0faa-ee36-40e6-8c08-e31c59769f6f" />


Munin has five local layers:

1. **Session ingestion** reads first-class Claude and Codex sessions: prompts,
assistant turns, shell commands, outcomes, corrections, working directories, and
timestamps. Claude subagent internals are excluded by design.
2. **Memory compilation** converts those sessions into a local Memory OS:
evidence-backed facts, active projects, strategic plans, continuity commitments,
open loops, repeated friction, command outcomes, strategy context, and next steps.
3. **Strategy and proactivity** turn memory into concrete tasks. `munin nudge`
now combines strategy red/yellow items with continuity work from previous
completed sessions, active projects, and verified incomplete tasks. The morning
proactivity can spawn either a Claude or Codex session based on the configured
provider or an explicit `--provider claude|codex` override. It infers what tasks
should be executed based on goal/project deltas. Where you are now,
where you want to be, what closes the delta? It will enable Claude/Codex to be proactive to help
you reach your goals.
4. **Agent access** installs one native plugin through each host's own plugin
manager so agents can query compiled memory instead of trawling raw transcripts.
5. **Session Brain** compiles the current live session into a short working
memory layer: who the user is, what they care about, the current ask, active
strategy, blockers, and next actions. It is for long context windows where the
agent needs current-session oversight, not historical search.

No hosted service is required. The compiled state stays on the machine running
Munin.

## Install For A New User

Munin distribution is binary-first. Install the `munin` binary, then let Munin
materialize its embedded marketplace and delegate installation to the native
Codex and Claude plugin managers. No hosted marketplace is required.

### Option A: Install From crates.io

```powershell
cargo install munin-memory --force
```

### Option B: Install From GitHub Source

```powershell
git clone https://github.com/inchwormz/munin-memory.git
cd munin-memory
cargo install --path . --force
```

Verify it:

```powershell
munin --version
munin install --check-resolvable
```

## Install The Native Agent Plugin

The plugin identity is `munin-memory@munin-memory`. Install it for one host or
both:

### Claude Code

```powershell
munin plugin install --claude --format text
munin plugin verify --claude --format text
```

### Codex

```powershell
munin plugin install --codex --format text
munin plugin verify --codex --format text
```

### Both Agents

```powershell
munin plugin install --format text
munin plugin verify --format text
```

The compatibility commands `munin install --claude`, `--codex`, or no host flag
forward to the same native lifecycle. The old `--force` and `--keep-legacy`
flags no longer control host caches.

Munin owns only:

- the versioned local marketplace under
  `~/.munin/plugin-marketplaces/<version>/`;
- schema-versioned lifecycle receipts under `~/.munin/`;
- timestamped migration backups under `~/.munin/migration-backups/`.

Codex and Claude own their plugin caches. A first native install moves only
positively identified legacy Munin files into the migration backup and removes
only positively identified Munin hook entries. Unknown files are left in place
and reported as conflicts.

The plugin supplies nine skills: `munin`, `munin-brain`, `munin-doctor`,
`munin-friction`, `munin-hygiene`, `munin-nudge`, `munin-prove`,
`munin-recall`, and `munin-resume`. Its session-start hook returns a freshness
receipt immediately and queues incremental ingestion in a detached worker.

## First Run

Run ingestion once after installing agent surfaces:

```powershell
munin memory-os ingest --format text
```

Direct CLI ingestion without `--force` is incremental and says `current` when
nothing new needs import. The native plugin hook also queues this incremental
check when compiled memory is stale.

Typical output:

```text
Memory OS Ingest
----------------
Elapsed: 46.70s
Mode: incremental
Status: imported
Sessions processed: 2910
Shell executions ingested: 35065
Corrections ingested: 868
```

Then check health:

```powershell
munin doctor --scope user --format text
```

From Claude or Codex, ask the agent to use the installed `munin-doctor` skill.

## Daily Usage

Use these surfaces at the start of work or when an agent gets lost:

```powershell
munin resume --format prompt
munin brain --format prompt
munin nudge --format text
munin recall --format text "<topic>"
munin memory-os brief --scope user --format text
munin memory-os overview --scope user --format text
munin memory-os friction --scope user --format text
```

The native plugin exposes these matching skill names:

```text
munin
munin-brain
munin-nudge
munin-recall
munin-friction
```

## What `munin nudge` Does

`munin nudge` returns concrete work, not just diagnosis. It combines:

- strategy items that are red/yellow or missing metrics
- logical next tasks from recent completed sessions and project activity
- continuity commitments from earlier work
- verified incomplete tasks and open obligations

Example shape:

```text
Strategy Nudge
--------------
Scope: example-business
Suggested task queue:
1. Address red-state `Outreach reply rate`
2. Resume incomplete work: Finish recording-ready Munin onboarding
3. Continue munin-memory: Update docs and verify install surfaces
Execution:
- Start with this intervention: Address red-state `Outreach reply rate`.
- Work it until implemented, verified, or blocked by a concrete recorded blocker.
```

## Strategy Planning

Use the CLI to create or update a One-Page Strategic Plan, bootstrap a strategy
kernel, or triage tasks against goals. After strategy setup, `munin nudge` uses
the strategy kernel and metrics snapshot to recommend the next task.

Useful CLI surfaces:

```powershell
munin strategy setup --scope <scope> --import <strategic-plan.context.json>
munin strategy status --scope <scope> --format text
munin strategy recommend --scope <scope> --format text
munin metrics set <metric_key> <value> --scope <scope>
```

## Install And Debug Commands

These commands are useful when testing a new install:

```powershell
munin install --check-resolvable
munin plugin install --dry-run --format json
munin plugin install --format json
munin plugin verify --format json
munin hook session-start --format json
munin plugin uninstall --format json
```

For an isolated proof against the installed Codex and Claude CLIs:

```powershell
pwsh -NoProfile -File .\scripts\munin-agent-plugin-check.ps1 -MuninBin <fresh-munin-binary>
```

The gate proves install, native list/details, Munin verify, forced partial
rollback, bounded refresh, uninstall, and semantic equality between native
pre-state and post-state.

## Non-Technical Release Check

For a plain PASS/WARN/FAIL readout before trusting a local build, run:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\munin-release-check.ps1
```

The check verifies the binary, install contract from the repo, install contract
from a temporary folder, memory health, promotion proof, and resolver routing.
`WARN` and `FAIL` both return a non-zero process status. Fix every named item
before trusting the release.

## Privacy And Storage

Munin reads local agent history and writes a local SQLite-backed Memory OS under
the platform local data directory. On Windows this is normally:

```text
%LOCALAPPDATA%\context\history.db
```

For testing or demos, you can isolate paths:

```powershell
$env:MUNIN_INSTALL_HOME = "C:\tmp\munin-home"
$env:MUNIN_SESSION_HOME = "C:\tmp\munin-home"
$env:MUNIN_DATA_DIR = "C:\tmp\munin-data"
```

## Current Distribution Status

The current supported install path is:

1. Install the binary from crates.io or GitHub source.
2. Run `munin plugin install --claude`, `--codex`, or no host flag for both.
3. Run `munin plugin verify`.
4. Restart the agent and use the installed Munin skills.

The repository and binary both contain the same dual-host marketplace. The
local gate currently pins Codex CLI `0.144.3` and Claude Code `2.1.207`.
Typed MCP tools are intentionally deferred to the next implementation phase;
the current plugin exposes skills and the bounded freshness hook.

## Development

Repository layout:

- `src/bin/munin.rs` - CLI entrypoint
- `src/agent_plugin.rs` - native marketplace lifecycle, rollback, and migration
- `src/agent_hook.rs` - bounded freshness hook and detached refresh worker
- `plugins/munin-memory/` - shared Codex/Claude plugin payload
- `src/analytics/` - Memory OS read surfaces and session ingestion
- `src/core/` - durable tracking, Memory OS, strategy, proactivity, and resolver
- `src/session_brain/` - current-session summary
- `src/session_intelligence/` - local Claude/Codex session readers
- `src/assets/skills/` - bundled installable prose skills
- `tests/` - CLI, resolver, package, and fixture tests

`munin proactivity schedule-install --provider codex` installs the morning
runner for Codex. `munin proactivity schedule-install --provider claude`
installs it for Claude. On Windows it uses Task Scheduler, on macOS it installs
a LaunchAgent, and on Linux it installs a systemd user timer. New installs
default to automatic spawning at the scheduled morning run.

## Notes

Validation:

```powershell
cargo fmt
cargo test
cargo build --bin munin
munin install --check-resolvable
```

Package note: the crate package is `munin-memory`; the installed command is
`munin`.
