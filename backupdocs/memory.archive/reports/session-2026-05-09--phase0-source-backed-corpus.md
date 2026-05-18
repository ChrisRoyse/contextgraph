---
namespace: reports
created: 2026-05-09T08:47:57Z
updated: 2026-05-09T08:47:57Z
status: active
tags: report, session, mejepa, phase0, corpus, swebench
---

# Session Report: 2026-05-09 — Phase 0 Source-Backed Corpus

## Task: Complete Phase 0 corpus generation scope from the specs.
## Outcome: complete for implementation path; full 2,400-entry operational run not executed because it is a long sequential Docker batch.

## Summary
Read the requested Phase 0 spec docs and confirmed the active gap: the CLI could generate small examples, but mutation planning was diff-only. That failed the full corpus contract because added official patch lines do not reliably contain all eight operator sites.

Implemented source-backed patch generation. The CLI now reads `base_commit` from manifests, checks out the real task repository, applies the official fix where appropriate, mutates real tracked source, emits `git diff --binary`, records mutation provenance, runs the SWE-bench Docker oracle, and writes/read-verifies index plus RocksDB state.

## Files Created or Modified in the Repo
- `crates/context-graph-mejepa-corpus/src/source_patch.rs` — new source-backed mutation planner.
- `crates/context-graph-mejepa-corpus/src/source_patch_support.rs` — git/cache/worktree/path validation support.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs` — switched `generate` from diff-only mutation to source-backed generation; added repo cache/work root args, `base_commit`, mutation provenance, and full-Phase-0 stats.
- `crates/context-graph-mejepa-corpus/src/lib.rs` — exported source-backed module and updated stale scope comment.

## Files Written or Updated in ./memory/
- `memory/discoveries/2026-05-09--phase0-source-backed-corpus-generation.md` — root cause and evidence.
- `memory/reports/session-2026-05-09--phase0-source-backed-corpus.md` — this report.

## Decisions Made
- Use source-backed generation as the Phase 0 batch path; keep diff-only mutation as a lower-level tested helper but stop using it in the corpus CLI. Rationale: the full 300 x 8 contract cannot depend on mutation sites being present in added diff hunks.

## Discoveries
- `sympy__sympy-20590` demonstrates the root cause: `subtle_flip`, `off_by_one`, and `delete_test_call` can require sites outside the official changed file. The new provenance field makes these broad searches explicit.

## Failures
- Running `stats` in parallel with `verify` can hit the RocksDB lock. Lesson: open the same RocksDB corpus through one process at a time for manual evidence reads.
- The first invalid-`base_commit` edge fixture used a one-task task directory, so split validation failed before SHA validation. Reran with all 300 manifests and only the selected task corrupted; it failed at `MEJEPA_CORPUS_INVALID_INPUT` as intended.

## Successes
- `cargo test -p context-graph-mejepa-corpus` passed: 87 library tests plus bin/doc tests.
- `cargo run -p context-graph-mejepa-corpus --example mejepa_corpus_fsv` passed and wrote `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`.
- `cargo run -p context-graph-mejepa-corpus --example split_fsv` passed and wrote `/tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json`.
- `cargo run -p context-graph-mejepa-corpus --example oracle_store_fsv` passed and wrote `/tmp/contextgraph-mejepa-corpus-fsv/oracle-store/oracle-store-evidence.json`.
- One real SWE-bench task x all 8 categories generated and verified at `/tmp/contextgraph-phase0-one-task-all-categories`.

## Open Blockers
- None for the implementation path. The full 2,400-entry corpus still needs to be launched as an operational batch and will take long wall-clock time because every entry runs Docker.

## Tests / Build
- Tests added: none; implementation reused existing source operators and added manual FSV evidence.
- Tests passing: yes.
- Build status: passing.

## Resume Here Next Session
- Start by reading: `memory/discoveries/2026-05-09--phase0-source-backed-corpus-generation.md`.
- Watch out for: do not run two read commands against the same RocksDB store in parallel.
- Priority: launch the full batch only when the machine can spend the Docker wall-clock budget:
  `cargo run --release -p context-graph-mejepa-corpus --bin mejepa-corpus -- generate --output <fresh-dir> --repo-cache-dir /tmp/contextgraph-mejepa-corpus-repo-cache --source-work-root /tmp/contextgraph-mejepa-corpus-source-work`
- Open question: whether non-Python per-language Phase 0 operators are still required before Phase 1a, since SWE-bench Lite manifests are Python repositories but the constitution names 11 supported languages.

## Confidence: high
The source-backed implementation compiles, passes package tests, completes real SWE-bench Docker runs, and the generated corpus state was independently read back through index files, RocksDB CF counts, `ldb` row scans, and SWE-bench summary artifacts.
