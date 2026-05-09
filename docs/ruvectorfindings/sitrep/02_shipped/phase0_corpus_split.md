# Phase 0 Corpus Split (Tier 1.4 / 2.4)

> **Shipped:** 2026-05-09
> **Module:** `crates/context-graph-mejepa-corpus/src/split.rs`
> **Tests:** included in 81 `context-graph-mejepa-corpus` tests
> **FSV evidence:** `/tmp/contextgraph-mejepa-corpus-fsv/{split-evidence.json, split-assignments.json, split-repo-atomic-assignments.json}`

## What this primitive does

Train/calibration/holdout split of the checked-in 300-task SWE-bench
Lite corpus with two explicit modes:

- `SplitMode::InstanceAtomic` is the default for Phase 0 corpus
  generation. It assigns whole task instances independently and reaches
  the clean 80/10/10 target exactly on Lite: `[240, 30, 30]`.
- `SplitMode::RepoAtomic` is retained as the no-shared-repo diagnostic
  mode. On the real Lite distribution it produces `[195, 77, 28]`
  because the repo groups are too skewed for exact 80/10/10 without
  splitting repos.

API:
- `split_corpus(entries: &[CorpusEntry], config: SplitConfig) ->
  SplitReport` — produces deterministic train/cal/holdout assignments
  using the selected mode.
- `cross_cut_weights(report: &SplitReport) -> [f64; 3]` — returns the
  total weight of edges crossing each bucket pair in the
  "tasks share a repo" graph. All zero ⇔ no repo-level cross-bucket
  edges in `RepoAtomic` mode.
- `assert_no_test_patch_leakage(...)` — checks the source-of-truth
  `official_test_patch_sha256` values and fails closed if an instance is
  missing a hash or if the same test-patch hash crosses buckets.

## Why it matters

Doc 09 requires a clean held-out gate for Phase 4/8. The original
repo-atomic interpretation removes repo-level leakage, but the real
Lite distribution has only 12 repos and two dominant groups:
`django/django=114` and `sympy/sympy=77`. That makes exact 80/10/10
mathematically unattainable if whole repos are indivisible.

The current contract therefore separates the two invariants instead of
hiding the tradeoff:

- Active corpus generation uses `InstanceAtomic` to satisfy the required
  240/30/30 train/cal/holdout sizes and verifies no exact test-patch SHA
  crosses buckets.
- Audits can still run `RepoAtomic` when the question is "could this
  metric be inflated by shared repo context?" The evidence records the
  unavoidable `[195, 77, 28]` shape for that stricter mode.

## Implementation notes

### RepoAtomic algorithm
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

### InstanceAtomic algorithm

1. Sort task ids deterministically after a seeded SplitMix64 shuffle.
2. Compute target counts by rounded target fractions.
3. Assign exactly `train`, `calibration`, and `holdout` counts.
4. Run `assert_no_test_patch_leakage` against the manifest
   `official_test_patch_sha256` map so duplicate or missing test patches
   cannot silently make an instance-level split unsafe.

### Linter's invariant additions
- Duplicate `task_id` rejection at split entry (would have produced
  silently inconsistent assignments).
- Whitespace-only string validation on `task_id` and `repo_id`.

### FSV at /tmp/contextgraph-mejepa-corpus-fsv/
- `split-evidence.json` — real checked-in 300-task SWE-bench Lite
  manifest corpus. It records `instance_atomic.bucket_sizes=[240,30,30]`,
  `instance_atomic.achieves_80_10_10=true`,
  `official_test_patch_sha256_count=300`,
  `unique_official_test_patch_sha256_count=300`, and
  `test_patch_sha_leakage_check_passed=true`.
- `split-assignments.json` — active instance-atomic per-task bucket
  assignment for reproducibility.
- `split-repo-atomic-assignments.json` — diagnostic repo-atomic
  assignment. It records `repo_atomic.bucket_sizes=[195,77,28]` and
  `cross_cut_weights=[0,0,0]`.

Boundary cases include empty corpus, duplicate task id, bad fractions,
single-repo repo-atomic degeneracy, missing test-patch hash, and
duplicate test-patch hash across buckets.

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
