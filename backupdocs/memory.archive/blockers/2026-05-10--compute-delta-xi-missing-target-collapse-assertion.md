---
namespace: blockers
agent_id: agent-208-coordinator
timestamp: 2026-05-10T00:00:00Z
updated: 2026-05-10T17:20:49-05:00
status: resolved
severity: high
tags: utml, mejepa, safety, invariant-violation, target-collapse, delta-xi
---

# `compute_delta_xi` silently averages `target_collapse` instead of asserting it is 0

## What

`crates/context-graph-mejepa-train/src/learning_signal.rs:184` ships:

```rust
pub fn compute_delta_xi(c: DeltaXiComponents) -> Result<f32, UtmlError> {
    mean_checked(
        "delta_xi",
        [c.target_collapse, c.predictor_collapse, c.redundancy_mi],
    )
}
```

This computes a simple arithmetic mean. There is **no hard assertion that `c.target_collapse == 0.0`**.

## Why it matters

Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` §5.2 and the constitution's `<utml_v1_invariants>` rule 2:

> ΔΞ.target_collapse = 0 by construction in ME-JEPA. This is the central architectural innovation of ME-JEPA: target representations are produced by frozen externally-defined instruments with zero trainable parameters. They cannot drift, cannot collapse, cannot be gradient-pushed.

If `target_collapse` is ever non-zero at runtime, that means an instrument has accidentally been made trainable — the failure mode the constitution names with the error code `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. The current implementation silently averages this signal with the other ΔΞ sub-terms instead of failing closed, which means:

1. The structural collapse-immunity claim of ME-JEPA is **unenforced at runtime**.
2. A trainable instrument could leak gradients into the target side without anyone noticing — exactly the failure mode the architecture is supposed to make structurally impossible.
3. UTML v1.0 §VI.2 falsification claim ("ME-JEPA is the first JEPA in the literature with structural collapse-immunity") is currently observationally unverifiable.

## Expected behavior

`compute_delta_xi` MUST return `Err(UtmlError::new(UtmlErrorCode::MejepaInstrumentGradientLeak, "...").with_context(...))` whenever `c.target_collapse != 0.0`. The remainder of ΔΞ then comes from `predictor_collapse` and `redundancy_mi` only. Per doc 11 §5.4, the precise formula is also weighted (geometric/power form), not arithmetic mean — so this fix is part of a larger semantic-drift correction across `compute_delta_p/k/omega/xi`.

## Where it was discovered

Agent-213 surfaced this during the 2026-05-10 task-doc audit. See `memory/discoveries/agent-213-task-updates-utml-learning-formula.md` and the spot-check confirmation at `learning_signal.rs:184` (line range 184-189).

## Why this is a blocker (severity: high)

- It silently breaks the central architectural property of ME-JEPA-Code.
- The constitution lists `MEJEPA_INSTRUMENT_GRADIENT_LEAK` in `<failure_taxonomy>` precisely for this case.
- TASK-UTML-006 is described as "CRITICAL `target_collapse == 0.0` assertion" — the implementation does not match the task's title.

## Remediation plan

1. Update `compute_delta_xi` to fail closed on `c.target_collapse != 0.0` with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.
2. Bring `compute_delta_p`, `compute_delta_k`, `compute_delta_omega`, `compute_delta_xi` into alignment with the doc 11 weighted/geometric formulas (currently all four are arithmetic means — see agent-213's report for the full semantic-drift list).
3. Add a regression test in `crates/context-graph-mejepa-train/src/learning_signal.rs` asserting that `compute_delta_xi(DeltaXiComponents { target_collapse: 0.001, ... })` returns an `Err` with code `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.
4. Update TASK-UTML-006's `<verification>` block to require the assertion test.
5. Re-run `cargo test -p context-graph-mejepa-train learning_signal` and `mejepa-heal-drill` to confirm no callers regress.

## Resolution

Resolved during the 2026-05-10 hardening pass.

- `crates/context-graph-mejepa-train/src/learning_signal.rs::compute_delta_xi` now rejects non-finite and non-zero `target_collapse` with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.
- The healthy path now computes the multiplicative formula `(1 - target_collapse)^0.5 * (1 - predictor_collapse)^0.3 * (1 - redundancy_mi)^0.2`.
- `From<UtmlError> for TrainerError` maps the leak to `TrainerErrorCode::MejepaInstrumentGradientLeak`.
- Unit tests cover known formula value, `target_collapse=0.001`, and `target_collapse=NaN`.
- `train_edge_cases_fsv` proves the leak is rejected before cert construction/write, and the cert source of truth remains at count 0.

FSV evidence:

- `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json`
- `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json`
- `docs/ruvectorfindings/15_PHASE_1_5_HARDENING_FSV_2026_05_10.md`

## Related

- Agent-213 discovery: `memory/discoveries/agent-213-task-updates-utml-learning-formula.md`
- Updated task: `docs/ruvectorfindings/specs/tasks/utml-learning-formula/TASK-UTML-006.md` (now has both the AI-Agent Mandate block AND the STATE AUDIT noting the safety gap)
- Constitution: `docs/ruvectorfindings/specs/constitution.md` `<utml_v1_invariants>` rule 2
- Doc 11: `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` §5.2 ("ME-JEPA's structural solution")
