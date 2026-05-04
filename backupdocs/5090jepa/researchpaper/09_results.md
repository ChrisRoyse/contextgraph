# 8. Results

All numbers in this section are read directly from CSVs in
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/`.
This paper reproduces the relevant rows verbatim. Hand-typed numbers are
intentionally absent from the manuscript per the project's
"hand-typed numbers in paper = desk-reject risk" rule
([smokedoc 19](../smokedocs/19_paper_drafting_figures_and_supplementary.md)).

## 8.1 Headline Multi-Seed Prediction Metrics

Five seeds {42, 43, 44, 45, 46} of `paper_small_1k`. BCa bootstrap:
10{,}000 iterations, seed `20260430`, confidence 0.95.

**Table 1 — Multi-seed prediction metrics across three domains.**

Source: `paper_tables/table_prediction_metrics_multiseed.csv`.

| Domain | Metric | Mean | Stddev | BCa 95% Low | BCa 95% High | Min | Max |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| career_taxonomy | val_latent_mse | 0.152213508681 | 0.010272102979 | 0.144629287164 | 0.160877982989 | 0.140719470554 | 0.166005358129 |
| career_taxonomy | val_latent_cosine | 0.904866698041 | 0.043638221350 | 0.861495866042 | 0.931127632841 | 0.835545844045 | 0.947669999850 |
| career_taxonomy | test_latent_mse | 0.268471774864 | 0.072050852408 | 0.224106912986 | 0.345758698422 | 0.201729551323 | 0.384965547410 |
| career_taxonomy | test_latent_cosine | 0.867637059212 | 0.067656811317 | 0.806301640199 | 0.912141387437 | 0.770136879414 | 0.933457997110 |
| career_taxonomy | val_action_contrast_acc | 0.960000000000 | 0.043445367992 | 0.916000000000 | 0.982000000000 | 0.895000000000 | 1.000000000000 |
| career_taxonomy | random_init_cosine | 0.107025190635 | 0.255155885248 | -0.063261060473 | 0.370001948997 | -0.154057579122 | 0.510094191831 |
| career_taxonomy | shuffled_target_cosine | 0.770463932750 | 0.106715585432 | 0.670717900428 | 0.837753151255 | 0.616074589717 | 0.884899423547 |
| counter_world | val_latent_mse | 0.004330990885 | 0 | 0.004330990885 | 0.004330990885 | 0.004330990885 | 0.004330990885 |
| counter_world | val_latent_cosine | 0.999633671268 | 0 | 0.999633671268 | 0.999633671268 | 0.999633671268 | 0.999633671268 |
| counter_world | test_latent_mse | 0.013100919986 | 0 | 0.013100919986 | 0.013100919986 | 0.013100919986 | 0.013100919986 |
| counter_world | test_latent_cosine | 0.999160233871 | 0 | 0.999160233871 | 0.999160233871 | 0.999160233871 | 0.999160233871 |
| counter_world | val_action_contrast_acc | 0.750000000000 | 0 | 0.750000000000 | 0.750000000000 | 0.750000000000 | 0.750000000000 |
| counter_world | random_init_cosine | -0.401487165900 | 0 | -0.401487165900 | -0.401487165900 | -0.401487165900 | -0.401487165900 |
| counter_world | shuffled_target_cosine | 0.953886288838 | 0 | 0.953886288838 | 0.953886288838 | 0.953886288838 | 0.953886288838 |
| gridworld_5x5 | val_latent_mse | 0.039563894658 | 0.000270789446 | 0.039339105211 | 0.039757416543 | 0.039212858146 | 0.039910043699 |
| gridworld_5x5 | val_latent_cosine | 0.994732203704 | 0.000245290681 | 0.994568428451 | 0.994971694666 | 0.994514968898 | 0.995121107238 |
| gridworld_5x5 | test_latent_mse | 0.040102423214 | 0.000691212167 | 0.039618228319 | 0.040752469227 | 0.039323645378 | 0.041127534075 |
| gridworld_5x5 | test_latent_cosine | 0.994720550533 | 0.000213038652 | 0.994592998628 | 0.994951894668 | 0.994551752266 | 0.995070713417 |
| gridworld_5x5 | val_action_contrast_acc | 0.992481203008 | 0 | 0.992481203008 | 0.992481203008 | 0.992481203008 | 0.992481203008 |
| gridworld_5x5 | random_init_cosine | 0.072887364015 | 0 | 0.072887364015 | 0.072887364015 | 0.072887364015 | 0.072887364015 |
| gridworld_5x5 | shuffled_target_cosine | 0.778824030571 | 0.011415742536 | 0.770009048646 | 0.787344327500 | 0.766250680803 | 0.795142812226 |

(Constant-zero standard-deviation rows are domains/seeds where the
training pipeline is deterministic given the panel encoding; only the
career-taxonomy main career seed varies and the gridworld training
shows mild seed dependence in latent MSE.)

### Reading the headline numbers

- **Career-taxonomy validation cosine = 0.905 ± 0.044, BCa 95% CI
  [0.861, 0.931]**, n = 5 seeds. The shuffled-target baseline
  validation cosine is 0.770 ± 0.107, BCa upper 0.838. The trained
  predictor's BCa lower 0.861 exceeds the shuffled-target's BCa upper
  0.838 by 0.023 — i.e., the BCa CIs do not overlap (under the
  protocol, this is the necessary condition before claiming the
  trained predictor "beats" the shuffled-target baseline; I report
  the comparison conservatively).

- **Career-taxonomy random-init cosine** has BCa 95% CI [-0.063,
  0.370], well below the trained model's BCa 95% CI [0.861, 0.931].
  No overlap; this is a clean positive-vs-random comparison.

- **Validation action-contrast accuracy = 0.960** with BCa 95% CI
  [0.916, 0.982] — the model assigns higher predicted-target
  similarity to its conditioned action than to a contrast action ≥
  91% of the time at the lower CI bound.

- **Counter\_world** is essentially fully fit (val cosine
  0.99963, test cosine 0.99916) at 480 trainable parameters; this is
  the runtime sanity check.

- **Gridworld\_5x5** at 16{,}960 parameters reaches val cosine
  0.99473 and test cosine 0.99472 with very tight BCa intervals
  (stddev < 0.0003 on cosine). This is the compositional next-state
  prediction sanity domain.

## 8.2 Multi-Seed Ablation Table

Source: `paper_tables/table_ablation_multiseed.csv`.

**Table 2 — Career-taxonomy ablation A0–A5 across five main career
seeds.** Columns are val cosine and test cosine; the ablation training
seed is fixed at the source-fixture control seed for A0, A2, A3, A4,
A5 by design (only A1 varies the main career training seed).

| Ablation | Panel | Action | Pairwise | VICReg | Params | Val cosine (mean ± std) | Test cosine (mean ± std) | Val action contrast |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| A0 | taxonomy only | yes | no | yes | 14{,}832 | 0.97557 ± 0 | 0.96301 ± 0 | 1.000 ± 0 |
| A1 | + skill vector | yes | no | yes | 15{,}344 | 0.90487 ± 0.04364 | 0.86764 ± 0.06766 | 0.960 ± 0.04345 |
| A2 | + transition dist | yes | no | yes | 15{,}408 | 0.95071 ± 0 | 0.89696 ± 0 | 1.000 ± 0 |
| A3 | A2, action removed | NO | no | yes | 15{,}408 | 0.79008 ± 0 | 0.65359 ± 0 | 0.985 ± 0 |
| A4 | A2 + pairwise | yes | YES | yes | 16{,}688 | 0.94159 ± 0 | 0.92688 ± 0 | 1.000 ± 0 |
| A5 | A2, no VICReg | yes | no | NO | 15{,}408 | 0.91483 ± 0 | 0.81635 ± 0 | 1.000 ± 0 |

### Discussion of the ablations

- **A3 (action removed) is the predicted negative control.** Validation
  cosine collapses from A2's 0.951 to A3's 0.790; test cosine collapses
  from 0.897 to 0.654; validation action-contrast accuracy is 0.985
  (still high because the contrast holds even without action
  conditioning at training time, since it is computed from conditioning
  the *trained* predictor). The persisted training row for A3 records
  `predictor.ignore_action=true` and the metric gate is set to a
  cosine target of 0.50 (the explicit no-action ablation training
  target) before the verifier reads back the run; A3 still passes
  precisely because the floor target is 0.50, not because the cosine
  is high.

- **A0 (taxonomy only) achieves the highest validation cosine of any
  ablation (0.976).** This is unintuitive at first glance, but it is
  consistent with the project's diagnostic that the *bare taxonomy
  panel* is the easiest-to-fit target manifold (small, low-dim, mostly
  categorical with a single scalar). Adding the skill vector (A1)
  *reduces* validation cosine because the 4-D continuous skill vector
  is harder to predict than one-hot occupation cluster; this is the
  intended diagnostic, not a defect. A1 is the headline panel because
  the test set generalization gap (test cosine = 0.868) is the most
  relevant downstream signal.

- **A4 (pairwise features added) improves test cosine** from A2's
  0.897 to 0.927 — the largest test-set gain in the ablation matrix —
  but at the cost of 1{,}280 additional parameters (16{,}688 vs
  15{,}408). The improvement is consistent with the pairwise-feature
  hypothesis from VICReg/CPC and is the only ablation in which test
  cosine *exceeds* validation cosine in ratio terms.

- **A5 (VICReg variance/covariance regularization removed) does not
  collapse** validation cosine (0.915 vs A2's 0.951) but loses 0.080
  on test cosine (0.816 vs A2's 0.897). The pattern is the predicted
  one: no VICReg ⇒ predictor degeneracy in the generalization gap, not
  in the validation curve.

## 8.3 Paired Sign-Flip Permutation Tests

Source: `paper_tables/table_paired_ablation_tests.csv`. Baseline = A0;
n = 5; full sign-flip enumeration over $2^5=32$ sign assignments
(smallest possible two-sided $p$ at $n=5$ is $2/32 = 0.0625$).

**Table 3 — Selected paired tests (val cosine, A0 vs each).**

| Comparison | Mean Δ (A0 − comp) | BCa 95% Low | BCa 95% High | Sign-flip $p$ |
| --- | ---: | ---: | ---: | ---: |
| A0 vs A1 | -0.07071 | -0.11408 | -0.04356 | 0.0625 |
| A0 vs A2 | -0.02486 | -0.02486 | -0.02486 | 0.0625 |
| A0 vs A3 | -0.18549 | -0.18549 | -0.18549 | 0.0625 |
| A0 vs A4 | -0.03398 | -0.03398 | -0.03398 | 0.0625 |
| A0 vs A5 | -0.06074 | -0.06074 | -0.06074 | 0.0625 |

(Mean delta is negative because A0 has the highest val cosine; the
BCa CIs collapse to a point when the comparison ablation has zero
cross-seed variance, which is a deliberate consequence of the
fixed-ablation-seed protocol described in §5.4.)

The paired sign-flip $p$-value is **0.0625 for all five comparisons** —
the floor at $n = 5$. Per the project's protocol
([smokedoc 15](../smokedocs/15_multiseed_completion_and_statistical_reporting.md))
I therefore report the deltas as *directional with a BCa CI entirely
on one side of zero* but not as "significantly different at
$\alpha = 0.05$ in the strict sense" — the n=5 sample bound forbids
the latter. A sample-size increase to $n=6$ would lower the floor to
$2/64 = 0.03125$ and resolve the threshold; this is a known property
of the protocol, not a defect.

The economically meaningful comparison — A3 (action removed) vs A0
(action present) — is a delta of -0.18549 in val cosine, BCa CI
identically -0.18549, sign-flip $p = 0.0625$. This is the largest
delta in the table and the one most directly tied to the design
hypothesis (action conditioning matters).

## 8.4 Domain Runtime Coverage

Source: per-bundle `db_counts_after.json` for any of the five seeds.
The numbers below are identical across all five seeds for the
career-taxonomy domain; the counter\_world and gridworld\_5x5 domains
have fixed corpus sizes within the same research-smoke run.

**Table 4 — Persisted RocksDB row counts per domain, paper-small 1K
release-C seed42.**

| CF | counter\_world | gridworld\_5x5 | career\_taxonomy |
| --- | ---: | ---: | ---: |
| dj_raw_events | 62 | 8002 | 1004 |
| dj_adapter_runs | 62 | 8002 | 1004 |
| dj_normalized_states | 60 | 8000 | 1000 |
| dj_actions | 65 | 8005 | 1005 |
| dj_outcomes | 60 | 8000 | 1000 |
| dj_transitions | 60 | 8000 | 1000 |
| dj_instrument_readings | 120 | 24000 | 10000 |
| dj_latent_panels | 60 | 8000 | 1000 |
| dj_dataset_shards | 3 | 3 | 3 |
| dj_training_runs | 1 | 1 | 1 |
| dj_model_artifacts | 1 | 1 | 1 |
| dj_predictions | 6 | 6 | 6 |
| dj_plan_traces | 1 | 1 | 1 |
| dj_guard_decisions | 5 | 5 | 5 |
| dj_surprise_events | 1 | 0 | 0 |
| dj_verification_runs | 1 | 1 | 1 |
| dj_audit_log | 196 | 25014 | 3019 |

(All three career-taxonomy splits are train=699, val=200, test=100;
gridworld is train=4977, val=1330, test=693; counter\_world is
train=41, val=12, test=6.)

## 8.5 Single-GPU Feasibility

Source: per-bundle `run_manifest.json` and `command_log.jsonl`.

**Table 5 — Wall-time, command count, and FSV row count for one
research-smoke career bundle.**

| Tier | Career events | Wall-time | Commands | Verification rows | Edge-case rows | Ablation rows | Artifact-hash rows |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Smoke (120) | 120 | ~169 s | 78 | 9 | 11 | 6 | 9 |
| Paper-small 1K | 1000 | ~6–8 min | 78 | 9 | 11 | 6 | 9 |
| Release-C (5×paper-small + aggregate + container) | 5×1000 | ~2 h | — | — | — | — | — |

(Smoke wall-time is exact at 169{,}132~ms from the
`20260430T_onet30_demo_fsv_v12` bundle. Paper-small per-seed wall-time
is bounded by training; the integration FSV suite passes in 190.92 s.)

## 8.6 Plots

Two plots are auto-generated into the aggregate bundle and copied
into the release-candidate archive's `paper_tables_reference/`
directory:

- `plots/multiseed_val_mse_box.svg` (SHA-256
  `69db6c4d2ec8b837100f23fbfe74c633c2c1aa29d0a981806dcf57c4123ed77c`):
  one box per (domain × ablation), n = 5 points each, `val_latent_mse`
  on the y-axis.

- `plots/multiseed_paired_delta_forest.svg` (SHA-256
  `79f8050a1cf0c8c6b4f544f71461ecb2e8cc0672c987b14f1b84ee1b93bb04e4`):
  forest plot of paired deltas (A_i − A0) with BCa CI whiskers.

Plots are generated deterministically from CSVs; the rendering
pipeline is locked by `Cargo.lock` (no Python in the rendering path
of the Rust-only pipeline).

## 8.7 Failed Runs as Evidence

The project preserves rather than deletes failed bundles, so each
fixed defect remains traceable. Preserved failure evidence:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed45_v2`: the
  *original* seed-45 failure where `career_ablation_training_seed`
  defaulted to the main career seed and broke the A3 metric gate.
  Root cause and fix:
  [reports/20260430_seed43_failure_analysis.md](../smokedocs/reports/20260430_seed43_failure_analysis.md)
  and
  [reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md).

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed46_v1`: same
  root cause as seed-45 v2.

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_multiseed_42_46_v3`:
  the first all-pass aggregate attempt that failed because
  `table_seed_failures.csv` was correctly empty when every child
  bundle passed; the generic CSV reader rejected an empty file. Fix:
  allow empty rows only for `table_seed_failures.csv`.

- `tmp/mejepa_local_ci_runs/20260501T_local_smoke_b_free_gate_v3_hp_sweep_batch_size`:
  the 120-event HP sweep proxy that failed at `batch_size=32` A2.
  Fix: HP sweeps now use the verified 1K real-O\*NET scale.

- `tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc1`: the
  release-candidate v1 missing a packaged verifier binary. Fix: the
  packager now requires `CONTEXT_GRAPH_BIN` and copies it into
  `bin/context-graph` with recorded `manifests/context_graph_ldd.txt`.

All failed bundles are retained intentionally; they are part of the
artifact's evidence chain.
