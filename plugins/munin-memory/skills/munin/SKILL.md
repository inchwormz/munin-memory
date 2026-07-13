---
name: munin
description: Use Munin when the user asks about active work, current session state, project memory, repeated mistakes, next steps, recall, proof, or health and the exact narrow Munin surface is unclear.
---

# Munin

## When to use

Use this resolver wrapper when a request is memory-shaped but spans multiple surfaces or the right narrow skill is not obvious.

## Flow

0. If the user invoked Munin without a substantive ask, stop and ask what they want checked. Do not run a status command for a bare invocation.
1. Run `munin resolve --format text "<user ask>"`.
2. Run the returned command.
3. Follow the matching narrow skill's trust, fallback, and done rules.

Routes are `resume`, `brain`, `friction`, `nudge`, `prove`, `recall`, `hygiene`, and `doctor`. Live-session continuity routes to `brain` only when Session Brain is live; fallback or stale continuity routes to `resume`.

## Trust

- Trust the route unless the user clearly asked for a different narrow surface.
- If recall returns zero topic matches, do not silently fall back to overview.
- If the route is brain, check freshness before saying anything is current.

## Done

The user has a compiled answer from the chosen Munin surface, not a raw transcript dump.
