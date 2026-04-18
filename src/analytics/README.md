# Analytics

## Scope

Read-only Munin memory surfaces over local tracking and session data.

Owns:

- Memory OS command renderers
- claim and trust summaries
- session backfill
- session impact reports used by Memory OS

Does not own command wrapping, shell output filtering, hooks, or Context runtime
setup. Those stay outside this project.

## Adding New Functionality

Add a new `*_cmd.rs` module when Munin needs a read-only memory surface. Query
`core/tracking` or `core/memory_os`, register the CLI in `src/bin/munin.rs`, and
add focused tests with sample memory/session data.
