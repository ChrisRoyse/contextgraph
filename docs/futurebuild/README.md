# futurebuild — ME-JEPA-Code Reference

**Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8.

Source code: `/home/cabdru/contextgraph/crates/` (22 active workspace crates).
Authority hierarchy: **universal doctrine** (`docs2/AICodingAgentSuperPrompt.md`) > **project addendum** (`CLAUDE.md`) > **this suite**.

---

## Read order

| # | Doc | When to read it |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time touching the system. What it is, tech stack, entry points, subsystem summary, ambiguity boundary. |
| 2 | [`02_source_map.md`](02_source_map.md) | Need to know which crate / file owns what. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB column families. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling or implementing an MCP tool. 204 tools (57 are the `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Working on the predictor: embedders, instruments, panels, heads, TCT, corpus, training, heal, mistake loop. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating the system: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Proof history: shipped task FSV evidence index, ship-gate progress, ablation results. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Picking up work. Current open issues, critical path to the 0.95 ship gate, post-ship plans. |

Plus:

- [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract.
- [`_archived/`](_archived/) — read-only history (do not read for current state).

---

## One-line system summary

> ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` (against the Docker oracle), with a named failure mode + closest exemplars on Fail (Q3), a binary claim-exists check (Q1) and binary shift-event predictions (Q5) as additional grounded surfaces. On top of the binary primitive it surfaces a **butterfly-effect pathway tree** (#416): top-K (3–5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves, letting the operator pick which future to realize.

Human review of AI patches becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` × 4 rolling windows on the 300×8 SWE-bench Lite Python corpus **AND** Panel B (non-overlapping encoder triangulation, #405) confirms within 0.05.

Current ship-gate status: **0.866667** vs target 0.95 on Panel A; Panel B has no measurement yet (#405 produces the first).

---

## Critical-path one-liner (read `08` for detail)

`#404 hierarchy` ✅ → `#413 labels` ✅ → `#414 skills` ✅ → `#418 resolver` ✅ → `#406/#523 mistake loop + MCP` ✅ → **`#405 Panel B`** (blocked p0) → **`#407 calibration disjointness`** (p1) → `#511 entropy loss` (p1, on the road to 0.95) → 0.95 ship gate.

Post-0.95: pathway exploration (#416), skill-linkage MCP (#417), info-physics formalization (#511/#512/#514), creativity engine (#516-#520).
