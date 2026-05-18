---
namespace: journal
agent_id: codex
created: 2026-05-12T02:43:34Z
updated: 2026-05-12T02:43:34Z
status: active
tags: success, mejepa, corpus, hardening, fsv, swebench
---

# Success: ME-JEPA Corpus Durable Write Hardening FSV

## What Happened

Hardened the ME-JEPA corpus generator after WSL2 `/mnt/d` ENOENT failures and
stale partial-corpus state.

Key outcomes:

- Removed durable patch-file writes from the rayon patch-generation workers.
- Durable patch files are now written serially only after the oracle returns a
  verdict for the row.
- Resume validates the full physical state: `index.json`, mutation patch files,
  and RocksDB corpus rows must all match.
- Remaining `Command::output()` subprocess call sites in corpus patch loading
  and Docker queries now use a timed subprocess helper with stdout/stderr drain
  threads and SIGTERM to SIGKILL timeout escalation.
- The old production partial corpus now fails closed because it has 1,800 patch
  files but only 1,500 indexed/store rows.

## FSV

Current-binary happy path:

`/mnt/d/contextgraph/corpus/codex-hardening-fsv-current-20260512T025100Z`

Verified:

- real SWE-bench Lite `sympy__sympy-20590` `known_good` generation succeeded;
- `index.json` has one complete entry;
- exactly one patch file exists;
- RocksDB mutation/verdict/provenance CF counts are `1/1/1`;
- `verify` reran the oracle and matched the persisted verdict hash.

Edge checks failed closed without creating output for invalid outside-root,
zero-worker, and zero-max-task inputs. Stale resume failed closed before Docker.

## Related

- Report: `memory/reports/session-2026-05-12--mejepa-corpus-write-hardening-fsv.md`
- Discovery: `memory/discoveries/2026-05-12--mejepa-corpus-durable-write-and-resume-root-cause.md`
