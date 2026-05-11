# ME-JEPA-Code Specs — Master Index

> **Spec hierarchy authority** per `/home/cabdru/contextgraph/docs2/prdtospec.md`:
> Level 1 Constitution → Level 2 Functional Specs → Level 3 Technical Specs → Level 4 Task Specs → Level 5 Context (active session memory).
>
> Source-of-truth design docs live at `/home/cabdru/contextgraph/docs/ruvectorfindings/{09,10,11,12,13,14}*.md`. The specs in this directory are the **executable blueprint** for implementing those design docs end-to-end.

**Date:** 2026-05-11 (status refresh; spec body authored 2026-05-09)
**Spec version:** 1.0.3
**Project:** ME-JEPA-Code (the Reality Compiler)
**Mission:** Make `prediction_oracle_correlation ≥ 0.95` stably across all 8 mutation categories AND all supported languages → human code review of AI-generated patches becomes statistically unjustified.

> **2026-05-11 hardening snapshot.** Phases 1-5 evidence ledger: `../15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`. **Phases 6/7/8 + UTML carriers (REQ-UTML-06/07/08) evidence ledger: `../16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`.** Phase 1a operational via the new `context_graph_core::memory::ast` 11-language chunking API (legacy `AstCodeChunker` type still Rust-only — converge/deprecate is the remaining work). Phase 1b verified for real model artifact integrity, active/retired policy, VRAM budget, learner-state calibration, and AST routing coverage; actual neural forward wrappers remain the critical deep-integration item and must not be implied by digest/VRAM evidence alone. Phases 1c, 2, 3, 4a, 4b, 5, 6, 7, and 8 are SHIPPED with FSV evidence. UTML carriers REQ-UTML-06 (pairwise MI auditor + CSV writer), REQ-UTML-07 (continual-backprop dormant-unit detector + re-init), and REQ-UTML-08 (state-transfer T detector) all SHIPPED in `crates/context-graph-mejepa-train/src/learning_signal/`. `compute_delta_xi` fails closed on target-collapse leaks with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. **Live ME-JEPA MCP surface is now 18 tools**: `mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`, `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`, `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`. New crates added since doc 15: `context-graph-mejepa-hygiene`, `context-graph-mejepa-shift-subscriber`, `context-graph-mejepa-cf` (shared CF registry). **Active forward edge is now operational/integration, not greenfield code**: full 300×8 SWE-bench Lite corpus run, neural per-embedder forward wrappers (E1/E7/E8/E10/E12/E13/E14), analyzer/embedder-backed semantic instrument heads, chunk-level TCT, per-language mutation operators (10 languages beyond Python), live conformal/OOD/p50/p99 capture under production traffic, rolling-window holdout against the ≥0.95 ship gate, `mejepa_audit_pairwise_mi` MCP wrapper, and continual-backprop intervention hook in `heal/regulate.rs`.

---

## Reading order

1. **`constitution.md`** — immutable rules. Read first; never violate.
2. **`implementation-plan.md`** — phase-ordered execution roadmap referencing every spec by ID.
3. **`functional/_index.md`** — Level 2 functional specs (what to build), one per domain.
4. **`technical/_index.md`** — Level 3 technical specs (how to build), one per functional spec.
5. **`tasks/_index.md`** — Level 4 task graph (atomic, ordered, sequential implementation units).
6. **`tasks/_traceability.md`** — coverage matrix proving every spec item has a task.

---

## Domain map (13 functional+technical pairs)

Each domain has one functional spec (SPEC-`<DOMAIN>`), one technical spec (TECH-`<DOMAIN>`), and one or more task specs (TASK-`<DOMAIN>`-NNN). Domains are phase-aligned per `09_END_GOAL_REVIEW_REPLACEMENT.md §7` and the post-pivot doc 10 §F priorities.

| # | Domain | Phase | Effort | Status (post-`sitrep/` removal in commit `6f72a02e`) |
|---|---|---|---|---|
| 1 | **CORPUS** — Phase 0 mutation corpus + Docker oracle bridge + RocksDB CFs + train/calib/holdout split | 0 | ~5-7 days | partial: 8 Python mutation operators ✅, repo/instance-atomic split ✅, Python lex ✅, Rust/Python/JS/TS/Go/Java/C/C++/C#/Ruby parser harness ✅, one-task SWE-bench bridge ✅ + batch oracle + shard merge ✅, source-site provenance ✅, `mejepa-corpus generate/verify/stats/prepare-images/cleanup` CLI ✅, Docker pull preflight + WSL→D migrator ✅, leakage detector ✅; full 300-task × 8-category batch run still pending (operational, not code) and per-language mutation operators (10 langs) still pending |
| 2 | **AST-CHUNKING** — Multi-language AST-perfect atomic chunker (11 languages) | 1 (item 6) | ~10 days | **operational / hardened**: `context_graph_core::memory::ast` ships 11-language tree-sitter chunking with typed `Language`/`EntityType`, SHA-256 chunk IDs, parse diagnostics, routing sets, and fail-closed errors; `ast_chunker_multilang_fsv` passed for all 11 languages. Compatibility caveat: legacy `crates/context-graph-core/src/memory/ast_chunker.rs::AstCodeChunker` is still Rust-only and should be converged onto the new API or deprecated. |
| 3 | **EMBEDDERS** — `Instrument` trait wrapper around embedder ensemble + per-chunk-type routing table | 1 | ~5 days | **control surface hardened / deep forward pending**: 21 declared enum slots, E5/E11 retired, 19 active registrations required, 12 active content embedders, SHA-256 digest verification, VRAM budget gate, E17 calibration cert, and 143-row routing wrapper over the core AST table all FSVed in `crates/context-graph-mejepa-embedders/`. Per-embedder neural `EmbedderForward` / `Instrument` wrappers that execute actual model forward passes remain pending and must be treated as a critical Phase 1b blindspot. |
| 4 | **INSTRUMENTS** — 14 instruments (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning) + scalar slot + Panel/PanelBuilder | 1 | ~25 days incl. embedder reuse | **shipped scaffold / semantic heads pending**: all 15 panel slots have deterministic typed encoders ✅, Panel + PanelBuilder ✅, PanelStore + JSON envelope + frozen-hook + linear-probe quality gate ✅, all-slot FSV ✅. E_Oracle, E_Witness, and E_StaticAnalysis are first-class today; analyzer/embedder-backed replacements for the deterministic code/text/runtime/reasoning heads remain capability work. |
| 5 | **PREDICTOR** — 6-layer transformer + frozen target readback + VICReg loss + Oracle head | 2 | ~7-10 days | **shipped**: `MeJepaPredictor` (6-layer transformer, hidden 1024, 8 heads), `VICRegLoss`, `OracleHead`, `FrozenTargetAdapter`, `PanelSource`, `predictor_fsv` example all in `crates/context-graph-mejepa/`. CUDA-only (RTX 5090 sm_120). Forward-pass FSV ✅; latency / VRAM measurements pending live capture |
| 6 | **TRAINING** — AdamW + cosine decay + UTML L_step + per-step training certificate + sampler | 3 | ~5-7 days | **shipped / hardened**: full `Trainer` + AdamW + cosine LR + `BatchSampler` (with `PatchSimilarityGraph` + `sampling_weight`) + `TrainingCertificate` hash chain + holdout eval + `LoRAWiring` + distillation in `crates/context-graph-mejepa-train/`. FSV examples passed, including 100,000-step cert-chain replay, holdout eval, and edge cases. `train_full_fsv` now byte-verifies evidence file readback. |
| 7 | **UTML-FORMULA** — ΔP / ΔK / ΔΩ / ΔΞ computation + sampling reweighting + consolidation integral M(t) | 3 (cross-cutting) | ~5 days | **shipped / safety-hardened**: `learning_signal::{compute_delta_p, compute_delta_k, compute_delta_omega, compute_delta_xi, compute_l_step}` ✅ shipped. `compute_delta_xi` implements the multiplicative non-collapse formula and fails closed on any non-zero/non-finite target collapse with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. Sampling reweighting ✅ in `BatchSampler`. **REQ-UTML-06 pairwise MI audit ✅ shipped** (`learning_signal/pairwise_mi_matrix.rs::PairwiseMiAuditor`). **REQ-UTML-07 continual-backprop dormant-unit re-init ✅ shipped** (`learning_signal/continual_backprop.rs::{DormantUnitDetector, reinit_dormant_units}`). **REQ-UTML-08 state-transfer T detector ✅ shipped** (`learning_signal/state_transfer.rs::compute_state_transfer`). Synthetic FSV envelope (REQ-UTML-09) covered by `train_edge_cases_fsv` + `utml_l_step_fsv`. Remaining work: reconcile simplified ΔP/ΔK/ΔΩ carriers with the full doc-11 formulas; expose `mejepa_audit_pairwise_mi` MCP wrapper; trigger continual-backprop re-init from `heal/regulate.rs`. |
| 8 | **INFERENCE** — `MeJepaCompiler::compile()` and `::verify()` API + conformal calibration + OOD detection | 4 | ~3-5 days | **shipped**: `MeJepaCompiler::{compile, compile_with_panel, verify, observe}` + conformal interval + OOD detection + `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}` in `crates/context-graph-mejepa/`. MCP tool `mcp__cgreality__mejepa_verify` ✅ live (with deterministic-harness opt-in test). `inference_fsv` example ✅. Conformal coverage / OOD AUC live measurements pending Phase 8 |
| 9 | **TCT-CONSTELLATION** — Per-(category × entity × language × embedder) centroid construction + Gτ guard + GuardRejected verdict | 4 (cross-cutting) | ~5 days | **shipped at panel level**: full `TctConstellation` + builder + `Gτ` guard + `Verdict::GuardRejected` + Bayesian shrinkage + freshness check + per-embedder threshold calibrator + `panel_slots`, `hinge`, `rate_aggregator` in `crates/context-graph-mejepa-tct/`. MCP tool `mcp__cgreality__mejepa_constellation_inspect` ✅ live. `tct_fsv` example ✅. Per-chunk-type centroids and chunk-level Gτ are pending integration with the now-shipped `memory::ast` chunk IDs. |
| 10 | **SELF-HEALING** — Online observe() + EWC++ + drift detection + A/B/C promotion + continual backprop | 5 | ~5-7 days | **shipped**: 16-module `heal/` tree (`drift`, `ewc`, `promote`, `full_retrain`, `integrity`, `lora_refresh`, `calibration`, `distill`, `regulate`, `pipeline`, `drill`, `store`, `cf`, `fsv`) in `crates/context-graph-mejepa/`. `mejepa-heal-drill` binary ✅ with happy/soft/hard/catastrophic injection. RocksDB CFs `CF_MEJEPA_FISHER_SNAPSHOTS`, `CF_MEJEPA_DRIFT_WINDOW`, `CF_MEJEPA_DRIFT_HISTORY`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DISTILL_STEPS`, `CF_MEJEPA_ACTIVE_POINTERS` ✅ persisted. MCP tools `mcp__cgreality__mejepa_heal_status` + `mejepa_rollback_to` ✅ live |
| 11 | **HYGIENE** — Tier compression integration + Stoer-Wagner mincut splits + Merkle compression + nightly GC + quota enforcement | 6 | ~3-5 days | **shipped 2026-05-11**: foundation crates `context-graph-tier-compression` ✅ + `context-graph-mincut` ✅. New `context-graph-mejepa-hygiene` crate ✅ ships tier transitions (`tier_demote/promote`), quota + emergency eviction (`quota_check_and_evict`), witness-chain Merkle compression of >90d segments (`witness_compress_old_segments`), nightly GC orchestrator (`gc_run_nightly`), and JSON codec with MCP path discipline. 3 MCP tools live: `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`. 3 FSV examples (`gc_stress_fsv`, `merkle_compression_fsv`, `quota_overflow_fsv`). REQ-HYG-03..07 satisfied. Remaining: live nightly GC scheduler under cron + 115 GB hard quota in production. See doc 16 §1. |
| 12 | **SHIFT-SUBSCRIBER** — Tokio task tailing cgreality-shift-log → mejepa.observe() + watermark CF + new MCP tools | 7 | ~5-7 days | **shipped 2026-05-11**: new `context-graph-mejepa-shift-subscriber` crate ✅ ships restart-safe Tokio tailer (`ShiftSubscriber::{run_until_idle, run_until_shutdown}`), watermark CF reads/writes (`WatermarkWriter`), per-shift instrument cache, lag alert, panic supervision, latency telemetry (`SubscriberMetrics`), and end-to-end shift→`shift_to_inference`→observe→prediction wiring. 3 MCP tools live: `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit` (plus the previously-shipped `mejepa_predict_latest`). FSV: `shift_subscriber_fsv` + 2 in-crate `tokio::test` round-trips. REQ-SHIFT-01..09 satisfied at code level. Remaining: record real production p50/p99 distribution under traffic against the REQ-SHIFT-03 envelope. See doc 16 §2. |
| 13 | **EVALUATION** — Holdout split evaluation + per-language stratification + ≥0.95 ship gate + active learning loop | 8 | ongoing | **harness shipped 2026-05-11**: full `crates/context-graph-mejepa/src/eval/` module ✅ with `EvalRunner`, `EvalReport`, `ship_gate_failures`, per-language conformal/OOD stratification, Bayesian shrinkage, `ActiveLearningQueueState`, `PatchSimilarityGraph`, weekly report writer, `RocksDbEvalStore`, 9 RocksDB CFs. 4 MCP tools live: `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`. FSV `phase8_eval_fsv` writes 17 evidence files. REQ-EVAL-01..07 satisfied at code level. Remaining: rolling-window holdout against the real 300×8 SWE-bench Lite corpus over enough windows to ship a stable ≥0.95 — production data, not greenfield code. See doc 16 §3. |

**Total estimated effort:** ~85 days of fresh implementation + ~10 days for the embedder integration shortcut + ongoing Phase 8.

---

## Spec ID conventions

Per `prdtospec.md §2.1`, every requirement uses `[REQ-<DOMAIN>-NN]` for traceability.

| Prefix | Meaning |
|---|---|
| `REQ-CORPUS-NN` | Phase 0 corpus requirements |
| `REQ-AST-NN` | AST chunking requirements |
| `REQ-EMBED-NN` | Embedder ensemble integration requirements |
| `REQ-INSTR-NN` | Instrument requirements |
| `REQ-PRED-NN` | Predictor requirements |
| `REQ-TRAIN-NN` | Training loop requirements |
| `REQ-UTML-NN` | UTML formula requirements |
| `REQ-INFER-NN` | Inference / verify() requirements |
| `REQ-TCT-NN` | Constellation / Gτ guard requirements |
| `REQ-HEAL-NN` | Self-healing requirements |
| `REQ-HYG-NN` | Storage hygiene requirements |
| `REQ-SHIFT-NN` | Shift subscriber + MCP tool requirements |
| `REQ-EVAL-NN` | Evaluation requirements |

| Prefix | Meaning |
|---|---|
| `TASK-CORPUS-NNN` | Phase 0 corpus tasks (sequence numbers) |
| `TASK-AST-NNN` | AST chunking tasks |
| `TASK-EMBED-NNN` | Embedder integration tasks |
| `TASK-INSTR-NNN` | Instrument tasks |
| `TASK-PRED-NNN` | Predictor tasks |
| `TASK-TRAIN-NNN` | Training loop tasks |
| `TASK-UTML-NNN` | UTML computation tasks |
| `TASK-INFER-NNN` | Inference tasks |
| `TASK-TCT-NNN` | TCT constellation tasks |
| `TASK-HEAL-NNN` | Self-healing tasks |
| `TASK-HYG-NNN` | Storage hygiene tasks |
| `TASK-SHIFT-NNN` | Shift subscriber tasks |
| `TASK-EVAL-NNN` | Evaluation tasks |

---

## Quality gates

Per `prdtospec.md §6.2`, four gates protect the spec → implementation pipeline:

1. **SPEC_REVIEW** (before task generation) — all functional+technical specs reviewed for ambiguity, completeness, traceability.
2. **TASK_REVIEW** (after task generation) — task graph acyclic, layer-ordered (foundation → logic → surface), atomic, definitions of done with exact signatures.
3. **DESIGN_REVIEW** (before implementation) — pseudo-code matches tech spec, follows constitution.
4. **CODE_REVIEW** (after task completion) — code follows constitution, all tests pass, signatures match definition_of_done.

---

## Cross-references

- **Master plan:** `../09_END_GOAL_REVIEW_REPLACEMENT.md`
- **Post-pivot architecture:** `../10_MEJEPA_PIVOT_2026_05_09.md`
- **UTML v1.0 learning formula:** `../11_MEJEPA_LEARNING_FORMULA.md`
- **21-embedder ensemble:** `../12_EMBEDDER_INTEGRATION.md`
- **TCT framework:** `../13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- **Multi-language AST chunking:** `../14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- **Capability claim synthesis:** `../09_END_GOAL_REVIEW_REPLACEMENT.md` + `../10_MEJEPA_PIVOT_2026_05_09.md` + `../13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- **Operator role:** `/home/cabdru/contextgraph/CLAUDE.md §11`
- **SMAP shared mind protocol:** `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md`
- **Spec authoring framework:** `/home/cabdru/contextgraph/docs2/prdtospec.md`
