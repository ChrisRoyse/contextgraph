# Session Report: Phase 1b Embedder Blockers + FSV

Date: 2026-05-10

## Scope

Audited Phase 1 AST chunking, ME-JEPA instruments, and Phase 1b embedders against the current codebase and task docs.

## Findings

- Phase 1 AST chunking is implemented and verified for 11 languages. `context-graph-core` AST tests pass and FSV evidence reports 143 routing rows (13 entity types x 11 languages).
- Phase 1 instruments are implemented and now physically persist/read back panels through RocksDB CFs `mejepa_panels` and `mejepa_panel_meta`. FSV evidence contains reopened-store counts and rows.
- Phase 1b embedders are not production-ready yet. The D: model config at `/mnt/d/cabdru/contextgraph/models/models_config.toml` is legacy `[models]` schema with stale `/home/cabdru/contextgraph/models/...` paths; the Phase 1b loader requires `schema_version = 1` and `[embedders.eN]`.
- WSL runtime GPU access is blocked: `nvidia-smi` reports `Failed to initialize NVML: GPU access blocked by the operating system`. This prevents CUDA/VRAM preflight and any no-fallback embedder loading.
- D: model files exist for several legacy/content embedders, but current Phase 1b required assets are incomplete: no `bge-m3-dense`, no `kepler`, and no E15-E21 learner-state model directories were found under `/mnt/d/cabdru/contextgraph/models`.

## Work Completed

- Added/verified fail-closed Phase 1b surfaces:
  - `context-graph-mejepa-embedders` E1-E21 ID/config/digest/routing/VRAM/calibration APIs.
  - MCP tools: `mejepa_load_learner_state`, `mejepa_routing_lookup`, `mejepa_vram_budget_report`.
  - `config/models_config.toml.example` schema fixture and test coverage.
- Added/verified instrument panel RocksDB persistence:
  - `PanelStore`, `PanelKey`, `PanelMeta`.
  - FSV writes and reopens RocksDB before readback.

## Verification

- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mejepa-embedders -- --nocapture`: 9 passed.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mejepa-instruments -- --nocapture`: 55 passed.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-mcp tools::definitions::tests -- --nocapture`: registry tests passed for lib and bin.
- `CUDA_COMPUTE_CAP=120 cargo test -p context-graph-core memory::ast::tests -- --nocapture`: 10 passed.
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-embedders --example embedders_fsv`: wrote `/tmp/contextgraph-mejepa-embedders-fsv/embedders_fsv_evidence.json`; `phase_ready=false` due config schema and GPU blockers.
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-mejepa-instruments --example phase1_extended_fsv`: wrote `/tmp/contextgraph-mejepa-instruments-fsv/phase1-extended/phase1-extended-evidence.json`; RocksDB counts are both 1.
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-core --example ast_chunker_multilang_fsv`: 11-language FSV passed.
- `CUDA_COMPUTE_CAP=120 cargo run -p context-graph-core --example swebench_ast_fsv`: SWE-bench AST FSV passed with 20 chunks.

## Next Required Action

Phase 1 cannot be called complete until WSL GPU access is fixed and the real D: model registry is upgraded to the Phase 1b schema with SHA-pinned real files for E1-E21, including missing E11 Kepler, E14 BGE-M3, and E15-E21 learner-state assets.
