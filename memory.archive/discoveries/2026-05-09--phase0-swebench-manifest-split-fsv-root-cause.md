---
namespace: discoveries
agent_id: codex
created: 2026-05-09T08:02:00Z
updated: 2026-05-09T08:24:44Z
timestamp: 2026-05-09T08:02:00Z
status: active
tags: discovery, phase0, swebench, split, fsv, root-cause
---

# Discovery: Phase 0 SWE-bench Manifest and Split FSV Root Causes

## Root Causes

1. The Phase 0 "Docker evaluator missing" gap was stale. The official
   SWE-bench Lite harness, one real Docker image, and one-task evaluator path
   are present and working. The remaining Phase 0 gap is breadth: no full
   300 tasks x 8 mutation categories generation batch has been run.

2. The original split status conflated two incompatible invariants:
   exact 80/10/10 sample counts and whole-repo atomicity. On the current
   SWE-bench Lite corpus, repo-atomic splitting is mathematically bounded by
   12 skewed repos, including `django/django=114` and `sympy/sympy=77`, so
   the repo-atomic diagnostic readback is `[195,77,28]`.

3. The preflight was checking official dataset ids but not all checked-in
   manifest metadata against the official dataset. That left
   `official_test_patch_sha256` as a weak trust boundary.

4. `examples/mejepa_corpus_fsv.rs` still deleted the whole shared
   `/tmp/contextgraph-mejepa-corpus-fsv` root and wrote repo-atomic
   assignments to the active `split-assignments.json`. This could erase
   sibling FSV artifacts and silently stale the active split evidence.

5. The first `mejepa-corpus` batch CLI version wrote partial `index.json`
   state during generation and verified only patch hashes + per-test counts.
   A failed generation could leave a corpus directory that looked structured
   but was not complete. Diff-added-line mutation also cannot generate every
   category for every SWE-bench task; `sympy__sympy-20590` has no boolean
   added line, so `subtle_flip` correctly fails with no site.

## Durable Fix

- `scripts/swebench_lite_ops.py preflight` now compares all 300 checked-in
  manifests against `princeton-nlp/SWE-bench_Lite:test` for repo, base commit,
  problem statement hash, oracle patch hash, official test patch hash,
  FAIL_TO_PASS list, and PASS_TO_PASS count.
- Active split evidence is instance-atomic `[240,30,30]` with
  `official_test_patch_sha256` uniqueness and cross-bucket collision checks.
- Repo-atomic evidence is diagnostic-only at
  `split-repo-atomic-assignments.json`, with `[195,77,28]` and
  `cross_cut_weights=[0,0,0]`.
- `mejepa_corpus_fsv` no longer deletes the shared evidence root; it clears
  only its generated source/pyc subdirectories.
- `mejepa-corpus generate` requires a fresh output directory, persists
  relative paths, marks `index.complete=false` until all requested entries are
  written, and only marks complete after stats write succeeds.
- `mejepa-corpus verify` rejects incomplete/empty corpora and compares exact
  patch SHA, exact serialized `OracleVerdict` SHA, and RocksDB CF counts.
- `patch_mutation::OverEngineer` now preserves added-line indentation, and
  WrongFile rejects absolute/path-traversal alternates.

## Source-of-Truth Evidence

- `/tmp/contextgraph-swebench-lite-fsv/preflight-evidence.json`
- `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/oracle-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`
- `/tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json`
- `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`
- `/tmp/contextgraph-mejepa-corpus-cli-fsv/{index.json,verify-evidence.json,stats.json}`
