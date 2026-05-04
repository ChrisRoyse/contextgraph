# 17 - Paper-Full Scaling Plan

## Goal

Define the budgeted path from `paper_small` (1K events, currently passing) to a
`paper_full` headline run on a single RTX 5090. Smokedoc 02 §Feasible Demo Sizes
sketches the envelope (10K-250K events, 1M-20M params, hours). This doc turns
that into a concrete ladder, VRAM/runtime budget, and promotion gate.

## Why A Ladder, Not A Jump

The paper-small bundle (`20260430T_paper_small_1k_v1`) currently has:

- 78 commands, 169 s wall time, 9 artifact hashes, 11 edge cases, 6 ablations;
- career splits 699 / 200 / 100;
- 15,344 trainable parameters;
- val MSE 0.18 / val cosine 0.88.

A direct jump to 250K events risks: silent OOM, training divergence, ablation
matrix not finishing in time, generated walk graph degenerating. The ladder
catches each failure at the smallest scale that exhibits it.

## Scaling Ladder

| Tier | Events | Walk Coverage | Params | Latent Dim | Batch | Expected Wall | Acceptance |
| --- | ---: | --- | ---: | ---: | ---: | --- | --- |
| smoke | 120 | computer occupations only | 15,344 | 16 | 16 | ~3 min | A0-A5 finish, all FSV passes |
| paper_small_1k | 1,000 | computer occupations | 15,344 | 16 | 32 | ~15 min | val cosine ≥ 0.85 (current 0.88) |
| paper_med_5k | 5,000 | + STEM cluster | ~50K | 32 | 64 | ~45 min | val cosine ≥ 0.88, no collapse |
| paper_med_10k | 10,000 | + STEM + healthcare | ~150K | 64 | 128 | ~2 h | top-1 occ acc ≥ random-init+15pp |
| paper_full_50k | 50,000 | full O*NET 30.2 | ~1M | 96 | 256 | ~6 h | top-1 occ acc ≥ random-init+25pp, ECE ≤ 0.10 |
| paper_full_250k | 250,000 | full O*NET + ESCO crosswalk | ~5M | 128 | 384 | ~24 h | optional headline; only if 50k succeeds |

Each tier is a strict prerequisite for the next. A failure at tier T preserves
the bundle and stops the ladder; doc 14 §Anti-Goals forbids skipping a tier to
chase the headline.

## VRAM Budget On 32 GB

Approximate budget (conservative, based on candle 0.10.2 + sm_120a):

| Component | smoke | 1K | 10K | 50K | 250K |
| ---: | ---: | ---: | ---: | ---: | ---: |
| Encoder activations | 0.1 GB | 0.1 GB | 0.4 GB | 1.2 GB | 3.5 GB |
| Predictor activations | 0.1 GB | 0.1 GB | 0.4 GB | 1.2 GB | 3.5 GB |
| Optimizer state (AdamW, fp32) | 0.05 GB | 0.05 GB | 0.6 GB | 4 GB | 20 GB |
| Activations (batch × dim) | 0.05 GB | 0.1 GB | 0.5 GB | 2 GB | 6 GB |
| RocksDB read buffers | 0.1 GB | 0.1 GB | 0.2 GB | 0.4 GB | 0.8 GB |
| Headroom | rest | rest | rest | ~24 GB | ~−1.8 GB ⚠️ |

The 250K row exceeds 32 GB headroom under fp32 AdamW. Mitigations, in order:

1. Use bfloat16 master parameters with fp32 accumulator (halves optimizer
   state); confirm candle path supports it.
2. Use 8-bit AdamW (bitsandbytes-style) if fp32 is required for stability;
   confirm available in candle.
3. Drop latent dim from 128 → 96.
4. Drop batch from 384 → 256.

If none restore headroom, **drop the 250K tier**. The paper headline survives
on 50K. The 250K row is explicitly optional.

## Runtime Budget

Runtime is wall-time, not GPU-time. Budget breakdown for paper_full_50k:

- O*NET walk generation: ~5 min (single-threaded, deterministic).
- Ingest + adapter + normalized states (50K events): ~12 min.
- Panel materialization (10 instruments × 50K): ~10 min.
- Trajectory + dataset compile: ~6 min.
- JEPA training (12 epochs, batch 256): ~3 h.
- Decoder heads (2 heads, 30 epochs each): ~30 min total.
- Prediction + plan + surprise: ~10 min.
- A0-A5 ablations (smaller; reuse main JEPA when possible): ~1.5 h.
- FSV readback + table generation: ~5 min.

Total: ~6 h. Plan the run for an overnight slot. Multi-seed × 5 = 30 h. With
ablation-stability (2 seeds × 6 ablations × shorter runs) ≈ +6 h. Total
**~36 h** for the full multi-seed paper_full_50k.

A 250K run is **single-seed** in the paper. Do not promise multi-seed at 250K.
Disclose this explicitly in the methods section.

## Walk Graph Scaling

The current generator restricts the smoke walk to "core computer occupations"
starting at SOC `15-1252.00`. At 5K+ events, the walk degenerates if the
restriction is kept. Required changes:

- 5K tier: add the full O*NET 11/15-cluster STEM bracket (≈300 occupations).
- 10K tier: add healthcare (29-*) and education (25-*) for diversity.
- 50K tier: full O*NET 30.2 occupation graph (~1,016 nodes, all edges).
- 250K tier: O*NET + ESCO crosswalk (doc 18 dependency).

Each tier records its walk coverage in `data_manifest.json` under
`walk_clusters` and `walk_node_count`. A change in coverage is a typed schema
change in the bundle, not a silent expansion.

## Determinism

A larger walk graph implies more random choices per event. The walk generator
must:

- accept a `--walk-seed` parameter (already supported via the per-event seed
  derived from `--career-training-seed`);
- record the walk seed in `data_manifest.json`;
- reject any run that mixes a walk seed across tiers without explicit
  `--allow-tier-mismatch` (which itself is recorded).

Determinism check: re-run the same seed at the same tier. The generated walk
SHA-256 must equal byte-for-byte. This is the existing same-seed reproducibility
test, just exercised at the new tier.

## Failure Modes At Scale

| Failure | Detection | Mitigation |
| --- | --- | --- |
| OOM during training | candle returns CUDA OOM | drop batch, then latent dim, then re-budget |
| Collapse (latent norm → 0) | doc 22 collapse diagnostics | tighten VICReg variance weight; re-train |
| Walk graph disconnected | reachable node count < expected | expand cluster set; re-record manifest |
| Single epoch > 1h | stage timer | drop epochs; report final epoch + curve |
| RocksDB compaction stall during ablations | inspect-counts hangs | run ablations against separate per-ablation DB (already supported) |
| Decoder head overfits | val accuracy gap > 0.15 train | label smoothing already on; add weight decay |
| Multi-seed cost overruns | wall time forecast | drop the optional 250K row; keep 50K headline |

## New Acceptance Gate

Smokedoc 04 gains §Gate 11 (Tier Promotion):

> **Gate 11 — Tier Promotion**
>
> A bundle is promoted from tier T to T+1 only if:
> - tier T multi-seed aggregate (doc 15) exists and passes;
> - tier T peak VRAM stays under the budget for T+1 minus 4 GB headroom;
> - tier T runtime ≤ projected T+1 runtime / scaling factor;
> - no doc 22 collapse diagnostic crossed its alarm threshold;
> - operator confirms the next tier wall-time fits the budget.

## CLI Additions

```bash
target/debug/context-graph dynamicjepa research-smoke \
  --run-id "$RUN_ID" \
  --purpose paper_med_5k \
  --career-event-count 5000 \
  --walk-clusters "computer,stem" \
  --career-training-seed 42 \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --json
```

`--walk-clusters` is a comma-separated allowlist tied to the manifest field of
the same name. Unknown cluster names fail closed with `VALIDATION` error.

## Out Of Scope For This Doc

- ESCO data ingestion: covered in doc 18.
- Multi-step planning at scale: covered in doc 24.
- Distributed multi-GPU: explicitly out (single-RTX-5090 is the paper claim).

## Acceptance

This work is done when:

- the ladder is implemented as `--purpose` enum values in the CLI;
- `paper_med_5k` and `paper_med_10k` bundles exist and pass FSV;
- `paper_full_50k` bundle exists and passes (multi-seed × 5 acceptable; single
  seed disclosed if multi-seed budget overruns);
- doc 13 is updated with the new headline numbers.
