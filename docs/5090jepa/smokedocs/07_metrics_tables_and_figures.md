# 07 - Metrics, Tables, And Figures

The paper should be generated from evidence files, not hand-maintained numbers.

## Core Metrics

Prediction:

- latent MSE by split;
- latent cosine by split;
- action-contrast accuracy;
- top-k next occupation cluster accuracy;
- skill-vector cosine;
- shuffled-target cosine;
- random-init cosine.

Collapse diagnostics:

- VICReg variance per dimension minimum;
- covariance off-diagonal Frobenius norm;
- latent norm distribution;
- pairwise latent distance distribution.

Runtime:

- ingest events/sec;
- panels/sec;
- dataset rows/sec;
- training examples/sec;
- peak VRAM;
- artifact size;
- prediction latency;
- planning latency.

Audit:

- number of CFs touched;
- count deltas by operation;
- verification row count;
- artifact hash checks;
- edge-case pass rate;
- decoded readback coverage.

## Required Tables

### Table 1 - Domain Runtime Coverage

Columns:

- domain;
- events;
- panels;
- trajectories;
- dataset rows;
- training runs;
- artifacts;
- predictions;
- plans;
- surprises;
- verification status.

### Table 2 - Prediction Metrics

Columns:

- domain;
- config ID;
- train rows;
- val rows;
- test rows;
- val MSE;
- val cosine;
- test MSE;
- test cosine;
- action contrast;
- random-init cosine;
- shuffled-target cosine.

### Table 3 - Career Ablations

Columns:

- ablation ID;
- panel slots;
- input dim;
- latent dim;
- params;
- val MSE;
- val cosine;
- top-k occupation;
- skill cosine;
- wall time;
- peak VRAM.

### Table 4 - Auditability And Failure Behavior

Columns:

- edge case;
- expected error;
- raw delta;
- adapter delta;
- normalized delta;
- audit delta;
- verification result;
- root-cause field path.

### Table 5 - Single-GPU Feasibility

Columns:

- run tier;
- events;
- rows;
- params;
- train time;
- total time;
- peak VRAM;
- artifact size;
- evidence size.

## Required Figures

- System architecture diagram: domain pack -> storage -> panels -> JEPA ->
  planning -> verification.
- Smoke ladder flowchart.
- Validation loss curve.
- Latent cosine/MSE by epoch.
- Ablation bar chart.
- Career taxonomy transition graph sample.
- Source-of-truth evidence diagram showing DB rows and artifact hashes.

## Statistical Reporting

For smoke runs, one seed is enough only to catch integration errors.

For paper tables:

- run at least 3 seeds for the career taxonomy benchmark if runtime allows;
- report mean and standard deviation;
- keep each seed's full bundle;
- do not average away failed runs.

## Failure Reporting

Failed runs are useful if they reveal strict failure behavior. Include an
appendix table of intentional failures and their typed error codes.

