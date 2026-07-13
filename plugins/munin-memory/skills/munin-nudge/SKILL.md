---
name: munin-nudge
description: Show Munin strategy-backed next-step nudges. Use when the user asks what to do next or wants proactive guidance.
---

# Munin nudge

Run `munin nudge --format text`.

Return the top one to three moves, why each matters now, confidence, and evidence. If metrics are missing, show the exact setup action instead of passing through a long generic instrumentation dump. Do not recommend a move without evidence.
