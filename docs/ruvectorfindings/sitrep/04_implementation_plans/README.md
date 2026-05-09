# Implementation Plans — Ready-To-Execute

Ready-to-execute plans for high-priority deferred items as of
2026-05-09. `plan_e_witness.md`, `plan_e_static_analysis.md`, and the
local wrapper/store portion of `plan_phase0_docker_oracle.md` have now
shipped; those files are retained as design history and for the
remaining official SWE-bench batch work.

## Files

| Plan | Priority | Effort | Crate / Module |
|---|---|---|---|
| `plan_e_witness.md` | ✅ shipped | design history | `context-graph-mejepa-instruments/src/e_witness.rs` |
| `plan_e_static_analysis.md` | ✅ shipped | design history | `context-graph-mejepa-instruments/src/e_static_analysis.rs` |
| `plan_phase0_docker_oracle.md` | HIGH | official image batch remains | `context-graph-mejepa-corpus/src/oracle.rs` |
| `plan_phase2_predictor.md` | MEDIUM | ~7-10 days | `context-graph-mejepa/` (new crate) |
| `plan_phase6_storage_wiring.md` | MEDIUM | ~3-5 days | `context-graph-mejepa-storage/` (new crate) |

## Order rationale

1. **Phase 0 official SWE-bench Docker batch** — local wrapper/parser/store
   are shipped; now pull/run the official task images and persist real
   `OracleVerdict` rows per `(task_id, mutation_category)`.

2. **E_Test + E_Problem + E_CommitMsg** — not yet covered by a dedicated
   plan file; reuse the E1 embedder path for three low-risk Phase 1 slots.

3. **E_Diff / E_AST scaffold** — tree-sitter-derived deterministic
   encoders before trained replacements.

4. **Phase 2 predictor** — the biggest single work item. Needs candle
   0.9.2-alpha sm_120a CUDA, ~7-10 days, but unblocks Phases 3..8.

5. **Phase 6 storage wiring** — wires the shipped tier-compression
   primitive into the storage layer. Necessary before Phase 3 produces
   thousands of training certificates.

## Skipping ahead

If you want to skip the E_* instruments and go directly to a working
end-to-end stub, you can do Phase 2 first by stubbing all 13 deferred
instruments with the hashed-feature placeholder pattern (see
`phase1.md` Notes section). This produces a runnable Phase 2 dry-run
gate, but the holdout-agreement gate (Phase 3 DoD) won't be meaningful
until the placeholders are replaced with real instruments.

## See also

- Per-phase context: `../03_deferred/phases/phase0.md`..`phase8.md`
- Top-level status: `../01_status_overview.md`
