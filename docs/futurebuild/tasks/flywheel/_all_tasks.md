# Flywheel domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory.

Phase A FLYWHEEL-001/002/006 have detailed individual task files. This file covers the rest of the FLYWHEEL + UTML tasks compactly.

**Status snapshot (2026-05-12):**
- FLYWHEEL-001: `code_complete` (daemon ships; fresh-host enablement proof still pending)
- FLYWHEEL-002: `complete` (scheduler skeleton, 7 tickers, FSV at `/mnt/d/contextgraph/fsv/mejepa-heal-fsv/scheduler_ticks.json`)
- FLYWHEEL-003: `complete (2026-05-12)` — PostToolUse Bash capture hook + parser wired into `.claude/settings.json`; source of truth `/mnt/d/contextgraph/runtime/test-outcomes/<session>.jsonl`.
- FLYWHEEL-004: `complete (2026-05-12 code path)` — observe tick consumes captured test-outcome JSONL, matches latest session prediction, writes readback-verified `CalibrationExample` rows into `CF_MEJEPA_ORACLE_VERDICTS`.
- FLYWHEEL-005: `complete (2026-05-11)` — `ActiveLearningKind` enum (Uncertainty / OutOfDistribution / AgentSurprise{prediction_id, severity_score}) shipped in `crates/context-graph-mejepa/src/eval/queue.rs`; `enqueue_agent_surprise()` + fast-track eviction; `tick_active_learning` orders AgentSurprise first. FSV at `/mnt/d/contextgraph/fsv/active-learning-fsv/agent_surprise.json`. 4 new unit tests pass; 80/80 mejepa lib tests pass; clippy clean. Real bug caught by FSV: bincode rejected `tag="type"` internal enum representation — fixed to default external tag.
- FLYWHEEL-006: `complete (2026-05-11)` — 3 CFs registered, FSV at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json`, all boundary cases pass; see `./memory/reports/session-2026-05-11--task-flywheel-006-new-cfs.md`.
- FLYWHEEL-007: `open` — Park-list CF (blocked-by FLYWHEEL-006 → now unblocked)
- UTML-001: `complete/code_complete (2026-05-12)` — per-head signal module added in `crates/context-graph-mejepa-train/src/learning_signal/per_head.rs`; `delta_xi_global_min = min_h(delta_xi_h)` validates fail-closed.
- UTML-002: `partial/code_complete (2026-05-12)` — `TrainingCertificate` now persists `per_head_l_step` and `delta_xi_global_min`; full per-head loss/delta/lambda columns remain open.
- UTML-003: `partial/code_complete (2026-05-12)` — min-head aggregation exists; BatchSampler skip/requeue policy for collapsed heads remains open.
- UTML-004: `complete (2026-05-12)` — sampling weight includes agent surprise severity.
- UTML-005..009: `open` — per-head Fisher, dormancy, lambda ramp, dissipation, and per-cell state-transfer T remain with Phase E/F policy hardening.

---

## TASK-FLYWHEEL-003 — Test-outcome capture hook

**Status:** complete (2026-05-12).

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** none · **Spec:** SPEC-FLYWHEEL REQ-FLYWHEEL-08
- **Goal:** New PostToolUse-on-Bash hook script `scripts/cc/hook_post_tool_use_test_capture.sh` that parses pytest / cargo-test / jest output and appends `TestOutcomeEvent { ts, session_id, test_id, outcome, duration_ms, error_log }` per test to `/mnt/d/contextgraph/runtime/test-outcomes/<session>.jsonl`.
- **Outputs:** `scripts/cc/hook_post_tool_use_test_capture.sh`, `scripts/cc/parse_test_output.py`, extension to `.claude/settings.json`.
- **DoD:** running `pytest -v` on a fixture → JSONL line per test with correct outcome; same for `cargo test --release -q`; same for `jest --json`. Boundary: empty output (no tests run) → no events emitted, exit 0; non-test command → exit 0.
- **FSV:** `/mnt/d/contextgraph/fsv/test-capture-fsv/three_frameworks.json`

## TASK-FLYWHEEL-004 — Asynchronous oracle binding inside observe()

**Status:** complete (2026-05-12 code path). Current live schema does not persist per-test IDs or panel handles inside `RealityPrediction`, so the shipped binding uses the real persisted session prediction and writes hard labels to `CF_MEJEPA_ORACLE_VERDICTS`; panel-rich observe remains Phase E/B follow-up once prediction provenance stores panel handles.

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** TASK-FLYWHEEL-003
- **Goal:** Self-optim scheduler's observe_tick consumes `TestOutcomeEvent`s from the JSONL queue and binds each to the most recent matching `RealityPrediction` (matching session + including the test_id), then calls `MeJepaCompiler::observe(panel, action, panel_after_test, OracleOutcome::from(event))`.
- **Outputs:** `crates/context-graph-mejepa/src/heal/scheduler_state.rs::drain_pending_oracle_outcomes`.
- **DoD:** synthetic test outcome arriving 5s after a prediction → observe() called within next 250ms tick; training cert chain entry written; if no matching prediction (TTL'd), `late_oracle = true` flag set.
- **FSV:** `/mnt/d/contextgraph/fsv/oracle-binding-fsv/round_trip.json`

## TASK-FLYWHEEL-005 — `ActiveLearningKind::AgentSurprise` enum variant

- **Phase:** A · **Effort:** ~0.25d (reduced; types pre-shipped) · **Blocked by:** none · **Status:** partial
- **Goal:** Extend `ActiveLearningKind` enum with `AgentSurprise` variant; queue the kind from `mejepa_record_agent_feedback` handler; consumer in `scheduler.tick_active_learning` fast-tracks observe() for queued surprise entries.
- **Audit (2026-05-11):** the `mejepa::types::SurpriseEvent` / `SurpriseSeverity` / `FeedbackKind` / `ActualOutcome` extensions originally listed under this task's Outputs are **already shipped** in `crates/context-graph-mejepa/src/types.rs` (TASK-FLYWHEEL-006 pre-emptively built the type system for REQ-FLYWHEEL-10). Remaining work is purely the `ActiveLearningKind` variant + queue/scheduler wiring.
- **Outputs:** `crates/context-graph-mejepa/src/eval/queue.rs` — extend `ActiveLearningKind` enum + `ActiveLearningEntry` constructor; `crates/context-graph-mejepa/src/eval/types.rs` if the enum lives there (audit first). DO NOT re-define types already in `mejepa::types`.
- **DoD:** enqueue with kind=AgentSurprise → readback in `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` shows entry; `tick_active_learning` drains it within one tick period; SoT round-trip verified per FSV-PROTOCOL §6.4.
- **FSV:** `/mnt/d/contextgraph/fsv/active-learning-fsv/agent_surprise.json` (with `cf_counts_before`/`after`, `readback_equal=true`, boundary cases per FSV-PROTOCOL §5).

## TASK-FLYWHEEL-007 — Park-list CF for the 3-attempt-park rule

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** TASK-FLYWHEEL-006 · **Resolves:** D12
- **Goal:** Add `CF_MEJEPA_PARKED_TASKS` keyed by `(task_id)` with value `{ attempt_count, last_attempt_ts, parked_until_active_learning_surfaces }`; loop's task picker reads this CF; active-learning queue lookup exposes parked tasks.
- **Outputs:** new CF in `mejepa-cf` crate; lookup helpers; `mejepa_park_status` MCP tool.
- **DoD:** task with 3 consecutive failed attempts → entry created; task picker skips parked tasks; surfacing back to non-parked when active-learning queue includes it.
- **FSV:** `/mnt/d/contextgraph/fsv/park-fsv/round_trip.json`

---

## TASK-UTML-001 — Per-head ΔP/ΔK/ΔΞ tracking

**Status:** complete/code_complete (2026-05-12). Implemented in `crates/context-graph-mejepa-train/src/learning_signal/per_head.rs`; focused tests prove `delta_xi_global_min` is the minimum head value and all outputs validate in [0,1].

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005 · **Spec:** TECH-PREDICT §3
- **Goal:** Extend `learning_signal::compute_delta_*` to compute per-head deltas + global aggregates per `10 §15`. ΔΞ_global = `min_h(ΔΞ_h)`.
- **Outputs:** `crates/context-graph-mejepa-train/src/learning_signal/per_head.rs`.
- **DoD:** for a synthetic batch with one head having ΔΞ=0, global L_step = 0; cert columns reflect per-head + global.
- **FSV:** focused unit tests plus Phase B FSV `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`.

## TASK-UTML-002 — Per-head training cert columns

**Status:** partial/code_complete (2026-05-12). `per_head_l_step` and `delta_xi_global_min` are persisted in `TrainingCertificate`; full `loss_per_head`, `delta_p_per_head`, `delta_k_per_head`, `delta_xi_per_head`, and `lambda_h` maps remain open.

- **Phase:** B · **Effort:** 0.25d · **Blocked by:** TASK-UTML-001
- **Goal:** Extend `TrainingCertificate` struct with `loss_per_head`, `delta_p_per_head`, `delta_k_per_head`, `delta_xi_per_head`, `lambda_h` fields per `10 §7`.
- **Outputs:** `crates/context-graph-mejepa-train/src/cert/mod.rs` extension; round-trip serde test.
- **DoD:** new cert serializes/deserializes losslessly; fail-closed validation rejects missing/non-finite head entries.

## TASK-UTML-003 — `min_h` aggregation in BatchSampler

**Status:** partial/code_complete (2026-05-12). The aggregation calculation exists and is trainer-wired; BatchSampler's epoch skip policy remains open.

- **Phase:** B · **Effort:** 0.25d · **Blocked by:** TASK-UTML-001
- **Goal:** BatchSampler reads ΔΞ_global per `10 §5`; if any head's ΔΞ_h = 0, BatchSampler skips this example for the next epoch.
- **Outputs:** `crates/context-graph-mejepa-train/src/sampler/batch_sampler.rs` extension.
- **DoD:** synthetic example with ΔΞ_failure_mode = 0 → not sampled in next epoch; verified by sample-counting in test.

## TASK-UTML-004 — `agent_surprise_severity_score` factor in sampling_weight

**Status:** complete (2026-05-12). The sampling path applies the `1 + 2 * severity_score` multiplier and is covered by focused tests.

- **Phase:** B · **Effort:** 0.25d · **Blocked by:** TASK-PREDICT-002
- **Goal:** Extend `compute_sampling_weight` per `TECH-FLYWHEEL §6` to multiply by `1 + 2 * severity_score` when feedback exists.
- **Outputs:** `compute_sampling_weight` extension.
- **DoD:** Medium severity (0.5) → weight bump 2.0×; verified vs no-feedback baseline.

## TASK-UTML-005 — Per-head Fisher matrices

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-UTML-001
- **Goal:** Extend `FisherSnapshot` per `10 §9` with `per_head_fisher: HashMap<HeadId, Tensor>`. EWC++ regularization uses per-head Fisher for head-specific updates.
- **Outputs:** `heal/ewc.rs` extension.
- **DoD:** per-head Fisher computed from per-head holdout; round-trip serde + on-disk verify.

## TASK-UTML-006 — Per-head dormancy detection

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-UTML-001
- **Goal:** `detect_dormant_units_per_head` returns `HashMap<HeadId, DormantUnitReport>`. Reinit applies per-head; trunk preserved.
- **Outputs:** `learning_signal/continual_backprop.rs` extension.
- **DoD:** inject 20% dormancy in failure_mode head's projection → detector fires for that head only; reinit succeeds.

## TASK-UTML-007 — Per-head λ ramp in heal scheduler

- **Phase:** E · **Effort:** 0.25d · **Blocked by:** TASK-HEAL-001, TASK-UTML-001
- **Goal:** `lambda_ramp::tick` per `TECH-HEAL §3 / TECH-HEAL.md lambda_ramp` increments λ_h when head's holdout metric crosses gate.
- **Outputs:** `heal/lambda_ramp.rs` (NEW).
- **DoD:** failure_mode head F1 crosses 0.6 → next tick increments λ_failure_mode by 0.05 toward 0.30.

## TASK-UTML-008 — Per-head dissipation detection

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-UTML-001
- **Goal:** Extend dissipation detection to per-head; per-head fires its own `MEJEPA_HEAD_DISSIPATION_DETECTED`.
- **Outputs:** `heal/dissipation.rs` extension.
- **DoD:** synthetic head-only dissipation pattern fires per-head warning without tripping global alarm.

## TASK-UTML-009 — Per-cell state-transfer T in eval reports

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-UTML-001
- **Goal:** Extend `EvalReport` per `10 §11` with `state_transfer_t_per_cell: HashMap<(MutationCategory, Language), f32>`.
- **Outputs:** `eval/types.rs` extension; `eval/metrics.rs::compute_state_transfer_per_cell`.
- **DoD:** weekly report shows per-cell T; cells with low T flagged as candidates for fine-tuning.
