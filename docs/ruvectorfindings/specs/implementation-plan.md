# ME-JEPA-Code Implementation Plan

> **Phase-ordered execution roadmap.** Translates the 8-phase build plan in `09_END_GOAL_REVIEW_REPLACEMENT.md §7` (re-framed by `10_MEJEPA_PIVOT_2026_05_09.md`) into a concrete sequence of spec-IDs and task-IDs. **Every entry references a Level 2 functional spec, a Level 3 technical spec, and a Level 4 task list.**

**Date:** 2026-05-11 (status refresh; spec body authored 2026-05-09)
**Spec version:** 1.0.4
**Authority:** `constitution.md §spec_authority_order` and `09_END_GOAL_REVIEW_REPLACEMENT.md §7` Phase plan.

> **2026-05-11 hardening snapshot.** Phases 1-5 evidence: `../15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`. **Phases 6/7/8 + REQ-UTML-06/07/08 evidence: `../16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`.** Phase 1a operational via the 11-language `context_graph_core::memory::ast` API; legacy `AstCodeChunker` compatibility/deprecation work is tracked in [#6](https://github.com/ChrisRoyse/contextgraph/issues/6), and the reformat-invariant chunk-ID audit is tracked in [#12](https://github.com/ChrisRoyse/contextgraph/issues/12). Phase 1b hardened for model artifact integrity, active/retired policy, VRAM, calibration, and routing; **neural per-embedder forward wrappers remain the critical deep-integration item** ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)). Phases 1c, 2, 3, 4a, 4b, 5, **6, 7, and 8 are SHIPPED end-to-end** with FSV evidence and MCP wrappers. UTML carriers REQ-UTML-06 (pairwise MI + MCP CSV/readback), REQ-UTML-07 (continual-backprop dormant-unit re-init + `heal/regulate.rs` hook), and REQ-UTML-08 (state-transfer T detector) are SHIPPED. `compute_delta_xi` fails closed on target-collapse leaks with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. Live ME-JEPA MCP surface is **19 tools** (8 from doc 15 + 3 hygiene + 3 shift-subscriber + 4 evaluation + `mejepa_audit_pairwise_mi`). New crates added since doc 15: `context-graph-mejepa-hygiene`, `context-graph-mejepa-shift-subscriber`, `context-graph-mejepa-cf` (shared CF registry). The active forward edge is now **tracked operational/integration work**: full 300×8 SWE-bench Lite corpus run ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5)), neural per-embedder forward wrappers ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)), analyzer/embedder-backed semantic instrument heads ([#4](https://github.com/ChrisRoyse/contextgraph/issues/4)), production chunk-level TCT centroid/UX evidence ([#9](https://github.com/ChrisRoyse/contextgraph/issues/9)), per-language mutation operators (10 languages beyond Python) ([#7](https://github.com/ChrisRoyse/contextgraph/issues/7)), live conformal / OOD / shift-subscriber p50/p99 capture under production traffic ([#11](https://github.com/ChrisRoyse/contextgraph/issues/11)), WSL GPU telemetry hardening ([#10](https://github.com/ChrisRoyse/contextgraph/issues/10)), rolling-window holdout against the ≥0.95 ship gate ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5)), supervised production scheduling ([#8](https://github.com/ChrisRoyse/contextgraph/issues/8)), and full UTML formula-carrier reconciliation ([#13](https://github.com/ChrisRoyse/contextgraph/issues/13)).

---

## Phase summary

| Phase | Domain(s) | Effort | Parallelizable? | Status (2026-05-10) |
|---|---|---|---|---|
| 0 | CORPUS | ~5-7 days | partially (mutation operators are independent) | partial — operators+split+oracle bridge+CLI shipped; full 300×8 batch run pending ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5)); non-Python operators pending ([#7](https://github.com/ChrisRoyse/contextgraph/issues/7)) |
| 1a | AST-CHUNKING | ~10 days | yes (per-language extension is parallel) | **operational / hardened** — new `context_graph_core::memory::ast` API chunks all 11 languages with FSV; legacy `AstCodeChunker` is still Rust-only and should converge to or be replaced by the new API ([#6](https://github.com/ChrisRoyse/contextgraph/issues/6)); reformat-invariant chunk-ID audit pending ([#12](https://github.com/ChrisRoyse/contextgraph/issues/12)) |
| 1b | EMBEDDERS | ~5 days | yes (per-embedder Instrument trait wrappers are parallel) | **control surface hardened / deep forward pending** — 21 declared slots, 19 active registrations, E5/E11 retired, digest verification, VRAM gate, E17 calibration, and 143-row routing FSV shipped; actual neural `EmbedderForward`/`Instrument` wrappers remain pending ([#3](https://github.com/ChrisRoyse/contextgraph/issues/3)) |
| 1c | INSTRUMENTS | ~15 days | partially (instruments depend on AST + embedders + chunker; each instrument is then independent) | **shipped scaffold / semantic heads pending** — all 15 panel slots have deterministic encoders + Panel/PanelBuilder/PanelStore + linear-probe gate shipped; embedder/analyzer-backed replacements remain capability work ([#4](https://github.com/ChrisRoyse/contextgraph/issues/4)) |
| 2 | PREDICTOR | ~7-10 days | no (sequential: panel → predictor → frozen target → VICReg → Oracle head) | **shipped** — `MeJepaPredictor`, `VICRegLoss`, `OracleHead`, `FrozenTargetAdapter` in `crates/context-graph-mejepa/`; `predictor_fsv` example |
| 3 | TRAINING | ~5-7 days | yes (training loop scaffolding parallels training-cert + sampler) | **shipped** — `Trainer` + AdamW + cosine LR + `BatchSampler` + `TrainingCertificate` chain + holdout eval + LoRA wiring + distillation in `crates/context-graph-mejepa-train/`; 5 FSV examples |
| 3+ | UTML-FORMULA | ~5 days | yes (ΔP, ΔK, ΔΩ, ΔΞ are independent computations) | **shipped / safety-hardened** — 4 deltas + L_step computation + sampling reweighting shipped in `learning_signal.rs`; `compute_delta_xi` enforces `target_collapse == 0.0` and multiplicative non-collapse; **REQ-UTML-06 pairwise MI ✅ shipped** (`learning_signal/pairwise_mi_matrix.rs`, train-cert pairwise-MI columns, and `mcp__cgreality__mejepa_audit_pairwise_mi` CSV/readback MCP surface); **REQ-UTML-07 continual-backprop ✅ shipped** (`learning_signal/continual_backprop.rs` plus `heal/regulate.rs` dormant-unit re-init and `CF_MEJEPA_WEIGHT_BLOBS` readback); **REQ-UTML-08 state-transfer T ✅ shipped** (`learning_signal/state_transfer.rs`); remaining semantic/operational work is full doc-11 formula-carrier reconciliation ([#13](https://github.com/ChrisRoyse/contextgraph/issues/13)) and production trend evidence ([#11](https://github.com/ChrisRoyse/contextgraph/issues/11), [#5](https://github.com/ChrisRoyse/contextgraph/issues/5)) |
| 4a | INFERENCE | ~3-5 days | no (sequential: verify() → conformal → OOD) | **shipped** — `MeJepaCompiler::{compile, verify, observe}` + conformal + OOD + `Verdict` enum; `mcp__cgreality__mejepa_verify` MCP tool live; `inference_fsv` example |
| 4b | TCT-CONSTELLATION | ~5 days | yes (centroid construction parallels Gτ guard + GuardRejected verdict) | **shipped at code/FSV level** — full `TctConstellation` + `Gτ` + `Verdict::GuardRejected` + Bayesian shrinkage + freshness check + threshold calibrator + per-chunk-type centroids + `gtau_check_chunks` localization in `crates/context-graph-mejepa-tct/`; `mcp__cgreality__mejepa_constellation_inspect` MCP tool live. Production centroid refresh/operator UX evidence tracked in [#9](https://github.com/ChrisRoyse/contextgraph/issues/9) |
| 5 | SELF-HEALING | ~5-7 days | partially (drift detection parallels EWC++; A/B/C promotion is sequential) | **shipped** — 16-module `heal/` tree + `mejepa-heal-drill` binary + 6 RocksDB CFs + `mcp__cgreality__mejepa_heal_status` and `mejepa_rollback_to` MCP tools live |
| 6 | HYGIENE | ~3-5 days | yes (tier integration parallels Merkle compression + GC scheduler) | **shipped 2026-05-11** — tier-compression + mincut foundation crates ✅. New `context-graph-mejepa-hygiene` crate ✅: tier transitions, quota + emergency eviction, witness Merkle compression of >90d segments, nightly GC orchestrator, JSON codec, MCP path discipline. 3 MCP tools live (`mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`). 3 FSV examples. REQ-HYG-03..07 satisfied. Live cron-driven nightly GC + 115 GB hard quota in production are operational follow-ups ([#8](https://github.com/ChrisRoyse/contextgraph/issues/8)) |
| 7 | SHIFT-SUBSCRIBER | ~5-7 days | yes (subscriber Tokio task parallels new MCP tools) | **shipped 2026-05-11** — new `context-graph-mejepa-shift-subscriber` crate ✅: restart-safe Tokio tailer (`run_until_idle`/`run_until_shutdown`), watermark CF, lag alert, panic supervision, latency telemetry, end-to-end shift→`shift_to_inference`→observe→prediction. 3 MCP tools live (`mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`) plus the previously-shipped `mejepa_predict_latest`. FSV: `shift_subscriber_fsv` + 2 in-crate `tokio::test` round-trips. REQ-SHIFT-01..09 satisfied at code level. Live p50/p99 capture under production traffic is the operational follow-up ([#11](https://github.com/ChrisRoyse/contextgraph/issues/11)) |
| 8 | EVALUATION | ongoing | yes (per-category eval parallels per-language eval + active-learning) | **harness shipped 2026-05-11** — full `crates/context-graph-mejepa/src/eval/` ✅: `EvalRunner`, ship-gate failure analysis, per-language conformal/OOD stratification, Bayesian shrinkage, ActiveLearning queue, PatchSimilarityGraph, weekly report writer, RocksDbEvalStore. 4 MCP tools live (`mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`). FSV `phase8_eval_fsv` writes 17 evidence files. REQ-EVAL-01..07 satisfied at code level. Rolling-window holdout against the real 300×8 SWE-bench Lite corpus over enough windows to ship a stable ≥0.95 is the operational follow-up ([#5](https://github.com/ChrisRoyse/contextgraph/issues/5), [#11](https://github.com/ChrisRoyse/contextgraph/issues/11)) |

**Total fresh effort estimate:** ~75-95 days. Embedder reuse and chunker extension shorten Phase 1 from "build 14 instruments from scratch" to "build 14 instruments as thin heads on existing embedders" per doc 12 §3.

---

## Critical path

The longest dependency chain — bottlenecks if not built in this order:

```
Phase 0 corpus generator (CORPUS)
   ↓ produces (panel[t=0..2], oracle_verdict) tuples
Phase 1a AST chunker (AST-CHUNKING)
   ↓ produces AST-perfect chunks for all 11 languages
Phase 1b Embedder registry/routing (EMBEDDERS)
   ↓ validates real artifacts, retired-slot policy, VRAM budget, and chunk routing
Phase 1c Instruments (INSTRUMENTS)
   ↓ produces 14 instruments + Panel materializer
Phase 2 Predictor + frozen target (PREDICTOR)
   ↓ produces predicted_panel[t=2]
Phase 3 Training loop + UTML L_step (TRAINING + UTML-FORMULA)
   ↓ trained predictor weights (theta_*)
Phase 4a Inference + conformal + OOD (INFERENCE)
Phase 4b TCT constellation + Gτ (TCT-CONSTELLATION)
   ↓ deterministic geometric verifier
Phase 7 Shift subscriber + MCP tools (SHIFT-SUBSCRIBER)
   ↓ real-time pipeline closing reality → predict → observe loop
Phase 8 Evaluation against ≥0.95 (EVALUATION)
```

Phase 5 (self-healing) and Phase 6 (hygiene) are operational gates that run AFTER phases 1-4 ship, and are executed in parallel with Phase 7 shift-subscriber work.

---

## Per-phase deliverable table

### Phase 0 — Corpus + Docker oracle (CORPUS)

| Deliverable | Spec ref | Status |
|---|---|---|
| 8 mutation operators (KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError) | REQ-CORPUS-01..08 | ✅ shipped (Python; Rust/JS/TS/Go/Java/C/C++/C#/Ruby/PHP extension is in scope) |
| Repo-atomic train/calib/holdout split via Stoer-Wagner mincut | REQ-CORPUS-09 | ✅ shipped (split helper + InstanceAtomic default) |
| Docker oracle bridge wrapping `swebench.harness.run_evaluation` | REQ-CORPUS-10 | ✅ shipped (one-task bridge); batch scaling deferred |
| `cargo run --release -p context-graph-mejepa-corpus -- generate` produces 2,400 entries | REQ-CORPUS-11 | partial (CLI shipped and FSVed on one SWE-bench Lite instance x two categories; full 300x8 batch pending) |
| RocksDB CFs: CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS | REQ-CORPUS-12 | ✅ shipped for corpus CLI; full population deferred until the 300x8 run |
| Per-language mutation operators (extend Python operators to 10 other languages) | REQ-CORPUS-13 | deferred |
| FSV harness: `mejepa_corpus_fsv` + `split_fsv` | REQ-CORPUS-14 | ✅ shipped |

### Phase 1a — Multi-language AST chunker (AST-CHUNKING)

| Deliverable | Spec ref | Status |
|---|---|---|
| Refactor `EntityType` to language-tagged enum | REQ-AST-01 | ✅ shipped in `context_graph_core::memory::ast` as typed `Language` + `EntityType`; old `AstCodeChunker` compatibility path remains Rust-only |
| Per-language node-type classifier reusing DynamicJEPA logic (Python, JS/TS, Go, Java, C/C++, C#, Ruby, PHP) | REQ-AST-02..09 | ✅ shipped operationally for all 11 languages via `memory::ast::{classify, chunk_with_options}`; FSVed with real compiler/linter toolchain checks |
| Cross-language test harness: chunk every fixture; assert AST entity boundaries | REQ-AST-10 | ✅ shipped via `cargo run -p context-graph-core --example ast_chunker_multilang_fsv` |
| FSV: SHA-pinned chunk boundaries persisted per fixture | REQ-AST-11 | ✅ shipped; evidence at `/tmp/contextgraph-ast-chunker-multilang-fsv/aggregate-evidence.json` and per-language dirs |
| Mixed-language refusal (HTML+JS, PHP+HTML) with MEJEPA_CHUNKER_MIXED_LANGUAGE | REQ-AST-12 | ✅ shipped; PHP/HTML mixed-language edge case readback passes |
| Reformat-invariant chunk SHAs (computed on AST node IDs, not raw text) | REQ-AST-13 | partial — chunk SHA is persisted and verified today; a separate reformat-invariance audit over equivalent AST node IDs is still needed |

### Phase 1b — Embedder Instrument trait wrappers (EMBEDDERS)

| Deliverable | Spec ref | Status |
|---|---|---|
| `Instrument` trait + `InstrumentSlot` enum (15 slots summing to 5,120-d panel) | REQ-EMBED-01 | ✅ shipped (Phase 1 scaffolding) |
| Per-embedder Instrument-trait wrapper (E1, E7, E8, E10, E12, E13, E14 active content set; E5/E11 retired) | REQ-EMBED-02..08 | pending deep integration — real model files are digest-verified, but actual neural forward wrappers are not complete |
| Per-chunk-type embedder routing table (function → E7+E14, class → active replacements, comment → E1+E10, etc.) | REQ-EMBED-09 | ✅ shipped for active embedders; 143 routing rows FSVed; all E5/E11 routes fail closed or are rewritten |
| Lazy-load policy for E15-E21 learner-state embedders (VRAM headroom) | REQ-EMBED-10 | ✅ shipped for explicit learner-state loads; E17 requires calibration cert before scoring |
| FSV: `embedder_routing_fsv` writes per-chunk-type route table + asserts all 11 languages × 13 entity types covered | REQ-EMBED-11 | ✅ shipped via `embedders_fsv`; evidence at `/mnt/d/contextgraph/fsv/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json` |

### Phase 1c — 14 instruments + Panel/PanelBuilder (INSTRUMENTS)

| Deliverable | Spec ref | Status |
|---|---|---|
| `E_Oracle` instrument (128-d) | REQ-INSTR-01 | ✅ shipped |
| `E_Witness` instrument (256-d, sequence transformer over 73-byte chain entries) | REQ-INSTR-02 | ✅ shipped |
| `E_StaticAnalysis` instrument (256-d, scalar aggregates) | REQ-INSTR-03 | ✅ shipped |
| `E_AST` (384-d, Qodo+AST-token-transformer over chunks) | REQ-INSTR-04 | partial: deterministic Python tree-sitter encoder shipped; Qodo/chunk-router replacement deferred |
| `E_CFG` (256-d, E8+graph-attention over basic blocks) | REQ-INSTR-05 | partial: deterministic Python tree-sitter/source-shape encoder shipped; E8 graph head deferred |
| `E_DataFlow` (256-d, E11+graph-attention over def-use chains) | REQ-INSTR-06 | partial: deterministic Python def/use feature encoder shipped; E11 graph head deferred |
| `E_TypeGraph` (256-d, E11+Pyright/mypy/rust-analyzer/javac/dotnet/etc. lattice) | REQ-INSTR-07 | partial: deterministic type-hint/source-shape encoder shipped; analyzer-backed lattice deferred |
| `E_Test` (384-d, E1 on test-fn descriptions) | REQ-INSTR-08 | partial: deterministic typed text encoder shipped; E1 replacement deferred |
| `E_Trace` (512-d, E2/E3/E4+sequence-transformer over `(fn_id, line_no, val_hash)` events) | REQ-INSTR-09 | partial: deterministic typed trace aggregate encoder shipped; sequence model deferred |
| `E_Diff` (256-d, Qodo+diff-edge encoder) | REQ-INSTR-10 | partial: deterministic before/after source-diff encoder shipped; Qodo diff-edge head deferred |
| `E_Problem` (1024-d, E1 on SWE-bench problem text) | REQ-INSTR-11 | partial: deterministic typed text encoder shipped; E1 replacement deferred |
| `E_CommitMsg` (384-d, E1 on commit text) | REQ-INSTR-12 | partial: deterministic typed text encoder shipped; E1 replacement deferred |
| `E_Runtime` (256-d, scalar aggregates: walltime + RSS + coverage Δ) | REQ-INSTR-13 | partial: deterministic typed runtime aggregate encoder shipped; live harness capture wiring deferred |
| `E_Reasoning` (384-d, E17 AffectText on agent CoT) | REQ-INSTR-14 | partial: deterministic typed reasoning-event encoder shipped; E17 replacement deferred |
| Panel + PanelBuilder + 5,120-d concatenation + JSON validator + dimensional fail-closed | REQ-INSTR-15 | ✅ shipped |
| Scalar feature aggregator (BFS depth, blame age, churn, complexity, coverage Δ, repo-health → 128-d) | REQ-INSTR-16 | partial: deterministic typed scalar encoder shipped; production signal plumbing deferred |

### Phase 2 — Predictor + frozen target + VICReg (PREDICTOR) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| 6-layer transformer (hidden 1024, 8 heads, GeLU, residual + layer-norm); inputs (panel[t=0], panel[t=1]) concat 8,192-d → output 5,120-d | REQ-PRED-01 | ✅ shipped (`crates/context-graph-mejepa/src/predictor/`) |
| Frozen target-readback adapter over panel[t=2] instrument outputs | REQ-PRED-02 | ✅ shipped (`frozen_target.rs`) |
| VICReg loss (variance + invariance + covariance) implementation | REQ-PRED-03 | ✅ shipped (`crates/context-graph-mejepa-train/src/loss/vicreg.rs`) |
| Oracle head: Linear&lt;5120, num_tests + 1&gt; on top of predictor output | REQ-PRED-04 | ✅ shipped (`oracle_head.rs`) |
| Forward-pass FSV: latency &lt; 50 ms / batch (batch=64, RTX 5090, bf16); loss finite; no NaN | REQ-PRED-05 | ✅ FSV evidence via `predictor_fsv` example; live latency capture pending |
| VRAM steady-state ≤ 8 GB at batch 64 | REQ-PRED-06 | partial — `vram.rs` accounting shipped; live measurement pending |

### Phase 3 — Training loop + UTML L_step + per-step certs (TRAINING + UTML-FORMULA) — **MOSTLY SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| AdamW + cosine decay (lr 5e-4, weight_decay 0.04, batch 64, 10 epochs) | REQ-TRAIN-01 | ✅ shipped (`crates/context-graph-mejepa-train/src/optim/`) |
| Per-step training certificate (CF_MEJEPA_TRAIN_CERTS) with all 4 UTML factors + components + sampling_weight + grad_norm + drift_coverage + merkle_root | REQ-TRAIN-02 | ✅ shipped (`cert/`) |
| Hash-chained certificate log: parent_witness_hash → self_hash | REQ-TRAIN-03 | ✅ shipped + FSV via `train_cert_chain_fsv` |
| Witness-chain entry per epoch | REQ-TRAIN-04 | ✅ shipped |
| Holdout evaluation every 1,000 steps | REQ-TRAIN-05 | ✅ shipped + FSV via `train_holdout_eval_fsv` |
| BatchSampler with sampling_weight = base × L_step × (1 + 5·operator_override) × decay^age_in_days | REQ-TRAIN-06 | ✅ shipped (`sampler/`) |
| `compute_delta_p(predicted, target, snr, exploration_bonus)` | REQ-UTML-01 | ✅ shipped (`learning_signal.rs`) |
| `compute_delta_k(grad, ema_grad, fisher, ece, embedder_coherence)` | REQ-UTML-02 | ✅ shipped |
| `compute_delta_omega(plasticity, landscape, stability, agent_state)` | REQ-UTML-03 | ✅ shipped |
| `compute_delta_xi(target_collapse, redundancy, violation)` (target_collapse always 0) | REQ-UTML-04 | ✅ shipped and hardened — non-zero/non-finite target collapse fails closed with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`; formula is multiplicative non-collapse |
| EMA running mean of recent batch gradients | REQ-UTML-05 | ✅ shipped |
| Pairwise MI audit job persisting `pairwise_mi_matrix.csv` | REQ-UTML-06 | ✅ shipped — `learning_signal/pairwise_mi_matrix.rs::{PairwiseMiAuditor, PairwiseMiMatrix, compute_pairwise_mi_matrix, load_pairwise_mi_matrix_csv}`, explicit `TrainingCertificate::{predictor_redundancy_pairwise_mi,predictor_redundancy_pairwise_mi_source}` columns, and `mcp__cgreality__mejepa_audit_pairwise_mi` MCP wrapper with CSV source-of-truth readback |
| Continual-backprop dormant-unit re-init (ΔΩ.effective_plasticity intervention) | REQ-UTML-07 | ✅ shipped — `learning_signal/continual_backprop.rs::{DormantUnitDetector, DormantUnitReport, detect_dormant_units, reinit_dormant_units}` plus `heal/regulate.rs` trigger that persists changed handles to `CF_MEJEPA_WEIGHT_BLOBS` and performs byte readback |
| State-transfer T detector for active-learning trigger | REQ-UTML-08 | ✅ shipped — `learning_signal/state_transfer.rs::{compute_state_transfer, performance_deploy, StateTransferDiagnostic}`; surfaced via `eval/types.rs::StateTransferDiagnostic` and Phase 8 weekly report |
| FSV: 100 synthetic examples spanning ΔP/ΔK/ΔΩ/ΔΞ corner cases; verify L_step matches expected envelope | REQ-UTML-09 | ✅ shipped via `train_edge_cases_fsv`; target-collapse leak edge proves no training cert is written on invariant violation |

### Phase 4a — Inference + conformal + OOD (INFERENCE) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| `MeJepaCompiler::compile(patch, context) -> RealityPrediction` | REQ-INFER-01 | ✅ shipped (`crates/context-graph-mejepa/src/compiler.rs`) |
| `MeJepaCompiler::verify(patch, context) -> Verdict` | REQ-INFER-02 | ✅ shipped + `inference_fsv` example |
| Conformal calibration on 10% calibration split; non-conformity score = max_test \|predicted - actual\| | REQ-INFER-03 | ✅ shipped (`conformal.rs`, `calibration.rs`) |
| OOD detection via predictor reconstruction error normalized by σ²; threshold 0.5 → escalate | REQ-INFER-04 | ✅ shipped (`ood.rs`) |
| `mcp__cgreality__mejepa_verify` MCP tool with include_provenance optional flag | REQ-INFER-05 | ✅ shipped (`crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`) |
| Single-prediction latency &lt; 100 ms p50 (cached instruments) | REQ-INFER-06 | partial — code path shipped; live measurement pending Phase 8 |
| Empirical conformal coverage on calibration split: stated 90% ± 2% | REQ-INFER-07 | partial — calibration code shipped; live coverage measurement pending Phase 8 |
| OOD AUC &gt; 0.85 between in-distribution and synthetic OOD inputs | REQ-INFER-08 | partial — OOD scorer shipped; live AUC measurement pending Phase 8 |

### Phase 4b — TCT constellation + Gτ guard (TCT-CONSTELLATION) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| `TctConstellation` data structure with per-(category × entity × language × embedder) cells | REQ-TCT-01 | ✅ shipped (`crates/context-graph-mejepa-tct/src/constellation.rs`) |
| Centroid construction over Phase 0 mutation corpus + KnownGood + cross-language stratification | REQ-TCT-02 | ✅ shipped (`builder.rs`) |
| Bayesian shrinkage on cells with N &lt; 50 supporting examples | REQ-TCT-03 | ✅ shipped (`shrinkage.rs`, `shrinkage_engine.rs`) |
| Per-embedder threshold τ_m calibration: Gτ false-positive rate ≤ 5% on held-out KnownGood | REQ-TCT-04 | ✅ shipped (`calibrator.rs`) |
| `Gτ(predicted_panel; CP) -> bool` deterministic O(N · d_max) test | REQ-TCT-05 | ✅ shipped (`gtau.rs`) |
| `Verdict::GuardRejected` variant with violating_embedders + violating_chunks: Vec&lt;ChunkId&gt; | REQ-TCT-06 | ✅ shipped — variant shipped (`verdict.rs`) and chunk-level localization covered by `gtau_check_chunks` plus `cargo test -p context-graph-mejepa-tct chunk_gtau --lib`; production-scale centroid refresh evidence tracked in [#9](https://github.com/ChrisRoyse/contextgraph/issues/9) |
| `L_constellation_centroid` loss term: Σ_m max(0, τ_m − cos(g_φ(panel)_m, c_m)) | REQ-TCT-07 | ✅ shipped (`crates/context-graph-mejepa-train/src/loss/constellation.rs`) |
| RocksDB CF_MEJEPA_CONSTELLATION keyed by version_id + corpus_provenance | REQ-TCT-08 | ✅ shipped (`store.rs`) |
| Constellation max-age policy (default 90 days; refuse stale without operator override) | REQ-TCT-09 | ✅ shipped (`freshness.rs`) |

### Phase 5 — Self-healing + EWC++ + drift + A/B/C (SELF-HEALING) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| `MeJepaCompiler::observe(panel, oracle_outcome)` online SGD with EWC++ regularization | REQ-HEAL-01 | ✅ shipped (`crates/context-graph-mejepa/src/compiler.rs`) |
| EWC++ Fisher matrix with online update + task-boundary detection | REQ-HEAL-02 | ✅ shipped (`heal/ewc.rs`) |
| Drift detection: rolling 1,000-attempt window; soft (88-85%) / hard (85-80%) / catastrophic (&lt; 80%) | REQ-HEAL-03 | ✅ shipped (`heal/drift.rs`) |
| A/B/C retrain pipeline: A=current, B=fresh corpus retrain, C=ensemble; promote winner | REQ-HEAL-04 | ✅ shipped (`heal/promote.rs`) |
| Periodic full retrain every N=10,000 attempts | REQ-HEAL-05 | ✅ shipped (`heal/full_retrain.rs`) |
| Continuous calibration sliding window every 1,000 attempts | REQ-HEAL-06 | ✅ shipped (`heal/calibration.rs`) |
| Witness-chain integrity check every 10,000 attempts | REQ-HEAL-07 | ✅ shipped (`heal/integrity.rs`) |
| LoRA adapter refresh on ΔΩ.effective_plasticity collapse | REQ-HEAL-08 | ✅ shipped (`heal/lora_refresh.rs`) |
| Distillation-based online observe (reuses crates/context-graph-embeddings/src/training/distillation.rs) | REQ-HEAL-09 | ✅ shipped (`heal/distill.rs`) |
| FSV: synthetic drift drill detects within 1,000 observations + promotes Mode B/C only on holdout win | REQ-HEAL-10 | ✅ shipped — `mejepa-heal-drill` binary + `phase5_self_healing_fsv` example; hard/soft/catastrophic drills detect at 700 obs, mode winner C, evidence files mode `0600`; invalid max-observations edge exits with `MEJEPA_HEAL_INVALID_STATE` and creates no output root |

### Phase 6 — Disk + memory hygiene (HYGIENE) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| Tier-compression integration: hot 8 / warm 7 / cool 5 / cold 3 / frozen 1 bit | REQ-HYG-01 | ✅ shipped — `tier-compression` foundation crate + `tier.rs`/`tier_frozen.rs`/`tier_ops.rs` in `context-graph-mejepa-hygiene` (`tier_for_score`, `decayed_score`, `score_after_read`, `encode_for_tier`, `decode_from_tier`, `tier_demote`, `tier_promote`, `tier_demote_all`, `tier_promote_all`) |
| Stoer-Wagner mincut for repo-atomic split (already shipped per crates/context-graph-mincut) | REQ-HYG-02 | ✅ shipped |
| Witness-chain Merkle compression for chains older than 90 days | REQ-HYG-03 | ✅ shipped — `witness_compress.rs::{witness_compress_old_segments, verify_witness_integrity, audit_verify_old_entry}` + `merkle_compression_fsv` example |
| Nightly GC at 03:00 local: demote + compact + evict + prune + verify | REQ-HYG-04 | ✅ shipped — `gc.rs::gc_run_nightly` returning `GcReport`; `gc_stress_fsv` example proves 10,000-entry stress run. Live cron scheduling is operational follow-up |
| Quota enforcement: hard 115 GB; emergency eviction on overflow | REQ-HYG-05 | ✅ shipped — `quota.rs::{quota_status, quota_check_and_evict}` + `quota_overflow_fsv` example |
| `me_jepa::ops::*` MCP-callable: gc_run_nightly, quota_check_and_evict, heal_detect_drift, heal_retrain_if_needed, witness_compress_old_segments, witness_verify_integrity, tier_demote, tier_promote | REQ-HYG-06 | ✅ shipped — `ops.rs` exports all eight ops; `mcp.rs::{mcp_gc_run, mcp_quota_status, mcp_witness_compress}` and dispatch wires `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress` MCP tools |
| FSV: 10,000-entry stress corpus → nightly GC keeps disk ≤ 110 GB | REQ-HYG-07 | ✅ shipped — `gc_stress_fsv` example writes evidence files mode `0600` |

### Phase 7 — Shift subscriber + MCP tools (SHIFT-SUBSCRIBER) — **SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| Tokio task tailing `cgreality-shift-log/<session_id>.jsonl`; per-shift instrument cache keyed by file_sha | REQ-SHIFT-01 | ✅ shipped — `context-graph-mejepa-shift-subscriber::{ShiftLogTail, ShiftSubscriber::{run_until_idle, run_until_shutdown}}` + `cache::{InstrumentCache, SlotKey}` |
| Watermark CF (CF_MEJEPA_SHIFT_WATERMARK) records last shift_id consumed; restart-safe | REQ-SHIFT-02 | ✅ shipped — `WatermarkWriter::{new, write, read_all}`; `ShiftSubscriber::open` reconstructs `ShiftLogTail` cursors from the persisted byte offset per session JSONL |
| Per-shift latency: p50 &lt; 5s; p99 &lt; 30s; subscriber-lag alert &gt; 500 unprocessed shifts / 1 minute | REQ-SHIFT-03 | ✅ shipped at code level — `SubscriberMetrics::{record_latency_ms, snapshot_latency_ms}` returns `LatencySnapshot { p50_ms, p99_ms }`; `update_lag_alert` fires `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT` on sustained breach. Live distribution capture under production traffic is operational follow-up |
| `mcp__cgreality__mejepa_predict_latest()` MCP tool | REQ-SHIFT-04 | ✅ shipped (`crates/context-graph-mcp/src/tools/names.rs` + `mejepa_tools.rs::call_mejepa_predict_latest`) |
| `mcp__cgreality__mejepa_observe_shift(shift_id)` MCP tool | REQ-SHIFT-05 | ✅ shipped — `mejepa_phase7_tools.rs::call_mejepa_observe_shift` + dispatch in `dispatch.rs` |
| `mcp__cgreality__mejepa_subscriber_status()` MCP tool | REQ-SHIFT-06 | ✅ shipped — `mejepa_phase7_tools.rs::call_mejepa_subscriber_status` returns `LiveSubscriberStatus` (counts, latency p50/p99, lag-alert flag, last panic) |
| `mcp__cgreality__mejepa_capture_audit(attempt_id)` MCP tool | REQ-SHIFT-07 | ✅ shipped — `mejepa_phase7_tools.rs::call_mejepa_capture_audit` + `CaptureAuditBundle` |
| Backpressure + durability: log queues; on resume, tail from last-processed offset | REQ-SHIFT-08 | ✅ shipped — `reality_shift_log` reader/writer + `ShiftLogTail::poll_next_line` + `WatermarkWriter` round-trip; in-crate `tokio::test` proves single-shift round-trip restarts at byte-offset = line.len()+1 |
| FSV: synthetic shift stream end-to-end latency profiled; persistence verified | REQ-SHIFT-09 | ✅ shipped — `shift_subscriber_fsv` + 2 in-crate `tokio::test` round-trips: `run_until_idle_consumes_jsonl_and_persists_readback_state` and `run_until_idle_skips_non_code_shift_and_advances_watermark` |

### Phase 8 — Evaluation against ≥0.95 (EVALUATION) — **HARNESS SHIPPED**

| Deliverable | Spec ref | Status |
|---|---|---|
| Holdout split evaluation: prediction-oracle correlation per (mutation_category, language) | REQ-EVAL-01 | ✅ shipped — `crates/context-graph-mejepa/src/eval/runner.rs::EvalRunner::run_holdout_eval` + `metrics.rs::{pearson_correlation, compute_correlations}` |
| Per-language stratification: conformal calibration + OOD calibration + Gτ thresholds calibrated per language | REQ-EVAL-02 | ✅ shipped — `metrics.rs::{conformal_health, ood_auc_by_language, gtau_pass_rate_by_language, bayesian_shrinkage}` |
| ≥0.95 ship gate: stable correlation across all 8 categories AND all supported languages on rolling-window holdout | REQ-EVAL-03 | ✅ shipped at code level — `metrics.rs::ship_gate_failures` + `EvalReport::{ship_gate_passed, ship_gate_failures}` + `mejepa_ship_gate_status` MCP tool. Production rolling-window data is operational follow-up |
| Active learning loop: when conformal set &gt; 3 AND OOD score &lt; 0.5, surface for operator label / Docker oracle re-run / adjacent-test re-run | REQ-EVAL-04 | ✅ shipped — `queue.rs::ActiveLearningQueueState::{new, enqueue_uncertain}` + `ActiveLearningLabel` + `LabelMethod` + `mejepa_active_learning_queue` MCP tool |
| Cross-task transfer: patch-similarity graph upweights similar tasks during fine-tune | REQ-EVAL-05 | ✅ shipped — `graph.rs::{PatchEmbedding, PatchGraphEdge, PatchSimilarityGraph, build_patch_similarity_graph}` + `mejepa_eval_build_graph` MCP tool + `RocksDbEvalStore::{persist_graph, load_graph}` |
| Aux-head distillation back into trunk every 1,000 steps | REQ-EVAL-06 | ✅ shipped — `types.rs::AuxHeadDistillationSummary` + report column; aux head wiring in `crates/context-graph-mejepa-train/src/aux/` |
| Weekly evaluation report: tracked metric + per-category + per-language; surface to operator | REQ-EVAL-07 | ✅ shipped — `report.rs::{write_weekly_report, seed_open_research_questions, write_json_0600}`; `phase8_eval_fsv` writes 17 evidence files to `/tmp/contextgraph-mejepa-phase8-eval-fsv/` |

---

## Spec → task mapping summary

After Level 2/3 specs are written, **Level 4 task generation** runs the Inside-Out, Bottom-Up slicing rule:

```
Layer 1: Foundation (data models, types, RocksDB CFs, error taxonomy)
   ↓
Layer 2: Logic (instruments, predictor forward, loss functions, observe/verify methods, drift detector, Gτ guard, mincut, tier compression integration)
   ↓
Layer 3: Surface (MCP tools, CLI commands, shift-log subscriber, ops endpoints)
```

Each phase's task list lives under `tasks/<phase>/TASK-<DOMAIN>-NNN.md`. The `tasks/_index.md` is the dependency DAG; `tasks/_traceability.md` is the spec-coverage matrix.

---

## Review and revision log

| Date | Revision | Author | Notes |
|---|---|---|---|
| 2026-05-09 | v1.0.0 initial | Claude Code (outer-loop optimizer) | First full plan covering all 8 phases × 13 domains |
| 2026-05-10 | v1.0.2 hardening refresh | Codex | Updated Phase 1-5 status after manual FSV; documented `ΔΞ.target_collapse` fail-closed fix, embedder forward blindspot, and AST compatibility caveat |
| 2026-05-11 | v1.0.3 Phase 6/7/8 + UTML refresh | Claude Code | Marked Phase 6 (HYGIENE) shipped (`context-graph-mejepa-hygiene` + 3 MCP tools), Phase 7 (SHIFT-SUBSCRIBER) shipped (`context-graph-mejepa-shift-subscriber` + 3 MCP tools), Phase 8 (EVALUATION) harness shipped (`crates/context-graph-mejepa/src/eval/` + 4 MCP tools), and UTML carriers REQ-UTML-06/07/08 shipped (`learning_signal/{pairwise_mi_matrix, continual_backprop, state_transfer}`). New crate `context-graph-mejepa-cf` (shared CF registry). Cross-references doc 16 evidence ledger throughout |
| 2026-05-11 | v1.0.4 issue-ledger refresh | Codex | Added issue links for unresolved forward-edge work, corrected live MCP surface to 19 tools, marked `mejepa_audit_pairwise_mi`, train-cert pairwise-MI columns, `heal/regulate.rs` continual-backprop hook, and chunk-level `gtau_check_chunks` localization as shipped. |
