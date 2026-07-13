---
name: munin-brain
description: Inspect Munin Session Brain for the current live session. Use for current-window state, agenda, blockers, or active context.
---

# Munin brain

Run `munin brain --format prompt`.

Check `sourceStatus` first. Treat the answer as current only when it is `live`. For `fallback-latest` or `stale`, name the session id and transcript modified time, then prefer `munin resume --format prompt` for project continuity. Never present fallback state as the user's live current ask.

Done means the answer states whether the source is live, fallback, stale, or absent and offers resume when live state is unavailable.
