# Source Of Truth And State Map

## E14 Model State

Source of truth:

- Filesystem: `models/bge-m3-dense/`
- Runtime: `BgeM3DenseModel::model_state`
- Output: `ModelEmbedding { model_id: BgeM3Dense, vector, latency_us }`

Verified state:

- Required files present: `model.safetensors`, `tokenizer.json`, `config.json`.
- Snapshot size: 4.3G.
- Real embedding output: 1024 floats, L2 norm 1.000000.

## Fingerprint State

Source of truth:

- `SemanticFingerprint.e14_bge_m3_dense`
- `SemanticFingerprint::get(13)`
- `EmbeddingSpaceMask` bit 13
- `NUM_EMBEDDERS = 14`

Verified state:

- E14 aliases parse (`E14`, `bge_m3_dense`, `bgem3dense`).
- Empty E14 is permitted only for legacy records.
- Non-empty E14 must be 1024D.
- `is_complete()` requires a valid 1024D E14 vector.

## Quantized Storage State

Source of truth:

- RocksDB quantized embedder column families `emb_0` through `emb_13`.
- E14 physical CF: `emb_13`.

Verified state:

- Exactly 14 quantized embedder CFs exist by convention.
- `load_embedder(id, 13)` returns E14 bytes.
- `load_embedder(id, 14)` fails fast.
- Whole fingerprint load returns 14 embeddings.

## Retrieval State

Source of truth:

- `EmbeddingSpaceMask::ALL`
- `EmbeddingSpaceMask::ALL_DENSE`
- HNSW registry `EmbedderIndex::E14BgeM3Dense`
- RocksDB search routing in `get_query_vector_for_embedder(13)`

Verified state:

- E14 participates in dense routing.
- E6/E12/E13 remain non-dense/non-HNSW special cases.
- E14 HNSW self-query returns the inserted id first.
- Wrong dimension queries are rejected.

## Graph-Link State

Source of truth:

- Embedder ids valid range `0..NUM_EMBEDDERS`.
- Per-edge `embedder_scores: [f32; NUM_EMBEDDERS]`.
- MCP graph-link DTO parsing and display names.

Verified state:

- Index 13 is valid.
- Index 14 is rejected.
- E14 is exposed to MCP tools as `E14 (V_multilingual_style)`.

## Training State

Source of truth:

- `TrainingRecord` version byte: `TRAINING_RECORD_VERSION = 3`.
- RocksDB `CF_TRAINING_RECORDS`.
- `NUM_CROSS_CORRELATIONS = C(14,2) = 91`.

Verified state:

- Records persist `e14_bge_m3_dense`.
- Topic profile is 14D.
- KNN neighbor outer vector is 14 slots.
- Cross-correlations persist as 91 values.

## Constellation State

Source of truth:

- `CONSTELLATION_VERSION = 2`.
- RocksDB `CF_CONSTELLATIONS`.
- `CROSS_CORRELATION_CENTROID_DIM = C(14,2) = 91`.

Verified state:

- `per_embedder[13]` is populated for E14.
- E14 centroid is observed via dense path.
- The previous 78-slot mismatch was fixed.

## CUDA State

Source of truth:

- `/usr/local/cuda -> /usr/local/cuda-13.2`
- `CUDA_HOME=/usr/local/cuda-13.2`
- Cargo build log for `context-graph-cuda`
- `nvidia-smi`

Verified state:

- `nvcc` release 13.2, V13.2.51.
- Build uses `/usr/local/cuda-13.2/bin/nvcc`.
- PTX target is `compute_120a`.
- GPU is RTX 5090, compute capability 12.0.

