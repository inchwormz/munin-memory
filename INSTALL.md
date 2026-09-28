# Local Install

Use this repo as the Munin CLI source of truth.

Repository package version: `0.6.0-rc.2` (local candidate; not released).
Latest GitHub release: `v0.5.13`. This checkout includes unreleased work, and
crate publishing remains disabled until the next release is explicitly approved.

## Build

```powershell
cargo build
cargo test
```

## Install

```powershell
cargo install --path . --force
```

## Verify

```powershell
munin --version
munin resolve "what keeps going wrong?"
munin hygiene --format text
munin install --check-resolvable
```

## Install For Codex And Claude

Munin ships one embedded marketplace with plugin identity
`munin-memory@munin-memory`.

```powershell
# Both hosts
munin plugin install
munin plugin verify

# Or one host
munin plugin install --codex
munin plugin install --claude
```

The install is local/offline after the binary is present. Munin materializes
the exact tracked bundle under `~/.munin/plugin-marketplaces/0.6.0-rc.2/`; Codex and
Claude then own their normal plugin caches. Lifecycle receipts are written
under `~/.munin/`.

To remove the native plugin and marketplace:

```powershell
munin plugin uninstall
```

Legacy direct Munin skills, commands, and global hook entries are migrated only
when they can be positively identified. Files are moved, not deleted, into a
timestamped `~/.munin/migration-backups/` directory. Unknown Munin-named files
remain in place and are reported as conflicts.

The current local compatibility proof uses Codex CLI `0.144.3` and Claude Code
`2.1.207`. Six read-only MCP tools (`search`, `session_summary`,
`project_state`, `friction`, `strategy_status`, and `health`) ship beside the
nine memory skills and bounded session-start refresh hook. Agents receive a
strict `munin.mcp.result.v1` response with provenance, freshness, sensitivity,
warnings, and coverage receipts.

For a clean-profile real-host proof:

```powershell
pwsh -NoProfile -File .\scripts\munin-agent-plugin-check.ps1 -MuninBin <fresh-munin-binary>
pwsh -NoProfile -File .\scripts\munin-mcp-host-check.ps1 -MuninBin <fresh-munin-binary>
```

The MCP host check proves its own judge first with a broken runtime, then
requires Claude to connect and Codex to list all six tools and call `health`.

## Inspect Or Remove Local Data

The default Windows database is `%LOCALAPPDATA%\munin\history.db`. Resolve the
actual data/config/database paths before backup or deletion:

```powershell
munin privacy inspect --format json
munin privacy export --output C:\secure\munin-export --format json
munin privacy retention --days 30 --format json
```

Retention is preview-only unless `--apply` is supplied. Project or full deletion
requires `--confirm FORGET-MUNIN`; source Claude/Codex transcripts and exports
outside Munin-owned roots are never deletion targets. Exports are not encrypted.
Read `docs/privacy/data-lifecycle.md` before moving or deleting private state.

End users should treat `munin` as the only supported runtime-context CLI.
Context may still exist as an internal local developer tool during migration,
but it is not part of the supported Munin install path.

See also:

- [Runtime Context Ownership](docs/architecture/runtime-context-ownership.md)
- [Context Strangler Migration](docs/architecture/context-strangler-migration.md)
