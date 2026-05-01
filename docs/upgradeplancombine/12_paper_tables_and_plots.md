# 12 — New Paper Tables and Plots

This document specifies the new paper tables and plots emitted by the
combined runtime, and how they slot into the v1.1 manuscript update.

Existing paper tables remain unchanged for v1 bundles. v2 bundles
emit both the existing tables (with extended schemas where noted in
`07_storage_schema_changes.md`) and four new tables.

## 1. New paper tables

### 1.1 `paper_tables/table_mc_ratio.csv`

Per-domain meaning-compression ratio. One row per domain in the bundle.

```csv
domain,n_panel_slots,s1_per_event_constructive,s2_per_plan_constructive,s3_per_bundle,n_events,total_signals,mc_ratio_constructive,mc_ratio_measured,n_eff_loose,n_eff_strict,verdict
counter_world,2,10,33,140,60,1100,18.3,18.3,2.0,2.0,STRONG
gridworld_5x5,3,16,68,140,8000,130000,16.25,16.25,3.0,3.0,STRONG
career_taxonomy_a1,10,114,117,3060,1000,118500,118.5,113.2,9.85,9.62,WEAK
```

The `mc_ratio_measured` column is filled in once
`compute-mc-ratio` runs at end-of-bundle; the `n_eff_*` columns are
filled in once `audit-pairwise-mi` lands. The `verdict` column is
the per-domain Axiom-2 verdict
(`10_pairwise_mi_audit.md` §5).

### 1.2 `paper_tables/table_pairwise_mi_summary.csv`

Per-domain pairwise mutual-information summary. One row per domain.

```csv
domain,n_panel_slots,n_pairs,mean_mi_nats,median_mi_nats,p10_mi_nats,p90_mi_nats,n_redundant_pairs_loose,n_redundant_pairs_strict,n_eff_loose,n_eff_strict,bootstrap_seed,n_audit_events,estimator
career_taxonomy_a1,10,45,0.412,0.380,0.121,0.842,3,1,9.85,9.62,20260501,1000,ksg
gridworld_5x5,3,3,0.331,0.330,0.290,0.371,0,0,3.0,3.0,20260501,1000,ksg
counter_world,2,1,0.215,0.215,0.215,0.215,0,0,2.0,2.0,20260501,60,ksg
```

This is the bundle-level summary. The full per-pair matrix is in
`pairwise_mi_matrix.csv` under the same audit output directory.

### 1.3 `paper_tables/table_constellation_guard.csv`

Per-(domain, subject, plan) breakdown of $G_\tau$ acceptance.
One row per (domain, subject, plan trace) tuple in the bundle.

```csv
domain,subject_id,plan_trace_id,k_candidates,utility_accepted_count,gtau_accepted_count,both_accepted_count,no_accepted_candidate,selected_action_kind,selected_utility,selected_gtau_pass
career_taxonomy_a1,cluster.software,...,5,4,3,3,false,skill_gain_high_demand,0.78,true
career_taxonomy_a1,cluster.software,...,5,5,1,1,false,role_move_near,0.65,true
career_taxonomy_a1,cluster.data,...,5,3,2,2,false,skill_gain_adjacent,0.72,true
counter_world,global,...,3,3,3,3,false,step_up,0.81,true
gridworld_5x5,global,...,4,4,4,4,false,up,0.69,true
```

This becomes the v1.1 paper's empirical anchor for the
$G_\tau$-vs-utility contrast.

### 1.4 `paper_tables/table_threshold_calibration.csv`

Per-(domain, subject, modality) threshold calibration record.

```csv
domain,subject_id,modality_id,modality_name,calibration_set_count,percentile,tau,calibration_min,calibration_max,calibration_p10,calibrated_at_unix_ms,supersedes_seq
career_taxonomy_a1,cluster.software,1,occupation_cluster_onehot,100,10,0.95,0.95,1.00,0.95,1779900000000,0
career_taxonomy_a1,cluster.software,2,job_zone_scalar,100,10,0.90,0.85,0.99,0.90,1779900000000,0
...
```

50 rows for career\_taxonomy A1 (5 constellations × 10 modalities).

## 2. New plots

### 2.1 `plots/pairwise_mi_heatmap.svg`

$N_d \times N_d$ lower-triangular heatmap of normalised pairwise MI
for each domain. Colour scale: 0 (no MI) to 1 (fully redundant).
Annotated with domain id, audit timestamp, audit event count,
estimator, ksg\_k.

For career\_taxonomy A1: a 10x10 figure with 45 lower-triangle cells.
The visual fingerprint of the panel's information geometry is the
v1.1 paper's most striking new figure.

### 2.2 `plots/signal_density_per_event_distribution.svg`

A histogram (or violin plot) of per-event signal-yield, derived from
the audit-log `signal_yield` field, broken out by `operation_kind`.
For each domain, the figure shows:

- Per-event Slice-1 yield (deterministic).
- Per-event Slice-2 yield (model-output, paired).
- Per-event Slice-3 yield (verifier-readback).

Stacked-bar distribution across $n$ events. Visualises the
"lower-collapse-hazard slice grows" claim.

### 2.3 `plots/g_tau_acceptance_curve.svg`

Per-domain $G_\tau$ acceptance rate as a function of the threshold
percentile setting. The shipped runtime uses the 10th-percentile
default; the curve sweeps 5th, 10th, 15th, 20th, 25th to show
sensitivity. X-axis: percentile. Y-axis: fraction of plan candidates
that pass $G_\tau$. Plotted per-domain (one line per domain).

This curve is informative about how aggressive the calibration is
and how that interacts with utility-only acceptance.

## 3. Extended existing tables

### 3.1 `paper_tables/table_edge_cases.csv`

Extended from 11 to 26 rows per career bundle (15 new edges from
`04_constellation_design.md` §9, `05_pairwise_features_design.md`
§9, `06_guard_upgrade_g_tau.md` §9, plus three wrapper edges from
`08_cli_and_mcp_surface.md` §6).

Schema unchanged: `edge_kind`, `expected_error_code`,
`raw_event_delta`, `adapter_run_delta`, `state_delta`,
`transition_delta`, `audit_delta`, `verification_result`,
`root_cause_field_path`.

### 3.2 `paper_tables/table_ablation*.csv` (multiseed and per-seed)

The ablation rows extend with two new metrics derived from the
combined runtime:

- `g_tau_pass_rate`: fraction of plan candidates accepted by
  $G_\tau$ for each ablation. (Existing utility-pass rate is also
  preserved; both reported.)
- `mc_ratio_measured`: per-ablation MC ratio.

This reveals whether richer panels (A2, A4) materially change the
$G_\tau$ pass rate vs.\ the baseline (A0, A1).

### 3.3 `paper_tables/table_prediction_metrics*.csv`

No schema change: the cosine / MSE / action-contrast columns remain
identical so v1↔v2 comparison preserves the existing headline.

### 3.4 `paper_tables/table_runtime.csv`

Extended with two stage rows: `pairwise_materialisation_ms` and
`g_tau_evaluation_ms`. Other rows unchanged.

## 4. Extended aggregate tables

### 4.1 `paper_tables/table_mc_ratio_multiseed.csv`

5-seed aggregate of `table_mc_ratio.csv`. Mean, std, BCa CI on
`mc_ratio_measured` and on `n_eff_loose`/`n_eff_strict`.

### 4.2 `paper_tables/table_constellation_guard_multiseed.csv`

5-seed aggregate of `table_constellation_guard.csv`. Mean, std, BCa
CI on `gtau_accepted_count / k_candidates` and on the combined
`both_accepted_count / k_candidates` ratio.

## 5. Updated `aggregate_manifest.json`

```jsonc
{
  // existing fields unchanged ...
  "bundle_schema_version": 2,                     // new
  "aggregate_tables": {
    // ... existing 4 tables (prediction, ablation, paired, seed_failures) ...
    "table_mc_ratio_multiseed": {
      "row_count": 18,                            // 6 metrics × 3 domains
      "source_headers": [ ... ]
    },
    "table_constellation_guard_multiseed": {
      "row_count": 25,
      "source_headers": [ ... ]
    },
    "table_pairwise_mi_summary": {                // not multi-seeded; 1 audit per release
      "row_count": 6,
      "source_headers": [ ... ]
    }
  },
  "aggregate_pairwise_mi_audit": {
    "audit_uuid": "...",
    "estimator": "ksg",
    "ksg_k": 5,
    "n_audit_events": 1000,
    "verdict_per_domain": {
      "career_taxonomy_a1": "WEAK",
      "gridworld_5x5": "STRONG",
      "counter_world": "STRONG"
    },
    "n_eff_strict_per_domain": { ... },
    "started_at_unix_ms": ...,
    "ended_at_unix_ms": ...
  }
}
```

## 6. v1 ↔ v2 comparability

Existing tables (`table_prediction_metrics.csv`,
`table_ablation.csv`, etc.) keep identical schemas so v1 and v2
bundles' headline numbers are directly comparable. The release
reference for v2.0 (`reference/reference_release_v2.0.0.json`)
includes:

- All v1 numeric tolerances, unchanged.
- Plus tolerances for the four new tables (defined in
  `reference/tolerances_v2.0.json`):

```jsonc
{
  "table_mc_ratio": {
    "mc_ratio_measured": 0.5,
    "n_eff_loose": 0.3,
    "n_eff_strict": 0.2
  },
  "table_pairwise_mi_summary": {
    "mean_mi_nats": 0.05,
    "median_mi_nats": 0.05,
    "p10_mi_nats": 0.10,
    "p90_mi_nats": 0.10,
    "n_eff_loose": 0.3,
    "n_eff_strict": 0.2
  },
  "table_constellation_guard": {
    "gtau_accepted_count": 1,
    "both_accepted_count": 1,
    "selected_utility": 0.05
  },
  "table_threshold_calibration": {
    "tau": 0.02,
    "calibration_min": 0.05,
    "calibration_max": 0.05,
    "calibration_p10": 0.05
  }
}
```

These tolerances allow for KSG-estimator stochasticity (5%
bootstrap variance is normal) and for fp32 cosine numerical noise
(~$10^{-4}$).

## 7. Discussion section in the v1.1 paper

The new tables and plots feed three subsections in the v1.1 paper
update (`16_paper_v1_1_outline.md`):

- §3 (Method) gains a "Meaning-compression ratio in practice" box
  citing `table_mc_ratio.csv`.
- §6 (FSV) gains FSV-9..12 evidence rows.
- §8 (Results) gains:
  - Table 7: `table_mc_ratio.csv` excerpt.
  - Table 8: `table_pairwise_mi_summary.csv` excerpt.
  - Figure 1: `pairwise_mi_heatmap.svg` for career\_taxonomy A1.
  - Figure 2: `signal_density_per_event_distribution.svg`.
  - Figure 3: `g_tau_acceptance_curve.svg`.

The four new tables and three new plots make the v1.1 paper a
visibly larger empirical artefact than v1.0 without inflating the
page count: the new content goes into the Results section as 2 new
tables and 3 new figures, replacing the v1's Table 4 (Domain Runtime
Coverage) and Table 5 (Single-GPU Feasibility) as the *primary*
new-vs-old-result comparators.

## 8. Reproducibility chain

Each new paper-table CSV carries provenance:

| File | Source records | Determinism |
| --- | --- | --- |
| `table_mc_ratio.csv` | `dj_audit_log.signal_yield` aggregation | Deterministic given the audit log. |
| `table_pairwise_mi_summary.csv` | `audit-pairwise-mi` output, KSG with fixed seed | Deterministic given seed + sample size + estimator. |
| `table_constellation_guard.csv` | `dj_guard_decisions` rows | Deterministic given the trained predictor + frozen constellation. |
| `table_threshold_calibration.csv` | `dj_threshold_calibrations` rows | Deterministic given the calibration set + percentile. |

All four are reproducible bit-equivalent across re-runs on the same
host, modulo the documented PyTorch / Candle GPU non-determinism caveat
that already applies to the v1 cosine numbers.

## 9. Plot generation pipeline

The plots are emitted from the existing pipeline that produces
`plots/multiseed_val_mse_box.svg` etc. The implementation language is
Rust (no new Python dependency). Plot manifests
(`plots/plot_manifest.json`) gain three entries.

The plots are deterministic SVG (matplotlib-equivalent rendering via
the `plotters` Rust crate already used in v1). SHA-256 of each plot
file is recorded in `archive_file_hashes.json`.
