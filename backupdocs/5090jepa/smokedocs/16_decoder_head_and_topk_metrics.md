# 16 - Decoder Head And Top-K Metrics

## Goal

Lift the current latent-only career predictor into a model that emits **top-k
occupation cluster** and **skill-vector reconstruction** metrics with full FSV
evidence. Smokedoc 07 §Core Metrics explicitly disclaims these today: *"top-k
occupation and skill-vector cosine require a decoder/head that is not part of
the current latent-only JEPA smoke. Do not claim those metrics until that head
exists and is verified."* This doc lifts that disclaimer.

## Why A Decoder Head At All

JEPA-style predictors live in a learned latent space. The latent MSE / cosine
numbers in `table_prediction_metrics.csv` are the right primary metric, but
they are not interpretable to a paper reader as "did we predict the right
occupation?" or "did we recover the next skill profile?". For paper readers:

- **Top-k occupation cluster accuracy** is the obvious next-state classifier
  metric.
- **Skill-vector reconstruction cosine** is the obvious continuous next-state
  metric on a recognizable feature space.

These are decoder heads added on top of the **frozen** JEPA target encoder
output, not changes to the JEPA training objective. Apple's "Rethinking JEPA"
shows the frozen-teacher pattern is compute-efficient and matches our envelope.

## Architecture

Two parallel heads, both small, both trained **after** the JEPA predictor is
frozen:

### Head H1 — Occupation Cluster Classifier

Input: predicted next-state latent $\hat{z}_{t+1}$ from the JEPA predictor
(trained, frozen).

Output: softmax over the K occupation clusters declared in
`career_taxonomy.v1.toml`.

Layers: Linear($d_{latent}$, 64) → GELU → Linear(64, K). No more.

Loss: cross-entropy with label smoothing 0.1 (mitigates the small-N overfit on
the 1K paper-small split).

### Head H2 — Skill-Vector Regressor

Input: same $\hat{z}_{t+1}$.

Output: skill vector of dimension $S$ matching the panel target.

Layers: Linear($d_{latent}$, 64) → GELU → Linear(64, S).

Loss: cosine + 0.1 × MSE. Cosine because the paper claim is on direction;
small MSE keeps norms from drifting.

### Why Two Heads, Not One Multi-Task Loss

Separation makes the FSV story cleaner:

- H1 metrics live in `table_topk_occupation.csv`;
- H2 metrics live in `table_skill_reconstruction.csv`;
- failures are localized;
- ablating H1 does not require retraining H2.

## Training Recipe

Run **after** the JEPA stage, with the JEPA encoder + predictor frozen
(`requires_grad=false`). New CLI subcommand:

```bash
target/debug/context-graph dynamicjepa train-decoder \
  --db "$DB" \
  --domain career_taxonomy \
  --jepa-artifact-id "$JEPA_ARTIFACT_ID" \
  --head occupation \
  --config configs/dynamicjepa/decoder/career_taxonomy_occupation.json \
  --artifact-root "$ARTIFACT_ROOT" \
  --json
```

Repeat for `--head skill_vector`. Each invocation:

1. Loads the JEPA artifact, asserts hash equality.
2. Freezes encoder + predictor.
3. Trains the head on the same train/val/test split as the JEPA run.
4. Records a new artifact in `dj_model_artifacts` linked to the JEPA artifact.
5. Writes per-epoch metrics to `dj_training_runs`.

Keep epochs small (≤30); heads should converge fast on top of frozen latents.
Peak VRAM should stay under 1 GB per head at paper-small.

## New Storage Schema

No new RocksDB CFs. Reuse `dj_model_artifacts` and `dj_training_runs` with a
new field on the artifact row:

```json
{
  "artifact_kind": "decoder_head",
  "head_kind": "occupation_cluster" | "skill_vector",
  "parent_jepa_artifact_id": "<uuid>",
  "parent_jepa_artifact_sha256": "<hex>"
}
```

The verifier asserts `parent_jepa_artifact_sha256` equals the recomputed JEPA
artifact hash. Mismatch is a typed `ARTIFACT_HASH_MISMATCH` error.

## New Metrics

Added to `evaluation_report.json` and exported to two new tables:

### `table_topk_occupation.csv`

| domain | run_id | split | k=1 acc | k=3 acc | k=5 acc | macro F1 | calibration ECE |
|---|---|---|---|---|---|---|---|

### `table_skill_reconstruction.csv`

| domain | run_id | split | cosine | MSE | rank correlation | per-skill MAE p50 | per-skill MAE p95 |
|---|---|---|---|---|---|---|---|

Both tables include random-init and shuffled-target baseline rows by training
the same heads on a randomly initialized JEPA latent and on shuffled targets,
respectively. This matches the existing baseline discipline in doc 07.

## Decoder Calibration Diagnostic

Top-k accuracy is paper-headline; **calibration** is the appendix. Compute
expected calibration error (ECE) with 15 bins on the val split. Report ECE in
the same table; flag any value above 0.10 as a discussion point.

Reasoning: a 5090 demo paper that claims high top-k without calibration will
draw reviewer skepticism on small-N benchmarks. ECE is cheap, deterministic,
and verifiable from persisted prediction rows.

## FSV Additions

Smokedoc 08 §Stage Sources Of Truth gains rows:

| Stage | Source Of Truth |
| --- | --- |
| Decoder head training | `dj_training_runs` (decoder_head kind), `dj_model_artifacts` (decoder_head kind) |
| Decoder head evaluation | `evaluation_report.json` (`decoder_heads` block), `table_topk_*.csv`, `table_skill_*.csv` |

Smokedoc 04 §Gate 7 (Prediction, Planning, Surprise) gains a Gate 7b:

> **Gate 7b — Decoder Heads**
>
> Trigger: `dynamicjepa train-decoder --head occupation`, then `--head
> skill_vector`.
>
> Pass criteria:
> - both head artifacts have hash-verifiable parent JEPA links;
> - `evaluation_report.json` contains both head blocks;
> - both top-k and skill-reconstruction tables have rows for train/val/test +
>   random-init + shuffled-target baselines;
> - ECE recorded.

## Promotion Rules Update

Smokedoc 06 §Promotion Rules adds:

- top-1 occupation accuracy must beat random-init by paired BCa CI excluding
  zero (per doc 15);
- skill-reconstruction cosine must beat shuffled-target by paired BCa CI
  excluding zero;
- ECE not used as a hard gate, only a discussion item.

## Edge Cases (Fail-Closed)

- `--jepa-artifact-id` not found: typed `ARTIFACT_NOT_FOUND` error.
- JEPA artifact hash mismatch on load: typed `ARTIFACT_HASH_MISMATCH`.
- Train/val/test split differs between JEPA artifact and decoder run: typed
  `SPLIT_DRIFT` error.
- Class count K does not match domain pack declared K: typed
  `SCHEMA_MISMATCH`.
- Skill-vector dimension does not match panel target dimension: typed
  `SCHEMA_MISMATCH`.

## Out Of Scope For This Doc

- Generative decoders (e.g. occupation title generation): explicitly out, per
  smokedoc 02 §What Not To Spend GPU On Yet.
- Decoder heads on counter_world / gridworld: not needed, those domains do not
  have a paper-cited top-k claim.
- Joint multi-task training: not needed and would entangle FSV across heads.

## Acceptance

This work is done when:

- `dynamicjepa train-decoder` exists with both head modes;
- the integration test trains both heads against a real JEPA artifact;
- `paper_tables/table_topk_occupation.csv` and
  `paper_tables/table_skill_reconstruction.csv` exist in every new paper-small
  bundle;
- the multi-seed aggregator (doc 15) handles both new tables;
- doc 13 is updated to remove the "top-k disclaimed" note and replace it with
  the new evidence row counts.
