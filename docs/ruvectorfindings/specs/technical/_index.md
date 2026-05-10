# Technical Specs Index (Level 3)

> **Level 3 — how to build.** Each technical spec implements a Level 2 functional spec. Captures architecture diagrams (mermaid), data models (Rust structs/enums), API contracts (MCP tools + CLI), component contracts (trait/impl signatures), implementation notes.

**Authority:** `../constitution.md` overrides. When this index conflicts with constitution or a functional spec it implements, the higher authority wins.

---

## Spec inventory (13 technical specs, paired 1:1 with functional) — **status refreshed 2026-05-10**

| ID | File | Implements | Phase | Status (2026-05-10) |
|---|---|---|---|---|
| TECH-CORPUS | [TECH-CORPUS.md](./TECH-CORPUS.md) | SPEC-CORPUS | 0 | partial — Python operators+split+oracle bridge+CLI shipped |
| TECH-AST-CHUNKING | [TECH-AST-CHUNKING.md](./TECH-AST-CHUNKING.md) | SPEC-AST-CHUNKING | 1a | partial — Rust-only chunker; multi-language extension pending |
| TECH-EMBEDDERS | [TECH-EMBEDDERS.md](./TECH-EMBEDDERS.md) | SPEC-EMBEDDERS | 1b | partial — enum + manifest + VRAM gate shipped; real model-load wrappers pending |
| TECH-INSTRUMENTS | [TECH-INSTRUMENTS.md](./TECH-INSTRUMENTS.md) | SPEC-INSTRUMENTS | 1c | partial — all 15 slots have deterministic encoders; embedder-backed replacements pending |
| TECH-PREDICTOR | [TECH-PREDICTOR.md](./TECH-PREDICTOR.md) | SPEC-PREDICTOR | 2 | **shipped** — `crates/context-graph-mejepa/` |
| TECH-TRAINING | [TECH-TRAINING.md](./TECH-TRAINING.md) | SPEC-TRAINING | 3 | **shipped** — `crates/context-graph-mejepa-train/` |
| TECH-UTML | [TECH-UTML.md](./TECH-UTML.md) | SPEC-UTML | 3 (cross-cutting) | partial — `learning_signal.rs` shipped; pairwise MI / continual-backprop / state-transfer T pending |
| TECH-INFERENCE | [TECH-INFERENCE.md](./TECH-INFERENCE.md) | SPEC-INFERENCE | 4a | **shipped** — `compiler.rs` + `conformal.rs` + `ood.rs` + `mejepa_verify` MCP tool |
| TECH-TCT | [TECH-TCT.md](./TECH-TCT.md) | SPEC-TCT | 4b (cross-cutting) | **shipped** — `crates/context-graph-mejepa-tct/` + `mejepa_constellation_inspect` MCP tool |
| TECH-SELF-HEALING | [TECH-SELF-HEALING.md](./TECH-SELF-HEALING.md) | SPEC-SELF-HEALING | 5 | **shipped** — `heal/` tree + `mejepa-heal-drill` + `mejepa_heal_status`/`mejepa_rollback_to` MCP tools |
| TECH-HYGIENE | [TECH-HYGIENE.md](./TECH-HYGIENE.md) | SPEC-HYGIENE | 6 | partial — foundation crates shipped; integration + ops MCP tools pending |
| TECH-SHIFT-SUBSCRIBER | [TECH-SHIFT-SUBSCRIBER.md](./TECH-SHIFT-SUBSCRIBER.md) | SPEC-SHIFT-SUBSCRIBER | 7 | partial — `reality_shift_log` infra + `mejepa_predict_latest` shipped; Tokio subscriber + remaining MCP tools pending |
| TECH-EVALUATION | [TECH-EVALUATION.md](./TECH-EVALUATION.md) | SPEC-EVALUATION | 8 | partial — holdout-eval shipped; per-language stratification + ship gate + active learning pending |

---

## Cross-spec data-model authority map

When a data type appears in multiple specs, only ONE spec is the authoritative source (definition + invariants). Other specs reference by name.

| Type | Authority spec |
|---|---|
| `MutationCategory`, `OracleVerdict`, `Language`, `EntityType`, `ChunkId` | TECH-CORPUS / TECH-AST-CHUNKING |
| `EmbedderId`, `Instrument`, `InstrumentSlot`, `Panel`, `PanelBuilder` | TECH-EMBEDDERS / TECH-INSTRUMENTS |
| `MeJepaPredictor`, `OracleHead`, `FrozenTargetAdapter`, `VicregLambdas`, `LossOutputs` | TECH-PREDICTOR |
| `TrainingCertificate`, `BatchSampler`, `EpochReport`, `HoldoutReport`, `LossCoefficients` | TECH-TRAINING |
| `LearningSignal`, `DeltaPComponents`, `DeltaKComponents`, `DeltaOmegaComponents`, `DeltaXiComponents`, `UtmlState`, `ClassifierVerdict` | TECH-UTML |
| `MeJepaCompiler`, `RealityPrediction`, `VerifyVerdict`, `FailedGate`, `PatchBundle`, `TaskContext`, `ConformalSet`, `OracleOutcome` | TECH-INFERENCE |
| `TctConstellation`, `GtauViolation`, `ChunkGtauOutput`, `ConstellationHingeOutput`, `VerdictGuardRejected`, `RollingWindow` | TECH-TCT |
| `EwcPlusPlus`, `FisherMatrix`, `DriftDetector`, `DriftSeverity`, `AbcPromoter`, `ModelHandle`, `EnsembleHandle`, `PromotionGate`, `HealReport` | TECH-SELF-HEALING |
| `me_jepa::ops::*`, `GcReport`, `EvictReport`, `CompressReport`, `ChainIntegrityReport`, `BUDGETS` | TECH-HYGIENE |
| `ShiftSubscriber`, `ShiftEntry`, `ShiftOutcome`, `WatermarkRecord`, `SubscriberStatus`, `CaptureAuditBundle`, `InstrumentCache` | TECH-SHIFT-SUBSCRIBER |
| `EvalRunner`, `EvalReport`, `ShipGateStatus`, `ActiveLearningQueue`, `PatchGraph` | TECH-EVALUATION |

---

## RocksDB column family inventory

| CF name | Authority spec | Purpose |
|---|---|---|
| CF_MEJEPA_MUTATION_CORPUS | TECH-CORPUS | (task_id, mutation_category) → mutated source bytes |
| CF_MEJEPA_ORACLE_VERDICTS | TECH-CORPUS | (task_id, mutation_category) → OracleVerdict |
| CF_MEJEPA_CORPUS_PROVENANCE | TECH-CORPUS | (corpus_version, embedder_version) → provenance metadata |
| CF_MEJEPA_PANELS | TECH-INSTRUMENTS | (attempt_id, time_step) → tier-compressed panel bytes |
| CF_MEJEPA_PANEL_META | TECH-INSTRUMENTS | tier, access_count_30d, created_at, last_accessed |
| CF_MEJEPA_WEIGHT_BLOBS | TECH-TRAINING + TECH-SELF-HEALING | theta_sha → deduplicated weight blob |
| CF_MEJEPA_TRAIN_CERTS | TECH-UTML + TECH-TRAINING | step → TrainingCertificate (hash-chained) |
| CF_MEJEPA_LIVE_PREDICTIONS | TECH-INFERENCE | session_id → latest live verify() outputs |
| CF_MEJEPA_CONSTELLATION | TECH-TCT | version_id → TctConstellation (frozen) |
| CF_MEJEPA_FISHER_SNAPSHOTS | TECH-SELF-HEALING | task_boundary_id → Fisher matrix snapshot |
| CF_MEJEPA_DRIFT_WINDOW | TECH-SELF-HEALING | rolling 1,000-attempt window |
| CF_MEJEPA_DRIFT_HISTORY | TECH-SELF-HEALING | drift severity transition log |
| CF_MEJEPA_HEAL_REPORTS | TECH-SELF-HEALING | promotion + retrain reports |
| CF_MEJEPA_ACTIVE_POINTERS | TECH-SELF-HEALING | promoted weight pointer (Mode A→B→C) |
| CF_MEJEPA_CALIBRATION_HISTORY | TECH-SELF-HEALING + TECH-INFERENCE | sliding 1,000-sample calibration window |
| CF_MEJEPA_SHIFT_WATERMARK | TECH-SHIFT-SUBSCRIBER | last shift_id consumed by subscriber |
| CF_AUDIT_LOG | (existing context-graph-mcp) | append-only audit log |
| CF_TOOL_CALL_INDEX | (existing context-graph-mcp) | tool-to-memory mapping |

---

## MCP tool inventory (`mcp__cgreality__mejepa_*`)

Per constitution `<mcp_tool_namespace_lock>`, all ME-JEPA MCP tools use the `mcp__cgreality__mejepa_*` prefix. Retired surfaces (`reality_run_attempt` etc.) emit `CCREALITY_ENGINE_RETIRED` and MUST NOT be reintroduced.

| MCP tool | Authority spec | Purpose |
|---|---|---|
| mejepa_verify | TECH-INFERENCE (+ TECH-SHIFT-SUBSCRIBER wires the handler) | run static gates + return VerifyVerdict |
| mejepa_predict_latest | TECH-SHIFT-SUBSCRIBER | most recent live prediction for active session/file set |
| mejepa_observe_shift | TECH-SHIFT-SUBSCRIBER | idempotently consume specific shift_id |
| mejepa_subscriber_status | TECH-SHIFT-SUBSCRIBER | watermark + lag + error rate + cache hit |
| mejepa_capture_audit | TECH-SHIFT-SUBSCRIBER | full signal capture for an attempt |
| mejepa_constellation_inspect | TECH-TCT | inspect frozen constellation centroids + thresholds |
| mejepa_heal_status | TECH-SELF-HEALING | observe metrics + drift state + last promotion |
| mejepa_rollback_to | TECH-SELF-HEALING | rollback to a specific witness-chain offset (operator-only) |
| mejepa_gc_run | TECH-HYGIENE | trigger nightly GC pass on demand |
| mejepa_quota_status | TECH-HYGIENE | per-category budget + tier breakdown |
| mejepa_witness_compress | TECH-HYGIENE | compress chain segments older than threshold |
| mejepa_eval_run | TECH-EVALUATION | run holdout eval over rolling window |
| mejepa_eval_ship_gate_status | TECH-EVALUATION | ship-gate per-category + per-language pass/fail |
| mejepa_active_learning_queue | TECH-EVALUATION | uncertain-prediction queue for operator labeling |
| mejepa_active_learning_label | TECH-EVALUATION | submit operator label for an active-learning entry |

---

## Cross-references

- Master index: `../_README.md`
- Implementation plan: `../implementation-plan.md`
- Constitution: `../constitution.md`
- Functional specs (matched pairs): `../functional/_index.md`
- Task graph: `../tasks/_index.md`
- Traceability matrix: `../tasks/_traceability.md`
