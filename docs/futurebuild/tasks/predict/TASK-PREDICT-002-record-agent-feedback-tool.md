---
task_id: TASK-PREDICT-002
title: mejepa_record_agent_feedback MCP tool + sampling-weight bump wiring
phase: A
status: open
blocked_by: [TASK-FLYWHEEL-005, TASK-FLYWHEEL-006]
blocks: [TASK-UTML-004]
effort: 1d
spec_refs: [SPEC-PREDICT.md REQ-PREDICT-10..11, TECH-PREDICT.md §10]
---

## Goal

Add the `mejepa_record_agent_feedback` MCP tool that accepts a `prediction_id`, `feedback_kind` ∈ {Confirmed, Surprise, Omission, Calibration}, agent explanation, actual outcome, severity. Persists a `SurpriseEvent` to `CF_MEJEPA_AGENT_FEEDBACK`. Bumps sampling weight on contributing chunks by `1 + 2 * severity_score`. Adds an `ActiveLearningKind::AgentSurprise` entry. Writes operator alert if `severity=Catastrophic`. Returns the feedback record + queue position.

This is the agent-side closing of the closed feedback loop. **Without this tool, ME-JEPA cannot learn from agent surprise events.**

## Inputs

- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` — existing tool registry
- `crates/context-graph-mejepa-cf/src/lib.rs` — extended with `CF_MEJEPA_AGENT_FEEDBACK` (FLYWHEEL-006)
- `crates/context-graph-mejepa::eval::queue::ActiveLearningQueueState` — existing queue
- `crates/context-graph-mejepa-train/src/sampler/batch_sampler.rs` — existing sampler
- `crates/context-graph-witness::cert` — for witness chain binding
- `docs/futurebuild/specs/functional/SPEC-PREDICT.md` REQ-PREDICT-10..11
- `docs/futurebuild/specs/technical/TECH-PREDICT.md` §10

## Outputs

- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` — extended with `handle_record_agent_feedback`
- `crates/context-graph-mcp/src/tools/names.rs` — extended with the tool name + JSON schema
- `crates/context-graph-mejepa/src/types.rs` — extension: `SurpriseEvent`, `FeedbackKind`, `SurpriseSeverity`, `ActualOutcome`
- `crates/context-graph-mejepa-train/src/sampler/batch_sampler.rs` — extended: surprise-weight bump in `compute_sampling_weight`
- `crates/context-graph-mejepa/src/heal/dispatch.rs` — wire AgentSurprise → fast_track_observe (Phase E will fully implement)
- FSV: `crates/context-graph-mcp/examples/agent_feedback_fsv.rs`

## Definition of done

1. `mcp__cgreality__mejepa_record_agent_feedback` is registered in the MCP tool registry with the JSON schema per `08 §2.1`.
2. Calling the tool with a synthetic `prediction_id` + `severity=Medium` persists a `SurpriseEvent` to `CF_MEJEPA_AGENT_FEEDBACK`; round-trip via `Storage::get_cf` returns the same struct.
3. Calling triggers an `ActiveLearningEntry { kind: ActiveLearningKind::AgentSurprise, ... }` write.
4. The next `BatchSampler` invocation observes the bumped sampling_weight = `1 + 2 * 0.5 = 2.0` for the affected chunk(s).
5. Calling with `severity=Catastrophic` writes an additional alert to `CF_MEJEPA_HEAL_REPORTS` with `severity=Catastrophic`.
6. Calling with non-existent `prediction_id` returns `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND`.
7. Rate-limited at 100/hour/agent_id; 101st call returns `MEJEPA_FEEDBACK_RATE_LIMITED`.
8. Witness-chain entry written binding `feedback_id` to original prediction's `witness_hash`.
9. `cargo build --release` + `cargo test -p context-graph-mcp` + `cargo clippy` all pass.
10. FSV evidence at `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/feedback.json` shows: input args, persisted SurpriseEvent, queue length before/after, sampling_weight delta, witness_hash chain.

## FSV evidence path

`/mnt/d/contextgraph/fsv/mejepa-predict-fsv/feedback.json`

## Boundary cases

- `severity=Low` → bump = 1 + 2*0.2 = 1.4
- `severity=High` → bump = 1 + 2*0.8 = 2.6
- `feedback_kind=Confirmed` (no severity) → bump = 1.1 (small reinforcement)
- `feedback_kind=Calibration` → bump = 1.2; recalibrate the involved head's conformal predictor
- `actual_outcome=None` (agent observed surprise but doesn't know the actual outcome yet) → still records; calibration_delta = None
- Same agent_id submitting > 100 events in 1 hour → `MEJEPA_FEEDBACK_RATE_LIMITED`
- `prediction_id` exists but has been TTL'd from `CF_MEJEPA_LIVE_PREDICTIONS` (after 30 days) → `MEJEPA_FEEDBACK_PREDICTION_NOT_FOUND`
