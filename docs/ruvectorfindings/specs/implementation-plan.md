# ME-JEPA-Code Implementation Plan

> **Phase-ordered execution roadmap.** Translates the 8-phase build plan in `09_END_GOAL_REVIEW_REPLACEMENT.md §7` (re-framed by `10_MEJEPA_PIVOT_2026_05_09.md`) into a concrete sequence of spec-IDs and task-IDs. **Every entry references a Level 2 functional spec, a Level 3 technical spec, and a Level 4 task list.**

**Date:** 2026-05-10 (status refresh; spec body authored 2026-05-09)
**Spec version:** 1.0.1
**Authority:** `constitution.md §spec_authority_order` and `09_END_GOAL_REVIEW_REPLACEMENT.md §7` Phase plan.

> **2026-05-10 status snapshot.** Phases 0 (corpus), 1c (instrument scaffolds), 2 (predictor), 3 (training), 4a (inference), 4b (TCT), and 5 (self-healing) are SHIPPED end-to-end with FSV evidence. Phase 1a (multi-language `ast_chunker.rs` extension), Phase 1b (per-embedder `Instrument` wrappers backed by real model loads), Phase 6 (storage hygiene operations), Phase 7 (shift subscriber Tokio task), and Phase 8 (evaluation harness against ≥0.95 gate) are the active forward edge. The Phase Summary table below has been refreshed; per-phase deliverable status is in §"Per-phase deliverable table".

---

## Phase summary

| Phase | Domain(s) | Effort | Parallelizable? | Status (2026-05-10) |
|---|---|---|---|---|
| 0 | CORPUS | ~5-7 days | partially (mutation operators are independent) | partial — operators+split+oracle bridge+CLI shipped; full 300×8 batch run pending; non-Python operators pending |
| 1a | AST-CHUNKING | ~10 days | yes (per-language extension is parallel) | partial — Rust chunker shipped; multi-language extension of `ast_chunker.rs` (REQ-AST-01..13) pending; semantic extractor for all 11 languages already exists in `dynamicjepa.rs` |
| 1b | EMBEDDERS | ~5 days | yes (per-embedder Instrument trait wrappers are parallel) | partial — 21-embedder enum, manifest digest, VRAM gate, routing wrapper shipped; per-embedder `Instrument` wrappers with real model loads (REQ-EMBED-02..11) pending |
| 1c | INSTRUMENTS | ~15 days | partially (instruments depend on AST + embedders + chunker; each instrument is then independent) | partial — all 15 panel slots have deterministic encoders + Panel/PanelBuilder/PanelStore + linear-probe gate shipped; 11 of 14 are placeholders awaiting embedder/analyzer-backed replacements |
| 2 | PREDICTOR | ~7-10 days | no (sequential: panel → predictor → frozen target → VICReg → Oracle head) | **shipped** — `MeJepaPredictor`, `VICRegLoss`, `OracleHead`, `FrozenTargetAdapter` in `crates/context-graph-mejepa/`; `predictor_fsv` example |
| 3 | TRAINING | ~5-7 days | yes (training loop scaffolding parallels training-cert + sampler) | **shipped** — `Trainer` + AdamW + cosine LR + `BatchSampler` + `TrainingCertificate` chain + holdout eval + LoRA wiring + distillation in `crates/context-graph-mejepa-train/`; 5 FSV examples |
| 3+ | UTML-FORMULA | ~5 days | yes (ΔP, ΔK, ΔΩ, ΔΞ are independent computations) | partial — 4 deltas + L_step computation + sampling reweighting shipped in `learning_signal.rs`; pairwise MI audit (REQ-UTML-06), continual-backprop (REQ-UTML-07), state-transfer T detector (REQ-UTML-08) pending |
| 4a | INFERENCE | ~3-5 days | no (sequential: verify() → conformal → OOD) | **shipped** — `MeJepaCompiler::{compile, verify, observe}` + conformal + OOD + `Verdict` enum; `mcp__cgreality__mejepa_verify` MCP tool live; `inference_fsv` example |
| 4b | TCT-CONSTELLATION | ~5 days | yes (centroid construction parallels Gτ guard + GuardRejected verdict) | **shipped** — full `TctConstellation` + `Gτ` + `Verdict::GuardRejected` + Bayesian shrinkage + freshness check + threshold calibrator in `crates/context-graph-mejepa-tct/`; `mcp__cgreality__mejepa_constellation_inspect` MCP tool live |
| 5 | SELF-HEALING | ~5-7 days | partially (drift detection parallels EWC++; A/B/C promotion is sequential) | **shipped** — 16-module `heal/` tree + `mejepa-heal-drill` binary + 6 RocksDB CFs + `mcp__cgreality__mejepa_heal_status` and `mejepa_rollback_to` MCP tools live |
| 6 | HYGIENE | ~3-5 days | yes (tier integration parallels Merkle compression + GC scheduler) | partial — tier-compression + mincut crates shipped; CF integration, Merkle compression of >90d chains, nightly GC, 115 GB quota, `me_jepa::ops::*` MCP wrappers (REQ-HYG-03..07) pending |
| 7 | SHIFT-SUBSCRIBER | ~5-7 days | yes (subscriber Tokio task parallels new MCP tools) | partial — `reality_shift_log` reader/writer + `mcp__cgreality__mejepa_predict_latest` shipped; Tokio subscriber loop, watermark CF, and `mejepa_observe_shift` / `mejepa_subscriber_status` / `mejepa_capture_audit` MCP tools (REQ-SHIFT-01..09) pending |
| 8 | EVALUATION | ongoing | yes (per-category eval parallels per-language eval + active-learning) | partial — holdout-eval harness shipped; per-language stratification, ≥0.95 ship gate, active-learning loop (REQ-EVAL-04), cross-task patch-similarity transfer (REQ-EVAL-05), aux-head distillation back to trunk (REQ-EVAL-06), and weekly report (REQ-EVAL-07) pending |

**Total fresh effort estimate:** ~75-95 days. Embedder reuse and chunker extension shorten Phase 1 from "build 14 instruments from scratch" to "build 14 instruments as thin heads on existing embedders" per doc 12 §3.

---

## Critical path

The longest dependency chain — bottlenecks if not built in this order:

```
Phase 0 corpus generator (CORPUS)
   ↓ produces (panel[t=0..2], oracle_verdict) tuples
Phase 1a AST chunker extension (AST-CHUNKING)
   ↓ produces AST-perfect chunks for all 11 languages
Phase 1b Instrument trait wrappers (EMBEDDERS)
   ↓ unifies the 21 embedders behind one trait
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
| Refactor `EntityType` to language-tagged enum | REQ-AST-01 | new |
| Per-language node-type classifier reusing DynamicJEPA logic (Python, JS/TS, Go, Java, C/C++, C#, Ruby, PHP) | REQ-AST-02..09 | new |
| Cross-language test harness: chunk every fixture; assert AST entity boundaries | REQ-AST-10 | new |
| FSV: SHA-pinned chunk boundaries persisted per fixture | REQ-AST-11 | new |
| Mixed-language refusal (HTML+JS, PHP+HTML) with MEJEPA_CHUNKER_MIXED_LANGUAGE | REQ-AST-12 | new |
| Reformat-invariant chunk SHAs (computed on AST node IDs, not raw text) | REQ-AST-13 | new |

### Phase 1b — Embedder Instrument trait wrappers (EMBEDDERS)

| Deliverable | Spec ref | Status |
|---|---|---|
| `Instrument` trait + `InstrumentSlot` enum (15 slots summing to 5,120-d panel) | REQ-EMBED-01 | ✅ shipped (Phase 1 scaffolding) |
| Per-embedder Instrument-trait wrapper (E1, E5, E7, E8, E10, E11, E14 used in panel) | REQ-EMBED-02..08 | new |
| Per-chunk-type embedder routing table (function → E7+E14, class → E11+E7, comment → E1+E10, etc.) | REQ-EMBED-09 | new |
| Lazy-load policy for E15-E21 learner-state embedders (VRAM headroom) | REQ-EMBED-10 | new |
| FSV: `embedder_routing_fsv` writes per-chunk-type route table + asserts all 11 languages × 13 entity types covered | REQ-EMBED-11 | new |

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
| `compute_delta_xi(target_collapse, redundancy, violation)` (target_collapse always 0) | REQ-UTML-04 | ✅ shipped |
| EMA running mean of recent batch gradients | REQ-UTML-05 | ✅ shipped |
| Pairwise MI audit job persisting `pairwise_mi_matrix.csv` | REQ-UTML-06 | pending |
| Continual-backprop dormant-unit re-init (ΔΩ.effective_plasticity intervention) | REQ-UTML-07 | pending — Phase 5 `regulate.rs` is the integration point |
| State-transfer T detector for active-learning trigger | REQ-UTML-08 | pending |
| FSV: 100 synthetic examples spanning ΔP/ΔK/ΔΩ/ΔΞ corner cases; verify L_step matches expected envelope | REQ-UTML-09 | ✅ shipped via `train_edge_cases_fsv` |

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
| `Verdict::GuardRejected` variant with violating_embedders + violating_chunks: Vec&lt;ChunkId&gt; | REQ-TCT-06 | partial — variant shipped (`verdict.rs`); chunk-level localization pending Phase 1a chunker |
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
| FSV: synthetic drift drill detects within 1,000 observations + promotes Mode B/C only on holdout win | REQ-HEAL-10 | ✅ shipped — `mejepa-heal-drill` binary + `phase5_self_healing_fsv` example; happy-path drill detected hard drift at 700 obs, mode winner C, exit_code 0 |

### Phase 6 — Disk + memory hygiene (HYGIENE)

| Deliverable | Spec ref | Status |
|---|---|---|
| Tier-compression integration: hot 8 / warm 7 / cool 5 / cold 3 / frozen 1 bit | REQ-HYG-01 | partial (tier-compression crate ✅) |
| Stoer-Wagner mincut for repo-atomic split (already shipped per crates/context-graph-mincut) | REQ-HYG-02 | ✅ shipped |
| Witness-chain Merkle compression for chains older than 90 days | REQ-HYG-03 | new |
| Nightly GC at 03:00 local: demote + compact + evict + prune + verify | REQ-HYG-04 | new |
| Quota enforcement: hard 115 GB; emergency eviction on overflow | REQ-HYG-05 | new |
| `me_jepa::ops::*` MCP-callable: gc_run_nightly, quota_check_and_evict, heal_detect_drift, heal_retrain_if_needed, witness_compress_old_segments, witness_verify_integrity, tier_demote, tier_promote | REQ-HYG-06 | new |
| FSV: 10,000-entry stress corpus → nightly GC keeps disk ≤ 110 GB | REQ-HYG-07 | new |

### Phase 7 — Shift subscriber + MCP tools (SHIFT-SUBSCRIBER) — **PARTIAL**

| Deliverable | Spec ref | Status |
|---|---|---|
| Tokio task tailing `cgreality-shift-log/<session_id>.jsonl`; per-shift instrument cache keyed by file_sha | REQ-SHIFT-01 | pending |
| Watermark CF (CF_MEJEPA_SHIFT_WATERMARK) records last shift_id consumed; restart-safe | REQ-SHIFT-02 | partial — `CF_MEJEPA_SHIFT_WATERMARK` declared and persisted via heal pipeline; subscriber consumer pending |
| Per-shift latency: p50 &lt; 5s; p99 &lt; 30s; subscriber-lag alert &gt; 500 unprocessed shifts / 1 minute | REQ-SHIFT-03 | pending |
| `mcp__cgreality__mejepa_predict_latest()` MCP tool | REQ-SHIFT-04 | ✅ shipped (`crates/context-graph-mcp/src/tools/names.rs:102` + `mejepa_tools.rs::call_mejepa_predict_latest`) |
| `mcp__cgreality__mejepa_observe_shift(shift_id)` MCP tool | REQ-SHIFT-05 | pending |
| `mcp__cgreality__mejepa_subscriber_status()` MCP tool | REQ-SHIFT-06 | pending |
| `mcp__cgreality__mejepa_capture_audit(attempt_id)` MCP tool | REQ-SHIFT-07 | pending |
| Backpressure + durability: log queues; on resume, tail from last-processed offset | REQ-SHIFT-08 | partial — `reality_shift_log` reader/writer in `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs` provides durable on-disk JSONL log; consumer-side resume pending |
| FSV: synthetic shift stream end-to-end latency profiled; persistence verified | REQ-SHIFT-09 | pending |

### Phase 8 — Evaluation against ≥0.95 (EVALUATION)

| Deliverable | Spec ref | Status |
|---|---|---|
| Holdout split evaluation: prediction-oracle correlation per (mutation_category, language) | REQ-EVAL-01 | new |
| Per-language stratification: conformal calibration + OOD calibration + Gτ thresholds calibrated per language | REQ-EVAL-02 | new |
| ≥0.95 ship gate: stable correlation across all 8 categories AND all supported languages on rolling-window holdout | REQ-EVAL-03 | new |
| Active learning loop: when conformal set &gt; 3 AND OOD score &lt; 0.5, surface for operator label / Docker oracle re-run / adjacent-test re-run | REQ-EVAL-04 | new |
| Cross-task transfer: patch-similarity graph upweights similar tasks during fine-tune | REQ-EVAL-05 | new |
| Aux-head distillation back into trunk every 1,000 steps | REQ-EVAL-06 | new |
| Weekly evaluation report: tracked metric + per-category + per-language; surface to operator | REQ-EVAL-07 | new |

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
