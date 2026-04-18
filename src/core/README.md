# Core Infrastructure

## Scope

Munin memory building blocks. Core owns the local memory database, projections,
strategy kernel, resolver, hygiene logic, and shared utilities used by the
`munin` binary.

Core does not own Context shell wrapping, hook lifecycle, or command-output
filtering.

## Tracking Database Schema

```sql
CREATE TABLE commands (
  id INTEGER PRIMARY KEY,
  timestamp TEXT,
  original_cmd TEXT,
  context_cmd TEXT,
  project_path TEXT,
  input_tokens INTEGER,
  output_tokens INTEGER,
  saved_tokens INTEGER,
  savings_pct REAL,
  exec_time_ms INTEGER
);
```

Project-scoped queries use GLOB patterns rather than LIKE so Windows paths do
not get treated as wildcard expressions.

## Shared Utilities

Key functions available to Munin modules:

| Function | Purpose |
|----------|---------|
| `truncate(s, max)` | Truncate string with `...` suffix |
| `strip_ansi(text)` | Remove ANSI escape/color codes |
| `count_tokens(text)` | Estimate tokens: `ceil(chars / 4.0)` |

## Adding New Functionality

Place new memory infrastructure here if it is shared by multiple Munin surfaces
and does not depend on a presentation layer. Keep command rendering in
`src/bin/munin.rs` or `src/analytics`.
