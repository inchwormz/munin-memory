# Munin Security Incident Response

This runbook covers suspected disclosure, tampering, unsafe deletion, malicious
session ingestion, compromised plugin/MCP identity, dependency advisories, and
release provenance failures.

## Severity

- **Critical:** confirmed remote code execution, release-signing or distribution
  compromise, widespread secret disclosure, or destructive behavior outside
  resolved Munin-owned paths.
- **High:** unauthorized local memory disclosure across a meaningful trust
  boundary, executable/plugin substitution, or reliable deletion of unrelated
  user data.
- **Medium:** bounded private-data exposure requiring local access, integrity
  loss limited to Munin-owned state, or a dependency advisory on a reachable
  path without known exploitation.
- **Low:** hardening gap with no demonstrated confidentiality, integrity, or
  availability impact.

## Triage and evidence

Record the Munin version, commit/package/binary hashes, operating system, host
versions, exact command, timestamps, resolved data/config/database paths, and
the smallest synthetic reproducer. Preserve relevant receipts and logs without
copying real transcripts or secrets into an issue. Label claims confirmed,
inferred, or assumed.

## Containment

1. Stop the affected Munin process and disable or uninstall its native plugin
   through the host manager if executable or plugin identity is suspect.
2. Disconnect or restrict external sync for any exposed export directory.
3. Preserve a read-only copy of relevant Munin-owned state only when doing so is
   safe and authorized. Do not run forget or retention before evidence is saved.
4. Rotate credentials whose values may have appeared in transcripts, compiled
   state, exports, logs, or agent conversations.
5. For a distribution incident, stop publishing and mark affected artifacts as
   untrusted. Do not rewrite tags or releases without explicit approval.

## Eradication

Identify the mechanism and affected versions. Remove or replace the compromised
binary, plugin, declaration, dependency, or imported source. Patch the earliest
pipeline boundary that lost ownership, provenance, or validation. Add a test
that reproduces the failure and proves the judge rejects a deliberately broken
fixture before accepting the fix.

Use `munin privacy forget` only after the scope is understood and evidence needs
are satisfied. Deleting Munin-owned state does not delete Claude/Codex source
transcripts, host caches, terminal logs, backups, or copies already sent
elsewhere.

## Recovery

1. Install a hash-verified fixed build in an isolated profile.
2. Run format, clippy, MSRV, tests, migrations, advisories, package, provenance,
   native plugin rollback, MCP retrieval, and privacy lifecycle gates.
3. Reinstall through each host's native plugin manager and verify the connected
   six-tool MCP surface before restoring normal use.
4. Re-ingest only trusted source sessions. Confirm Doctor is fast and read-only.
5. Monitor new lifecycle and host receipts for recurrence.

## Disclosure and review

Coordinate disclosure through the private process in `SECURITY.md`. Publish the
affected versions, impact, remediation, rotation/deletion guidance, and which
claims remain uncertain. After resolution, record root cause, trigger, missed
gate, worst-case impact, and the permanent control added.
