# 16 — Phase 6/7/8 + UTML Hardening + FSV Refresh (2026-05-11)

> **Status:** This is the current authoritative current-state ledger. It supersedes the "Phase 6, 7, 8 still pending" framing in docs 09 §0, 10 (build-plan delta table), 11 §13 (UTML roadmap), 12 §11 (embedder roadmap), 14 §0 (chunking status), and 15 ("Still to do" list). Doc 15 remains the authoritative FSV-evidence ledger for Phases 1-5; this doc is its extension into Phases 6-8 and the rest of the UTML carrier set. Any earlier doc that says "Phase 6 / Phase 7 / Phase 8 / pairwise MI / continual-backprop / state-transfer T is pending" is stale relative to this doc.
>
> **Source-of-truth code anchors:** `crates/context-graph-mejepa-hygiene/`, `crates/context-graph-mejepa-shift-subscriber/`, `crates/context-graph-mejepa/src/eval/`, `crates/context-graph-mejepa-train/src/learning_signal/`, `crates/context-graph-mejepa-cf/`, `crates/context-graph-mcp/src/handlers/tools/{mejepa_hygiene_tools,mejepa_phase7_tools,mejepa_phase7_storage,mejepa_eval_tools,mejepa_tools}.rs`, `crates/context-graph-mcp/src/tools/names.rs`.

---

## 0. Headline

Doc 15 (2026-05-10) reported Phases 1-5 hardened with FSV. Since then, Phases 6, 7, 8 and the pending UTML carriers have shipped end-to-end:

- **Phase 6 (Hygiene)** — full `context-graph-mejepa-hygiene` crate with tier transitions, quota enforcement + emergency eviction, witness-chain Merkle compression, nightly GC orchestrator, and 3 MCP wrappers (`mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`). 3 FSV examples (`gc_stress_fsv`, `merkle_compression_fsv`, `quota_overflow_fsv`).
- **Phase 7 (Shift Subscriber)** — full `context-graph-mejepa-shift-subscriber` crate with restart-safe Tokio tailer (`run_until_idle` / `run_until_shutdown`), watermark CF reads/writes, lag-alert system, panic supervision, latency telemetry, and end-to-end shift→`observe()` pipeline. 3 MCP tools wired (`mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`). FSV: `shift_subscriber_fsv` plus in-crate `tokio::test` round-trip from JSONL line through `infer_db` live-prediction CF readback.
- **Phase 8 (Evaluation)** — full `eval/` module under `context-graph-mejepa` with `EvalRunner`, `EvalReport`, ship-gate failure analysis, conformal/OOD per-language stratification, Bayesian shrinkage, ActiveLearning queue, PatchSimilarityGraph, weekly report writer, RocksDB-backed `RocksDbEvalStore`. 4 MCP tools wired (`mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`). FSV: `phase8_eval_fsv` produces 17 evidence files including ship-gate, regression, conformal, OOD, state-transfer, aux-head distillation, leakage, Bayesian shrinkage, provenance, latency, MCP round-trip.
- **UTML carriers** — `learning_signal/{pairwise_mi_matrix,continual_backprop,state_transfer,consolidation,classifier,embedder_coherence}.rs` ship the previously-pending REQ-UTML-06/07/08 items: `PairwiseMiAuditor` + CSV writer, `DormantUnitDetector` + `reinit_dormant_units`, `compute_state_transfer` + `performance_deploy`, `ConsolidationMt`, UTML state classifier + prune decision, embedder-coherence Jaccard.
- **Shared CF crate** — new dependency-free `context-graph-mejepa-cf` with 32 canonical CF name constants + grouping consts (`SELF_HEALING_CFS`, `SELF_HEALING_REFERENCED_CFS`, `INFER_CFS`, `EVAL_CFS`, `HYGIENE_CFS`) so runtime / hygiene / MCP / training crates share names without cycles.

The active forward edge is now **operational/integration work, not greenfield code**: full 300×8 SWE-bench Lite corpus run, neural per-embedder forward wrappers (E1/E7/E8/E10/E12/E13/E14), analyzer/embedder-backed semantic instrument heads, chunk-level TCT + per-chunk `GuardRejected` localization, legacy `AstCodeChunker` convergence/deprecation, and live calibration + production rolling-window evidence against the ≥0.95 ship gate.

---

## 1. Phase 6 — Storage hygiene (FSV+MCP shipped)

### 1.1 What ships

| Capability | Module | Public surface |
|---|---|---|
| Tier transitions (hot/warm/cool/cold/frozen ↔ 8/7/5/3/1-bit) | `tier.rs`, `tier_frozen.rs`, `tier_ops.rs` | `tier_for_score`, `decayed_score`, `score_after_read`, `encode_for_tier`, `decode_from_tier`, `tier_demote`, `tier_promote`, `tier_demote_all`, `tier_promote_all` |
| Quota enforcement | `quota.rs`, `quota_types.rs` | `quota_status`, `quota_check_and_evict` returning `QuotaStatus` / `QuotaEvictionReport` |
| Witness-chain Merkle compression of >90d segments | `witness_compress.rs` | `witness_compress_old_segments`, `verify_witness_integrity`, `audit_verify_old_entry`, `WitnessCompressionReport`, `WitnessIntegrityReport` |
| Nightly GC orchestrator | `gc.rs` | `gc_run_nightly` returning `GcReport` |
| MCP wrappers | `mcp.rs`, `ops.rs` | `mcp_gc_run`, `mcp_quota_status`, `mcp_witness_compress`, `heal_detect_drift`, `heal_retrain_if_needed` |

### 1.2 MCP surface

Wired in `crates/context-graph-mcp/src/handlers/tools/mejepa_hygiene_tools.rs` and dispatched from `dispatch.rs`:

- `mcp__cgreality__mejepa_gc_run` (REQ-HYG-04, REQ-HYG-06)
- `mcp__cgreality__mejepa_quota_status` (REQ-HYG-05, REQ-HYG-06)
- `mcp__cgreality__mejepa_witness_compress` (REQ-HYG-03, REQ-HYG-06)

### 1.3 FSV evidence

| Example | What it proves |
|---|---|
| `cargo run --release -p context-graph-mejepa-hygiene --example gc_stress_fsv` | 10,000-entry stress corpus → nightly GC keeps disk under quota (REQ-HYG-07). |
| `cargo run --release -p context-graph-mejepa-hygiene --example merkle_compression_fsv` | Witness-chain segments older than 90 days roll into Merkle DAG; integrity verifier round-trips; old-entry audit. |
| `cargo run --release -p context-graph-mejepa-hygiene --example quota_overflow_fsv` | Hard quota breach triggers fail-closed eviction, per-CF accounting, and a structured `QuotaEvictionReport`. |

### 1.4 Codec + MCP path discipline

- Codec was migrated from `bincode` to JSON (`6c478ee6`/`bb7ba361`) so hygiene records survive schema evolution without a binary-shape migration.
- MCP tool handlers fail closed on path arguments outside the project's data root.
- Panic paths in the hygiene runtime are converted to structured `OpsResult<_>` errors so MCP callers receive recoverable error codes, not a process crash.

---

## 2. Phase 7 — Shift subscriber (FSV+MCP shipped)

### 2.1 What ships

`crates/context-graph-mejepa-shift-subscriber/` exports a complete restart-safe pipeline:

| Capability | Module | Public surface |
|---|---|---|
| Tail JSONL with restart-safe byte-offset cursor | `tail.rs` | `ShiftLogTail::{new, offset, path, poll_next_line}` |
| Watermark CF (CF_MEJEPA_SHIFT_WATERMARK) read/write | `watermark.rs`, `models.rs::WatermarkRecord` | `WatermarkWriter::{new, write, read_all}`; `WatermarkRecord::validate` |
| Per-shift instrument cache | `cache.rs` | `InstrumentCache`, `SlotKey` |
| `shift → InferenceObservation` transform | `shift_transform.rs` | `shift_to_inference` |
| Subscriber driver with Tokio runtime + supervised process loop | `subscriber.rs`, `runner.rs` | `MeJepaShiftSubscriberConfig`, `ShiftSubscriber::{open, run_until_idle, run_until_shutdown, process_shift, metrics, read_watermarks}`, `SubscriberRunSummary` |
| Live status / latency / lag / panic telemetry | `models.rs::{SubscriberMetrics, LatencySnapshot, LiveSubscriberStatus, PanicInfo}` | `SubscriberMetrics::{record_latency_ms, record_panic, set_lag_alert_active, mark_processed, mark_observed, snapshot_counts, snapshot_latency_ms}` |
| Skip semantics for non-code shifts | `models.rs::SkipReason` | `SkipReason::NoOracleSignal` etc. |

### 2.2 MCP surface

Wired in `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs` + `mejepa_phase7_storage.rs` and dispatched from `dispatch.rs`:

- `mcp__cgreality__mejepa_observe_shift` (REQ-SHIFT-05)
- `mcp__cgreality__mejepa_subscriber_status` (REQ-SHIFT-06)
- `mcp__cgreality__mejepa_capture_audit` (REQ-SHIFT-07)
- `mcp__cgreality__mejepa_predict_latest` (REQ-SHIFT-04, previously shipped)

Legacy `reality_run_attempt` / `reality_alter` MCP tools now return retired-tool errors (`14d53ecd`) and new harness-transition shifts wire end-to-end into `shift_to_inference` (`7bd7cbdb`).

### 2.3 FSV evidence

| Example | What it proves |
|---|---|
| `cargo run --release -p context-graph-mejepa-shift-subscriber --example shift_subscriber_fsv` | End-to-end durable JSONL → tailer → instrument-cache lookup → `shift_to_inference` → observe → `infer_db` live-prediction CF readback; watermark restart-safe. |
| In-crate `tokio::test` `run_until_idle_consumes_jsonl_and_persists_readback_state` | Single shift → `processed=1`, `observed=1`, watermark updated to byte-offset = line.len()+1, `RocksDbInferStore::read_live_predictions` returns one prediction. |
| In-crate `tokio::test` `run_until_idle_skips_non_code_shift_and_advances_watermark` | Non-code shift advances watermark, increments `processed=1`, leaves `observed=0` and `RocksDbInferStore::read_live_predictions` empty. |

### 2.4 What "restart-safe" means in practice

`ShiftSubscriber::open` reconstructs `ShiftLogTail` cursors from the current `WatermarkWriter::read_all` snapshot; on resume, the tail starts from the persisted byte offset for each session JSONL. `update_lag_alert` measures pending bytes after the cursor; `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` fires when pending shifts exceed `lag_alert_threshold_shifts` for `lag_alert_sustain_seconds`. Panics in `process_shift` are caught (`process_shift_supervised` + `catch_unwind`), recorded into `SubscriberMetrics::record_panic`, and converted into `MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC` errors; the subscriber loop bubbles the error up cleanly.

### 2.5 Latency / liveness telemetry

`SubscriberMetrics` exposes:

- `processed`, `observed`, `dropped_l_step_below_threshold`, `lag_alert_active` counters (`snapshot_counts`).
- Wall-clock latency p50/p99 in ms (`snapshot_latency_ms` → `LatencySnapshot { p50_ms, p99_ms, samples }`).
- Last task-alive timestamp (`task_alive_since`) and last panic detail (`last_panic`) for `mejepa_subscriber_status` MCP responses.

REQ-SHIFT-03's "p50 < 5 s, p99 < 30 s" is now measurable end-to-end; the live distribution is captured in `LatencySnapshot` and surfaced through `mejepa_subscriber_status`. The remaining work is recording these in production over enough turns to ship a verified percentile, not building the measurement plumbing.

---

## 3. Phase 8 — Evaluation harness (FSV+MCP shipped)

### 3.1 What ships

`crates/context-graph-mejepa/src/eval/`:

| Module | Public surface |
|---|---|
| `types.rs` | `EvalConfig`, `HoldoutPanel`, `EvalObservation`, `ConformalHealthEntry`, `ActiveLearningSummary`, `StateTransferDiagnostic`, `AuxHeadDistillationSummary`, `RegressionCheck`, `OpenResearchQuestionStatus`, `EvalProvenance`, `EvalReport`, `MutationCategory::{KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError}`, `language_slug`, `cell_key`. |
| `runner.rs` | `EvalRunner::{new, run_holdout_eval}`, `corpus_sha_from_holdout`. |
| `metrics.rs` | `pearson_correlation`, `oracle_target`, `compute_correlations`, `conformal_health`, `ood_auc_by_language`, `gtau_pass_rate_by_language`, `bayesian_shrinkage`, `ship_gate_failures`. |
| `queue.rs` | `ActiveLearningQueueState::{new, enqueue_uncertain}`, `ActiveLearningQueueEntry`, `ActiveLearningLabel`, `LabelMethod`. |
| `report.rs` | `write_weekly_report`, `seed_open_research_questions`, `write_json_0600`. |
| `store.rs` | `RocksDbEvalStore::{persist_report, persist_graph, persist_queue, load_latest_report, load_queue, load_graph, load_research_questions}`. |
| `graph.rs` | `PatchEmbedding`, `PatchGraphEdge`, `PatchSimilarityGraph`, `build_patch_similarity_graph`. |
| `fixtures.rs` | `build_eval_compiler`, `synthetic_holdout`, `synthetic_patch_embeddings`. |
| `error.rs` | `EvalError`, `EvalErrorCode`. |

### 3.2 RocksDB CFs (in `context-graph-mejepa-cf`)

`CF_MEJEPA_EVAL_REPORTS`, `CF_MEJEPA_TASK_GRAPH`, `CF_MEJEPA_TASK_GRAPH_HISTORY`, `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`, `CF_MEJEPA_ACTIVE_LEARNING_LABELS`, `CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS`, `CF_MEJEPA_OOD_ESCALATIONS`, `CF_MEJEPA_RESEARCH_QUESTIONS`, `CF_MEJEPA_MODEL_PROMOTIONS`. The grouping consts `EVAL_CFS`, `INFER_CFS`, `HYGIENE_CFS`, `SELF_HEALING_REFERENCED_CFS` define the open-set every storage handle uses; `tests::no_duplicate_hygiene_cfs` enforces no-duplication.

### 3.3 MCP surface

Wired in `crates/context-graph-mcp/src/handlers/tools/mejepa_eval_tools.rs` and dispatched from `dispatch.rs`:

- `mcp__cgreality__mejepa_eval_run` (REQ-EVAL-01..03)
- `mcp__cgreality__mejepa_ship_gate_status` (REQ-EVAL-03)
- `mcp__cgreality__mejepa_active_learning_queue` (REQ-EVAL-04)
- `mcp__cgreality__mejepa_eval_build_graph` (REQ-EVAL-05)

### 3.4 FSV evidence

`cargo run --release -p context-graph-mejepa --example phase8_eval_fsv` writes 17 evidence files mode `0600` to `/tmp/contextgraph-mejepa-phase8-eval-fsv/`:

- `holdout-eval-evidence.json` — full report including per-(category, language) Pearson correlation, conformal coverage, OOD AUC, Gτ pass rate, Bayesian shrinkage, ship-gate failure analysis.
- `latest-report-readback.json` — RocksDB round-trip of the same report.
- `active-learning-queue.json` — uncertainty-driven queue contents from `ActiveLearningQueueState::enqueue_uncertain`.
- `ship-gate.json` — `{ship_gate_passed, failures, report_hash}`.
- `patch-similarity-graph.json` — `PatchSimilarityGraph` round-trip from `eval_store.persist_graph` → `eval_store.load_graph`.
- `regression-evidence.json`, `ood-evidence.json`, `conformal-evidence.json` — slice-level health checks.
- `q1q2q3-evidence.json` — pass rate, report correlation, and live-prediction readback agreement for approved predictions (the "three operator questions").
- `state-transfer-evidence.json` — UTML state-transfer T diagnostic.
- `aux-head-distillation.json` — REQ-EVAL-06 trunk distillation summary when real distillation evidence exists; remains `null` instead of fabricating acceptance.
- `open-research-questions.json` — `seed_open_research_questions` snapshot.
- `leakage-check-evidence.json` — `assert_no_test_patch_leakage`-style edge-case audit.
- `bayesian-shrinkage.json`, `provenance.json`, `latency.json`, `mcp-roundtrip.json`, `manifest.json`.

### 3.5 What ship-gate currently means at the code layer

`ship_gate_failures` (`metrics.rs`) inspects overall correlation, conformal lower-bound health, OOD AUC, Gτ pass rate, Q1 pass rate, Q3 live-prediction readback agreement, and regression checks. Q3 is now derived from `CF_MEJEPA_LIVE_PREDICTIONS` readback for approved predictions; missing side-effect evidence fails the gate instead of returning `Some(1.0)`. Regression requires a previous persisted `EvalReport`; the Phase 8 FSV writes a baseline report and then verifies the current report against it, so `CF_MEJEPA_EVAL_REPORTS` contains two rows. `EvalReport::ship_gate_passed` is the AND across all required checks; `EvalReport::ship_gate_failures` is the explanatory list. The remaining shipping work is rolling this against the real 300×8 SWE-bench Lite corpus over enough holdout windows to show a stable ≥0.95 — not building the gate; the gate exists, runs, and persists.

---

## 4. UTML — pairwise MI / continual backprop / state transfer (REQ-UTML-06..08 shipped)

### 4.1 What ships

`crates/context-graph-mejepa-train/src/learning_signal/`:

| Module | Public surface | UTML mapping |
|---|---|---|
| `pairwise_mi_matrix.rs` | `PairwiseMiMatrix`, `PairwiseMiAuditor::{new, should_run, run_from_panels, run_from_slot_series}`, `PairwiseMiSummary`, `compute_pairwise_mi_matrix`, `load_pairwise_mi_matrix_csv` | REQ-UTML-06 — pairwise MI audit job persisting `pairwise_mi_matrix.csv`; ME-JEPA's predictor-side ΔΞ defense beyond VICReg. |
| `continual_backprop.rs` | `DormantLayer`, `DormantUnitDetector`, `DormantUnitReport`, `detect_dormant_units`, `reinit_dormant_units` | REQ-UTML-07 — continual-backprop dormant-unit re-init; the ΔΩ.effective_plasticity intervention. |
| `state_transfer.rs` | `StateTransferDiagnostic`, `compute_state_transfer`, `performance_deploy` | REQ-UTML-08 — state-transfer T detector; surfaces active-learning trigger when train-time L_step diverges from deploy-time agreement. |
| `consolidation.rs` | `ConsolidationEvidence`, `ConsolidationMt::compute_m_t`, `update_m_t`, `detect_dissipating` | UTML §VIII — consolidation integral M(t); periodic full-retrain trigger. |
| `classifier.rs` | `ClassifierVerdict`, `LStepWindow`, `PruneDecision`, `classify_utml_state`, `prune_decision` | UTML §VII — multiplicative-gate state classifier + pruning rule that decides whether to keep a primitive in the active rotation. |
| `embedder_coherence.rs` | `EmbedderCoherenceReport`, `compute_embedder_coherence`, `jaccard` | UTML §IV (ΔK carrier) — cross-embedder agreement Jaccard; load-bearing for the predictor-collapse score. |

### 4.2 ΔΞ structural invariant (recap from doc 15)

`learning_signal.rs::compute_delta_xi` already fails closed on non-zero / non-finite `target_collapse` with `MEJEPA_INSTRUMENT_GRADIENT_LEAK` before any training certificate is constructed or written. ΔΞ now uses the multiplicative non-collapse formula. This is the structural collapse-immunity property doc 11 §0 calls "ΔΞ.target_collapse = 0 by construction".

### 4.3 What's still operational, not greenfield

REQ-UTML-09 (synthetic FSV envelope spanning ΔP/ΔK/ΔΩ/ΔΞ corner cases) is covered by `train_edge_cases_fsv` and refreshed by `utml_l_step_fsv`. The operational work that remains is:

- **Wire the pairwise MI matrix into per-step train certs as a column** so doc 12 §11 item 17 (cross-embedder coherence into ΔK) is observable from the cert chain.
- **Trigger continual-backprop dormant-unit re-init from the live ΔΩ.effective_plasticity signal** in `heal/regulate.rs` — the detector + re-init exist; the policy hook in self-healing remains.

### 4.4 MCP surface

`mcp__cgreality__mejepa_audit_pairwise_mi` is exposed through `context-graph-mcp` and writes a CSV source-of-truth under the configured data root, then reloads it before returning.

---

## 5. Shared CF crate (`context-graph-mejepa-cf`)

A new dependency-free crate centralizes 32 RocksDB column-family names so runtime / hygiene / training / MCP surfaces share one source of truth. Shipping this eliminated several near-misses where two crates spelled the same logical CF differently. The grouping consts also define the open-set every store handle uses; `no_duplicate_hygiene_cfs` is enforced by a unit test.

---

## 6. What exists now (Phase 6 / 7 / 8 / UTML — code anchors)

### Crates added since doc 15 (2026-05-10)

| Crate | Purpose |
|---|---|
| `crates/context-graph-mejepa-hygiene/` | Phase 6 storage hygiene + MCP wrappers + 3 FSV examples. |
| `crates/context-graph-mejepa-shift-subscriber/` | Phase 7 restart-safe shift subscriber + 1 FSV example + tokio::test round-trips. |
| `crates/context-graph-mejepa-cf/` | Shared CF name registry. |

### Modules added inside existing crates

| Path | Purpose |
|---|---|
| `crates/context-graph-mejepa/src/eval/` | Phase 8 evaluation harness + RocksDbEvalStore + ship-gate + active-learning queue + patch-similarity graph + weekly report. |
| `crates/context-graph-mejepa/examples/phase8_eval_fsv.rs` | Phase 8 FSV (17 evidence files). |
| `crates/context-graph-mejepa-train/src/learning_signal/` | UTML pairwise MI / continual backprop / state transfer / consolidation / classifier / embedder coherence. |
| `crates/context-graph-mejepa-train/examples/utml_l_step_fsv.rs` | UTML carrier FSV. |
| `crates/context-graph-mcp/src/handlers/tools/mejepa_hygiene_tools.rs` | Phase 6 MCP wrappers. |
| `crates/context-graph-mcp/src/handlers/tools/mejepa_phase7_tools.rs` + `mejepa_phase7_storage.rs` | Phase 7 MCP wrappers. |
| `crates/context-graph-mcp/src/handlers/tools/mejepa_eval_tools.rs` | Phase 8 MCP wrappers. |

### MCP tools added since doc 15

```
mcp__cgreality__mejepa_gc_run                    # Phase 6 nightly GC
mcp__cgreality__mejepa_quota_status              # Phase 6 quota + emergency-eviction state
mcp__cgreality__mejepa_witness_compress          # Phase 6 Merkle compression of >90d segments
mcp__cgreality__mejepa_observe_shift             # Phase 7 manual one-shift observe
mcp__cgreality__mejepa_subscriber_status         # Phase 7 latency/lag/panic telemetry
mcp__cgreality__mejepa_capture_audit             # Phase 7 attempt-bound capture audit
mcp__cgreality__mejepa_eval_run                  # Phase 8 holdout-eval driver
mcp__cgreality__mejepa_ship_gate_status          # Phase 8 ship-gate inspection
mcp__cgreality__mejepa_active_learning_queue     # Phase 8 active-learning queue
mcp__cgreality__mejepa_eval_build_graph          # Phase 8 patch-similarity graph build
```

Combined with the doc-15 set (`mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`), the live ME-JEPA MCP surface is **18 tools**.

---

## 7. Updated Phase status table (supersedes doc 15 §"Still to do" and earlier docs)

| Phase | Code state | FSV state | Operational state |
|---|---|---|---|
| 0 — Corpus | ✅ all 8 mutation operators (Python), split, oracle bridge, batch CLI, RocksDB CFs, source-site provenance | ✅ `mejepa_corpus_fsv`, `oracle_fsv`, `oracle_store_fsv`, `split_fsv`, `swebench_oracle_fsv` | full 300×8 SWE-bench Lite batch run pending; per-language operators (Rust, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) pending |
| 1a — AST chunking | ✅ `context_graph_core::memory::ast` 11-language API + chunk SHA + routing sets + fail-closed errors | ✅ `ast_chunker_multilang_fsv` | converge/deprecate legacy `AstCodeChunker`; reformat-invariant AST-node-ID SHA audit |
| 1b — Embedders | ✅ 21-slot enum (E5/E11 retired), 19 active registrations, 12 active content, 7 learner-state, digest verification, VRAM budget, 143 routing rows | ✅ `embedders_fsv` | **deep blindspot**: per-embedder neural `EmbedderForward` / `Instrument` wrappers running real model forward passes for E1/E7/E8/E10/E12/E13/E14 |
| 1c — Instruments | ✅ all 15 panel slots + Panel/PanelBuilder/PanelStore + linear-probe quality gate; E_Oracle / E_Witness / E_StaticAnalysis first-class | ✅ `instruments_fsv`, `phase1_extended_fsv`, `ast_embedder_smoke_fsv` | replace deterministic encoders with embedder/analyzer-backed heads for E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test, E_Trace, E_Runtime, E_Reasoning |
| 2 — Predictor | ✅ `MeJepaPredictor`, `VICRegLoss`, `OracleHead`, `FrozenTargetAdapter`, `PanelSource` | ✅ `predictor_fsv` | live latency / VRAM percentile capture under production load |
| 3 — Training + UTML | ✅ Trainer + AdamW + cosine LR + BatchSampler + 100k-step cert chain + holdout eval + LoRA wiring + distillation; ΔP/ΔK/ΔΩ/ΔΞ formulas hardened and `compute_delta_xi` fail-closed | ✅ 5 training FSVs incl. `train_full_fsv` byte-readback + refreshed `utml_l_step_fsv` | production-scale cert-chain monitoring |
| 3+ UTML — pairwise MI / continual BP / state transfer | ✅ shipped (§4 above), including `mcp__cgreality__mejepa_audit_pairwise_mi` | ✅ `utml_l_step_fsv`, `train_edge_cases_fsv` | wire pairwise MI into per-step cert columns; trigger continual-BP re-init from `heal/regulate.rs` |
| 4a — Inference | ✅ `MeJepaCompiler::{compile, verify, observe}` + conformal + OOD + `Verdict` | ✅ `inference_fsv` | live conformal coverage + OOD AUC measurement |
| 4b — TCT | ✅ panel-level `TctConstellation` + `Gτ` + `GuardRejected` + Bayesian shrinkage + freshness + threshold calibrator | ✅ `tct_fsv` | per-AST-chunk centroids + chunk-level Gτ + chunk-level `GuardRejected` localization |
| 5 — Self-healing | ✅ 16-module `heal/` tree + `mejepa-heal-drill` binary + 6 RocksDB CFs + 3 MCP tools | ✅ hard/soft/catastrophic drift drills, mode-C promotion only on holdout win | wire continual-backprop intervention from §4 above into `heal/regulate.rs` |
| 6 — Hygiene | ✅ `context-graph-mejepa-hygiene` crate, tier transitions, quota + emergency eviction, witness Merkle compression, nightly GC, JSON codec, MCP path discipline | ✅ `gc_stress_fsv`, `merkle_compression_fsv`, `quota_overflow_fsv` | live nightly GC scheduler under cron + 115 GB hard quota in production |
| 7 — Shift subscriber | ✅ `context-graph-mejepa-shift-subscriber` crate, restart-safe Tokio tailer, watermark CF, lag alert, panic supervision, latency telemetry, end-to-end shift→observe→prediction wiring | ✅ `shift_subscriber_fsv` + 2 in-crate `tokio::test` round-trips | record real-world p50/p99 distribution under production traffic against REQ-SHIFT-03 envelope |
| 8 — Evaluation | ✅ `eval/` module + `EvalRunner` + ship-gate + active-learning + patch-similarity graph + weekly report writer + 4 MCP tools | ✅ `phase8_eval_fsv` (17 evidence files) | run rolling-window holdout against full 300×8 corpus with per-language stratification; ship gate ≥0.95 across all (category, language) cells |

---

## 8. Critical blindspots (current — replaces doc 15 §"Critical blindspots" where overlapping)

1. **Neural embedder forward execution still pending.** Phase 1b proves real artifact existence, digest integrity, VRAM budget, active/retired policy, learner-state calibration, and routing coverage; it does **not** prove that any of E1/E7/E8/E10/E12/E13/E14 actually runs a forward pass and produces a finite, expected-dimension vector. The `EmbedderForward` / `Instrument` wrappers are the largest deep-integration item between "control surface FSV" and "production-ready embedding pipeline."
2. **Panel-level vs chunk-level TCT.** `Verdict::GuardRejected` exists at the panel level. Per-AST-chunk centroid construction over the now-shipped `memory::ast` chunk IDs and chunk-level guard diagnostics are still required to surface function/class-specific failures with named geometric reasons (per doc 13 §3.3).
3. **Synthetic FSV vs SWE-bench gate.** Every Phase 1-8 FSV today uses deterministic synthetic fixtures to prove mechanics. The full 300×8 SWE-bench Lite corpus + rolling-window holdout evaluation against the ≥0.95 ship gate is the evidence required for the "review replacement" claim. The harness, ship gate, active-learning queue, and weekly report all exist; the missing piece is the production data.
4. **Operational scheduling.** Nightly GC, `mejepa-shift-subscriber` Tokio task, and `EvalRunner::run_holdout_eval` all run; they are not yet on a cron / systemd-timer / supervised long-running process. This is operational shipping, not code shipping, but it's load-bearing for the autonomous ME-JEPA narrative.
5. **`mejepa_audit_pairwise_mi` MCP tool gap.** The audit code exists in `learning_signal/pairwise_mi_matrix.rs` and round-trips CSV; the MCP wrapper that surfaces it for operator review (doc 12 item 20) is the obvious follow-up.
6. **Legacy `AstCodeChunker` not yet deprecated.** The Rust-only legacy API in `crates/context-graph-core/src/memory/ast_chunker.rs` still exists for compatibility; new work targets `context_graph_core::memory::ast::{chunk, chunk_path, chunk_with_options}`. Either converge the old API or mark it `#[deprecated]` with a migration shim.
7. **Per-language mutation operators.** Phase 0's 8 mutation operators are Python-only. Doc 14's chunking now spans 11 languages; the corpus generator should follow. Until it does, ME-JEPA's training corpus is single-language and the cross-language ship-gate is structurally untestable.
8. **WSL GPU telemetry constraint (carried from doc 15).** `nvidia-smi` exposes a limited feature set under WSL 2; VRAM gates must continue to use the code's CUDA/allocator readbacks and fail closed when telemetry is absent.

---

## 9. Cross-references

- Phase 1-5 hardening evidence ledger: `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`
- Master plan (architecture authority): `09_END_GOAL_REVIEW_REPLACEMENT.md`
- Post-pivot architecture: `10_MEJEPA_PIVOT_2026_05_09.md`
- UTML v1.0 learning formula: `11_MEJEPA_LEARNING_FORMULA.md`
- 21-embedder ensemble: `12_EMBEDDER_INTEGRATION.md`
- TCT framework: `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- AST chunking: `14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- Per-spec status table: `specs/_README.md`
- Per-phase deliverable table: `specs/implementation-plan.md`
- Spec hierarchy authority: `/home/cabdru/contextgraph/docs2/prdtospec.md`
- UTML source theory: `/home/cabdru/contextgraph/docs2/TheoryOfMachineLearning.md`
