# TECH-ENRICHMENT — Technical Specification

> **Level 3.** Defines HOW the multi-embedder DDA/TCT enrichment pipeline is built. Implements [`SPEC-ENRICHMENT.md`](../functional/SPEC-ENRICHMENT.md).

---

## Module layout

```
crates/context-graph-mejepa-embedders/
  src/
    forward/
      mod.rs                  # NEW: EmbedderForward trait
      e1_e5_large.rs          # NEW: Candle/CUDA wrapper for E5-style dense
      e7_qodo.rs              # NEW: Qodo-Embed code-specific
      e8_graph_attention.rs   # NEW: graph adapter on top of E7
      e10_paraphrase.rs       # NEW
      e12_colbert.rs          # NEW
      e14_codet5.rs           # NEW
      e17_affect.rs           # NEW
      deterministic.rs        # existing: kept for E2/E3/E4/E6/E9/E13 (no neural model)
    cache.rs                  # NEW: (embedder_id, chunk_sha) → vector LRU
    routing.rs                # existing: 143-row routing table
    digest_verify.rs          # existing: SHA-256 verification at startup
    vram_budget.rs            # existing: extended with startup gate

crates/context-graph-mejepa-train/
  src/
    dda/
      mod.rs                  # NEW: DDA signal computation
      pairwise_mi.rs          # existing (REQ-UTML-06 shipped); reused
      blind_spot.rs           # NEW: cross-embedder z-score features

crates/context-graph-mejepa-tct/
  src/
    constellation.rs          # existing
    builder.rs                # existing; extended for per-cell refresh
    gtau.rs                   # existing
    freshness.rs              # existing; consumed by scheduler

crates/context-graph-mejepa-instruments/
  data/
    stdlib_registry/
      python.yml              # NEW
      rust.yml                # NEW
      javascript.yml          # NEW
      typescript.yml          # NEW
      go.yml                  # NEW
      java.yml                # NEW
      c.yml                   # NEW
      cpp.yml                 # NEW
      csharp.yml              # NEW
      ruby.yml                # NEW
      php.yml                 # NEW
```

---

## EmbedderForward trait

```rust
pub trait EmbedderForward: Send + Sync {
    fn embedder_id(&self) -> EmbedderId;
    fn dim(&self) -> usize;
    fn forward(&self, chunk: &Chunk) -> Result<EmbeddingVec>;
    fn supports_batch(&self) -> bool { false }
    fn forward_batch(&self, chunks: &[Chunk]) -> Result<Vec<EmbeddingVec>> {
        chunks.iter().map(|c| self.forward(c)).collect()
    }
}
```

Per-embedder implementations load Candle/CUDA models from `/mnt/d/cabdru/contextgraph/models/` at construction (with digest verify per `Phase 1b`). Forward methods run real neural inference.

---

## Embedder cache

```rust
// crates/context-graph-mejepa-embedders/src/cache.rs
pub struct EmbedderCache {
    inner: Arc<RwLock<LruCache<CacheKey, Arc<EmbeddingVec>>>>,
    persist_path: PathBuf,                     // /mnt/d/contextgraph/storage/embedder-cache/
}

#[derive(Hash, PartialEq, Eq)]
pub struct CacheKey {
    pub embedder_id: EmbedderId,
    pub chunk_sha: Sha256,
}

impl EmbedderCache {
    pub fn get_or_compute(&self, key: CacheKey, compute: impl FnOnce() -> Result<EmbeddingVec>) -> Result<Arc<EmbeddingVec>> {
        if let Some(v) = self.inner.read().peek(&key) {
            return Ok(v.clone());
        }
        let v = Arc::new(compute()?);
        self.inner.write().put(key.clone(), v.clone());
        self.persist_async(key, v.clone());
        Ok(v)
    }
}
```

Persistence to disk under `/mnt/d/contextgraph/storage/embedder-cache/<embedder_id>/<chunk_sha>.bin`. LRU bounded at 100k entries × 12 embedders × 4 KB ≈ 4.8 GB max.

---

## DDA signal computation

```rust
// crates/context-graph-mejepa-train/src/dda/mod.rs
pub fn compute_dda_signals(
    panel_id: PanelId,
    chunk: &Chunk,
    per_embedder_vecs: &HashMap<EmbedderId, EmbeddingVec>,
    constellation: &TctConstellation,
    cached_mi_matrix: &PairwiseMiMatrix,
) -> DdaSignals {
    let active = active_embedders();
    let n = active.len();
    
    // Per-embedder cosine to centroid
    let mut per_embedder_cosine = Vec::with_capacity(n);
    for emb_id in &active {
        let cell = constellation.lookup_default(chunk, *emb_id);
        let cosine = cosine_sim(&per_embedder_vecs[emb_id], &cell.centroid);
        per_embedder_cosine.push(cosine);
    }
    
    // Pairwise cosine (upper triangle)
    let mut pairwise_cosine = Vec::with_capacity(n * (n - 1) / 2);
    for (i, m1) in active.iter().enumerate() {
        for m2 in active.iter().skip(i + 1) {
            pairwise_cosine.push(cosine_sim(&per_embedder_vecs[m1], &per_embedder_vecs[m2]));
        }
    }
    
    // Pairwise MI (cached weekly; just look up)
    let pairwise_mi = cached_mi_matrix.upper_triangle();
    
    // Blind-spot z-scores
    let blind_spot_z = compute_blind_spot_zs(&pairwise_cosine, cached_mi_matrix.expected_pairwise());
    
    DdaSignals {
        per_embedder_cosine,
        pairwise_cosine_upper: pairwise_cosine,
        pairwise_mi_upper: pairwise_mi,
        blind_spot_z_scores: blind_spot_z,
    }
}
```

Persisted to `CF_MEJEPA_DDA_SIGNALS` keyed by `(panel_id, chunk_id)`.

---

## VRAM budget enforcement at startup

```rust
// crates/context-graph-mejepa-embedders/src/vram_budget.rs (extension)
pub fn enforce_startup_budget(active: &[EmbedderId], budget_gb: f32) -> Result<()> {
    let mut total_gb = 0.0;
    for emb_id in active {
        let model_size_gb = get_model_size_gb(*emb_id)?;
        total_gb += model_size_gb;
    }
    if total_gb > budget_gb {
        return Err(EmbedderError::VramBudgetExceeded {
            requested_gb: total_gb,
            budget_gb,
            embedders: active.to_vec(),
        });
    }
    Ok(())
}
```

Called by daemon startup right after digest verification. Refuses to start the daemon if exceeded.

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
# crates/context-graph-mejepa-instruments/data/stdlib_registry/python.yml
version: 1
language: python
patterns:
  - kind: ReinventedWheel
    pattern_name: hand_rolled_retry_with_backoff
    detection:
      ast_match: |
        for attempt in range(MAX_RETRIES):
          try:
            ...
          except Exception:
            time.sleep(2 ** attempt)
    suggested_alternative: "tenacity.retry()"
    confidence: 0.85
  
  - kind: ReinventedWheel
    pattern_name: hand_rolled_logger
    detection:
      ast_match: |
        def log(msg):
          print(f"[{datetime.now()}] {msg}")
    suggested_alternative: "logging.getLogger(__name__)"
    confidence: 0.95
  
  # ... more patterns
```

Loaded at startup; consumed by the redundancy / dead-code label pipelines and surfaced as `PredictedRedundancy { kind: ReinventedWheel, ... }` in predictions.

---

## FSV strategy

| FSV | What it proves | Output path |
|---|---|---|
| `embedder_forward_e1_fsv` | E1 forward returns expected vector for deterministic chunk | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/e1.json` |
| ... per embedder ... | each of the 12 active embedders has its own FSV | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/<embedder_id>.json` |
| `vram_budget_fsv` | over-budget config refuses to start | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/vram_budget.json` |
| `dda_signal_compute_fsv` | DdaSignals computed correctly for synthetic chunks | `/mnt/d/contextgraph/fsv/mejepa-train-fsv/dda_signal.json` |
| `embedder_cache_fsv` | cache hit returns identical vector | `/mnt/d/contextgraph/fsv/mejepa-embedders-fsv/cache.json` |
| `constellation_refresh_fsv` | stale cell refit produces new centroid + tau_m | `/mnt/d/contextgraph/fsv/mejepa-tct-fsv/refresh.json` |
| `stdlib_registry_load_fsv` | all 11 per-language YAMLs load + parse | `/mnt/d/contextgraph/fsv/mejepa-instruments-fsv/stdlib_registry.json` |
