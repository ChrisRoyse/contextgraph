# Phase 4 — Agent D summary (Tucker-core export, CPU)

## Build status

`cargo build --release -p context-graph-mcp` with
`CUDARC_CUDA_VERSION=13000 CUDA_HOME=/usr/local/cuda-13.1 CUDA_PATH=/usr/local/cuda-13.1 PATH=/usr/local/cuda-13.1/bin:$PATH`:
- errors: 0
- warnings: 0 (the two `warning: context-graph-cuda@0.1.0: Running PTX compilation ...` log lines are NVCC build-script info notes, not Rust compiler warnings)
- elapsed: ~1m39s (final MCP crate compile, incremental from cached deps)

## Tests

### Phase 4 tests (this phase)

| Suite | Result |
| --- | --- |
| `context-graph-core --lib teleological::tucker` | 8 / 8 PASS |
| `context-graph-storage --test training_records_integration` | 15 / 15 PASS (13 legacy + 2 new tucker) |

### Regression suites (unchanged behaviour)

| Suite | Result |
| --- | --- |
| `context-graph-core --lib training::` | 18 / 18 PASS |
| `context-graph-core --lib constellation::` | 19 / 19 PASS |
| `context-graph-core --lib contrastive::` | 21 / 21 PASS |
| `context-graph-mcp --lib tools::definitions::` | 31 / 31 PASS (schema count + submodule counts unchanged) |

Total: 112 tests run for this phase, 112 PASS.

## Files created

- `/home/cabdru/contextgraph/crates/context-graph-core/src/teleological/tucker.rs` — streaming HOSVD compressor (~950 LOC incl. tests). Exposes `CpuTuckerCompressor`, `TuckerCompressor`, `TuckerError`, `extract_reference_view`, `reconstruct_tensor`, `relative_frobenius_error`, `REFERENCE_EMBEDDING_DIM = 1024`, `DEFAULT_RANKS = (4, 4, 128)`.

## Files modified

- `/home/cabdru/contextgraph/crates/context-graph-core/src/teleological/mod.rs` — added `pub mod tucker;` and `pub use tucker::{CpuTuckerCompressor, TuckerCompressor, TuckerError};`.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/training.rs` — added `includeTuckerCore` property (default `false`) to the `export_training_corpus` schema; tool description now mentions the Phase-4 knob.
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/training_tools.rs` — parses `includeTuckerCore`; threads it through `build_training_record`; calls `CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS)` when true and logs `warn!` + keeps `tucker_core=None` on compression failure; adds `tucker_core_included` flag to the summary response; adds `tucker_core` shape payload to `render_training_record`.
- `/home/cabdru/contextgraph/crates/context-graph-storage/tests/training_records_integration.rs` — added `record_with_tucker_core_roundtrips_through_rocksdb` and `record_without_tucker_core_has_none`.

No other files touched. Phase 1/2/3 core logic is untouched.

## Key signatures added

```rust
// crates/context-graph-core/src/teleological/tucker.rs

pub const REFERENCE_EMBEDDING_DIM: usize = 1024;
pub const DEFAULT_RANKS: (usize, usize, usize) = (4, 4, 128);

pub enum TuckerError {
    InvalidRank { r1: usize, r2: usize, r3: usize, d1: usize, d2: usize, d3: usize },
    EmptyFingerprint,
    Numerical(String),
}

pub trait TuckerCompressor {
    fn compress(
        &self,
        fingerprint: &SemanticFingerprint,
        ranks: (usize, usize, usize),
    ) -> Result<TuckerCore, TuckerError>;
}

pub struct CpuTuckerCompressor;

pub fn extract_reference_view(
    fp: &SemanticFingerprint,
) -> Box<[[f32; REFERENCE_EMBEDDING_DIM]; NUM_EMBEDDERS]>;

pub fn reconstruct_tensor(core: &TuckerCore) -> Vec<f32>;      // length 13*13*1024

pub fn relative_frobenius_error(a: &[f32], b: &[f32]) -> f32;
```

The MCP handler now calls `CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS)` whenever `includeTuckerCore=true` and assigns the result to `TrainingRecord.tucker_core`.

## Algorithm notes

Implements a streaming HOSVD that never materializes the full 13 x 13 x 1024 (173 056-float) interaction tensor `T[i][j][d] = v_i[d] * v_j[d]`:

1. **Reference-frame projection** (`extract_reference_view`). All 13 embeddings are mapped into a shared 1024-D frame:
   - 1024-D embedders (E1, E8-source, E9): used as-is
   - Smaller dense embedders (E2/E3/E4 512D; E5-cause, E10-paraphrase, E11 768D): zero-padded on the right
   - E7 (1536D): truncated to the first 1024 entries
   - E6 + E13 sparse: zero rows
   - E12 token-level: mean-pooled per token into 128 leading entries, zero-padded after; empty token stream → zero row
   - Asymmetric E5/E8/E10: use cause / source / paraphrase side (verified via FSV test).
2. **Mode-1 / mode-2 Gram matrices** built in O(13^2 * 1024) FMAs using the identity
   `G_1[a, b] = sum_d v_a[d] * v_b[d] * w[d]` with `w[d] = sum_j v_j[d]^2`. Since i<->j is symmetric, `G_2 == G_1`.
3. **Small-matrix eigendecomposition**: Jacobi sweep on the two 13×13 Grams (scale-aware convergence, up to 50 sweeps). Returns the top `rank_1` / `rank_2` eigenvectors.
4. **Mode-3 Gram (1024 x 1024)**: computed via `M = V^T V` then squared element-wise. Top `rank_3` eigenvectors extracted with **power iteration + deflation + double-pass Gram–Schmidt reorthogonalization**. Matrix M is stored as an f32 flat buffer (~4 MB at 1024x1024); G_3 is applied lazily (never built).
5. **Core tensor** assembly via
   `core[r1, r2, r3] = sum_d U3[d, r3] * A[r1, d] * B[r2, d]`
   where `A = U1^T V`, `B = U2^T V`. For defaults (4, 4, 128) this is ~2M FMAs (<10 ms per memory).

Factor matrices are packed row-major: `u1[i * rank_1 + r1]` gives `U1[i, r1]`.

## FSV status

| FSV | Description | Status |
| --- | --- | --- |
| **FSV-15** | Degenerate single-direction fingerprint yields rank-1 structure in U1 | PASS — `degenerate_single_direction_yields_rank_1_structure`. Top eigenvector equals `1/sqrt(10)` on the 10 dense rows and <1e-5 on the 3 sparse/token rows; unit-norm check passes within 1e-4. |
| **FSV-16** | Reconstruction error < 0.15 on a non-degenerate structured fingerprint | PASS — `reconstruction_error_bound_synthetic`. Structured data is a rank-2 combination of two 1024-D basis functions; `relative_frobenius_error(T_full, T_hat) < 0.15` under rank (4, 4, 128). |
| **FSV-17** | Bincode roundtrip preserves every `TuckerCore` field byte-for-byte | PASS — `bincode_roundtrip_tucker_core` (in-memory) + `record_with_tucker_core_roundtrips_through_rocksdb` (through RocksDB persistence). Bit-level equality asserted on data/u1/u2/u3. |
| **FSV-18** | GPU vs CPU parity | DEFERRED — Phase 4 is CPU-only per plan §6.3 ("GPU path deferred unless trivial. It is not trivial."). No `candle` / `cudarc` code added. |

Additional tests beyond the FSV baseline:
- `empty_fingerprint_returns_empty_error` — all-zero fingerprint → `TuckerError::EmptyFingerprint` (not silent).
- `invalid_rank_returns_invalid_rank` — three distinct out-of-range rank tuples all produce `TuckerError::InvalidRank`.
- `extract_reference_view_asymmetric_uses_cause_source_paraphrase_sides` — confirms E5 uses the cause side, E8 uses the source side, E10 uses the paraphrase side (not the 99/88/77 sentinel effect/target/context sides).
- `extract_reference_view_truncates_e7_to_1024` — 1536-D E7 input yields the first 1024 entries only.
- `extract_reference_view_zero_pads_small_embedder` — 512-D E2 input yields 512 populated entries followed by 512 zeros.
- `record_without_tucker_core_has_none` — a record stored with `tucker_core=None` comes back `None` after RocksDB roundtrip.

## Known limitations

- **Power-iteration tolerance** is 1e-5 (direction + Rayleigh quotient). This is deliberately relaxed from 1e-9 because pure power iteration stalls on closely-spaced eigenvalues (Golub & Van Loan §7.3). For full machine precision on clustered spectra we would need block / subspace iteration or Lanczos; both are out of scope for Phase 4. The relaxed tolerance still achieves < 0.15 Frobenius error on the structured FSV-16 fixture.
- **E6, E12, E13 contribute zero rows** in the reference frame. This is the documented simplification from plan §6.3 ("reference dimension is 1024D"). Consequences:
  - Mode-1/mode-2 Gram has effective rank ≤ 10 (minus E11-disabled corpus data where E11 is also zero).
  - E6/E13 keyword signals and E12 token-level structure are NOT visible to the Tucker decomposition — they live in incomparable spaces. A future phase that wants to surface them would need a multi-frame HOSVD (outside this plan).
- **Mode-3 Gram `M = V^T V`** is stored as a 1024 x 1024 f32 buffer (4 MB peak per compression). At defaults this comfortably fits in any training host; `rank_3` up to 1024 is supported in principle, but anything above ~256 linearly scales the power-iteration cost.
- **No GPU path** (FSV-18). Deferred per plan §6 and called out in the module docstring. If a future phase adds a Candle path it must live behind a feature flag and be benchmarked against the CPU path before replacing it.
- **Per-memory cost**: ~4 MB peak transient RAM + ~10–30 ms CPU per memory at defaults. A 10 K-memory export with `includeTuckerCore=true` adds ~5 minutes of compute and ~180 MB of on-disk payload. `includeTuckerCore` defaults to `false` precisely because of this cost.
- Compression-failure path (e.g. fingerprint with entirely-zero dense embeddings and empty sparse) sets `tucker_core=None` on the persisted record and logs `warn!` with the memory id and the `TuckerError::Display`. The rest of the record still persists — the Tucker compression is strictly additive.

## Memory for next agent (use verbatim)

Title: **IMPL-PHASE-4-DONE**

Content:

Phase 4 (Tucker-core export, CPU-only) is complete and green.

Module: `crates/context-graph-core/src/teleological/tucker.rs`
- `CpuTuckerCompressor` implements `TuckerCompressor`: `compress(&SemanticFingerprint, (rank_1, rank_2, rank_3)) -> Result<TuckerCore, TuckerError>`
- `REFERENCE_EMBEDDING_DIM = 1024`, `DEFAULT_RANKS = (4, 4, 128)`
- Streaming HOSVD: mode-1/2 Gram built from `w[d] = sum_j v_j[d]^2`, mode-3 Gram stored as M^2 (`M = V^T V`), eigenvectors via Jacobi (13x13) and power iteration with deflation + double-pass Gram-Schmidt (1024x1024).
- Helpers exposed for downstream testing: `extract_reference_view`, `reconstruct_tensor`, `relative_frobenius_error`.

Reference-frame conventions (documented in the module docstring):
- E1 / E8-source / E9 (1024D): as-is.
- E2 / E3 / E4 (512D), E5-cause / E10-paraphrase / E11 (768D): zero-padded on the right to 1024D.
- E7 (1536D): truncated to first 1024.
- E6, E13 sparse, E12 empty tokens: zero rows.
- E12 with tokens: per-token mean-pool into 128 leading entries, zero-padded after.
- Asymmetric E5/E8/E10 use cause/source/paraphrase side.

MCP wiring:
- `export_training_corpus` schema has `includeTuckerCore: boolean` (default `false`).
- Handler: when true, calls `CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS)` per memory and writes `TrainingRecord.tucker_core = Some(core)`. Compression errors log `warn!` and leave `tucker_core = None`; rest of the record persists.
- Response summary includes `tucker_core_included: bool`.
- `get_training_record` response includes a `tucker_core` block with `ranks`, `data_len`, `u1_len`, `u2_len`, `u3_len`, `compression_ratio` when present.
- `list_training_records(includeShape=true)` is unchanged (shape summary did not need a new tucker field — Phase 4 is opt-in and heavy; the presence flag is already on the `get` path).

TuckerCore layout (unchanged — this phase only populates it):
- `ranks: (rank_1, rank_2, rank_3)`
- `data: Vec<f32>` length `rank_1 * rank_2 * rank_3` (for defaults = 2048)
- `u1: Vec<f32>` length `13 * rank_1` — row-major, `u1[i*rank_1 + r1]` = U1[i, r1]
- `u2: Vec<f32>` length `13 * rank_2`
- `u3: Vec<f32>` length `1024 * rank_3` (for defaults = 131 072)

On-disk: identical layout as Phase 1. Version byte unchanged (bincode encodes `Option<TuckerCore>` transparently; `None` is one byte, `Some(...)` is the normal struct tag). No migration required. All pre-existing Phase-1 records still deserialize, with `tucker_core = None`.

FSV: 15/16/17 green. 18 (GPU parity) intentionally deferred per plan §6.3.

What Phase 6 / 7 agents can rely on:
- `TrainingRecord.tucker_core: Option<TuckerCore>` — always `Some` for records exported with `includeTuckerCore=true`, always `None` otherwise (or on compression failure — check logs).
- Asymmetric embedders ARE normalized to their cause/source/paraphrase side before Tucker — consumers don't need to re-resolve.
- E6 / E12 / E13 show up as zero rows in U1/U2/U3. Do not interpret zero mass on indices 5, 11, 12 as "no signal" — it's the reference-frame limitation.
- CPU-only. `context-graph-core` has no new dep. No GPU kernels changed.
