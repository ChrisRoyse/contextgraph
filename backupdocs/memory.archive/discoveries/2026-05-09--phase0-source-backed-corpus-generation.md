---
namespace: discoveries
created: 2026-05-09T08:47:57Z
updated: 2026-05-09T08:47:57Z
status: active
severity: high
tags: mejepa, phase0, corpus, swebench, source-backed-patches
---

# Phase 0 Source-Backed Corpus Generation

## What Happened
Phase 0 corpus generation was still relying on `patch_mutation::mutate_unified_diff`, which only mutates lines added by the official SWE-bench patch. That is insufficient for the full 300 x 8 corpus: many official fixes do not add a boolean operator, integer literal, assertion, or swappable local variable in the diff hunk, so category planning can fail with `MEJEPA_CORPUS_NO_MUTATION_SITE`.

The robust path is source-backed generation:

- checkout the task repository at manifest `base_commit`;
- apply the official SWE-bench patch for all categories except `WrongFile`;
- run the existing parser-backed source operators over real tracked Python files;
- capture the result with `git diff --binary --no-ext-diff`;
- persist patch bytes and oracle verdicts through the normal corpus CLI and RocksDB CFs.

## Why It Matters
The full Phase 0 batch requires one generated patch per `(task, category)` pair. Mutation planning must not depend on an operator site being present in added diff lines. The source-backed path makes planning depend on actual repository state at `base_commit`, validates manifests before use, and records the mutated source path/site in `GeneratedEntry.mutation_note` so broad candidate searches are auditable.

Manual evidence from this session:

- `/tmp/contextgraph-phase0-one-task-all-categories/index.json` contains all 8 categories for `sympy__sympy-20590`.
- `/tmp/contextgraph-phase0-one-task-all-categories/verify-evidence.json` reports `entry_count=8`, `mutation_cf_count=8`, `verdict_cf_count=8`, `failures=[]`.
- `ldb` scans of `mejepa_mutation_corpus` and `mejepa_oracle_verdicts` each returned 8 rows.
- `wrong_file` left the official fix absent and Docker recorded `test_immutable` as failed.

## Related
- Code: `crates/context-graph-mejepa-corpus/src/source_patch.rs`
- Code: `crates/context-graph-mejepa-corpus/src/source_patch_support.rs`
- Code: `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`
- Report: `memory/reports/session-2026-05-09--phase0-source-backed-corpus.md`
