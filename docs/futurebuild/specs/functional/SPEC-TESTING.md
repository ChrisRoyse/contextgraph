# SPEC-TESTING — Functional Specification

> **Level 2.** Cross-cutting integration / E2E / chaos / load / soak testing strategy.
> Implementation reference: [`18_INTEGRATION_AND_E2E_TESTING.md`](../../18_INTEGRATION_AND_E2E_TESTING.md).

## User stories

- **As a maintainer**, I want E2E tests that prove the whole system works before any release.
- **As an SRE**, I want chaos tests that prove the daemon survives faults.
- **As a researcher**, I want soak tests that catch slow drift over weeks.

## Requirements

### REQ-TEST-01 — Happy-path E2E
Single test exercises full Edit → shift → panel → predict → inject → feedback → test outcome → observe → improved prediction loop. Pass criterion: every step's expected artifact exists with expected SHA.

### REQ-TEST-02 — Restart-safety E2E
Hard-kill daemon mid-operation; restart; verify subscriber resumes from watermark; no loss; no double-counting.

### REQ-TEST-03 — Multi-agent E2E
3 concurrent sessions; per-session predictions isolated; per-session feedback events isolated.

### REQ-TEST-04 — Catastrophic-event E2E
Inject catastrophic drift; full retrain queues + blocks; operator approval unblocks; subsequent predictions improved.

### REQ-TEST-05 — Storage-exhaustion E2E
Quota saturation triggers emergency eviction; quota recovers ≤ 110% of cap.

### REQ-TEST-06 — Chaos test harness
`failpoints` crate-based fault injection on every supervised tokio task. Daemon survives 5+ injected faults / 5 min.

### REQ-TEST-07 — Load test harness
10 concurrent agents × 1 edit/s × 10 min. p99 prediction latency < 30s; subscriber lag never exceeds 50.

### REQ-TEST-08 — Soak test harness
7-day continuous run with synthetic load + injected drift on day 4 + catastrophic on day 5. Day 7 assertions: no leaks, drift addressed, witness chain valid.

### REQ-TEST-09 — Toolchain fail-closed matrix
Each enabled per-language label source fails closed when a required toolchain is absent. The error must identify the missing binary/config and no partial labels may be persisted.

### REQ-TEST-10 — Adversarial test corpus
30 adversarial cases (10 buggy-but-passing-tests + 10 KnownGood-look-alikes + 10 embedder-targeting). ≥80% correctly classified non-Pass; 0 false-confident-Pass.

### REQ-TEST-11 — CI workflow integration
Unit + per-task FSV + integration on every PR; E2E on every PR (15-min budget); chaos nightly; load weekly; soak quarterly.

## Acceptance criteria

1. `cargo test --workspace --features e2e` runs the full E2E happy-path suite.
2. `cargo test --workspace --features chaos` runs chaos tests with failpoints.
3. CI YAML invokes the right tier on the right schedule.
4. Test results land at `/mnt/d/contextgraph/exports/test-runs/<date>/` for operator review.

## Non-functional

- E2E suite total wall-time < 15 min.
- Chaos suite total wall-time < 60 min.
- Load suite < 4 hours.
- Soak suite (intentionally) 7 days.
