# 06 — Embedder Substrate (E1–E21)

Source files covered: `crates/context-graph-mejepa-embedders/src/{lib,routing,digest,vram,calibration,ensemble,forward,embedder_id,types,config}.rs`, `crates/context-graph-embeddings/src/{models/mod,traits/mod,provider/,gpu/,training/}.rs`, `crates/context-graph-core/src/types/`, `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md`, `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md`, `config/models_config.toml`.

---

## 1. The 21 embedder slots

| ID | Name | Dim | Family | Active for code? | VRAM (fp16) | Source family |
|---|---|---|---|---|---|---|
| E1 | e5-large-v2 (general semantic) | 1024 | dense general-purpose | YES | 0.6 GB | `intfloat/e5-large-v2` |
| E2 | sinusoidal time-PE | 512 | temporal positional | partial | 0 | deterministic |
| E3 | recurrence time-PE | 512 | temporal cycle | partial | 0 | deterministic |
| E4 | session-position-PE | 512 | session ordering | partial | 0 | deterministic |
| E5 | causal asymmetric | 768 | **RETIRED** | NO | — | post-2026-05-09 retire |
| E6 | sparse-keyword (BM25-ish) | 30k sparse | sparse lexical | YES | 0 | codebook |
| E7 | Qodo-Embed (code-specific) | 1536 | dense code | YES | 1.5 GB | `Qodo/Qodo-Embed-1-1.5B` |
| E8 | graph-attention (code graph) | 1024 | graph | YES | <1 GB | adapter on E7 |
| E9 | HDC (hyperdimensional) | 1024 | binary high-dim | YES | <0.1 GB | codebook |
| E10 | multimodal-paraphrase (doc/query asym) | 1024 | dense paraphrase | YES | 0.7 GB | `intfloat/multilingual-e5-large` |
| E11 | KEPLER entity | 768 | **RETIRED** | NO | — | post-2026-05-09 retire |
| E12 | ColBERT (token-level multi-vec) | 128/token | late-interaction | YES (selective) | 0.5 GB | `colbert-ir/colbertv2.0` |
| E13 | SPLADE (sparse expansion) | 30k sparse | sparse expansion | YES (sparse) | 0.4 GB | `naver/splade-v3` |
| E14 | semantic-code (CodeT5+) | 1536 | dense code | YES | 0.4 GB | `Salesforce/codet5p-110m-embedding` |
| E15 | learner — affect (HRV-style) | 256 | learner-state | NO (non-code) | <0.2 GB | adapter |
| E16 | learner — focus | 256 | learner-state | NO (non-code) | <0.2 GB | adapter |
| E17 | learner — affect-text (E_Reasoning input) | 768 | learner-state | YES | 0.5 GB | `cardiffnlp/twitter-roberta-base-sentiment-latest` |
| E18 | learner — context novelty | 256 | learner-state | partial | <0.2 GB | adapter |
| E19 | learner — load (cognitive load proxy) | 256 | learner-state | NO (non-code) | <0.2 GB | adapter |
| E20 | learner — confidence | 256 | learner-state | partial | <0.2 GB | adapter |
| E21 | learner — generative-style | 256 | learner-state | NO (non-code) | <0.2 GB | adapter |

**Active set for ME-JEPA-Code**: {E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17} = **13 embedders**. E18/E20 in routing at low weight. **Total active VRAM at fp16 ≈ 6.5 GB** (RTX 5090 has 32 GB; ample headroom).

---

## 2. Unique contribution per embedder

Source: `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md §2`.

| Embedder | Unique contribution | Example concern surfaced |
|---|---|---|
| E1 | What the code does in NL terms | docstring↔body drift |
| E2 | Recency | weight recent edits higher |
| E3 | Cyclic timing pattern | rushed-Friday-commit detection |
| E4 | Position within agent session | over-thinking around edit 15-20 |
| E6 | Exact identifier match (typo-resistant via sparse) | new identifier `_walk_compaund` has no callsite |
| E7 | Code semantics (Qodo) | implementation matches buggy exemplar at 0.91 cosine |
| E8 | Call graph / def-use / control flow | new function calls helper with wrong arg order |
| E9 | Surface-form fingerprint | duplicated code block at 0.97 surface |
| E10 | Comment ↔ code paraphrase | commit message disagrees with diff |
| E12 | Per-token fine-grained matching | `Model.coeff_` symbol resolution |
| E13 | Term expansion | finds 12 related callsites needing update |
| E14 | Ensemble diversity vs E7 | blind-spot when E7 vs E14 disagree |
| E17 | Tone of agent prose | hedging detection for reasoning class |

---

## 3. Per-failure-mode embedder contribution (cross-reference)

Source: `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md §3`. Full table in [`17_prediction_taxonomy_reference.md §3`](17_prediction_taxonomy_reference.md). Highlights:

| Failure / concern | Leading embedders |
|---|---|
| WrongAlgorithm / OffByOne / NullDeref | E7, E8, E14 |
| TypeError / SignatureMismatch | E8 + E12 |
| ImportMissing | E6 + E12 |
| SyntaxError | E_AST instrument (deterministic; embedder-independent) |
| ResourceLeak / RaceCondition | E8 + E_StaticAnalysis |
| EdgeCase | E_DataFlow + E8 + symbolic-execution proxy |
| ShadowedVariable / InconsistentNullCheck | E_StaticAnalysis + E_DataFlow |
| DuplicatedCode | E7 + E14 + E9 |
| Perf regression | scalar bench delta + E8 (loop pattern) |
| Security (SQL/Cmd/XSS injection) | E_StaticAnalysis + E12 |
| Reasoning class | E17 + pairwise[E_Reasoning][E_Diff] / [E_Oracle] |

---

## 4. Routing table

Source: `crates/context-graph-mejepa-embedders/src/routing.rs`. **143 rows** covering 11 languages × ~13 entity types. Example rows:

| Language | EntityType | Active embedder set |
|---|---|---|
| Python | Function | {E1, E7, E10, E12, E14} |
| Python | Class | {E1, E7, E8, E10, E14} |
| Python | Module | {E1, E10, E14} |
| Python | AsyncFunction | {E1, E7, E10, E12, E14, E18} (low-weight novelty) |
| Rust | Function | {E1, E7, E8, E10, E12, E14} |
| Rust | Impl | {E1, E7, E8, E14} |
| Rust | Macro | {E1, E7, E9} (HDC for surface fingerprint) |
| TS | Interface | {E1, E7, E8, E10, E12} |
| C++ | Template | {E1, E7, E8, E12} |
| (any) | Comment | {E1, E10} |
| (any) | DocString | {E1, E10, E17} |

`mejepa_routing_lookup` MCP tool exposes this table. Retired slots (E5, E11) fail closed with `MEJEPA_EMBEDDER_RETIRED` if ever called.

---

## 5. Public API

### 5.1 `Instrument` trait (used by ME-JEPA-instruments)

```rust
pub trait Instrument: Send + Sync {
    fn slot(&self) -> InstrumentSlot;
    fn output_dim(&self) -> usize;
    fn encode(&self, event: &TypedEvent) -> Result<Vec<f32>, InstrumentError>;
}
```

### 5.2 `EmbedderForward` trait (in `forward.rs`)

```rust
pub trait EmbedderForward: Send + Sync {
    fn embedder_id(&self) -> EmbedderId;
    fn output_dim(&self) -> usize;
    fn forward(&self, input: &EmbedderInput) -> Result<Vec<f32>, EmbedderError>;
}

pub struct PretrainedEmbedderForward {
    embedder_id: EmbedderId,
    model_path: PathBuf,
    digest_sha256: [u8; 32],
    runtime: CandleRuntime,
    tokenizer: Tokenizer,
    vram_reserved_gb: f32,
}
```

Wrappers exist for E1, E6, E7, E8, E10, E12, E13, E14. E10 has full FSV. Remaining work: full per-embedder FSV + cache + production integration ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)).

### 5.3 `EmbedderBackedInstrumentHead` (in `forward.rs`)

```rust
pub struct EmbedderBackedInstrumentHead {
    base: Box<dyn EmbedderForward>,
    projection: LinearProjection,
    target_slot: InstrumentSlot,
}
```

Used to map E7/E1/E14/E10 outputs into code/text/trace panel slots with source-id checks and fail-closed analyzer-feature requirements.

---

## 6. Sourcing, digest verification, retirement policy

Source: `crates/context-graph-mejepa-embedders/src/digest.rs`.

- **Models live under `/mnt/d/cabdru/contextgraph/models/<embedder_id>/`** (separate from the 115 GB hot quota).
- **`models_config.toml`** declares per-embedder: model path, SHA-256 digest, dim, family, tokenizer path, VRAM cost.
- **At daemon startup**, the digest is verified for every active embedder. Mismatch → `MEJEPA_EMBEDDER_DIGEST_MISMATCH`; daemon refuses to start.
- **Retired slots (E5, E11)** are not loaded; calls to them fail closed with `MEJEPA_EMBEDDER_RETIRED`.
- **E17 requires a calibration cert** at `data/utl_calibration/e17/` (per `calibration.rs`); load fails closed otherwise.
- **Clean-room rule** per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`: never vendor or path-dep on `ruvnet/agentdb` or `ruvnet/ruvector`. Reimplement clean-room only. Clones are read-only inspiration.

---

## 7. VRAM budget gate

Source: `crates/context-graph-mejepa-embedders/src/vram.rs`.

```
VramBudget {
    available_gb: f32,    // measured from nvidia-smi at startup
    reserved_gb: f32,     // budget = available - reserved headroom
    per_embedder: HashMap<EmbedderId, f32>,
}
```

- Default reserved headroom: 3.0 GB (for predictor activations + KV-cache + workspaces).
- Refuse start if `sum(per_embedder × active) + reserved > available`.
- `mejepa_vram_budget_report` MCP tool returns the breakdown.
- **WSL GPU telemetry hardening** is futurebuild ([#10](https://github.com/ChrisRoyse/contextgraph/issues/10)).

---

## 8. Embedder cache (per-(embedder, chunk_sha))

Futurebuild Phase D, `TASK-ENRICH-004`. Cache size 5 GB default (`embedder.cache_size_gb`). 100% hit on re-embedding the same chunk → near-zero re-cost. Lives under `/mnt/d/contextgraph/storage/embedder-cache/`.

---

## 9. Lazy loading

- **E15, E16, E19, E21** (non-code learner-state) are lazy-loaded — only loaded when a non-code domain prediction is requested.
- **E17, E18, E20** are loaded eagerly when code-domain routing requires them.
- **Active code-domain set** (13 embedders) loads at daemon startup; warmup CLI command (`context-graph-cli warmup`) pre-loads all 14 production models in 20-30 seconds on RTX 5090.

---

## 10. Per-embedder asymmetry (where applicable)

Source: `crates/context-graph-core/src/types/model_id/core.rs` + `crates/context-graph-core/src/similarity/`.

| Embedder | Asymmetry | Storage |
|---|---|---|
| E5 (retired) | cause / effect dual vector | dual CF (`CF_E5_CAUSAL_CAUSE`, `CF_E5_CAUSAL_EFFECT`) |
| E8 | source / target dual vector | dual storage |
| E10 | doc / query dual vector | dual storage |

E5 direction-mask was 1.2× boost for cause→effect, 0.8× for effect→cause (retired; CF kept for repair).

---

## 11. The cross-embedder blind-spot detection

Source: `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md §6`.

```rust
fn cross_embedder_anomaly(chunk, panel) -> Vec<BlindSpot> {
    for (m1, m2) in unique_pairs(active_embedders) {
        let c = cosine(panel.per_chunk_emb[chunk][m1],
                       panel.per_chunk_emb[chunk][m2]);
        let mu = expected_pairwise[m1][m2];          // baseline on KnownGood corpus
        let z = (c - mu) / sigma_pairwise[m1][m2];
        if z.abs() > BLIND_SPOT_Z_THRESHOLD {
            yield BlindSpot { chunk, m1, m2, observed_cosine: c, expected_cosine: mu, z_score: z };
        }
    }
}
```

Surfaced via `search_cross_embedder_anomalies` MCP tool (shipped). Becomes a 144-d auxiliary input to the failure-mode head in futurebuild Phase B (`TASK-PREDICT-006-blind-spot-features`).

---

## 12. ModelId vs NUM_EMBEDDERS gotchas

Per `MEMORY.md`:

- `ModelId` has **14 variants**; `NUM_EMBEDDERS = 13`.
- `ModelId::Kepler` (E11) is the 14th ModelId but shares the E11 weight slot.
- `ModelId::Entity` (legacy MiniLM, 384-d) is NOT the same as the E11 KEPLER (768-d production); changing one breaks `EntityModel` tests.
- `MODEL_COUNT ≠ NUM_EMBEDDERS`. Always verify.

---

## 13. Pairwise MI audit (REQ-UTML-06, shipped)

Source: `crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi_matrix.rs`.

- Recompute weekly across rolling holdout split.
- Persist CSV at `/mnt/d/contextgraph/exports/pairwise-mi/<date>.csv`.
- MCP tool: `mejepa_audit_pairwise_mi`.
- Heatmap renderer is futurebuild (`TASK-OBS-002`).
- **Redundancy ceiling**: `MI > 0.85` between slots → flag for operator decision (retire / recalibrate / accept).

---

## 14. What this substrate commits to

1. Keep all 13 active code-domain embedders in routing; drop none (pairwise diversity = blind-spot signal).
2. SHA-pinned model digests; daemon fails closed on mismatch.
3. VRAM budget gate; daemon fails closed on overspend.
4. Retired slots fail closed; do not return placeholders.
5. Cache layer for per-(embedder, chunk_sha) re-edits.
6. Cross-references: [`05_ast_chunker_subsystem.md`](05_ast_chunker_subsystem.md) (input chunks), [`07_instruments_panel.md`](07_instruments_panel.md) (Panel slots backed by embedders), [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) (DDA signals consume embedder outputs).
