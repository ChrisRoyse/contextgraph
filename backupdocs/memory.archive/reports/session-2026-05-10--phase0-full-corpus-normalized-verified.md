---
namespace: reports
created: 2026-05-10T03:27:28Z
updated: 2026-05-10T03:28:41Z
status: active
tags: report, session, phase0, corpus, swebench, rocksdb, docker, fsv
---

# Session Report: 2026-05-10 — Phase 0 Full Corpus Verified

## Task: Complete and verify Phase 0 SWE-bench mutation corpus with D-backed Docker storage
## Outcome: complete

## Summary

The second half of Phase 0 had already generated successfully, but final merged
verification exposed a root-cause issue: the canonical oracle verdict hash used
raw `test_output.txt` exception hints. The structured SWE-bench report for the
failing row was stable, while pytest rendered different raw exception text
across equivalent unresolved runs.

The fix canonicalizes SWE-bench verdicts from structured `report.json` /
`tests_status`, adds a fresh-output `normalize` command for real corpus repair,
and adds targeted `verify --sample-key task_id|category` so exact rows can be
re-run through Docker without expanding to a huge sample.

## Files Created or Modified in the Repo

- `crates/context-graph-mejepa-corpus/src/swebench.rs` — stops hashing raw log exception hints; adds stable SWE-bench verdict normalization.
- `crates/context-graph-mejepa-corpus/src/bin/mejepa-corpus.rs` — adds `normalize` and targeted verify sample keys.
- `docs/ruvectorfindings/specs/tasks/phase0-corpus/_index.md` — updates Phase 0 production status to complete and verified.
- `docs/ruvectorfindings/specs/tasks/phase0-corpus/TASK-CORPUS-018.md` — updates verdict mapping docs to match the structured-report canonicalization.

## Files Written or Updated in ./memory/

- `memory/discoveries/2026-05-10--phase0-swebench-log-exception-hash-instability.md`
- `memory/journal/2026-05-10--success-phase0-full-corpus-normalized-verified.md`
- `memory/reports/session-2026-05-10--phase0-full-corpus-normalized-verified.md`

## Decisions Made

- Canonical SWE-bench corpus hashes must use `report.json` / `tests_status`, not raw `test_output.txt` exception hints. Rationale: raw logs are diagnostic and can differ across equivalent unresolved runs.
- The repaired corpus is written to a fresh directory rather than mutating the failed merged corpus in place. Rationale: preserve failed evidence and avoid partial in-place rewrites.

## Discoveries

- Equivalent SWE-bench unresolved runs can have identical `tests_status` and different raw pytest-rendered exception text, which invalidates hashes if log text drives canonical verdicts.

## Failures

- Merged verify failed on `sympy__sympy-13146|wrong_file` before the fix. Lesson: structured report artifacts are the source of truth; log-scraped exception hints cannot be part of stable corpus identity.

## Successes

- Normalized full corpus source of truth:

```text
/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509
entries=2400
RocksDB mutation/verdict/provenance CFs=2400/2400/1
stats.passes=true
verify.passes=true
corpus_sha256=f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33
```

## Open Blockers

- None for Phase 0 Python corpus production.

## Tests / Build

- `cargo fmt --package context-graph-mejepa-corpus`
- `cargo test -p context-graph-mejepa-corpus`
- `cargo clippy -p context-graph-mejepa-corpus --no-deps --lib --bins --examples -- -D warnings`
- `cargo build --release -p context-graph-mejepa-corpus`
- `target/release/mejepa-corpus normalize --corpus /tmp/contextgraph-mejepa-corpus-prod-merged-20260509 --output /tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509`
- `target/release/mejepa-corpus stats --corpus /tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509`
- `sg docker -c 'target/release/mejepa-corpus verify --corpus /tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509 --sample-fraction 0.001 --sample-key "sympy__sympy-13146|wrong_file" --run-id-prefix mejepaPhase0VerifyMergedNorm --overall-timeout-secs 3600'`
- Final sanity readback: `verify-evidence.json` has `passes=true`, `entry_count=2400`, `mutation_cf_count=2400`, `verdict_cf_count=2400`, `provenance_cf_count=1`, `oracle_sample_count=4`, and no failures.

## Resume Here Next Session

- Use `/tmp/contextgraph-mejepa-corpus-prod-merged-normalized-20260509` as the Phase 0 corpus source of truth.
- Keep Docker evaluator images until no further SWE-bench verification is needed; all 300 are present and stored in Docker Desktop's D-backed VHDX.
- Watch for old references to `/tmp/contextgraph-mejepa-corpus-prod-merged-20260509`; that path is preserved as failed pre-normalization evidence, not the active corpus.
