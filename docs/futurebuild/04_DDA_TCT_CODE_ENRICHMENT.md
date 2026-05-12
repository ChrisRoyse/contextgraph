# 04 — DDA / TCT Code Enrichment Pipeline

> **Purpose.** Specify how arbitrary code chunks are exploded into the dense, multi-embedder, multi-signal training/inference substrate that makes ME-JEPA's multi-head prediction possible. Builds on `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` (the 21-embedder ensemble) + `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` (the constellation/Gτ math) + `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` (the AST-perfect chunker).
>
> **The slogan:** **every byte of reality becomes hundreds of labeled training examples.** This document specifies the exact pipeline that delivers that.

**Status snapshot (2026-05-12):** The storage substrate (`CF_MEJEPA_DDA_SIGNALS`) and Phase B typed prediction surfaces are shipped, but production DDA signal computation and real neural feature consumption remain Phase D. Current Phase B predictions use deterministic patch/panel surfaces and must not be treated as trained DDA-backed head quality.

---

## 1. The signal-count math (the operator's "blow up the code")

### 1.1 DDA counting identity

Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.1` (Royse 2026c §3.1): for an input `x` viewed under `N` frozen embedders, every input yields up to

```
n · (N + N²)  structured signals
```

at `n` aggregation points (chunks, lines, paragraphs, ...) where the `N` is per-embedder cosine to its constellation centroid and the `N²` is the pairwise cosine + MI matrix between embedder outputs.

For **N=21** (the full ContextGraph ensemble), this is **n × 462 signals per input**. For a typical SWE-bench Lite mutation entry of ~30 chunks, that's **~13,860 signals per training example.** That is the operator's "insane amount of reward signals from durable reality."

For ME-JEPA's *active* code-domain set (N=12 active content embedders + 7 learner-state per `docs/ruvectorfindings/specs/_README.md` §1; we use the 12 active for code-reality prediction), the math is **n × (12 + 144) = n × 156 signals per input**, or ~4,680 per typical patch.

### 1.2 What each signal carries

| Signal class | Count per chunk | What it measures |
|---|---|---|
| Per-embedder cosine to centroid (N) | 12 | how close the chunk's embedder-m output is to the (category × entity × language × embedder) centroid for that slot — **direct surface for "this chunk looks anomalous to embedder m"** |
| Pairwise cosine between embedders (N²) | 144 | whether two embedders agree on the chunk; high disagreement is a red flag for blind-spot-where-one-embedder-misses |
| Pairwise mutual information between embedders (cached, recomputed periodically) | 144 | structural redundancy across embedders — used by the ΔΞ.predictor_redundancy term |
| **Total per chunk** | **300** | |
| For ~30 chunks per typical patch | **~9,000** | dense signal budget per training example |

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
│   Real neural forward (CodeT5+, Qodo-Embed, GTE, BGE-M3, CodeBERT, ...)    │
│   per doc 12 §6.1 cache key = (embedder_id, chunk_sha) → 100% hit on       │
│   re-embedding the same chunk → near-zero re-cost                           │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ Step 4 — DDA signal computation (NEW)                                      │
│   for each chunk:                                                          │
│     - cosine_to_centroid[m] = cosine(emb_m(chunk),                         │
│                                       constellation[category, kind, lang, m]) │
│     - pairwise_cosine[m1][m2] = cosine(emb_m1(chunk), emb_m2(chunk))       │
│     - pairwise_mi[m1][m2] = cached MI matrix (recomputed weekly)           │
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

Steps 1, 2, 3 (with deterministic placeholders), 5, and 6 are shipped today. Step 3's neural forward and Step 4's DDA signal computation are the futurebuild work for this domain.

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
    pub freshness_ts: SystemTime,             // refresh policy: stale > 90 days → refuse without operator override
}
```

**Per the corpus geometry**, cells form a 4-D table:
- 8 mutation categories × Q4 add ones for non-test categories
- ~13 entity types per language (function, class, impl, module-decl, type-alias, trait, macro, ...)
- 11 languages
- 12 active embedders

≈ 8 × 13 × 11 × 12 = **13,728 cells.** Many will have `N < 50` and be Bayesian-shrunk; some combinations are physically impossible (e.g., `impl` for Python) and are absent. The current `TctConstellation` type already supports this layout per `crates/context-graph-mejepa-tct/src/constellation.rs`.

**Refresh cadence**:
- The continuous self-optimization daemon refreshes a centroid when (a) > 100 new training examples have landed in that cell since last refresh, OR (b) per-embedder threshold τ_m calibration drifts > 3% on the rolling validation window, OR (c) the cell's freshness is > 90 days. Operator override (via `mejepa_constellation_inspect` MCP tool) can force a refresh at any time.

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

---

## 6. The "blow up code" pipeline as a runnable example

Walking through what happens for a single Edit:

1. **Edit lands.** `astropy/modeling/separable.py` modified at lines 62-118.
2. **Hook captures.** PostToolUse computes after_sha = `e3a4...`, runs `python -c "import ast; ast.parse(open('astropy/modeling/separable.py').read())"`, appends shift-log entry.
3. **Subscriber tails.** Within 250ms, the shift subscriber reads the line.
4. **Chunker runs.** `memory::ast::chunk_with_options(...)` returns `[Chunk(separable.py:1-25, Module, Python), Chunk(separable.py:27-58, Function, "def is_separable"), Chunk(separable.py:62-118, Function, "def _walk_compound"), Chunk(separable.py:120-150, Function, "def separability_matrix"), ...]` — say 12 chunks for the file.
5. **Routing.** Each chunk gets its embedder set per the routing table. For `Function` in Python, the active set is `{E1, E7, E10, E12, E14}` (5 embedders). For `Module`, the set is `{E1, E10, E14}`. For `Comment`, `{E1, E10}`.
6. **Forward pass.** For each (chunk, embedder), the EmbedderForward returns a vector. Cache hit on chunks unchanged from last shift; cache miss on the modified chunks. Total: ~30 forward passes for the modified region.
7. **DDA signals.** For each chunk: 5 cosine-to-centroid scalars + 25 pairwise-cosines + 25 cached pairwise-MI = 55 signals per chunk × 12 chunks = **660 signals for this file.** Persist to `CF_MEJEPA_DDA_SIGNALS`.
8. **Gτ guard.** Each chunk's 5 cosines vs the corresponding `(predicted_category, entity_kind, language, embedder)` τ_m thresholds. The chunk at lines 62-118 violates on E_TypeGraph and E_DataFlow; that's appended to `violating_chunks`.
9. **Panel.** The 15 instrument slots are populated by aggregating the chunk-level signals + scalar features. Total Panel = 5,120-d. Persist to `CF_MEJEPA_PANELS`.
10. **Predictor forward.** 6-layer transformer trunk + the 10 heads run a single forward pass over `(panel_t-1, action, panel_t)`. Outputs are assembled into `RealityPrediction` per the schema in `15_PREDICTION_TAXONOMY.md §1`.
11. **Persist.** `RealityPrediction` written to `CF_MEJEPA_LIVE_PREDICTIONS` keyed by `session_id`.
12. **Inject.** On the agent's next prompt, the UserPromptSubmit hook reads the latest prediction and renders the markdown block per `15_PREDICTION_TAXONOMY.md §8`.

The whole pipeline takes ≤ 5s p50 / 30s p99 per `REQ-SHIFT-03`, ensuring the prediction is available before the agent's next prompt.

---

## 7. What the per-language operator extension changed

As of 2026-05-12, the chunker handles all 11 languages and Phase C has a verified non-Python fixture corpus. The Python SWE-bench corpus remains the real Docker-oracle corpus; the 10 non-Python languages now have deterministic fixture corpora with real toolchain validation. Source of truth:

- Python: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` — 2,400 verified entries.
- Non-Python: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` — 5,120 verified entries, 512 per non-Python language, 64 per `(language × category)` cell.
- Combined Phase C split: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json` — `passes=true`, 940 tasks, bucket sizes `[752, 94, 94]`.

- **Rust**: 8 operators ported. `SubtleFlip` swaps `==` ↔ `!=`, `<=` ↔ `>=`, `&&` ↔ `||`. `OffByOne` perturbs integer literals. `SwapVariable` renames a local var. `DeleteTestCall` removes one `assert!` / `assert_eq!`. `WrongFile` patches a sibling. `OverEngineer` appends an unused fn. `CompileError` introduces a syntax error. `KnownGood` echoes.
- **JS/TS**: same 8, JS/TS-aware. `SubtleFlip` swaps equality/comparison/logical operators and boolean literals while requiring the mutated source to remain parseable. `OffByOne` perturbs numeric literals. Etc.
- **Go**: same 8, Go-aware. `SubtleFlip` swaps `==` ↔ `!=`, `<` ↔ `>=`. `DeleteTestCall` removes one `t.Errorf` / `t.Fatal`. Etc.
- **Java**: same 8, Java-aware. `SubtleFlip` swaps `==` ↔ `!=` for primitives, `.equals` ↔ `!.equals` for objects. Etc.
- **C/C++/C#/Ruby/PHP**: same 8, language-aware.

The non-Python implementation is currently shared generic operator logic in `crates/context-graph-mejepa-corpus/src/generic_language_ops.rs`, exercised through `apply_mutation_for_language`, not one file per language. `generate-fixtures` produces fixture tasks and `phase-c-verify` verifies the combined Python + non-Python corpus. Future corpus broadening can add scraped/public CI corpora, but the Phase C ship gate no longer waits on that.

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

1. **Build neural per-embedder forward wrappers** so `EmbedderForward::forward(chunk)` actually runs the model instead of returning deterministic placeholders. (Task: `tasks/enrichment/TASK-ENRICH-001-neural-forward-wrappers.md`.)
2. **Wire DDA signal computation** as a step between embedder forward and panel assembly. (Task: `TASK-ENRICH-002-dda-signal-computation.md`.)
3. **Add `CF_MEJEPA_DDA_SIGNALS` and `CF_MEJEPA_FAILURE_EXEMPLARS` to the CF registry.** (Task: `TASK-ENRICH-003-new-cfs.md`.)
4. **Multi-head extensions** to the predictor that consume DDA signals as auxiliary features. (Task: `TASK-PREDICT-004-multi-head-stack.md`.)
5. **Per-language mutation operators** for the 10 non-Python languages are complete in Phase C. Future work here is corpus broadening, not the Phase C ship gate.
6. **Pairwise MI heatmap renderer** as an operator UI surface. (Task: `TASK-OBS-002-mi-heatmap-renderer.md`.)
7. **Constellation refresh trigger** wired into the continuous self-optimization daemon. (Task: `TASK-HEAL-005-constellation-refresh.md`.)

All listed in the task graph at [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
