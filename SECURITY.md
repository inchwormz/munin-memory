# Security Policy

## Supported code

Security fixes are made on the current `main` branch and the newest published
release. Older releases may not receive a backport. Check `CHANGELOG.md` and the
GitHub Releases page before reporting an issue that may already be fixed.

## Report a vulnerability

Use GitHub's private security-advisory flow for this repository. Do not open a
public issue until the report has been triaged and a coordinated disclosure is
agreed.

Include the affected Munin version, operating system, host and host version
(Claude Code or Codex), the smallest reproducible steps, expected and actual
behavior, and the security impact. Do not include real transcripts or secrets.
Use a synthetic isolated profile and redact user names, paths, tokens, prompts,
and exported memory.

We will acknowledge the report, assess severity and affected versions, agree on
a disclosure plan, and publish remediation guidance when a fix is available.
Response times are targets, not guarantees: acknowledgement within three
business days and an initial severity assessment within seven.

## Security boundaries

Munin is a local tool, not a sandbox or secret vault. It reads local Claude and
Codex session sources, compiles derived state into local files and SQLite, and
exposes bounded read-only memory tools to the local agent hosts. Anyone who can
run code as the user can generally read or alter those files and can replace the
`munin` binary or plugin declarations.

Exports are not encrypted. Treat an export, plugin migration backup, lifecycle
receipt, and the compiled database as private data. See
`docs/security/threat-model.md`, `docs/security/incident-response.md`, and
`docs/privacy/data-lifecycle.md` for exact boundaries and recovery steps.
