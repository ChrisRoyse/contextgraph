# Full 14-Embedder Redo FSV

Date: 2026-04-27

## Scope

Redo verification covered all 14 active production slots, all 91 pairwise
cross-correlations, group/weight/topic-profile calculations, E14 real inference,
MCP/CLI surfaces, storage readback, and live CUDA/store state.

## Source Of Truth

- Active slot order: `ModelId::production()` and `Embedder::from_index(0..13)`.
- Physical vector storage: RocksDB column families `emb_0` through `emb_13`.
- Physical topic profile storage: RocksDB column family `topic_profiles`, 56 bytes
  per row.
- Correlation calculations: `compute_cross_correlations()` output vector, ordered
  `(0,1) ... (0,13), (1,2) ... (12,13)`.
- Group calculations: `compute_group_alignments()` and `GroupAlignments`.
- Topic weights: `category_for()` and `MAX_WEIGHTED_AGREEMENT`.
- Real E14 inference: `BgeM3DenseModel::embed()` with the local model snapshot.
- CUDA runtime/build state: `nvidia-smi`, `nvcc --version`, and Cargo CUDA build
  logs.

## Fixes Made During This Redo

- Corrected `ModelId::production()` order so production slot 10 is Kepler/E11,
  followed by E12 LateInteraction, E13 Splade, and E14 BgeM3Dense.
- Changed quantized method validation to use `ModelId::production()` instead of
  raw enum discriminants, avoiding legacy `Entity` shifting E14 out of slot 13.
- Made `store_quantized_fingerprint()` write `topic_profiles` in the same atomic
  batch as `emb_0..emb_13`; `delete_quantized_fingerprint()` now clears that row.
- Aligned core embedder config with active storage methods for E9 Binary,
  E11 Kepler PQ8, and E12 TokenPruning.
- Included E14 in `QuantizedSemanticFingerprint::estimated_size_bytes()`.
- Added `crates/context-graph-storage/tests/full_14_embedder_fsv.rs`.
- Hardened `scripts/check-embedder-count.sh` to verify exact production slot
  order without over-reading the next function.

## Physical Evidence

Synthetic RocksDB FSV wrote one fingerprint and reopened the DB before reading:

```text
loaded_full.embeddings.len() = 14
topic_profiles raw bytes = 56
topic_profile = [0.01, 0.02, ..., 0.14]
```

Every embedder CF was read from RocksDB after reopen:

```text
emb_0  E1  PQ8           dim=1024  data_len=8
emb_1  E2  Float8E4M3    dim=512   data_len=512
emb_2  E3  Float8E4M3    dim=512   data_len=512
emb_3  E4  Float8E4M3    dim=512   data_len=512
emb_4  E5  PQ8           dim=768   data_len=8
emb_5  E6  SparseNative  dim=30522 data_len=32
emb_6  E7  PQ8           dim=1536  data_len=8
emb_7  E8  Float8E4M3    dim=1024  data_len=1024
emb_8  E9  Binary        dim=10000 data_len=1250
emb_9  E10 PQ8           dim=768   data_len=8
emb_10 E11 PQ8           dim=768   data_len=8
emb_11 E12 TokenPruning  dim=128   data_len=64
emb_12 E13 SparseNative  dim=30522 data_len=32
emb_13 E14 PQ8           dim=1024  data_len=8
```

Delete verification read every slot again and confirmed all 14 returned missing;
the `topic_profiles` row was also absent.

## Manual Edge Cases

- Missing slot: constructing a `StoredQuantizedFingerprint` with 13 embeddings
  panicked with the missing index.
- Invalid slot: `load_embedder(id, 14)` panicked with valid range `0-13`.
- Delete path: all 14 CF reads returned missing and `topic_profiles` was gone.
- Correlation zero profile: all 91 outputs were exactly zero.
- Correlation clamp: `2.0` clamped to `1.0`, `-1.0` clamped to `0.0`.
- Invalid synergy index: `get_synergy(14, 0)` panicked.
- Topic fallback: complete unit fingerprint produced fourteen `1.0` slots;
  empty fingerprint produced fourteen `0.0` slots; NaN input produced `0.0`.

## Calculation Evidence

- Cross-correlation count: `91`.
- First pair: `E1xE2 = 0.05 * 0.10 * 0.6 = 0.0030000003`.
- Last pair: `E13xE14 = 0.65 * 0.70 * 0.6 = 0.273`.
- Group result for profile `[1..14]`:
  `[10.0, 2.5, 5.5, 7.3333335, 10.5, 6.0]`.
- Category counts:
  `Semantic=8`, `Temporal=3`, `Relational=2`, `Structural=1`.
- Weighted agreement sum:
  `9.5`, matching `max_weighted_agreement()` and `MAX_WEIGHTED_AGREEMENT`.

## Runtime Evidence

```text
GPU: NVIDIA GeForce RTX 5090
Driver: 596.21
VRAM: 32607 MiB
Compute capability: 12.0
nvcc: CUDA compilation tools release 13.2, V13.2.51
Cargo CUDA build: /usr/local/cuda-13.2/bin/nvcc -arch compute_120a
```

Real E14 model inference:

```text
E14 real embed output:
len=1024
norm=1.000000
latency_us=180276
first5=[-0.03992623, 0.036425337, -0.060839016, -0.004773454, -0.050047856]
```

Live store dry run:

```text
contextgraph_data backfill-e14 --dry-run:
0 seen, 0 already had E14, 0 backfilled, 0 skipped (missing content)
```

## Test Results

```text
scripts/check-embedder-count.sh                                      PASS
cargo test -p context-graph-storage --test full_14_embedder_fsv       4 passed
cargo test -p context-graph-storage --test e14_fsv                    14 passed
cargo test -p context-graph-storage --test training_records_integration 15 passed
cargo test -p context-graph-embeddings --lib                          1148 passed, 2 ignored
cargo test -p context-graph-core --lib                                2042 passed
cargo test -p context-graph-storage --lib                             501 passed, 1 ignored
cargo test -p context-graph-mcp --lib                                 340 passed
cargo test -p context-graph-cli --test e14_phase_b_full_state_verification 12 passed
cargo test -p context-graph-cli --test e14_optimization_fsv           10 passed
cargo test -p context-graph-embeddings test_embed_real_text_produces_normalised_1024d_vector -- --ignored 1 passed
```

## Residual Notes

- The live `contextgraph_data` store currently has zero fingerprints, so live
  backfill validated command behavior and database open/read state but did not
  prove migration over existing live records.
- Existing docs still contain older 13-embedder narrative in several research
  drafts; runtime code and tests now verify 14 active production embedders.
