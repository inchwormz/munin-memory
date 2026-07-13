---
name: munin-resume
description: Open the compiled Munin startup brief for this project. Use when starting, resuming, or handing off work.
---

# Munin resume

Run `munin resume --format prompt`.

Treat the result as compiled startup truth, not raw transcript search. Answer from the current project, active work, next step, and watchout sections. If the brief is empty or mismatched, run `munin doctor --scope user --format text` before answering.

Done means the answer names the active project or says none was found, gives one useful next step, and includes one watchout when available.
