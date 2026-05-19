# 05 — Predictor Pipeline (Embedders → Instruments → Panels → Heads → TCT → Training)

**Last refreshed:** 2026-05-19.

Source files covered: `crates/context-graph-mejepa-embedders/src/lib.rs` + modules, `crates/context-graph-mejepa-instruments/src/lib.rs` + modules, `crates/context-graph-mejepa/src/lib.rs` + `compiler.rs` + `verdict_assembly.rs` + `claim_graph.rs` + `patch_similarity.rs` + `failure_fingerprint.rs` + `project_ingest.rs` + `live_session_trace.rs`, `crates/context-graph-mejepa-tct/src/lib.rs` + modules, `crates/context-graph-mejepa-train/src/lib.rs` + `learning_signal.rs` + `replay_buffer.rs` + `compression_progress.rs` + `loss/inverse.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs` + ops, `crates/context-graph-mejepa-shift-subscriber/src/lib.rs`, `crates/context-graph-mejepa/src/heal/`.

---

## 0. The Predictor Contract (2026-05-19 reframe — binary + labeled + mistake-driven, no ambiguous heads)

This pipeline implements **three binary predicates** with free binary form against reality, plus one explanation surface, plus the deliberately-removed Q4 surface:

| # | Predicate | Reality that renders the verdict |
|---|---|---|
| Q1 | `claim_exists(candidate_patch) → { Yes, No }` | Bytes/AST on disk after the patch |
| Q2 | `oracle_passes(candidate_patch) → { Pass, Fail }` (**the product output**) | Docker oracle (`swebench.harness.run_evaluation`) report.json |
| Q5 | `predicted_shift_event_occurred(event_id, patch) → { Yes, No }` per leaf event | Durable shift-log JSONL on aiwonder |

Q3 is an **explanation surface only**, not a prediction: when Q2 returns Fail, Q3 attaches the named failure mode + closest exemplar from the labeling hierarchy (§4.3). Q3 itself does not produce a prediction subject to oracle correlation; it labels the binary verdict Q2 already made.

Q4 (perf, accuracy, cost, reasoning class, latent bugs, tech debt, dead code, redundancy, edge cases) is **out of the prediction contract**. Q4 questions are ambiguous by nature — they require imposing arbitrary thresholds on continuous heuristics ("perf regression ≥ 10%") or asking about taste judgments ("is this maintainable?") that reality does not render a binary verdict on. The system does not predict ambiguous things because predicting heuristics introduces noise into the mistake-driven loop that cannot be cleaned up by future ground-truth observations. Existing Q4 producers and calibration rows are **frozen as display-only historical data**; they are not trained, not consumed by the predictor, not ship-gate-relevant. See §16 below for the Q4 freeze.

The whole architecture below — embedders, instruments, panel, heads, TCT, training, heal — is in service of those three binary predicates. The 5-question `RealityPrediction` schema stays in `types.rs` for backward-compat, but the **only fields that determine product behavior are `Verdict::{Pass, Fail}` (Q2), `claim_reconciliation` (Q1), and per-leaf-event `predicted_shift_outcomes` (Q5)**.

Three architectural primitives back the binary contract:

1. **Labeling layer (§4.3 below).** Raw slot violations + cross-embedder correlations are clustered into a 4-level hierarchy of named failure modes (Action-Unit-style → Group → Skill → Constellation). The product returns the named failure mode that applies, grounded by the closest historical exemplar from the 300×8 corpus.
2. **Online mistake loop (§9.6 below).** When the binary prediction disagrees with reality (oracle / PostToolUse capture / shift-log replay), the disagreement materializes as a `CF_MEJEPA_MISTAKE_LOG` row and the trainer consumes it under EWC++ protection. The convergence target is: never repeat the same mistake on the same `(cell × failure_mode)` cell.
3. **Cross-panel triangulation (§4.4 below).** Panel B (a deliberately non-overlapping instrument panel) re-scores every gate-relevant patch. A 0.95 result on Panel A only counts when Panel B agrees within 0.05; Goodhart on Panel A fails closed.

**No best-of-K at the product surface.** `mejepa_rank_candidates` and `mejepa_search_latent_actions` remain available as *internal* tools for training-time exploration and operator debugging, but the AI agent calling MCP only ever sees one binary verdict per candidate. "Pass with 0.74 confidence" is not a product output; if the predictor cannot be confident enough to commit, the lack of confidence is itself a mistake to be learned from on the next ground-truth observation.

**Internal Abstain / OOD / GuardRejected are not product hedges.** They are present-tense failures to be eliminated. The path out is the mistake loop, not "try K alternatives." When the gate is at 0.95, the `Abstain` rate must asymptotically approach 0; an Abstain that ships to the agent is a wrong answer.

**The ambiguity boundary (locked-in rule).** ME-JEPA only predicts questions with a free binary form against reality. A question has free binary form when the verdict comes from observing reality directly (a test runner, the bytes on disk, the shift log), not from imposing an arbitrary threshold on a continuous heuristic. Questions like "is this performant?", "is this maintainable?", "are there latent bugs?", "is this idiomatic?" do not have free binary form — they require an arbitrary threshold or rely on a taste judgment without ground truth. Predicting such questions trains the system to match a heuristic, not reality. We do not predict heuristics; we predict reality.

---

## 1. End-to-End Data Flow

```
[Operator / AI agent Edit + tool calls + test outcomes]
                 │
                 ▼
       Claude Code hooks
   (PostToolUse Bash + Stop +
    UserPromptSubmit + observe ticker)
                 │  JSONL
                 ▼
   /zfs/archive/contextgraph/runtime/shift-log.jsonl
                 │
                 ▼  (tail)
      context-graph-mejepa-shift-subscriber
        runner.rs → shift_transform.rs
                 │
                 ▼
     21 embedders → 12 active content slots
     (E1, E6, E7, E8, E10, E12, E13, E14 frozen;
      plus dynamic embedders behind freeze gate)
                 │
                 ▼
      15 frozen instruments → array of 15 per-embedder vectors (a teleological constellation)
        E_AST 384 | E_CFG 256 | E_DataFlow 256 | E_TypeGraph 256 |
        E_Test 384 | E_Trace 512 | E_Diff 256 | E_Witness 256 |
        E_Oracle 128 | E_Problem 1024 | E_CommitMsg 384 |
        E_StaticAnalysis 256 | E_Runtime 256 | E_Reasoning 384 |
        Scalars 128
        (each slot's vector lives in its own embedder's space;
         the array IS the constellation, NOT a 5,120-d fused embedding;
         5,120 is contiguous-serialization byte layout only)
                 │  PanelBuilder validates: dim + finite + no double-fill
                 ▼
      CF_MEJEPA_PANELS + CF_MEJEPA_PANEL_META + CF_MEJEPA_SHIFT_WATERMARK
                 │
                 ▼
      Live label compiler (#413)
        -> live-input-safe slot/pair/group labels per chunk
                 │
                 ▼
      Runtime ability resolver (#418)
        -> AbilityContext from chunk-skill/higher-ability memberships
        -> PredictionLabelContext with all active memberships preserved
                 │
                 ▼
      MeJepaPredictor.forward(panel, PredictionLabelContext)
        + per-head DDA projection (CF_MEJEPA_HEAD_PROJECTION_REGISTRY)
        + split-conformal interval (CF_MEJEPA_CALIBRATION_HISTORY)
        + OOD gate (CF_MEJEPA_OOD_ESCALATIONS)
        + TCT Gτ guard (CF_MEJEPA_GUARD_DECISIONS)
        + cold-cell metric (CF_MEJEPA_COLD_CELL_METRICS, abstain if n_supporting<50)
        + multi-head contradiction gate (CF_MEJEPA_CONTRADICTION_THRESHOLDS)
        + constellation intelligence heads (CF_MEJEPA_CONSTELLATION_PATTERNS)
        + matched fingerprint (CF_MEJEPA_FINGERPRINT_REVERSE_INDEX)
        + closest exemplars HNSW (CF_MEJEPA_PATCH_SIMILARITY_META)
        + claim graph reconciliation (CF_MEJEPA_CLAIM_RECONCILIATION)
                 │
                 ▼
      RealityPrediction (Q1 claim_graph + Q2 verdict + Q3 fail modes +
                         Q5 reality_impact + label/skill context +
                         constellation_intelligence)
                 │
                 ▼
      CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_HIERARCHICAL_PREDICTIONS
                 │
       ┌─────────┴───────────┐
       ▼                     ▼
   AI agent             Verification capture
   (mejepa_predict_     (PostToolUse Bash matcher
    latest /            → CF_MEJEPA_PREDICTION_VERIFICATIONS)
    project_report)
                              │
                              ▼  Confirmed / Refuted
                       Active learning queue + replay buffer
                       (CF_MEJEPA_ACTIVE_LEARNING_QUEUE +
                        CF_MEJEPA_REPLAY_BUFFER)
                              │
                              ▼
                       Trainer (UTML L_step = ΔP·ΔK·ΔΩ·ΔΞ)
                       + EWC++ Fisher
                       + Continual Backprop
                       + Inverse-map head (λ_inv)
                              │
                              ▼
                       CF_MEJEPA_TRAIN_CERTS
                       (incl. CPΦ conditional_description_length_bits)
                              │
                              ▼
                       Heal scheduler (drift detection,
                       per-cell convergence, promote / rollback)
                              │
                              ▼
                       CF_MEJEPA_MODEL_PROMOTIONS + CF_MEJEPA_ACTIVE_POINTERS
```

The pipeline is a consequence predictor, not an intention engine (#397/#358). It learns from observable reality-chain events:

```
explicit task text / agent claim
  → edit event
  → AST delta
  → per-embedder packed outputs
  → teleological constellation
  → frozen instruments
  → oracle / test / static / runtime / witness outcomes
  → prediction verification
  → replay / reward signal
```

That chain is how ME-JEPA learns to predict what happens when code is live. The human or their AI agent receives the predicted consequences and decides how to act; ME-JEPA does not infer hidden human intent or synthesize a north-star objective.

---

## 2. Embedder Registry (21 slots; E5 retired)

`crates/context-graph-mejepa-embedders/src/lib.rs:1-9`. Production loading requires schema-versioned `models_config.toml`, SHA-256 manifest pins, runtime CUDA, and E17 calibration evidence.

| Slot | Family | Dim | Frozen | Active in production | Vector source |
|---|---|---|---|---|---|
| E1 | e5-large-v2 | 1024 | ✅ frozen | ✅ | Semantic (default content embedder) |
| E2 | temporal (recency PE) | 512 | tunable | active (weight-gated) | Position-encoded recency embedding |
| E3 | temporal (periodic) | 512 | tunable | active (weight-gated) | Periodic-pattern embedding |
| E4 | temporal (V_ordering) | 512 | tunable | active (weight-gated) | Session-sequence position |
| E5 | causal (asymmetric) | 768 | — | **retired** | Was cause/effect dual vector; tools fail closed |
| E6 | sparse keyword | 30K vocab | ✅ frozen | ✅ | Keyword inverted index |
| E7 | Qodo-Embed code | 1536 | ✅ frozen | ✅ | Code AST/semantic embedder |
| E8 | graph | 1024 | ✅ frozen | ✅ | Graph-aware embedder |
| E9 | HDC | 1024 | tunable | active | Typo-robust hyperdimensional |
| E10 | paraphrase | 768 (multimodal: doc/query dual) | ✅ frozen | ✅ | Paraphrase asymmetric |
| E11 | entity (KEPLER/TransE) | 768 | — | **disabled** | Replaced by TASK-EK-018F TransE-on-corpus |
| E12 | ColBERT late-interaction | 128 per token | ✅ frozen | ✅ | Per-token MaxSim |
| E13 | SPLADE sparse | 30K vocab | ✅ frozen | ✅ | Sparse expansion |
| E14 | (frozen-by-design slot) | (per config) | ✅ frozen | ✅ | Reserved |
| E15-E16 | reserved | — | — | — | — |
| E17 | (calibration certificate slot) | — | — | gated by `CalibrationCertificate` | `agent_state_score` |
| E18-E21 | dynamic-registration slots | per registration | gated | gated | Dynamic embedders (TASK-EK-018A) |

**Frozen-by-design weights:** E1, E6, E7, E8, E10, E12, E13, E14 (per CLAUDE.md key facts).
**Dynamic embedders:** `crates/context-graph-mejepa-embedders/src/dynamic_registry.rs` — `DynamicEmbedderKind`, `DynamicEmbedderRegistryRecord`, `DynamicEmbedderProvenanceRecord`. Hot-swap registry with VRAM budget enforcement (`vram.rs::query_vram_budget`).
**Falsification + freeze:** `crates/context-graph-mejepa/src/{embedder_falsification.rs, dynamic_embedder_freeze.rs}` — TASK-EK-018C/G/J gate proposed embedders through falsification + freeze + adversarial-injection mitigation before they reach production.
**Foundationality:** PageRank on chunk/library/embedder dependency graphs (TASK-EK-016 / EK-017 / EK-016-ext) → `CF_MEJEPA_*_FOUNDATIONALITY`. Used to weight `BatchSampler` and Fisher.

---

## 3. Instrument Panel — Array of Per-Embedder Vectors (a teleological constellation)

`crates/context-graph-mejepa-instruments/src/lib.rs:158-199`. The panel is **an array of 15 per-embedder/per-instrument vectors**, one vector per slot, each living in its own embedder's vector space. It is a teleological constellation in the strict sense: the array IS the constellation, not a fused embedding of the array.

**The architectural rule (locked in):**

1. **Each slot's vector lives in its own embedder's space.** E_AST's 384-d vector is a tree-sitter AST embedding; E_TypeGraph's 256-d vector is a type-graph embedding; E1's vector is an e5-large-v2 semantic embedding. These vector spaces are **not** dimension-compatible with each other.

2. **Same-embedder vectors are dimension-wise comparable.** E_AST(chunk A) cosine E_AST(chunk B) is valid because both vectors live in the same E_AST space. Same for every other slot pair across different chunks (E_TypeGraph ↔ E_TypeGraph, E1 ↔ E1, E_Trace ↔ E_Trace, etc.). The Gτ guard at `gtau.rs:14` enforces this by iterating per embedder and computing `cosine_similarity(slot, centroid)` only between same-embedder slots.

3. **Cross-embedder dimension comparison is forbidden.** You cannot compare E1's dim-3 to E_AST's dim-3 because those numbers come from unrelated spaces. The `cosine_similarity` function at `gtau.rs:90` fails closed with `MEJEPA_TCT_DIM_MISMATCH` if you attempt it (different-length vectors can never be compared; same-length vectors from different embedders MUST not be compared).

4. **Whole arrays can be compared to whole arrays.** Two constellation arrays (e.g. a candidate panel and a centroid panel from the same `(cell × language × entity_type)` cell) can undergo array-level operations: per-slot cosine map (the Gτ guard), per-slot violation accumulation, full pairwise per-slot similarity matrix, slot-set intersection/union over filled bits, matrix-style projections that treat the array as a typed structured object, and any other operation that respects slot identity.

5. **Cross-embedder relationships within the same array are scalar pairwise metrics, not vector dim-wise compares.** MI(E_AST, E_TypeGraph), correlation(E_Test_outcomes, E_Oracle_outcomes), agreement-or-disagreement scores between pair (E_AST, E_DataFlow) — these are valid scalar relationships. They compute *over* the two slot distributions; they don't try to align dimensions across them.

6. **`PANEL_DIM = 5_120` is BYTE-LAYOUT only.** The constant in `crates/context-graph-mejepa-instruments/src/lib.rs:165` and the offset table in `InstrumentSlot::layout()` exist solely to serialize the structured array into a contiguous f32 buffer for RocksDB persistence. The 5,120 number is a sum of per-slot dimensions for storage and transport; it is NOT a unified semantic embedding, it is NOT comparable to any other 5,120-d vector, and it is NOT a valid input to a single-projection "linear(5120, hidden)" predictor head. Any flat projection over the packed 5,120 buffer is forbidden; the slot-preserving refactor (#385/#387) replaced that pattern with per-slot encoders + late fusion.

**Slot inventory (per-slot vector dimensions; each lives in its own space):**

| # | Slot | Variant | Dim | Offset | Encoder source | Encodes |
|---|---|---|---|---|---|---|
| 1 | `EAst` | `e_ast` | 384 | 0 | `e_code::EAstInstrument` | tree-sitter AST |
| 2 | `ECfg` | `e_cfg` | 256 | 384 | `e_code::ECfgInstrument` | Control-flow graph attention |
| 3 | `EDataFlow` | `e_data_flow` | 256 | 640 | `e_code::EDataFlowInstrument` | Def-use graph attention |
| 4 | `ETypeGraph` | `e_type_graph` | 256 | 896 | `e_code::ETypeGraphInstrument` | Type signature + inferred-type graph |
| 5 | `ETest` | `e_test` | 384 | 1152 | `e_text::ETestInstrument` (reuses E1) | Test descriptions |
| 6 | `ETrace` | `e_trace` | 512 | 1536 | `e_trace::ETraceInstrument` | Execution-trace sequence transformer |
| 7 | `EDiff` | `e_diff` | 256 | 2048 | `e_code::EDiffInstrument` | AST-diff edge encoder |
| 8 | `EWitness` | `e_witness` | 256 | 2304 | `e_witness::EWitnessInstrument` | Witness-chain entry sequence |
| 9 | `EOracle` | `e_oracle` | 128 | 2560 | `e_oracle::EOracleInstrument` | Test outcomes + 10-class exception one-hot |
| 10 | `EProblem` | `e_problem` | 1024 | 2688 | `e_text::EProblemInstrument` (reuses E1) | SWE-bench problem text |
| 11 | `ECommitMsg` | `e_commit_msg` | 384 | 3712 | `e_text::ECommitMsgInstrument` (reuses E1) | Candidate fix commit message |
| 12 | `EStaticAnalysis` | `e_static_analysis` | 256 | 4096 | `e_static_analysis::EStaticAnalysisInstrument` | Linter / typecheck / complexity aggregates |
| 13 | `ERuntime` | `e_runtime` | 256 | 4352 | `e_runtime::ERuntimeInstrument` | Wall-clock / RSS / coverage / network |
| 14 | `EReasoning` | `e_reasoning` | 384 | 4608 | `e_reasoning::EReasoningInstrument` | Reasoning / reflexion trace |
| 15 | `Scalars` | `scalars` | 128 | 4992 | `e_scalars::ScalarsInstrument` | BFS depth, blame age, churn, coverage Δ, repo-health |

**Storage byte layout total:** 5,120 f32 (sum of per-slot dims: 4992 + 128 = 5,120). This is the contiguous serialization size only; the SEMANTIC unit is the array of 15 per-embedder vectors, NOT a 5,120-d fused embedding. A `Panel` deserialized from disk is sliced back into per-slot regions before any operation; no operation reads across slot boundaries as if the dimensions were homogeneous.

**Panel invariants** (`PanelBuilder::set_slot`, `validate_panel_parts`):
- `vector.len() == slot.dim()` else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Every element finite else `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`
- Slot not already filled else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Build rejects `filled_mask == 0` (no slots set is indistinguishable from missing evidence)
- `panel.data.len() == PANEL_DIM`
- `filled_mask` bits outside `VALID_FILLED_MASK` (0x7FFF) rejected
- All persistence flows through `try_from PanelSerde` so JSON round-trip re-validates

**Structural collapse-immunity invariant.** Instruments carry zero trainable parameters. `FrozenGuard` / `FrozenSnapshot` / `hash_f32s` (`frozen_hook.rs`) enforce `ΔΞ.target_collapse = 0` by construction. `crates/context-graph-mejepa/src/grad_hook.rs::run_gradient_hook` checks for `MEJEPA_INSTRUMENT_GRADIENT_LEAK` and fails closed if any gradient touches a frozen-target tensor.

**Retired Q4/RWD producer surfaces.** The unconsumed TASK-RWD producer modules (`composite_reward_signals.rs`, `static_analysis_reward.rs`, `telemetry_signals.rs`, `type_graph_signals.rs`, `live_session_signals.rs`, plus the standalone coverage/cyclomatic/diff-topology helpers) were removed from the active crate surface on 2026-05-19. This does not remove durable historical CF evidence and does not remove the live deterministic target instruments (`e_code`, `e_static_analysis`, `e_runtime`, `e_oracle`, `e_witness`, etc.). The only Q4-related code that remains compiled is the deprecated-readable label/trust-gate surface described in §16.

---

## 4. Predictor Architecture

`crates/context-graph-mejepa/src/predictor/` — `MeJepaPredictor`, `TransformerLayer`, `ArchitectureSummary`. CUDA-required.

**Architectural rule (locked in by #385 / #386 / #387; flat-vector pattern is forbidden):** the predictor consumes the panel **as an array of per-embedder vectors**, NOT as a flat 5,120-d input. Per-slot input encoders project each slot vector from its own embedder space into a shared latent representation; the latent representations are then late-fused. There is no single `linear(5120, hidden)` projection over the packed byte buffer — that pattern is the architectural error #385 corrected. The slot-preserving late-fusion predictor shipped via #387 (FSV PASS, `slot_identity_preserved=true`, `flat_vector_concat_used=false`).

**Why this matters operationally:** the per-embedder vectors live in distinct spaces (e5-large-v2 semantic, tree-sitter AST, type-graph attention, ColBERT late-interaction, etc.). Dimension `k` of E1 means nothing in relation to dimension `k` of E_AST — they are unrelated coordinate systems. A flat `linear(PANEL_DIM, hidden)` projection treats all 5,120 dimensions as one homogeneous coordinate system and silently learns cross-embedder compensations that are mathematically meaningless. Per-slot encoders + late fusion respect the array structure: each slot is projected by its own dedicated input encoder; per-slot evidence flows forward as typed entries that downstream heads can attribute violations to.

**Legal operations on the panel (per slot or array-level):**
- **Per-slot dim-wise:** cosine, dot product, L2 distance against same-embedder vectors only (e.g. `cosine(panel.E_AST, centroid.E_AST)`). Implementation: `gtau.rs:14` iterates per embedder.
- **Array-vs-array:** per-slot cosine map (vector of 15 same-embedder cosines), strict-conjunction gate over the map (all slots must clear their per-slot threshold), per-pair similarity matrix (where each cell is a same-embedder cosine), aggregate scalar statistics over the map (min margin, slot-count violating, etc.).
- **Cross-slot SCALAR pairwise within the same array:** MI(slot_a_distribution, slot_b_distribution), correlation(slot_a, slot_b) — scalar relationship metrics over the two slot distributions, NOT a vector dim-wise compare. Shipped via #388 (78 pairwise correlation tensors per cell).
- **Matrix-style operations on the structured array:** any operation that treats the array as a typed 15-entry structured object — per-slot weighted aggregation, slot-set selection, masked matrix projections that respect slot identity, slot-preserving fusion through 15 dedicated linear projections rather than one fused projection.

**Forbidden operations:**
- Single linear/conv/attention projection over the packed 5,120-d buffer as if it were one embedding space.
- Dimension-wise comparison between two slots from different embedders (e.g. `cosine(panel.E_AST_subvector_padded_to_768, panel.E_TypeGraph_subvector_padded_to_768)` is forbidden even if you pad/truncate — the dimensions live in unrelated coordinate systems).
- Flat residual computation over the whole 5,120 buffer (`residual_norm_sq(predicted.data, target.data)` over the packed buffer) — must decompose to per-slot residuals first.
- Any operation that allows a strong slot to compensate for a violation in a weak slot. The anti-compensation FSV (#394) is the regression gate.

Default architecture (`config.rs`) — per-slot:
- 15 per-slot input encoders (one per `InstrumentSlot`), each `linear(slot.dim(), per_slot_latent_dim)`
- Late-fusion stack: typed concatenation of the 15 latents followed by transformer layers operating on the slot-preserving sequence (slot index becomes a positional encoding feature so the transformer cannot accidentally treat dimensions as exchangeable)
- 15 per-slot output decoders that emit per-slot residuals against the frozen target (rather than one flat decoder)
- `DEFAULT_HIDDEN_DIM`, `DEFAULT_NUM_HEADS`, `DEFAULT_NUM_LAYERS`, `DEFAULT_FF_EXPANSION`, `DEFAULT_LAYER_NORM_EPS`
- `INVERSE_ACTION_DIM` — 16-d action latent for inverse map (TASK-EK-012)
- VICReg: `VICREG_INV_LAMBDA`, `VICREG_VAR_LAMBDA`, `VICREG_COV_LAMBDA`, `VICREG_GAMMA` — computed per-slot, aggregated via fail-closed conjunction
- `GRAD_NORM_NOISE_FLOOR`
- VRAM budgets: `VRAM_STEADY_STATE_TARGET_BYTES`, `VRAM_WARN_THRESHOLD_BYTES`

Legacy constants (`PANEL_DIM`, `CONCAT_INPUT_DIM`) remain in `config.rs` as serialization helpers but MUST NOT appear as the input shape of any semantic projection. Any new module that requests them as a single-projection input must fail closed via the #385 freeze guard or be rejected at code review.

Inference-time config (`MeJepaInferConfig`):
- `INFER_DEFAULT_ALPHA` — conformal α
- `INFER_DEFAULT_P_THRESHOLD`, `INFER_DEFAULT_P_TEST_THRESHOLD`
- `INFER_DEFAULT_OOD_REFUSE_THRESHOLD`
- `INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT` — TASK-PY-G-061 (cell starves → Abstain)
- `INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS`, `INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS`
- `INFER_DEFAULT_TRAIN_CERT_WINDOW_STEPS`
- `INFER_DEFAULT_OUTCOME_SET_MAX`
- `INFER_DEFAULT_BOOTSTRAP_DELTA_OMEGA`, `INFER_DEFAULT_BOOTSTRAP_DELTA_XI`
- `INFER_DEFAULT_INSTRUMENT_CACHE_CAPACITY`
- `INFER_DEFAULT_REQUIRE_DDA_FEATURES`
- `INFER_DEFAULT_DDA_EXPECTED_EMBEDDER_COUNT` + `INFER_MAX_DDA_EXPECTED_EMBEDDER_COUNT`

### 4.1 Heads (per Q-question)

| Head module | Q | Output | Source file |
|---|---|---|---|
| `oracle_head.rs::OracleHead` | Q2 | `OracleLogits` over Pass/Fail/Skip/Error | `oracle_head.rs` |
| `claim_graph.rs` | Q1 | `ClaimGraph` + reconciliation against bytes-on-disk and AST | `claim_graph.rs` |
| `patch_similarity.rs` | Q3 | `closest_exemplars: Vec<Exemplar>` from usearch HNSW over `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` L2-normalized signature | `patch_similarity.rs` |
| `failure_fingerprint.rs` | Q3 | `matched_fingerprint`, predicted_failure_modes ranked | `failure_fingerprint.rs` |
| `q4_security_labels.rs` | Q4 | Security concerns (Bandit/Semgrep classes) | (instruments crate) |
| Q4 perf / cost / accuracy heads | Q4 | (training pipelines TASK-PY-G-047/048/049; predictor surfaces in `prediction_surfaces.rs`) | per task |
| `reality_impact.rs` | Q5 | `RealityImpact` from shift-log replay against predicted side effects | `reality_impact.rs` |
| `hierarchical.rs` | spans Q1-Q5 | file/function/AST/chunk hierarchy → `CF_MEJEPA_HIERARCHICAL_PREDICTIONS` | TASK-FP-102 |
| `latent_search.rs` | Q3 best-of-K | latent-action proposals | TASK-FP-103 |
| `candidate_ranking.rs` | Q3 ranking | counterfactual repair ranking | TASK-FP-105 |
| `objective_safety.rs` | constraints | hardwired objective + safety constraints | TASK-FP-104 |

### 4.2 Verdict assembly

`crates/context-graph-mejepa/src/verdict_assembly.rs` stitches heads into the public binary verdict plus internal-state markers:

```rust
enum Verdict { Pass, Fail, OutOfDistribution, Abstain, GuardRejected }
```

**Product surface:** `Pass` or `Fail` are the only values that ship to the MCP caller. `OutOfDistribution`, `Abstain`, and `GuardRejected` are internal-state markers indicating the predictor is not yet good enough to commit on this `(cell × named_failure_mode)` cell. They MUST still be projected to `Pass | Fail` at the MCP boundary (using closest-exemplar majority outcome) so the agent always receives a binary answer; the projected verdict is then subject to ground-truth verification and the mistake-loop update (§9.6). The internal-state marker is persisted alongside in `CF_MEJEPA_LIVE_PREDICTIONS.internal_marker` for forensic audit and to drive heal-scheduler attention.

Decision flow (per `compiler.rs::compile`):
1. Materialize `Panel` for the candidate change.
2. Forward through `MeJepaPredictor`.
3. Project per-head DDA features (`head_projection.rs` + `CF_MEJEPA_HEAD_PROJECTION_REGISTRY`).
4. Apply split-conformal interval (`conformal.rs`).
5. **Per-slot OOD reasons (TASK-PY-G-089 / #389).** `RealityPrediction.per_slot_ood_reasons: Vec<PerSlotOodReason>` is materialized from per-slot/per-embedder calibration state under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`. Any slot/embedder/pair-specific OOD reason persists with the prediction row, byte-replays from RocksDB, and is exposed by `mejepa_explain_prediction` / `mejepa_inspect_prediction`. `MAX_EMBEDDERS_PER_PANEL` guard rejects over-wide arrays.
6. Aggregate OOD gate (`ood.rs` + `CF_MEJEPA_OOD_ESCALATIONS`). Strict-conjunction policy: aggregate score never overrides a slot violation (`aggregate_score_never_overrides_slot_violation=true` in `ood_guard_policy.json`).
7. TCT Gτ guard (`tct/gtau.rs` + `CF_MEJEPA_GUARD_DECISIONS`).
8. Cold-cell metric: if cell `n_supporting < INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT` (50) → `Verdict::Abstain` + `CF_MEJEPA_COLD_CELL_METRICS` row (TASK-PY-G-061).
9. Apply contradiction thresholds (`contradiction.rs` + `CF_MEJEPA_CONTRADICTION_THRESHOLDS`). If the Q2 pass head is high while calibrated consequence-pressure evidence from Q3 explanation/TCT constellation signals is also high, emit `Verdict::Abstain` with `MULTI_HEAD_CONTRADICTION`. Missing thresholds fail closed with `CONTRADICTION_THRESHOLD_MISSING`; single-head evidence records `SINGLE_HEAD_ONLY`. Q4 thresholds are historical/display-only and are not consumed by the active verdict path.
10. Summarize constellation intelligence (`constellation_intelligence.rs` + `CF_MEJEPA_CONSTELLATION_PATTERNS`). Slot-pair consensus, contradiction, redundancy, novelty, and blind-spot evidence becomes `RealityPrediction.constellation_intelligence`; high consequence pressure can abstain before a false confident pass, and promoted relationship patterns can seed active-learning rows.
11. Match against fingerprint catalog (`CF_MEJEPA_FINGERPRINT_REVERSE_INDEX`).
12. Pull closest exemplars (`CF_MEJEPA_PATCH_SIMILARITY_META`).
13. Reconcile claim graph (`CF_MEJEPA_CLAIM_RECONCILIATION`).
14. Emit `RealityPrediction` → write `CF_MEJEPA_LIVE_PREDICTIONS` + `CF_MEJEPA_HIERARCHICAL_PREDICTIONS`.

**Anti-compensation guard (TASK-PY-G-094 / #394).** `crates/context-graph-mejepa/examples/python_anti_compensation_fsv.rs` and the calibrated ablation suite (#398) prove corrupted / missing / stale / dimension-shifted slots, contradictory pair relationships, and temporal slot jumps cannot be accepted through aggregate evidence even when a flat normalized score would pass. A code scan prevents flat-vector ship-gate eligibility regressions.

**Contradiction gate (TASK-PY-G-044 / #263).** `contradiction.rs`, `verdict_assembly.rs`, and `compiler.rs` read per-cell thresholds from `CF_MEJEPA_CONTRADICTION_THRESHOLDS`. The active gate abstains when calibrated Q2/pass confidence and high-severity Q3 explanation/TCT consequence-pressure evidence conflict, records structured evidence in the prediction/park-list path, fails closed on missing thresholds, and skips only when there is genuinely a single head of evidence. Historical Q4 contradiction rows remain readable but are frozen display-only under #408. aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-044-multi-head-contradiction-fsv/run-20260519T114854Z/multi_head_contradiction_fsv.json`, sha256 `39fac46044f4a3a8a9d43a5ff117af8c1b8b46906f186f284c89120460b1cf48`.

**Constellation intelligence heads (TASK-PY-G-090 / #390).** `constellation_intelligence.rs`, `compiler.rs`, `verdict_assembly.rs`, `eval/queue.rs`, and `store.rs` now convert DDA/slot-pair relationship rows into structured consensus, contradiction, redundancy, novelty, blind-spot, and consequence-pressure evidence. The live prediction surface carries `RealityPrediction.constellation_intelligence`; `CF_MEJEPA_CONSTELLATION_PATTERNS` stores promoted real-corpus pair patterns; high-value disagreement can enter `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`; and high Q2 pressure abstains before a false confident pass. aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-090-consensus-contradiction-heads-fsv/run-20260519T124035Z/consensus_contradiction_heads_fsv.json`, sha256 `b1d500f008b0a14a704c4c2f83fe8cd19349f0dfb0c78baedb35c711abc50b64`.

---

### 4.3 Failure-mode labeling hierarchy (the meaning layer over the slot panel)

**Level 0 (deterministic label compiler) — SHIPPED 2026-05-19 via #413 / TASK-PY-G-117.** Full-corpus aiwonder FSV passed. The Level-1/2/3 hierarchical naming on top of this Level-0 registry remains open under #404 / TASK-PY-G-110. The sequence-aware chunk-to-skill membership layer now has code-level synthetic FSV plus full aiwonder corpus materialization under #414 / TASK-PY-G-118.

**Shipped source (Level 0):** `scripts/swebench_lite_ops/python_auto_label_discovery.py` (1,641 lines, deterministic Python, no inner LLM). CLI entrypoint `python-auto-label-discovery`. Output root `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/`. FSV `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json` (`passes=true`).

**Source (Level 1/2/3, #404 + #414):** planned runtime hierarchy consumers remain in `crates/context-graph-mejepa/src/failure_mode_hierarchy.rs`, `failure_mode_aggregator.rs`, `failure_mode_vocabulary.rs`, and `failure_mode_consumer.rs`. The first #414 train-side implementation is now `crates/context-graph-mejepa-train/src/skill_sequence_discovery.rs`, `skill_sequence_types.rs`, `chunk_skill_membership.rs`, and `skill_validation.rs`, with FSV example `task_py_g_118_skill_sequence_discovery_fsv.rs`. It turns ordered Level-0 label episodes into promotion-ready Level-2 skill rows and writes the reverse membership rows that tell each chunk which skills and higher-level abilities it participates in. Storage CFs registered in `context-graph-mejepa-cf`: `CF_MEJEPA_FAILURE_MODE_LEVEL1`, `CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS`, `CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS`, `CF_MEJEPA_FAILURE_MODE_VOCABULARY`, `CF_MEJEPA_NAMED_PREDICTIONS`, `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`, `CF_MEJEPA_SKILL_REVERSE_INDEX`, `CF_MEJEPA_SKILL_LIFECYCLE_AUDIT`. MCP tools (planned): `mejepa_list_failure_modes`, `mejepa_label_failure_cluster`, `mejepa_explain_named_failure_mode`, `mejepa_list_accepted_labels`, `mejepa_label_compiler_status`, `mejepa_list_chunk_skills`, `mejepa_skill_lifecycle_audit`.

**Why it exists.** The 300×8 corpus has been projected through 13 retrieval embedders + 15 frozen instrument slots, producing 78 pairwise correlation tensors per cell (#388), per-slot calibration state (#389), and consensus/contradiction/blind-spot/redundancy/novelty surfaces (#390). All of that is raw numerical evidence. The labeling layer turns those numbers into named rules so the predictor can say "this patch will Fail because of `type_signature_drift_under_test_coverage`" rather than "this patch's per-slot OOD trace is `{E_TypeGraph: 0.42, E_DataFlow: 0.31, ...}`."

**Level 0 — Discovery procedure (shipped).** Deterministic label compiler emits candidate labels for every chunk/slot/pair/group across seven families: (1) AST/source surfaces, (2) source-site relation (exact-mutation / changed-file / wrong-file / missing-target / test-patch / no-compact-source-site), (3) mutation mechanism + micro-label, (4) per-slot region labels (`norm_l2` / `std` / `abs_mean` / `entropy_proxy` / `zero_fraction`; never cross-slot vector concatenation), (5) pair relationship labels, (6) group signatures (`temporal_triad`, `semantic_content_panel`, `sparse_lexical_panel`, `sparse_hdc_bridge`), (7) target-side outcome labels marked `leakage_policy:target_side_only`. **Full-corpus output: 399,777 chunk labels, 25,289,552 pair labels, 3,106,565 group labels, 3,000 code-state constellations, 4,224 file-path constellations, 1,139 failure evidence sets.**

**Usefulness scoring (shipped)** rejects labels that don't survive: minimum support, pass/fail lift threshold, train/calibration/holdout stability, closest-exemplar oracle agreement, incremental predictive contribution over simpler labels, degenerate/leaky-label rejection, typed uncertainty for unknown/unsupported/low-power labels. Of 1,800+ candidates, **491 entered the durable accepted registry**.

**Three localization modes (shipped)** for failed/problematic code states: `single_point_candidate` (612), `multi_point_evidence_set` (1,479), `diffuse_code_state_risk` (18), `working_state_no_failure_localization` (891).

**Learning bridge manifest (shipped, `learning_bridge_manifest.json`)** maps accepted Level-0 label IDs into existing UTML/replay consumers — no new training loop required:
- `delta_p.per_chunk_values` consumes per-chunk labels
- `delta_xi.constellation_violation_rate` consumes constellation patterns (`CF_MEJEPA_CONSTELLATION_PATTERNS`)
- `ReplayBufferRow.cell_id` keyed from Level-2 code-state constellations
- `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` consumes unknown/OOD reasons
- `CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT` consumes promotion/rejection events

**Chunk-to-skill reverse membership (production FSV passed, #414).** After live label compilation, every AST chunk's TCT constellation is matched against accepted Level-1 groups, Level-2 skills, and Level-3/higher constellations. Membership is many-to-many: one chunk can participate in multiple skills, and one skill can span many chunks, files, ordered steps, prerequisite labels, intermediate states, and outcome observations. Rows record `chunk_id`, `code_state_key`, `skill_id`, `hierarchy_level`, `membership_score`, source accepted label IDs, ordered step evidence, `live_input_allowed`, and provenance hashes. The prediction path consumes these active skill IDs in `PredictionLabelContext`; it does not infer hidden human intent. Synthetic FSV: `/tmp/contextgraph-task-py-g-118-skill-sequence-fsv/task_py_g_118_skill_sequence_discovery_fsv.json` (`passes=true`). Production aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-118-skill-sequence-discovery-fsv/run-1779213075188/task_py_g_118_aiwonder_skill_materialization_fsv.json` (`passes=true`, `sha256:0df1437ae5e0542b356777b29df700b521b83b054113697ca280ae3d242d3dee`), streaming all 399,777 chunk rows and materializing 128 Level-2 skills, 36,773 chunk-skill memberships, 10,579 reverse indexes, and 128 lifecycle audits.

**Bidirectional skill/ability linkage surface (first read surface shipped, #417 / TASK-PY-G-120; runtime resolver open, #418 / TASK-PY-G-121).** The shipped #414 rows are not only a training artifact. They are the runtime bridge between code and abilities:
- **Code -> skill:** given a live AST chunk, return every Level-1 group, Level-2 skill, and Level-3/higher ability the chunk participates in, with ordered step evidence and source label IDs.
- **Skill -> code:** given a skill or ability, return the chunks/files/source bytes that instantiate it, using `CF_MEJEPA_SKILL_REVERSE_INDEX`.
- **Prediction input:** live predictions populate `PredictionLabelContext.active_skill_ids` from live-input-safe memberships only; target-side oracle/test labels remain supervision after reality responds.
- **Pathway attachment:** #416 Fail leaves and Q5 leaves attach the active skill IDs plus historical source chunks that explain that consequence when available.
- **Learning update:** if the prediction is wrong, #406 must write a skill lifecycle audit that updates/splits/creates/demotes/no-changes the involved skill model rather than leaving the chunk membership static.
- **Runtime resolver:** #418 owns `AbilityContext`: resolve all many-to-many chunk memberships, preserve higher-ability IDs and membership keys, pass them into prediction/replay, and create a candidate ability when no safe existing ability explains a refuted prediction.

This means the live system does not merely predict from chunk vectors. It compiles chunks into TCT constellations, resolves their hierarchical abilities, predicts consequences through those abilities, and repairs the ability model when durable reality falsifies the prediction.

**Skill lifecycle on mistakes (partial code-level FSV, #414 feeding #406).** When a prediction disagrees with durable reality, the mistake loop must decide whether the active skill model was right but underweighted, too broad, too narrow, stale, leaky, or missing. Each mistake creates a lifecycle decision: `UpdateExistingSkill`, `SplitMixedSkill`, `CreateNewCandidateSkill`, `DemoteUnstableSkill`, or `NoChangeWithEvidence`. #414 now persists `SkillLifecycleAuditRow` for created candidate skills; wiring those decisions into the live #406 mistake consumer remains open. This is how ME-JEPA learns new abilities and repairs old ones as it sees new projects after the SWE-bench baseline.

**Level 1/2/3 — Aggregation procedure (planned, #404).** Aggregate over the shipped 491-row Level-0 accepted registry (NOT a new K-means pass — the K-means equivalent is the slot-region label emitter in Level 0). Group co-firing accepted labels by support count + co-occurrence per `(mutation_category × language × code_state)` cell into Level-1 groups. Name groups via cosine-vs-vocabulary against an operator-curated **failure-mode vocabulary** (`CF_MEJEPA_FAILURE_MODE_VOCABULARY`) embedded with E1 e5-large-v2. Top-1 match becomes the Level-1 group name. Operator approves / renames / merges via `mejepa_label_failure_cluster`.

**Failure-mode vocabulary (level 2 skills).** Seeded from published SWE-bench failure taxonomies, NOT invented from scratch:

| Source | Categories used as vocabulary seeds |
|---|---|
| Aleithan et al. 2025 "Are Solved Issues Really Solved" | `solution_leak`, `incorrect_fix_weak_test`, `different_files_changed`, `incomplete_fix`, `different_from_gold`, `more_comprehensive_than_gold` |
| Yang et al. 2024 "Empirical Study on Failures in Automated Issue Solving" | 9 categories × 25 subcategories across Location / Repair / Iteration & Validation phases — e.g. `localization_error`, `flawed_plan_persistence`, `endless_iteration_loop`, `repair_logic_error` |
| SWE-Bench Pro 2025 (Scale AI) | `wrong_solution`, `tool_use_error`, `syntax_error`, `incorrect_file`, `missed_edge_case`, `misunderstood_problem_statement`, `context_overflow_from_listing`, `endless_file_reading` |
| Our adversarial corpus (TASK-PY-G-060 / TASK-FP-009) | 30 adversarial fingerprints → seeded skill names per slot violation pattern |
| Our 8-mutation operator set | `subtle_flip_invariant_violation`, `off_by_one_boundary_miss`, `swap_variable_aliasing`, `delete_test_call_oracle_loss`, `wrong_file_target`, `over_engineer_no_value`, `compile_error_only` |

These are persisted in `CF_MEJEPA_FAILURE_MODE_VOCABULARY` keyed by `vocabulary_id` with body `{name, description, seed_source, embedding_e1, embedding_e7, operator_approved}`. Operator can extend via `mejepa_label_failure_cluster`.

**Level 3 — behavioural failure constellations.** Per `(mutation_category × language × failure_mode_skill)` cell, mine the ordered co-occurrence graph of skills and labels that explain consequence patterns in reality. Constellations are named compositions of skills: e.g. `unsafe_security_change = {security_label_present -> dataflow_drift -> no_test_coverage_added}`. Stored in `CF_MEJEPA_FAILURE_MODE_LEVEL3_CONSTELLATIONS`. Used by the Q2 binary verdict, Q3 explanation surface, closest-exemplar retrieval, and #406 replay keys.

**Prediction integration.** Once the hierarchy and reverse membership layers ship, every live prediction carries `PredictionLabelContext.active_skill_ids` and can expose the chunk memberships behind those skills. A chunk with multiple valid memberships must preserve all of them; a prediction over several chunks must preserve the ordered skill signature, because `skill:A -> skill:B` is not equivalent to `skill:B -> skill:A`. Every `RealityPrediction.verdict == Fail` row also carries:

```rust
pub struct NamedFailureMode {
    pub level1_group: Option<FailureModeGroupId>,    // K-means cluster ID
    pub level2_skill: Option<FailureModeSkillName>,  // operator-approved name
    pub level3_constellation: Option<FailureModeConstellationName>,
    pub confidence: f32,                              // distance to centroid
    pub closest_exemplars: Vec<ExemplarPointer>,     // top-K from CF_MEJEPA_PATCH_SIMILARITY_META
    pub vocabulary_source: VocabularySeedSource,     // which seed taxonomy this name came from
}
```

Persisted in `CF_MEJEPA_NAMED_PREDICTIONS` keyed by `prediction_id`, with chunk-level reverse membership rows in `CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP`. Exposed via `mejepa_explain_prediction`, `mejepa_predict_latest`, and `mejepa_list_chunk_skills`.

**FSV expectations.** Synthetic + real-corpus FSV must prove (a) cluster purity (≥0.7 silhouette on the 300×8 holdout), (b) operator-rename idempotence (rename -> reopen -> same id), (c) every Fail prediction in the holdout receives at least one named failure mode, (d) the named failure mode's top closest_exemplar has the same oracle outcome ≥85% of the time, (e) a chunk with multiple valid skill memberships preserves all memberships without flattening them into one label, (f) a live-codebase simulation without oracle labels still emits only live-input-safe memberships, and (g) a wrong prediction writes a skill lifecycle audit row with update/split/create/demote/no-change decision and independent RocksDB readback.

### 4.4 Cross-panel triangulation (the Goodhart guard)

Source files (planned): `crates/context-graph-mejepa/src/cross_panel.rs`, `crates/context-graph-mejepa-instruments/src/panel_b/`, new instrument modules `e_ast_codebert.rs`, `e_cfg_pythia.rs`, `e_dataflow_joern.rs`, `e_code_voyage.rs`, `e_text_bge.rs`. Storage CFs: `CF_MEJEPA_PANEL_B_OBSERVATIONS`, `CF_MEJEPA_CROSS_PANEL_AGREEMENT`. MCP tools (planned): `mejepa_cross_panel_score`, `mejepa_cross_panel_dashboard`. Tracked by GitHub epic **#405 (new) / TASK-PY-G-111 — Cross-panel triangulation Panel B**.

**Why it exists.** The 15-slot instrument panel (Panel A) is both the training substrate for the predictor AND the centroid set the Gτ guard evaluates against. That is encoder-matched by construction. A 0.95 oracle correlation on Panel A does not prove the verdict is right against reality; it could prove the predictor learned the panel's idiosyncrasies. The PAPER §6.3 limitation list calls this out explicitly for the voice case (named-future-work: ECAPA-TDNN cross-encoder triangulation); the same risk applies more sharply to the code case because the panel + predictor are jointly trained over the same 2,400-instance corpus.

**Panel B instrument map (deliberately non-overlapping with Panel A):**

| Slot purpose | Panel A (production) | Panel B (triangulation) |
|---|---|---|
| AST | tree-sitter encoder → 384-d | CodeBERT (microsoft/codebert-base) → 768-d projected to 384-d |
| CFG | attention encoder → 256-d | Pythia program-graph encoder → 256-d |
| Data-flow | def-use attention encoder → 256-d | joern-driven data-flow encoder → 256-d |
| Type-graph | tree-sitter type encoder → 256-d | inferred-type from `pytype` JSON → 256-d |
| Code-semantic (E7-equivalent) | Qodo-Embed-1 → 1536-d | Voyage-code-2 → 1536-d |
| Test / Problem / CommitMsg text | e5-large-v2 → variable | bge-large-en-v1.5 → variable |
| Trace / Witness / Oracle | unchanged | unchanged (deterministic, not encoder-bound) |
| StaticAnalysis / Runtime / Reasoning / Scalars | unchanged | unchanged |

The deterministic slots (Trace/Witness/Oracle/StaticAnalysis/Runtime/Reasoning/Scalars) are shared between panels because they encode reality directly, not learned representations. Only the *learned encoder* slots get swapped for triangulation.

**Triangulation procedure.**

1. Every gate-relevant prediction (any patch from the 300×8 holdout window) gets scored on Panel A *and* Panel B.
2. Panel A returns `verdict_a + score_a`. Panel B returns `verdict_b + score_b`. Both go to `CF_MEJEPA_CROSS_PANEL_AGREEMENT` keyed by `(prediction_id, panel_pair_id)`.
3. **Agreement metric:** for the ship-gate window, compute `cross_panel_oracle_correlation = corr(verdict_a, oracle) AND corr(verdict_b, oracle)` — both must clear 0.95.
4. **Drift metric:** `|corr_a - corr_b|`. Must be ≤ 0.05 for ship eligibility.
5. **Fail-closed condition:** if Panel A ≥ 0.95 but Panel B < 0.85, the gate fails closed with `CROSS_PANEL_GOODHART_DETECTED`. The predictor was encoder-matched-Goodharting Panel A; the next training cycle must include Panel B observations in the loss.

**Loss integration.** Optionally (under operator-flagged ablation), the trainer can add a Panel-B agreement term to the loss:

```
L_cross_panel = λ_xp * (1 - cos(predictor_logit_a, predictor_logit_b))
```

Default `λ_xp = 0` (panel B is evaluation-only); when the operator enables `λ_xp = 0.1` we expect the predictor to robustify against Panel A idiosyncrasies. Tracked as an ablation in #393's suite.

**Cost.** Panel B requires loading 5 new encoders. VRAM budget is ~12 GB during evaluation only; never resident during normal inference. Panel B can be evaluated in a separate worker that reads `CF_MEJEPA_LIVE_PREDICTIONS` periodically and writes back `CF_MEJEPA_PANEL_B_OBSERVATIONS`.

**FSV expectations.** Cross-panel FSV must prove (a) both panels evaluated on the same 30-instance holdout, (b) per-instance verdict + score persisted in `CF_MEJEPA_CROSS_PANEL_AGREEMENT`, (c) `CROSS_PANEL_GOODHART_DETECTED` fires when synthetic Panel-A-only correlation is injected, (d) the metric is byte-replayable from RocksDB.

---

## 5. TCT Constellations

`crates/context-graph-mejepa-tct/src/lib.rs:8-26`. Fail-closed: missing centroids, missing thresholds, under-sampled calibration cells, stale constellations, provenance drift, malformed RocksDB rows, non-finite floats are all errors. No permissive thresholds.

For code, a teleological constellation is the exact-state, slot-preserving observation of one reality object (for example an AST chunk, patch, file, or panel row) across all active embedders/instruments and its downstream consequence labels. Each embedder output remains in its own vector space. Mathematical work happens over the collection: per-embedder centroids/variance, pairwise correlation/MI, redundancy, disagreement, temporal movement, attribution, and calibrated uncertainty. Flat `PANEL_DIM` arrays are serialization only; they are not the semantic unit ME-JEPA trains or trusts for ship-gate evidence.

| Module | Purpose |
|---|---|
| `builder.rs` | Build constellation cells from training data |
| `calibrator.rs` | Per-cell calibration |
| `constellation.rs` | Centroid + threshold container |
| `freshness.rs` + `freshness_policy.rs` | Per-cell freshness checks; TASK-PY-G-052 scheduler |
| `gtau.rs` | Gτ guard predicate |
| `guard_audit.rs` | TASK-PY-G-015 per-Python-cell Gτ audit over #388 tensors and #389 per-slot calibration; reports pass/fail counts, missing reward-signal neighborhoods, and ship-gate countability without flattening slots |
| `hinge.rs` + `hinge_types.rs` | Decision hinges |
| `inspect.rs` | MCP `constellation_inspect` surface |
| `panel_slots.rs` | Slot definitions for TCT consumption |
| `rate.rs` + `rate_aggregator.rs` | Per-cell hit-rate accounting |
| `refresh_report.rs` | Human-readable refresh report (rendered into weekly markdown) |
| `shrinkage.rs` + `shrinkage_engine.rs` | Shrinkage for small-sample cells |
| `store.rs` | RocksDB CRUD on `CF_MEJEPA_CONSTELLATION` |
| `types.rs` | Cell key + Constellation DTOs |
| `verdict.rs` | TCT-side verdict overlay |

Cell key = `(mutation_category, language, …)`. Refresh log: `CF_MEJEPA_CONSTELLATION_REFRESH_LOG`. Refresh report cadence: weekly via TASK-G-018 operator triage.

---

## 6. Corpus Generator (the training-data flywheel)

`crates/context-graph-mejepa-corpus/src/lib.rs`. Inspired by mutation-testing approaches (Mothra, MuJava, mutmut); no upstream code copied.

### 6.1 8 mutation categories

| # | Variant | Slug | Description | Expected oracle |
|---|---|---|---|---|
| 1 | `KnownGood` | `known_good` | Echo input unchanged | PASS |
| 2 | `SubtleFlip` | `subtle_flip` | Invert one boolean operator (`==`↔`!=`, `and`↔`or`, …) | FAIL |
| 3 | `OffByOne` | `off_by_one` | Add ±1 to one numeric literal | FAIL |
| 4 | `SwapVariable` | `swap_variable` | Rename one local to another in-scope name | FAIL |
| 5 | `DeleteTestCall` | `delete_test_call` | Delete one assertion call | mixed (PASS deleted test, FAIL integration) |
| 6 | `WrongFile` | `wrong_file` | Apply patch text to a DIFFERENT base file | FAIL on test discovery / runtime |
| 7 | `OverEngineer` | `over_engineer` | Append unused helper function | PASS (correct but messy) |
| 8 | `CompileError` | `compile_error` | Introduce a Python syntax error | FAIL at parse |

`apply_mutation(category, primary_source, MutationConfig { seed, alternate_source })` — `seed` is `SplitMix64` site selector when multiple candidates; `alternate_source` required for `WrongFile`. Fail-closed errors: `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_OPERATOR_FAILED`.

### 6.2 11 languages

`Language::all() = [Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php]`. Python uses parser-backed operators (via `python_lex.rs` for triple-quote/f-string-aware lex mask). Other 10 use `generic_language_ops`. **Current ship gate is Python only** (`CLAUDE.md §6.3` and `27_phase_g_roadmap` legacy doc). Non-Python is deferred.

### 6.3 SWE-bench bridge

`swebench.rs::run_swebench_lite_oracle()`:
- Invokes `python -m swebench.harness.run_evaluation` per-instance (gold / custom mode).
- Parses per-instance `report.json`.
- Converts `FAIL_TO_PASS` / `PASS_TO_PASS` / `FAIL_TO_FAIL` / `PASS_TO_FAIL` → `OracleVerdict` per `swebench/harness/grading.py` semantics.
- Docker-name-safe `run_id` validator (regression: `+` in run_id was rejected by Docker container-name regex; fix shipped 2026-05-09e).

Lite dataset: 300 test instances + 23 dev instances across 12 repos (django=114, sympy=77, matplotlib=23, sklearn=23, pytest=17, sphinx=16, astropy=6, requests=6, +4 more). HF cache on aiwonder at `/zfs/hot/contextgraph/.cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/`.

### 6.4 Split planner

`split.rs::SplitMode`:
- `InstanceAtomic` (default) — clean 240/30/30 on Lite (80/10/10)
- `RepoAtomic` — structurally bounded to 195/77/28 (django=38% / sympy=25.7%); `assert_no_test_patch_leakage` helper

SplitMix64 shuffle + LPT pre-sort + lowest-current/target-ratio bin-pack.

### 6.5 Provenance

`CorpusProvenance` = sha256 over sorted `(task_id, category, patch_sha256, oracle_verdict_sha256)` rows. `MEJEPA_CORPUS_VERSION_V1` = `"phase0-corpus-v1"`. `MEJEPA_EMBEDDER_VERSION_V1` = `"mejepa-embedder-v1"`. `verify_provenance_consistent` re-derives and compares.

### 6.6 Adversarial + synthetic stress corpora

- `crates/context-graph-mejepa/src/adversarial_corpus.rs` — 30-case adversarial Python corpus (TASK-PY-G-060) → `CF_MEJEPA_ADVERSARIAL_CORPUS`.
- `crates/context-graph-mejepa/src/synthetic_stress.rs` + `synthetic_stress_cases.rs` + `synthetic_stress_corpus.rs` + `synthetic_stress_eval.rs` + `synthetic_stress_store.rs` — 30 known-answer cases (TASK-PY-G-071) → `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS`.
- `crates/context-graph-mejepa/src/project_stress.rs` — end-to-end reality-compiler stress harness (TASK-PY-G-072) → `CF_MEJEPA_STRESS_RUNS`.

---

## 7. Reality Compiler (per-project ingest)

`crates/context-graph-mejepa/src/project_ingest.rs` (~52K LOC), `project_cache.rs` (~46K, Merkle-tree cache per TASK-PY-G-062), `project_report.rs` (~44K).

| Capability | Source | Issue |
|---|---|---|
| Merkle-tree per-project chunk cache; embedding writes fail closed unless real active embedder outputs are supplied | `project_cache.rs` | #281 / TASK-PY-G-062, #424 |
| 250 ms debounced watcher-status scan | `project_cache.rs` | #281 |
| Transitive invalidation + schema-version-aware chunk refresh; real project-cache re-embed provider still required before live consequence prediction | `project_cache.rs` | #281, #424 |
| `mejepa_project_ingest` MCP tool | `project_ingest.rs` + MCP wrapper | #282 / TASK-PY-G-063 |
| `mejepa_project_report` MCP tool | `project_report.rs` + MCP wrapper | #283 / TASK-PY-G-064 |

Production output paths (aiwonder): `/zfs/archive/contextgraph/projects/<project_id>/{manifest.json, cache/, predictions/, report/{report.json, report.md}}` + `CF_MEJEPA_PROJECT_REPORTS`. Project-local CF rows persist alongside, with physical-readback FSV proof.

Performance gate (per phase-g-roadmap §4): fresh ingest of 50-file Python project < 60 s on RTX 5090; re-ingest after 1-file edit < 5 s.

---

## 8. Live Session Trace

`crates/context-graph-mejepa/src/live_session_trace.rs` (~30K). TASK-PY-G-056. End-to-end Claude Code session integration FSV. Writes `CF_MEJEPA_LIVE_SESSION_TRACES` keyed by `(session_hex32, ts_ns)`. Tooling: `mejepa observe`, `mejepa stop-self-verify`.

---

## 9. Training (UTML v1.0)

`crates/context-graph-mejepa-train/src/learning_signal.rs`. UTML `L_step = ΔP × ΔK × ΔΩ × ΔΞ` where:

| Term | What it measures | Source |
|---|---|---|
| `ΔP` | Predictor improvement | `DeltaPComponents` |
| `ΔK` | Catastrophic-forgetting violation (Fisher diag) | `DeltaKComponents` |
| `ΔΩ` | Plasticity (continual backprop) | `DeltaOmegaComponents` |
| `ΔΞ` | Structural-collapse safety margin (frozen-target invariant) | `DeltaXiComponents` |

`compute_l_step(...)`, `compute_per_head_learning_signal(...)`, `HeadSignalInput`, `LearningSignal`, `PerHeadLearningSignal`, `SkipReason`, `UtmlError` (with `UtmlErrorCode`).

### 9.1 Replay buffer (M(t) consolidation)

`replay_buffer.rs` (~20K). TASK-RWD-210 / TASK-PY-G-068. `BatchSampler::next_replay_batch` reads from `CF_MEJEPA_REPLAY_BUFFER` priority-sorted by surprise + per-cell coverage gap.

### 9.2 Inverse-map head (bidirectional training, TASK-EK-012)

`loss/inverse.rs` — `inverse_map_loss`, `compose_bidirectional_l_full`, `unit_gaussian_nll_bits`, `InverseMapLossBreakdown`, `InverseMapOutputs`, `InverseMapTargets`. Shared-trunk target-panel → input-panel + 16-d action latent. Coefficient `inverse_map_coefficient` in `TrainingConfig`. **Ablation result (2026-05-18):** all `λ_inv ∈ {0, 0.25, 0.5, 1.0}` measured 0.8666666746139526; best delta vs baseline 0.0; production stays `λ_inv = 0.0`.

### 9.3 EWC++ + Continual Backprop

- `aux/` — Fisher snapshots → `CF_MEJEPA_FISHER_SNAPSHOTS`. TASK-PY-G-065 EWC catastrophic-forgetting protection.
- `aux/` — dormancy histories → `CF_MEJEPA_PLASTICITY_HISTORY`. TASK-PY-G-066 continual backprop (Dohare 2024).

### 9.4 Compression progress (TASK-EK-001 — CPΦ as primary claim)

`compression_progress.rs`. `TrainingCertificate.conditional_description_length_bits`. `compression_progress_report(...)` returns `CompressionProgressReport` with `CompressionProgressMonotonicity` over `DEFAULT_COMPRESSION_PROGRESS_WINDOW` (~rolling weeks) and `DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS`. Renders into weekly markdown via `render_compression_progress_weekly_section`. Exposed by `mejepa_compression_progress` MCP + `context-graph-cli mejepa compression-progress`.

### 9.5 Per-fingerprint catalog stability (TASK-FP-008)

`CF_MEJEPA_FINGERPRINT_FISHER` + `CF_MEJEPA_FINGERPRINT_DORMANCY`. Per-fingerprint EWC + CBP for catalog stability under continual learning.

### 9.6 Online mistake-driven adaptation ("isn't wrong in the future" property)

Source files (planned): `crates/context-graph-mejepa/src/mistake_log.rs`, `crates/context-graph-mejepa-train/src/mistake_consumer.rs`, `crates/context-graph-mejepa-train/src/online_perceptron.rs`. Storage CFs: `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_MISTAKE_REPLAY_PRIORITY`, `CF_MEJEPA_ONLINE_HEAD_STATE`. MCP tools (planned): `mejepa_record_mistake`, `mejepa_mistake_history`, `mejepa_mistake_loop_status`. Tracked by **#406 (new) / TASK-PY-G-112 — Online mistake-driven learning loop**.

**The classical primitive.** The binary head over the panel is implemented as a margin-based online learner in the kernel-Perceptron / Second-Order-Perceptron family (Rosenblatt 1957; Cesa-Bianchi & Conconi 2005; Orabona 2008 Projectron). Update rule, per mistake:

```
if predicted_verdict != ground_truth_verdict:
    support_set := support_set ∪ { (panel, ground_truth_verdict) }
    head_weights := head_weights + η · ground_truth_verdict · panel_projection
    record_mistake_in_CF_MEJEPA_MISTAKE_LOG with full context
    enqueue at 8× weight in CF_MEJEPA_REPLAY_BUFFER
    boost Fisher diagonal for the affected slots (EWC++ already implemented)
```

The "correct on first try, learn when wrong" discipline maps directly onto the Perceptron mistake-bound regime: each mistake strictly improves the head on that specific `(cell × failure_mode × panel_signature)` cell, and the mistake bound is finite under the margin assumption. The Second-Order Perceptron variant uses ridge-style whitening so a single mistake updates the head against the local covariance of the panel, which adapts faster to new instrument additions.

**Ground-truth sources** (the "reality says yes/no" signal):

| Source | When | CF |
|---|---|---|
| SWE-bench Docker oracle | Corpus training + heldout eval | `CF_MEJEPA_ORACLE_VERDICTS` |
| PostToolUse Bash capture | Live agent session | `CF_MEJEPA_PREDICTION_VERIFICATIONS` |
| Shift-log replay (Q5) | After the patch lives in a session | `CF_MEJEPA_REALITY_IMPACT` |
| Operator override | Operator labels a row | `CF_MEJEPA_OPERATOR_OVERRIDES` |

Each ground-truth event materializes a `MistakeRow`:

```rust
pub struct MistakeRow {
    pub mistake_id: MistakeId,
    pub prediction_id: PredictionId,
    pub ts_ns: u64,
    pub cell_key: CellKey,                        // (mutation_category, language, entity_type)
    pub predicted_verdict: Verdict,               // what we said
    pub ground_truth_verdict: Verdict,            // what reality said
    pub predicted_named_failure_mode: Option<NamedFailureMode>,
    pub ground_truth_named_failure_mode: Option<NamedFailureMode>,  // from closest exemplar
    pub source_panel: PanelId,                    // panel A or B
    pub panel_signature_hash: [u8; 32],           // for dedup
    pub slot_traces: Vec<PerSlotOodReason>,       // #389 evidence at prediction time
    pub accepted_label_ids: Vec<AcceptedLabelId>,
    pub active_skill_ids: Vec<SkillId>,            // ordered; A->B differs from B->A
    pub label_signature_hash: LabelSignatureHash,
    pub skill_signature_hash: Option<SkillSignatureHash>,
    pub skill_lifecycle_decision: SkillLifecycleDecision,
    pub skill_lifecycle_audit_id: Option<SkillAuditId>,
    pub closest_exemplar_at_predict: ExemplarPointer,
    pub ground_truth_source: GroundTruthSource,
    pub replayed_to_trainer: bool,
}
```

Stored in `CF_MEJEPA_MISTAKE_LOG` keyed by `mistake_id` with secondary index by `cell_key`.

**Convergence target (the ship-gate-adjacent property).** For every `(cell_key × predicted_named_failure_mode × ordered_skill_signature)` tuple, the second occurrence of the same panel signature must produce the correct verdict. Operationalized as a metric: `mistake_repeat_rate = |second_mistake_on_same_cell_and_pattern| / |first_mistake_on_that_cell_and_pattern|`. Target: `≤ 0.05`. If this rate climbs above 0.05 on any rolling window, the heal scheduler is failing to consolidate; promotion blocked.

**Why this beats best-of-K.** The user's constraint: "if it needs 4 guesses to be most correct then it is already wrong." Best-of-K papers over an under-trained predictor by spending compute at inference time. The online perceptron pays the cost at *learning* time and converges to a predictor that is right on the first try. Best-of-K stays available as a debug tool but is never the product path.

**FSV expectations.** Synthetic + real FSV must prove (a) a deliberately-injected wrong prediction lands in `CF_MEJEPA_MISTAKE_LOG`, (b) the next presentation of the same panel signature yields the corrected verdict, (c) the mistake_repeat_rate metric is byte-readable from RocksDB, (d) Fisher protection prevents the mistake update from regressing unrelated cells, (e) `skill:A -> skill:B` and `skill:B -> skill:A` produce distinct replay keys, and (f) the same mistake writes a skill lifecycle audit row that updates, splits, creates, demotes, or explicitly leaves unchanged the involved skill/ability model.

---

## 10. Heal Scheduler

`crates/context-graph-mejepa/src/heal/` (5 modules). Reads drift signals, decides among:
- **Retrain** — append new training step; write `CF_MEJEPA_TRAIN_CERTS`, update `CF_MEJEPA_DRIFT_WINDOW`.
- **Promote** — flip `CF_MEJEPA_ACTIVE_POINTERS` to a new model (TASK-EK-018H operator-approval-gated for first promotion + catastrophic class).
- **Rollback** — revert active pointer; emit `CF_MEJEPA_HEAL_REPORTS` row.
- **Pause** — `mejepa_pause_predictions` (Phase F pause-state).
- **Distill** — distill from a champion model to a smaller learner; rows in `CF_MEJEPA_DISTILL_STEPS`.

Per-cell convergence driver (TASK-G-002) tracks per-cell `prediction_oracle_correlation`, drives targeted mutation expansion (TASK-G-003), conformal recalibration (TASK-G-004), holdout rotation (TASK-G-005), four-window ship-gate stability (TASK-G-006), per-cell exemptions (TASK-G-007).

MCP surfaces: `mejepa_heal_status`, `mejepa_daemon_status`, `mejepa_rollback_to`, `mejepa_promote_approval`.

---

## 11. Shift Subscriber

`crates/context-graph-mejepa-shift-subscriber/src/` (~95 LOC across 9 modules). Tokio task:
1. `tail.rs::ShiftLogTail` tails `runtime/shift-log.jsonl`.
2. `models.rs` decodes `ShiftEntry` (incl. `ShiftId`, `ShiftSide`, `ShiftOutcome`, `PanicInfo`, `LatencySnapshot`, `UtmlFactorBundle`, `CaptureAuditBundle`, etc.).
3. `shift_transform.rs::shift_to_inference` maps shift → inference input.
4. `subscriber.rs::ShiftSubscriber::run()` materializes a `Panel` (using `cache.rs::InstrumentCache`) and writes `CF_MEJEPA_PANELS`.
5. `watermark.rs::WatermarkWriter` persists watermark to `CF_MEJEPA_SHIFT_WATERMARK`. This is the durability gate.
6. `status.rs::LiveSubscriberStatus` + `SubscriberMetrics` surfaced via `mejepa_subscriber_status`.

Fail-closed on `decode_session_hex32` mismatch, malformed shift entries, watermark regression, etc. (`SubscriberError`).

---

## 12. Conformal + OOD + Cold-cell

| Module | Purpose | Source |
|---|---|---|
| `conformal.rs` | Split-conformal predictor: per-head α=`INFER_DEFAULT_ALPHA`, calibration window over `CF_MEJEPA_CALIBRATION_HISTORY` | `crates/context-graph-mejepa/src/conformal.rs` |
| `calibration.rs` + `calibration_types.rs` | `CalibrationCertificate`, per-head calibration | `crates/context-graph-mejepa-embedders/src/calibration.rs` + mejepa |
| `ood.rs` | Residual OOD score math and AUC helpers. Static `INFER_DEFAULT_OOD_REFUSE_THRESHOLD` remains compatibility fallback only when strict OOD calibrator mode is disabled. | `crates/context-graph-mejepa/src/ood.rs` |
| `ood_harvest/` + `ood_harvest.rs` | TASK-PY-G-057 / #276 live-session harvest pipeline plus TASK-PY-G-014 / #129 serving calibrator: above-threshold predictions → `CF_MEJEPA_OOD_HARVEST`, per-cell threshold/AUC/FPR/recall → `CF_MEJEPA_OOD_CALIBRATIONS`, 4× active-learning projection, orphan/degenerate quarantine, expired-row tier-down, operator-labeled downweight, `OOD_HARVEST_EMPTY`, `OOD_CELL_AUC_REGRESSION`, `OOD_GATE_OVER_FLAGGING`, `OOD_CALIBRATOR_MISSING`, and `OOD_SCORE_ABOVE_THRESHOLD` decisions | `crates/context-graph-mejepa/src/ood_harvest.rs` |
| Per-slot OOD reasons | `RealityPrediction.per_slot_ood_reasons: Vec<PerSlotOodReason>` materialized from `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`; persists slot/embedder/pair-specific OOD evidence per prediction; exposed via `mejepa_explain_prediction` / `mejepa_inspect_prediction`; `MAX_EMBEDDERS_PER_PANEL` guard | TASK-PY-G-089 / #389 |
| `ColdCellMetric` (in `compiler.rs`) | Abstain when `n_supporting < INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT` (50); writes `CF_MEJEPA_COLD_CELL_METRICS` | TASK-PY-G-061 |

---

## 13. Foundationality + Bedrock Consistency

| Surface | Source | CF |
|---|---|---|
| Chunk dependency graph | `chunk_foundationality.rs` (~90K) | `CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH` |
| Chunk foundationality (PageRank) | `chunk_foundationality.rs` | `CF_MEJEPA_CHUNK_FOUNDATIONALITY` |
| Library registry / per-library / cross-library | `library_foundationality.rs` (~38K) | `CF_MEJEPA_LIBRARY_REGISTRY`, `CF_MEJEPA_LIBRARY_FOUNDATIONALITY`, `CF_MEJEPA_CROSS_LIBRARY_REFERENCES` |
| Embedder dependency graph / foundationality | `embedder_foundationality.rs` (~20K) | `CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH`, `CF_MEJEPA_EMBEDDER_FOUNDATIONALITY` |
| MCP bedrock-consistency check | `mejepa_check_bedrock_consistency` tool | reads foundationality CFs |
| MCP library-foundationality surface | `mejepa_library_foundationality` tool | reads `CF_MEJEPA_LIBRARY_FOUNDATIONALITY` |

PageRank scores weight `BatchSampler` and Fisher (TASK-EK-016 / EK-017 / EK-016-ext).

---

## 14. Q1 Claim Graph

`crates/context-graph-mejepa/src/claim_graph.rs` (~30K). TASK-PY-G-041.

- Deterministic claim parser over agent response bodies.
- Reconciles claims against bytes-on-disk + AST.
- Validates `repo_after_sha` reachability.
- Writes sync/readback rows to `CF_MEJEPA_CLAIM_RECONCILIATION`.

---

## 15. Q3 Patch Similarity / Closest Exemplars

`patch_similarity.rs` (~28K). TASK-PY-G-042.

- usearch HNSW index over L2-normalized `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` signatures.
- Metadata in `CF_MEJEPA_PATCH_SIMILARITY_META`.
- Index storage at `/mnt/d/contextgraph/storage/patch-similarity-index-v1/...` (local FSV; aiwonder production path equivalent).
- Compiler-side `RealityPrediction.closest_exemplars` population.
- 2026-05-18 FSV: top-1 same-failure-mode accuracy `1.0`.

---

## 16. Q4 Heads — OUT OF SCOPE (frozen as display-only historical data, 2026-05-19)

**Status: Q4 heads are removed from the prediction contract.** They remain in the schema, the existing CFs (`CF_MEJEPA_Q4_PERF_LABELS`, `CF_MEJEPA_Q4_ACCURACY_LABELS`, `CF_MEJEPA_Q4_COST_LABELS`, `CF_MEJEPA_Q4_REASONING_LABELS`, `CF_MEJEPA_Q4_PASS_ANNOTATIONS`, `CF_MEJEPA_Q4_SECURITY_LABELS`) are preserved for forensic continuity, and `mejepa_explain_prediction` can still surface their historical rows. But:

- **No new Q4 training** — Q4 producers are frozen; no new rows are written by the production pipeline after 2026-05-19.
- **No Q4 calibration prerequisite for ship** — the 0.95 ship gate measures Q2 oracle correlation only. Q4 readiness is not a prerequisite.
- **No Q4 contribution to the predictor's loss** — `inverse_map_coefficient` and any future per-head coefficients on Q4 heads default to 0 and stay there. The trainer ignores Q4 producer rows when assembling the replay batch.
- **No Q4 trust gate consumption** — `mejepa_predict_latest` and `mejepa_inspect_prediction` continue to expose `q4.rawObservations` for read-only audit; `q4.trustedConsequences` is no longer populated by new predictions.

**Code enforcement added by #408 / TASK-PY-G-114.** `Q4_DOCTRINE_FREEZE_ACTIVE` is true in `q4_trust_gate.rs`; the trust gate forces every Q4 head to `trusted_in_decision=false`, the builder and prediction-surface inference clear Q4 display-only fields on new predictions, `Q4_LOSS_COEFFICIENT_MAX` is pinned to `0.0`, and `q4_inputs_to_ship_gate()` returns `0`. `mejepa_weekly_eval_dashboard` now has a `q4Freeze` section that reads the frozen-CF baseline and fails closed if any deprecated-readable Q4 CF grows.

**Why removed.** Q4 questions are ambiguous (§0): they require an arbitrary threshold on a continuous heuristic, or ask a taste question that reality does not render a binary verdict on. Predicting them trains the system to match heuristic tools (mutmut, semgrep, vulture, hyperfine, pytest-benchmark) rather than reality. The system does not predict ambiguous things because predicting noise corrupts the mistake-driven learning loop (§9.6).

**Historical record of the frozen Q4 producers (kept here for continuity):**

| Frozen head | Source (frozen) | Final state |
|---|---|---|
| Predicted security concerns | `q4_security_labels.rs` (Bandit + Semgrep) | Frozen 2026-05-19; `CF_MEJEPA_Q4_SECURITY_LABELS` remains readable; #402 closed as `wontfix` under the ambiguity boundary |
| Predicted perf regressions | `q4_perf_labels.rs` | Frozen; 260 historical rows in `CF_MEJEPA_Q4_PERF_LABELS` preserved; #266, #399 closed as `archived-display-only` |
| Predicted accuracy degradations | `q4_accuracy_labels.rs` | Frozen; 240 historical rows in `CF_MEJEPA_Q4_ACCURACY_LABELS` preserved; #267 closed as `archived-display-only` |
| Predicted cost regressions | `q4_cost_labels.rs` | Frozen; 240 historical rows in `CF_MEJEPA_Q4_COST_LABELS` preserved; #268, #400 closed as `archived-display-only` |
| Reasoning class | `q4_reasoning_labels.rs` | Frozen; 300 historical rows in `CF_MEJEPA_Q4_REASONING_LABELS` preserved; #269 closed as `archived-display-only` |
| Non-trivial-on-Pass | `prediction_surfaces.rs` | Frozen; 500 historical rows in `CF_MEJEPA_Q4_PASS_ANNOTATIONS` preserved; #270 closed as `archived-display-only` |
| Q4 confidence calibration | `HEAD_CALIBRATIONS` consumer | Frozen; #127 closed as `archived-display-only` |
| Q4 trust gate | `mejepa_predict_latest` exposure | Frozen as a no-op pass-through; #377 closed as `archived-display-only` |
| Q4 harvest expansion | #401 | Closed as `archived-display-only` |
| Q4 security/structural producer expansion | #402 | Closed as `wontfix` (ambiguity boundary) |

The corresponding `CF_*_Q4_*` CFs are NOT deleted (would force a schema-version bump and DB rewipe); they are deprecated-readable. A future schema-version bump may delete them; tracked separately under `phase:cleanup`.

**#408 FSV.** One-time freeze proof lives at `/zfs/archive/contextgraph/fsv/q4-doctrine-freeze-fsv/`: `allPassed=true`, `q4RowCountDelta=0`, `q4LossCoefficientMax=0.0`, `q4InputsToShipGate=0`, historical Q4 display-only fields remain readable, and new predictions return zero Q4 display-only fields. The proof also covers `q4cal::*` row-count freeze in `CF_MEJEPA_HEAD_CALIBRATIONS`. Main FSV sha256 `23408b7d09571ca123447d05b365d1ac33b200142be9ce64a75d07c163eb32c3`; independent readback sha256 `5e2f0213a36d9463bd2f630d45f1928fd3c2a80e21b1fa7e06825d80c10412d5`.

**What replaces Q4 in the operator-facing answer.** Operators who used to want "what else could go wrong" answers get them from the **named failure mode** Q3 surface (§4.3): if the binary verdict is Fail, the named failure mode + closest exemplar tell the operator the specific named cause. If the binary verdict is Pass, the operator-facing answer is "Pass + reasons-to-trust: claim landed in bytes (Q1=Yes), oracle agrees (Q2=Pass), closest exemplars all Pass" — not a list of speculative ambiguous risks.

---

## 17. Q5 Reality Impact

`reality_impact.rs`. Shift-log replay engine (TASK-PY-G-045). Replays predicted side effects against the actual shift log and persists `RealityImpactRecord` rows in `CF_MEJEPA_REALITY_IMPACT`.

Current shipped behavior (2026-05-19, #264):
- Reads persisted `RealityPrediction` rows from `CF_MEJEPA_LIVE_PREDICTIONS` by `prediction_id`.
- Parses the session `cgreality-shift-log/<session>.jsonl` over a bounded replay window.
- Classifies predicted consequences as `Confirmed`, `Missed`, or `NotYetObserved`, and unpredicted observed consequences as `Surprise`.
- Fails closed on missing predictions, out-of-order timestamps, explicit shift-log SHA-chain breaks, unverifiable file-shift SHA evidence, and per-file before/after SHA discontinuity.
- MCP surface: `mcp__cgreality__mejepa_reality_impact`.
- aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-045-shift-log-replay-engine-fsv/shift_log_replay_engine_fsv.json`, sha256 `2e2116e2c0f15c2861d52798b1aefe0435b8acda50399f4a0547b51190fb0959`; independent reopened-RocksDB readback sha256 `8ef576a9353015ea1e4957b753dbe4c717cc92431fcd58824574ca159ee55df5`.

Current calibration behavior (2026-05-19, #128):
- Consumes durable `RealityImpactRecord` rows from `CF_MEJEPA_REALITY_IMPACT`.
- Writes typed `Q5CalibrationRecord` rows to `CF_MEJEPA_Q5_CALIBRATIONS`, keyed by `calibration_id`, `language`, `mutation_category`, and `side_effect_kind`.
- Computes true positives, false positives, false negatives, awaiting-oracle count, support, precision, recall, F1, surprise rate, sample prediction IDs, and replay-exclusion reasons.
- Excludes fail-closed replay errors such as `SHIFT_LOG_CHAIN_BROKEN` from metric denominators while preserving explicit `ReplayExcluded` evidence.
- Emits flags for `AwaitingOracle`, `Q5HighSurpriseRate`, `Q5CellNeedsRetraining`, and dominant-class F1 below floor.
- aiwonder FSV: `/zfs/archive/contextgraph/fsv/task-py-g-013-q5-calibration-fsv/q5_calibration_fsv.json`, sha256 `042bd8ad45df924b6f607c3c2f2bcacc339ad91a11e5457bd1da52b24b22f133`; independent reopened-RocksDB readback sha256 `aa8f56ec0be0bc2e85e9232f391a2f7e3b9c013db37a5ba2091135c85df45a26`; sample sha256 `a0ce89e3a859b634677ab14222f8b5ce8875740074b621cdf9e2f04041db7731`.

---

## 17bis. Pathway Exploration Surface (the binary-leaf butterfly-effect tree — planned, #416 / TASK-PY-G-119)

This is the L3 capability in the JEPA world-model rubric (arXiv 2511.16333) — **counterfactual rollouts for decision support** — instantiated on the Q1 / Q2 / Q5 binary-leaf substrate. See `01_overview.md §1.5` for the conceptual framing. This section specifies the operational architecture.

**Status:** planned (not shipped). Tracked under the new epic **#416 / TASK-PY-G-119 — Pathway exploration surface (binary-leaf butterfly-effect tree)**.

### 17bis.1 Architecture

Planned source modules (additive to `context-graph-mejepa`):

| Module | Role |
|---|---|
| `src/pathway/tree.rs` | `PathwayNode { leaf_kind: Q1\|Q2\|Q5, leaf_id, predicted_outcome: bool, predicted_prob: f32, conformal_interval, parent_path_prob, children: Vec<NodeId> }` + `PathwayTree` build / prune / serialize |
| `src/pathway/beam_search.rs` | Beam expansion with UCB1-style selection (Narrative Studio + SWE-Search pattern); top-K retention by cumulative path probability (Diffusion Scenario Tree pattern, arXiv 2509.14832); pruning threshold `ε ≈ 0.01` for `P(path)` |
| `src/pathway/leaf_predictor.rs` | Routes each candidate leaf to the right predictor head (Q1 claim-graph head, Q2 oracle head, Q5 shift-event head). Strict assertion: any non-Q1/Q2/Q5 leaf attempt → `PATHWAY_AMBIGUOUS_LEAF_REJECTED` error |
| `src/pathway/exemplar_attach.rs` | For each surfaced pathway, retrieve the closest historical pathway from the 491-row Level-0 accepted label registry (#413) keyed by leaf-sequence signature |
| `src/pathway/skill_attach.rs` | Attach active skill / higher-ability IDs and source chunk memberships from #414/#417 to each leaf when available; emit typed `UNKNOWN_PATHWAY_SIGNATURE` when no safe linkage exists |
| `src/pathway/store.rs` | `CF_MEJEPA_PATHWAY_TREES` + `CF_MEJEPA_SURFACED_PATHWAYS` RocksDB writers |
| `examples/task_py_g_118_pathway_surface_fsv.rs` | FSV harness: 3-5 surfaced pathways, joint probabilities sum-normalized within the beam, ambiguous-leaf rejection, byte-replay, mistake-loop integration |

### 17bis.2 Storage CFs (planned, additive to `mejepa-cf/lib.rs`)

| CF Constant | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_PATHWAY_TREES` | `(prediction_id, tree_run_id)` | bincode `PathwayTree { root_patch_id, nodes, beam_width, depth, pruning_epsilon }` | Full pathway tree for a candidate patch; byte-replayable per FSV |
| `CF_MEJEPA_SURFACED_PATHWAYS` | `(prediction_id, pathway_id)` | bincode `SurfacedPathway { leaf_chain: Vec<BinaryLeaf>, joint_probability, closest_historical_pathway, named_failure_mode_if_fail_leaf, active_skill_ids, source_chunk_memberships }` | The top-K pathways actually returned to the operator; skill/chunk attachment is evidence only, not a new leaf type |
| `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` | `(prediction_id, chooser_id)` | bincode `{chosen_pathway_id, chose_at_ts_ns, rationale_text?}` | Operator's chosen future; feeds the reverse-prediction tool (#335 `mejepa_reverse_prediction`) when it ships |

All three are **target-side leakage-policy:target_side_only** for live-input training purposes (the chosen pathway is a downstream effect of the operator's choice, not a live predictor input).

### 17bis.3 MCP tools (planned)

| Tool | Purpose |
|---|---|
| `mcp__cgreality__mejepa_pathway_surface(prediction_id, k: int=5)` | Compute and return the top-K most-probable pathways for a candidate patch |
| `mcp__cgreality__mejepa_pathway_inspect(prediction_id, pathway_id)` | Return full leaf chain + per-leaf probabilities + conformal intervals + closest historical exemplar |
| `mcp__cgreality__mejepa_pathway_record_choice(prediction_id, pathway_id, rationale_text?)` | Record the operator's chosen future; idempotent |
| `mcp__cgreality__mejepa_pathway_history(window_id?)` | Read-only audit over `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES` |

### 17bis.4 Algorithm (beam search over binary leaves)

```
fn surface_pathways(candidate_patch: Patch, k: usize = 5, beam_width: usize = 32,
                    max_depth: usize = 6, pruning_epsilon: f32 = 0.01) -> Vec<SurfacedPathway> {
    let root = PathwayNode::seed(candidate_patch);             // Q1 binary leaf: claim_exists
    let mut frontier: Vec<PathwayNode> = vec![root];
    for depth in 0..max_depth {
        let mut next_frontier = Vec::new();
        for node in &frontier {
            let children = node.expand_binary_children();      // ONLY Q1/Q2/Q5 leaves
            for child in children {
                if !child.leaf_kind.is_binary() {
                    return Err(PATHWAY_AMBIGUOUS_LEAF_REJECTED); // ambiguity boundary
                }
                let predicted = predictor.binary_head_predict(child);
                child.path_prob = node.path_prob * predicted.calibrated_prob;
                if child.path_prob >= pruning_epsilon {
                    next_frontier.push(child);
                }
            }
        }
        // beam_width retention by path_prob (Diffusion Scenario Tree pattern)
        next_frontier.sort_by(|a, b| b.path_prob.partial_cmp(&a.path_prob).unwrap());
        next_frontier.truncate(beam_width);
        frontier = next_frontier;
        if frontier.is_empty() { break; }
    }
    // Normalize within the surviving frontier; return top-K terminal pathways
    let total: f32 = frontier.iter().map(|n| n.path_prob).sum();
    frontier.iter_mut().for_each(|n| n.path_prob /= total);
    frontier.sort_by(|a, b| b.path_prob.partial_cmp(&a.path_prob).unwrap());
    frontier.truncate(k);
    frontier.into_iter().map(SurfacedPathway::from).collect()
}
```

The expansion grammar for `PathwayNode::expand_binary_children` is the operative spec; it MUST only emit Q1/Q2/Q5 leaves. Adding any other leaf type to the grammar is forbidden until and unless the ambiguity-boundary rule is revised.

### 17bis.5 Required FSV assertions

1. **Ambiguous-leaf rejection.** `ambiguous_leaves_in_pathway == 0` across every surfaced pathway in the holdout window. Attempting to inject a non-Q1/Q2/Q5 leaf fails closed with `PATHWAY_AMBIGUOUS_LEAF_REJECTED`.
2. **Joint probability normalization.** For each surfaced top-K set, joint probabilities are normalized (`sum(P(pathway_i)) == 1.0 ± 1e-4` after frontier normalization).
3. **Byte-replay.** Every node, every edge probability, every per-leaf conformal interval byte-replayable from `CF_MEJEPA_PATHWAY_TREES` after flush/drop/reopen.
4. **Closest-exemplar attachment.** Every surfaced pathway has a non-empty `closest_historical_pathway` field grounded in the 491-row Level-0 accepted label registry from #413 (or typed-empty for novel pathways with explicit `UNKNOWN_PATHWAY_SIGNATURE`).
5. **Skill/chunk attachment.** Every Fail leaf and every Q5 leaf carries active skill IDs plus source chunk memberships from #414/#417 when live-input-safe evidence exists, or a typed `UNKNOWN_PATHWAY_SIGNATURE` reason when no safe skill linkage exists.
6. **Mistake-loop integration.** When reality renders any binary leaf of a surfaced pathway and the predicted outcome was wrong, the disagreement materializes as a `CF_MEJEPA_MISTAKE_LOG` row (per #406 / TASK-PY-G-112). The mistake row records `(pathway_id, leaf_id, predicted, observed, active_skill_ids, skill_signature_hash)` so the trainer can credit-assign across the pathway and update/split/create/demote/no-change the involved skill model, not just on the candidate-patch root.
7. **Goodhart-safe surfacing.** No pathway is surfaced solely on the predictor's own confidence — every leaf's calibration row must be present in the disjoint calibration partition (#407 / TASK-PY-G-113). Pathways through cold cells (`n_supporting < INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT`) carry an explicit `cold_cell_warning` flag.

### 17bis.6 Operator-facing semantics

The operator sees a list like:

```
Candidate patch: <patch_sha256>
Most probable futures (top-5):

[1] P = 0.42  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=Yes → Q5.next_session_admin_save=Yes
                "patch lands, tests pass, isolation holds, downstream admin.save() consumer unaffected"
                Closest historical: django__django-13230 (PASS) — registry label `chunk:source_site:exact_mutation_overlap+pair:e7_e9:zero_fraction_high`

[2] P = 0.21  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=Yes → Q5.next_session_admin_save=No
                "patch lands, tests pass, isolation holds, BUT downstream admin.save() consumer breaks next session"
                Closest historical: django__django-11211 (FAIL after merge) — registry label `code_state:multi_point_evidence_set`
                ⚠ named failure mode (#404 hierarchy): `downstream_consumer_regression`

[3] P = 0.17  →  Q1=Yes → Q2=Fail
                "patch lands but oracle rejects (target_fail_to_pass tests failed)"
                Closest historical: django__django-12286 (FAIL) — registry label `mutation_mechanism:subtle_flip_invariant_violation`

[4] P = 0.12  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=No
                "patch lands, oracle passes solo, BUT test isolation breaks when other tests run"
                ⚠ named failure mode: `mock_only_change`

[5] P = 0.08  →  Q1=No
                "patch will not land in bytes as claimed (claim-vs-AST mismatch)"
```

The operator picks one of `[1]–[5]`, the system records the choice in `CF_MEJEPA_OPERATOR_PATHWAY_CHOICES`, and (when #335 `mejepa_reverse_prediction` lands) the system produces patch-shape constraints that would satisfy the chosen pathway's binary leaves.

This is what "the system enlightens the operator on the butterfly-effect pathways and lets them choose which path to choose" means architecturally: every node is a binary leaf the predictor commits on, every pathway is a conjunction of falsifiable binary claims, every probability is calibrated against the disjoint calibration partition, and every operator choice is a durable row that feeds back into the mistake-loop training signal.

### 17bis.7 What this is NOT

- **Not generative.** Pathways are projections of the predictor's existing binary-head knowledge into a tree, not invented futures.
- **Not ambiguous.** Q4 heads (perf / accuracy / cost / latent bugs / tech debt / idiomaticity) cannot enter a pathway. The ambiguity boundary applies leaf-by-leaf.
- **Not best-of-K patch selection.** Pathway surfacing operates on *one* candidate patch and shows its possible future trajectories. It is NOT the §0 "best-of-K candidates" pattern that the binary doctrine forbids.
- **Not a probability ranking over ambiguous outcomes.** "How performant" / "how maintainable" are never pathway nodes. Pathway probabilities multiply binary leaf probabilities only.
- **Not unlimited.** Beam width × max depth bounds the tree; default `beam_width=32`, `max_depth=6`, `pruning_epsilon=0.01`, `k=5`. Operator can tune within sane bounds; pathway counts above 5 typically exceed operator decision bandwidth.

### 17bis.8 Open question: ambiguous-pathway extension

Whether the pathway surface can ever expand to ambiguous leaves (e.g. "this pathway probably degrades perf by ≥10%") is unanswered. The current ambiguity-boundary rule says no — ambiguous leaves cannot be falsified by reality, so a pathway through one cannot be probability-weighted by the mistake loop in a way that converges. Whether some richer formulation (interval-typed pathway leaves with externally falsifiable thresholds against deterministic tools? typed-uncertainty leaves with explicit calibration partitions?) can include them is future work and is **explicitly out of scope for #416 / TASK-PY-G-119**. The FSV asserts `ambiguous_leaves_in_pathway == 0` at every release.

---

## 18. Adversarial Defenses

| Defense | Source | Issue |
|---|---|---|
| Negative-action ablation falsifier | `crates/context-graph-mejepa-train/examples/phase_g_adversarial_retrain_fsv.rs` + CF_MEJEPA_ABLATION_REPORTS | TASK-PY-G-054 |
| Multi-head contradiction | `contradiction.rs` + `verdict_assembly.rs` + `CF_MEJEPA_CONTRADICTION_THRESHOLDS`; per-cell thresholds fail closed on missing calibration and abstain on calibrated Q2/Q3/Q4 conflict | TASK-PY-G-044 |
| Constellation disagreement pressure | `constellation_intelligence.rs` + `verdict_assembly.rs` + `CF_MEJEPA_CONSTELLATION_PATTERNS`; slot-pair contradiction/novelty/blind-spot pressure can abstain and queue active learning when a relationship beats the own-slot baseline | TASK-PY-G-090 |
| OOD gate floor | `ood.rs` + `ood_harvest.rs`; `mejepa_verify` consumes selected `CF_MEJEPA_OOD_CALIBRATIONS` when `require_ood_calibrator=true`, fails closed on missing/corrupt/unselected calibrator, and preserves cold-cell precedence | TASK-PY-G-014 |
| Conformal interval floor | `conformal.rs` | TASK-PY-G-006 |
| Adversarial-embedder-injection mitigation | `adversarial_embedder_injection.rs` | TASK-EK-018J |
| Secret redaction (19 patterns, < 1ms p99 on 10KB) | `scripts/swebench_lite_ops/python_secret_redaction.py` wrapped by `secret_redaction.rs` → `CF_MEJEPA_REDACTION_LOG` | TASK-PY-G-059 (#278) |
| Cross-language label gating | `cross_language_label_gating.rs` | Phase F |
| 30-case adversarial Python corpus | `adversarial_corpus.rs` → `CF_MEJEPA_ADVERSARIAL_CORPUS` + fingerprint catalog | TASK-PY-G-060 / TASK-FP-009 |

---

## 19. What This Doc Does NOT Cover

- The exact transformer layer counts / hidden dims at runtime — read `MeJepaInferConfig` from the active checkpoint manifest.
- Per-cell calibration scores — live in `CF_MEJEPA_CALIBRATION_HISTORY` and `CF_MEJEPA_FINGERPRINT_CALIBRATION`; inspect via `mejepa_constellation_inspect` and `mejepa_fingerprint_inspect`.
- Heal-scheduler scheduling cadence — controlled by `~/.config/mejepa/daemon.toml` and per-cell convergence ETA tracking (TASK-G-021).
