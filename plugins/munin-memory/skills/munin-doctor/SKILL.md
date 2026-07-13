---
name: munin-doctor
description: Diagnose Munin Memory OS health. Use when the user asks whether Munin is healthy, stale, empty, or release-ready.
---

# Munin doctor

Run `munin doctor --scope user --format text`. For release readiness, add `--release --repo-root .` and the explicit site root when one exists.

Fast Doctor proves substrate health, not the quality of every answer. Report the process status, corpus state, source health, and recommended permanent fix. Do not claim release readiness from a fast health pass.

Done means substrate health and release-quality proof are clearly separated.
