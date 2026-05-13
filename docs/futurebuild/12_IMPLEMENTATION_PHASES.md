# 12 — Implementation Phases

> **Purpose.** The phased rollout of the futurebuild plan. Each phase has a goal, deliverables (referenced by TASK-* IDs from `13_TASK_INDEX.md`), an effort estimate, dependencies, and a ship gate. Phases are designed so each can be merged independently and produces operator-visible value at completion.
>
> **Authority.** This document is the canonical execution roadmap; `13_TASK_INDEX.md` is the atomic task graph that implements it.

---

## Phase E entry rule

Phase E started when the Phase A-D substrate became source-of-truth verified: daemon/job-run ledgers, prediction/feedback surfaces, corpus roots, embedder forwards, DDA/TCT rows, semantic heads, production telemetry windows, and UTML carrier certs all had independent readback evidence. That gate was satisfied at implementation/FSV level on 2026-05-12, and Phase E self-optimization policy is code/FSV-shipped as of 2026-05-13.

This does **not** mean the system can replace human code review yet. The latest rolling holdout evidence reports `prediction_oracle_correlation = 0.866667` against the `0.95` ship gate. That is Phase G model-quality convergence. Phase E's job is now to keep feeding durable-reality reward signals back into ME-JEPA so the Phase G gate can eventually become true.

## Phase summary

| Phase | Goal | Effort | Dependencies | Ship gate |
|---|---|---|---|---|
| **A. Operational unblock** | Daemon mode + prediction injection + agent-feedback channel + D-drive enforcement | code/FSV closure 2026-05-12; fresh-host operator burn-in remains | none | Daemon task set runs; GC/eval job-run rows persist; first prediction injected; first SurpriseEvent recorded; cargo lint passes |
| **B. Multi-head extension** | Q1-Q5 `RealityPrediction` schema, what-if/explain APIs, auxiliary heads, deterministic label extractors, per-head UTML accounting | code + FSV complete 2026-05-12; training/eval hardening remains | Phase A | Code/FSV gate passed; production holdout metrics and remaining per-head self-healing tasks still open |
| **C. Corpus expansion** | Completed Python 300×8 corpus + 10-language fixture corpus + combined split | complete 2026-05-12 | Phase A | 2,400 Python entries verified; 5,120 non-Python entries verified; combined corpus FSV passes |
| **D. Neural embedder forwards** | Ship real forward/DDA enrichment substrate and remove placeholder-only Phase D paths | code/FSV substrate shipped 2026-05-12 | Phase A | Forward wrappers, full pretrained/routing FSV, DDA compute/store, live shift DDA rows, cache, stdlib registry, TCT refresh, semantic-head evidence, production telemetry, and UTML carrier FSV pass |
| **E. Continuous self-optimization** | Scheduler tokio task + per-cell stratified drift + dissipation detection + lambda ramp | **code/FSV shipped 2026-05-13** | Phase A-D substrate verified | Scheduler runs; drift detected within 1k obs; A/B/C promotion executes autonomously; catastrophic promotion requires two distinct approvals |
| **F. Production observability + runbook + DR + API** | Live conformal/OOD/p50/p99 capture + weekly eval + active-learning dashboard + WSL GPU telemetry hardening + operator CLI + storage restore/migrate + DR scripts + deprecation framework + integration tests | **code/FSV complete 2026-05-13** | Phase A | Weekly report writes; conformal coverage measured; active-learning queue is operator-actionable; operator CLI surface complete (`mejepa {active-learning, replay, daemon-status, report-weekly, hygiene, rollback, eval-ship-gate, storage-verify, storage restore-cf, storage migrate-cf, session-cleanup, pause}`); DR scripts ship with FSV; CI workflows (e2e/chaos/load/soak) live; remaining implementation issues = none |
| **G. Rolling-window holdout to ≥0.95** | Run live evaluations until ship gate stably met across (category × language) | ongoing | Phase B + C + D + E + F | 4 consecutive rolling windows ≥ 0.95 |
| **H. Inner-loop residue cleanup** | Apply deletion list, refactor loop.md, supersede SMAP files | 1-2 days | Phase A (so daemon is the new norm) | All `MEJEPA_*_RETIRED` symbols fail-closed; CLAUDE.md + agents/README.md updated |

**Cross-cutting domains added by `17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md`** distribute across the existing phases:

- **TESTING** (11 tasks, ~13 days) — most in Phase F; a few in Phase A (test-outcome capture hook is FLYWHEEL-003)
- **RUNBOOK** (12 tasks, ~5.5 days) — RUN-001 (first-time setup) + RUN-010 (config files) + RUN-011 (INSTALL.md) in Phase A; rest of CLI commands in Phase F
- **DR** (8 tasks, ~4 days) — DR-001/002/005/006 in Phase A (snapshot scripts + model digest list); DR-003/004/007/008 in Phase F
- **SECURITY** (8 tasks, ~3.5 days) — SEC-001 (secret redaction) + SEC-006/007/008 (markdown hardening / D-drive perms / hook lint) in Phase A; SEC-003 (multi-head contradiction) in Phase B; SEC-005 (two-person rule) in Phase E; SEC-002 (adversarial corpus) in Phase F
- **BOOTSTRAP** (3 tasks, ~2 days) — BOOT-001 + BOOT-003 in Phase A; BOOT-002 baseline checkpoint FSV is closed, with release packaging tracked by the readiness gate if it regresses
- **API** (5 tasks, ~2.5 days) — API-001 (semver in MCP) + API-005 (changelog) in Phase A; API-002/003/004 in Phase F

**Updated remaining fresh implementation effort:** Phase C no longer counts as remaining work. Phase A, the Phase B prediction/API/schema carrier, Phase C corpus expansion, Phase D enrichment substrate, Phase E autonomous self-optimization policy, and Phase F production operations are code/FSV-shipped. The remaining estimate is dominated by Phase G model-quality convergence and Phase H residue cleanup, not Phase F implementation.

**Current Phase A-E baseline (2026-05-13).** `TASK-FLYWHEEL-001/002/003/004/005/006`, `TASK-PREDICT-001/002`, `TASK-LOOP-001/007`, `TASK-OBS-005/006`, `TASK-RUN-001/010/011`, `TASK-BOOT-001/003`, and `TASK-SEC-001/006/007/008` are code-shipped or operationally applied. Daemon supervision now has durable `CF_MEJEPA_FLYWHEEL_JOB_RUNS` evidence plus GC/eval source-of-truth readback at `/mnt/d/contextgraph/fsv/mcp-daemon-supervision-fsv/daemon-supervision-evidence.json`. Phase B code/FSV ships `TASK-PREDICT-003/004/005/006/007/008`, `TASK-SEC-003`, deterministic `TASK-PREDICT-LABEL-001..009` coverage in one label module, and the core `TASK-UTML-001/002/003/004` accounting path with carrier evidence. Phase D ships active content forward wrappers, DDA compute/store/readback, live shift DDA rows, persistent cache behavior, stdlib registry, TCT refresh support, semantic analyzer/head evidence, production telemetry windows, and UTML cert readback. Phase E now ships the autonomous self-optimization policy layer with FSV at `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`. Phase G remains non-green until the 0.95 prediction-oracle correlation requirement is met.

---

## Phase A — Operational Unblock (5-7 days)

**Goal:** Turn the daemon on. Close the prediction-injection loop. Open the agent-feedback channel. Migrate to D-drive everywhere. Without Phase A, the truth system is theoretical — code shipped but not running as a service.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-FLYWHEEL-001` | MCP daemon mode with supervised tokio tasks + systemd template + durable job-run ledger | complete at code/FSV level; fresh-host enablement proof remains in runbook/install tasks |
| `TASK-FLYWHEEL-002` | Continuous self-optimization scheduler skeleton (7 tickers, calls into shipped modules) | complete |
| `TASK-FLYWHEEL-003` | Test-outcome capture hook (parse pytest/cargo-test/jest output) | complete (2026-05-12) |
| `TASK-FLYWHEEL-004` | Oracle-binding from captured test outcomes into `CF_MEJEPA_ORACLE_VERDICTS` | complete (2026-05-12 code path) |
| `TASK-FLYWHEEL-005` | ActiveLearningKind::AgentSurprise variant + queue extension | complete (2026-05-11) |
| `TASK-FLYWHEEL-006` | New CFs: `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK`, `CF_MEJEPA_FLYWHEEL_JOB_RUNS` | complete (2026-05-12) |
| `TASK-PREDICT-001` | UserPromptSubmit prediction-injection hook (script + `mejepa_predict_latest` call + markdown render) | complete (2026-05-12) |
| `TASK-PREDICT-002` | `mejepa_record_agent_feedback` MCP tool + sampling-weight bump wiring | complete (2026-05-12; no feedback rate limit by operator directive) |
| `TASK-LOOP-001` | Apply new `.claude/loop.md` content from `14_NEW_LOOP_MD.md` | complete (already applied) |
| `TASK-LOOP-002` | Update CLAUDE.md §11.11/§13/§3 + `agents/README.md` retirements | 0.25 day |
| `TASK-LOOP-007` | Hook output paths → `/mnt/d/contextgraph/runtime/cgreality-shift-log/` | complete |
| `TASK-OBS-005` | C-drive lint in CI | complete (2026-05-12) |
| `TASK-OBS-006` | FSV path migration sweep across `crates/*/examples/*_fsv.rs` | complete (2026-05-12) |
| `TASK-OBS-007` | Per-slice quota observability extension | 0.5 day |

**Ship gate:**
1. `cargo run --release -p context-graph-mcp -- --daemon` boots; `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` exists; `mejepa_subscriber_status` returns OK; daemon job starts/exits are written to `CF_MEJEPA_FLYWHEEL_JOB_RUNS`.
2. Make a synthetic Edit to a tracked file → `cgreality-shift-log` line lands → daemon logs "shift consumed in X ms" → `mejepa_predict_latest` returns a `RealityPrediction`.
3. Submit a synthetic prompt → UserPromptSubmit hook injects the prediction in markdown form (visible in agent context).
4. Call `mejepa_record_agent_feedback` with a synthetic SurpriseEvent → entry persisted to `CF_MEJEPA_AGENT_FEEDBACK` → active-learning queue length increments.
5. CI lint passes with no C-drive paths.
6. Daemon FSV proves scheduled GC and weekly eval complete, then flushes/drops/reopens RocksDB and reads back `CF_MEJEPA_FLYWHEEL_JOB_RUNS`, `CF_MEJEPA_GC_HISTORY`, and `CF_MEJEPA_EVAL_REPORTS`.

**Risks / mitigations:**
- *Risk:* daemon supervisor's panic restart logic flaps under transient errors. *Mitigation:* exponential backoff + `MEJEPA_SCHEDULER_FLAPPING` alert after 5 restarts in 5 min; manual cooldown.
- *Risk:* prediction-injection rendering exceeds the 4 KB cap. *Mitigation:* truncate Q4 concerns to top-5 per category by confidence; full predictions still queryable via `mejepa_predict_latest`.

---

## Phase B — Multi-Head Extension (code/FSV completed 2026-05-12; training gates remain)

**Goal:** Add the 8 new prediction heads (failure-mode, edge-case, tech-debt, perf, security, accuracy, cost, reasoning) per `03_ARCHITECTURE_BLUEPRINT.md §3` + `15_PREDICTION_TAXONOMY.md`. Wire their label sources. Train each head's holdout metric to beat random.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-PREDICT-003` | `mejepa_predict_what_if` MCP tool | **complete (2026-05-12; non-mutating disk SHA readback)** |
| `TASK-PREDICT-004` | `mejepa_explain_prediction` MCP tool | **complete (2026-05-12; persisted-row readback + normalized saliency)** |
| `TASK-PREDICT-005` | Multi-head stack (8 new heads + their per-head feature projections) | **code_complete (2026-05-12; auxiliary heads emit logits/probabilities; production DDA feature consumption still Phase D)** |
| `TASK-PREDICT-006` | Rich `RealityPrediction` schema per `15 §1` | **complete (2026-05-12; persisted through RocksDB FSV)** |
| `TASK-PREDICT-007` | Agent claim graph extractor (deterministic rule-based) | **complete (2026-05-12)** |
| `TASK-PREDICT-008` | Reality-impact reconciler (Q5 path) | **complete (2026-05-12)** |
| `TASK-UTML-001..004` | Per-head learning signal skeleton, cert readout, global `min_h` value, surprise sampling factor | **complete/code_complete (2026-05-12; BatchSampler policy hardening remains)** |
| `TASK-UTML-005..009` | Per-head Fisher, dormancy, lambda ramp, dissipation, per-cell T | **complete/code+FSV (2026-05-13)** — `TASK-UTML-009` evidence at `/mnt/d/contextgraph/fsv/utml-009-fsv/per_cell_state_transfer_fsv.json` |
| `TASK-PREDICT-LABEL-001..009` | Label-source extractors: failure trace classifier, edge/security/debt/perf/accuracy/cost/reasoning signals | **code_complete (2026-05-12 deterministic unified extractor; per-tool/per-language analyzers remain hardening work)** |

**Code/FSV gate passed 2026-05-12:**
1. `cargo run -p context-graph-mejepa --example phase_b_prediction_fsv` wrote a real `RealityPrediction` to `CF_MEJEPA_LIVE_PREDICTIONS` and read it back by session.
2. Evidence file: `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`.
3. Persisted prediction id: `78e5bc08a3e79d8d47487dc9efd7bd7f`.
4. Manual edge cases passed with no new DB rows: empty patch, path traversal hunk, malformed witness segment.
5. MCP readbacks passed: `mcp__cgreality__mejepa_predict_latest`, `mcp__cgreality__mejepa_explain_prediction`, and `mcp__cgreality__mejepa_predict_what_if`.

**Remaining Phase B-adjacent ship gate before declaring model quality complete:**
1. Each new head's holdout metric beats random baseline by ≥ 10 pp after real training.
2. Per-step training cert expands from persisted per-head L-step/global-min to all loss/delta/lambda values.
3. Production DDA/neural embedder features are consumed by the heads and shown to improve holdout quality, not merely persisted as side-channel evidence.
4. Per-head Fisher, dormancy, lambda ramp, dissipation, and per-cell T reporting are complete with Phase E/F FSV.

**Risks / mitigations:**
- *Risk:* auxiliary heads destabilize the trunk. *Mitigation:* per-head λ defaults are small (0.05); ramp gated on holdout improvement (per `10 §8`).
- *Risk:* label sources are noisy. *Mitigation:* per-head pre-training on the Phase 0 corpus (where labels are deterministic) before live data; conformal predictor acknowledges noise via interval width.

---

## Phase C — Corpus Completion (complete 2026-05-12)

**Goal:** Run the 300 × 8 SWE-bench Lite corpus through the Phase 0 generator. Port the 8 mutation operators to all 10 non-Python languages. Produce the seed training-data backbone for the multi-head predictor with source-of-truth readback.

**Completion state (2026-05-12):**
- Python corpus source of truth: `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` — 2,400 entries, 2,400 mutation CF rows, 2,400 verdict CF rows, 1 provenance row, Docker oracle verification passed.
- Non-Python corpus source of truth: `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` — 5,120 entries, 5,120 mutation CF rows, 5,120 verdict CF rows, 5,120 real-toolchain report files, 1 provenance row.
- Per non-Python language count: 512 entries each for Rust, JavaScript, TypeScript, Go, Java, C, C++, C#, Ruby, PHP.
- Combined split evidence: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json` — `passes=true`, 940 tasks, bucket sizes `[752, 94, 94]`, leakage passed.
- New CLI surface: `mejepa-corpus generate-fixtures`, `mejepa-corpus phase-c-verify`, and expanded `stats` language/category reporting.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-CORPUS-001` | Per-language mutation operators (Rust, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) — implemented through generic non-Python operator dispatch with parse-preserving candidate checks and real-toolchain validation | complete |
| `TASK-CORPUS-002` | 300 × 8 SWE-bench Lite Python corpus generation (Docker oracle, real evaluator) | complete: 2400/2400 at `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` |
| `TASK-CORPUS-003` | Per-language fixture corpora (64 deterministic fixture tasks per language × 8 categories) | complete: 5120/5120 at `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` |
| `TASK-CORPUS-004` | Combined train/calib/holdout split applied to Python + non-Python corpora | complete: `/mnt/d/contextgraph/fsv/corpus-fsv/split_full.json` |

**Ship gate:**
1. `mejepa_corpus_stats` reports 2,400 Python entries + ≥500 per-language entries per non-Python language with verified Docker (Python) or real-toolchain (others) oracle outcomes.
2. Train/calib/holdout split runs without leakage (verified by `assert_no_test_patch_leakage`).
3. Per-(category × language) cell sample counts ≥ 50 wherever physically possible (some cells may remain Bayesian-shrunk).
4. Python corpus source of truth resides under `/mnt/d/contextgraph/corpus/`; `/tmp` and C-drive corpus outputs fail closed before writing. `index.json`, `verify-evidence.json`, and `oracle-store/` RocksDB CF counts are read back after generation.

**Ship gate result:** passed 2026-05-12.

**Risks / mitigations:**
- *Risk:* Docker eval times are long (each task ~20-30s × 300 × 8 = ~16 hours wall time). *Mitigation:* parallelize across cores; checkpoint per row to the D-backed corpus root; write index/evidence files atomically; make reruns resumable from the RocksDB/index source of truth.
- *Risk:* corpus evidence is written to ephemeral storage and lost after reboot. *Mitigation:* corpus CLI and library generation reject any output outside `/mnt/d/contextgraph/corpus/`; storage docs and FSV commands name the D-backed source of truth explicitly.
- *Risk:* per-language operator implementations have subtle bugs. *Mitigation:* Phase C generation runs each non-Python row through the real language toolchain and fails closed on mismatch. This caught and fixed Rust arrow mutation, declaration-site swap, assertion substring deletion, C# helper deletion, and Go unused/local declaration failures before the final corpus was accepted.

---

## Phase D — Neural Embedder Forwards and DDA Substrate (code/FSV shipped 2026-05-12; consumed by Phase E)

**Goal:** Replace deterministic placeholder encoders with real forward outputs in the embedder routing layer and persist DDA enrichment signals. The forward/routing gap tracked in [#3](https://github.com/ChrisRoyse/contextgraph/issues/3), the DDA predictor-consumption gap tracked in [#16](https://github.com/ChrisRoyse/contextgraph/issues/16), the cache LRU/telemetry gap tracked in [#17](https://github.com/ChrisRoyse/contextgraph/issues/17), production telemetry gap tracked in [#11](https://github.com/ChrisRoyse/contextgraph/issues/11), analyzer-head gap tracked in [#4](https://github.com/ChrisRoyse/contextgraph/issues/4), and UTML-carrier gap tracked in [#13](https://github.com/ChrisRoyse/contextgraph/issues/13) are closed for the Phase A-D readiness scope.

**Code/FSV shipped 2026-05-12:**
- `EmbedderForward` now covers the 12 active content slots in the live registry: pretrained E1/E6/E7/E8/E10/E12/E13/E14 through `PretrainedEmbedderForward`, plus algorithmic E2/E3/E4/E9 through `AlgorithmicEmbedderForward`. E17 remains learner-state/affect, not a content embedder in the live code.
- Full forward FSV now runs all eight pretrained content slots, all four algorithmic content slots, all 11 E7 language-specific code inputs, and all 143 Language × EntityType routing rows with output-file readback evidence.
- Persistent `EmbedderCache` exists under `crates/context-graph-mejepa-embedders/src/cache.rs`, keyed by schema version, embedder id, model version, and chunk SHA-256. Corrupt cache entries fail closed instead of being treated as misses.
- `compute_dda_signals`, `persist_dda_signals`, `read_dda_signals`, and row counting exist in `crates/context-graph-mejepa-train/src/dda.rs`. The source-of-truth CF is `CF_MEJEPA_DDA_SIGNALS`, keyed by `(panel_id, chunk_id)`.
- Runtime inference RocksDB now opens `CF_MEJEPA_DDA_SIGNALS` and `CF_MEJEPA_FAILURE_EXEMPLARS`; this fixes the root cause where the CFs existed but the inference opener could not physically read/write them.
- TCT centroid refresh now has durable reward-signal evidence: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-tct-refresh-fsv/tct-centroid-refresh-evidence.json` proves `CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS` readback after reopen for 2,400 Python corpus rows, 10,560 AST-derived chunk-family observations, all 8 mutation categories, 1,008 centroid cells, and fail-closed edge cases.
- Semantic analyzer/head evidence now proves source-bound analyzer facts and real pretrained vectors for E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test, E_Trace, E_Runtime, and E_Reasoning. Source of truth: `/mnt/d/contextgraph/fsv/contextgraph-python-semantic-analyzers-fsv/python-semantic-analyzers-evidence.json` and `/mnt/d/contextgraph/fsv/contextgraph-mejepa-semantic-heads-fsv/semantic-heads-evidence.json`.
- Production telemetry windows now persist/readback `CF_MEJEPA_PRODUCTION_TELEMETRY` with conformal ECE, OOD AUC, Gτ pass rate, prediction-oracle agreement, ship-gate target/pass flag, p50/p95/p99/max latency, and VRAM free/required/total. Source of truth: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-production-telemetry-fsv/production-telemetry-evidence.json`.
- UTML carrier reconciliation now persists/readbacks `CF_MEJEPA_TRAIN_CERTS` with explicit ΔP/ΔK/ΔΩ/ΔΞ carriers, per-head L-step count, sampling-weight formula fields, and state-transfer evidence. Source of truth: `/mnt/d/contextgraph/fsv/contextgraph-utml-fsv/utml-fsv-summary.json`.
- The live shift subscriber now computes real algorithmic embedder outputs for each code hunk, persists DDA rows before prediction/watermark side effects, and readbacks the RocksDB rows through `persist_dda_signals`.
- The 11-language `ReinventedWheel` registry is tracked under `crates/context-graph-mejepa-instruments/assets/stdlib_registry/` and loaded through a schema-validating YAML parser.
- FSV evidence: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/forward-wrapper-evidence.json`, `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/routing-forward-resolution.json`, `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-signal-compute.json`, `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json`, `/mnt/d/contextgraph/fsv/phase-d-fsv/stdlib-registry-fsv.json`, and `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/`.

**Not a Phase D blocker:** the `0.95` prediction-oracle threshold is a Phase G convergence gate. Phase E begins before that gate so the self-optimization policy can generate the learning pressure needed to reach it.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-ENRICH-001` | `EmbedderForward` trait + real pretrained wrappers for E1/E6/E7/E8/E10/E12/E13/E14 and algorithmic wrappers for E2/E3/E4/E9 | **complete (2026-05-12; full pretrained/routing FSV passed)** |
| `TASK-ENRICH-002` | DDA signal computation per chunk with strict upper-triangle pairwise features and blind-spot z-scores | **complete (2026-05-12; FSV writes 30 RocksDB rows and reopens/readbacks)** |
| `TASK-ENRICH-003` | New CFs from Phase A's `TASK-FLYWHEEL-006` opened by inference runtime and populated through DDA persistence API | **complete (2026-05-12; live algorithmic DDA rows FSV writes/readbacks 100 shifts)** |
| `TASK-ENRICH-004` | Embedder cache layer keyed by `(schema, embedder_id, model_version, chunk_sha)` for re-edit scenarios | **complete (2026-05-12; persistent cache, corrupt-entry fail-closed, LRU/telemetry evidence closed in #17)** |
| `TASK-ENRICH-005` | VRAM budget gate enforcement at startup (refuse to start over budget) | complete for Phase A-D telemetry capture; long-running trend dashboards move to Phase F |
| `TASK-ENRICH-006` | `EmbedderForward` FSV per embedder (real model file, deterministic input → expected output) | **complete (2026-05-12; 8 pretrained + 4 algorithmic + 143 routing rows read back)** |
| `TASK-ENRICH-007` | Per-language stdlib registry for `ReinventedWheel` detection | **complete (2026-05-12; 11 tracked YAMLs + FSV)** |

**Ship gate:**
1. All 12 active content embedders return valid real outputs when called via `EmbedderForward::forward(chunk)`; pretrained slots use SHA-pinned model artifacts and algorithmic slots use their real mathematical implementations. **Passed 2026-05-12.**
2. Routing FSV runs all 11 languages × 13 entity types and confirms each route resolves to a read-back forward output file. **Passed 2026-05-12.**
3. DDA FSV writes and reopens `CF_MEJEPA_DDA_SIGNALS` with 30 chunk rows. **Passed 2026-05-12.**
4. VRAM peak under load < 10 GB (10× headroom on RTX 5090). **Budget gate exists and production telemetry windows persist CUDA-driver VRAM readback.**
5. Cache hit rate > 90% for re-Edit scenarios on the same chunk. **Cache hit/LRU telemetry FSV closed in #17.**

**Risks / mitigations:**
- *Risk:* Candle / CUDA model loading is slow; daemon startup balloons. *Mitigation:* lazy-load embedders on first use; the warm daemon keeps them resident.
- *Risk:* per-embedder model artifacts not present at the D-drive paths. *Mitigation:* SHA-pinned digest verification at startup; fail-closed `MEJEPA_EMBED_DIGEST_MISMATCH`, `MEJEPA_EMBED_WEIGHT_MISSING`, or `MEJEPA_EMBED_FORWARD_FAILED` with artifact context.

---

## Phase E — Continuous Self-Optimization (code/FSV shipped 2026-05-13)

**Goal:** Make the heal scheduler autonomous. Phase E replaces the earlier conservative inspection/action handlers with the full per-cell stratified, dissipation-aware, lambda-ramping autonomous learner. The scheduler now treats durable RocksDB rows and the scheduler status file as source of truth, not tool return values.

**Deliverables (TASK IDs):**

| Task | Description | Status |
|---|---|---|
| `TASK-HEAL-001` | Self-optimization scheduler full implementation (drift, active-learning, promote, continual_backprop, constellation freshness ticks) | complete/code+FSV |
| `TASK-HEAL-002` | Per-cell stratified drift detection | complete/code+FSV |
| `TASK-HEAL-003` | Dissipation detection (training falls, holdout doesn't) | complete/code+FSV |
| `TASK-HEAL-004` | Bayesian small-window drift (Jeffreys/Beta posterior on per-cell sample counts < 50) | complete/code+FSV |
| `TASK-HEAL-005` | Production constellation refresh policy | complete/code+FSV |
| `TASK-HEAL-006` | `mejepa_promote_approval` MCP tool (two-distinct-operator approval for catastrophic promotions) | complete/code+FSV |
| `TASK-HEAL-007` | Emergency eviction policy validation | complete/code+FSV |
| `TASK-HEAL-008` | Drift attribution and operator alert policy stream | complete/code+FSV |

**Ship gate:**
1. Controlled soft-drift window (`860/1000`) is persisted to `CF_MEJEPA_DRIFT_WINDOW`; scheduler reads it back, writes `CF_MEJEPA_HEAL_REPORTS`, and persists A/B/C policy records in `CF_MEJEPA_MODEL_PROMOTIONS`.
2. Controlled catastrophic-drift window (`750/1000`) queues full retrain approval, writes an operator alert, and does not execute promotion until two distinct `mejepa_promote_approval` approvals are durably read back.
3. Per-head dormancy, dissipation, lambda ramp, Fisher snapshot, active-learning surprise, emergency eviction, per-cell promotion rejection, and invalid approval edge cases are verified by independent source-of-truth reads in `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`.

**Risks / mitigations:**
- *Risk:* over-eager promotion regresses some cells while improving others. *Mitigation:* per-(category × language) acceptance criterion in promotion gate (every cell must hold or improve; not just the average).
- *Risk:* full retrain takes hours. *Mitigation:* run during off-peak; predictor's old weights stay live until retrain completes; A/B/C is offline.

---

## Phase F — Production Observability + Runbook + DR + API (complete 2026-05-13)

**Goal:** Make the system legible and operable: weekly evaluation report, live conformal/OOD/p50/p99 capture, per-axis regression summaries, WSL GPU telemetry hardening, operator CLI, DR restore/drill tooling, deprecation framework, and integration/chaos/load/soak coverage.

**Status (2026-05-13):** complete at code/FSV level. The expanded Phase F tracker has 0 open Phase F implementation issues after closure of the integration-test harnesses, DR/API storage CLIs, and follow-up consumer-wiring tasks. GitHub `phase:f-followup` is empty. See `docs/futurebuild/13_TASK_INDEX.md` for the task ledger and artifact hashes.

**Delivered surface:**

| Area | Delivered |
|---|---|
| Observability | weekly dashboard/report, MI heatmap, daemon status, per-head calibration, WSL GPU hardening, `mejepa_inspect_prediction`, reasoning/executive renderers, health/readiness probes, system-cost counters |
| Runbook CLI | `active-learning`, `replay`, `daemon-status`, `report-weekly`, `hygiene`, `rollback`, `eval-ship-gate`, `storage-verify`, `storage restore-cf`, `storage migrate-cf`, `session-cleanup`, `pause` |
| DR/API/security | storage verify/restore/migrate, monthly restore verification, quarterly DR drill, deprecation framework, workflow SHA pin audit, witness-chain quarantine, retention policy, agent-identity attestation |
| Testing | happy-path, restart-safety, multi-agent, catastrophic-event, storage-exhaustion, chaos failpoints, load, soak, toolchain fail-closed, adversarial corpus, and CI workflow discovery |

**Ship gate result:** passed at code/FSV level. Weekly reports and dashboards write/read their SoTs; `mejepa_ship_gate_status` reports the still-non-green Phase G convergence gate; pairwise MI renders; GPU telemetry handles absent/malformed `nvidia-smi`; operator CLI commands read/write only declared SoTs; DR scripts have restore/drill evidence; integration harnesses write independent FSV artifacts.

---

## Phase G — Ship-Gate Convergence (ongoing)

**Goal:** Run the system continuously, capture data, observe drift, retrain as needed, until `prediction_oracle_correlation ≥ 0.95` per (category × language) is stable across 4 consecutive rolling windows.

This phase is **operational, not implementation**. The system is built; this phase runs it. Estimated wall-clock: weeks to months depending on operator session intensity. Per `01_VISION_TRUTH_SYSTEM.md §8`, success is observable from the weekly report; failure is observable as sustained gate misses → diagnosis → retrain → re-evaluate.

**No new tasks. The system operates itself.**

---

## Phase H — Inner-Loop Residue Cleanup (1-2 days)

**Goal:** Apply the deletion list from `11_DELETION_LIST_INNER_LOOP.md`. Cosmetic but important — keeps the codebase aligned with the new reality.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-LOOP-003` | Mark superseded SMAP decisions with frontmatter | **complete (#132)** — `scripts/ci/phase_h_smap_superseded_decisions_fsv.sh` |
| `TASK-LOOP-004` | Retire legacy MCP tools (mark fail-closed, update tool registry) | 0.25 day |
| `TASK-LOOP-005` | Confirm absence of inner-actor model files (audit script) | 0.1 day |
| `TASK-LOOP-006` | Delete stale `target/release/reality-loop` binary if present | 0.1 day |

**Ship gate:**
1. No `MEJEPA_*_RETIRED` symbols accidentally invokable.
2. SMAP supersession is consistent (no active decision asserts the retired inner-actor contract; legacy decisions have `superseded_by` pointers). **Passed for TASK-LOOP-003 on 2026-05-13.**
3. CLAUDE.md + `agents/README.md` reflect current state.

---

## Cross-phase parallelism

Recommended order (with parallelism):

```
Phase A    [days 1–7]    ============
Phase B    [days 4–18]       =====================
Phase C    [complete 2026-05-12] ================= closed
Phase D    [days 8–22]            ===================
Phase E    [complete 2026-05-13]       ========== closed
Phase F    [complete 2026-05-13]          =========== closed
Phase H    [days 21–22]                          ==
Phase G                                            ──── ongoing
```

Phases A → B; A → C; A → D; then Phase E shipped from the verified A-D substrate. Phase F is now closed at code/FSV level. Phase G never gates Phase E or Phase F because it is the convergence phase; its non-green `0.866667 < 0.95` evidence blocks only the human-review-replacement claim.

**Most aggressive single-engineer schedule:** Phases A-F are shipped; remaining implementation work is Phase H residue cleanup, with G running until the 0.95 gate is stable.

---

## Per-phase risks summary

| Phase | Top risk | Mitigation |
|---|---|---|
| A | Daemon supervision flapping | exponential backoff + `MEJEPA_SCHEDULER_FLAPPING` alert |
| B | Auxiliary heads destabilize trunk | small λ defaults + ramp gated on metric improvement |
| C | Completed 2026-05-12; future corpus broadening can regress operators | real-toolchain reports + fail-closed generation gate + checkpointed source-of-truth readback |
| D | VRAM exhaustion under all-12-loaded | budget gate + lazy load + Candle CUDA precision tuning |
| E | Over-eager promotion regresses cells | per-cell promotion criterion |
| F | Completed 2026-05-13; future risk is cron/report drift | use UTC + explicit calendar weekly boundary + FSV reruns |
| G | Ship gate never stably reached | active learning + operator gold labels + LoRA refresh + dissipation detection — all already shipped |
| H | Accidental deletion of load-bearing artifact | per-item action class (DELETE/SUPERSEDE/RETIRE/KEEP) explicit in `11` |

---

## Milestones an operator can point to

- **End of Phase A**: "ME-JEPA is now a service. Edit a file → see prediction in next prompt → record feedback if wrong."
- **End of Phase B**: "Predictions now cover Q1–Q5; agent gets 'what's wrong' for every change, even on Pass."
- **End of Phase C** (reached 2026-05-12): "Corpus is full; per-language operators ship; ME-JEPA can train against the real distribution."
- **End of Phase D**: "Embedders are real neural forwards; DDA signal substrate is live."
- **End of Phase E** (reached 2026-05-13): "Self-healing actually heals; operator approves only catastrophic promotions."
- **End of Phase F** (reached 2026-05-13): "Operator has the dashboard, runbook CLI, DR/API tooling, and integration harnesses needed to operate ME-JEPA."
- **End of Phase H**: "No inner-loop residue; codebase is post-pivot clean."
- **Phase G stable**: "ME-JEPA prediction-oracle correlation ≥ 0.95 across all cells over 4 rolling windows. Code review of AI patches is statistically unjustified."
