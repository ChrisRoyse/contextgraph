# SPEC-ENRICHMENT — Functional Specification

> **Level 2.** Defines WHAT the multi-embedder DDA/TCT enrichment pipeline must do.
> Owns: REQ-ENRICH-01..15.
> Implementation reference: [`04_DDA_TCT_CODE_ENRICHMENT.md`](../../04_DDA_TCT_CODE_ENRICHMENT.md), [`05_EMBEDDER_SIGNAL_MAP.md`](../../05_EMBEDDER_SIGNAL_MAP.md).

---

## User stories

- **As ME-JEPA training**, I want every code chunk converted to N + N² structured signals across all valuable embedders so the multi-head predictor has dense supervision.
- **As ME-JEPA inference**, I want chunk-level Gτ guard checks against per-(category × language × embedder) centroids so I can localize geometric anomalies.
- **As an operator**, I want predictable storage growth with the new DDA signal block, controlled via tier compression.

## Requirements

### REQ-ENRICH-01 — Neural per-embedder forward wrappers

An `EmbedderForward` trait SHALL be implemented for each of the 12 active content embedders in the live registry: E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14. Pretrained slots run real Candle/CUDA forwards against SHA-pinned model artifacts under `/mnt/d/cabdru/contextgraph/models/`; algorithmic slots (E2/E3/E4/E9) run their real mathematical implementations. E17 is learner-state/affect, not a content embedder.

### REQ-ENRICH-02 — DDA signal computation per chunk per shift

For each chunk per shift, the system SHALL compute:
- Per-embedder cosine to the corresponding `(predicted_category, entity_kind, language, embedder)` centroid (N values).
- Pairwise cosine between embedder outputs (strict upper-triangle stored).
- Pairwise mutual information (strict upper-triangle, cached weekly; reused).
- Blind-spot z-scores (strict upper-triangle).

The theoretical feature budget is `n × (N + N²)`, but the persisted `DdaSignals` source of truth stores strict upper triangles. For N=12, each chunk stores 12 per-embedder values plus 66 pairwise cosine values, 66 MI values, and 66 z-scores.

### REQ-ENRICH-03 — Persist DDA signals to `CF_MEJEPA_DDA_SIGNALS`

The DDA signal block SHALL be persisted keyed by `(panel_id, chunk_id)` so it can be consumed by the TCT Gτ guard in O(1) and reused across training epochs without recomputation.

### REQ-ENRICH-04 — Embedder cache (chunk_sha, embedder_id) → vector

To avoid re-running forward passes on unchanged chunks, an embedder cache SHALL key on `(schema_version, embedder_id, model_version, chunk_sha)` and return cached vectors. Cache size bounded; LRU eviction. Persists across daemon restarts (under `/mnt/d/contextgraph/storage/embedder-cache/`).

### REQ-ENRICH-05 — VRAM budget gate at startup

The daemon SHALL refuse to start if the configured embedder set exceeds the GPU VRAM budget (default 10 GB on RTX 5090). Fail closed with `MEJEPA_EMBED_VRAM_EXCEEDED`. Lazy loading of embedders is permitted but the cap applies.

### REQ-ENRICH-06 — Per-embedder digest verification at startup

Each embedder loader SHALL SHA-256-verify its model artifact at startup. Mismatch fails closed with `MEJEPA_EMBED_DIGEST_MISMATCH`.

### REQ-ENRICH-07 — Routing table consistency

The 143-row routing table from `crates/context-graph-mejepa-embedders::routing` SHALL be exercised by an FSV that for every (Language × EntityType) combination, the routed embedder set is non-empty AND every embedder in the set returns a valid forward output.

### REQ-ENRICH-08 — Per-(category × entity × language × embedder) centroid table

The TCT constellation (already shipped) SHALL be populated by per-cell centroid construction over the Phase 0 mutation corpus + KnownGood baseline. Cells with N < 50 supporting examples use Bayesian shrinkage.

### REQ-ENRICH-09 — TCT Gτ guard at inference

For each `verify(patch, context)` call, the system SHALL run `gtau_check_chunks` against the predicted panel; per-chunk × per-embedder cosine < τ_m → guard violation entry → `Verdict::GuardRejected`.

### REQ-ENRICH-10 — Per-embedder threshold τ_m calibration

τ_m thresholds SHALL be calibrated for false-positive rate ≤ 5% on a held-out KnownGood split, per existing `calibrator.rs`. Recalibrated when constellation refresh fires.

### REQ-ENRICH-11 — Constellation freshness policy

A constellation cell is `fresh` if updated within 90 days AND fewer than 100 new training examples have arrived since last refresh. Stale cells SHALL be refreshed by the self-optimization scheduler's constellation_freshness_tick.

### REQ-ENRICH-12 — Pairwise MI matrix maintenance

The pairwise MI matrix between active embedders SHALL be recomputed weekly via `mejepa_audit_pairwise_mi` and persisted to `/mnt/d/contextgraph/exports/pairwise-mi/<date>.csv`. Slots with MI > 0.85 ceiling are flagged.

### REQ-ENRICH-13 — Failure-exemplar reverse index

`CF_MEJEPA_FAILURE_EXEMPLARS` keyed by `(failure_mode_class, embedder_violation_signature)` SHALL be populated from corpus failures. Used by the prediction's `closest_exemplars` field.

### REQ-ENRICH-14 — Per-language stdlib registry

A small per-language YAML registry of standard solutions (e.g., "tenacity for retry-with-backoff", "logging instead of print") SHALL inform the `ReinventedWheel` redundancy class. Registry under `crates/context-graph-mejepa-instruments/assets/stdlib_registry/<language>.yml`.

### REQ-ENRICH-15 — Cross-embedder anomaly detection

For each shift, the system SHALL compute `cross_embedder_anomaly_z_score` per chunk per embedder pair. High-magnitude z-scores feed the failure-mode head as auxiliary input.

## Acceptance criteria

1. `EmbedderForward::forward(chunk)` for all 12 active content embedders returns valid real outputs that match the expected output for a deterministic test chunk SHA. Pretrained wrappers must prove SHA-pinned artifact readback; algorithmic wrappers must prove reproducible mathematical output.
2. DDA signal computation for a 30-chunk patch persists 30 `DdaSignals` rows to `CF_MEJEPA_DDA_SIGNALS`; each row has N per-embedder values and strict-upper-triangle pairwise arrays matching `N × (N - 1) / 2`.
3. TCT Gτ guard for a synthetic "intentionally bad" chunk fires `Verdict::GuardRejected` with the expected violating embedders.
4. VRAM peak under all 12 embedders loaded < 10 GB.
5. Embedder cache hit rate > 90% on a session with 10 small edits to the same chunk.
6. Pairwise MI heatmap rendered weekly to `/mnt/d/contextgraph/exports/pairwise-mi/<date>.csv`.

## Edge cases

- **Chunk too small to embed (< 16 chars)**: fail closed before persistence with a structured invalid-input error unless the caller has explicitly produced a validated degenerate-chunk record in the schema. Do not write silent zero vectors.
- **Chunk too large (> max model context)**: split into sub-chunks via the existing AST splitter; aggregate via mean pooling.
- **Embedder model file missing**: fail closed at startup with `MEJEPA_EMBED_WEIGHT_MISSING` or `MEJEPA_EMBED_FORWARD_FAILED`, depending on whether the missing file is caught during digest registration verification or runtime artifact verification.
- **Routing returns empty embedder set for an unknown (Language × EntityType)**: fail closed with a structured routing error; no panel is produced for it.
- **MI matrix not yet computed (cold start)**: require an explicit baseline record in the source of truth; do not silently synthesize priors.

## Non-functional

- Per-shift DDA computation < 200ms (cached MI; only fresh cosines).
- Embedder cache memory bounded at < 2 GB.
- Constellation refresh < 5 min for an updated cell.
