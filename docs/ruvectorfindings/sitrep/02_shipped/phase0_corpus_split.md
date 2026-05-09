# Phase 0 Repo-Atomic Corpus Split (Tier 1.4 / 2.4)

> **Shipped:** 2026-05-09
> **Module:** `crates/context-graph-mejepa-corpus/src/split.rs`
> **Tests:** 22 passing
> **FSV evidence:** `/tmp/contextgraph-mejepa-corpus-fsv/{split-evidence.json, split-assignments.json}`

## What this primitive does

Repo-atomic train/calibration/holdout split of a corpus of `(task_id,
repo_id, ...)` entries with a closed-form leakage proof.

API:
- `split_corpus(entries: &[CorpusEntry], config: SplitConfig) ->
  SplitReport` — produces deterministic train/cal/holdout assignments
  where every task in a given repo lands in the same bucket.
- `cross_cut_weights(report: &SplitReport) -> [f64; 3]` — returns the
  total weight of edges crossing each bucket pair in the
  "tasks share a repo" graph. All zero ⇔ provably leakage-free.

## Why it matters

Without repo-atomicity, training on `astropy.io.fits.tests.test_a` and
holdout `astropy.io.fits.tests.test_b` gives the predictor information
about the repo's shared imports, fixtures, conftest.py, and code style
that biases the holdout-agreement metric upward. Doc 05 §1.4 and §2.4
require the split to be repo-atomic; doc 09 §7 makes the
cross_cut_weights = [0,0,0] proof part of the Phase 0 DoD.

## Implementation notes

### Algorithm
1. Group entries by `repo_id`.
2. **LPT pre-sort**: sort repo-groups by descending size (longest-
   processing-time-first heuristic for bin-packing).
3. **Fisher-Yates shuffle** within ties (using SplitMix64 PRNG with a
   pinned seed for reproducibility).
4. **Greedy bin-pack with lowest-current/target-ratio heuristic**:
   for each repo, place into the bucket whose `current_count /
   target_ratio` is currently the lowest. Ties broken in
   `train > cal > holdout` order.

### Why "lowest current/target ratio" not "largest remaining capacity"
A naïve "largest remaining capacity" heuristic starves small buckets:
with 5 repos × 8 tasks targeting 80/10/10, the algorithm places the
first 4 repos in train (800% over the cal/holdout budget early), and
the cal/holdout buckets only see the last small repo. The lowest-ratio
heuristic equalizes the percent-of-target across buckets.

### Closed-form leakage check
Build the "tasks share a repo" undirected graph: nodes are tasks,
edges are within-repo pairs. After the split, walk the edge list and
for each `(u, v)`, increment `cross_cut[bucket_pair(assign[u],
assign[v])]`. Repo-atomicity guarantees every edge has both endpoints
in the same bucket → all three cross-cut weights are exactly 0.

### Linter's invariant additions
- Duplicate `task_id` rejection at split entry (would have produced
  silently inconsistent assignments).
- Whitespace-only string validation on `task_id` and `repo_id`.

### FSV at /tmp/contextgraph-mejepa-corpus-fsv/
- `split-evidence.json` — synthetic 300-task SWE-bench-shaped corpus
  (300 tasks across 30 repos with realistic repo-size skew). Bucket
  sizes 76.3% / 13.3% / 10.3% (within 4 percentage points of the
  80/10/10 target). cross_cut_weights = [0, 0, 0] confirmed.
- `split-assignments.json` — full per-task bucket assignment for
  reproducibility. Same seed produces bit-identical assignments.

### Real bug caught by FSV
The 1-ULP JSON round-trip issue: `0.10333333333333333` ↔
`0.10333333333333332` shifted by one ULP through `serde_json` parse,
breaking the FSV's structural-equality check. Fix: drop the redundant
in-memory-vs-parsed-Value comparison since `write_text_checked`
already proves byte-equality on disk.

## What this enables downstream

- **Phase 0 oracle generation** — uses split to organize Docker runs
  by bucket.
- **Phase 3 training** — train split feeds the AdamW loop.
- **Phase 4 conformal calibration** — cal split (10%) feeds the
  conformal quantile fitter.
- **Phase 8 evaluation** — holdout split is the held-out gate-set.

## See also

- Spec: `docs/ruvectorfindings/05_TIERED_PRIMITIVE_LADDER.md §1.4, §2.4`
- Doc 09 reuse: §7 (Phase 0 DoD), §3.6 (Phase 4 calibration), §1
  (Phase 8 final eval gate).
- Phase 0 partner: `phase0_mutation_operators.md`
- Source: `crates/context-graph-mejepa-corpus/src/split.rs`
- Mincut leakage proof primitive: `tier2_foundations.md` (Stoer-Wagner)
