# futurebuild — ME-JEPA-Code Reference

**Last refreshed:** 2026-05-18. **Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8 (down from 31).

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

## What changed in this refresh (2026-05-18)

1. **31 → 8 docs** condensed for AI agent token efficiency. Originals at `_archived/`.
2. **WHAT-IS rewrite** — every claim now traces to a source file/line. No aspirational content. Planning content compressed into `08_roadmap_and_open_work.md`.
3. **Dedicated proof-history doc** (`07_results_and_benchmarks.md`) — FSV evidence index, ablation results, audit history, benchmark numbers, ship-gate progress.
4. **Aiwonder storage policy** (`/zfs/archive/contextgraph/` durable + `/zfs/hot/contextgraph/` hot) made first-class in `01_overview.md §6-7` and `03_storage_schema.md §5`. Legacy `/mnt/d/contextgraph/` evidence reclassified as historical (#369 tracks revalidation).
5. **Phase H** (inner-loop retirement) marked complete throughout. `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
6. **EK / FP / RWD epic shipping status** captured in `07 §2.9-§2.11`.

Memory of *why* this refresh happened lives in operator-feedback memory:
`~/.claude/projects/-home-cabdru-contextgraph/memory/feedback_docs_rewrite_no_delegation_condense.md`.
