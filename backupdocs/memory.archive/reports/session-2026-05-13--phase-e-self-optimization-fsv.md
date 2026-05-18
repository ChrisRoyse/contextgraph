# Phase E self-optimization implementation + FSV

Date: 2026-05-13
Status: complete/code+FSV

## Scope

Phase E was implemented and documented as the autonomous ME-JEPA reality-compiler feedback loop. The system now focuses policy on ME-JEPA as the predictor/verifier of durable software reality, separate from the LLM. The LLM proposes edits; ME-JEPA consumes durable events, predicts reality, validates against oracle/source-of-truth outcomes, and turns mistakes/surprises/drift into reward signal.

## Source of truth

FSV artifact:
- `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`

Durable state verified by independent reads:
- `CF_MEJEPA_MODEL_PROMOTIONS`: promotion approvals, alerts, refresh policies, lambda/dissipation/Fisher records, emergency decisions, active-learning fast-path records.
- `CF_MEJEPA_HEAL_REPORTS`: intervention/heal reports.
- `CF_MEJEPA_DRIFT_WINDOW`: global drift observations.
- `CF_MEJEPA_ACTIVE_POINTERS`: promoted active model pointer.
- `CF_MEJEPA_WEIGHT_BLOBS`: targeted dormancy reinit artifacts.
- `self_optimization_status.json`: scheduler ticker health and last-success details.

## FSV evidence

Command:
- `cargo run -p context-graph-mejepa --example phase_e_self_optimization_fsv`

Result:
- Passed.
- Wrote `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json` with mode `0600`.
- Artifact contains 11 cases.

Manual readback summary:
- `sourceOfTruth.kind = rocksdb_plus_scheduler_status_file`
- `sourceOfTruth.policyCf = CF_MEJEPA_MODEL_PROMOTIONS`
- `sourceOfTruth.healReportCf = CF_MEJEPA_HEAL_REPORTS`
- `caseCount = 11`

Boundary/edge cases verified:
- `per_cell_small_window_7_of_10_noise`: policy rows stayed `10 -> 10`; no pending promotion.
- `emergency_eviction_exact_110gb`: policy rows `0 -> 1`; repeat action was `suppressed_repeat`.
- `per_cell_promotion_rejects_regression`: winner rejected because `off_by_one::python` regressed.
- `invalid_approval_operator_id`: returned `MEJEPA_HEAL_INVALID_STATE`; policy rows stayed `1 -> 1`; pending promotion contents unchanged.

## Tests

Passed:
- `cargo check -p context-graph-mejepa -p context-graph-mcp`
- `cargo test -p context-graph-mejepa --lib heal -- --nocapture` (56 passed)
- `cargo test -p context-graph-mcp mejepa_promote -- --nocapture` (compile/test filter passed; 0 named tests)

## Root causes fixed or documented

- Bincode cannot safely round-trip internally tagged serde enums that rely on `deserialize_any`; promotion approval kinds were made bincode-compatible.
- Catastrophic approval could not be allowed to downgrade to one approval; policy now enforces the maximum of persisted, requested, and catastrophic-required approval counts.
- Invalid approval requests must not mutate durable state; FSV now reads the pending promotion before and after invalid operator input and checks exact equality.
- Heterogeneous policy records should not be written into `CF_MEJEPA_HEAL_REPORTS`, because heal-status readers decode that CF as `HealReport`; policy-style records belong in `CF_MEJEPA_MODEL_PROMOTIONS`.
- Per-cell drift should not invent corpus hashes; the per-cell corpus slice is derived from persisted observations and fails closed when observations are absent.

## Docs updated

- `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md`
- `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md`
- `docs/futurebuild/12_IMPLEMENTATION_PHASES.md`
- `docs/futurebuild/13_TASK_INDEX.md`
- `docs/futurebuild/specs/functional/SPEC-HEAL.md`
- `docs/futurebuild/specs/technical/TECH-HEAL.md`
- `docs/futurebuild/tasks/heal/_all_tasks.md`

Other Phase E narrative docs were already dirty in the working tree and now align with the shipped Phase E status.

## Research alignment

Official sources reviewed:
- Google Cloud ML quality guidance: continuous monitoring, joining true labels with predictions when ground truth is available, efficiency metrics, rollout/rollback.
- Azure ML model monitoring: production/reference data, drift thresholds, alerts, multiple monitoring signals, objective performance signals when ground truth exists.
- AWS Prescriptive Guidance for generative AI production operations: automated quality gates, A/B/canary rollout, automatic rollback, security gates, and human approval for high-impact promotion.

## Remaining non-Phase-E work

Phase E being complete does not satisfy the human-review-replacement gate. The remaining gate is Phase G: `prediction_oracle_correlation >= 0.95` stable over four rolling windows across the 300x8 SWE-bench Lite mutation corpus, stratified by mutation category and language.
