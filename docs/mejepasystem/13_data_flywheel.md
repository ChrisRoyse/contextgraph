# 13 — Data Flywheel (Shift Subscriber + Reward Channels)

Source files covered: `crates/context-graph-mejepa-shift-subscriber/src/{lib,runner,subscriber,tail,watermark,cache,shift_transform,models}.rs`, `crates/context-graph-mcp/src/daemon.rs`, `crates/context-graph-mejepa/src/heal/scheduler.rs`, `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md`, `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`.

---

## 1. The 5 reward channels

| # | Channel | Source | Frequency | Label kind |
|---|---|---|---|---|
| C1 | **Oracle ground truth** | Docker tests + live `pytest`/`cargo test` | per invocation | hard: per-test pass/fail |
| C2 | **Shift channel** | every Edit/Write → pre/after SHA + git diff | per file mutation (~250ms) | soft: panel transition for SSL |
| C3 | **Agent-feedback** | `mejepa_record_agent_feedback` calls | event-driven | weak: prediction-correctness with severity |
| C4 | **Operator-override** | active-learning queue label | event-driven | hard: gold-standard |
| C5 | **Static-analysis snapshot deltas** | clippy/pylint/eslint/bandit/cargo-audit before vs after | per shift | soft: warning class deltas → Q4 heads |

---

## 2. Shift subscriber

Source: `crates/context-graph-mejepa-shift-subscriber/`.

| Component | Purpose |
|---|---|
| `ShiftLogTail` | Cursor-based JSONL tail at 250ms |
| `WatermarkWriter` | Persists last-consumed byte offset to `CF_MEJEPA_SHIFT_WATERMARK` per session |
| `InstrumentCache` | Keyed by `(slot, file_sha)` |
| `ShiftSubscriber::open` | Reconstructs cursor from watermark on restart |
| `ShiftSubscriber::run_until_idle` | Drain pending shifts until none remain |
| `ShiftSubscriber::run_until_shutdown` | Run forever with watch-channel shutdown |
| `shift_to_inference` | Convert shift → Panel → verify() → RealityPrediction |

Latency budget: **p50 < 5s, p99 < 30s**. Lag alert at > 500 unprocessed shifts / 1 min sustained → `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT`. Escalate after 5 min.

---

## 3. Per-shift pipeline

```
PostToolUse hook fires → appends JSONL to /mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl

~250ms later (poll_next_line tick):

ShiftSubscriber.poll_next_line() reads line:
  { ts, session_id, file, before_sha, after_sha, line_count_before, line_count_after,
    cargo_check_result, git_diff_summary, hook_id }

materialize_shift(shift) -> PanelMaterializationOutcome:
  1. Read file at after_sha
  2. memory::ast::chunk_with_options(content, language, opts) → Vec<Chunk>
  3. for each chunk:
     a. routing::lookup(chunk.lang, chunk.kind) → embedder set
     b. for each embedder: EmbedderForward::forward(chunk)
     c. compute DDA signals → CF_MEJEPA_DDA_SIGNALS
  4. PanelBuilder.build() → Panel(5,120-d) → CF_MEJEPA_PANELS
  5. update CF_MEJEPA_SHIFT_WATERMARK with consumed shift_id

shift_to_inference(panel, prior_panel, action):
  - predictor.forward → predicted_panel + per-head outputs
  - ConformalPredictor.wrap → interval
  - mejepa_tct::gtau::gtau_check_chunks → violations
  - assemble RealityPrediction
  - persist to CF_MEJEPA_LIVE_PREDICTIONS keyed by session_id

(asynchronously, when oracle outcome arrives):

observe(panel_t, action, panel_t+1, oracle_outcome):
  - EWC++ regularized SGD step
  - compute ΔP/ΔK/ΔΩ/ΔΞ → L_step
  - update sampling_weight for involved chunks
  - write TrainingCertificate to CF_MEJEPA_TRAIN_CERTS (witness-chain bound)
  - if drift detector hits Soft/Hard, queue intervention
  - if SurpriseEvent on this prediction, bump weight by 1+2·severity_score
```

---

## 4. Latency budget breakdown

| Stage | Time |
|---|---|
| AST chunking | ~50 ms typical file |
| Embedder forwards (neural, parallelized over chunks) | ~200 ms |
| DDA signals | ~50 ms (cached MI; only fresh cosines) |
| Panel build + persist | ~100 ms |
| Predictor forward | ~50 ms (CUDA, batch=1) |
| Conformal + Gτ | ~10 ms |
| **Total p50** | **~500 ms** |
| **p99** | dominated by embedder forward tail latency |

---

## 5. Test-outcome capture hook (futurebuild `TASK-FLYWHEEL-003`)

PostToolUse on `Bash` captures `pytest`/`cargo test`/`jest` output via regex parsing:

```rust
struct TestOutcomeEvent {
    ts: SystemTime,
    session_id: SessionId,
    test_id: TestId,
    outcome: TestOutcome,  // Pass | Fail | Error | Skip
    duration_ms: u64,
    error_log: Option<String>,
}
```

Persisted to `CF_MEJEPA_ORACLE_VERDICTS` keyed by `(session_id, test_id, ts)`. The scheduler's `observe_tick` picks it up on next 250ms tick.

---

## 6. Asynchronous oracle binding (futurebuild `TASK-FLYWHEEL-004`)

Predictions are made BEFORE tests run. Oracle outcomes arrive LATER. The scheduler binds retroactively:

```
On new oracle entry in CF_MEJEPA_ORACLE_VERDICTS:
  1. Look up most-recent prediction in CF_MEJEPA_LIVE_PREDICTIONS for this session
     that includes this test in predicted_failed_tests OR predicted_passed_tests
  2. Look up corresponding panel
  3. Call observe(panel, action, panel_after_test, oracle=outcome)
     - prediction correctness computed
     - if wrong AND severity High/Critical, queue ActiveLearningKind::PredictionMissed
     - panel + outcome → training cert chain entry
```

---

## 7. Agent-feedback path (futurebuild `TASK-PREDICT-002`)

```
Agent calls mejepa_record_agent_feedback(prediction_id, kind, explanation, severity, actual_outcome)

  → SurpriseEvent persisted to CF_MEJEPA_AGENT_FEEDBACK keyed by (prediction_id, agent_id, ts)
  → Active-learning queue entry created (kind=AgentSurprise, high priority)
  → On next active_learning_tick (≤ 600s):
       - reads SurpriseEvent
       - calls observe() with sampling weight × (1 + 2 · severity_score)
       - updates calibration-error tracking for the head that made the wrong prediction
  → If severity = Catastrophic AND calibration error > drift threshold:
       - fast-track to lora_refresh OR full_retrain queue
  → On operator side: SurpriseEvent appears in weekly eval report under "agent-flagged misses"
```

---

## 8. The replay buffer (BatchSampler)

Per `crates/context-graph-mejepa-train/src/sampler/`:

```
sampling_weight(ex) = base_weight × L_step(ex)
                    × (1 + 5 · operator_override_flag)
                    × (1 + 2 · agent_surprise_severity_score)
                    × decay^age_in_days
```

| Behavior | Effect |
|---|---|
| High `L_step` (wrong prediction) | over-sampled |
| Operator override | 6× weight |
| Agent-flagged surprise | 1+2·severity weight bump |
| Stale low-L_step | decays out (`decay=0.995`, half-life ~140d) |
| `sampling_weight < 0.05` | drops from current epoch |
| `sampling_weight > 1.0` | guaranteed inclusion |

**Self-curating**: the replay buffer continuously over-fits to hard-and-recent examples.

---

## 9. New CFs added by the flywheel (Phase A, **shipped 2026-05-11**)

| CF | Key | Value | Purpose |
|---|---|---|---|
| `CF_MEJEPA_DDA_SIGNALS` | `(panel_id, chunk_id)` | `DdaSignals` | n·(N+N²) cache |
| `CF_MEJEPA_FAILURE_EXEMPLARS` | `(failure_mode_class, embedder_violation_signature)` | `Vec<ExemplarRef>` | per-failure-mode reverse index |
| `CF_MEJEPA_AGENT_FEEDBACK` | `(prediction_id, agent_id, ts)` | `SurpriseEvent` | agent-side feedback ledger |

All registered via `mejepa-cf` and wired through `open_hygiene_rocksdb`. FSV at `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json`.

---

## 10. Failure modes the flywheel must survive

| Failure | Detection | Response |
|---|---|---|
| Subscriber lag > 500/min | `mejepa_subscriber_status` | `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT`; daemon escalates > 5 min |
| Scheduler tokio panic | `tokio::JoinSet` supervisor | restart with exp. backoff (cap 60s); `MEJEPA_SCHEDULER_FLAPPING` > 5 restarts/5 min |
| Catastrophic drift | `heal::drift::classify` | A/B/C retrain alert; pause prediction emission |
| Storage overflow | `hygiene::quota::quota_status` | emergency eviction; if still overflowing, alert + halt panel writes |
| Embedder digest mismatch | startup digest check | refuse to start; `MEJEPA_EMBEDDER_DIGEST_MISMATCH` |
| Oracle outcome rate drops | rolling-window arrival rate | log warning; observe() falls back to prediction-only confidence |
| RocksDB write contention | latency telemetry | auto-shrink batch size; if persistent, alert |

---

## 11. MCP daemon mode

Source: `crates/context-graph-mcp/src/daemon.rs`. Started by:

```bash
cargo run --release -p context-graph-mcp -- \
    --daemon-server-only \
    --d-root /mnt/d/contextgraph \
    --daemon-port 3100 \
    --http-port 3101
```

Or via systemd: `systemctl --user enable --now mejepa-daemon`.

| Supervised task | What it runs |
|---|---|
| Shift subscriber | `ShiftSubscriber::run_until_shutdown` |
| Self-optimization scheduler | `heal::scheduler::run_self_optimization_scheduler` |
| Nightly GC scheduler | `hygiene::gc::gc_run_nightly` at 03:00 local |
| Weekly evaluation scheduler | `eval::runner::run_holdout_eval` weekly |

All under `tokio::JoinSet` supervisor; exponential backoff; flap detection.

---

## 12. Operator-visible health tools

| Tool | Status |
|---|---|
| `mejepa_subscriber_status` | shipped |
| `mejepa_heal_status` | shipped |
| `mejepa_quota_status` | shipped |
| `mejepa_vram_budget_report` | shipped |
| `mejepa_daemon_status` (aggregator) | futurebuild Phase F |

---

## 13. Cross-references

[`07_instruments_panel.md`](07_instruments_panel.md) (panel materialization), [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md) (scheduler tickers), [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md) (storage budget), [`20_hooks_layer.md`](20_hooks_layer.md) (shift-log generation).
