# Phase 8 — Handoff

## What's true after Phase 8

- `CLAUDE.md` ends with the ccreality role addendum.
- Repo-root `CLAUDE.md` is no longer ignored, while nested `CLAUDE.md` and `CLAUDE.local.md` files remain ignored.
- `config/ccreality_governed_paths.json` allows root `CLAUDE.md` and `.gitignore`, so future governed harness edits can maintain the project-memory contract without widening private-file access.
- Every new Claude Code session in this project loads the addendum as project memory at startup through CLAUDE.md discovery.
- The role contract, the reality-shift principle, the always-do/never-do rules, the cost policy, and the artifact layout are all available to the agent without it having to read separate docs.

## Manual sanity check

If `claude` CLI is installed:
```bash
cd /home/cabdru/contextgraph
claude --print "Quote the first three lines of the 'ccreality' section in your project memory verbatim. Return only those three lines."
```

Expected: the three lines starting from the addendum's heading.

If Claude Code says it doesn't see the addendum, either:
- `--bare` mode was used (which skips CLAUDE.md auto-discovery).
- The addendum is malformed and Claude Code skipped it.
- The append went to a different file (check working directory).

## Verification record

```bash
echo "phase-08: PASS, $(date -u +%FT%TZ)" >> /tmp/ccreality-phase-progress.log
```

Actual verification records from this implementation:

- Canonical verification: `phase-08: PASS, 2026-05-05T19:05:34Z`.
- Manual FSV: `/tmp/ccreality-phase8-fsv-1778007923.log`.

## Next phase

→ [phase-09-smoke/](../phase-09-smoke/README.md)

Phase 9 builds the smoke test infrastructure that exercises everything from Phases 1–8 end-to-end.
