# 02 — Current State, Honest

> **Purpose.** Plain accounting of what's shipped at code level, what's shipped operationally, what's a placeholder, and what doesn't exist. No marketing, no aspirational language. Source of truth: `docs/ruvectorfindings/specs/_README.md` (Spec version 1.0.4, 2026-05-11) + the hardening ledgers `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` and `16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`.
>
> **Last refreshed:** 2026-05-11 (post-TASK-FLYWHEEL-006). Active in-flight work: full 2,400-row corpus regeneration is running against `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` with 300 known_good rows committed and 300 subtle_flip patches generated; Docker oracle is processing subtle_flip verdicts. **Every task picked up from `docs/futurebuild/tasks/` MUST first read `docs/futurebuild/specs/FSV-PROTOCOL.md` — that file is the binding verification contract.**

---

## 1. What is *actually* shipped at code/FSV level

| Phase | Domain | Shipped (code + FSV) | Crate(s) |
|---|---|---|---|
| 0 | CORPUS | 8 Python mutation operators; repo+instance-atomic split; one-task SWE-bench Docker bridge; corpus CLI (`generate/verify/stats/prepare-images/cleanup`); WSL→D migrator; leakage detector; D-backed corpus-root enforcement; 8 categories tested on at least 1 SWE-bench Lite instance round-trip with real Docker | `context-graph-mejepa-corpus` |
| 1a | AST-CHUNKING | 11-language tree-sitter chunker: Rust, Python, JS, TS, Go, Java, C, C++, C#, Ruby, PHP. Typed `Language`/`EntityType`. SHA-256 chunk IDs. FSVed for all 11 languages | `context-graph-core::memory::ast` |
| 1b | EMBEDDERS | 21 declared slots; E5/E11 retired; 19 active registrations; SHA-256 digest verification; VRAM budget gate; E17 calibration cert; 143-row routing table; fail-closed `EmbedderForward`/`PretrainedEmbedderForward` wrappers for E1/E6/E7/E8/E10/E12/E13/E14 with runtime artifact checks and E10 FSV | `context-graph-mejepa-embedders` |
| 1c | INSTRUMENTS | All 15 panel slots (5,120-d total) with deterministic typed encoders + Panel + PanelBuilder + PanelStore + linear-probe gate. E_Oracle (128-d), E_Witness (256-d), E_StaticAnalysis (256-d), and embedder-backed projection heads for code/text/trace semantic slots are first-class today | `context-graph-mejepa-instruments` |
| 1d | FLYWHEEL-TYPES | **(2026-05-11)** REQ-FLYWHEEL-10 value types shipped in `crates/context-graph-mejepa/src/types.rs`: `PredictionId`/`FeedbackId`/`PanelId`/`WitnessHash`/`AgentId` newtypes, `SurpriseSeverity`/`FeedbackKind`/`FailureModeClass` enums, `DdaSignals`/`ExemplarRef`/`SurpriseEvent`/`ActualOutcome`/`EmbedderViolationSignature` structs with `validate()` invariants. Three new CFs (`CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`) registered in `mejepa-cf` and wired through `open_hygiene_rocksdb`. FSV at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json` with `all_passed=true` against real RocksDB SST files. | `context-graph-mejepa-cf`, `context-graph-mejepa::types`, `context-graph-mejepa-hygiene::storage` |
| 2 | PREDICTOR | 6-layer transformer (hidden 1024, 8 heads), VICReg loss, Oracle head, FrozenTargetAdapter, PanelSource, predictor_fsv example. CUDA-only sm_120 | `context-graph-mejepa::predictor` |
| 3 | TRAINING | Full Trainer + AdamW + cosine LR + BatchSampler + PatchSimilarityGraph + sampling_weight + TrainingCertificate hash chain + holdout eval + LoRA + distillation. 5 FSV examples including 100k-step cert-chain replay | `context-graph-mejepa-train` |
| 3+ | UTML-FORMULA | `compute_delta_p / _k / _omega / _xi / _l_step` shipped with `target_collapse == 0.0` enforced via `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. PairwiseMiAuditor + CSV writer + `mejepa_audit_pairwise_mi` MCP tool. DormantUnitDetector + `reinit_dormant_units` wired into `heal/regulate.rs`. State-transfer T detector | `context-graph-mejepa-train::learning_signal` |
| 4a | INFERENCE | `MeJepaCompiler::{compile, verify, observe, compile_with_panel}` + conformal interval + OOD detection + `Verdict::{Pass, Fail, OutOfDistribution, Abstain, GuardRejected}`. `mejepa_verify` MCP tool live | `context-graph-mejepa::compiler` |
| 4b | TCT-CONSTELLATION | TctConstellation + builder + `Gτ` guard + `Verdict::GuardRejected` + Bayesian shrinkage + freshness check + per-embedder threshold calibrator + per-chunk-type centroids + chunk-level localization (`gtau_check_chunks`). `mejepa_constellation_inspect` MCP tool live | `context-graph-mejepa-tct` |
| 5 | SELF-HEALING | 18-module `heal/` tree (`drift`, `ewc`, `promote`, `full_retrain`, `integrity`, `lora_refresh`, `calibration`, `distill`, `regulate`, `pipeline`, `drill`, `store`, `cf`, `fsv`, `scheduler`, `scheduler_state`). `mejepa-heal-drill` binary. Scheduler skeleton with 7 tickers and FSV. `mejepa_heal_status` + `mejepa_rollback_to` MCP tools | `context-graph-mejepa::heal` |
| 6 | HYGIENE | tier-compression + mincut foundation crates + new `context-graph-mejepa-hygiene` crate: tier transitions, quota + emergency eviction, witness Merkle compression of >90d segments, nightly GC orchestrator, JSON codec, MCP path discipline. 3 MCP tools live | `context-graph-mejepa-hygiene` |
| 7 | SHIFT-SUBSCRIBER | Restart-safe Tokio tailer (`run_until_idle`/`run_until_shutdown`), watermark CF, lag alert, panic supervision, latency telemetry, end-to-end shift→`shift_to_inference`→observe→prediction. 3 new MCP tools (`mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`) plus the previously-shipped `mejepa_predict_latest` | `context-graph-mejepa-shift-subscriber` |
| 8 | EVALUATION | Full `crates/context-graph-mejepa/src/eval/`: EvalRunner, ship_gate_failures, per-language conformal/OOD stratification, Bayesian shrinkage, ActiveLearningQueueState, PatchSimilarityGraph, weekly report writer, RocksDbEvalStore, 9 RocksDB CFs. 4 MCP tools live. `phase8_eval_fsv` writes 17 evidence files | `context-graph-mejepa::eval` |

**Live ME-JEPA MCP surface: 19 tools.** Per `docs/ruvectorfindings/specs/_README.md`:
`mejepa_verify`, `mejepa_predict_latest`, `mejepa_constellation_inspect`, `mejepa_heal_status`, `mejepa_rollback_to`, `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`, `mejepa_gc_run`, `mejepa_quota_status`, `mejepa_witness_compress`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`, `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`, `mejepa_audit_pairwise_mi`.

## 2. What is *partial* — code shipped, real-world capability lagging

These are the items the operator's prompt is asking about — they are the gap between "the model exists" and "the model is the truth system":

| Item | What's there | What's missing | GH issue |
|---|---|---|---|
| **Phase 0 corpus** — 300×8 batch | One-instance round-trip Docker oracle; all 300 SWE-bench Lite images prewarmed on the D-backed Docker store; corpus generation now rejects `/tmp` and any output outside `/mnt/d/contextgraph/corpus/` | Regenerate the 2,400-row source of truth on D-backed storage because the previously verified `/tmp` corpus was deleted after reboot; rerun readback/targeted Docker verify before using it as ship-gate evidence | [#5](https://github.com/ChrisRoyse/contextgraph/issues/5) |
| **Per-language mutation operators** | 11-language mutation dispatch exists for Rust, Python, JS, TS, Go, Java, C, C++, C#, Ruby, PHP | Broaden real toolchain FSV beyond Python before treating non-Python corpora as production training data | [#7](https://github.com/ChrisRoyse/contextgraph/issues/7) |
| **Neural per-embedder forward wrappers** | `EmbedderForward` trait + fail-closed `PretrainedEmbedderForward` load/forward path for E1/E6/E7/E8/E10/E12/E13/E14; model artifact digest/runtime checks; E10 forward FSV | Full production FSV for every supported D-root model artifact; cache layer; latency/VRAM telemetry under daemon load; integration into all shift→panel build paths | [#3](https://github.com/ChrisRoyse/contextgraph/issues/3) |
| **Analyzer/embedder-backed semantic instruments** | `EmbedderBackedInstrumentHead` maps validated E7/E1/E14/E10 outputs into code/text/trace panel slots with source-id checks and fail-closed analyzer-feature requirements | Real Pyright/mypy/rust-analyzer/javac/dotnet lattices for E_TypeGraph; real coverage Δ aggregator; production chunk pipeline feeding neural vectors into every semantic slot | [#4](https://github.com/ChrisRoyse/contextgraph/issues/4) |
| **Production chunk-level TCT centroids** | `gtau_check_chunks` algorithm + `Gτ` guard + chunk-level violation localization | Production-scale centroid refresh evidence against full corpus; operator-facing UX surfacing violating chunks | [#9](https://github.com/ChrisRoyse/contextgraph/issues/9) |
| **Live conformal/OOD/p50/p99 capture** | All scorers shipped; FSV against synthetic data | Live measurement under production traffic; weekly stratified report driven by real prediction stream | [#11](https://github.com/ChrisRoyse/contextgraph/issues/11) |
| **WSL GPU telemetry** | `vram.rs` accounting + nvidia-smi shell-out | Hardened `nvidia-smi` polling + WSL-specific GPU telemetry edge cases | [#10](https://github.com/ChrisRoyse/contextgraph/issues/10) |
| **Rolling-window holdout vs. ≥0.95 ship gate** | `ship_gate_failures` algorithm + `mejepa_ship_gate_status` MCP tool + per-language stratification | Production data points — actual holdout evaluations against real corpus at the cadence required to ship | [#5](https://github.com/ChrisRoyse/contextgraph/issues/5) |
| **Nightly GC scheduler in production** | `gc_run_nightly` orchestrator + `gc_stress_fsv` example proves 10k-entry stress run | systemd timer or cron entry that actually runs the GC every night at 03:00 with operator-visible reports | [#8](https://github.com/ChrisRoyse/contextgraph/issues/8) |
| **Continuous self-optimization scheduler skeleton** | `context_graph_mejepa::heal::run_self_optimization_scheduler` exists, daemon calls it, 7 tickers write readback-verified status and inspect real RocksDB CFs | Phase E full policy logic: per-cell stratified drift, autonomous A/B/C promotion decisions, production oracle binding | [`TASK-HEAL-001`](tasks/heal/_all_tasks.md) |
| **AST chunker convergence** | New `context_graph_core::memory::ast` API ships 11-language chunking; old `AstCodeChunker` still Rust-only | Converge legacy `AstCodeChunker` callers onto new API or delete | [#6](https://github.com/ChrisRoyse/contextgraph/issues/6) |
| **Reformat-invariant chunk SHAs** | Chunk SHA computed on text content + persisted | Audit that equivalent AST node IDs across reformatting produce equivalent chunk SHAs | [#12](https://github.com/ChrisRoyse/contextgraph/issues/12) |
| **UTML formula-carrier full reconciliation** | 4-factor delta computation shipped; `learning_signal/` modules in place | Reconcile simplified ΔP/ΔK/ΔΩ carriers in train certs against full doc-11 formulas | [#13](https://github.com/ChrisRoyse/contextgraph/issues/13) |

## 3. What does *not* exist — operational/integration work this directory plans

These are the items the futurebuild docs propose to add. None of them exist today:

| Capability | Why it doesn't exist | Where it's planned |
|---|---|---|
| **Live data flywheel** — every Claude Code Edit becomes a panel + corpus row + observe() call without operator intervention | Daemon mode, shift subscriber, scheduler skeleton, and weekly/GC scheduler tasks exist; remaining blockers are test-outcome capture, async oracle binding, prompt prediction injection, and agent-feedback persistence | [`07_TRAINING_DATA_FLYWHEEL.md`](07_TRAINING_DATA_FLYWHEEL.md), [`SPEC-FLYWHEEL.md`](specs/functional/SPEC-FLYWHEEL.md) |
| **Agent-facing live prediction injection** — `RealityPrediction` rendered into the agent's UserPromptSubmit context based on the latest panel | Hooks exist; no inject-from-MCP-state path for predictions | [`08_LIVE_PREDICTION_AGENT_API.md`](08_LIVE_PREDICTION_AGENT_API.md), [`SPEC-PREDICT.md`](specs/functional/SPEC-PREDICT.md) |
| **DDA enrichment wrapper** — emit n·(N+N²) signals per shift per chunk, persist to a new CF, expose to predictor as auxiliary panel slots | TCT crate has the centroids; DDA signal computation is one call site away but unwrapped | [`04_DDA_TCT_CODE_ENRICHMENT.md`](04_DDA_TCT_CODE_ENRICHMENT.md), [`SPEC-ENRICHMENT.md`](specs/functional/SPEC-ENRICHMENT.md) |
| **Refactored `loop.md`** — single outer loop around Claude Code, no Mode A/B split, focused on attempt-driven data capture | Current loop.md is bipartite (Mode A build / Mode B reality-capture) and references Phase 1-5 as deferred when they are SHIPPED | [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md), [`SPEC-LOOP.md`](specs/functional/SPEC-LOOP.md) |
| **Inner-loop residue deletion** — delete any remaining `reality-loop` binary code paths, Mode B drip references, ccreality runner shims | The `reality-loop` binary was deleted 2026-05-09 but the conceptual references remain in docs and hooks | [`11_DELETION_LIST_INNER_LOOP.md`](11_DELETION_LIST_INNER_LOOP.md) |
| **Self-healing scheduler full autonomy** — autonomous A/B/C promotion driven by drift detection on production traffic, no operator pull required | Phase A scheduler skeleton exists and is daemon-wired; full production policy loop is still Phase E | [`09_SELF_HEALING_AUTONOMY.md`](09_SELF_HEALING_AUTONOMY.md), [`SPEC-HEAL.md`](specs/functional/SPEC-HEAL.md) |
| **Live observability dashboard** — operator-facing weekly report + on-demand `mejepa_eval_run` invocations driven by real production data | Code paths shipped; no scheduled invocation; no operator-facing dashboard | [`SPEC-OBSERVABILITY.md`](specs/functional/SPEC-OBSERVABILITY.md) |
| **MeJEPA-as-MCP-server installation** — operator runs one command and the truth system stays up | MCP server, daemon mode, `scripts/ensure_daemon.sh`, and `scripts/systemd/mejepa-daemon.service` ship; missing first-time setup/runbook proof that an operator can enable it cleanly on a fresh machine | covered in [`12_IMPLEMENTATION_PHASES.md`](12_IMPLEMENTATION_PHASES.md) |

## 4. What is *deleted* (operational reality, not aspirational)

- **Inner-actor LLM (Qwen3.6-27B / Hermes-2-Pro / Qwen3-Coder MoE).** Deleted from disk 2026-05-09 per the pivot. **Do not reintroduce.**
- **`reality-loop` binary.** Removed 2026-05-09 ME-JEPA hardening pass. **Do not rebuild.**
- **`mcp__cgreality__reality_run_attempt` engine spawn path.** Fails closed with `CCREALITY_ENGINE_RETIRED`. **Do not reanimate.**
- **Path-policy gate in MCP server.** Removed 2026-05-07 by operator directive ("the AI needs to be able to do anything it needs to do"). **Path discipline is now behavioral** per CLAUDE.md §8.
- **All cost-cap / dollar-budget / `cost_cap_reached` blocked-status concepts.** Removed 2026-05-XX. **Do not reintroduce.**
- **Local-LLM model pin file `config/ccreality_model_pins.json` for inner actor.** Retired. **Do not reactivate.**

## 5. What the `tasks/swebench-lite/*.json` corpus is for

300 SWE-bench Lite Python task manifests. They are NOT for the inner LLM (which doesn't exist). They are:

- **The Phase 0 mutation generator's input** — each task's known-good fix is mutated 8 ways to produce the corpus.
- **The Phase 8 holdout split's input** — train/calibration/holdout (240/30/30) under InstanceAtomic split.
- **The reality-capture corpus seed** — Claude Code attempts these tasks one-at-a-time; every Edit becomes a panel; every test outcome becomes an oracle label.

The 300 manifests live on. They feed the data flywheel, not the dead inner-loop.

## 6. Verifiable bottom line

If you `cargo build --release --workspace` from a fresh clone of `ccreality-cutover` branch right now:

- **Builds.** Workspace compiles. ME-JEPA crates compile.
- **Tests.** `cargo test --workspace` passes (~6.5k+ tests on the consolidated tree).
- **MCP starts.** `cargo run --release -p context-graph-mcp` boots, exposes 19 ME-JEPA tools. `--daemon-server-only --d-root /mnt/d/contextgraph` starts the long-running daemon task set when D-root validation passes.
- **`mejepa_verify` answers.** Calling the tool with a synthetic patch returns a `Verdict` (likely `OutOfDistribution` because no production training has run yet — that's expected and correct).
- **`mejepa_predict_latest` answers.** Returns a deterministic placeholder verdict if no shift has been observed, or the real predicted panel if a shift has been observed via `mejepa_observe_shift`.

What does NOT happen out of the box:

- Predictions do not get *automatically* injected into Claude Code's prompts (no UserPromptSubmit-from-MCP-state path).
- The shift subscriber and self-optimization scheduler start under daemon mode, and a systemd template ships; the missing piece is operator-verified first-time installation/enablement on a fresh host.
- The 300×8 corpus is not currently present as durable D-backed source-of-truth data. A prior full run existed in `/tmp` with 2,400 verified rows and SHA `f5a413eadbb1fa08f7fa51fd05c704eebe7ba51789c3ac0194592825dc2dae33`, but it was lost after reboot. The rerun target is `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/`.
- The ship gate ≥ 0.95 is not actually met (because the predictor has not been trained on a fully-populated corpus).

The futurebuild plan in this directory turns the "what does NOT happen" list into the "what happens" list, *without* rebuilding any of the Phase 0–8 architecture.
