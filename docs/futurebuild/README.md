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
- #393 capability-discovery ablation suite + #398 calibrated rerun (Brier 0.7539 was against synthetic-input predictor)
- The "slot-preserving architecture doesn't close the 0.083 gap by itself" claim (untestable until post-fix rebaseline)
- The framing that "the 0.866 → 0.95 gap is a function of labels, not embedders/storage" (also untestable)

**P0 blocker:** rebaseline the ship-gate FSV against real-instruments inference path + real trained checkpoint before continuing #407 / #511 / #405 work. Tracked under new context issue (filed alongside this doc refresh).

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

**Current ship-gate status: PENDING REBASELINE.** The previous 0.866667 figure was measured against a synthetic-input inference pipeline (see ⚠ box above). No post-fix end-to-end ship-gate measurement exists yet on aiwonder.

---

## Critical-path one-liner (read `08` for detail)

**P0 blocker:** rebaseline ship-gate FSV against real-instruments inference path + real trained checkpoint.

After rebaseline: `#404 hierarchy` ✅ → `#413 labels` ✅ → `#414 skills` ✅ → `#418 resolver` ✅ → `#406/#523 mistake loop + MCP` ✅ → **rebaseline #393/#398/`λ_inv` ablations on real inputs** → **`#407 calibration disjointness`** (p1) → `#511 entropy loss` (p1) → **`#405 Panel B`** (blocked p0) → 0.95 ship gate.

Post-0.95: pathway exploration (#416), skill-linkage MCP (#417), info-physics formalization (#511/#512/#514), creativity engine (#516-#520).
