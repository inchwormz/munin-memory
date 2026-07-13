---
name: munin-recall
description: Use Munin compiled memory for topic recall. Use when the user asks to remember prior work, decisions, preferences, or project context.
---

# Munin recall

Run `munin recall --format text "<query>"`.

Trust topic-ranked compiled matches before raw archive search. Return three to eight matched facts with evidence pointers when available. If there are zero matches, say so explicitly; do not dump overview or raw transcripts as a substitute.
