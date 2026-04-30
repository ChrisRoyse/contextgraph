# 06 - Experiment Matrix And Ablations

The paper should demonstrate both system behavior and model behavior. The matrix
below is sized for a single RTX 5090.

## Experiment A - Runtime Universality

Domains:

- `counter_world`;
- `gridworld_5x5`;
- `career_taxonomy`.

Question:

> Does the same runtime handle unrelated domains using domain packs, not
> domain-specific core branches?

Evidence:

- one verification row per domain;
- domain pack hashes;
- final CF counts;
- artifact hashes;
- command logs.

Metrics:

- pass/fail;
- runtime;
- CF count deltas;
- artifact size;
- prediction/plan/surprise row counts.

## Experiment B - Career Next-State Prediction

Question:

> Can ME-JEPA predict next career-taxonomy latent state better than random-init
> and shuffled-target baselines?

Metrics:

- validation/test latent MSE;
- validation/test cosine;
- action-contrast accuracy;
- random-init baseline;
- shuffled-target baseline;
- VICReg variance/covariance diagnostics.

Current implementation note: the smoke harness is latent-panel only. It does not
yet train a decoder/head for top-k occupation or skill-vector reconstruction, so
those metrics are not emitted as paper claims.

Current O*NET smoke metrics:

| Domain | Train | Val | Test | Params | Val MSE | Val Cosine | Test MSE | Test Cosine |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| counter_world | 41 | 12 | 6 | 480 | 0.004330990884665671 | 0.9996336712676964 | 0.013100919986046714 | 0.9991602338714566 |
| gridworld_5x5 | 4977 | 1330 | 693 | 16960 | 0.03936942384623141 | 0.9950110445426108 | 0.04023547898409154 | 0.9949794096655592 |
| career_taxonomy | 83 | 24 | 12 | 15344 | 0.17999517062789205 | 0.8776115717741934 | 0.25307116609684355 | 0.867371052118299 |

Paper-small 1K explicit-seed career metrics:

| Run ID | Seed | Val MSE | Val Cosine | Test MSE | Test Cosine |
| --- | ---: | ---: | ---: | ---: | ---: |
| `20260430T_paper_small_1k_seed42_v2` | 42 | 0.14071947055420186 | 0.9048476588324232 | 0.2017295513225419 | 0.8942226140087763 |
| `20260430T_paper_small_1k_seed43_v2` | 43 | 0.15950453028461578 | 0.9476699998495745 | 0.2869484249392498 | 0.9334579971102336 |
| `20260430T_paper_small_1k_seed44_v1` | 44 | 0.16600535812875747 | 0.9004208990365291 | 0.384965547409772 | 0.82687494874531 |

Paper-small 1K mean/stddev:

| Metric | Mean | Stddev | N |
| --- | ---: | ---: | ---: |
| `val_latent_mse` | 0.155409786323 | 0.013130850075 | 3 |
| `val_latent_cosine` | 0.917646185906 | 0.026095423035 | 3 |
| `test_latent_mse` | 0.291214507891 | 0.091692459686 | 3 |
| `test_latent_cosine` | 0.884851853288 | 0.053905889447 | 3 |

## Experiment C - Action-Conditioned Planning

Question:

> Do different candidate career actions produce different predicted next states
> and persisted guard decisions?

Candidate actions:

- skill_gain adjacent skill;
- skill_gain high-demand skill;
- role_move near cluster;
- experience_gain;
- noop.

Evidence:

- persisted hypothetical actions;
- predictions for every candidate;
- guard decisions;
- selected action;
- utility scores;
- decoded plan trace.

## Experiment D - Ablation Study

Minimum ablations:

| ID | Panel | Action Conditioning | Pairwise Features | VICReg |
| --- | --- | --- | --- | --- |
| A0 | taxonomy scalar/categorical only | yes | no | yes |
| A1 | taxonomy + skill vector | yes | no | yes |
| A2 | taxonomy + skill + transition distance | yes | no | yes |
| A3 | A2 | no | no | yes |
| A4 | A2 | yes | occupation/action pair one-hot | yes |
| A5 | A2 | yes | no | no |

Do not include an ablation row unless it is backed by a real run artifact and
source-of-truth readback.

Implemented smoke IDs in `ablation_results.json`:

- `A0_taxonomy_only_action_vicreg`
- `A1_taxonomy_skill_action_vicreg`
- `A2_taxonomy_skill_transition_action_vicreg`
- `A3_taxonomy_skill_transition_no_action_vicreg`
- `A4_taxonomy_skill_transition_pairwise_action_vicreg`
- `A5_taxonomy_skill_transition_action_no_vicreg`

Implementation details:

- A0 removes skill-vector schema fields, adapter mappings, and instruments.
- A2 adds `state.transition_distance` and `transition_distance_scalar`.
- A3 persists `model.predictor.ignore_action=true`; training and inference zero
  the action vector instead of inventing a fallback path.
- A4 adds `state.occupation_move_pair` and
  `occupation_move_pair_onehot` with 20 declared occupation/action pairs.
- A5 sets VICReg variance/covariance loss weights to zero in the persisted
  artifact config.
- Multi-seed paper-small runs vary the main career training seed through
  `--career-training-seed`. Ablations remain controlled on the checked-in seed
  unless `--career-ablation-training-seed` is explicitly supplied.

Current O*NET smoke ablation metrics:

| ID | Val MSE | Val Cosine | Test MSE | Test Cosine |
| --- | ---: | ---: | ---: | ---: |
| A0 | 0.07541437962770774 | 0.9745100258571918 | 0.15218315756551268 | 0.9637773457224168 |
| A1 | 0.17999517062789205 | 0.8776115717741934 | 0.25307116609684355 | 0.867371052118299 |
| A2 | 0.07861760961555968 | 0.9584134219819948 | 0.2269089035423139 | 0.9097942446694102 |
| A3 | 0.35150993421354887 | 0.7115692482591077 | 0.6428963516399238 | 0.5567848258865903 |
| A4 | 0.07653267888245231 | 0.9316788348516672 | 0.11159787261840416 | 0.9196961741053704 |
| A5 | 0.07947118619845824 | 0.91059956073034 | 0.19526649207584376 | 0.8155035850933209 |

## Experiment E - Auditability Stress

Question:

> Does the system fail closed and leave useful diagnostics?

Edge cases:

- missing license manifest;
- missing domain pack;
- corrupt artifact file;
- invalid vector dimension;
- train/test leakage;
- missing observed panel for surprise;
- invalid CF inspection.

Metrics:

- expected typed error code;
- before/after counts;
- audit row count delta;
- whether invalid data rows were avoided;
- root-cause field path present.

## Experiment F - 5090 Feasibility

Question:

> Is the complete run feasible on a single RTX 5090?

Metrics:

- wall time by stage;
- peak VRAM;
- training examples/sec;
- prediction latency;
- plan latency;
- artifact size;
- evidence bundle size.

## Promotion Rules

A smoke run is promoted to paper-small only if:

- all gates pass;
- career benchmark trains and beats random-init on validation;
- all artifact hashes verify;
- no source/provenance gaps exist;
- edge cases match expected deltas;
- paper tables are generated from bundle data.
