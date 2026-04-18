# Munin Memory

This repository is the source of truth for the `munin` memory CLI.

## Boundaries

- Owns Munin memory read surfaces, Session Brain, Memory OS, strategy, recall,
  proof, install, and hygiene.
- Does not own the `context` shell wrapper, Context hooks, command-output
  filtering, or wrapper install flows.
- Do not add Context wrapper modules (`cmds`, `hooks`, `filters`,
  `command_surfaces`, `discover`) to this repo.
- Keep `Cargo.toml` `publish = false` until a release process is explicitly
  approved.

## Verify

```powershell
cargo fmt
cargo build
cargo test
```

## Hygiene Rules

- Worktrees, runtime state, and generated session folders are not memory hygiene
  evidence.
- Global memory that applies to every project should live in the global memory
  surface, not be repeated in each project.
- Project memory should keep only scoped rules, local exceptions, paths,
  commands, and decisions that are not inherited from an applying ancestor.
