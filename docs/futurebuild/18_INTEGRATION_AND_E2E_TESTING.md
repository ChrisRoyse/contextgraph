# 18 — Integration, End-to-End, Chaos, and Load Testing

> **Purpose.** Per-task FSV proves each primitive in isolation. This doc specifies the cross-cutting tests that prove the *whole system* works: provenance integrity end-to-end, watermark restart-safety, daemon supervision under chaos, multi-agent concurrency, full data flywheel from Edit → prediction → feedback → re-train → improved prediction.
>
> Owned by [`SPEC-TESTING.md`](specs/functional/SPEC-TESTING.md) + [`TECH-TESTING.md`](specs/technical/TECH-TESTING.md).

---

## 1. Test taxonomy

| Tier | Scope | Tooling | Frequency |
|---|---|---|---|
| Unit (per-crate) | Single function / module | `cargo test -p <crate>` | every commit |
| Per-task FSV | One task's definition-of-done | crate `examples/*_fsv.rs` writing to `/mnt/d/contextgraph/fsv/` | when task lands |
| Integration | Cross-crate, single process | `cargo test --workspace --features integration` | every PR |
| **End-to-end** (NEW) | Full daemon + hooks + agent + corpus + Docker oracle | dedicated test harness (`tests/e2e/`) | every PR |
| **Chaos** (NEW) | E2E + injected faults (panic, network partition, disk full, OOM) | dedicated harness with `failpoints` crate | nightly |
| **Load** (NEW) | E2E + N concurrent agents | dedicated harness with k6-equivalent | weekly |
| **Soak** (NEW) | E2E running for 7 days; check for drift, leaks, degradation | dedicated harness | quarterly |

The shipped substrate covers Tier 1-3 well per `15_PHASE_1_5_HARDENING_FSV_2026_05_10.md` and `16_PHASE_6_8_HARDENING_FSV_2026_05_11.md`. Tiers 4-7 are the gap this doc fills.

---

## 2. End-to-end test scenarios

### 2.1 The "happy path" E2E

```
1. Boot daemon: cargo run --release -p context-graph-mcp -- --daemon --d-root /tmp/e2e-d-root
2. Create a synthetic source file in a fixture repo
3. Simulate Claude Code Edit → PostToolUse hook → shift-log line lands
4. Wait ≤ 250ms; assert daemon's shift subscriber consumed the line
5. Assert RealityPrediction landed in CF_MEJEPA_LIVE_PREDICTIONS for the session
6. Simulate UserPromptSubmit → assert prediction-injection markdown is rendered
7. Run synthetic pytest → test outcome captured by hook
8. Wait ≤ 250ms; assert observe() ran and CF_MEJEPA_TRAIN_CERTS got a new entry
9. Call mejepa_record_agent_feedback with kind=Surprise → assert SurpriseEvent persisted, sampling weight bumped
10. Trigger a synthetic drift (force several wrong predictions); assert drift detector fires within 1k obs; assert LoRA refresh runs; assert promotion happens; assert next prediction is improved
11. Cleanly shut down daemon; assert no panics, no orphan processes
```

### 2.2 Restart safety E2E

```
1. Boot daemon
2. Generate 100 shifts; wait for subscriber to consume
3. Hard-kill daemon (SIGKILL)
4. Restart daemon
5. Generate 100 more shifts
6. Assert subscriber resumes from last watermark; no double-counting; no skipped shifts
7. Assert all 200 panels are in CF_MEJEPA_PANELS
```

### 2.3 Multi-agent E2E

```
1. Boot daemon
2. Simulate 3 concurrent agent sessions (different session_ids)
3. Each agent makes 10 edits
4. Assert each session has its own RealityPrediction in CF_MEJEPA_LIVE_PREDICTIONS
5. Each agent reads its prediction (UserPromptSubmit hook)
6. Assert no cross-session prediction leakage
7. Each agent records 1 SurpriseEvent
8. Assert each event is bound to the correct session's prediction
```

### 2.4 Catastrophic event E2E

```
1. Boot daemon with a small corpus
2. Simulate 1000 wrong predictions (force oracle outcomes that disagree)
3. Assert drift detector fires Catastrophic
4. Assert operator alert written to CF_MEJEPA_HEAL_REPORTS with severity=Catastrophic
5. Assert full retrain queued but BLOCKED until mejepa_promote_approval called
6. Operator approves via mejepa_promote_approval; assert retrain runs and promotion executes
7. Subsequent predictions show recovery
```

### 2.5 Storage exhaustion E2E

```
1. Boot daemon with quota set artificially low (1 GB)
2. Generate enough shifts to exceed quota
3. Assert emergency eviction tick fires
4. Assert quota_status returns ≤ 110% of cap after eviction
5. Assert operator alert written if hard-cap-exceeded for > 5 minutes
```

---

## 3. Chaos test scenarios

| Scenario | Injection | Expected behavior |
|---|---|---|
| Subscriber tokio panic | `failpoints::on("subscriber.consume")` triggers panic | JoinSet supervisor restarts; state recovers from watermark |
| Scheduler tokio panic | failpoint in `tick_drift` | restart with backoff; no double-promotion |
| RocksDB write fails (e.g., disk full mid-write) | failpoint in CF write | observe() returns error; training cert NOT written; sample is skipped |
| Embedder forward returns NaN | failpoint in `EmbedderForward::forward` | prediction marked OOD; `MEJEPA_EMBEDDER_NAN` logged; no panel persisted |
| Witness chain integrity check finds corruption | inject hash mismatch | auto-repair attempted; if fail, panel writes paused + alert |
| Hook script crashes | replace hook with `exit 1` | agent's Edit succeeds; shift entry is lost; daemon logs missing-shift |
| `nvidia-smi` returns malformed output | mock command | telemetry returns `unavailable: true`; daemon still runs |
| 5 supervised tasks panic in 5 min | failpoints flap | `MEJEPA_SCHEDULER_FLAPPING` alert; cooldown enforced |
| Network partition between hooks and MCP daemon | block socket access | hook silently exits 0; agent's edit completes; shift queued for retry |
| Concurrent two daemon instances | start a second one | second fails pid lock; clean exit `MEJEPA_DAEMON_LOCK_HELD` |

Implementation: `cargo test -p e2e-tests --features chaos` with `failpoints` crate; CI nightly job.

---

## 4. Load test scenarios

| Scenario | Load profile | Pass threshold |
|---|---|---|
| 10 concurrent agents, 1 edit/s each | sustained 10 min | p99 prediction latency < 30s; subscriber lag never exceeds 50 |
| 100 shifts/min for 1 hour | bursty | no shift loss; lag alert never fires for > 60s |
| 1k feedback events / hour from 10 agents | continuous | rate limit kicks in at 100/hr/agent; daemon stable |
| Continuous training during inference | trainer + inference at saturation | inference p99 < 3× idle latency; training cert rate ≥ 50% of idle rate |

Tooling: custom harness or `goose`-equivalent for Rust; results emitted to `/mnt/d/contextgraph/exports/load-tests/<date>.json`.

---

## 5. Soak test (7-day continuous run)

Goal: detect slow-onset failures (memory leak, file-descriptor leak, slow drift, GC inefficiency).

Setup:
- Boot daemon
- Synthetic agent makes 1 edit/min for 7 days
- 1% of edits trigger `mejepa_record_agent_feedback` with random severity
- Synthetic drift injected on day 4 (90% → 85% accuracy)
- Synthetic catastrophic event on day 5 (75% accuracy)

Assertions on day 7:
- No daemon restarts (or, if restarts, all clean)
- Memory RSS within 20% of day-1 baseline
- File descriptor count within 10% of day-1
- Disk usage < 115 GB hard cap (GC kept it bounded)
- Drift detected on day 4 → LoRA refresh promoted
- Catastrophic on day 5 → alert written; full retrain queued (operator simulator approves on day 5.5 → promotion executes)
- Day-6 prediction accuracy returns to > 88%
- All training certs hash-chain verified

---

## 6. Per-language toolchain detection (D2)

Some prediction-label sources require external toolchains:
- E_TypeGraph for Python: `pyright` / `mypy`
- E_StaticAnalysis: `bandit`, `pylint`, `clippy`, `eslint`, `cargo-audit`, `semgrep`
- Perf head: `pytest-benchmark`, `cargo bench`, `valgrind`
- Cost head: CI runner's time output

The integration tests SHALL include a "toolchain fallback" matrix:
- If `pyright` not present, mark E_TypeGraph fields as `unavailable: true` and use deterministic fallback
- If `cargo` not present in the agent's environment, the Rust prediction heads degrade to deterministic-only
- Operator can configure required-vs-optional toolchains per language

`TASK-PREDICT-LABEL-010-toolchain-fallback.md` covers this.

---

## 7. Test data corpus

Beyond Phase 0's 300×8 SWE-bench Lite mutation corpus, the integration tests use:
- 50 synthetic micro-tasks per language (fixture repos with known fixes)
- 100 historical CI failure logs from public Rust/Python/JS repos for label-source training
- 30 adversarial test cases (intentionally crafted to fool the predictor — see `22`)

Stored under `tests/fixtures/e2e/` (committed to git, small files only).

---

## 8. CI / cron schedule

| Test tier | Trigger | Where it runs |
|---|---|---|
| Unit | every push | local + CI |
| Per-task FSV | when task PR lands | CI |
| Integration | every PR | CI |
| End-to-end | every PR | CI (15-min budget) |
| Chaos | nightly | dedicated CI job (1-hour budget) |
| Load | weekly Sunday 02:00 UTC | dedicated CI job (4-hour budget) |
| Soak | quarterly | manually triggered on dedicated machine |

Results land in `/mnt/d/contextgraph/exports/test-runs/<date>/` for operator review.

---

## 9. What this commits us to

Tasks (in `tasks/testing/`):

- `TASK-TEST-001` — E2E happy-path harness
- `TASK-TEST-002` — restart-safety E2E
- `TASK-TEST-003` — multi-agent E2E
- `TASK-TEST-004` — catastrophic-event E2E
- `TASK-TEST-005` — storage-exhaustion E2E
- `TASK-TEST-006` — chaos test harness with failpoints
- `TASK-TEST-007` — load test harness
- `TASK-TEST-008` — soak test harness + nightly cron job
- `TASK-TEST-009` — toolchain fallback matrix (cross-cuts label-source tasks)
- `TASK-TEST-010` — adversarial test corpus (cross-cuts `22`)
- `TASK-TEST-011` — CI workflow updates (nightly chaos, weekly load, quarterly soak)
