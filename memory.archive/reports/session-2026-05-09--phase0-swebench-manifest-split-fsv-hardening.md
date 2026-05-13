---
namespace: reports
agent_id: codex
created: 2026-05-09T08:02:00Z
updated: 2026-05-09T08:24:44Z
timestamp: 2026-05-09T08:02:00Z
status: active
tags: report, session, phase0, phase1, swebench, split, fsv, hardening
---

# Session Report: 2026-05-09 — Phase 0 SWE-bench Manifest/Split FSV Hardening

## Task

Review Phase 0/1 recent code and SWE-bench download state, identify root
causes for the remaining gaps, research best-practice direction, fix the
system without workarounds, and perform Full State Verification.

## Outcome

Partial Phase 0/1 hardening complete. Phase 0's official one-task Docker
oracle path is proven, manifest integrity is checked against the official
dataset, the split contract is repaired, and the `mejepa-corpus`
Generate/Verify/Stats CLI now has real-Docker FSV for one SWE-bench Lite
instance x two categories. Phase 0 still needs the full 300-task x
8-category corpus batch. Phase 1 no longer has 11 missing slots: all 15 panel
slots have deterministic typed encoders and all-slot FSV; the deferred Phase 1
work is neural/embedder-backed replacement, multi-language chunk routing,
Pyright/rust-analyzer evidence, runtime trace capture wiring, and the
linear-probe quality gate.

## Changes

- `scripts/swebench_lite_ops/commands.py`
  - official dataset probe now hashes row metadata;
  - preflight verifies checked-in manifests against official rows;
  - failures use explicit `SWEBENCH_TASK_MANIFEST_METADATA_MISMATCH` and
    `SWEBENCH_TEST_PATCH_SHA_COLLISION` codes.
- `crates/context-graph-mejepa-corpus/examples/split_fsv.rs`
  - active assignment file is instance-atomic `[240,30,30]`;
  - repo-atomic assignment file is diagnostic-only;
  - official test-patch SHA leakage guard and boundary cases are recorded.
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs`
  - stopped deleting the shared FSV root;
  - writes active and diagnostic split assignment files consistently.
- `crates/context-graph-mejepa-corpus/src/lib.rs`
  - comments now reflect shipped oracle/store/SWE-bench bridge state.
- `crates/context-graph-mejepa-instruments/src/lib.rs`
  - comments now state all deterministic typed slot encoders are shipped and
    neural replacements can be added behind the same trait.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs`
  - Generate/Verify/Stats CLI hardening: fresh-output requirement,
    relative SoT paths, `index.complete`, exact patch/verdict hashes, CF
    count checks, and strict Docker run-id validation.
- `crates/context-graph-mejepa-corpus/src/patch_mutation.rs`
  - over-engineer diff mutation now preserves added-line indentation and
    WrongFile rejects absolute/path-traversal alternates.
- Live docs updated to reflect the deleted `sitrep/` tree and current Phase
  0/1 status in doc 09/10 and `docs/ruvectorfindings/specs/*`.

## FSV Evidence

- Preflight:
  `/tmp/contextgraph-swebench-lite-fsv/preflight-evidence.json`
  - `manifest_metadata.checked_count=300`
  - `unique_official_test_patch_sha256=300`
  - `rows_sha256=sha256:5798e24d0bcf040e5cf6aaba5685b703f5bceb7eeec153f690670af4c46ec321`
- Docker cache:
  `/tmp/contextgraph-swebench-lite-fsv/image-status-evidence.json`
  - exactly one expected image:
    `swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest`
- Split:
  `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`
  - `instance_atomic.bucket_sizes=[240,30,30]`
  - `repo_atomic.bucket_sizes=[195,77,28]`
  - `test_patch_sha_leakage_check_passed=true`
- SWE-bench bridge:
  `/tmp/contextgraph-mejepa-corpus-fsv/swebench-oracle-evidence.json`
  - gold/custom resolved true for `sympy__sympy-20590`
  - 22 per-test rows
  - RocksDB round-trip verdict matches
- Phase 1:
  `/tmp/contextgraph-mejepa-instruments-fsv/instruments-evidence.json`
  and `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`
- Corpus CLI:
  `/tmp/contextgraph-mejepa-corpus-cli-fsv/index.json`,
  `/tmp/contextgraph-mejepa-corpus-cli-fsv/verify-evidence.json`, and
  `/tmp/contextgraph-mejepa-corpus-cli-fsv/stats.json`
  - `complete=true`
  - `entries=2`
  - categories `known_good`, `over_engineer`
  - split counts `[240,30,30]`
  - `mutation_cf_count=2`
  - `verdict_cf_count=2`
  - both SWE-bench aggregate reports resolved.

## Next

Broaden the corpus generator from diff-added-line mutations to source-aware
patched-repository mutations so every category can be generated across all
300 SWE-bench Lite tasks without no-site failures. Then run a 5-task x
8-category batch, verify persisted `OracleVerdict` rows, and run the full
300-task operational batch.
