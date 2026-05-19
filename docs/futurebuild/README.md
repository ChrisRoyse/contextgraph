# futurebuild — ME-JEPA-Code Reference

**Last refreshed:** 2026-05-19. **Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8 (down from 31).

Source code: `/home/cabdru/contextgraph/crates/` (25 default-build crates; 1 excluded benchmark suite).
Authority hierarchy: **universal doctrine** (`docs2/AICodingAgentSuperPrompt.md`) > **project addendum** (`CLAUDE.md`) > **this suite**.

---

## Read order

| # | Doc | When to read it |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time touching the system. What it is, tech stack, entry points, subsystem summary, exception hierarchy. |
| 2 | [`02_source_map.md`](02_source_map.md) | Need to know which crate / file owns what. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB column families. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling or implementing an MCP tool. 196 tools (49 are the `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Working on the predictor: embedders, instruments, panels, heads, TCT, corpus, training, heal. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating the system: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Want the proof history: shipped task FSV evidence, ablation results, ship-gate progress, audit history. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Picking up work. Current open issues by phase, critical path to the 0.95 ship gate. |

Plus:

- [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract (still authoritative; not archived).
- [`_archived/`](_archived/) — read-only history of the original 31-doc planning suite (do not read for current state; see `_archived/STOP_READING.md`).

---

## One-line system summary

> ME-JEPA-Code is a deterministic predictive truth system that returns a calibrated multi-head `RealityPrediction` (Q1 claim-exists / Q2 works / Q3 why-fails / Q4 what-else / Q5 reality-impact) for any AI-generated code change, so human review of AI patches becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` stably × 4 rolling windows on the 300×8 SWE-bench Lite Python corpus.

Operationally, "predicting reality" means predicting the consequences of code being live: how the change will interact with durable repository bytes, AST, tests, runtime, dependencies, security/perf/cost surfaces, witness chain, maintenance blast radius, and later shift-log observations. The MCP product shows those predicted consequences and confidence to the human/operator-agent; it does not choose the human's goal and it does not infer hidden human intent (#397, #358).

Current ship-gate status: **0.866667** vs target 0.95. Phase G critical path drives the remaining 0.083 (see `08_roadmap_and_open_work.md §3`).

---

## What changed in this refresh (2026-05-18 → 2026-05-19)

1. **31 → 8 docs** condensed for AI agent token efficiency. Originals at `_archived/`.
2. **WHAT-IS rewrite** — every claim now traces to a source file/line. No aspirational content. Planning content compressed into `08_roadmap_and_open_work.md`.
3. **Dedicated proof-history doc** (`07_results_and_benchmarks.md`) — FSV evidence index, ablation results, audit history, benchmark numbers, ship-gate progress.
4. **Aiwonder storage policy** (`/zfs/archive/contextgraph/` durable + `/zfs/hot/contextgraph/` hot) made first-class in `01_overview.md §6-7` and `03_storage_schema.md §5`. Legacy `/mnt/d/contextgraph/` evidence reclassified as historical (#369 tracks revalidation).
5. **Phase H** (inner-loop retirement) marked complete throughout. `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
6. **EK / FP / RWD epic shipping status** captured in `07 §2.9-§2.11`.

**2026-05-19 closures captured:**

- **#388 / TASK-PY-G-088** — Real aiwonder cross-embedder constellation tensors. 516 per-cell/window/slot rows + 2,838 per-cell/window upper-triangle pair tensors written to `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/`.
- **#389 / TASK-PY-G-089** — Per-slot calibration + OOD guard live. `RealityPrediction.per_slot_ood_reasons` now persists per-slot/embedder/pair OOD reasons through `CF_MEJEPA_LIVE_PREDICTIONS`; exposed by `mejepa_explain_prediction` / `mejepa_inspect_prediction`; durable calibration state under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`.
- **#393 / TASK-PY-G-093** — Capability-discovery ablation suite ran on #396 arrays. All ship-gate-eligible ablations demoted vs the local baseline ladder; flat-vector baseline marked comparison-only.
- **#394 / TASK-PY-G-094** — Anti-compensation FSV suite: corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, and temporal jumps remain non-ship-gate-eligible even when aggregate normalized scores would pass.
- **#398 / TASK-PY-G-097** — Calibrated constellation ablation rerun. Result `no_promotion`: calibrated full constellation Brier `0.7539…` worse than best local baseline Brier `0.00855…`. The richer slot/pair calibration readout is architecturally correct but does not close the 0.083 correlation gap by itself.
- **#276 / TASK-PY-G-057** — Live-session OOD harvest pipeline shipped. `CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` + 4× active-learning projection + `OOD_GATE_OVER_FLAGGING` flag.
- **#377 / TASK-PY-G-078** — Q4 trust gate shipped. `mejepa_predict_latest`, `mejepa_explain_prediction`, and `mejepa_inspect_prediction` now expose `q4.rawObservations`, `q4.trustGate`, and `q4.trustedConsequences`; raw Q4 fields remain display-only until producer, calibration, and exact 12-active-slot evidence exists.
- **#383 / TASK-PY-G-084** — Locked-in rule (pinned `type:context`): gate-active issue order must be evidence-weighted, not vision-weighted. `08 §3.0` now leads with this and reorders the critical path: train missing Q4/Q5 label producers + calibrate OOD on harvested data BEFORE further slot-preserving predictor refactors.
- **Newly filed since 2026-05-17** captured in `08`: PY-G-076/077/078/079/080/081/082/083/084, AIWONDER-019/020/021. Migration p0/p1 follow-ups (#372 ZFS mount, #380 off-host backup, #384 CUDA rollback) included.

Memory of *why* this refresh happened lives in operator-feedback memory:
`~/.claude/projects/-home-cabdru-contextgraph/memory/feedback_docs_rewrite_no_delegation_condense.md`.
