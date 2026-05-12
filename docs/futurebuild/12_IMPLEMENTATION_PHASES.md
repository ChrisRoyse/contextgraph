# 12 — Implementation Phases

> **Purpose.** The phased rollout of the futurebuild plan. Each phase has a goal, deliverables (referenced by TASK-* IDs from `13_TASK_INDEX.md`), an effort estimate, dependencies, and a ship gate. Phases are designed so each can be merged independently and produces operator-visible value at completion.
>
> **Authority.** This document is the canonical execution roadmap; `13_TASK_INDEX.md` is the atomic task graph that implements it.

---

## Phase summary

| Phase | Goal | Effort | Dependencies | Ship gate |
|---|---|---|---|---|
| **A. Operational unblock** | Daemon mode + prediction injection + agent-feedback channel + D-drive enforcement | code closure 2026-05-12; operator burn-in remains | none | Daemon runs; first prediction injected; first SurpriseEvent recorded; cargo lint passes |
| **B. Multi-head extension** | Q1-Q5 `RealityPrediction` schema, what-if/explain APIs, auxiliary heads, deterministic label extractors, per-head UTML accounting | code + FSV complete 2026-05-12; training/eval hardening remains | Phase A | Code/FSV gate passed; production holdout metrics and remaining per-head self-healing tasks still open |
| **C. Corpus expansion** | Completed Python 300×8 corpus + 10-language fixture corpus + combined split | complete 2026-05-12 | Phase A | 2,400 Python entries verified; 5,120 non-Python entries verified; combined corpus FSV passes |
| **D. Neural embedder forwards** | Replace deterministic placeholders with real neural forward passes | 10-15 days | Phase A | All 12 active embedders run real forward; routing FSV passes; pairwise MI matrix updates |
| **E. Continuous self-optimization** | Scheduler tokio task + per-cell stratified drift + dissipation detection + lambda ramp | 5-7 days | Phase A + Phase B | Scheduler runs; drift detected within 1k obs; A/B/C promotion executes autonomously |
| **F. Production observability** | Live conformal/OOD/p50/p99 capture + weekly eval + active-learning dashboard + WSL GPU telemetry hardening | 5-7 days | Phase A | Weekly report writes; conformal coverage measured; active-learning queue cleared by operator weekly |
| **G. Rolling-window holdout to ≥0.95** | Run live evaluations until ship gate stably met across (category × language) | ongoing | Phase B + C + D + E + F | 4 consecutive rolling windows ≥ 0.95 |
| **H. Inner-loop residue cleanup** | Apply deletion list, refactor loop.md, supersede SMAP files | 1-2 days | Phase A (so daemon is the new norm) | All `MEJEPA_*_RETIRED` symbols fail-closed; CLAUDE.md + agents/README.md updated |

**Cross-cutting domains added by `17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md`** distribute across the existing phases:

- **TESTING** (11 tasks, ~13 days) — most in Phase F; a few in Phase A (test-outcome capture hook is FLYWHEEL-003)
- **RUNBOOK** (12 tasks, ~5.5 days) — RUN-001 (first-time setup) + RUN-010 (config files) + RUN-011 (INSTALL.md) in Phase A; rest of CLI commands in Phase F
- **DR** (8 tasks, ~4 days) — DR-001/002/005/006 in Phase A (snapshot scripts + model digest list); DR-003/004/007/008 in Phase F
- **SECURITY** (8 tasks, ~3.5 days) — SEC-001 (secret redaction) + SEC-006/007/008 (markdown hardening / D-drive perms / hook lint) in Phase A; SEC-003 (multi-head contradiction) in Phase B; SEC-005 (two-person rule) in Phase E; SEC-002 (adversarial corpus) in Phase F
- **BOOTSTRAP** (3 tasks, ~2 days) — BOOT-001 + BOOT-003 in Phase A; BOOT-002 (baseline checkpoint) remains open release-prep work, now unblocked by the completed Phase C corpus
- **API** (5 tasks, ~2.5 days) — API-001 (semver in MCP) + API-005 (changelog) in Phase A; API-002/003/004 in Phase F

**Updated remaining fresh implementation effort:** Phase C no longer counts as remaining work. Phase A, the Phase B prediction/API/schema carrier, and Phase C corpus expansion are code/FSV-shipped; the remaining estimate is dominated by real neural/DDA enrichment, autonomous self-optimization policy, observability, DR, bootstrap checkpoint release prep, and production hardening.

**Current Phase A/B baseline (2026-05-12).** `TASK-FLYWHEEL-001/002/003/004/005/006`, `TASK-PREDICT-001/002`, `TASK-LOOP-001/007`, `TASK-OBS-005/006`, `TASK-RUN-001/010/011`, `TASK-BOOT-001/003`, and `TASK-SEC-001/006/007/008` are code-shipped or operationally applied. Phase B code/FSV now ships `TASK-PREDICT-003/004/005/006/007/008`, `TASK-SEC-003`, deterministic `TASK-PREDICT-LABEL-001..009` coverage in one label module, and the core `TASK-UTML-001/002/003/004` accounting path. Phase E still owns autonomous policy logic, per-head Fisher/dormancy/lambda/dissipation, and production holdout gates.

---

## Phase A — Operational Unblock (5-7 days)

**Goal:** Turn the daemon on. Close the prediction-injection loop. Open the agent-feedback channel. Migrate to D-drive everywhere. Without Phase A, the truth system is theoretical — code shipped but not running as a service.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-FLYWHEEL-001` | MCP daemon mode with supervised tokio tasks + systemd template | complete at code/FSV level; fresh-host enablement proof remains in runbook/install tasks |
| `TASK-FLYWHEEL-002` | Continuous self-optimization scheduler skeleton (7 tickers, calls into shipped modules) | complete |
| `TASK-FLYWHEEL-003` | Test-outcome capture hook (parse pytest/cargo-test/jest output) | complete (2026-05-12) |
| `TASK-FLYWHEEL-004` | Oracle-binding from captured test outcomes into `CF_MEJEPA_ORACLE_VERDICTS` | complete (2026-05-12 code path) |
| `TASK-FLYWHEEL-005` | ActiveLearningKind::AgentSurprise variant + queue extension | complete (2026-05-11) |
| `TASK-FLYWHEEL-006` | New CFs: `CF_MEJEPA_DDA_SIGNALS`, `CF_MEJEPA_FAILURE_EXEMPLARS`, `CF_MEJEPA_AGENT_FEEDBACK` | complete (2026-05-11) |
| `TASK-PREDICT-001` | UserPromptSubmit prediction-injection hook (script + `mejepa_predict_latest` call + markdown render) | complete (2026-05-12) |
| `TASK-PREDICT-002` | `mejepa_record_agent_feedback` MCP tool + sampling-weight bump wiring | complete (2026-05-12; no feedback rate limit by operator directive) |
| `TASK-LOOP-001` | Apply new `.claude/loop.md` content from `14_NEW_LOOP_MD.md` | complete (already applied) |
| `TASK-LOOP-002` | Update CLAUDE.md §11.11/§13/§3 + `agents/README.md` retirements | 0.25 day |
| `TASK-LOOP-007` | Hook output paths → `/mnt/d/contextgraph/runtime/cgreality-shift-log/` | complete |
| `TASK-OBS-005` | C-drive lint in CI | complete (2026-05-12) |
| `TASK-OBS-006` | FSV path migration sweep across `crates/*/examples/*_fsv.rs` | complete (2026-05-12) |
| `TASK-OBS-007` | Per-slice quota observability extension | 0.5 day |

**Ship gate:**
1. `cargo run --release -p context-graph-mcp -- --daemon` boots; `/mnt/d/contextgraph/state/locks/context-graph-daemon.pid` exists; `mejepa_subscriber_status` returns OK.
2. Make a synthetic Edit to a tracked file → `cgreality-shift-log` line lands → daemon logs "shift consumed in X ms" → `mejepa_predict_latest` returns a `RealityPrediction`.
3. Submit a synthetic prompt → UserPromptSubmit hook injects the prediction in markdown form (visible in agent context).
4. Call `mejepa_record_agent_feedback` with a synthetic SurpriseEvent → entry persisted to `CF_MEJEPA_AGENT_FEEDBACK` → active-learning queue length increments.
5. CI lint passes with no C-drive paths.

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
| `TASK-UTML-005..009` | Per-head Fisher, dormancy, lambda ramp, dissipation, per-cell T | **open; moves with Phase E/F policy hardening** |
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
3. Production DDA/neural embedder features are consumed by the heads, not only deterministic patch-surface heuristics.
4. Per-head Fisher, dormancy, lambda ramp, dissipation, and per-cell T are completed under Phase E/F.

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

## Phase D — Neural Embedder Forwards (10-15 days)

**Goal:** Replace deterministic placeholder encoders with real neural forwards in the embedder routing layer. This is the [#3](https://github.com/ChrisRoyse/contextgraph/issues/3) gap.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-ENRICH-001` | `EmbedderForward` trait + per-embedder Candle/CUDA implementation for E1, E7, E8 (graph-attention adapter), E10, E12, E14, E17 | 4-5 days |
| `TASK-ENRICH-002` | DDA signal computation (n × N + N²) per chunk per shift | 1 day |
| `TASK-ENRICH-003` | New CFs from Phase A's `TASK-FLYWHEEL-006` populated with real signals | 0.25 day |
| `TASK-ENRICH-004` | Embedder cache layer (key = `(embedder_id, chunk_sha)`) for re-edit scenarios | 1 day |
| `TASK-ENRICH-005` | VRAM budget gate enforcement at startup (refuse to start over budget) | 0.5 day |
| `TASK-ENRICH-006` | `EmbedderForward` FSV per embedder (real model file, deterministic input → expected output) | 2-3 days (one FSV per embedder) |
| `TASK-ENRICH-007` | Per-language stdlib registry for `ReinventedWheel` detection | 1 day |

**Ship gate:**
1. All 12 active content embedders return non-deterministic vectors when called via `EmbedderForward::forward(chunk)`.
2. Routing FSV runs all 11 languages × 13 entity types and confirms each route returns a valid vector.
3. VRAM peak under load < 10 GB (10× headroom on RTX 5090).
4. Cache hit rate > 90% for re-Edit scenarios on the same chunk.

**Risks / mitigations:**
- *Risk:* Candle / CUDA model loading is slow; daemon startup balloons. *Mitigation:* lazy-load embedders on first use; the warm daemon keeps them resident.
- *Risk:* per-embedder model artifacts not present at the D-drive paths. *Mitigation:* SHA-pinned digest verification at startup; fail-closed `MEJEPA_EMBEDDER_DIGEST_MISMATCH`.

---

## Phase E — Continuous Self-Optimization (5-7 days)

**Goal:** Make the heal scheduler autonomous. The Phase A `TASK-FLYWHEEL-002` skeleton already fires seven tickers, delegates to shipped modules, persists readback-verified status, and is called by daemon mode. Phase E replaces the conservative inspection/action handlers with the full per-cell stratified, dissipation-aware, lambda-ramping autonomous learner.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-HEAL-001` | Self-optimization scheduler full implementation (drift, active-learning, promote, continual_backprop, constellation freshness ticks) | 1.5 days |
| `TASK-HEAL-002` | Per-cell stratified drift detection | 1 day |
| `TASK-HEAL-003` | Dissipation detection (training falls, holdout doesn't) | 1 day |
| `TASK-HEAL-004` | Bayesian small-window drift (Beta posterior on per-cell sample counts < 50) | 0.5 day |
| `TASK-HEAL-005` | Production constellation refresh policy (scheduler tick exists; refresh thresholds/actions still need Phase E policy) | 0.5 day |
| `TASK-HEAL-006` | `mejepa_promote_approval` MCP tool (operator approval for catastrophic promotions) | 0.5 day |
| `TASK-HEAL-007` | Emergency eviction policy validation (scheduler tick exists; production quota thresholds/reporting still need hardening) | 0.5 day |

**Ship gate:**
1. Inject synthetic drift (90% → 84% over 1000 obs); scheduler detects within 1k more obs; queues LoRA refresh; A/B/C runs; winner promoted.
2. Inject catastrophic drift (90% → 75%); scheduler queues full retrain + writes operator alert; promotion blocked until `mejepa_promote_approval` called.
3. Inject dormancy (force 20% units to zero pre-activation); detector fires; reinit runs; re-eval shows recovery.

**Risks / mitigations:**
- *Risk:* over-eager promotion regresses some cells while improving others. *Mitigation:* per-(category × language) acceptance criterion in promotion gate (every cell must hold or improve; not just the average).
- *Risk:* full retrain takes hours. *Mitigation:* run during off-peak; predictor's old weights stay live until retrain completes; A/B/C is offline.

---

## Phase F — Production Observability (5-7 days)

**Goal:** Make the system legible to the operator. Weekly evaluation report. Live conformal coverage tracking. Per-axis perf-regression histograms. WSL GPU telemetry hardening.

**Deliverables (TASK IDs):**

| Task | Description | Effort |
|---|---|---|
| `TASK-OBS-001` | Weekly evaluation production feed/dashboard/runbook (daemon scheduler + report writer exist; ship-gate per-cell + active-learning + agent-feedback summaries still need production data) | 1 day |
| `TASK-OBS-002` | Pairwise MI heatmap renderer (CSV → PNG/markdown) | 0.5 day |
| `TASK-OBS-003` | Daemon status aggregator MCP tool | 0.5 day |
| `TASK-OBS-004` | Per-prediction-class running calibration tracker (live ECE per head) | 1 day |
| `TASK-OBS-008` | WSL GPU telemetry hardening (nvidia-smi NA-sentinel diagnostics, retry, fail-closed malformed output handling) — addresses [#10](https://github.com/ChrisRoyse/contextgraph/issues/10) | 1-2 days |
| `TASK-OBS-009` | Per-prediction observability: `mejepa_inspect_prediction` returns full provenance trace (witness_hash, predictor version, calibration version, contributing chunks) | 0.5 day |
| `TASK-OBS-010` | Active-learning operator CLI (`cargo run -p context-graph-cli mejepa active-learning`) | 0.5 day |

**Ship gate:**
1. Weekly cron writes a report at `/mnt/d/contextgraph/exports/eval/<date>/weekly.md`; operator can read it.
2. `mejepa_ship_gate_status` returns the per-cell status table.
3. Pairwise MI heatmap rendered weekly.
4. WSL GPU telemetry no longer reports `N/A` spuriously.

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
| `TASK-LOOP-003` | Mark superseded SMAP decisions with frontmatter | 0.25 day |
| `TASK-LOOP-004` | Retire legacy MCP tools (mark fail-closed, update tool registry) | 0.25 day |
| `TASK-LOOP-005` | Confirm absence of inner-actor model files (audit script) | 0.1 day |
| `TASK-LOOP-006` | Delete stale `target/release/reality-loop` binary if present | 0.1 day |

**Ship gate:**
1. No `MEJEPA_*_RETIRED` symbols accidentally invokable.
2. SMAP supersession is consistent (no live reference to a retired concept without a `superseded_by` pointer).
3. CLAUDE.md + `agents/README.md` reflect current state.

---

## Cross-phase parallelism

Recommended order (with parallelism):

```
Phase A    [days 1–7]    ============
Phase B    [days 4–18]       =====================
Phase C    [complete 2026-05-12] ================= closed
Phase D    [days 8–22]            ===================
Phase E    [days 12–18]                ============
Phase F    [days 15–21]                   ===========
Phase H    [days 21–22]                          ==
Phase G                                            ──── ongoing
```

Phases A → B → E (each waits on the prior); A → C → G; A → D → G; A → F → G; A → H. Phase A is the gate for everything else; Phase G never gates because it's continuous.

**Most aggressive single-engineer schedule:** ~3-4 weeks to all-shipped (Phases A-F + H), with G running indefinitely after week 1.

---

## Per-phase risks summary

| Phase | Top risk | Mitigation |
|---|---|---|
| A | Daemon supervision flapping | exponential backoff + `MEJEPA_SCHEDULER_FLAPPING` alert |
| B | Auxiliary heads destabilize trunk | small λ defaults + ramp gated on metric improvement |
| C | Completed 2026-05-12; future corpus broadening can regress operators | real-toolchain reports + fail-closed generation gate + checkpointed source-of-truth readback |
| D | VRAM exhaustion under all-12-loaded | budget gate + lazy load + Candle CUDA precision tuning |
| E | Over-eager promotion regresses cells | per-cell promotion criterion |
| F | Weekly cron drift between machine clock and quota window | use UTC + explicit calendar weekly boundary |
| G | Ship gate never stably reached | active learning + operator gold labels + LoRA refresh + dissipation detection — all already shipped |
| H | Accidental deletion of load-bearing artifact | per-item action class (DELETE/SUPERSEDE/RETIRE/KEEP) explicit in `11` |

---

## Milestones an operator can point to

- **End of Phase A**: "ME-JEPA is now a service. Edit a file → see prediction in next prompt → record feedback if wrong."
- **End of Phase B**: "Predictions now cover Q1–Q5; agent gets 'what's wrong' for every change, even on Pass."
- **End of Phase C** (reached 2026-05-12): "Corpus is full; per-language operators ship; ME-JEPA can train against the real distribution."
- **End of Phase D**: "Embedders are real neural forwards; DDA signal substrate is live."
- **End of Phase E**: "Self-healing actually heals; operator approves only catastrophic promotions."
- **End of Phase F**: "Operator has a dashboard; weekly report writes; active-learning queue surfaces."
- **End of Phase H**: "No inner-loop residue; codebase is post-pivot clean."
- **Phase G stable**: "ME-JEPA prediction-oracle correlation ≥ 0.95 across all cells over 4 rolling windows. Code review of AI patches is statistically unjustified."
