---
namespace: discoveries
agent_id: codex
created: 2026-05-09T09:20:00Z
updated: 2026-05-09T09:20:00Z
timestamp: 2026-05-09T09:20:00Z
status: active
tags: discovery, mejepa, phase-0, phase-1, fsv, source-of-truth
---

# Discovery: Phase 0/1 source-of-truth gaps and fixes

## Findings

- Phase 0's documented user entrypoint was missing: there was no
  `context-graph-mejepa-corpus` `generate`/`verify`/`stats` CLI capable of
  producing a SWE-bench-backed corpus artifact and then re-reading the durable
  store.
- The initial generated corpus index stored absolute paths. A copied/tampered
  corpus could accidentally verify against the original index/store paths,
  hiding tampering. This is a source-of-truth bug, not a test issue.
- Category and mutation-site validation initially happened after the output
  store was opened. Invalid categories and no-site mutations could leave
  partial filesystem state.
- Phase 1's panel scaffold could enforce all 15 slots, but only three concrete
  encoders existed. The rest of the panel could only be filled by callers or
  left zero.

## Fixes

- Added the corpus CLI with fail-closed `generate`, `verify`, and `stats`
  subcommands.
- Corpus indexes now store corpus-relative `store_path` and `patch_path`
  values. `verify` rejects absolute or path-escaping entries and resolves all
  sources of truth under the requested corpus root.
- `generate` plans all requested mutations before creating the output
  directory or RocksDB store. Invalid categories and no-site mutations now
  leave no corpus directory behind.
- `index.complete=false` is written before oracle execution and only flipped
  true after all entries and stats are persisted. `verify`/`stats` reject
  incomplete indexes.
- Phase 1 now has deterministic typed encoders for all 15 panel slots:
  code/tree-sitter Python structure, diff, text, trace, runtime, reasoning,
  scalars, witness, oracle, and static analysis. Unsupported code languages and
  tree-sitter recovered/error parses fail closed.

## FSV evidence

- Phase 0 final happy-path corpus:
  `/tmp/contextgraph-mejepa-corpus-cli-fsv-final`
  - `index.json`: `complete=true`, relative paths, one `known_good` entry.
  - `verify-evidence.json`: `passes=true`, mutation CF count 1, verdict CF
    count 1.
  - SWE-bench `report.json`: `patch_successfully_applied=true`,
    `resolved=true`, FAIL_TO_PASS success `["test_immutable"]`, PASS_TO_PASS
    count 21.
- Phase 0 tamper edge:
  `/tmp/contextgraph-mejepa-corpus-cli-fsv-final-tampered/verify-evidence.json`
  records `passes=false` and a `patch hash mismatch`.
- Phase 0 preflight edges:
  `/tmp/contextgraph-mejepa-corpus-edge-invalid-category2` and
  `/tmp/contextgraph-mejepa-corpus-edge-no-site2` were not created after the
  fail-closed errors.
- Phase 1 final panel:
  `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`
  records `panel_dim=5120`, `panel_filled_mask=32767`, `filled_slot_count=15`,
  and 11 fail-closed edge cases.

