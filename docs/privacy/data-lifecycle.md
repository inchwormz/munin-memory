# Munin Data Lifecycle

Munin is local-first. It reads Claude and Codex session sources in place and
compiles selected, derived state into Munin-owned local storage. It does not send telemetry
or compiled memory to a hosted Munin service. The legacy `telemetry.enabled`
setting does not enable outbound pings; the telemetry implementation is a
deliberate no-op.

## What Munin owns

- The resolved data root and SQLite database (`history.db`).
- The resolved config root (`config.toml`).
- Local artifacts, strategy state, lifecycle/plugin receipts, the materialized
  plugin marketplace, and positively identified migration backups.

By default, Windows stores the database under
`%LOCALAPPDATA%\munin\history.db`. Platform paths and environment overrides are
resolved at runtime. Run `munin privacy inspect --format json` to see the exact
owned roots, database path/size, configured retention, and file/byte counts.

Claude and Codex own their raw session transcripts and installed plugin caches.
Munin reads source transcripts but does not treat them as owned lifecycle data.
Retention and forget do not delete them.

## Sensitivity and telemetry

Compiled state, exports, receipts, and backups can contain prompts, user names,
file paths, business decisions, and secrets. MCP secret-shape redaction is
defense in depth and cannot identify every sensitive value. Apply operating-
system permissions and full-disk encryption to local state.

Munin does not send telemetry, prompts, or memory to a Munin-operated endpoint.
Claude, Codex, source-control remotes, package registries, and operator-selected
sync or backup tools have their own policies and remain separate trust
boundaries.

## Retention

The default compiled-row retention is 90 days. Inspect it with:

```powershell
munin privacy inspect --format text
```

Preview a different window without mutation:

```powershell
munin privacy retention --days 30 --format json
```

Apply it only after reviewing the `found`, `eligible`, and `kept` receipt:

```powershell
munin privacy retention --days 30 --apply --format json
```

Allowed values are 1 through 3650 days. Apply saves
`tracking.history_days`, deletes eligible timestamped compiled rows, and reports
the actual removed count. Automatic cleanup uses the same configured value.
Source transcripts remain under the host application's retention policy.

## Export

Create a new local directory containing Munin-owned data/config files, a
consistent SQLite snapshot, and a versioned SHA-256 manifest:

```powershell
munin privacy export --output C:\secure\munin-export --format json
```

The destination must not already exist or be inside an owned data/config root.
Source transcripts are excluded. The export is not encrypted by Munin; protect
it with an encrypted destination and encrypted transport. The operator owns its
later retention and deletion.

## Project-scoped deletion

Previewing is not meaningful for an exact project delete, so Munin requires the
literal acknowledgement token and reports found/removed rows:

```powershell
munin privacy forget --project "C:\exact\project" --confirm FORGET-MUNIN --format json
```

Matching is the exact stored `project_path` across project-scoped compiled
tables. It is not prefix, case-normalized, canonicalized, or fuzzy matching.

## Full deletion

Delete all resolved Munin-owned data, configuration, and database paths:

```powershell
munin privacy forget --all --confirm FORGET-MUNIN --format json
```

The command refuses dangerous targets such as the current workspace, home
directory, filesystem roots, and their ancestors. It does not delete source
transcripts, host-managed plugin caches, exports stored elsewhere, terminal
logs, source-control history, backups, or copies in agent conversations.

After deletion, uninstall the native plugin separately with
`munin plugin uninstall` if the host integration should also be removed.
