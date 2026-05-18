---
namespace: reports
created: 2026-05-10T06:14:42Z
updated: 2026-05-10T06:14:42Z
status: active
tags: phase1b, gpu, wsl, registry, embedders, fsv
---

# Session Report: 2026-05-10 - Phase 1b GPU + Model Registry FSV

## Task
Resolve remaining Phase 1 blockers: WSL GPU access, D: Phase 1b model registry, missing E11/E14/E15-E21 assets, and full-state verification.

## Outcome
Complete.

## Summary
WSL GPU access was verified working after the user rebooted and WSL was updated. `/usr/lib/wsl/lib/nvidia-smi` reports `NVIDIA GeForce RTX 5090`, driver `596.21`, compute capability `12.0`, and the Rust FSV linked `libcuda.so.1` from `/usr/lib/wsl/lib`. The earlier failure was not caused by project code; it was a stale/broken host-managed WSL GPU bridge (`/dev/dxg`/NVML blocked by the OS). Code was still hardened so CUDA name/capability/memory queries now fail closed instead of returning benign-looking placeholder values.

The D: model registry is now materialized at `/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml` with `schema_version = 1`, all E1-E21 registrations, real relative weight files, and manifest SHA-256 pins. Missing assets were added: E11 Kepler official checkpoints, E14 BGE-M3 `pytorch_model.bin`, E15 affect speech, E16 OpenFace, E17 affect text, E18 PaPaGei, E19 WESAD-derived EDA stress head, E20 LaBraM EEG, and E21 EEGPT robust EEG. E19 was first attempted with raw EDA features and failed the S17 holdout gate; the accepted artifact uses real WESAD baseline-normalized features and passes holdout balanced accuracy `0.7251655629139073`.

Full State Verification passed. Source of truth is the D: registry plus D: model files. `embedders_fsv` read the config back from disk, verified 21 digests and 7 learner-state slots, checked RTX 5090 VRAM via CUDA Driver API, loaded the 14 content registrations, and printed expected fail-closed edge cases for invalid embedder id, missing config, digest mismatch, and missing E17 calibration certificate.

## Files Created or Modified in the Repo
- `crates/context-graph-cuda/src/safe/device.rs` - CUDA device name, compute capability, SM count, and memory usage now return explicit `CudaResult` errors.
- `crates/context-graph-cuda/src/context/green_contexts.rs` - Green Contexts now fail closed and query real SM count via CUDA Driver API.
- `crates/context-graph-cuda/src/ffi/cuda_driver.rs` - Added `CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT`.
- `crates/context-graph-mejepa-embedders/src/config.rs` - Enforced non-empty repo/source, non-empty safe relative weight files, no placeholder SHA pins.
- `crates/context-graph-mejepa-embedders/src/vram.rs` - VRAM report now propagates CUDA attribute/name failures.
- `crates/context-graph-mejepa-embedders/examples/materialize_models_config.rs` - New D: registry materializer with identity checks, SHA materialization, readback, and separate evidence path.
- `crates/context-graph-mejepa-embedders/examples/embedders_fsv.rs` - FSV now verifies all 21 registrations, learner-state count, and non-placeholder digest mismatch.
- `crates/context-graph-embeddings/src/provider/multi_array.rs` - E14 BGE-M3 preflight updated to the actual upstream `pytorch_model.bin` artifact.
- `config/models_config.toml.example` - Kept as invalid template-only fixture with placeholders rejected by validation.
- `memory/reports/session-2026-05-10--phase1b-gpu-registry-fsv.md` - This report.

## D: Artifacts Created or Verified
- `/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml`
- `/mnt/d/cabdru/contextgraph/models/kepler/{KEPLERforKE.pt,KEPLERforNLP.pt,dict.txt}`
- `/mnt/d/cabdru/contextgraph/models/bge-m3-dense/pytorch_model.bin`
- `/mnt/d/cabdru/contextgraph/models/learner-state/ppg/{config.json,papagei_s.pt}`
- `/mnt/d/cabdru/contextgraph/models/learner-state/eda/{config.json,wesad_cnn.safetensors}`
- `/mnt/d/cabdru/contextgraph/calibration/wesad/WESAD.zip`
- `/mnt/d/cabdru/contextgraph/calibration/wesad/eda_window_features_v2.npz`

## Evidence
- Materializer evidence: `/tmp/contextgraph-mejepa-models-config-materialization/models-config-materialization.json`, `passes=true`, `digest_verified_count=21`.
- Embedder FSV evidence: `/tmp/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json`, `passes=true`, `phase_ready=true`, `digest_verified_count=21`, `learner_state_verified_count=7`.
- WESAD ZIP: size `2249444501`, SHA1 `bae786bf599ef28e42e1c2e18da9b84d685e84e7`, MD5 `e328980e7992502de30c96ba2a7cb3e2`.
- PaPaGei weight: MD5 `a4cdb32392e2a7b25999128af92813b5` matched Zenodo metadata.
- E19 stress head: SHA256 `b96611681c5225122798744fe158558222140796af731af80cb9dc7f65afd805`.

## Verification Commands
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-embedders --example materialize_models_config`
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-embedders --example embedders_fsv`
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-cuda -- --nocapture`
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mejepa-embedders -- --nocapture`
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-embeddings e14_asset_preflight --lib -- --nocapture`
- `cargo fmt --all --check`

## Decisions Made
- Phase 1b registry source of truth is `/mnt/d/cabdru/contextgraph/models/mejepa_models_config.toml`; legacy `/mnt/d/cabdru/contextgraph/models/models_config.toml` remains non-authoritative.
- CUDA Driver API failures must propagate as errors; no placeholder device names, `(0,0)` compute capability, or `0.0` memory usage.
- E14 BGE-M3 must pin `pytorch_model.bin` because the official BAAI/BGE-M3 snapshot used here does not provide `model.safetensors`.
- E19 is accepted only as a real WESAD-trained artifact with persisted config, safetensors header readback, and held-out subject metrics.

## Discoveries
- NVIDIA WSL CUDA best practice matches this environment: Windows installs the driver; WSL consumes `/usr/lib/wsl/lib/libcuda.so`; installing Linux display drivers in WSL is the wrong direction.
- The official WESAD page now points at sciebo share `HGdUkoNlW1Ub0Gx`; the older `pYjSgfOVs6Ntahr` link returns 404.
- Nomic E5 config identifies as `model_type = "nomic_bert"`, not `bert`; the materializer now checks actual upstream identity.
- The first E19 raw-feature logistic model failed S17 holdout balanced accuracy; baseline-normalized EDA features were required.

## Open Items
- None for the Phase 1b blockers covered here. Phase 1 is ready to proceed from the embedder/GPU/model-registry standpoint.
