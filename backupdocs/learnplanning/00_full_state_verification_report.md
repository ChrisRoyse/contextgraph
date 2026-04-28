# Full State Verification Report - E14 / 14-Embedder System

Date: 2026-04-27

## Scope

This verification audited the Context Graph runtime after adding E14 BGE-M3 Dense. The check covered:

- 14 production embedder shape contracts.
- Real E14 model load/embed path.
- RocksDB quantized E14 column family persistence.
- HNSW E14 indexing/search.
- Retrieval masks, weighting, graph-link score arrays, MCP JSON/tool surfaces.
- Training export record shape.
- Constellation/topic/cross-correlation derived state.
- CUDA 13.2 / RTX 5090 build/runtime state.

## Source Of Truth

- E14 model snapshot: `models/bge-m3-dense/`.
- Real E14 output: `BgeM3DenseModel::embed()` output vector.
- Quantized embedding persistence: RocksDB column family `emb_13`.
- Whole quantized fingerprint persistence: RocksDB quantized fingerprint store, expected 14 embedder entries.
- HNSW source of truth: `EmbedderIndex::E14BgeM3Dense` registry/index.
- Training records: RocksDB `CF_TRAINING_RECORDS`, `TrainingRecord` version 3.
- Constellations: RocksDB `CF_CONSTELLATIONS`, `CONSTELLATION_VERSION = 2`.
- CLI/MCP wire surfaces: JSON schemas, DTOs, `AllEmbedderScores.e14`, `EmbedderId::E14`.
- CUDA build source of truth: Cargo build log invoking `/usr/local/cuda-13.2/bin/nvcc` with `-arch compute_120a`.

## Fixes Made During FSV

- Added/validated E14 across fingerprint validation, equality, storage size, `is_complete()`, and `get()` paths.
- Added E14 to query masks, dense-space masks, in-memory executor, search routing, graph-link validation, typed-edge validation, MCP DTOs, and search transparency.
- Corrected weighted agreement denominator from 8.5 to 9.5 with E14 as a semantic signal.
- Corrected training export to version 3 with `e14_bge_m3_dense` and 91 cross-correlations.
- Fixed constellation cross-correlation centroid from stale 78 slots to 91 slots and bumped `CONSTELLATION_VERSION` to 2.
- Fixed provider readiness so `is_ready()` and `health_status()` check the actual E14 adapter instead of treating E14 as implicitly healthy.
- Updated CUDA discovery to prefer `CUDA_PATH` / `CUDA_HOME` and CUDA 13.2 before PATH.
- Updated `.cargo/config.toml` locally for CUDA 13.2, `compute_120a`, and RTX 5090-oriented NVCC flags.

## Evidence Log

Real GPU/runtime state:

```text
GPU: NVIDIA GeForce RTX 5090
Driver: 596.21
VRAM: 32607 MiB total, 26716 MiB free after verification
Compute capability: 12.0
Power limit: 575 W
nvcc: CUDA compilation tools release 13.2, V13.2.51
/usr/local/cuda -> /usr/local/cuda-13.2
```

Real E14 model output from local BGE-M3 snapshot:

```text
models/bge-m3-dense: 4.3G
E14 real embed output: len=1024, norm=1.000000,
latency_us=104432,
first5=[-0.03992623, 0.036425337, -0.060839016, -0.004773454, -0.050047856]
```

RocksDB E14 physical persistence:

```text
Source of Truth: RocksDB CF emb_13
Input bytes: [0, 37, 74, 111, 148, 185, 222, 3]
Stored id: fe14f5fe-0014-dead-beef-cafebabe1014
Read after close+reopen:
  method=PQ8
  original_dim=1024
  data=[0, 37, 74, 111, 148, 185, 222, 3]
  loaded_full.embeddings.len()=14
Delete check: load after delete returned missing.
```

Training-record physical persistence:

```text
Source of Truth: CF_TRAINING_RECORDS
BEFORE store_training_record: count=0, e14_len=8, cross_len=91
AFTER store_training_record: count=1
AFTER get_training_record:
  e14_len=8
  e14_values=[0.14, 0.14, 0.14, 0.14, 0.14, 0.14, 0.14, 0.14]
  cross_len=91
```

Live production store:

```text
contextgraph_data size: 2.3M
backfill-e14 --dry-run:
  0 seen, 0 already had E14, 0 backfilled, 0 skipped
```

Interpretation: the live store is healthy/openable but currently has no stored fingerprints. Synthetic FSV stores supplied the physical write/read/delete proof.

## Manual Edge Cases

1. Legacy JSON missing E14:
   - Before: no `e14_bge_m3_dense` key.
   - After: restored E14 vector length is 0, no panic.

2. Empty E14 quantization:
   - Before: `fp.e14_bge_m3_dense.is_empty() = true`.
   - After: quantized/dequantized E14 remains empty, preserving legacy safety.

3. Invalid E14 out-of-bounds index:
   - Trigger: `load_embedder(id, 14)`.
   - Outcome: fail-fast panic with valid range `0-13`.

4. HNSW wrong dimension:
   - Trigger: E14 search with non-1024D query.
   - Outcome: rejected without padding/truncation.

5. Backfill limit and missing storage:
   - `--limit 1` processed exactly 1.
   - missing `--storage` returned clap exit 2.
   - nonexistent storage returned exit 1 with a clear error.

## Verification Commands

All commands completed successfully:

```bash
scripts/check-embedder-count.sh
cargo check -p context-graph-core --lib
cargo check -p context-graph-embeddings --lib
cargo check -p context-graph-mcp --lib
cargo test -p context-graph-core --lib -- --test-threads=4
cargo test -p context-graph-storage --lib -- --test-threads=4
cargo test -p context-graph-mcp --lib -- --test-threads=4
cargo test -p context-graph-embeddings --lib -- --test-threads=4
cargo test -p context-graph-storage --test e14_fsv -- --nocapture --test-threads=1
cargo test -p context-graph-cli --test e14_phase_b_full_state_verification -- --nocapture --test-threads=1
cargo test -p context-graph-cli --test e14_optimization_fsv -- --nocapture --test-threads=1
cargo test -p context-graph-storage --test training_records_integration -- --test-threads=1
cargo test -p context-graph-embeddings test_embed_real_text_produces_normalised_1024d_vector -- --ignored --nocapture --test-threads=1
target/debug/context-graph-cli backfill-e14 --storage contextgraph_data --dry-run --scan-batch 2000 --log-every 1000
```

Pass counts:

- Core lib: 2042 passed.
- Storage lib: 501 passed, 1 ignored.
- MCP lib: 340 passed.
- Embeddings lib: 1148 passed, 2 ignored.
- Storage E14 FSV: 14 passed.
- CLI E14 Phase B FSV: 12 passed.
- CLI E14 optimization FSV: 10 passed.
- Training records integration: 15 passed.
- Real E14 embed ignored test: 1 passed when run explicitly.

## Residual Risks

- `.cargo/config.toml` is currently local/untracked in this repository. The CUDA 13.2 settings are active locally but should be promoted intentionally if this environment config is meant to be shared.
- `CUDARC_CUDA_VERSION` remains pinned to `13000` because the cudarc versions in this workspace do not yet advertise CUDA 13.2. Native PTX builds still use CUDA 13.2.
- The old specs under `docs2/contextgraphspecs/` still contain stale 13-embedder / CUDA 13.1 language. This report and the new `docs/learnplanning/` docs reflect the current verified state.
- The live production RocksDB has zero fingerprints, so real user-data backfill was not exercised. Synthetic RocksDB FSV did exercise physical writes, reopen reads, and deletes.

