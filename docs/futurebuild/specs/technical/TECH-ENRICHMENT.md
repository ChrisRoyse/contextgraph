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

Triggered by the scheduler's constellation_freshness_tick when stale cells are found.

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
| `forward_wrapper_fsv` | E10 pretrained forward wrapper loads SHA-pinned artifacts and returns valid output | `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/` |
| `phase_d_embedder_cache_fsv` | E9 real algorithmic forward writes and reads a persistent cache file; corrupt cache fails closed | `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json` |
| ... per pretrained embedder ... | each pretrained active content embedder still needs its own full artifact FSV | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/<embedder_id>.json` |
| `vram_budget_fsv` | over-budget config refuses to start | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/vram_budget.json` |
| `phase_d_dda_fsv` | DdaSignals computed, persisted, reopened, and read back for 30 chunks | `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-signal-compute.json` |
| `constellation_refresh_fsv` | stale cell refit produces new centroid + tau_m | `/mnt/d/contextgraph/fsv/mejepa-tct-fsv/refresh.json` |
| `phase_d_stdlib_registry_fsv` | all 11 per-language YAMLs load + parse; Python retry-backoff detects; missing/unknown registries fail closed | `/mnt/d/contextgraph/fsv/phase-d-fsv/stdlib-registry-fsv.json` |
