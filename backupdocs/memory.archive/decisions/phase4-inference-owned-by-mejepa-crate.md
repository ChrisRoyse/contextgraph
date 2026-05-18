# Decision: Phase 4 Inference Lives In `context-graph-mejepa`

Date: 2026-05-10

## Decision

Phase 4 inference is implemented inside `crates/context-graph-mejepa`, not as a new crate.

## Rationale

- Phase 2 predictor APIs and Phase 4 task docs both target the ME-JEPA crate.
- Creating a new crate would duplicate predictor/instrument/cert abstractions and make downstream MCP routing ambiguous.
- Existing Phase 2 `config.rs` and `error.rs` are preserved; inference-specific config/error types are added without replacing predictor behavior.

## Consequences

- Public inference modules are exported from `context_graph_mejepa`.
- MCP depends on `context-graph-mejepa` for Phase 4 types and handlers.
- `context-graph-mejepa-train` remains independent; Phase 4 reads train-health summaries through a trait so no crate cycle is introduced.
