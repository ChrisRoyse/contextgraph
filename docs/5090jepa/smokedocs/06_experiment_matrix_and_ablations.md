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
- top-k next occupation cluster accuracy;
- skill-vector cosine;
- action-contrast accuracy;
- random-init baseline;
- shuffled-target baseline;
- VICReg variance/covariance diagnostics.

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
| A4 | A2 | yes | yes, if implemented | yes |
| A5 | A2 | yes | no | no, if config-supported |

Do not include an ablation row unless it is backed by a real run artifact and
source-of-truth readback.

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

