# 05 — Predictor Pipeline (Embedders → Instruments → Panels → Heads → TCT → Training)

**Source files:** `crates/context-graph-mejepa-embedders/src/lib.rs` + modules, `crates/context-graph-mejepa-instruments/src/lib.rs` + modules, `crates/context-graph-mejepa/src/lib.rs` + `compiler.rs` + `verdict_assembly.rs` + `claim_graph.rs` + `patch_similarity.rs` + `failure_fingerprint.rs` + `project_ingest.rs`, `crates/context-graph-mejepa-tct/src/lib.rs` + modules, `crates/context-graph-mejepa-train/src/lib.rs` + `learning_signal.rs` + `replay_buffer.rs` + `mistake_log.rs` + `online_head_state.rs` + `loss/inverse.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs`, `crates/context-graph-mejepa-shift-subscriber/src/lib.rs`, `crates/context-graph-mejepa/src/heal/`.

---

## 0b. ⚠ 2026-05-23 3-Sherlock audit — pipeline reality

The pipeline downstream of the embedders currently rests on a split training story: the in-repo Rust `Trainer` is certificate-only, while the latest aiwonder ship-gate FSV loaded an out-of-band task-py-g-127 checkpoint (`optimizer_steps=64`, `weight_delta_observed=true`). That checkpoint is not produced by the public `Trainer` API and is not countable under the trainer's own `trained_predictor` / `ship_gate_countable_training` contract. Combined with 4/12 dead embedder slots, the `0.7942190170` rank-lift measurement is gate-metric Goodhart, not a clean predictor evaluation. See #712 (pinned context) and `docs/SHERLOCK_{PREDICTOR,SUBSTRATE,INFRA}_FINDINGS_2026-05-22.md`.

**Predictor-head defects:**

| Issue | Surface | File:line | Defect |
|---|---|---|---|
| **#683** | trainer | `mejepa-train/src/trainer/mod.rs:31-34, 409-422` | Hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`; no optimizer, no backward pass, no predictor field; `predictor_parameter_update_count=0`; `ship_gate_countable_training=false` |
| **#683** | production CLI | `mejepa/src/cli.rs:307-314` | Checkpoint loading depends on `trained_checkpoint_manifest_path`; if absent, the compiler can construct a fresh `VarMap` predictor. The latest aiwonder FSV did load `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json`, so #683 is a fail-closed/countability defect, not proof that that specific FSV used random weights |
| **#684** | OOD reporting | `compiler.rs:2267-2296` | `per_slot_ood_reasons` is built 1:1 from `guard_violations` — same data, different name; live serving never computes per-slot residuals |
| **#686** | OOD policy | conformal policy | `interval_width_threshold=0.8` hardcoded; forces cold-start OOD on under-1000-outcome cells |
| **#689** | mistake loop | `online_head_state.rs` | LUT keyed on exact panel-signature hash; `protection_mode=ExactPanelKeyIsolation`; production panels almost never collide → LUT inert |
| **#699** | confidence | `compiler.rs` | `calibrated_confidence_multiplier` derived from pseudo-certificates rather than real learning signal |

**Embedder-substrate defects (4 of 12 active slots dead):**

| Issue | Slot | Root cause | Symptom |
|---|---|---|---|
| **#704** | `e2`, `e3`, `e4` | `chunk.time` wall-clock injection at `python_forward_cache_fsv.rs:2308` identical for all 399,777 chunks | Constant sinusoidal-PE output; 1 unique vector per slot |
| **#704** | `e8` | `GraphModel::embed_batch()` missing `project_source_vector` call before commit `ff4d4c03` (2026-05-22); prod corpus generated 7 days earlier | Byte-identical to `e1`; full 1024-float equality; identical `vector_sha256` |
| **#705** | distill / heal | cross-embedder InfoNCE without slot-identity preservation | Doctrine §1.5ter violation in distillation path |
| **#706** | latent search | cross-embedder delta computation | Compares across non-comparable coordinate systems |
| **#709** | FSV gate | absence of inter-embedder distinctness check | Allowed `e8 ≡ e1` collapse to ship undetected |
| **#710** | `PanelBuilder` | accepts degenerate slot vectors | Should fail-closed on rank-1 / variance-0 slot input |
| **#670** | overall | regenerate/retire `e2/e3/e4/e8` before any geometry-label promotion | Substrate-level rebuild |

**Gate-metric defect:**

| Issue | Surface | Defect |
|---|---|---|
| **#695** | ship-gate measurement | Rank-lift = category-local RBF exemplar similarity model fitted on calibration split; lifts raw `0.4843 → 0.7942` without improving Brier/ECE; **measures exemplar similarity, not predictor**. Honest gap = `0.4657`, not `0.1558` |
| **#692** | abstain policy | `python_calibration_active_gate.py:322-378` selects `coverage` + `margin_threshold` per cell to maximize gate pass on calibration split; active rows = high `\|score-0.5\|` = easy rows by construction; the `0.953` "diagnostic" is selection bias |

**Carryover from 2026-05-22 5-Sherlock audit:**

| Issue | Surface | Defect |
|---|---|---|
| #618 | `compiler.rs:502` aggregate OOD | Flat 5,120-byte buffer (doctrine §1.5ter) |
| #619 | `mejepa-train/src/loss/inverse.rs::forward_inverse_inner` | Flat `Linear(PANEL_DIM, hidden)` projections (doctrine §1.5ter) |
| #620 | training loop | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook from `frozen_hook.rs` never invoked |
| #621 | HoldoutReport publisher | Hardcoded `gtau_pass_rate: 1.0` to `mejepa_weekly_eval_dashboard` |
| #623 | `cold_start_conformal_policy` | Emits `alpha=0.0` (dead public API + landmine) |
| #625 | `lib.rs` public re-exports | `DeterministicPredictor` publicly re-exported (synthetic-vector landmine post-#422) |

**Pipeline implication.** Every #392 / #511 / #393 / #398 / #536 / #549 / #554 / #558 / #560 measurement to date measured (checkpoint-loaded but trainer-orphaned predictor) composed with (4/12 dead slots) and (exemplar-similarity Goodhart magnifier). Five #392 trajectory repairs hit ECE walls because they perturbed this composite, not because of proven calibration limits.

**Geometry-label epic (#657, blocked behind #670).** Per-embedder vector-space label discovery via Leiden-on-kNN / HDBSCAN / spherical k-means / UMAP-diagnostic bakeoff. Cannot ship until #670 resolves degenerate spaces.

**Auto-FSV retirement (#655).** Predictor-pipeline FSV harnesses in §6/§7 retired. Manual operator-driven SoT readback (`06 §1.9`) is binding closeout.

---

## 0a. ⚠ Synthetic-vector remediation (2026-05-19/20) — real-input baseline active

Between Phase G ramp and 2026-05-19 22:45 UTC, **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of real frozen `E*Instrument` encoders. Forward cache pipeline (`crates/context-graph-mejepa-embedders/examples/python_forward_cache_fsv.rs`, ~24h GPU, `precision_class: "candle_real_forward"`) was unaffected.

**Six P0 fixes shipped 2026-05-19/20 (all closed):**

| Issue | Fix |
|---|---|
| #422 | Production inference panels route through real `E*Instrument` encoders (`compiler.rs::build_inference_panel_inputs`, commit `e2c51669`) |
| #424 | Project cache fails closed on fake `deterministic_vector_blob` embeddings (commit `5330629e`) |
| #426 | FSV/MCP deterministic harness can no longer pass without real compiler semantics |
| #437 | Project ingest wired to active embedder forward providers (commit `a0849ac3`) |
| #438 | Slot-preserving CUDA compiler must load verified trained checkpoint (commit `f982f29d`) |
| #441 | Training loop owns and optimizes real `MeJepaPredictor` weights; exports via manifest-verified pipeline (commit `db83923a`) |
| #452 | Native active-embedder constellation adapter preserves per-embedder ragged spaces (commit `d068d2ed`) |

**⚠ 2026-05-23 Sherlock correction.** The #441 closure asserted "training loop owns and optimizes real `MeJepaPredictor` weights" — verification at `mejepa-train/src/trainer/mod.rs:31-34, 253-261, 409-422` shows the public trainer path is permanently in `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE` and emits `predictor_parameter_update_count=0` in every trainer certificate. Direct aiwonder readback shows the latest ship-gate FSV loaded an out-of-band checkpoint at `/zfs/archive/contextgraph/models/task-py-g-127-trained-checkpoint-export-fsv/best.manifest.json` with `training_mode=full_finetune_predictor_oracle_supervised`, `optimizer_steps=64`, and `weight_delta_observed=true`; the accompanying `training_certificate.json` lacks the trainer contract fields `trained_predictor`, `predictor_parameter_update_count`, `checkpoint_readback_verified`, and `ship_gate_countable_training`. Tracked as **#683**.

**Code guards in production** (`crates/context-graph-mejepa/src/predictor_checkpoint.rs`):

- Lines 147-158: reject any checkpoint containing `PREDICTOR_CHECKPOINT_STUB_MAGIC = b"MEJEPA_BEST_STUB_V1"` at export
- Lines 239-250: reject at load
- `validate_aiwonder_checkpoint_path` rejects non-aiwonder roots
- `trained_weight_sha256 != initial_weight_sha256` invariant on every manifest
- ⚠ The invariant proves a loaded checkpoint changed weights; it does not prove the public Rust `Trainer` produced countable ship-gate training.

**P0 blocker for further gate work:** training lineage/checkpoint loading is not unified or fail-closed (#683) and 4/12 embedder slots are dead (#670/#704). Real-input rebaseline exists but rides Goodhart-magnifier metric (#695). Latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`: raw holdout `prediction_oracle_correlation = 0.4843192995`, rank-safe `0.6916886568`, rank-lift `0.7942190170`, Brier `0.0923729688`, ECE `0.0240316074`.

**#392 active/abstain diagnostic** is valid and promising: `205/240` active rows, corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. Not ship-gate-countable until per-cell active contracts, rolling windows, statistical power, Panel B pass. Previous `0.866667` ship-gate measurement remains invalidated. `compile_error` now uses first-class invalid-source panels instead of carrier workaround; rank-reversing calibration transforms blocked unless calibration support + raw Pearson justify them; rank-lift excludes target-side oracle/docker/test labels. See `01 §1.7` and `08 §0.5`.

---

## 0. The Predictor Contract — binary + labeled + mistake-driven, no ambiguous heads

Three binary predicates with free binary form against reality, plus one explanation surface:

| # | Predicate | Reality that renders verdict |
|---|---|---|
| Q1 | `claim_exists(candidate_patch) → { Yes, No }` | Bytes/AST on disk after patch |
| Q2 | `oracle_passes(candidate_patch) → { Pass, Fail }` **(product output)** | Docker oracle (`swebench.harness.run_evaluation`) |
| Q5 | `predicted_shift_event_occurred(event_id, patch) → { Yes, No }` per leaf | Durable shift-log JSONL on aiwonder |

Q3 is **explanation only**: on Fail, attaches named failure mode + closest exemplar. Does NOT produce its own prediction subject to oracle correlation.

Q4 (perf, accuracy, cost, reasoning class, latent bugs) is **out of the prediction contract**. Ambiguous heads require arbitrary thresholds on continuous heuristics or ask taste judgments with no ground truth. Predicting heuristics introduces noise the mistake loop cannot clean up.

The architecture below serves those three binary predicates. The 5-question `RealityPrediction` schema stays in `types.rs` for backward-compat, but only `Verdict::{Pass, Fail}` (Q2), `claim_reconciliation` (Q1), per-leaf-event `predicted_shift_outcomes` (Q5) determine product behavior.

**Three architectural primitives back the binary contract:**

1. **Labeling layer (§4.3).** Raw slot violations + cross-embedder correlations → 4-level hierarchy of named failure modes. Grounded by closest historical exemplar.
2. **Online mistake loop (§9.6).** Disagreement with reality (oracle / PostToolUse / shift-log replay) materializes as `CF_MEJEPA_MISTAKE_LOG` row; trainer consumes through exact-panel online-head state.
3. **Cross-panel triangulation (§4.4).** Panel B (non-overlapping instruments) re-scores every gate-relevant patch. 0.95 on Panel A counts only when Panel B agrees within 0.05.

**No best-of-K at product surface.** Internal Abstain / OOD / GuardRejected are present-tense failures to be eliminated, not product hedges. Path out is the mistake loop.

**The ambiguity boundary (locked-in rule).** ME-JEPA only predicts questions with free binary form against reality. "Is this performant?" has no free binary form.

---

## 1. End-to-End Data Flow

```
[Operator / AI agent Edit + tool calls + test outcomes]
                 │
                 ▼
       Claude Code hooks (PostToolUse Bash + Stop + UserPromptSubmit + observe ticker)
                 │  JSONL
                 ▼
   /zfs/archive/contextgraph/runtime/shift-log.jsonl
                 │  (tail)
                 ▼
      context-graph-mejepa-shift-subscriber
        runner.rs → shift_transform.rs
                 │
                 ▼
     21 embedders → 12 active content slots (E1, E6, E7, E8, E10, E12, E13, E14 frozen)
                 │
                 ▼
      15 frozen instruments → array of 15 per-embedder vectors (teleological constellation)
        E_AST 384 | E_CFG 256 | E_DataFlow 256 | E_TypeGraph 256 |
        E_Test 384 | E_Trace 512 | E_Diff 256 | E_Witness 256 |
        E_Oracle 128 | E_Problem 1024 | E_CommitMsg 384 |
        E_StaticAnalysis 256 | E_Runtime 256 | E_Reasoning 384 | Scalars 128
        (each slot in own embedder's space; array IS the constellation,
         NOT a 5,120-d fused embedding; 5,120 is byte-layout only)
                 │  PanelBuilder validates: dim + finite + no double-fill
                 ▼
      CF_MEJEPA_PANELS + CF_MEJEPA_PANEL_META + CF_MEJEPA_SHIFT_WATERMARK
                 │
                 ▼
      Live label compiler (#413) -> live-input-safe slot/pair/group labels per chunk
      Runtime ability resolver (#418) -> AbilityContext from chunk-skill/higher-ability memberships
      -> PredictionLabelContext with all active memberships preserved
                 │
                 ▼
      MeJepaPredictor.forward(panel, PredictionLabelContext)
        + per-slot input encoders + late fusion + per-slot output decoders
        + split-conformal interval + OOD gate + TCT Gτ guard
        + cold-cell metric (Abstain if n_supporting<50)
        + multi-head contradiction gate
        + constellation intelligence heads
        + matched fingerprint + closest exemplars HNSW
        + claim graph reconciliation
                 │
                 ▼
      RealityPrediction (Q1 claim_graph + Q2 verdict + Q3 fail modes +
                         Q5 reality_impact + label/skill context + constellation_intelligence)
                 │
                 ▼
      CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_HIERARCHICAL_PREDICTIONS
                 │
       ┌─────────┴───────────┐
       ▼                     ▼
   AI agent             Verification capture (PostToolUse Bash matcher
   (mejepa_predict_     → CF_MEJEPA_PREDICTION_VERIFICATIONS)
    latest / project_report)
                              │
                              ▼  Confirmed / Refuted
                       Active learning queue + replay buffer
                       (CF_MEJEPA_ACTIVE_LEARNING_QUEUE + CF_MEJEPA_REPLAY_BUFFER)
                              │
                              ▼  Mistake loop (#406)
                       CF_MEJEPA_MISTAKE_LOG + CF_MEJEPA_ONLINE_HEAD_STATE
                              │
                              ▼
                       Trainer (UTML L_step = ΔP·ΔK·ΔΩ·ΔΞ)
                       + EWC++ Fisher + Continual Backprop + Inverse-map head
                              │
                              ▼
                       CF_MEJEPA_TRAIN_CERTS (incl. CPΦ bits)
                              │
                              ▼
                       Heal scheduler (drift, per-cell convergence, promote / rollback)
                              │
                              ▼
                       CF_MEJEPA_MODEL_PROMOTIONS + CF_MEJEPA_ACTIVE_POINTERS
```

Pipeline is a consequence predictor, not an intention engine. It learns from observable reality-chain events: `explicit task text / agent claim → edit event → AST delta → per-embedder packed outputs → teleological constellation → frozen instruments → oracle / test / static / runtime / witness outcomes → prediction verification → replay / reward signal`.

---

## 2. Embedder Registry (21 slots; E5 retired)

`crates/context-graph-mejepa-embedders/src/lib.rs`. Production loading requires schema-versioned `models_config.toml`, SHA-256 manifest pins, runtime CUDA, E17 calibration evidence.

| Slot | Family | Dim | Frozen | Active | Source |
|---|---|---|---|---|---|
| E1 | e5-large-v2 | 1024 | ✅ | ✅ | Semantic (default content embedder) |
| E2 | temporal (recency PE) | 512 | tunable | weight-gated | Position-encoded recency |
| E3 | temporal (periodic) | 512 | tunable | weight-gated | Periodic-pattern |
| E4 | temporal (V_ordering) | 512 | tunable | weight-gated | Session-sequence position |
| E5 | causal (asymmetric) | 768 | — | **retired** | Tools fail closed |
| E6 | sparse keyword | 30K vocab | ✅ | ✅ | Keyword inverted index |
| E7 | Qodo-Embed code | 1536 | ✅ | ✅ | Code AST/semantic |
| E8 | graph | 1024 | ✅ | ✅ | Graph-aware |
| E9 | HDC | 1024 | tunable | ✅ | Typo-robust hyperdimensional |
| E10 | paraphrase | 768 (dual) | ✅ | ✅ | Paraphrase asymmetric |
| E11 | entity (KEPLER/TransE) | 768 | — | **disabled** | Replaced by TransE-on-corpus |
| E12 | ColBERT late-interaction | 128 per token | ✅ | ✅ | Per-token MaxSim |
| E13 | SPLADE sparse | 30K vocab | ✅ | ✅ | Sparse expansion |
| E14 | (frozen-by-design slot) | per config | ✅ | ✅ | Reserved |
| E17 | (calibration certificate) | — | — | gated | `agent_state_score` |
| E18-E21 | dynamic-registration | per registration | gated | gated | Dynamic embedders |

**Dynamic embedders** (`dynamic_registry.rs`): hot-swap registry with VRAM budget + falsification gate + freeze + adversarial-injection mitigation.

**Foundationality:** PageRank on chunk/library/embedder dependency graphs → `CF_MEJEPA_*_FOUNDATIONALITY`. Weights `BatchSampler` and Fisher.

---

## 3. Instrument Panel — Array of Per-Embedder Vectors

`crates/context-graph-mejepa-instruments/src/lib.rs:158-199`. Panel is **array of 15 per-embedder/per-instrument vectors**, one per slot, each in its own embedder's space. The array IS the constellation, not a fused embedding.

**The locked-in rule:**

1. Each slot's vector lives in its own embedder's space.
2. Same-embedder cosines valid (`gtau.rs:14` iterates per embedder).
3. **Cross-embedder dim compare forbidden.** `cosine_similarity` at `gtau.rs:90` fails closed with `MEJEPA_TCT_DIM_MISMATCH`.
4. Whole arrays can be compared array-vs-array (per-slot cosine map, strict-conjunction gate, per-pair matrix with diagonal cosines).
5. Cross-slot relationships are **scalar pairwise metrics** (MI, correlation), not vector dim compares.
6. **`PANEL_DIM = 5_120` is byte-layout only.** Forbidden: single linear/conv/attention over packed buffer; flat residual; any operation where strong slot compensates for weak.

**Slot inventory:**

| # | Slot | Variant | Dim | Offset | Encodes |
|---|---|---|---|---|---|
| 1 | `EAst` | `e_ast` | 384 | 0 | Tree-sitter AST |
| 2 | `ECfg` | `e_cfg` | 256 | 384 | Control-flow graph |
| 3 | `EDataFlow` | `e_data_flow` | 256 | 640 | Def-use graph |
| 4 | `ETypeGraph` | `e_type_graph` | 256 | 896 | Type signature + inferred-type graph |
| 5 | `ETest` | `e_test` | 384 | 1152 | Test descriptions |
| 6 | `ETrace` | `e_trace` | 512 | 1536 | Execution trace |
| 7 | `EDiff` | `e_diff` | 256 | 2048 | AST-diff edge encoder |
| 8 | `EWitness` | `e_witness` | 256 | 2304 | Witness-chain sequence |
| 9 | `EOracle` | `e_oracle` | 128 | 2560 | Test outcomes + 10-class exception one-hot |
| 10 | `EProblem` | `e_problem` | 1024 | 2688 | SWE-bench problem text |
| 11 | `ECommitMsg` | `e_commit_msg` | 384 | 3712 | Candidate fix commit message |
| 12 | `EStaticAnalysis` | `e_static_analysis` | 256 | 4096 | Linter / typecheck / complexity |
| 13 | `ERuntime` | `e_runtime` | 256 | 4352 | Wall-clock / RSS / coverage / network |
| 14 | `EReasoning` | `e_reasoning` | 384 | 4608 | Reasoning / reflexion trace |
| 15 | `Scalars` | `scalars` | 128 | 4992 | BFS depth, blame age, churn, coverage Δ, repo-health |

Total contiguous byte layout: 5,120 f32. Semantic unit is the array of 15 per-embedder vectors.

**Panel invariants** (`PanelBuilder::set_slot`, `validate_panel_parts`):
- `vector.len() == slot.dim()` else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Every element finite else `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`
- Slot not already filled else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Build rejects `filled_mask == 0`
- All persistence through `try_from PanelSerde` so JSON round-trip re-validates

**Structural collapse-immunity invariant.** Instruments carry zero trainable params. `FrozenGuard` / `FrozenSnapshot` / `hash_f32s` enforce `ΔΞ.target_collapse = 0`. `grad_hook.rs::run_gradient_hook` checks `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.

**Anti-compensation — formal ECC (planned, #512).** Replaces heuristic FSV with self-dual doubly-even ECC over 15-slot occupancy bitmap (Gates et al. 2008 lineage). Recovery radius `t = 2`: corrupt up to 2 slots → guaranteed reconstruction; more → `PANEL_ECC_UNCORRECTABLE` typed reason into mistake log.

---

## 4. Predictor Architecture

`crates/context-graph-mejepa/src/predictor/` — `MeJepaPredictor`, `TransformerLayer`, `ArchitectureSummary`. CUDA-required.

**Architectural rule:** predictor consumes panel **as array of per-embedder vectors**, NOT as flat 5,120-d input. Per-slot input encoders → late fusion → per-slot output decoders. No `linear(PANEL_DIM, hidden)` projections — architecturally forbidden.

Default architecture:
- 15 per-slot input encoders, each `linear(slot.dim(), per_slot_latent_dim)`
- Late-fusion stack: typed concatenation of 15 latents → transformer layers operating on slot-preserving sequence (slot index = positional feature so transformer cannot treat dims as exchangeable)
- 15 per-slot output decoders emit per-slot residuals against frozen target
- VICReg + Huber loss per-slot, aggregated via fail-closed conjunction
- `INVERSE_ACTION_DIM = 16` action latent for inverse map

Inference config (`MeJepaInferConfig`):
- `INFER_DEFAULT_ALPHA` — conformal α
- `INFER_DEFAULT_P_THRESHOLD`, `INFER_DEFAULT_OOD_REFUSE_THRESHOLD`
- `INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT = 50` — cell starves → Abstain
- `INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS`, `INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS`

### 4.1 Heads (per Q-question)

| Head | Q | Output | Source |
|---|---|---|---|
| `OracleHead` | Q2 | `OracleLogits` over Pass/Fail/Skip/Error | `oracle_head.rs` |
| `claim_graph.rs` | Q1 | `ClaimGraph` + reconciliation against bytes/AST | `claim_graph.rs` |
| `patch_similarity.rs` | Q3 | `closest_exemplars` from usearch HNSW | `patch_similarity.rs` |
| `failure_fingerprint.rs` | Q3 | Matched fingerprint, predicted failure modes | `failure_fingerprint.rs` |
| `reality_impact.rs` | Q5 | `RealityImpact` from shift-log replay | `reality_impact.rs` |
| `hierarchical.rs` | spans Q1-Q5 | File/function/AST/chunk hierarchy | `hierarchical.rs` |

### 4.2 Verdict assembly

`crates/context-graph-mejepa/src/verdict_assembly.rs` stitches heads:

```rust
enum Verdict { Pass, Fail, OutOfDistribution, Abstain, GuardRejected }
```

**Product surface:** only `Pass | Fail` ships to MCP caller. Internal `OutOfDistribution`/`Abstain`/`GuardRejected` are markers indicating predictor lacks confidence on `(cell × named_failure_mode)`. MUST be projected to `Pass | Fail` at boundary (closest-exemplar majority outcome). Projected verdict subject to ground-truth verification + mistake-loop update.

**Decision flow (`compiler.rs::compile`):**

1. Materialize `Panel` for candidate change.
2. Forward through `MeJepaPredictor`.
3. Per-head DDA features (`head_projection.rs`).
4. Split-conformal interval (`conformal.rs`).
5. Per-slot OOD reasons (#389): `RealityPrediction.per_slot_ood_reasons` from `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`. Persists slot/embedder/pair-specific OOD reasons.
6. Aggregate OOD gate. Strict-conjunction policy: aggregate score never overrides slot violation.
7. TCT Gτ guard.
8. Cold-cell metric: `n_supporting < 50` → `Abstain`.
9. Contradiction thresholds (#263): high Q2/pass + high Q3 explanation/TCT consequence-pressure → `Abstain` with `MULTI_HEAD_CONTRADICTION`. Missing thresholds → `CONTRADICTION_THRESHOLD_MISSING`. Q4 thresholds frozen display-only.
10. Constellation intelligence (#390): slot-pair consensus, contradiction, redundancy, novelty, blind-spot, consequence pressure.
11. Match fingerprint catalog.
12. Pull closest exemplars (`CF_MEJEPA_PATCH_SIMILARITY_META`).
13. Reconcile claim graph (`CF_MEJEPA_CLAIM_RECONCILIATION`).
14. Emit `RealityPrediction` → write `CF_MEJEPA_LIVE_PREDICTIONS` + `CF_MEJEPA_HIERARCHICAL_PREDICTIONS`.

**Anti-compensation guard.** Corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, temporal jumps cannot be accepted through aggregate evidence even when flat normalized score would pass. Code scan prevents flat-vector regressions.

---

### 4.3 Failure-mode labeling hierarchy (the meaning layer)

4-level hierarchy turns raw numerical evidence into named rules. Level 0 (deterministic compiler) and Level 1/2/3 (hierarchical naming) shipped.

**Shipped substrate:**

| Layer | Source | Output |
|---|---|---|
| Level 0 deterministic compiler (#413) | `scripts/swebench_lite_ops/python_auto_label_discovery.py` (1,641 lines, no inner LLM) | 399,777 chunk labels, 491-row accepted registry, 3,000 code-state constellations under `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` |
| Level 1/2/3 hierarchy (#404) | `crates/context-graph-mejepa-train/src/failure_mode_hierarchy.rs`, `src/skill_corpus_materialization.rs` | 8 Level-1, 157 Level-2, 379 Level-3, 536 named predictions |
| Skill substrate (#414) | `src/skill_sequence_discovery.rs`, `src/chunk_skill_membership.rs` | 128 Level-2 skills, 36,773 memberships, 10,579 reverse rows |
| Runtime resolver (#418) | `src/ability_resolver_*.rs` | `AbilityContext`, ordered higher-ability IDs into `PredictionLabelContext` |

**Holdout proof (#404):** 114/114 actual Fail rows have FailLikely named-mode coverage; closest-exemplar oracle agreement is 1.0 over 113/114. Post-fix active-prediction view: 117/117 active predicted true-Fail rows resolve to FailLikely; aggregate predicted-Fail coverage is 137/150 — 13 false-positive Fail verdicts have no named consequence explanation. Predictor policy/feature-selection blocker.

**Leakage policy.** Target-side labels (oracle/Docker/test outcomes) marked `leakage_policy:target_side_only`. Supervision/reward inputs only — **never** live predictor inputs.

**Failure-mode vocabulary** (seeded, not invented):

| Source | Categories |
|---|---|
| Aleithan et al. 2025 "Are Solved Issues Really Solved" | `solution_leak`, `incorrect_fix_weak_test`, `different_files_changed`, `incomplete_fix` |
| Yang et al. 2024 "Empirical Study on Failures" | 9 categories × 25 subcategories across Location / Repair / Iteration |
| SWE-Bench Pro 2025 (Scale AI) | `wrong_solution`, `tool_use_error`, `missed_edge_case`, `context_overflow_from_listing` |
| Our 30-case adversarial corpus | 30 adversarial fingerprints |
| Our 8-mutation operator set | `subtle_flip_invariant_violation`, `off_by_one_boundary_miss`, `swap_variable_aliasing`, etc. |

Persisted in `CF_MEJEPA_FAILURE_MODE_VOCABULARY`.

**Prediction integration.** Every live prediction carries `PredictionLabelContext.active_skill_ids`. Every `Verdict == Fail` row carries:

```rust
pub struct NamedFailureMode {
    pub level1_group: Option<FailureModeGroupId>,
    pub level2_skill: Option<FailureModeSkillName>,
    pub level3_constellation: Option<FailureModeConstellationName>,
    pub confidence: f32,
    pub closest_exemplars: Vec<ExemplarPointer>,
    pub vocabulary_source: VocabularySeedSource,
}
```

**Bidirectional skill linkage (#417 shipped read surfaces, #557 audit surface).** Read-only MCP surfaces: `mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_skill_impact`, `mejepa_skill_graph_inspect`, `mejepa_skill_conflict_graph`, `mejepa_skill_browse`. #557 adds usefulness audit (candidate kind, support/fail-pass/lift, genericity, per-cell contribution, live-safe vs target supervision labels, source-byte sequences, lifecycle, uncovered code-state keys, unknown ordered constellations, rejected/demoted). Remaining #417: cross-repo transfer + pathway-leaf attachment.

---

### 4.4 Cross-panel triangulation (the Goodhart guard)

Panel A is both training substrate AND centroid set Gτ guard evaluates against. Encoder-matched → 0.95 on Panel A could mean predictor learned the panel.

**Panel B instrument map (non-overlapping):**

| Slot | Panel A | Panel B |
|---|---|---|
| AST | tree-sitter → 384-d | CodeBERT → 384-d |
| CFG | attention encoder → 256-d | Pythia program-graph → 256-d |
| Data-flow | def-use attention → 256-d | joern → 256-d |
| Type-graph | tree-sitter type → 256-d | pytype JSON → 256-d |
| Code-semantic (E7) | Qodo-Embed-1 → 1536-d | Voyage-code-2 → 1536-d |
| Text (Test/Problem/CommitMsg) | e5-large-v2 | bge-large-en-v1.5 |
| Trace/Witness/Oracle/StaticAnalysis/Runtime/Reasoning/Scalars | unchanged | unchanged (deterministic) |

Only learned encoder slots swap; deterministic slots shared (encode reality directly, not learned representations).

**Triangulation procedure:**

1. Every gate-relevant prediction scored on Panel A AND Panel B.
2. Both go to `CF_MEJEPA_CROSS_PANEL_AGREEMENT` keyed by `(prediction_id, panel_pair_id)`.
3. **Agreement metric:** `cross_panel_oracle_correlation = corr(verdict_a, oracle) AND corr(verdict_b, oracle)` — both must clear 0.95.
4. **Drift metric:** `|corr_a - corr_b| ≤ 0.05` for ship eligibility.
5. **Fail-closed:** Panel A ≥ 0.95 AND Panel B < 0.85 → `CROSS_PANEL_GOODHART_DETECTED`.

**Cost.** Panel B requires loading 5 new encoders. VRAM ~12 GB during evaluation only; never resident during inference. Panel B worker reads `CF_MEJEPA_LIVE_PREDICTIONS` periodically and writes `CF_MEJEPA_PANEL_B_OBSERVATIONS`.

Tracked by **#405 (open p0, blocked)**.

---

## 5. TCT Constellations

`crates/context-graph-mejepa-tct/src/lib.rs`. Fail-closed: missing centroids, missing thresholds, under-sampled cells, stale constellations, provenance drift, non-finite floats are all errors. No permissive thresholds.

A teleological constellation is the exact-state, slot-preserving observation of one reality object across all active embedders/instruments + downstream consequence labels. Each embedder output remains in its own vector space. Math happens over the collection: per-embedder centroids/variance, pairwise correlation/MI, redundancy, disagreement, temporal movement, attribution, calibrated uncertainty.

| Module | Purpose |
|---|---|
| `builder.rs` | Build constellation cells from training data |
| `calibrator.rs` | Per-cell calibration |
| `constellation.rs` | Centroid + threshold container |
| `freshness.rs` + `freshness_policy.rs` | Per-cell freshness checks |
| `gtau.rs` | Gτ guard predicate |
| `guard_audit.rs` | Per-Python-cell Gτ audit over cross-embedder tensors + per-slot calibration |
| `hinge.rs` + `hinge_types.rs` | Decision hinges |
| `inspect.rs` | MCP `constellation_inspect` surface |
| `shrinkage.rs` + `shrinkage_engine.rs` | Shrinkage for small-sample cells |
| `store.rs` | RocksDB CRUD on `CF_MEJEPA_CONSTELLATION` |
| `verdict.rs` | TCT-side verdict overlay |

Cell key = `(mutation_category, language, …)`. Refresh log: `CF_MEJEPA_CONSTELLATION_REFRESH_LOG`. Weekly cadence via operator triage.

---

## 6. Corpus Generator

`crates/context-graph-mejepa-corpus/src/lib.rs`. Inspired by mutation-testing (Mothra, MuJava, mutmut); no upstream code copied.

### 6.1 8 mutation categories

| # | Variant | Slug | Description | Expected oracle |
|---|---|---|---|---|
| 1 | `KnownGood` | `known_good` | Echo input unchanged | PASS |
| 2 | `SubtleFlip` | `subtle_flip` | Invert one boolean operator | FAIL |
| 3 | `OffByOne` | `off_by_one` | Add ±1 to one numeric literal | FAIL |
| 4 | `SwapVariable` | `swap_variable` | Rename one local to another in-scope name | FAIL |
| 5 | `DeleteTestCall` | `delete_test_call` | Delete one assertion call | mixed |
| 6 | `WrongFile` | `wrong_file` | Apply patch text to DIFFERENT base file | FAIL on test discovery / runtime |
| 7 | `OverEngineer` | `over_engineer` | Append unused helper function | PASS (correct but messy) |
| 8 | `CompileError` | `compile_error` | Introduce Python syntax error | FAIL at parse |

### 6.2 Languages

`Language::all() = [Rust, Python, JavaScript, TypeScript, Go, Java, C, Cpp, CSharp, Ruby, Php]`. Python uses parser-backed operators; others use `generic_language_ops`. **Current ship gate is Python only.** Non-Python deferred.

### 6.3 SWE-bench bridge

`swebench.rs::run_swebench_lite_oracle()`:
- Invokes `python -m swebench.harness.run_evaluation` per-instance.
- Parses per-instance `report.json`.
- Converts `FAIL_TO_PASS` / `PASS_TO_PASS` / `FAIL_TO_FAIL` / `PASS_TO_FAIL` → `OracleVerdict`.
- Docker-name-safe `run_id` validator.

Lite dataset: 300 test + 23 dev instances across 12 repos (django=114, sympy=77, matplotlib=23, sklearn=23, pytest=17, sphinx=16, astropy=6, requests=6, +4 more). HF cache: `/zfs/hot/contextgraph/.cache/huggingface/hub/datasets--princeton-nlp--SWE-bench_Lite/`.

### 6.4 Split planner

`split.rs::SplitMode`:
- `InstanceAtomic` (default) — clean 240/30/30 on Lite (80/10/10)
- `RepoAtomic` — structurally bounded to 195/77/28

SplitMix64 shuffle + LPT pre-sort + lowest-current/target-ratio bin-pack.

### 6.5 Adversarial + synthetic stress corpora

- 30-case adversarial Python corpus → `CF_MEJEPA_ADVERSARIAL_CORPUS`
- 30 known-answer synthetic cases → `CF_MEJEPA_SYNTHETIC_STRESS_RESULTS`
- End-to-end reality-compiler stress → `CF_MEJEPA_STRESS_RUNS`

---

## 7. Reality Compiler (per-project ingest)

`crates/context-graph-mejepa/src/project_ingest.rs` (~52K LOC), `project_cache.rs` (~46K, Merkle-tree cache), `project_report.rs` (~44K).

| Capability | Source |
|---|---|
| Merkle-tree per-project chunk cache; embedding writes fail closed unless real active embedder outputs supplied | `project_cache.rs` |
| 250 ms debounced watcher-status scan | `project_cache.rs` |
| Transitive invalidation + schema-version-aware chunk refresh | `project_cache.rs` |
| `mejepa_project_ingest` MCP tool | `project_ingest.rs` |
| `mejepa_project_report` MCP tool | `project_report.rs` |

Production output paths (aiwonder): `/zfs/archive/contextgraph/projects/<project_id>/{manifest.json, cache/, predictions/, report/{report.json, report.md}}` + `CF_MEJEPA_PROJECT_REPORTS`.

Performance gate: fresh ingest of 50-file Python project < 60 s on RTX 5090; re-ingest after 1-file edit < 5 s.

---

## 8. Live Session Trace

`crates/context-graph-mejepa/src/live_session_trace.rs`. End-to-end Claude Code session integration. Writes `CF_MEJEPA_LIVE_SESSION_TRACES` keyed by `(session_hex32, ts_ns)`. Tooling: `mejepa observe`.

---

## 9. Training (UTML v1.0)

`crates/context-graph-mejepa-train/src/learning_signal.rs`. UTML `L_step = ΔP × ΔK × ΔΩ × ΔΞ`:

| Term | What it measures |
|---|---|
| `ΔP` | Predictor improvement |
| `ΔK` | Catastrophic-forgetting violation (Fisher diagonal) |
| `ΔΩ` | Plasticity (continual backprop) |
| `ΔΞ` | Structural-collapse safety margin (frozen-target invariant) |

### 9.1 Replay buffer (M(t) consolidation)

`replay_buffer.rs`. `BatchSampler::next_replay_batch` reads from `CF_MEJEPA_REPLAY_BUFFER` priority-sorted by surprise + per-cell coverage gap. Mistake-loop rows get 8× weight boost.

### 9.2 Inverse-map head (bidirectional training)

`loss/inverse.rs` — `inverse_map_loss`, `compose_bidirectional_l_full`. Shared-trunk target-panel → input-panel + 16-d action latent.

**Ablation result:** original `λ_inv ∈ {0, 0.25, 0.5, 1.0}` fixed-point result (`0.8666666746139526` for every variant) is invalid synthetic-input evidence. Real-input rerun at `fsv/task-ek-012-bidirectional-ablation-realinput-fsv/run-20260520T172657Z/bidirectional_ablation_realinput_fsv.json` kept `lambda_inv=0.0` as best (`0.6704055071` rank-safe); positive lambdas regressed.

### 9.3 EWC++ + Continual Backprop

- `aux/` — Fisher snapshots → `CF_MEJEPA_FISHER_SNAPSHOTS` (EWC++ catastrophic-forgetting protection)
- `aux/` — dormancy histories → `CF_MEJEPA_PLASTICITY_HISTORY` (continual backprop, Dohare 2024)

### 9.4 Compression progress (CPΦ)

`compression_progress.rs`. `TrainingCertificate.conditional_description_length_bits` over rolling window. Renders into weekly markdown. Exposed by `mejepa_compression_progress`.

### 9.5 Per-fingerprint catalog stability

`CF_MEJEPA_FINGERPRINT_FISHER` + `CF_MEJEPA_FINGERPRINT_DORMANCY`. Per-fingerprint EWC + CBP.

### 9.5bis Information-entropy auxiliary loss (#511, planned)

`crates/context-graph-mejepa-train/src/loss/entropy.rs`. Adds `L_entropy = λ · H(z_latent)` to `L_step` where `H` is differential entropy of slot-preserving predictor's late-fused latent (Kraskov k-NN estimator or variational lower bound). Formalizes Vopson 2nd-law-of-infodynamics (AIP Advances 12, 075310, 2022). λ starts at 1e-4, auto-calibrated against holdout Brier. **On the road to 0.95** — worst case = no movement, best case = 1-2 correlation points.

### 9.6 Online mistake-driven adaptation — SHIPPED (#406 / #523)

Source files: `crates/context-graph-mejepa-train/src/mistake_log.rs`, `replay_buffer.rs`, `online_head_state.rs` + 2 keys/support files, `ability_resolver_refutation.rs`. MCP wrapper: `crates/context-graph-mcp/src/handlers/tools/mejepa_mistake_loop_tools.rs`.

Storage CFs: `CF_MEJEPA_MISTAKE_LOG`, `CF_MEJEPA_REPLAY_BUFFER`, `CF_MEJEPA_ONLINE_HEAD_STATE`, skill lifecycle audit CFs from #414/#418.

**The shipped primitive.** Current online head is key-local exact-panel state over `panel_signature_hash`. It corrects the repeated same-panel mistake, records label/skill/ability/membership context, measures unrelated control panel before/after to prove update did not perturb unrelated state. Does NOT flatten slots, does NOT update trainable vector head, does NOT claim Fisher/EWC.

```
if predicted_verdict != ground_truth_verdict:
    record_mistake_in_CF_MEJEPA_MISTAKE_LOG with full context
    enqueue label/skill-aware row in CF_MEJEPA_REPLAY_BUFFER (8× boost)
    online_head[panel_signature_hash].correction_bias += eta * sign(ground_truth_verdict)
    online_head[panel_signature_hash].corrected_verdict = sign(correction_bias)
    repeat_metric[cell,label,skill,ability,membership] = byte-readable rolling metric
    protection_mode = exactPanelKeyIsolation
    claims_fisher_ewc_protection = false
    assert unrelated_control_delta <= 0.02
```

**Ground-truth sources:**

| Source | When | CF |
|---|---|---|
| SWE-bench Docker oracle | Corpus training + heldout eval | corpus RocksDB `mejepa_oracle_verdicts` JSON source rows |
| PostToolUse Bash capture | Live agent session | `CF_MEJEPA_PREDICTION_VERIFICATIONS` |
| Shift-log replay (Q5) | After patch lives in a session | `CF_MEJEPA_REALITY_IMPACT` |
| Operator override | Operator labels a row | `CF_MEJEPA_OPERATOR_OVERRIDES` |

Each ground-truth event materializes `MistakeRow` with full context (prediction_id, ts_ns, cell_key, predicted/ground-truth verdict + named failure mode, source panel, panel signature hash, per-slot OOD reasons, accepted label IDs, ordered active skill IDs, skill_signature_hash, skill_lifecycle_decision, closest_exemplar_at_predict, ground_truth_source).

**Convergence target:** `mistake_repeat_rate = |second_mistake_on_same_cell+pattern| / |first_mistake| ≤ 0.05` over rolling window. Byte-readable from `CF_MEJEPA_MISTAKE_LOG` aggregations + `mejepa_mistake_loop_status`.

**Why this beats best-of-K.** "If it needs 4 guesses to be most correct then it is already wrong." Best-of-K papers over an under-trained predictor by spending compute at inference time. Online perceptron pays the cost at *learning* time and converges to a predictor right on first try.

**#524 guard.** Any future Fisher/EWC claim requires a durable Fisher/EWC row; reports without one fail FSV.

---

## 10. Heal Scheduler

`crates/context-graph-mejepa/src/heal/` (5 modules). Reads drift signals, decides:
- **Retrain** — append new training step
- **Promote** — flip `CF_MEJEPA_ACTIVE_POINTERS` (operator-approval-gated for first promotion + catastrophic class)
- **Rollback** — revert active pointer
- **Pause** — `mejepa_pause_predictions`
- **Distill** — distill from champion to smaller learner

Per-cell convergence driver tracks per-cell ship-gate status. Mixed oracle cells use `prediction_oracle_correlation`; zero-variance all-pass/all-fail cells use #532 replacement contract (target-class recall + actual-class confidence + Brier/ECE) because Pearson is undefined there. Driver uses those typed metrics for targeted mutation expansion, conformal recalibration, holdout rotation, four-window ship-gate stability, per-cell exemptions.

MCP: `mejepa_heal_status`, `mejepa_daemon_status`, `mejepa_rollback_to`, `mejepa_promote_approval`.

**Cold-load behavioural FSV (planned, #514).** After promotion, sealed cold-load run on 30-instance random holdout must reproduce promotion-time oracle correlation within 0.02 and per-cell verdict distribution within 5% — with no further training/calibration/replay-buffer access. Catches training-state Goodhart (orthogonal to #405's encoder-matched defense). Promotion-time snapshots persist into `CF_MEJEPA_COLD_LOAD_SNAPSHOTS`.

---

## 11. Shift Subscriber

`crates/context-graph-mejepa-shift-subscriber/src/` (9 modules). Tokio task:
1. `tail.rs::ShiftLogTail` tails `runtime/shift-log.jsonl`.
2. `models.rs` decodes `ShiftEntry`.
3. `shift_transform.rs::shift_to_inference` maps shift → inference input.
4. `subscriber.rs::ShiftSubscriber::run()` materializes `Panel` + writes `CF_MEJEPA_PANELS`.
5. `watermark.rs::WatermarkWriter` persists watermark — the durability gate.
6. `status.rs::LiveSubscriberStatus` surfaced via `mejepa_subscriber_status`.

Fail-closed on `decode_session_hex32` mismatch, malformed shift entries, watermark regression.

---

## 12. Conformal + OOD + Cold-cell

| Module | Purpose |
|---|---|
| `conformal.rs` | Split-conformal predictor; per-head α=`INFER_DEFAULT_ALPHA` |
| `calibration.rs` + `calibration_types.rs` | `CalibrationCertificate`, per-head calibration |
| `ood.rs` | Residual OOD score math; `INFER_DEFAULT_OOD_REFUSE_THRESHOLD` compatibility fallback only |
| `ood_harvest/` + `ood_harvest.rs` | Live-session harvest + serving calibrator: `CF_MEJEPA_OOD_HARVEST`, `CF_MEJEPA_OOD_CALIBRATIONS`, 4× active-learning projection, orphan/degenerate quarantine, expired-row tier-down, operator downweight, flags `OOD_HARVEST_EMPTY`, `OOD_CELL_AUC_REGRESSION`, `OOD_GATE_OVER_FLAGGING`, `OOD_CALIBRATOR_MISSING`, `OOD_SCORE_ABOVE_THRESHOLD` |
| Per-slot OOD reasons | `RealityPrediction.per_slot_ood_reasons` from `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`; `MAX_EMBEDDERS_PER_PANEL` guard |
| `ColdCellMetric` (in `compiler.rs`) | Abstain when `n_supporting < 50`; writes `CF_MEJEPA_COLD_CELL_METRICS` |

---

## 13. Foundationality + Bedrock Consistency

| Surface | Source | CF |
|---|---|---|
| Chunk dependency graph + PageRank | `chunk_foundationality.rs` | `CF_MEJEPA_CHUNK_DEPENDENCY_GRAPH`, `CF_MEJEPA_CHUNK_FOUNDATIONALITY` |
| Library registry / per-library / cross-library | `library_foundationality.rs` | `CF_MEJEPA_LIBRARY_REGISTRY`, `CF_MEJEPA_LIBRARY_FOUNDATIONALITY`, `CF_MEJEPA_CROSS_LIBRARY_REFERENCES` |
| Embedder dependency graph / foundationality | `embedder_foundationality.rs` | `CF_MEJEPA_EMBEDDER_DEPENDENCY_GRAPH`, `CF_MEJEPA_EMBEDDER_FOUNDATIONALITY` |
| MCP bedrock-consistency check | `mejepa_check_bedrock_consistency` | reads foundationality CFs |

PageRank scores weight `BatchSampler` and Fisher.

---

## 14. Q1 Claim Graph

`crates/context-graph-mejepa/src/claim_graph.rs`. Deterministic claim parser. Reconciles claims against bytes-on-disk + AST. Validates `repo_after_sha` reachability. Writes sync/readback rows to `CF_MEJEPA_CLAIM_RECONCILIATION`.

---

## 15. Q3 Patch Similarity / Closest Exemplars

`patch_similarity.rs`. usearch HNSW over L2-normalized `(E_AST, E_CFG, E_DataFlow, E_TypeGraph)` signatures. Metadata in `CF_MEJEPA_PATCH_SIMILARITY_META`. Index storage at `/zfs/archive/contextgraph/storage/patch-similarity-index-v1/`.

Top-1 same-failure-mode accuracy: 1.0.

---

## 16. Q4 Heads — OUT OF SCOPE (frozen as display-only historical data)

**Status: Q4 heads removed from prediction contract.** Schema + CFs (`CF_MEJEPA_Q4_*`) preserved for forensic continuity. `mejepa_explain_prediction` can surface historical rows. But:

- **No new Q4 training** — producers frozen; no new rows after freeze.
- **No Q4 calibration prerequisite for ship** — 0.95 measures Q2 oracle correlation only.
- **No Q4 contribution to loss** — `Q4_LOSS_COEFFICIENT_MAX = 0.0` pinned.
- **No Q4 trust gate consumption** — `q4.trustedConsequences` no longer populated by new predictions.

**Code enforcement.** `Q4_DOCTRINE_FREEZE_ACTIVE = true` in `q4_trust_gate.rs`. Builder + prediction-surface inference clear Q4 display-only fields. `q4_inputs_to_ship_gate()` returns `0`. `mejepa_weekly_eval_dashboard.q4Freeze` section fails closed if frozen Q4 CFs grow.

**Why removed.** Q4 questions are ambiguous (§0). They require arbitrary threshold or ask taste question reality does not render binary verdict on. Predicting them trains system to match heuristic tools (mutmut, semgrep, vulture, hyperfine, pytest-benchmark) rather than reality.

**What replaces Q4 in operator answer.** Operators who used to want "what else could go wrong" answers get them from **named failure mode** Q3 surface: Fail verdict → named cause + closest exemplar. Pass → "Pass + claim landed in bytes (Q1=Yes), oracle agrees (Q2=Pass), closest exemplars all Pass" — not speculative ambiguous risks.

---

## 17. Q5 Reality Impact

`reality_impact.rs`. Shift-log replay engine. Replays predicted side effects against actual shift log; persists `RealityImpactRecord` in `CF_MEJEPA_REALITY_IMPACT`.

Behavior:
- Reads persisted `RealityPrediction` rows from `CF_MEJEPA_LIVE_PREDICTIONS` by `prediction_id`.
- Parses session `cgreality-shift-log/<session>.jsonl` over bounded replay window.
- Classifies as `Confirmed`, `Missed`, or `NotYetObserved`, and unpredicted observed consequences as `Surprise`.
- Fails closed on missing predictions, out-of-order timestamps, explicit shift-log SHA-chain breaks, unverifiable file-shift SHA evidence.
- MCP: `mejepa_reality_impact`.

Q5 calibration consumer writes `CF_MEJEPA_Q5_CALIBRATIONS` from replay records. Cells keyed by `calibration_id`, `language`, `mutation_category`, `side_effect_kind`. Metrics: TP/FP/FN, awaiting-oracle, replay-exclusion, precision/recall/F1, surprise rate. High-surprise and low-F1 cells emit retraining flags.

---

## 17bis. Pathway Exploration Surface — planned (#416)

L3 capability in JEPA world-model rubric — counterfactual rollouts for decision support — instantiated on Q1/Q2/Q5 binary-leaf substrate. See `01 §1.5`.

**Architecture** (planned modules additive to `context-graph-mejepa`):

| Module | Role |
|---|---|
| `src/pathway/tree.rs` | `PathwayNode { leaf_kind: Q1\|Q2\|Q5, leaf_id, predicted_outcome, predicted_prob, conformal_interval, parent_path_prob, children }` |
| `src/pathway/beam_search.rs` | UCB1-style selection; top-K retention by cumulative path probability; pruning `ε ≈ 0.01` |
| `src/pathway/leaf_predictor.rs` | Routes each candidate leaf to Q1/Q2/Q5 predictor head. Non-Q1/Q2/Q5 → `PATHWAY_AMBIGUOUS_LEAF_REJECTED` |
| `src/pathway/exemplar_attach.rs` | Closest historical pathway from #413 491-row registry |
| `src/pathway/skill_attach.rs` | Active skill IDs + source chunk memberships from #414/#417 |
| `src/pathway/store.rs` | RocksDB writers for `CF_MEJEPA_PATHWAY_TREES` + `CF_MEJEPA_SURFACED_PATHWAYS` |

**Algorithm (beam search over binary leaves):**

```
fn surface_pathways(candidate_patch, k=5, beam_width=32, max_depth=6, ε=0.01) {
    let root = PathwayNode::seed(candidate_patch);          // Q1 binary leaf
    let mut frontier = vec![root];
    for depth in 0..max_depth {
        let mut next_frontier = Vec::new();
        for node in &frontier {
            for child in node.expand_binary_children() {    // ONLY Q1/Q2/Q5
                if !child.leaf_kind.is_binary() { return Err(PATHWAY_AMBIGUOUS_LEAF_REJECTED); }
                let predicted = predictor.binary_head_predict(child);
                child.path_prob = node.path_prob * predicted.calibrated_prob;
                if child.path_prob >= ε { next_frontier.push(child); }
            }
        }
        next_frontier.sort_by(|a,b| b.path_prob.cmp(&a.path_prob));
        next_frontier.truncate(beam_width);
        frontier = next_frontier;
        if frontier.is_empty() { break; }
    }
    // Normalize and return top-K
}
```

**Operator-facing semantics:**

```
Candidate patch: <patch_sha256>
Most probable futures (top-5):

[1] P = 0.42  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=Yes → Q5.next_session_admin_save=Yes
                Closest historical: django__django-13230 (PASS)
[2] P = 0.21  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=Yes → Q5.next_session_admin_save=No
                ⚠ named failure mode: downstream_consumer_regression
[3] P = 0.17  →  Q1=Yes → Q2=Fail
                Closest historical: django__django-12286 (FAIL)
                Named: mutation_mechanism:subtle_flip_invariant_violation
[4] P = 0.12  →  Q1=Yes → Q2=Pass → Q5.django_test_isolation=No
                ⚠ named failure mode: mock_only_change
[5] P = 0.08  →  Q1=No
                "patch will not land in bytes as claimed"
```

Operator picks one. System (when #335 ships) reverse-engineers patch-shape constraints that land that pathway.

**Not generative.** Pathways are projections of predictor's existing binary-head knowledge into a tree, not invented futures. Every probability calibrated against disjoint calibration partition (#407). Cold-cell pathways carry explicit `cold_cell_warning`.

**Open question.** Whether ambiguous-leaf pathways are ever admissible (interval-typed? externally-falsifiable thresholds against deterministic tools?) is future work and explicitly out of scope for #416. FSV asserts `ambiguous_leaves_in_pathway == 0` at every release.

---

## 18. Adversarial Defenses

| Defense | Source | Status |
|---|---|---|
| Negative-action ablation falsifier | `examples/phase_g_adversarial_retrain_fsv.rs` + `CF_MEJEPA_ABLATION_REPORTS` | shipped |
| Multi-head contradiction | `contradiction.rs` + `verdict_assembly.rs` + `CF_MEJEPA_CONTRADICTION_THRESHOLDS` | shipped (#263) |
| Constellation disagreement pressure | `constellation_intelligence.rs` + `CF_MEJEPA_CONSTELLATION_PATTERNS` | shipped (#390) |
| OOD gate floor | `ood.rs` + `ood_harvest.rs` | shipped (#129) |
| Conformal interval floor | `conformal.rs` | shipped |
| Adversarial-embedder-injection mitigation | `adversarial_embedder_injection.rs` | shipped |
| Secret redaction (19 patterns, <1ms p99 on 10KB) | `scripts/swebench_lite_ops/python_secret_redaction.py` → `CF_MEJEPA_REDACTION_LOG` | shipped |
| Cross-language label gating | `cross_language_label_gating.rs` | shipped |
| 30-case adversarial Python corpus | `adversarial_corpus.rs` → `CF_MEJEPA_ADVERSARIAL_CORPUS` + fingerprint catalog | shipped |

---

## 18.5. Neuroscience-Inspired Reserve (filed 2026-05-20, DO NOT START until current queue concludes)

8 contingency issues (#538-#545) at `priority:p3`, `status:needs-triage`, with `⚠ DO NOT START — Neuroscience-reserve contingency` markers. Activation: only after current Phase G queue + creativity engine fail to close gap + post-queue gap analysis identifies need.

**Tier 1 — high-impact, addresses gap directly:**

| Issue | Planned modules / pipeline addition |
|---|---|
| **#538** Predictive-coding head (Rao & Ballard 1999; Friston) | `crates/context-graph-mejepa/src/predictor/top_down_prediction.rs` per-slot top-down prediction + residual against observation. `crates/context-graph-mejepa-train/src/loss/predictive_coding.rs` per-slot prediction-error term in `L_step`. Rewires bottom-up-only → bidirectional predict-then-observe-then-residual. Slot identity preserved. |
| **#539** Schema-prioritized replay (Mattar & Daw 2018; Liu et al. 2019) | `crates/context-graph-mejepa-train/src/replay_scheduler.rs` schema-grouped batch sampler over #414 skill IDs. `reverse_replay.rs` for pathway credit assignment (#416 dep). `generative_replay.rs` for idle-time rehearsal. |
| **#540** Hierarchical predictive coding on #404 hierarchy | `crates/context-graph-mejepa/src/hierarchical_prediction.rs` Level-3 → 2 → 1 → 0 backward prediction; per-level error attribution feeds skill lifecycle audit (#414). No new substrate. |
| **#541** Mixed-selectivity binding (Rigotti et al. 2013) | `crates/context-graph-mejepa/src/predictor/mixed_selectivity_binding.rs` nonlinear basis layer between per-slot encoders and late fusion; replaces #390's linear pairwise statistics. Inputs are same-embedder scalar summaries only (anti-compensation preserved). |

**Tier 2 — design-influence on existing epics:**

| Issue | Design influence |
|---|---|
| **#542** Population-dynamics trajectories | #416 pathway exploration design |
| **#543** Per-slot sparse representations | Micro-analog of #519 macro-sparsity; per-slot top-k gates |
| **#544** Synchrony-based temporal binding | #392 temporal trajectory features design |
| **#545** Idle-time generative rollouts | Composes with #416 / #517 / #406 / #520 |

**Doctrinal compatibility verified before filing:** slot identity preserved, anti-compensation invariant (#394) preserved, binary contract preserved, ambiguity boundary preserved, no inner LLM, no synthetic-input shortcuts. See `08 §6.9`.

---

## 19. What This Doc Does NOT Cover

- Exact transformer layer counts / hidden dims at runtime — read `MeJepaInferConfig` from active checkpoint manifest.
- Per-cell calibration scores — live in `CF_MEJEPA_CALIBRATION_HISTORY` and `CF_MEJEPA_FINGERPRINT_CALIBRATION`; inspect via `mejepa_constellation_inspect` and `mejepa_fingerprint_inspect`.
- Heal-scheduler scheduling cadence — controlled by `~/.config/mejepa/daemon.toml` and per-cell convergence ETA tracking.
