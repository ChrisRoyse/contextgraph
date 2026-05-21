# futurebuild — ME-JEPA-Code Reference

**Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8.

Source code: `/home/cabdru/contextgraph/crates/` (22 active workspace crates).
Authority hierarchy: **universal doctrine** (`docs2/AICodingAgentSuperPrompt.md`) > **project addendum** (`CLAUDE.md`) > **this suite**.

---

## ⚠ Synthetic-vector remediation (2026-05-19 / 20) — read first

Between Phase G ramp and 2026-05-19 22:45 UTC, the **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of calling the real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452, all closed 2026-05-19/20) document and fix this contamination across the inference panel, project cache, FSV deterministic harness, project ingest, trained-checkpoint pipeline, trainer weight update, and native active-embedder adapter.

**What survived the bug:**

- 399,777 AST chunks + 391-shard packed forward cache + 12 active embedders (real Candle CUDA forward; ~24h GPU run; SHA-pinned safetensors; fail-closed `"no deterministic vector fallback is permitted"` guards in `crates/context-graph-mejepa-embedders/src/forward.rs`)
- Anything deterministically derived from the real cache: #388 cross-embedder tensors, #389 per-slot calibration, #413 Level-0 labels + 491-row accepted registry, #414 skill substrate (128 / 36,773 / 10,579), #404 hierarchy (8 / 157 / 379)
- All structural / invariant work: slot-preserving doctrine, anti-compensation, ambiguity boundary, mistake-loop machinery, runtime ability resolver

**What is invalidated:**

- The **0.866667** ship-gate measurement (came from inference path through synthetic vectors + untrained predictor + `MEJEPA_BEST_STUB_V1` checkpoint)
- The `λ_inv ∈ {0, 0.25, 0.5, 1.0}` ablation (every variant measured 0.8666666746139526 — same synthetic input)
- Original #393 capability-discovery ablation suite + #398 calibrated rerun (pre-fix Brier 0.7539 was against synthetic-input predictor; real-input reruns now exist and remain `no_promotion`)
- The "slot-preserving architecture doesn't close the 0.083 gap by itself" claim (pre-fix claim invalid; real-input #393/#398/#546 reruns are negative evidence, not promotion evidence)
- The framing that "the 0.866 → 0.95 gap is a function of labels, not embedders/storage" (invalid as a broad causal claim; real-input constellation/λ_inv ablations did not identify those surfaces as gate movers)

**Post-fix baseline:** aiwonder FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/` is the current Panel A measurement of record: raw `prediction_oracle_correlation = 0.4843192995`, rank-safe calibrated `0.6916886568`, rank-lift calibrated from real code-state constellations `0.7942190170`. This rerun preserves #536 metrics and adds row-level calibration/holdout raw, rank-safe, and rank-lift prediction traces for #549. #547 revalidated that the selected rank-lift policy is split-disjoint and not holdout-selected. #548 then reproduced #536 exactly and accepted typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`. #549 formally proved repo/path metadata plus calibration-selected raw/rank-safe/rank-lift score fusion are useful but insufficient. #392 aggregate trajectories first lifted to `0.8008320332`; file-path plus skill/ability trajectories lifted to `0.8422425985`; sparse out-of-fold probability bins regressed ECE. The hardened support-aware slice at `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` selected a split-legal global mean-bias correction over the affine file/skill surface: `0.8497869968` correlation, Brier `0.0697008073`, ECE `0.0299093761`, and accuracy `0.9083333611`, with exact #536 row-trace reproduction and no mixed-cell threshold degeneracy. The FSV also recorded skill coverage explicitly: `1531 / 3000` expected code-state keys are covered and `1469` are deliberately marked uncovered, not silently zero-filled. #554 proved usefulness-weighted skills carry stronger consequence signal but were not active against the old artifact. #558 then rematerialized the full v2 label corpus (`399777` chunk rows, `398215` episodes, `157` skills, `9489` memberships) and reran #392 from `/zfs/archive/contextgraph/fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/`. That rerun selects `usefulness_weighted_combined`: correlation `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`, with report sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`. #560 then tried split-legal ECE-focused mean-bias repair against the rematerialized skill DB and reproduced the same legal best (`0.8679359555` correlation, Brier `0.0620564893`, ECE `0.0396374688`; report sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`). #557 adds the skill usefulness audit surface that makes source-byte sequences, lifecycle/demotion history, generic high-support rejections, and blanket-expansion regressions visible before further ECE work. This is the strongest legal trajectory signal so far, but it remains non-promotable because ECE is still above #536's `0.0240316074`; typed blockers `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED` and `TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED` remain accepted. The next gate mover must reduce ECE while preserving this targeted skill trajectory lift through per-cell/per-constellation consequence evidence, without sparse bin overfit, score blending, logit calibration, ECE-overweighted selection, support-only shrinkage, raw blanket skill expansion, or holdout-selected thresholds.

---

## Read order

| # | Doc | When to read it |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time touching the system. What it is, tech stack, entry points, subsystem summary, ambiguity boundary. |
| 2 | [`02_source_map.md`](02_source_map.md) | Need to know which crate / file owns what. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB column families. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling or implementing an MCP tool. 214 tools (67 are the `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Working on the predictor: embedders, instruments, panels, heads, TCT, corpus, training, heal, mistake loop. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating the system: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Proof history: shipped task FSV evidence index, ship-gate progress, ablation results. Marks pre-fix evidence INVALID. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Picking up work. Current open issues, critical path to the 0.95 ship gate, rebaseline blocker, post-ship plans. |

Plus:

- [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract.
- [`_archived/`](_archived/) — read-only history (do not read for current state).

---

## One-line system summary

> ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` (against the Docker oracle), with a named failure mode + closest exemplars on Fail (Q3), a binary claim-exists check (Q1) and binary shift-event predictions (Q5) as additional grounded surfaces. On top of the binary primitive it surfaces a **butterfly-effect pathway tree** (#416): top-K (3–5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves, letting the operator pick which future to realize.

Human review of AI patches becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` × 4 rolling windows on the 300×8 SWE-bench Lite Python corpus **AND** Panel B (non-overlapping encoder triangulation, #405) confirms within 0.05.

**Current ship-gate status:** `0.7942190170` rank-lift calibrated from real code-state constellation labels (`0.6916886568` rank-safe, `0.4843192995` raw) from aiwonder run `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260520T185945Z/`. The previous 0.866667 figure was measured against a synthetic-input inference pipeline (see warning box above) and is invalid.

---

## Critical-path one-liner (read `08` for detail)

**P0 blocker:** preserve the #558 usefulness-weighted skill trajectory lift while reducing aggregate ECE below the #536 baseline. #549 proved static repo/path metadata and raw-score fusion do not legally repair the mixed cells; #392 proved sparse out-of-fold bin calibration, affine/rank-lift blending, monotone logit calibration, ECE-overweighted selection, and support-only prior shrinkage are not the repair; #558 proved full-corpus targeted skills do move correlation/Brier but remain ECE-blocked; #560 proved ECE-focused mean-bias selection also keeps the same blocker; #557 gives the read surface needed to target useful skills and reject generic/regressing ones. #551 still tracks the explicit skill coverage gap (`1469 / 3000` code-state keys uncovered). The next work should repair calibration with per-cell/per-constellation consequence-to-cause evidence without losing the `0.8679359555` targeted skill trajectory signal.

Critical path: `#404 hierarchy` ✅ → `#413 labels` ✅ → `#414 skills` ✅ → `#418 resolver` ✅ → `#406/#523 mistake loop + MCP` ✅ → `#535 mixed-cell rank lift on real labels` ✅ → `#537 rebaseline #393/#398/λ_inv ablations` ✅ → `#536 weak-cell repair` ✅ → `#547 post-#536 calibration disjointness` ✅ → `#548 mixed-cell diagnostic blocker` ✅ → `#549 static signal fusion blocker` ✅ → **`#392 temporal constellation trajectory features`** → `#511 entropy loss` (p1, only if it improves mixed cells) → **`#405 Panel B`** (blocked p0) → 0.95 ship gate.

Post-0.95 roadmap stays archived/deferred. The active roadmap stops at the 0.95 × 4-window ship gate plus Panel B cross-check; pathway exploration, remaining skill-linkage expansions, info-physics formalization, and creativity-engine work do not take priority until that gate is met.

**Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes):** eight contingency issues (#538-#545) for if the current queue + creativity engine fail to close the gap — predictive-coding head, schema-prioritized replay, hierarchical predictive coding, mixed-selectivity binding, population-dynamics trajectories, sparse representations, synchrony binding, idle-time rollouts. All at `priority:p3` with explicit `DO NOT START` markers. Reassess only after post-queue gap analysis identifies need. See `08_roadmap_and_open_work.md §7`.
