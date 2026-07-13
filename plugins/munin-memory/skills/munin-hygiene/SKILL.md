---
name: munin-hygiene
description: Audit duplicated CLAUDE.md, AGENTS.md, CONTEXT.md, and related memory guidance. Use when agent memory files are bloated or repeat known rules.
---

# Munin hygiene

Run `munin hygiene --format text`; add `--include-codex` when the suspected duplication lives under Codex memory files.

Default output is read-only. Write mode prunes exact normalized duplicates only and creates `.munin-bak` backups. Cross-agent duplicates are report-only because different hosts may each require the instruction.

Do not semantically rewrite guidance, delete scoped instruction files, or remove cross-agent duplicates without an explicit policy decision.
