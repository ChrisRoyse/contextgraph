---
namespace: reports
agent_id: agent-claude-opus-4.7
created: 2026-05-11T16:45:00Z
updated: 2026-05-11T16:45:00Z
status: active
tags: report, task-flywheel-005, active-learning, fsv, bincode
---

# Session Report: TASK-FLYWHEEL-005 — ActiveLearningKind::AgentSurprise variant

## Task: Add the AgentSurprise variant to the active-learning queue + fast-track eviction + scheduler ranking, FSV against real RocksDB.
## Outcome: complete

## Summary

The original task spec said "extend `ActiveLearningKind` enum" — but per FSV-PROTOCOL §7 audit, no such enum existed in the codebase. The active-learning queue used `ActiveLearningQueueEntry` with a free-form `reason: String` field. Per FSV-PROTOCOL §3 rule 7 (Occam's razor) and rule 2 (no backwards compat), I introduced the enum cleanly as a new mandatory field on the entry struct rather than retrofitting a shim.

Three variants: `Uncertainty` (existing wide-conformal path), `OutOfDistribution` (existing OOD-escalation path), `AgentSurprise { prediction_id, severity_score }` (new — from `mejepa_record_agent_feedback`). Each has a `fast_track_rank()` method (0/1/2 respectively) so the scheduler's `tick_active_learning` drains agent-flagged surprises first, regardless of within-kind score.

Real bug caught by FSV (per FSV-PROTOCOL §10): unit tests passed using serde-JSON, but the eval store uses bincode, which doesn't support internally-tagged enums. The first FSV run errored with `MEJEPA_EVAL_STORE: Bincode does not support the serde::Deserializer::deserialize_any method`. Fix was to drop `#[serde(tag = "type")]` from `ActiveLearningKind` — the default external-tag representation works with both JSON and bincode.

## Files Created or Modified in the Repo

- `crates/context-graph-mejepa/src/eval/queue.rs` — added `ActiveLearningKind` enum with `agent_surprise()` constructor + `fast_track_rank()` method; added mandatory `kind` field to `ActiveLearningQueueEntry`; refactored `enqueue_uncertain` to set kind from `ood_score`; added `enqueue_agent_surprise` method; extracted `evict_to_capacity` helper that compares by kind rank first, score second (so AgentSurprise survives eviction even at score 0.2 vs Uncertainty 0.99); 4 new unit tests.
- `crates/context-graph-mejepa/src/eval/mod.rs` — re-exported `ActiveLearningKind`.
- `crates/context-graph-mejepa/src/heal/scheduler_state.rs` — updated `tick_active_learning` sort to use kind rank as primary key; added `kind` to the top_tasks JSON output.
- `crates/context-graph-mejepa/examples/scheduler_5_ticker_fsv.rs` — updated synthetic queue entry constructor with `kind: ActiveLearningKind::Uncertainty`.
- `crates/context-graph-mejepa/examples/active_learning_surprise_fsv.rs` — new FSV harness using real `open_hygiene_rocksdb` + `RocksDbEvalStore::new(Arc<DB>)` to round-trip an `AgentSurprise` entry; 3 boundary cases.
- `docs/futurebuild/tasks/flywheel/_all_tasks.md` — FLYWHEEL-005 status updated to `complete (2026-05-11)` with FSV evidence path.
- `docs/futurebuild/13_TASK_INDEX.md` — FLYWHEEL-005 effort cell updated to `complete (2026-05-11)`.

## Files Written or Updated in ./memory/

- `reports/session-2026-05-11--task-flywheel-005-active-learning-surprise.md` — this report.

## Decisions Made

- **Approach chosen**: introduce `ActiveLearningKind` enum as a mandatory field on `ActiveLearningQueueEntry`, refactor existing 3 construction sites to set the kind explicitly.
- **Considered alternatives**:
  1. Keep the existing `reason: String` field and overload it for surprise detection (e.g., `reason.starts_with("agent_surprise:")`). Rejected: string-typed dispatch is a code smell; eviction-ordering code would need to parse strings; future variants would compound the brittleness.
  2. Use `#[serde(tag = "type")]` internally-tagged enum for ergonomic JSON. Rejected after FSV caught bincode incompatibility — fail-fast against the real store. Default external tag works in both formats.
- **Why this one wins**: typed dispatch in `evict_to_capacity` and `tick_active_learning` becomes match-exhaustive instead of string-parsing; no backwards-compat shim because the codebase is pre-production; cleanest Occam's-razor option that satisfies the spec.
- **What changes the decision**: if a future task needs cross-format compatibility with consumers that expect a specific JSON tag layout, swap to `#[serde(tag, content)]` adjacent tagging (still bincode-safe).

## Discoveries

- **bincode does not support internally-tagged enums.** Any new persisted enum that lives in `mejepa::types` or `mejepa::eval` must use either default external tagging (`{"AgentSurprise": {...}}`) or adjacent tagging (`{"kind": "agent_surprise", "data": {...}}`) — never internal (`tag = "type"`). The compile passes, JSON round-trips, but bincode hits `deserialize_any` panic at runtime. **Filed as discovery**: any future enum with associated data persisted via `RocksDbEvalStore` or similar bincode-backed store must verify against the real store, not just serde-JSON.
- **Default fast-track rank is intentional**: `AgentSurprise = 0`, `OutOfDistribution = 1`, `Uncertainty = 2`. Lower rank → higher priority. The choice was deliberate — agent-observed reality surprises are the strongest training signal, so they MUST drain before mere uncertainty backlog. The ranking is exercised by the new `agent_surprise_survives_eviction_when_uncertainty_rows_would_lose_first` unit test (capacity=1, AgentSurprise score 0.2 vs Uncertainty score 0.99, AgentSurprise wins).

## Failures

- **First FSV run errored** with `MEJEPA_EVAL_STORE: Bincode does not support the serde::Deserializer::deserialize_any method`. Caused by `#[serde(tag = "type")]` on `ActiveLearningKind`. Fixed by removing the attribute. Lesson: always FSV against the real store, not just JSON unit tests — the FSV-PROTOCOL §10 "tests pass when the project is in a broken state" trap.

## Successes

- `cargo test -p context-graph-mejepa --lib`: 80/80 passed (4 new in `eval::queue::tests` + 0 regressions in the existing 76).
- `cargo clippy -p context-graph-mejepa --no-deps --lib --examples -- -D warnings`: clean.
- `cargo build --release -p context-graph-mejepa --example active_learning_surprise_fsv`: clean.
- FSV evidence at `/mnt/d/contextgraph/fsv/active-learning-fsv/agent_surprise.json` shows `all_passed=true` with 1 happy-path case (`agent_surprise_round_trip` via real `RocksDbEvalStore`) + 3 boundary cases (`fast_track_eviction`, `empty_task_id_rejected`, `catastrophic_severity_score_one`).
- Real RocksDB on D-drive at `/mnt/d/contextgraph/fsv/active-learning-fsv/rocks-1778517844004/`.

## Open Blockers

None new.

## Tests / Build

- Tests added: 4 new in `eval::queue::tests`; integration via the FSV example.
- Tests passing: yes — 80/80 mejepa lib + 4/4 new + scheduler 6/6.
- Build status: `cargo build --release -p context-graph-mejepa --example active_learning_surprise_fsv` clean.

## Resume Here Next Session

- **Background corpus run** continues. Last observed: 527 oracle reports, still in subtle_flip primary phase, 0 verify reports yet. Index still shows complete=false with only 300 known_good entries committed. Watch the persistent monitor (`bufg0cd8k`) for the process-exit milestone, then run inspect_corpus_store + the post-corpus task #17 (already created).
- **Next-up Phase A**: with FLYWHEEL-005/006 done and FLYWHEEL-001/002 already complete, the remaining open Phase A tasks are:
  - TASK-FLYWHEEL-003 (test-outcome capture hook, 0.5d)
  - TASK-FLYWHEEL-004 (async oracle binding, 0.5d — blocks on 003)
  - TASK-FLYWHEEL-007 (park-list CF, 0.5d — now unblocked by FLYWHEEL-006)
  - TASK-PREDICT-001 (UserPromptSubmit injection, 1d)
  - TASK-PREDICT-002 (mejepa_record_agent_feedback MCP tool, ~0.5d after audit reductions)
  - TASK-LOOP-001, LOOP-007 (each 0.25d)
  - TASK-OBS-005 (C-drive lint, 0.25d)
- **Watch out for**: any new persisted enum types must avoid internally-tagged serde — confirmed by this session's real bug.

## Confidence: high

Real-RocksDB FSV passes (`all_passed=true`); 4 new unit tests + 0 regressions in 76-test mejepa lib suite; clippy clean; the bincode compatibility issue was caught and fixed at root cause (not papered over).
