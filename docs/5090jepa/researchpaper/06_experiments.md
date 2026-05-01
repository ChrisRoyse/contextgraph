# 5. Experiments

This section enumerates the experimental matrix. Numerical results are
reported in §8 and read directly from CSVs in
`tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/`.
We follow the project's seeded-and-aggregated protocol: every paper-cited
number is derived from at least five real O\*NET-backed seeds {42, 43,
44, 45, 46}, run as `paper_small_1k`, then aggregated through
`dynamicjepa multiseed-aggregate` with `--min-passed 5` and BCa
bootstrap (10{,}000 iterations, seed `20260430`, confidence 0.95).

## 5.1 Experiment A — Runtime Universality

**Question.** Does the *same* runtime, storage, trainer, and FSV harness
handle three unrelated domains by domain pack alone, with no
domain-specific code branches?

**Domains.** `counter_world.v1` (480 params), `gridworld_5x5.v1`
(16{,}960 params), `career_taxonomy.v1` (15{,}344 params at A1).

**Acceptance.** Pass requires (a) all three domains produce a
`dj_verification_runs` row with status `passed`; (b) the same five
artifact files (`config.json`, `evaluation_report.json`,
`manifest.json`, `metrics.json`, `model.safetensors`) are persisted in
`dj_model_artifacts` for each; (c) all artifact registry hashes match
recomputed file hashes (`artifact_hashes.json` mismatch count = 0);
(d) dataset shards for each domain pass leakage checks; (e) the same
`research-smoke` command produces all three.

## 5.2 Experiment B — Career Next-State Prediction

**Question.** Does the trained predictor beat random-init and
shuffled-target baselines on `career_taxonomy.v1`?

**Headline metrics** (from `paper_tables/table_prediction_metrics_multiseed.csv`,
`career_taxonomy` rows; 5 seeds; bootstrap 10{,}000 iters):

| Metric | Mean | Stddev | BCa 95% Low | BCa 95% High |
| --- | ---: | ---: | ---: | ---: |
| `val_latent_mse` | 0.152213508681 | 0.010272102979 | 0.144629287164 | 0.160877982989 |
| `val_latent_cosine` | 0.904866698041 | 0.043638221350 | 0.861495866042 | 0.931127632841 |
| `test_latent_mse` | 0.268471774864 | 0.072050852408 | 0.224106912986 | 0.345758698422 |
| `test_latent_cosine` | 0.867637059212 | 0.067656811317 | 0.806301640199 | 0.912141387437 |
| `val_action_contrast_acc` | 0.960000000000 | 0.043445367992 | 0.916000000000 | 0.982000000000 |
| `random_init_cosine` | 0.107025190635 | 0.255155885248 | -0.063261060473 | 0.370001948997 |
| `shuffled_target_cosine` | 0.770463932750 | 0.106715585432 | 0.670717900428 | 0.837753151255 |

**Acceptance.** The trained predictor passes if `val_latent_cosine`'s BCa
lower bound exceeds `shuffled_target_cosine`'s BCa upper bound by a
seed-paired margin (paired sign-flip permutation reported in §8) AND
exceeds `random_init_cosine`'s BCa upper bound by the same margin.
The two conditions are met for cosine but not for MSE because MSE is
unbounded above and cosine is the directly comparable scale.

## 5.3 Experiment C — Action-Conditioned Planning

**Question.** Do different candidate actions produce different predicted
next-state panels, and do guard decisions vary with predicted utility?

**Procedure.** `research-smoke` runs `dynamicjepa plan` on a chosen
state with the five career candidate actions enumerated in §3.5. Five
`dj_predictions` and five `dj_guard_decisions` rows are persisted; one
`dj_plan_traces` row records the candidate set, predictions per
candidate, guard decision per candidate, selected action, and utility
scores. Each row is independently re-decoded.

**Acceptance.** Pass requires (a) the five guard decisions are not all
identical (variation in guard verdicts proves action conditioning is
informative); (b) the selected action matches the maximum-utility
candidate; (c) the model artifact hash referenced by every candidate's
prediction equals the current career model artifact's recomputed hash.

## 5.4 Experiment D — Ablation Matrix

The six ablations are A0–A5. Definitions are reproduced from
[smokedocs/06_experiment_matrix_and_ablations.md](../smokedocs/06_experiment_matrix_and_ablations.md):

| ID | Panel | Action conditioning | Pairwise features | VICReg |
| --- | --- | --- | --- | --- |
| A0 | taxonomy only | yes | no | yes |
| A1 | taxonomy + skill vector | yes | no | yes |
| A2 | taxonomy + skill + transition distance | yes | no | yes |
| A3 | A2 (`ignore_action=true`) | NO | no | yes |
| A4 | A2 + occupation/action pairwise one-hot | yes | YES | yes |
| A5 | A2 (variance/covariance loss = 0) | yes | no | NO |

A3 is the negative control for action conditioning. It persists
`model.predictor.ignore_action=true` in the config; training and
inference *zero* the action vector rather than inventing a fallback
path. The ablation effective seed is the source-fixture control seed
unless `--career-ablation-training-seed` is explicitly supplied; this
prevents ablation comparisons from being polluted by main-seed
variation
([reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md)).

**Multi-seed ablation table** (5 seeds; from
`paper_tables/table_ablation_multiseed.csv`, val cosine column):

| Ablation | Mean | Stddev | BCa Low | BCa High |
| --- | ---: | ---: | ---: | ---: |
| A0 | 0.97557 | 0 | 0.97557 | 0.97557 |
| A1 | 0.90487 | 0.04364 | 0.86150 | 0.93113 |
| A2 | 0.95071 | 0 | 0.95071 | 0.95071 |
| A3 | 0.79008 | 0 | 0.79008 | 0.79008 |
| A4 | 0.94159 | 0 | 0.94159 | 0.94159 |
| A5 | 0.91483 | 0 | 0.91483 | 0.91483 |

Why is the standard deviation zero for A0, A2, A3, A4, A5 across the
five seeds? Because the *ablation* training seed is fixed at the
source-fixture control seed (effective `42`) for all five main career
seeds — only A1 (the headline panel) varies its training seed across
seeds 42–46. This is the deliberate, documented choice from
[smokedocs/15_multiseed_completion_and_statistical_reporting.md](../smokedocs/15_multiseed_completion_and_statistical_reporting.md):
the *main* ablation table is a controlled comparison; main-seed
variation is reserved for the headline metrics. A separate stability
appendix would re-run A0–A5 across two ablation seeds, but is not the
headline contribution.

A3 (action removed) collapses to val cosine 0.790 (constant across
seeds because the seed is fixed). The validation cosine of every other
ablation exceeds A3 by 0.10 to 0.19. The paired sign-flip permutation
test at $\alpha=0.05$ across 5 seeds returns $p = 0.0625$ for the
A0→A3 comparison on `val_latent_cosine` (read from
`table_paired_ablation_tests.csv` row `val_latent_cosine,
A0_taxonomy_only_action_vicreg, A3_taxonomy_skill_transition_no_action_vicreg`,
field `sign_flip_p` = 0.0625, mean delta = -0.185489665803). Per
[smokedoc 15](../smokedocs/15_multiseed_completion_and_statistical_reporting.md)
we therefore report the A3 collapse as **directionally significant in
the predicted direction with sign-flip $p = 0.0625$ and BCa CI for the
delta entirely below zero** rather than as "significantly different"
in the strict sense; the n=5 sample bound forbids the latter.

## 5.5 Experiment E — Auditability and Failure Behavior

**Question.** Does the system fail closed and leave usable diagnostics
for every named edge case?

**Edge cases.** §4.4 enumerates the data-side edges; §6 enumerates the
infrastructure-side edges (existing run root, missing O\*NET ZIP,
existing aggregate root, expected-bundle-count mismatch, duplicate
seed, invalid reference kind, missing aggregate, numeric drift,
existing release archive output, missing required SIF, invalid SIF).
Per
[reports/20260501_release_c_repro_artifact_fsv.md](../smokedocs/reports/20260501_release_c_repro_artifact_fsv.md)
and
[reports/20260501_sif_release_candidate_fsv.md](../smokedocs/reports/20260501_sif_release_candidate_fsv.md)
each edge produced a typed error code (e.g. `VALIDATION`, `SCHEMA`,
`RELEASE_REFERENCE_MISMATCH`,
`RELEASE_ARCHIVE_APPTAINER_INSPECT_FAILED`) and *did not* mutate the
output filesystem.

## 5.6 Experiment F — Single-GPU Feasibility

**Question.** Does the entire research-smoke ladder run on one RTX 5090
under realistic time budgets?

**Wall time, smoke (120 events).** 169{,}132~ms = ~169~s for the full
run, including 78 commands, 9 verification domains, 11 edge cases, and
6 ablations
([smokedocs/13_current_implementation_and_benchmark_status.md](../smokedocs/13_current_implementation_and_benchmark_status.md)).

**Wall time, paper-small (1{,}000 events).** Per-seed runs are bounded
by training; the integration FSV suite passes in 190.92~s
[reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md).
A complete five-seed release-C gate (including container build,
deterministic tar exports, container verify, aggregate, and edge cases)
fits in ~2 hours of wall time on this host.

**VRAM.** The 32~GiB RTX 5090 is far above the working set; the
trainer's working set never exceeds ~8~GiB during career-taxonomy
training. We do not report an empirical peak-VRAM table because the
trainer is not memory-bound at this model size; the headline numbers
in §8 are instead time- and quality-bound.

## 5.7 Deferred Axes

Excluded from the headline matrix and called out as future work in §11:

- a top-k occupation decoder head with reconstruction metrics
  ([smokedoc 16](../smokedocs/16_decoder_head_and_topk_metrics.md));
- multi-step rollouts $H>1$, direct vs. autoregressive
  ([smokedoc 24](../smokedocs/24_multistep_rollout_and_planning_horizon.md));
- ESCO replication and O\*NET ↔ ISCO crosswalk
  ([smokedoc 18](../smokedocs/18_esco_crosswalk_and_replication_study.md));
- larger model scales beyond paper-small (1{,}000 events).
