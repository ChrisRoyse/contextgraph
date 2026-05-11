# Testing domain — all atomic tasks

## TASK-TEST-001 — E2E happy-path harness

- **Phase:** F · **Effort:** 2d · **Blocked by:** Phase A complete · **Spec:** SPEC-TESTING REQ-TEST-01 + TECH-TESTING §1-2
- **Outputs:** `tests/e2e/happy_path.rs`, `tests/e2e/harness.rs`.
- **DoD:** full Edit → predict → feedback → observe → improved-prediction loop runs end-to-end in CI; passes all assertions.

## TASK-TEST-002 — Restart-safety E2E

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/e2e/restart_safety.rs`.
- **DoD:** SIGKILL daemon mid-operation → restart → watermark resume verified; no shift loss; no double-counting.

## TASK-TEST-003 — Multi-agent E2E

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/e2e/multi_agent.rs`.
- **DoD:** 3 concurrent sessions; per-session predictions isolated; feedback events isolated.

## TASK-TEST-004 — Catastrophic-event E2E

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-TEST-001 + TASK-HEAL-006
- **Outputs:** `tests/e2e/catastrophic_event.rs`.
- **DoD:** drift catastrophic → alert + retrain queued + blocks → operator approval → promotion → recovery.

## TASK-TEST-005 — Storage-exhaustion E2E

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/e2e/storage_exhaustion.rs`.
- **DoD:** quota saturation triggers eviction; recovery within 110% of cap.

## TASK-TEST-006 — Chaos test harness with failpoints

- **Phase:** F · **Effort:** 2d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/chaos/failpoints_supervised.rs`; `failpoints` crate integration in supervised tasks.
- **DoD:** 5+ injected faults / 5min → daemon survives; alerts fired correctly.

## TASK-TEST-007 — Load test harness

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/load/concurrent_agents.rs`.
- **DoD:** 10-agent x 10-min run; latency p99 within budget per `18 §4`.

## TASK-TEST-008 — Soak test harness + nightly cron

- **Phase:** F · **Effort:** 2d · **Blocked by:** TASK-TEST-001
- **Outputs:** `tests/soak/seven_day.rs`; `scripts/soak/*.sh` orchestrators.
- **DoD:** 7-day soak run produces day-7 assertions document; no leaks, drift addressed.

## TASK-TEST-009 — Toolchain fallback matrix

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-PREDICT-LABEL-010
- **Outputs:** `tests/toolchain_fallback/matrix.rs`.
- **DoD:** for each per-language label source, missing toolchain → graceful degradation.

## TASK-TEST-010 — Adversarial test corpus

- **Phase:** F · **Effort:** 1d · **Blocked by:** TASK-PREDICT-005
- **Outputs:** `tests/fixtures/adversarial/{buggy_passing,knowngood_lookalike,embedder_targeted}/*.diff`; integration test runner.
- **DoD:** ≥80% non-Pass; 0 false-confident-Pass.

## TASK-TEST-011 — CI workflow updates

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-TEST-001..010
- **Outputs:** `.github/workflows/{e2e,chaos,load,soak}.yml`.
- **DoD:** PRs trigger E2E + chaos; nightly cron triggers chaos; weekly cron triggers load; quarterly manual triggers soak.
