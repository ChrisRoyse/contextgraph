# 15 - Multi-Seed Completion And Statistical Reporting

## Goal

Convert the current paper-small evidence (one implicit-seed pass, one
seed-42 pass, one preserved seed-43 failure, one seed-42 v2 in flight) into a
paper-quality multi-seed result with rigorous statistical reporting. The paper
should never report a single-seed number without explicit qualification.

## Why This Comes First

Smokedoc 07 §Statistical Reporting already requires "at least 3 seeds for the
career taxonomy benchmark" with "mean and standard deviation" and "do not
average away failed runs". The seed-control patch
(`20260430_seed_control_patch.md`) added the necessary CLI surface
(`--career-training-seed`, `--career-ablation-training-seed`). The remaining
work is execution + statistical analysis tooling.

## Required Seed Set

Minimum: **5 seeds** for the main career run (not 3). Reasoning:

- The seed-43 failure shows the A0 gate is seed-sensitive at small data scale;
  3 seeds gives n=2 if one fails, which is not enough for a meaningful std.
- 5 seeds keeps wall-time tractable on a single 5090 (each 1K run is ~169s of
  research-smoke + training; ~15 minutes total per seed; <90 minutes for 5
  seeds end-to-end).
- 5 seeds matches the n=5 floor used in NeurIPS reproducibility checklist
  guidance for "reasonable runs to reduce variance".

Seeds: `42, 43, 44, 45, 46`. Use prime offsets if any of these fail
deterministically: `47, 53, 59, 61, 67`. Record every attempted seed in
`run_manifest.json`, including failures.

## Decision: Ablation Seed Policy

The seed-control patch defaults ablations to the checked-in control seed when
only `--career-training-seed` is passed. This is correct for the **main**
ablation table (A0-A5 must remain a controlled comparison). For multi-seed
**stability** of the ablation table, run the full A0-A5 matrix on **2 seeds**
out of the 5 (e.g. `42, 44`) by passing
`--career-ablation-training-seed=$SEED`. Report:

- Main ablation table at the control ablation seed (existing).
- Stability appendix table: A0-A5 values at 2 ablation seeds, with paired delta
  per ablation row.

## Statistical Reporting Pipeline

The paper-small run already writes `paper_tables/table_prediction_metrics.csv`
and `paper_tables/table_ablation.csv`. Add a post-processing CLI command that
reads N bundle directories and writes:

```text
paper_tables/
  table_prediction_metrics_multiseed.csv     # mean, std, n, BCa CI lower, upper
  table_ablation_multiseed.csv               # same shape across A0-A5
  table_paired_ablation_tests.csv            # paired bootstrap delta + permutation p
  table_seed_failures.csv                    # failed seeds, error codes, gate, metric
```

The new CLI command:

```bash
target/debug/context-graph dynamicjepa multiseed-aggregate \
  --bundle-root tmp/mejepa_research_runs \
  --bundle-glob '20260430T_paper_small_1k_seed*_v1' \
  --output-root tmp/mejepa_research_runs/aggregate_paper_small_v1 \
  --bootstrap-iters 10000 \
  --confidence 0.95 \
  --json
```

It must fail closed if:

- fewer than 3 passed bundles match the glob;
- any bundle directory is missing `run_manifest.json` or `fsv_report.json`;
- bundle metric schemas disagree (different column counts, different ablation
  IDs, different domain set);
- the in-flight seed-42-v2 bundle is partial (`status != passed/failed`).

Failed bundles are listed in `table_seed_failures.csv` and are **never** mixed
into mean/std rows.

## Bootstrap And Permutation Test Choice

Use **BCa (bias-corrected and accelerated) bootstrap** for confidence intervals.
Justification: small n (5), skew likely on MSE, BCa corrects both bias and
acceleration unlike percentile bootstrap. 10,000 resamples per metric.

For paired ablation comparisons (e.g. A1 vs A0 on the same seed), use the
**paired sign-flip permutation test** described in the November 2025 paired
bootstrap protocol. Report two-sided p-values; state α=0.05 explicitly; state
that we do **not** Bonferroni-correct across ablations because A0-A5 are not
all directly compared (only A_i vs A0 baseline). Disclose this choice.

## What "Significantly Outperforms" Means In This Paper

The paper must not casually claim significance. Concrete rules:

1. Single-seed deltas are never reported as "significantly better".
2. A claim that ablation A_i differs from A_0 requires:
   - paired n≥5 across seeds;
   - sign-flip p < 0.05;
   - BCa 95% CI of the paired delta excludes zero.
3. A claim that the model beats a baseline (random-init, shuffled-target)
   requires the same plus the baseline's own multi-seed BCa CI not overlapping
   the model's BCa CI.

Otherwise the paper says "reduced", "trended toward", or "comparable", with the
delta and CI in parentheses.

## Source-Of-Truth Additions

Each multi-seed aggregate run writes a new immutable bundle:

```text
tmp/mejepa_research_runs/aggregate_paper_small_v1/
  aggregate_manifest.json          # input bundle paths + sha256
  aggregate_fsv_report.json        # all 5 child manifest hashes; pass/fail per seed
  paper_tables/
    table_prediction_metrics_multiseed.csv
    table_ablation_multiseed.csv
    table_paired_ablation_tests.csv
    table_seed_failures.csv
  plots/
    multiseed_val_mse_box.svg      # one box per (domain × ablation), n points
    multiseed_paired_delta_forest.svg
```

The aggregate bundle path is recorded in every paper figure caption.

## Edge Cases (Fail-Closed)

- Bundle path glob matches zero bundles: typed `VALIDATION` error, no aggregate
  dir created.
- Two bundles claim the same `run_id`: typed `VALIDATION` error.
- A bundle's `run_manifest.json` says `passed` but its
  `paper_tables/table_prediction_metrics.csv` is empty: typed `EVIDENCE` error
  naming the bundle.
- Aggregate run started against a bundle whose ablation ID set differs from
  the others: typed `SCHEMA` error naming the offending bundle and missing IDs.
- Aggregate run that would silently exclude a passed bundle (e.g. via
  `--exclude-seed`) is **not allowed**; exclusion requires explicit `--reason`
  flag and is recorded in `aggregate_manifest.json`.

## FSV Extension

Smokedoc 08 §Stage Sources Of Truth gains one row:

| Stage | Source Of Truth |
| --- | --- |
| Multi-seed aggregate | `aggregate_manifest.json`, child bundle hashes, `*_multiseed.csv`, `*_paired_*.csv` |

Smokedoc 04 §Gate 10 (Reproducibility Details) gains:

- Pass requires ≥3 passed seeds + BCa CI for every paper-cited metric +
  paired permutation results for every ablation comparison cited in the paper.

## Promotion To Paper

A paper-cited number is promotable only if:

1. It comes from `*_multiseed.csv`, not from a single-seed
   `table_prediction_metrics.csv`.
2. Its CI is reported alongside the point estimate.
3. The bundle list backing it is named in the figure/table caption.

The arXiv source must `\input{...}` these CSVs through a small generator (doc
19 specifies the LaTeX table generator). Hand-typed numbers in the paper are a
desk-reject risk under NeurIPS reproducibility expectations.

## Open Choices

- **Bootstrap iterations**: 10,000 is overkill for n=5 and ~6 metrics; 5,000 is
  also defensible. Lock 10,000 to remove the option from later debate.
- **Random seed for the bootstrap itself**: must be recorded in
  `aggregate_manifest.json` and held constant across paper revisions.
- **Whether to use median + IQR instead of mean + std**: stick with mean + BCa
  CI for paper because the metrics are bounded (cosine in [−1,1], MSE ≥ 0); BCa
  handles the skew without replacing the central tendency.

## Acceptance

This work is done when:

- 5 paper-small bundles exist (≥3 passed);
- `aggregate_paper_small_v1/` exists with all four CSVs;
- `multiseed-aggregate` CLI fails closed on every edge case above;
- the integration test in `crates/context-graph-cli/tests/` exercises real
  child bundles and a synthetic missing-bundle case;
- doc 13 is updated with the multi-seed evidence summary.
