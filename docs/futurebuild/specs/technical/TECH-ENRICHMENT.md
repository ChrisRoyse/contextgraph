# TECH-ENRICHMENT — Technical Specification

> **Level 3.** Defines HOW the multi-embedder DDA/TCT enrichment pipeline is built. Implements [`SPEC-ENRICHMENT.md`](../functional/SPEC-ENRICHMENT.md).

---

## Module layout

```
crates/context-graph-mejepa-embedders/
  src/
    forward.rs                # shipped: EmbedderForward, PretrainedEmbedderForward, AlgorithmicEmbedderForward
    cache.rs                  # shipped: (schema, embedder_id, model_version, chunk_sha) → vector
    routing.rs                # existing: 143-row routing table
    digest.rs                 # existing: SHA-256 verification at startup
    vram.rs                   # existing: VRAM budget query/gate

crates/context-graph-mejepa-train/
  src/
    dda.rs                    # shipped: DDA signal computation + RocksDB persistence/readback
    learning_signal/
      pairwise_mi_matrix.rs   # existing (REQ-UTML-06 shipped); weekly MI source

crates/context-graph-mejepa-tct/
  src/
    constellation.rs          # existing
    builder.rs                # existing; extended for per-cell refresh
    gtau.rs                   # existing
    freshness.rs              # existing; consumed by scheduler
    refresh_report.rs         # shipped: durable reward-signal support report for constellation refresh
    store.rs                  # shipped: sync write + WAL/CF flush + read-after-write/reopen readback for constellation artifacts

crates/context-graph-mejepa-instruments/
  assets/
    stdlib_registry/
      python.yml              # shipped
      rust.yml                # shipped
      javascript.yml          # shipped
      typescript.yml          # shipped
      go.yml                  # shipped
      java.yml                # shipped
      c.yml                   # shipped
      cpp.yml                 # shipped
      csharp.yml              # shipped
      ruby.yml                # shipped
      php.yml                 # shipped
  src/
    e_code.rs                 # shipped: Python Ruff semantic analyzer facts for E_AST/E_CFG/E_DataFlow/E_TypeGraph; non-Python parity remains staged
    stdlib_registry.rs        # shipped: schema-validating loader + detector
```

---

## EmbedderForward trait

```rust
pub trait EmbedderForward: Send + Sync {
    fn embedder(&self) -> EmbedderId;
    fn model_version(&self) -> &str;
    fn artifact_root(&self) -> &Path;
    async fn forward(&self, input: &EmbedderInput) -> EmbedResult<EmbedderOutput>;
}
```

`PretrainedEmbedderForward` loads Candle/CUDA models from `/mnt/d/cabdru/contextgraph/models/` at construction with digest/runtime artifact verification. `AlgorithmicEmbedderForward` covers E2/E3/E4/E9 with real mathematical implementations. Temporal algorithmic inputs require an explicit `timestamp:` or `epoch:` instruction; missing time fails closed so cache keys are reproducible.

---

## Embedder cache

```rust
// crates/context-graph-mejepa-embedders/src/cache.rs
pub struct EmbedderCache {
    root: PathBuf,
}

pub struct EmbedderCacheKey {
    pub schema_version: u16,
    pub embedder: EmbedderId,
    pub model_version: String,
    pub chunk_sha256: String,
}

impl EmbedderCache {
    pub async fn forward_cached(
        &self,
        forwarder: &dyn EmbedderForward,
        input: &EmbedderInput,
    ) -> EmbedResult<EmbedderOutput>;
}
```

Persistence writes JSON under `<root>/<embedder>/<model_version_hash>/<chunk_sha>.json` with atomic temp-file write, fsync, rename, directory fsync, and readback. Corrupt entries fail closed instead of being treated as misses. LRU pruning and production hit-rate telemetry remain open.

---

## DDA signal computation

```rust
// crates/context-graph-mejepa-train/src/dda.rs
pub fn compute_dda_signals(
    inputs: &[DdaVectorInput],
    baseline: &DdaPairwiseBaseline,
) -> DdaResult<DdaSignals> {
    // validates non-empty, sorted unique embedder ids, finite vectors,
    // vector/centroid dimension equality, baseline lengths, and positive sigmas
}
```

Per-embedder cosine is computed against each input's centroid. Pairwise cosine uses a versioned signed-hash projection (`signed-hash-projection-v1`, 128 dims) so heterogeneous embedder dimensions can be compared in a common DDA basis without dropping or padding data. Pairwise arrays are stored as strict upper triangles. Persisted to `CF_MEJEPA_DDA_SIGNALS` keyed by `(panel_id, chunk_id)` through `persist_dda_signals`, which sync-writes, flushes the CF, and immediately reads back/deserializes/validates the value.

---

## VRAM budget enforcement at startup

```rust
// crates/context-graph-mejepa-embedders/src/vram.rs
pub fn query_vram_budget(budget: VramBudget) -> EmbedResult<VramBudgetReport>;
```

`query_vram_budget` reads CUDA driver memory info (`cuMemGetInfo_v2`) and returns a source-of-truth `VramBudgetReport` with required/free/total bytes, GPU name, compute capability, telemetry source, and non-authoritative `nvidia-smi` diagnostics. It fails closed with `EmbedError::VramExceeded` if free bytes are below the configured content-set or full-Phase-1 budget. Daemon startup evidence under the full active model load remains open.

---

## Constellation refresh

```rust
// crates/context-graph-mejepa-tct/src/builder.rs (extension)
pub async fn refit_cell(storage: &Storage, cell_key: ConstellationCellKey) -> Result<RefitOutcome> {
    // Pull all training examples matching (category, entity_kind, language) from CF_MEJEPA_PANELS
    let examples = pull_training_examples_for_cell(storage, &cell_key)?;
    
    if examples.len() < 50 {
        // Bayesian shrinkage path
        let new_centroid = compute_shrunk_centroid(&examples, &storage.global_marginal())?;
        persist_constellation_cell(storage, cell_key, new_centroid, n_supporting=examples.len())?;
        return Ok(RefitOutcome::Shrunk);
    }
    
    // Standard path
    let new_centroid = compute_centroid(&examples)?;
    let new_tau_m = calibrate_tau_m(&examples, fp_target=0.05)?;
    persist_constellation_cell(storage, cell_key, new_centroid, n_supporting=examples.len())?;
    Ok(RefitOutcome::Refit { new_tau_m })
}
```

The production artifact for a refresh is `ConstellationRefreshReport`, persisted to `CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS`. The report is content-addressed by `report_id`, validates its own digest, and records the durable reward-signal support needed to audit a constellation: source corpus digest/path, code commit, source row count, source chunk count, per-entity support, per-mutation-category support, per-language support, per-oracle-outcome support, per-cell centroid support, shrinkage-origin counts, operator violating-chunk diagnostics, and an estimated reward-scalar count. `ConstellationStore::persist` and `persist_refresh_report` use sync write options, explicit WAL/CF flush, and immediate deserialize/validate readback.

Triggered by the scheduler's constellation_freshness_tick when stale cells are found. The current Phase A-D FSV exercises the refresh path manually against the real Python corpus and AST evidence; Phase E wires this into the periodic scheduler policy.

---

## Per-language stdlib registry format

```yaml
# crates/context-graph-mejepa-instruments/assets/stdlib_registry/python.yml
schema_version: 1
language: python
patterns:
  - id: python_retry_backoff
    kind: ReinventedWheel
    name: Hand-rolled retry with exponential backoff
    stdlib_alternative: tenacity or urllib3.util.Retry
    confidence: 0.93
    detection:
      required_substrings:
        - "for attempt in range"
        - "try:"
        - "except"
        - "time.sleep"
        - "2 ** attempt"
      forbidden_substrings:
        - "from tenacity import"
```

Loaded at startup; consumed by the redundancy / dead-code label pipelines and surfaced as `PredictedRedundancy { kind: ReinventedWheel, ... }` in predictions.

---

## FSV strategy

| FSV | What it proves | Output path |
|---|---|---|
| `forward_wrapper_fsv` | all 8 pretrained content wrappers, all 4 algorithmic wrappers, E7 11-language code inputs, and 143 routing rows load/forward/read back valid output evidence | `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/` |
| `phase_d_embedder_cache_fsv` | E9 real algorithmic forward writes and reads a persistent cache file; corrupt cache fails closed | `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json` |
| `forward_wrapper_fsv` output files | one source-of-truth JSON output per active forward slot, plus E7 per-language files | `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/forward-outputs/` |
| `vram_budget_fsv` | over-budget config refuses to start | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/vram_budget.json` |
| `phase_d_dda_fsv` | DdaSignals computed, persisted, reopened, and read back for 30 chunks | `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-signal-compute.json` |
| `tct_centroid_refresh_fsv` | real 2,400-row Python corpus index plus 300-task AST evidence produces a persisted constellation + refresh report; after reopen the report proves 10,560 chunk-family observations, all 8 mutation categories, 1,008 centroid cells, shrinkage support, and operator rejected-chunk diagnostics; four fail-closed edge cases leave row count unchanged | `/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv/tct-centroid-refresh-evidence.json` |
| `phase_d_stdlib_registry_fsv` | all 11 per-language YAMLs load + parse; Python retry-backoff detects; missing/unknown registries fail closed | `/mnt/d/contextgraph/fsv/phase-d-fsv/stdlib-registry-fsv.json` |
| `python_semantic_analyzers_fsv` | Python E_AST/E_CFG/E_DataFlow/E_TypeGraph use distinct Ruff semantic facts, external Ruff/Pyright/py_compile evidence is persisted, and empty/invalid/non-Python/max-size edge cases print before/after source-of-truth state | `/mnt/d/contextgraph/fsv/contextgraph-python-semantic-analyzers-fsv/python-semantic-analyzers-evidence.json` |
