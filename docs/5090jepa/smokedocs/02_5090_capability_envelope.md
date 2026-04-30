# 02 - RTX 5090 Capability Envelope

This demo is designed for the operator's RTX 5090 with 32GB VRAM. The goal is
not to maximize model size; it is to maximize evidence quality and novelty per
unit of compute.

## Hardware Assumptions

| Component | Expected |
| --- | --- |
| GPU | NVIDIA RTX 5090 |
| VRAM | 32GB class |
| CUDA | 13.2 |
| Compute capability | 12.0 / `CUDA_COMPUTE_CAP=120` |
| Rust | Stable workspace toolchain |
| Trainer | Candle CUDA |

Observed on 2026-04-30:

| Component | Observed |
| --- | --- |
| GPU | NVIDIA GeForce RTX 5090 |
| Driver | 596.21 |
| VRAM | 32607 MiB |
| CUDA | 13.2, V13.2.78 |
| Compute capability | 12.0 / `CUDA_COMPUTE_CAP=120` |
| Rust | rustc 1.95.0 |
| Cargo | cargo 1.95.0 |
| Kernel | Linux 6.6.87.2-microsoft-standard-WSL2 |
| System RAM | 110GiB visible to WSL2 |

The smoke harness must record actual `nvidia-smi`, `nvcc`, `rustc`, `cargo`,
and `cargo tree -p candle-core` outputs. If the observed environment differs,
the run still proceeds only if the variance is explicitly recorded and the CUDA
trainer passes.

## Feasible Demo Sizes

| Tier | Purpose | Rows | Model Size | Expected Runtime |
| --- | --- | ---: | ---: | ---: |
| Smoke | catch integration bugs | 120 O*NET-derived career events | <100K params | minutes |
| Paper small | first real tables | 1K-10K events | <1M params | <1 hour |
| Paper full 5090 | main result | 10K-250K events if available | 1M-20M params | hours |

The full 5090 run should stay well below 32GB VRAM. Prefer richer ablations and
evidence capture over a larger model that obscures failure analysis.

## Runtime Components To Measure

Each run records:

- ingest throughput: events/sec;
- panel materialization throughput: panels/sec;
- dataset compilation throughput: rows/sec;
- training throughput: examples/sec;
- peak VRAM;
- model artifact size;
- prediction latency;
- plan latency per candidate action;
- evidence bundle size.

## Model Envelope

The Phase 10 runtime already proves:

- counter_world tiny model: 480 trainable parameters;
- gridworld model: <20K trainable parameters;
- artifact hash verification works for both.

The career-taxonomy smoke should start with:

- panel input dimension: keep under 256 dense dimensions;
- latent dimension: 16-64;
- predictor: 1-3 MLP layers;
- trainable params: <250K for smoke, <5M for paper small;
- batch size: tune to fit under 8GB VRAM first, then scale.

Current O*NET smoke result:

- career panel dimensions: 30 input, 30 target, 5 action;
- career trainable params: 15344;
- career splits: 83 train, 24 validation, 12 test;
- complete research-smoke wall time: 210385 ms;
- command count: 78;
- artifact hash rows: 9;
- edge-case rows: 11;
- ablation rows: 6.

## What Not To Spend GPU On Yet

- Large text encoders inside the ME-JEPA trainer.
- Full E1-E14 + Erec1-Erec5 dense copies until the public-taxonomy smoke passes.
- Product heads before the foundation predictor artifact is verified.
- Multi-step rollouts before one-step action-conditioned prediction is stable.
