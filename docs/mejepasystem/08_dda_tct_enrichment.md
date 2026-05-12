# 08 — DDA + TCT Enrichment (Gτ guard, constellation)

Source files covered: `crates/context-graph-mejepa-tct/src/{lib,builder,calibrator,constellation,freshness,gtau,hinge,hinge_types,inspect,panel_slots,rate,rate_aggregator,shrinkage,shrinkage_engine,store,types,verdict}.rs`, `crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi_matrix.rs`, `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md`, `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`.

---

## 1. The signal-count math (DDA)

For input × N frozen embedders, every input yields **n · (N + N²)** structured signals at n aggregation points (chunks):

| Signal class | Per chunk | What it measures |
|---|---|---|
| Per-embedder cosine to centroid (N) | 13 (active code) | "this chunk looks anomalous to embedder m" |
| Pairwise cosine between embedders (N²) | 169 | "embedders m1, m2 agree or disagree" — blind-spot signal |
| Pairwise mutual information (cached) | 169 | structural redundancy across embedders → ΔΞ.predictor_redundancy |
| **Total per chunk** | **351** | |
| For ~30 chunks per patch | **~10,530** | dense signal budget per training example |

For N=21 (full ensemble), it's **n × 462 signals**.

---

## 2. The pipeline (Step 4 of `04_DDA_TCT_CODE_ENRICHMENT.md §2`)

```
For each chunk × N active embedders:
  - cosine_to_centroid[m] = cosine(emb_m(chunk), constellation[category, kind, lang, m].centroid)
  - pairwise_cosine[m1][m2] = cosine(emb_m1(chunk), emb_m2(chunk))
  - pairwise_mi[m1][m2] = cached MI matrix (recomputed weekly via mejepa_audit_pairwise_mi)
Persist to CF_MEJEPA_DDA_SIGNALS keyed by (panel_id, chunk_id)
```

`DdaSignals` struct (futurebuild Phase A, **shipped 2026-05-11**):

```rust
pub struct DdaSignals {
    pub per_embedder_cosine: [f32; N],
    pub pairwise_cosine: [[f32; N]; N],
    pub pairwise_mi: [[f32; N]; N],  // cached; recomputed weekly
}
```

---

## 3. The TCT constellation

Per `docs/ruvectorfindings/13 §2`, the constellation is a per-(`category × entity × language × embedder`) centroid table built from the Phase 0 mutation corpus + KnownGood baseline.

### 3.1 Cell schema

```rust
pub struct ConstellationCell {
    pub category: MutationCategory,          // KnownGood, SubtleFlip, OffByOne, ...
    pub entity_kind: EntityType,             // Function, Class, Impl, ...
    pub language: Language,
    pub embedder: EmbedderId,
    pub centroid: EmbeddingVec,              // mean of training set's embedder-m output
    pub bayesian_shrunk: bool,               // true when N < 50
    pub n_supporting: u32,                   // training examples in cell
    pub tau_m: f32,                          // per-embedder threshold (FP rate ≤ 5% on heldout KnownGood)
    pub freshness_ts: SystemTime,            // refresh policy: stale > 90 days → refuse without override
    pub corpus_provenance: CorpusProvenance, // corpus_sha + embedder_versions + frozen_at + code_version
}
```

Persisted to `CF_MEJEPA_CONSTELLATION` keyed by `(version_id, category, entity_kind, language, embedder_id)`.

### 3.2 Cell math

| Dimension | Cardinality |
|---|---|
| Mutation categories | 8 |
| Entity types | ~13 per language |
| Languages | 11 |
| Active embedders (code-domain) | 13 |
| **Total max cells** | **≈ 13,728** |

Many cells are **physically impossible** (e.g., `Impl` for Python; `Trait` for Java) and absent. Many will have `N < 50` and be **Bayesian-shrunk** toward the language-aggregate or entity-aggregate centroid.

### 3.3 Bayesian shrinkage

Source: `shrinkage.rs`, `shrinkage_engine.rs`. When `n_supporting < 50`:

```
centroid_shrunk = (n × centroid_observed + shrinkage_strength × centroid_marginal)
                 / (n + shrinkage_strength)
```

with `shrinkage_strength = 50 − n`. The marginal is computed at the language-aggregate level for that (category × embedder) pair.

### 3.4 Threshold calibration

Source: `calibrator.rs`. Per-(category × language × embedder), choose `τ_m` such that false-positive rate ≤ 5% on heldout KnownGood examples. The calibration is **adversarial** to the training set: it uses held-out shards, not the cells' supporting set.

---

## 4. Constellation construction (Phase 4b builder)

Source: `builder.rs`. Pipeline:

1. **Read mutation corpus** from `CF_MEJEPA_MUTATION_CORPUS` for the selected `corpus_id`.
2. **For each (task, category)** in the corpus:
   - Read the source file at the after_sha.
   - Chunk via `memory::ast::chunk_with_options(...)`.
   - For each chunk × active embedder: compute the embedder vector (real neural forward).
3. **Aggregate by cell** `(category, entity_kind, language, embedder_id)`:
   - Append each chunk's vector to the cell's training set.
4. **Compute centroid** (mean) + variance.
5. **Bayesian shrink** if `n_supporting < 50`.
6. **Calibrate τ_m** on heldout split.
7. **Persist** to `CF_MEJEPA_CONSTELLATION` with provenance.

Total cells materialized scales with corpus size; for the 300×8 SWE-bench Lite (Python), expect ~1,000–3,000 cells initially, growing as additional languages and per-language corpora ship (Phase C).

---

## 5. Refresh policy

Source: `freshness.rs`. A cell is refit when **any** of:

| Trigger | Default |
|---|---|
| More than 100 new training examples since last refit | 100 |
| Per-embedder threshold τ_m calibration drifts > 3% on rolling validation | 0.03 |
| Cell's freshness ts is older than max_age_days | 90 days |
| Operator force-refit via `mejepa_constellation_inspect --force-refit` | n/a |

Stale cells → `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` on every call until refit (no silent operation on stale data).

---

## 6. The Gτ guard

Source: `gtau.rs`. **Deterministic O(N · d_max) test**:

```rust
fn gtau_check_panel(predicted_panel: &Panel, constellation: &Constellation) -> Vec<GuardViolation> {
    let mut violations = vec![];
    for chunk in &predicted_panel.chunks {
        for embedder_m in active_embedders() {
            let cell = constellation.lookup(
                category = "predicted",
                entity_kind = chunk.kind,
                language = chunk.language,
                embedder = embedder_m,
            );
            let cosine = cosine_similarity(
                predicted_panel.per_chunk_emb(chunk, embedder_m),
                cell.centroid,
            );
            if cosine < cell.tau_m {
                violations.push(GuardViolation {
                    embedder: embedder_m,
                    chunk: chunk.id,
                    centroid_id: cell.id,
                    cosine,
                    threshold_tau_m: cell.tau_m,
                    deficit: cell.tau_m - cosine,
                });
            }
        }
    }
    violations
}
```

If `len(violations) > 0`, verdict = `Verdict::GuardRejected` with `violating_embedders: Vec<EmbedderId>` and `violating_chunks: Vec<ChunkId>`. Chunk-level localization via `gtau_check_chunks` (shipped, FSV-verified).

### 6.1 Why this is structurally separate from the Oracle prediction

The Gτ guard asserts the predicted panel's per-embedder geometry sits inside the manifold of acceptable predictions for this chunk in this language. **The optimizer cannot Goodhart against this** — there's no gradient flowing through the constellation centroids; they're descriptive, not learned. Per `docs/ruvectorfindings/13 §1.4`.

---

## 7. The `L_constellation_centroid` loss (training-side use)

Source: `crates/context-graph-mejepa-train/src/loss/constellation.rs` + `hinge.rs`. Hinge loss term:

```
L_constellation = Σ_m max(0, τ_m − cos(g_φ(panel)_m, c_m))
```

This pushes the predictor's output panel back toward the constellation manifold during training. Coefficient `δ = 0.2` by default in the multi-head loss aggregator (`docs/futurebuild/10 §8`).

---

## 8. The pairwise MI matrix (REQ-UTML-06, shipped)

Source: `crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi_matrix.rs`.

```rust
pub struct PairwiseMiMatrix {
    pub matrix: Vec<Vec<f32>>,           // N × N
    pub embedder_ids: Vec<EmbedderId>,
    pub computed_at: DateTime<Utc>,
    pub source_window: (StepId, StepId),
}

pub struct PairwiseMiAuditor {
    pub redundancy_ceiling: f32,         // default 0.85
}

impl PairwiseMiAuditor {
    pub fn compute(&self, storage: &Storage) -> Result<PairwiseMiMatrix, AuditError>;
    pub fn write_csv(&self, matrix: &PairwiseMiMatrix, path: &Path) -> Result<()>;
}
```

- **Recompute weekly** via the daemon's weekly tick.
- **Persist CSV** at `/mnt/d/contextgraph/exports/pairwise-mi/<date>.csv`.
- **MCP tool**: `mejepa_audit_pairwise_mi`.
- **Heatmap renderer** is futurebuild (`TASK-OBS-002`).
- **Flag pairs with MI > 0.85**: operator decides retire / recalibrate / accept.

---

## 9. Per-head DDA signal consumption

For the multi-head predictor (see [`09_predictor_multi_head.md`](09_predictor_multi_head.md)), each head reads its DDA-signal subset as auxiliary features:

| Head | DDA signals consumed | Aggregation |
|---|---|---|
| Panel | per-chunk embedder vectors (raw) | latent target |
| Oracle | cosine_to_centroid for E_Oracle, E_Test, E_Reasoning | direct correlation |
| Failure-mode | cosine for E_AST, E_CFG, E_DataFlow, E_TypeGraph + pairwise[E_TypeGraph][E_DataFlow] | type/flow disagreement |
| Edge-case | cosine for E_DataFlow + scalar coverage_delta | uncovered branch + flow signature |
| Tech-debt | scalars (complexity, length, nesting) + cosine for E_AST | structural smells |
| Perf | cosine for E_CFG (loops, hot paths) + scalar runtime | loop / call-stack signature |
| Security | cosine for E_StaticAnalysis + scalar SAST | direct binding to analyzer |
| Accuracy | cosine for E_Test + scalar metric deltas | ML test outputs |
| Cost | scalar (CI mins, token telemetry, dep count) | non-embedded scalars |
| Reasoning | pairwise[E_Reasoning][E_Diff], pairwise[E_Reasoning][E_Oracle] | prose ↔ change agreement |

Each head's projection layer learns a head-specific attention over the DDA signals it cares about. **The trunk parameter count doesn't grow with the signal count**; only the head-side projections do.

---

## 10. Constellation max-age + operator override

Per `freshness.rs` and `REQ-TCT-09`:
- Default `max_age_days = 90`.
- Reading a stale cell → `MEJEPA_INFER_GTAU_STALE_CONSTELLATION`.
- Operator override: `mejepa_constellation_inspect --force-allow-stale --category <c> --language <l>`.
- Force-refit: `mejepa_constellation_inspect --force-refit --category <c> --language <l>`.

---

## 11. FSV evidence

| FSV | Output |
|---|---|
| Constellation construction over Phase 0 corpus | `/tmp/contextgraph-mejepa-tct-fsv/constellation-evidence.json` |
| Gτ check (chunk-level localization) | `cargo test -p context-graph-mejepa-tct chunk_gtau --lib` |
| Bayesian shrinkage | `/tmp/contextgraph-mejepa-tct-fsv/shrinkage-evidence.json` |
| Threshold calibration FP ≤ 5% | `/tmp/contextgraph-mejepa-tct-fsv/calibration-evidence.json` |

---

## 12. What this subsystem commits to

1. Per-chunk DDA signals computed for every active embedder; persisted to `CF_MEJEPA_DDA_SIGNALS`.
2. Per-(category × entity × language × embedder) constellation cells with Bayesian shrinkage on N<50.
3. Per-embedder threshold τ_m calibrated for FP ≤ 5% on heldout KnownGood.
4. Deterministic Gτ guard with chunk-level localization (no gradient flow through centroids — collapse-immune by structure).
5. Pairwise MI auditor surfacing redundancy ceilings.
6. Cross-references: [`07_instruments_panel.md`](07_instruments_panel.md) (Panel composition), [`09_predictor_multi_head.md`](09_predictor_multi_head.md) (per-head DDA consumption), [`14_corpus_phase0.md`](14_corpus_phase0.md) (constellation training data).
