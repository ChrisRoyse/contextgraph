# 04 — DDA / TCT Code Enrichment Pipeline

> **Purpose.** Specify how arbitrary code chunks are exploded into the dense, multi-embedder, multi-signal training/inference substrate that makes ME-JEPA's multi-head prediction possible. Builds on `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` (the 21-embedder ensemble) + `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` (the constellation/Gτ math) + `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` (the AST-perfect chunker).
>
> **The slogan:** **every byte of reality becomes hundreds of labeled training examples.** This document specifies the exact pipeline that delivers that.

**Status snapshot (2026-05-18):** The storage substrate (`CF_MEJEPA_DDA_SIGNALS`) and Phase B typed prediction surfaces are shipped. Phase D also ships the DDA compute/store/readback API, persistent embedder cache, 12-slot content forward surface, full pretrained/routing forward FSV, inference CF opener fix, stdlib registry FSV, and live shift-subscriber DDA persistence for real algorithmic forward outputs. TASK-PY-G-043/#262 now ships the per-head DDA projection table from §3 plus durable schema rows in `CF_MEJEPA_HEAD_PROJECTION_REGISTRY`; trained-head model-quality gains remain Phase G convergence work.

---

## 1. The signal-count math (the operator's "blow up the code")

### 1.1 DDA counting identity

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.1` (Royse 2026c §3.1): for an input `x` viewed under `N` frozen embedders, every input yields up to

```
n · (N + N²)  structured signals
```

at `n` aggregation points (chunks, lines, paragraphs, ...) where the `N` is per-embedder cosine to its constellation centroid and the `N²` is the pairwise cosine + MI matrix between embedder outputs.

For **N=21** (the full ContextGraph ensemble), this is **n × 462 signals per input**. For a typical SWE-bench Lite mutation entry of ~30 chunks, that's **~13,860 signals per training example.** That is the operator's "insane amount of reward signals from durable reality."

For ME-JEPA's *active* code-domain set, the live content embedders are **E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14**. E17 is a learner-state/affect slot, not a content embedder in the live registry.

The theoretical Royse identity still describes the dense feature budget. The persisted `DdaSignals` schema intentionally stores **strict upper triangles** for pairwise arrays to avoid duplicated symmetric entries:

- per-embedder cosine: `N = 12`
- pairwise cosine upper triangle: `N × (N - 1) / 2 = 66`
- pairwise MI upper triangle: `66`
- blind-spot z-score upper triangle: `66`

So the current persisted DDA block has **210 scalar values per chunk** for N=12, or **6,300 scalar values across a 30-chunk FSV patch**. If a downstream feature projection needs a full `N²` view, it reconstructs the symmetric matrix from the upper triangle.

### 1.2 What each signal carries

| Signal class | Count per chunk | What it measures |
|---|---|---|
| Per-embedder cosine to centroid (N) | 12 | how close the chunk's embedder-m output is to the (category × entity × language × embedder) centroid for that slot — **direct surface for "this chunk looks anomalous to embedder m"** |
| Pairwise cosine between embedders (strict upper triangle) | 66 | whether two embedders agree on the chunk; high disagreement is a red flag for blind-spot-where-one-embedder-misses |
| Pairwise mutual information between embedders (strict upper triangle, cached/recomputed periodically) | 66 | structural redundancy across embedders — used by the ΔΞ.predictor_redundancy term |
| Blind-spot z-scores (strict upper triangle) | 66 | normalized pairwise-disagreement features for auxiliary heads |
| **Total persisted per chunk** | **210** | |
| For 30 chunks in the Phase D FSV | **6,300** | persisted scalar DDA budget |

These signals are persisted to `CF_MEJEPA_DDA_SIGNALS` keyed by `(panel_id, chunk_id)` so they can be reused across training epochs without recomputation, and so the inference path's TCT Gτ guard can read them in O(1).

---

## 2. The pipeline, end-to-end

```
┌────────────────────────────────────────────────────────────────────────────┐
│ Source byte stream (file at after_sha)                                     │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 1 — AST-aware atomic chunking                                         │
│   memory::ast::chunk_with_options(file, language, opts)                    │
│     → Vec<Chunk { chunk_id, language, kind, byte_range, line_range, sha }> │
│   chunk boundaries respect AST entity boundaries (function, class, impl,   │
│   module-decl) per docs/ruvectorfindings/14 §3 (cAST / Qodo methodology)   │
│   target ~500 non-whitespace chars per chunk; bounded by AST node          │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 2 — Per-chunk routing                                                 │
│   for each chunk: routing::lookup(chunk.language, chunk.kind)              │
│     → Vec<EmbedderId>  (subset of the 12 active content embedders)         │
│   Routing table is 143 rows: per (Language × EntityType) → embedder set    │
│   Extension by per-chunk-type per doc 12 §3                                │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 3 — Per-chunk embedder forward pass                                   │
│   for each (chunk, embedder_id) in routing × chunks:                       │
│     EmbedderForward::forward(chunk) → EmbeddingVec                         │
│   Real forward: pretrained Candle/CUDA for E1/E6/E7/E8/E10/E12/E13/E14     │
│   and real algorithmic forward for E2/E3/E4/E9. Cache key includes         │
│   schema_version, embedder_id, model_version, and chunk_sha → hit on       │
│   re-embedding the same chunk → near-zero re-cost                           │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 4 — DDA signal computation (code/FSV shipped 2026-05-12)             │
│   for each chunk:                                                          │
│     - cosine_to_centroid[m] = cosine(emb_m(chunk),                         │
│                                       constellation[category, kind, lang, m]) │
│     - pairwise_cosine_upper[m1][m2] = cosine(projected_m1, projected_m2)   │
│     - pairwise_mi_upper[m1][m2] = cached MI matrix (strict upper triangle) │
│     - blind_spot_z_scores[m1][m2] = normalized pairwise disagreement       │
│   Persist to CF_MEJEPA_DDA_SIGNALS                                         │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 5 — TCT Gτ guard                                                      │
│   for each chunk × embedder m:                                             │
│     guard_pass[chunk][m] = cosine_to_centroid[m] ≥ τ_m                     │
│   chunk-level guard: any guard_pass[chunk][m] == false → chunk in          │
│   Verdict::GuardRejected.violating_chunks                                  │
│   per-embedder guard: any chunk fails for embedder m → embedder m in       │
│   Verdict::GuardRejected.violating_embedders                               │
│   shipped today as `gtau_check_chunks` in context-graph-mejepa-tct          │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 6 — PanelBuilder assembly                                             │
│   15 instrument slots, total 5,120-d:                                      │
│     - per-chunk DDA signals are aggregated per slot                        │
│     - E_AST = 384-d Qodo + AST-token-transformer over chunks               │
│     - E_CFG = 256-d E8 + graph-attention over basic blocks                 │
│     - ... (full table in 05_EMBEDDER_SIGNAL_MAP.md)                        │
│   Panel(5,120-d) materialized; persist to CF_MEJEPA_PANELS                 │
└────────────────────────────────────────────────────────────────────────────┘
```

Steps 1, 2, 5, and 6 are shipped. Step 3's real forward surface and Step 4's DDA compute/store/readback substrate are code/FSV-shipped, and the live shift subscriber writes DDA rows for the real algorithmic forward set. TASK-PY-G-043/#262 adds the deterministic per-head feature projection/registry layer over those rows; the remaining production work is proving trained-head quality improvement during Phase G convergence.

---

## 3. The role of each signal in the multi-head predictor

The multi-head prediction stack from `03_ARCHITECTURE_BLUEPRINT.md §3` consumes DDA signals as follows:

| Head | Primary DDA signals consumed | How they contribute |
|---|---|---|
| Panel head | per-chunk embedder vectors (raw) | the latent-prediction target |
| Oracle head | per-chunk cosine_to_centroid for E_Oracle, E_Test, E_Reasoning | direct correlation with test pass/fail |
| Failure-mode head | per-chunk cosine_to_centroid for E_AST, E_CFG, E_DataFlow, E_TypeGraph; pairwise_cosine[E_TypeGraph][E_DataFlow] | type/flow/structure disagreement → bug class |
| Edge-case head | per-chunk cosine_to_centroid for E_DataFlow + scalar `coverage_delta` | uncovered branch + flow signature |
| Tech-debt head | scalar features (complexity, length, nesting) + per-chunk cosine_to_centroid for E_AST | structural code smells |
| Perf head | per-chunk cosine_to_centroid for E_CFG (loops, hot paths) + scalar runtime aggregates | loop / call-stack signature |
| Security head | per-chunk cosine_to_centroid for E_StaticAnalysis + scalar SAST findings | direct binding to security analyzer outputs |
| Accuracy head | per-chunk cosine_to_centroid for E_Test + scalar metric deltas | for ML/data-pipeline test outputs |
| Cost head | scalar (CI minutes, token telemetry, dep count) | non-embedded scalars dominate |
| Reasoning head | pairwise_cosine[E_Reasoning][E_Diff], pairwise_cosine[E_Reasoning][E_Oracle] | does the agent's prose match the change + the outcome? |

**Architectural note.** Each head reads the same shared trunk output AND a small head-specific projection of the DDA signals it cares about. The DDA signals act as auxiliary features that the trunk transformer cross-attends to via a learned attention head. The trunk's parameter count doesn't grow with the signal count; only the head-side projections do.

**TASK-PY-G-043/#262 implementation note (2026-05-18).** `context_graph_mejepa::head_projection` materializes this table as a `PerHeadProjection` surface with fail-closed `HEAD_PROJECTION_MISSING_SLICE`, `HEAD_PROJECTION_SCHEMA_MISMATCH`, and `HEAD_PROJECTION_NO_DDA` errors. The source-of-truth schema registry is `CF_MEJEPA_HEAD_PROJECTION_REGISTRY`. FSV: `/mnt/d/contextgraph/fsv/task-py-g-043-per-head-dda-projection-fsv/run-1779078323947/head_projection_fsv.json` sha256 `dff41ba10cffa744eaa7921e78b359ca4f022fd5945b0b8cc18b7ee639201784`.

---

## 4. Constellation construction (what the signals are compared against)

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §2`, the constellation is the per-(category × entity × language × embedder) centroid table built from the Phase 0 mutation corpus + KnownGood baseline. Each cell holds:

```rust
pub struct ConstellationCell {
    pub category: MutationCategory,           // KnownGood, SubtleFlip, OffByOne, ...
    pub entity_kind: EntityType,              // Function, Class, Impl, Module, ...
    pub language: Language,
    pub embedder: EmbedderId,
    pub centroid: EmbeddingVec,               // mean of training set's embedder-m output for cells matching (cat × ent × lang × emb)
    pub bayesian_shrunk: bool,                // when N < 50, shrunk toward the marginal
    pub n_supporting: u32,                    // number of training examples in the cell
    pub tau_m: f32,                           // per-embedder threshold calibrated for false-positive rate ≤ 5% on heldout KnownGood
    pub last_refresh_ts: SystemTime,          // refresh policy: stale > 90 days → refuse without operator override
    pub examples_since_last_refresh: u32,     // refresh policy: >100 new examples → refit
    pub tau_m_drift_pct_rolling: f32,         // refresh policy: >3% rolling τ_m drift → refit
}
```

**Per the corpus geometry**, cells form a 4-D table:
- 8 mutation categories × Q4 add ones for non-test categories
- ~13 entity types per language (function, class, impl, module-decl, type-alias, trait, macro, ...)
- 11 languages
- 12 active embedders

≈ 8 × 13 × 11 × 12 = **13,728 cells.** Many will have `N < 50` and be Bayesian-shrunk; some combinations are physically impossible (e.g., `impl` for Python) and are absent. The current `TctConstellation` type already supports this layout per `crates/context-graph-mejepa-tct/src/constellation.rs`.

**Refresh cadence**:
- The continuous self-optimization daemon refreshes a centroid when (a) > 100 new training examples have landed in that cell since last refresh, OR (b) per-embedder threshold τ_m calibration drifts > 3% on the rolling validation window, OR (c) the cell's freshness is > 90 days. TASK-RWD-212 materializes this as compatibility-safe per-cell views plus durable `CF_MEJEPA_CONSTELLATION_REFRESH_LOG` rows rather than changing the frozen constellation payload layout. Operator override records `MANUAL_OVERRIDE_FRESHNESS_BYPASS`; stale target cells fail closed at inference without an explicit override.

---

## 5. The Gτ guard at inference time

For each `verify(patch, context)` call:

```python
def gtau_check_panel(predicted_panel, constellation):
    # mejepa-tct gtau_check_chunks (shipped) — pseudocode
    violations = []
    for chunk in predicted_panel.chunks:
        for embedder_m in active_embedders:
            cell = constellation.lookup(
                category="predicted",         # we're guarding the prediction itself
                entity_kind=chunk.kind,
                language=chunk.language,
                embedder=embedder_m,
            )
            cosine = cosine(predicted_panel.per_chunk_emb[chunk][embedder_m],
                            cell.centroid)
            if cosine < cell.tau_m:
                violations.append(GuardViolation(
                    embedder=embedder_m,
                    chunk=chunk.id,
                    centroid_id=cell.id,
                    cosine=cosine,
                    threshold_tau_m=cell.tau_m,
                    deficit=cell.tau_m - cosine,
                ))
    return violations
```

If `len(violations) > 0`, the verdict becomes `GuardRejected` regardless of the headline Oracle prediction. **This is structurally separate from the Oracle prediction**: it asserts that the predicted panel's per-embedder geometry sits inside the manifold of "acceptable predictions" for this kind of chunk in this language. Per `docs/ruvectorfindings/13 §1.4`, the optimizer cannot Goodhart against this — there's no gradient flowing through the constellation centroids; they're descriptive, not learned.

Per `docs/ruvectorfindings/specs/_README.md`, this is shipped as `mejepa_constellation_inspect` MCP tool + chunk-level localization via `gtau_check_chunks --lib` test.

**Durable refresh evidence (2026-05-12):** `crates/context-graph-mejepa-tct/examples/tct_centroid_refresh_fsv.rs` now refits a constellation from the real 2,400-row SWE-bench Lite Python corpus index plus the 300-task Python AST FSV entity-family evidence. The source of truth is RocksDB: `CF_MEJEPA_CONSTELLATION` and `CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS` under `/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv/tct-refresh-rocksdb`. The persisted `ConstellationRefreshReport` records:

- 2,400 source corpus rows and 10,560 AST-derived chunk-family observations.
- all 8 mutation categories, Python language support, pass/fail oracle support, and per-entity chunk support.
- 1,008 `(mutation, entity_type, language, embedder)` centroid-support cells with shrinkage origin and sample counts.
- reward-signal summary (`estimated_reward_scalar_count=244266`) and operator-visible rejected chunk diagnostics (`inspected=2`, `rejected=1`, `violating_embedder_count=1`, finite worst margin).

FSV readback: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv/tct-centroid-refresh-evidence.json` (`passes=true`, SHA-256 `e9dd03b6ad345632e1ec9a798dea78aaf4d3d5f12181b55636e80a6e32ca270a`). Edge cases for empty cell support, category-support mismatch, tampered report id, and zero report limit fail closed without increasing the refresh-report row count.

---

## 6. The "blow up code" pipeline as a runnable example

Walking through what happens for a single Edit:

1. **Edit lands.** `astropy/modeling/separable.py` modified at lines 62-118.
2. **Hook captures.** PostToolUse computes after_sha = `e3a4...`, runs `python -c "import ast; ast.parse(open('astropy/modeling/separable.py').read())"`, appends shift-log entry.
3. **Subscriber tails.** Within 250ms, the shift subscriber reads the line.
4. **Chunker runs.** `memory::ast::chunk_with_options(...)` returns `[Chunk(separable.py:1-25, Module, Python), Chunk(separable.py:27-58, Function, "def is_separable"), Chunk(separable.py:62-118, Function, "def _walk_compound"), Chunk(separable.py:120-150, Function, "def separability_matrix"), ...]` — say 12 chunks for the file.
5. **Routing.** Each chunk gets its embedder set per the routing table. For `Function` in Python, the active set is `{E1, E7, E10, E12, E14}` (5 embedders). For `Module`, the set is `{E1, E10, E14}`. For `Comment`, `{E1, E10}`.
6. **Forward pass.** For each (chunk, embedder), the EmbedderForward returns a vector. Cache hit on chunks unchanged from last shift; cache miss on the modified chunks. Total: ~30 forward passes for the modified region.
7. **DDA signals.** For a Python `Function` chunk routed to 5 content embedders: 5 cosine-to-centroid scalars + 10 strict-upper pairwise-cosines + 10 cached pairwise-MI values + 10 blind-spot z-scores = 35 persisted scalars per chunk. If all 12 chunks used the same 5-embedder route, that would be **420 persisted scalars for this file**. Persist to `CF_MEJEPA_DDA_SIGNALS`.
8. **Gτ guard.** Each chunk's routed cosines are checked against the corresponding `(predicted_category, entity_kind, language, embedder)` τ_m thresholds. A chunk at lines 62-118 might violate on E7/E14 code-semantics thresholds while deterministic instruments add E_TypeGraph/E_DataFlow context; the violating chunk ids are appended to `violating_chunks`.
9. **Panel.** The 15 instrument slots are populated by aggregating the chunk-level signals + scalar features. Total Panel = 5,120-d. Persist to `CF_MEJEPA_PANELS`.
10. **Predictor forward.** 6-layer transformer trunk + the 10 heads run a single forward pass over `(panel_t-1, action, panel_t)`. Outputs are assembled into `RealityPrediction` per the schema in `15_PREDICTION_TAXONOMY.md §1`.
11. **Persist.** `RealityPrediction` written to `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `session_id`.
12. **Inject.** On the agent's next prompt, the UserPromptSubmit hook reads the latest prediction and renders the markdown block per `15_PREDICTION_TAXONOMY.md §8`.

The whole pipeline takes ≤ 5s p50 / 30s p99 per `REQ-SHIFT-03`, ensuring the prediction is available before the agent's next prompt.

---

## 7. What the per-language operator extension changed

As of 2026-05-12, the chunker handles all 11 languages and Phase C has a verified archived non-Python fixture corpus. The Python SWE-bench corpus remains the active real Docker-oracle corpus; the 10 non-Python languages have deterministic fixture corpora with real toolchain validation for future generalization. Source of truth:

- Python: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` — 2,400 verified entries.
- Archived non-Python: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` — 5,120 verified entries, 512 per non-Python language, 64 per `(language × category)` cell.
- Combined Phase C split: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json` — `passes=true`, 940 tasks, bucket sizes `[752, 94, 94]`.

**Active execution scope:** Phase G consumes the Python SWE-bench corpus as the ship corpus. Non-Python fixture rows are archived future-generalization evidence and must not drive the current ME-JEPA-Code review-replacement gate.

- **Rust**: 8 operators ported. `SubtleFlip` swaps `==` ↔ `!=`, `<=` ↔ `>=`, `&&` ↔ `||`. `OffByOne` perturbs integer literals. `SwapVariable` renames a local var. `DeleteTestCall` removes one `assert!` / `assert_eq!`. `WrongFile` patches a sibling. `OverEngineer` appends an unused fn. `CompileError` introduces a syntax error. `KnownGood` echoes.
- **JS/TS**: same 8, JS/TS-aware. `SubtleFlip` swaps equality/comparison/logical operators and boolean literals while requiring the mutated source to remain parseable. `OffByOne` perturbs numeric literals. Etc.
- **Go**: same 8, Go-aware. `SubtleFlip` swaps `==` ↔ `!=`, `<` ↔ `>=`. `DeleteTestCall` removes one `t.Errorf` / `t.Fatal`. Etc.
- **Java**: same 8, Java-aware. `SubtleFlip` swaps `==` ↔ `!=` for primitives, `.equals` ↔ `!.equals` for objects. Etc.
- **C/C++/C#/Ruby/PHP**: same 8, language-aware.

The archived non-Python implementation is shared generic operator logic in `crates/context-graph-mejepa-corpus/src/generic_language_ops.rs`, exercised through `apply_mutation_for_language`, not one file per language. `generate-fixtures` produces fixture tasks and `phase-c-verify` verifies the combined Python + archived non-Python corpus. Future corpus broadening can add scraped/public CI corpora, but the active Phase G ship gate no longer waits on that.

---

## 8. The pairwise MI audit (REQ-UTML-06, shipped)

Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.3 (b)` and `13 §3.5.2`, the pairwise mutual information matrix between embedder outputs is the operational form of the predictor-redundancy diagnostic. Shipped today as `learning_signal/pairwise_mi_matrix.rs::PairwiseMiAuditor` with `mejepa_audit_pairwise_mi` MCP tool.

Operationally:
- The MI matrix is recomputed weekly across the rolling holdout split (per `12_IMPLEMENTATION_PHASES.md`).
- The CSV is persisted under `pairwise_mi_matrix.csv` for operator inspection.
- A heatmap renderer is the operator UI surface (proposed; small task).
- Slots whose MI to another slot is > 0.85 (the redundancy ceiling) are flagged. The operator can then decide whether to retire the redundant slot, recalibrate, or accept the redundancy.

The MI matrix is also a **diagnostic for collapse**: blocks of high off-diagonal MI between many slots indicate the predictor is using a low-rank shortcut. ME-JEPA's structural target-side immunity (target_collapse = 0 by construction) means this only manifests on the predictor side, where VICReg + the constellation guard catch it.

---

## 9. Storage costs and the budget envelope

Per `docs/ruvectorfindings/specs/_README.md` Phase 6, the system has a **115 GB hard quota** with nightly GC and tier compression.

| Data class | Size per entity | Total at 30k panels | Tier compression | After 90 days |
|---|---|---|---|---|
| Panel (5,120-d × f32) | 20 KB | 600 MB | 8-bit hot → 5 KB | 150 MB cold |
| DDA signals per chunk | ~60 × f32 = 240 B | ~30k × 30 chunks × 240 B = 216 MB | 5-bit cool → 67 B | 60 MB cold |
| Per-embedder vectors per chunk (cached) | 5 × 1024 × f32 = 20 KB | ~30k × 30 chunks × 20 KB = 18 GB | 5-bit cool → 5 KB | 4.5 GB cold |
| Constellation cells | ~13k cells × 1 KB | 13 MB | n/a | 13 MB |
| Training certs | ~2 KB / step | 100k steps × 2 KB = 200 MB | n/a | 200 MB |
| Weight blobs (deduplicated) | ~30 MB / variant | ~10 variants = 300 MB | n/a | 300 MB |
| Witness chain | 73 B / step | 100k × 73 B = 7 MB | Merkle compress @ 90d → 1 MB | 1 MB |
| Live predictions | 5 KB | ephemeral; bounded by session count | tier_demote(promote) | 100 MB warm |
| Agent feedback events | ~1 KB | bounded ~10k events | n/a | 10 MB |
| **Total** | | ~19 GB hot | | ~5.4 GB cold (90 days) |

**Headroom**: at 30 GB hot working set + ~70 GB long-tail cold storage, we sit comfortably under the 115 GB hard quota. Nightly GC + emergency eviction maintain this.

---

## 10. What this enrichment pipeline commits us to

1. **Build per-embedder forward wrappers** so `EmbedderForward::forward(chunk)` runs a real pretrained or algorithmic implementation instead of returning placeholders. The 12 active content forward surface and full pretrained/routing FSV are shipped.
2. **Wire DDA signal computation** as a step between embedder forward and panel assembly. The compute/store API, RocksDB FSV, and live shift-subscriber write/readback path are shipped for real algorithmic forward outputs.
3. **Open `CF_MEJEPA_DDA_SIGNALS` and `CF_MEJEPA_FAILURE_EXEMPLARS` in inference runtime.** Shipped 2026-05-12; this fixed the root cause where the CF constants existed but `open_infer_rocksdb` did not open them.
4. **Multi-head extensions** to the predictor that consume DDA signals as auxiliary features. Phase B schema/API code exists; trained DDA feature consumption remains open.
5. **Python mutation-category convergence first.** The 10 non-Python fixture operators are archived complete Phase C evidence, but Phase G work here is Python DDA/TCT reward-signal density, Q1-Q5 calibration, and the 8 Python mutation-category ship gate.
6. **Pairwise MI heatmap renderer** as an operator UI surface. (Task: `TASK-OBS-002-mi-heatmap-renderer.md`.)
7. **Constellation refresh trigger** wired into the continuous self-optimization daemon. (Task: `TASK-HEAL-005-constellation-refresh.md`.)

All listed in the task graph at [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
