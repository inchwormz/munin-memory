---
name: munin-friction
description: Show repeated Munin friction and correction patterns. Use when the user asks what keeps going wrong or what agents keep repeating.
---

# Munin friction

Run `munin friction --last 30d --format text`.

Prioritize active and codified friction. Treat monitoring items as high-signal but not yet repeated enough for codification. Keep fixed and retired items in the background unless the user asks for history. Include the permanent-fix pointer for every surfaced issue.

If the report is empty, say there is no active friction in the requested window. Do not invent a pattern or present retired work as urgent.
