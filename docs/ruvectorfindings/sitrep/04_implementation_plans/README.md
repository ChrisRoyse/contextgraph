# Implementation Plans — Ready-To-Execute

Five ready-to-execute plans for the highest-priority deferred items as
of 2026-05-09. Each plan includes deliverables, definition of done,
test plan, design notes, and an explicit "fresh-agent recipe" for
picking up the work.

## Files

| Plan | Priority | Effort | Crate / Module |
|---|---|---|---|
| `plan_e_witness.md` | **HIGHEST** | ~1.5 days | `context-graph-mejepa-instruments/src/e_witness.rs` |
| `plan_e_static_analysis.md` | HIGH | ~2 days | `context-graph-mejepa-instruments/src/e_static_analysis.rs` |
| `plan_phase0_docker_oracle.md` | HIGH | ~3-5 days | `context-graph-mejepa-corpus/src/oracle.rs` |
| `plan_phase2_predictor.md` | MEDIUM | ~7-10 days | `context-graph-mejepa/` (new crate) |
| `plan_phase6_storage_wiring.md` | MEDIUM | ~3-5 days | `context-graph-mejepa-storage/` (new crate) |

## Order rationale

1. **E_Witness first** — pure-Rust, validates the trait surface with a
   second concrete instrument (after E_Oracle), unblocks no other work.
   Self-contained; ~300 LOC + tests + FSV.

2. **E_StaticAnalysis next** — same trait surface, pure-Rust scalar
   feature aggregates, no external dependencies. Doubles the linear-
   probe gate evidence base.

3. **Phase 0 Docker oracle** — gates the full corpus generation. Without
   real `OracleVerdict` per `(task_id, mutation_category)`, Phase 1
   instrument linear-probe gate cannot pass.

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
