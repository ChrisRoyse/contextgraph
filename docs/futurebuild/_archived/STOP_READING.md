# STOP — These docs are archived

**Archived:** 2026-05-18.

This `_archived/` directory holds the original 31-doc futurebuild planning suite that was authored 2026-05-10 through 2026-05-17. It is preserved as historical narrative only; do NOT read or cite it for current state.

## Where current docs live

The active 8-doc reference suite is one directory up:

- `../README.md` — index
- `../01_overview.md` — system, stack, entry points, subsystem summary
- `../02_source_map.md` — crates, binaries, dependency graph
- `../03_storage_schema.md` — RocksDB CFs, storage policy, on-disk layout
- `../04_mcp_tools.md` — every MCP tool, params, returns
- `../05_predictor_pipeline.md` — embedders, instruments, panels, heads, TCT, corpus, heal, training
- `../06_operations_and_fsv.md` — FSV protocol, runbook, DR, observability, test suite
- `../07_results_and_benchmarks.md` — historical FSV evidence, ship-gate progress, ablations
- `../08_roadmap_and_open_work.md` — current state, open issues, Phase G/I/J

## Why archived

- The original suite was planning-oriented (vision, phases, tasks, deletion list) and grew to 31 files with significant overlap.
- After Phase A–H shipped, much content became stale or duplicative.
- The operator asked for source-grounded WHAT-IS reference documentation, condensed for AI agent token efficiency, with proof/results history captured.
- 31 → 8 documents at ~30% of the doc-set size, with a dedicated `07_results_and_benchmarks.md` to preserve the proof history.

## Authoritative protocol

- Universal doctrine: `../../../docs2/AICodingAgentSuperPrompt.md`
- Project addendum: `../../../CLAUDE.md`
- Per-turn agent contract: `../../../AGENTS.md`
- FSV verification contract: `../specs/FSV-PROTOCOL.md` (still authoritative)

## Memory ≠ this directory

Per doctrine §3 (post-2026-05-13), institutional memory lives in this repo's GitHub Issues — `type:context` / `type:decision` / `type:discovery` / `type:pattern` plus `status:in-progress` / `status:blocked` plus CLAIM/PAUSE/RESOLVED comments. Do NOT write new markdown into either this archive or the parent doc set. The legacy `./memory/` directory is also archived at `../../../memory.archive/`.
