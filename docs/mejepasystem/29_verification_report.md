# 29 — Verification Report

Source files covered: counted at HEAD `ccreality-cutover` branch, 2026-05-11. Cross-referenced against `docs/ruvectorfindings/specs/_README.md`, `docs/futurebuild/02_CURRENT_STATE_HONEST.md`, `Cargo.toml`.

> Codebase health snapshot. Counts of source files, crates, MCP tools, RocksDB CFs, prediction heads, mutation categories, embedders, futurebuild docs. **All counts taken from live tree, not aspirational.**

---

## 1. Codebase metrics

| Metric | Count |
|---|---|
| **Total Rust source files** (`crates/**/*.rs`) | **1,618** |
| **Workspace crates** | **25** |
| **ME-JEPA-specific crates** | 10 (`mejepa*`) — corpus, instruments, cf, embedders, tct, train, shift-subscriber, hygiene + `mejepa` + `tier-compression` |
| **Foundation crates** (workspace) | 5 (`witness`, `solver`, `mincut`, `paths`, `tier-compression`) |
| **Memory + retrieval crates** | 6 (`core`, `embeddings`, `mcp`, `cli`, `storage`, `cuda`) |
| **Domain crates** | 4 (`benchmark`, `dynamicjepa`, `causal-agent`, `graph-agent`) |

---

## 2. MCP tool surface

| Surface | Count |
|---|---|
| **Total tools registered in `dispatch.rs`** | **206** |
| ME-JEPA tools (live, shipped) | 19 |
| ME-JEPA tools (futurebuild Phase A–F) | 7 (`mejepa_record_agent_feedback`, `mejepa_predict_what_if`, `mejepa_explain_prediction`, `mejepa_promote_approval`, `mejepa_daemon_status`, `mejepa_inspect_prediction`, `mejepa_pause_predictions`) |
| Reality-loop / ccreality tools (active) | ~45 (`reality_*`, `harness_*`, `optimizer_*`) |
| DynamicJEPA tools | 45 |
| Core memory + search | ~20 |
| Learning + UTL learner-state | ~30 |
| Provenance + admin | ~10 |
| Retired (fail-closed) | `reality_run_attempt`, E5 causal (4 tools), E11 entity (6 tools) |

---

## 3. RocksDB column families

| Class | Count |
|---|---|
| **Total CFs** | **~101** |
| ME-JEPA-specific CFs | 28+ (per `mejepa-cf` registry) |
| Memory / fingerprint / embedder CFs | ~25 |
| Teleological / training / learning CFs | ~20 |
| Constellation / contrastive / typed-edge | ~10 |
| Code-store CFs | 5 |
| DynamicJEPA CFs (`dj_*`) | ~20 |
| Phase 8 eval CFs | 9 |
| Other (topics, profiles, snapshots, system) | ~10 |

New CFs added 2026-05-11 (Phase A futurebuild, **shipped**): `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`.

---

## 4. ME-JEPA constants

| Constant | Value |
|---|---|
| Embedder slots (declared) | 21 (E1–E21) |
| Embedder slots (retired) | 2 (E5 causal, E11 KEPLER) |
| Embedder slots (active code-domain) | 13 (E1, E2, E3, E4, E6, E7, E8, E9, E10, E12, E13, E14, E17) |
| Embedder slots (non-code learner-state) | 4 (E15, E16, E19, E21) |
| Embedder slots (partial code-domain) | 2 (E18, E20) |
| Instrument slots | 15 |
| Total panel dimension | 5,120 |
| Mutation categories | 8 |
| Prediction heads (shipped) | 2 (Panel, Oracle) |
| Prediction heads (futurebuild Phase B) | 8 (Failure-mode, Edge-case, Tech-debt, Perf, Security, Accuracy, Cost, Reasoning) |
| Verdict variants | 5 (`Pass`, `Fail`, `OutOfDistribution`, `Abstain`, `GuardRejected`) |
| Supported languages | 11 (Rust, Python, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) |
| Routing table rows | 143 |
| Tree-sitter grammar version | 0.23.x |
| Constellation cells (max) | ~13,728 (8 × ~13 × 11 × 13) |
| `FailureModeClass` enum variants | 27 + `Other` |
| `EdgeCaseClass` enum variants | 22 + `Other` |
| `LatentBugClass` enum variants | 17 |
| `TechDebtClass` enum variants | 20 |
| `DeadCodeKind` enum variants | 10 |
| `RedundancyKind` enum variants | 6 |
| `PerfAxis` enum variants | 16 |
| `SecurityConcernClass` enum variants | 19 |
| `AccuracyMetric` enum variants | 13 |
| `CostAxis` enum variants | 10 |
| `ReasoningClass` enum variants | 8 |
| `ClaimKind` enum variants | 9 |
| `ReconciliationStatus` enum variants | 7 |
| `PredictionCorrectness` enum variants | 5 |

---

## 5. Phase status (per `docs/futurebuild/02_CURRENT_STATE_HONEST.md`)

| Phase | Status |
|---|---|
| 0 — CORPUS | partial (Python operators + 1-instance Docker FSV shipped; full 300×8 corpus run partial at 1,200/2,400 rows; non-Python operators pending) |
| 1a — AST-CHUNKING | shipped (11 languages via new `context_graph_core::memory::ast` API; legacy `AstCodeChunker` convergence pending) |
| 1b — EMBEDDERS | partial (21 declared, 13 active code-domain, 4 learner-state; routing table 143 rows FSVed; SHA-pinned digests + VRAM budget + E17 calibration cert shipped; full neural forward wrappers for all 13 active code-domain pending) |
| 1c — INSTRUMENTS | shipped (15 panel slots with deterministic encoders + Panel/PanelBuilder/PanelStore + linear-probe gate + 4 fully-instrumented slots: E_Oracle, E_Witness, E_StaticAnalysis, EmbedderBackedInstrumentHead) |
| 1d — FLYWHEEL-TYPES | shipped 2026-05-11 (PredictionId/FeedbackId/PanelId/WitnessHash/AgentId newtypes; SurpriseSeverity/FeedbackKind/FailureModeClass enums; DdaSignals/ExemplarRef/SurpriseEvent/ActualOutcome/EmbedderViolationSignature; 3 new CFs registered) |
| 2 — PREDICTOR | shipped (6-layer transformer, hidden 1024, 8 heads, VICReg loss, Oracle head, FrozenTargetAdapter, PanelSource, predictor_fsv) |
| 3 — TRAINING | shipped (Trainer + AdamW + cosine LR + BatchSampler + PatchSimilarityGraph + sampling_weight + TrainingCertificate hash chain + holdout eval + LoRA + distillation; 5 FSV examples including 100k-step cert-chain replay) |
| 3+ — UTML-FORMULA | shipped (compute_delta_p / _k / _omega / _xi / _l_step; PairwiseMiAuditor + CSV + MCP tool; DormantUnitDetector + reinit_dormant_units; state-transfer T detector) |
| 4a — INFERENCE | shipped (MeJepaCompiler::{compile, verify, observe, compile_with_panel} + conformal interval + OOD + Verdict + mejepa_verify MCP) |
| 4b — TCT-CONSTELLATION | shipped (TctConstellation + Gτ guard + Verdict::GuardRejected + Bayesian shrinkage + freshness check + per-embedder calibrator + per-chunk-type centroids + chunk-level localization gtau_check_chunks + mejepa_constellation_inspect MCP) |
| 5 — SELF-HEALING | shipped (18-module heal/ tree; mejepa-heal-drill binary; scheduler skeleton with 7 tickers + FSV; mejepa_heal_status + mejepa_rollback_to MCP) |
| 6 — HYGIENE | shipped 2026-05-11 (tier-compression + mincut foundation + context-graph-mejepa-hygiene: tier transitions, quota + emergency eviction, witness Merkle compression > 90d, nightly GC, JSON codec, MCP path discipline; 3 MCP tools) |
| 7 — SHIFT-SUBSCRIBER | shipped 2026-05-11 (restart-safe Tokio tailer, watermark CF, lag alert, panic supervision, latency telemetry, end-to-end shift→inference→observe→prediction; 3 MCP tools + mejepa_predict_latest) |
| 8 — EVALUATION | harness shipped 2026-05-11 (full eval/ with EvalRunner, ship_gate_failures, per-language conformal/OOD stratification, Bayesian shrinkage, ActiveLearningQueueState, PatchSimilarityGraph, weekly report writer, RocksDbEvalStore, 9 RocksDB CFs; 4 MCP tools; phase8_eval_fsv writes 17 evidence files) |

---

## 6. Futurebuild docs

| Class | Count |
|---|---|
| **Total `.md` docs in `docs/futurebuild/`** | **27** (00–26) |
| **mejepasystem docs (this set)** | **29** (00–29) |
| **`docs/ruvectorfindings/`** | 16 design docs + specs |
| **`docs/ccreality/`** historical | 21 (pre-pivot reference) |

---

## 7. Task DAG

| Class | Count |
|---|---|
| Total atomic tasks (futurebuild) | ~115 |
| FLYWHEEL | 7 (~5.5d) |
| PREDICT | 23 (12 + 11 LABEL, ~12d) |
| ENRICH | 7 (~13d) |
| LOOP | 7 (~1.5d) |
| HEAL | 8 (~6.5d) |
| CORPUS | 4 (~10d) |
| OBS | 16 (~12d) |
| UTML | 9 (~3.5d) |
| TESTING | 11 (~13d) |
| RUNBOOK | 12 (~5.5d) |
| DR | 8 (~4d) |
| SECURITY | 8 (~3.5d) |
| BOOTSTRAP | 3 (~2d) |
| API | 5 (~2.5d) |
| **Total fresh effort** | **~95 engineer-days** |

---

## 8. SWE-bench Lite task corpus

| Metric | Value |
|---|---|
| Total task manifests in `tasks/swebench-lite/` | **300** |
| Languages | Python (all 300) |
| Per-task fields | `task_id`, `repo`, `base_commit`, `problem_statement`, `patch`, `test_patch`, `FAIL_TO_PASS[]`, `PASS_TO_PASS[]` |
| Repo concentration (top-3) | django (38%), sympy (25.7%), astropy (others) |
| Phase 0 corpus full size | 2,400 entries (300 × 8 mutation categories) |
| Disk size (raw, full corpus) | ~5 GB |

---

## 9. Performance budgets (constitution)

| Metric | Budget |
|---|---|
| `panel_materialization_p50` | < 100 ms (cached instruments) |
| `panel_materialization_p99` | < 500 ms (cold) |
| `predictor_forward_p50` | < 50 ms / batch=64 (RTX 5090, bf16) |
| `single_prediction_latency_p50` | < 100 ms end-to-end |
| `shift_subscriber_observe_latency_p50` | < 5 s |
| `shift_subscriber_observe_latency_p99` | < 30 s |
| `vram_steady_state` | ≤ 29 GB (21 embedders + predictor + LoRA + activations + KV-cache) |
| `rocksdb_disk_total_quota` | ≤ 115 GB hard cap (`CG_MEJEPA_DISK_QUOTA_GB`) |
| `phase3_training_walltime` | < 12 hours for 10 epochs on 2,400-example corpus |
| `conformal_coverage_target` | 1-α ± 2% (default α=0.10 → 88–92%) |
| `ood_separation_target` | AUC > 0.85 |
| `phase8_ship_gate` | prediction-oracle correlation ≥ 0.95 stable across all 8 mutation categories × all supported languages on rolling-window holdout |

---

## 10. Test counts (from `cargo test --workspace`)

Per `MEMORY.md`:

| Crate / suite | Test count (at last verified state) |
|---|---|
| MCP + Storage | 1,179 |
| Workspace total | ~6,500+ |
| `cargo build --release --workspace` | zero errors, zero warnings (at last verified) |
| `cargo clippy --workspace` | clean (after Audit #15+) |

---

## 11. Schema / spec versions

| Spec | Version |
|---|---|
| Constitution | 1.0.0 (`docs/ruvectorfindings/specs/constitution.md`) |
| Implementation plan | 1.0.4 (2026-05-11) |
| Futurebuild docs | active blueprint, 2026-05-10 |
| MCP tool family | 206 tools registered |
| `RealityPrediction` schema | basic (Phase A) → rich (Phase B, futurebuild) |
| Witness chain entry size | 73 bytes (SHAKE-256 + ts + op_type) |

---

## 12. Key magic numbers / constants

| Constant | Value | Source |
|---|---|---|
| Panel total dim | 5,120 | constitution + `mejepa-instruments` |
| Active embedders (code-domain) | 13 | `mejepa-embedders::routing.rs` |
| Mutation categories | 8 | `mejepa-corpus` |
| Prediction heads (futurebuild) | 10 | `03_ARCHITECTURE_BLUEPRINT.md §3` |
| Tree-sitter grammar version | 0.23.x | constitution `<tech_stack>` |
| Witness chain entry | 73 bytes | constitution |
| HNSW M (graph degree) | 32 (dense), 16 (HDC) | `crates/context-graph-storage/src/indexes.rs` |
| HNSW efConstruction | 200 (dense), 100 (HDC) | same |
| HNSW efSearch default | 50 (dense), 30 (HDC) | same |
| MAX_VECTORS per HNSW | 2,000,000 | per AP-XX fix |
| Conformal coverage target | 0.90 | constitution |
| OOD soft threshold | 0.50 | constitution |
| OOD hard threshold | 0.85 | constitution |
| Drift soft / hard / catastrophic | 88% / 85% / 80% | constitution |
| Sampling weight decay | 0.995 (per day, half-life ~140d) | constitution |
| Operator-override sampling multiplier | 6× | `docs/futurebuild/10` |
| Agent surprise sampling multiplier | 1 + 2·severity | `docs/futurebuild/10` |
| Rolling drift window | 1,000 attempts | constitution |
| LoRA rank | 16 (default) | `lora_wiring.rs` |
| Constellation max age | 90 days | constitution |
| Disk quota hard | 115 GB | constitution |
| Disk quota warn | 100 GB | hygiene |
| Disk quota emergency | 110 GB | hygiene |
| Subscriber lag alert | > 500 / 1 min | constitution |
| Scheduler flap threshold | > 5 restarts / 5 min | daemon |
| Agent feedback rate limit | 100 / hour / agent_id | `08_LIVE_PREDICTION_AGENT_API.md` |
| Prediction-injection cap | 4 KB | turn-79 decision |
| Predictor forward batch | 64 | constitution |
| AdamW LR | 5e-4 | constitution |
| AdamW weight decay | 0.04 | constitution |

---

## 13. Cross-doc consistency check

All 29 mejepasystem docs cite the following source-of-truth pointers consistently:

| SoT | Cited in |
|---|---|
| `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` | 00, 01, 17 |
| `docs/futurebuild/02_CURRENT_STATE_HONEST.md` | 00, 01, 21, 29 |
| `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md` | 00, 01, 09, 13 |
| `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` | 08, 14 |
| `docs/futurebuild/05_EMBEDDER_SIGNAL_MAP.md` | 06, 08 |
| `docs/futurebuild/06_REALITY_LOOP_REFACTOR.md` | 20, 21 |
| `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md` | 13, 18 |
| `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md` | 11, 17, 18, 24, 25 |
| `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md` | 12 |
| `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md` | 03, 09, 10 |
| `docs/futurebuild/11_DELETION_LIST_INNER_LOOP.md` | 21 |
| `docs/futurebuild/12_IMPLEMENTATION_PHASES.md` | 00, 01, 03, 21 |
| `docs/futurebuild/13_TASK_INDEX.md` | 00, 04–28 |
| `docs/futurebuild/14_NEW_LOOP_MD.md` | 21 |
| `docs/futurebuild/15_PREDICTION_TAXONOMY.md` | 11, 17 |
| `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md` | 03, 04, 13, 22, 23 |
| `docs/futurebuild/17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md` | 28 |
| `docs/futurebuild/18_INTEGRATION_AND_E2E_TESTING.md` | 26 |
| `docs/futurebuild/19_OPERATOR_RUNBOOK.md` | 19, 22 |
| `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md` | 23 |
| `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` | 24 |
| `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md` | 24, 25 |
| `docs/futurebuild/24_API_STABILITY.md` | 27 |
| `docs/futurebuild/25_FUTURE_DOMAINS_MEJEPA_X.md` | 28 |
| `docs/ruvectorfindings/specs/constitution.md` | 01, 24, 26 |
| `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` | 09, 10 |
| `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md` | 06 |
| `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md` | 08 |
| `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md` | 05, 08 |
| `CLAUDE.md` | 01, 03, 11, 20, 21, 24 |

---

## 14. Verifiable bottom line

Per `docs/futurebuild/02_CURRENT_STATE_HONEST.md §6`:

If you `cargo build --release --workspace` from a fresh clone of `ccreality-cutover` branch:

- ✅ **Builds**. Workspace compiles.
- ✅ **Tests**. `cargo test --workspace` passes (~6.5k+ tests on the consolidated tree).
- ✅ **MCP starts**. `cargo run --release -p context-graph-mcp` boots, exposes 19 ME-JEPA tools (+ 187 other tools = 206 total).
- ✅ **Daemon mode boots**. `--daemon-server-only --d-root /mnt/d/contextgraph` starts long-running supervised tasks when D-root validation passes.
- ✅ **`mejepa_verify` answers**. Returns `Verdict` (likely `OutOfDistribution` because no production training has run).
- ✅ **`mejepa_predict_latest` answers**. Returns deterministic placeholder or real predicted panel.

What does NOT happen out of the box:

- ❌ Predictions are NOT *automatically* injected into Claude Code's prompts (no UserPromptSubmit-from-MCP-state path yet — `TASK-PREDICT-001` futurebuild Phase A).
- ❌ Shift subscriber + self-optim scheduler start under daemon mode (code shipped; operator-verified first-time install proof pending).
- ❌ 300×8 corpus not durable on D-drive (`/tmp` run lost after reboot; rerun in progress at `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`; 1,200/2,400 rows complete; blocked on `source_patch_support.rs` timeout fix).
- ❌ Ship gate ≥ 0.95 not met (predictor not yet trained on full corpus).

The futurebuild plan (Phases A–H) turns the ❌ list into ✅.

---

## 15. End

This concludes the 29-document mejepasystem reference. For source-of-truth status updates: read `docs/futurebuild/02_CURRENT_STATE_HONEST.md` + `docs/ruvectorfindings/specs/_README.md` + this doc's §5 + §14.

Next refresh: when futurebuild Phase A ships (estimated 5–7 engineer-days).
