# Munin Threat Model

## Scope and security goals

This model covers the local `munin` process, compiled Memory OS state, the
native Claude/Codex plugin bundle, the local stdio MCP server, privacy lifecycle
commands, and release artifacts. The goals are to keep private memory local,
prevent unintended mutation or deletion, return bounded and attributed agent
context, and make executable/package identity auditable.

Munin does not protect a user whose operating-system account or agent host is
already compromised. It does not make untrusted session text safe to execute.

## Assets

- Raw Claude and Codex session transcripts, owned by their host applications.
- Compiled SQLite rows, artifacts, strategy state, local configuration, hook
  status, lifecycle receipts, and plugin migration backups owned by Munin.
- Export directories, which can contain prompts, paths, decisions, and derived
  memory even when obvious secret shapes are redacted at an MCP boundary.
- Plugin, marketplace, MCP, binary, and package identity.

## Actors

- The local user, who may inspect, export, configure retention, or explicitly
  delete owned state.
- Claude Code and Codex, which load a local plugin and invoke local MCP tools.
- Repository contributors and release operators.
- Malicious or accidental local processes, session content, dependencies, and
  substituted binaries or plugin files.

## Entry points

- CLI arguments, environment-variable path overrides, config files, and import
  source files.
- Native plugin manifests, hooks, marketplaces, and host-managed caches.
- Newline-delimited JSON-RPC on MCP standard input; JSON-RPC output on standard
  output and diagnostics on standard error.
- SQLite files, export destinations, install bundles, and release packages.

## Trust boundaries

1. **Host transcript to compiler:** transcript text is untrusted data. It may
   contain prompt injection, secret-shaped values, malformed JSON, or paths.
2. **Compiled state to agent:** MCP adapters are read-only and return provenance,
   freshness, sensitivity, warnings, redaction, and coverage receipts. Those
   labels do not guarantee the underlying prose is true or safe to execute.
3. **Munin to host manager:** Codex and Claude own installed plugin caches;
   Munin owns only its materialized marketplace, receipts, and positively
   identified migration backups.
4. **Owned state to export:** export crosses from protected local state into an
   operator-chosen, unencrypted directory.
5. **Operator to destructive lifecycle:** retention and forget can remove
   compiled state. Preview is the default for retention; forget requires an
   exact token and fail-closed root checks.

## Plugin and MCP risks

- A replaced `munin` executable or modified plugin declaration can run with the
  user's permissions. Verify installation and provenance before trusting it.
- A hostile local MCP client can query all six read-only tools. The stdio server
  has no network listener or per-client authentication because the spawning
  local host is the security boundary.
- Tool output can contain sensitive derived text. Secret-shape redaction is
  defense in depth, not a data-loss-prevention guarantee.
- Imported prompt injection can appear in memory. Agents must treat retrieved
  memory as evidence, not executable instruction, and prefer the current user
  request over recalled text.
- Plugin hooks may trigger bounded local ingestion. They do not make network
  telemetry calls, and Doctor does not ingest or repair state.

## Lifecycle and path risks

- `privacy export` refuses an existing destination and any destination inside
  Munin data/config roots, then creates a consistent SQLite snapshot.
- `privacy retention` is report-only unless `--apply` is present and reports
  found, eligible, removed, and kept counts.
- `privacy forget --project` uses an exact `project_path`; it does not fuzzy
  match. `privacy forget --all` deletes only resolved owned roots and rejects
  the workspace, home, filesystem roots, and their ancestors.
- Host source transcripts are never an owned deletion target.

## Supply-chain risks and controls

Locked dependencies, RustSec advisory checks, warnings-denied linting, the
declared Rust 1.78 MSRV, old-schema migration tests, package-content tests, and
one provenance receipt form the release boundary. A local pass does not prove a
GitHub workflow ran; releases must state which environment was actually tested.

## Residual risk

Operating-system access controls, disk encryption, encrypted transport/storage
for exports, host integrity, backup deletion, and secret rotation are operator
responsibilities. Munin cannot revoke memory already copied into an agent
conversation, terminal log, backup, or external system.
