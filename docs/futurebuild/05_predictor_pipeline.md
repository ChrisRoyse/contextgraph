# 05 — Predictor Pipeline (Embedders → Instruments → Panels → Heads → TCT → Training)

**Last refreshed:** 2026-05-19.

Source files covered: `crates/context-graph-mejepa-embedders/src/lib.rs` + modules, `crates/context-graph-mejepa-instruments/src/lib.rs` + modules, `crates/context-graph-mejepa/src/lib.rs` + `compiler.rs` + `verdict_assembly.rs` + `claim_graph.rs` + `patch_similarity.rs` + `failure_fingerprint.rs` + `project_ingest.rs` + `live_session_trace.rs`, `crates/context-graph-mejepa-tct/src/lib.rs` + modules, `crates/context-graph-mejepa-train/src/lib.rs` + `learning_signal.rs` + `replay_buffer.rs` + `compression_progress.rs` + `loss/inverse.rs`, `crates/context-graph-mejepa-corpus/src/lib.rs` + ops, `crates/context-graph-mejepa-shift-subscriber/src/lib.rs`, `crates/context-graph-mejepa/src/heal/`.

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
      15 frozen instruments → typed 5,120-d Panel storage layout
        E_AST 384 | E_CFG 256 | E_DataFlow 256 | E_TypeGraph 256 |
        E_Test 384 | E_Trace 512 | E_Diff 256 | E_Witness 256 |
        E_Oracle 128 | E_Problem 1024 | E_CommitMsg 384 |
        E_StaticAnalysis 256 | E_Runtime 256 | E_Reasoning 384 |
        Scalars 128
                 │  PanelBuilder validates: dim + finite + no double-fill
                 ▼
      CF_MEJEPA_PANELS + CF_MEJEPA_PANEL_META + CF_MEJEPA_SHIFT_WATERMARK
                 │
                 ▼
      MeJepaPredictor.forward(panel)
        + per-head DDA projection (CF_MEJEPA_HEAD_PROJECTION_REGISTRY)
        + split-conformal interval (CF_MEJEPA_CALIBRATION_HISTORY)
        + OOD gate (CF_MEJEPA_OOD_ESCALATIONS)
        + TCT Gτ guard (CF_MEJEPA_GUARD_DECISIONS)
        + cold-cell metric (CF_MEJEPA_COLD_CELL_METRICS, abstain if n_supporting<50)
        + matched fingerprint (CF_MEJEPA_FINGERPRINT_REVERSE_INDEX)
        + closest exemplars HNSW (CF_MEJEPA_PATCH_SIMILARITY_META)
        + claim graph reconciliation (CF_MEJEPA_CLAIM_RECONCILIATION)
                 │
                 ▼
      RealityPrediction (Q1 claim_graph + Q2 verdict + Q3 fail modes +
                         Q4 always-on heads + Q5 reality_impact)
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

## 3. Instrument Panel (15 slots, 5,120-d storage layout)

`crates/context-graph-mejepa-instruments/src/lib.rs:158-199`. `PANEL_DIM = 5_120`. `SLOT_COUNT = 15`. Layout MUST sum to PANEL_DIM (asserted at debug build via `debug_assert_eq!`).

**Semantic contract:** the panel is a typed constellation carrier, not one fused embedding space. Each slot remains an independent evidence surface with its own encoder, dimension, filled bit, and downstream violation attribution. Cross-slot correlations and pairwise MI are valid signals; cross-slot compensation is not. A wrong/weak slot must fail closed even when the aggregate 5,120-d distance looks acceptable.

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

**Total:** 5,120 (4992 + 128 = 5,120 ✅).

**Panel invariants** (`PanelBuilder::set_slot`, `validate_panel_parts`):
- `vector.len() == slot.dim()` else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Every element finite else `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`
- Slot not already filled else `MEJEPA_INSTRUMENTS_INVALID_INPUT`
- Build rejects `filled_mask == 0` (no slots set is indistinguishable from missing evidence)
- `panel.data.len() == PANEL_DIM`
- `filled_mask` bits outside `VALID_FILLED_MASK` (0x7FFF) rejected
- All persistence flows through `try_from PanelSerde` so JSON round-trip re-validates

**Structural collapse-immunity invariant.** Instruments carry zero trainable parameters. `FrozenGuard` / `FrozenSnapshot` / `hash_f32s` (`frozen_hook.rs`) enforce `ΔΞ.target_collapse = 0` by construction. `crates/context-graph-mejepa/src/grad_hook.rs::run_gradient_hook` checks for `MEJEPA_INSTRUMENT_GRADIENT_LEAK` and fails closed if any gradient touches a frozen-target tensor.

**Composite reward signals.** `composite_reward_signals.rs` analyzes coverage + churn + complexity + doc-coverage + dep-CVE labels (TASK-RWD-209). `q4_security_labels.rs` runs Bandit + Semgrep. `static_analysis_reward.rs` runs mypy + pyright + ruff. `type_graph_signals.rs`, `telemetry_signals.rs`, `live_session_signals.rs` cover TASK-RWD-205/206/207.

---

## 4. Predictor Architecture

`crates/context-graph-mejepa/src/predictor/` — `MeJepaPredictor`, `TransformerLayer`, `ArchitectureSummary`. CUDA-required.

**P0 correction (#385):** the current predictor/head implementation still contains flat `PANEL_DIM` and `CONCAT_INPUT_DIM` projections. That is acceptable only as a storage-compatible implementation detail while #385 is open; it is not acceptable ship-gate evidence. The production architecture must preserve per-slot/per-embedder constellation semantics through predictor input, output, heads, OOD scoring, calibration, and FSV readback.

Default architecture (`config.rs`):
- `PANEL_DIM` = 5,120 input
- `CONCAT_INPUT_DIM` = (constant; ≥5120 + auxiliary)
- `DEFAULT_HIDDEN_DIM`, `DEFAULT_NUM_HEADS`, `DEFAULT_NUM_LAYERS`, `DEFAULT_FF_EXPANSION`, `DEFAULT_LAYER_NORM_EPS`
- `INVERSE_ACTION_DIM` — 16-d action latent for inverse map (TASK-EK-012)
- VICReg: `VICREG_INV_LAMBDA`, `VICREG_VAR_LAMBDA`, `VICREG_COV_LAMBDA`, `VICREG_GAMMA`
- `GRAD_NORM_NOISE_FLOOR`
- VRAM budgets: `VRAM_STEADY_STATE_TARGET_BYTES`, `VRAM_WARN_THRESHOLD_BYTES`

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

`crates/context-graph-mejepa/src/verdict_assembly.rs` stitches heads into:

```rust
enum Verdict { Pass, Fail, OutOfDistribution, Abstain, GuardRejected }
```

Decision flow (per `compiler.rs::compile`):
1. Materialize `Panel` for the candidate change.
2. Forward through `MeJepaPredictor`.
3. Project per-head DDA features (`head_projection.rs` + `CF_MEJEPA_HEAD_PROJECTION_REGISTRY`).
4. Apply split-conformal interval (`conformal.rs`).
5. **Per-slot OOD reasons (TASK-PY-G-089 / #389).** `RealityPrediction.per_slot_ood_reasons: Vec<PerSlotOodReason>` is materialized from per-slot/per-embedder calibration state under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`. Any slot/embedder/pair-specific OOD reason persists with the prediction row, byte-replays from RocksDB, and is exposed by `mejepa_explain_prediction` / `mejepa_inspect_prediction`. `MAX_EMBEDDERS_PER_PANEL` guard rejects over-wide arrays.
6. Aggregate OOD gate (`ood.rs` + `CF_MEJEPA_OOD_ESCALATIONS`). Strict-conjunction policy: aggregate score never overrides a slot violation (`aggregate_score_never_overrides_slot_violation=true` in `ood_guard_policy.json`).
7. TCT Gτ guard (`tct/gtau.rs` + `CF_MEJEPA_GUARD_DECISIONS`).
8. Cold-cell metric: if cell `n_supporting < INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT` (50) → `Verdict::Abstain` + `CF_MEJEPA_COLD_CELL_METRICS` row (TASK-PY-G-061).
9. Match against fingerprint catalog (`CF_MEJEPA_FINGERPRINT_REVERSE_INDEX`).
10. Pull closest exemplars (`CF_MEJEPA_PATCH_SIMILARITY_META`).
11. Reconcile claim graph (`CF_MEJEPA_CLAIM_RECONCILIATION`).
12. Emit `RealityPrediction` → write `CF_MEJEPA_LIVE_PREDICTIONS` + `CF_MEJEPA_HIERARCHICAL_PREDICTIONS`.

**Anti-compensation guard (TASK-PY-G-094 / #394).** `crates/context-graph-mejepa/examples/python_anti_compensation_fsv.rs` and the calibrated ablation suite (#398) prove corrupted / missing / stale / dimension-shifted slots, contradictory pair relationships, and temporal slot jumps cannot be accepted through aggregate evidence even when a flat normalized score would pass. A code scan prevents flat-vector ship-gate eligibility regressions.

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
| Merkle-tree per-project chunk/embed cache | `project_cache.rs` | #281 / TASK-PY-G-062 |
| 250 ms debounced watcher-status scan | `project_cache.rs` | #281 |
| Transitive invalidation + schema-version-aware re-embed | `project_cache.rs` | #281 |
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

## 16. Q4 Always-on Heads

| Head | Source | Issue |
|---|---|---|
| Predicted security concerns | `q4_security_labels.rs` (Bandit + Semgrep) | TASK-RWD-204 / TASK-PY-G-046 |
| Predicted perf regressions | `q4_perf_labels.rs` (pytest-benchmark + cProfile label producer) | TASK-PY-G-047 |
| Predicted accuracy degradations | `q4_accuracy_labels.rs` (ML/data metric label producer) | TASK-PY-G-048 |
| Predicted cost regressions | `q4_cost_labels.rs` (CI minutes + dep/wheel cost producer) | TASK-PY-G-049 |
| Predicted-Works + test-coverage + test-flakiness sub-heads | `prediction_surfaces.rs` | TASK-PY-G-073 |
| Reasoning class | `q4_reasoning_labels.rs` over observed agent prose + oracle/prediction outcomes; `e_reasoning.rs` remains the frozen trace/affect input slot | TASK-PY-G-050 |

`Q4SecurityClass`, `Q4SecurityDetector`, `Q4SecuritySeverity`, `Q4SecuritySource` — exported by `q4_security_labels.rs`. Default timeout: `DEFAULT_Q4_SECURITY_TIMEOUT_SECS`.
Q4 producer rows are display-only until #377's trust gate sees the relevant producer evidence, #127 calibration rows in `CF_MEJEPA_HEAD_CALIBRATIONS`, and exact active-slot evidence for the 12 active embedders.

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

## 18. Adversarial Defenses

| Defense | Source | Issue |
|---|---|---|
| Negative-action ablation falsifier | `crates/context-graph-mejepa-train/examples/phase_g_adversarial_retrain_fsv.rs` + CF_MEJEPA_ABLATION_REPORTS | TASK-PY-G-054 |
| Multi-head contradiction | `verdict_assembly.rs` + Q4 always-on | TASK-PY-G-044 |
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
