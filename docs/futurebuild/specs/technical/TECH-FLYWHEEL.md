# TECH-FLYWHEEL — Technical Specification

> **Level 3.** Defines HOW the data flywheel is built. Implements [`SPEC-FLYWHEEL.md`](../functional/SPEC-FLYWHEEL.md).

**Status snapshot (2026-05-12):** The daemon supervisor, D-root validation, scheduler skeleton, systemd template, test-capture hook, flywheel CF registrations, `AgentSurprise`, surprise sampling weight, and supervised GC/eval job-run ledger are code/FSV-shipped. Full scheduler policy, populated weekly production eval feed, and fresh-host enablement proof remain open.

---

## Architecture

The flywheel is a single supervised process (the MCP daemon) hosting five cooperating tokio tasks: the JSON-RPC handler, the shift subscriber, the self-optimization scheduler, the nightly GC scheduler, and the weekly evaluation scheduler. Task starts, scheduled run completions/errors, and task exits are persisted to `CF_MEJEPA_FLYWHEEL_JOB_RUNS` with sync write + readback.

```
┌─────────────────────────────────────────────────────────────────────────┐
│ MCP Daemon process (cargo run --release -p context-graph-mcp -- --daemon) │
│                                                                          │
│   ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────┐    │
│   │ MCP stdio handler│  │ Shift subscriber │  │ Self-optim         │    │
│   │ (JSON-RPC)       │  │ (run_until_      │  │ scheduler          │    │
│   │                  │  │  shutdown)       │  │ (7 tickers)        │    │
│   └──────────────────┘  └──────────────────┘  └────────────────────┘    │
│                                                                          │
│   ┌──────────────────┐  ┌──────────────────────────────────────────┐    │
│   │ Nightly GC       │  │ Weekly eval cron                          │    │
│   │ scheduler        │  │ (writes /mnt/d/.../exports/eval/<date>/)  │    │
│   └──────────────────┘  └──────────────────────────────────────────┘    │
│                                                                          │
│   JoinSet supervisor: detects panics, restarts with exponential backoff │
│   Job-run ledger: CF_MEJEPA_FLYWHEEL_JOB_RUNS + scheduler status JSON     │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Module layout

```
crates/context-graph-mcp/
  src/
    main.rs                      # extended with --daemon flag (REQ-FLYWHEEL-01)
    daemon.rs                    # supervisor + JoinSet + restart logic + job-run ledger (REQ-FLYWHEEL-03)
    daemon_validate.rs           # D-drive validation (REQ-FLYWHEEL-02)
    handlers/
      tools/
        mejepa_tools.rs          # extended with mejepa_record_agent_feedback (Phase A)
        mejepa_phase7_tools.rs   # already shipped: observe_shift, subscriber_status, capture_audit
crates/context-graph-mejepa/
  src/
    heal/
      scheduler.rs               # run_self_optimization_scheduler skeleton (REQ-FLYWHEEL-05)
      ...                        # existing heal modules consumed by scheduler
crates/context-graph-mejepa-cf/
  src/
    lib.rs                       # extended: register CF_MEJEPA_DDA_SIGNALS, _FAILURE_EXEMPLARS, _AGENT_FEEDBACK, _FLYWHEEL_JOB_RUNS (REQ-FLYWHEEL-10)
crates/context-graph-mejepa-train/
  src/
    sampler/
      batch_sampler.rs           # extended: surprise-weight bump (REQ-FLYWHEEL-12)
scripts/
  systemd/
    mejepa-daemon.service        # systemd template (REQ-FLYWHEEL-15)
  cc/
    hook_post_tool_use_test_capture.sh  # pytest/cargo-test/jest output parser (REQ-FLYWHEEL-08)
```

---

## Key data structures

### `SchedulerState`

```rust
// crates/context-graph-mejepa/src/heal/scheduler.rs
pub struct SchedulerState {
    storage: Storage,
    pending_lora_refresh: Option<HeadId>,
    pending_full_retrain: bool,
    pending_promote_approvals: Vec<PromotionCandidate>,
    consumed_shift_watermark: ShiftId,
    last_drift_class: DriftLevel,
    last_continual_backprop: Instant,
    last_constellation_refresh: Instant,
}
```

### `SurpriseEvent`

```rust
// crates/context-graph-mejepa/src/types.rs (extension)
pub struct SurpriseEvent {
    pub feedback_id: FeedbackId,
    pub prediction_id: PredictionId,
    pub agent_id: AgentId,
    pub ts: SystemTime,
    pub feedback_kind: FeedbackKind,
    pub agent_explanation: String,
    pub actual_outcome: Option<ActualOutcome>,
    pub severity: SurpriseSeverity,
    pub extra_structured_data: serde_json::Value,
    pub witness_hash: WitnessHash,
}

pub enum FeedbackKind { Confirmed, Surprise, Omission, Calibration }
pub enum SurpriseSeverity { Low, Medium, High, Catastrophic }
```

### `TestOutcomeEvent`

```rust
// crates/context-graph-mejepa-shift-subscriber/src/types.rs
pub struct TestOutcomeEvent {
    pub ts: SystemTime,
    pub session_id: SessionId,
    pub test_id: TestId,
    pub outcome: TestOutcome,
    pub duration_ms: u64,
    pub error_log: Option<String>,
    pub witness_hash: WitnessHash,
}

pub enum TestOutcome { Pass, Fail, Error, Skip }
```

### `RealitySignalEnvelope`

`RealitySignalEnvelope` is the cross-channel audit envelope for `REQ-FLYWHEEL-16`. It prevents a capture path from claiming success unless the event is connected to ME-JEPA's training, calibration, evaluation, or serving surface.

```rust
pub struct RealitySignalEnvelope {
    pub event_id: String,
    pub event_kind: RealityEventKind,
    pub session_id: String,
    pub repo_before_sha: Option<String>,
    pub repo_after_sha: Option<String>,
    pub trigger_ref: String,
    pub ast_chunk_ids: Vec<String>,
    pub active_embedder_ids: Vec<String>,
    pub dda_row_keys: Vec<String>,
    pub tct_cell_keys: Vec<String>,
    pub panel_id: Option<String>,
    pub prediction_id: Option<String>,
    pub outcome_refs: Vec<String>,
    pub mejepa_consumers: Vec<MeJepaConsumer>,
    pub witness_hash: String,
}

pub enum RealityEventKind { Edit, Write, TestOutcome, AgentFeedback, OperatorOverride, StaticAnalysis, RuntimeTelemetry }
pub enum MeJepaConsumer { PredictorHead(String), CalibrationWindow, ReplayWeight, DriftDetector, TctRefresh, TrainingCertificate, ActiveLearningQueue }
```

Validation rules:

- `event_id`, `session_id`, `trigger_ref`, `witness_hash`, and `mejepa_consumers` are required.
- code-changing events require `repo_before_sha`, `repo_after_sha`, and at least one `ast_chunk_id`;
- prediction-producing events require `panel_id` and `prediction_id`;
- DDA/TCT-bearing events require at least one `dda_row_key` or `tct_cell_key`;
- oracle, feedback, static-analysis, runtime, and operator events require at least one `outcome_ref`;
- unknown or empty consumers fail closed with `MEJEPA_REALITY_SIGNAL_UNCONNECTED`.

---

## Daemon main loop pseudocode

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let opts = parse_opts()?;
    
    // REQ-FLYWHEEL-02
    daemon_validate::validate_d_drive_root(&opts.d_root)?;
    
    // Open storage
    let storage = Storage::open(&opts.d_root.join("storage/contextgraph-rocksdb"))?;
    
    // Lock the daemon pid file
    let _lock = pid::acquire(&opts.d_root.join("state/locks/context-graph-daemon.pid"))?;
    
    // Always-on JSON-RPC handler
    let mut tasks = tokio::task::JoinSet::new();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    
    tasks.spawn(run_mcp_stdio(storage.clone(), opts.clone(), shutdown_rx.clone()));
    
    if opts.daemon {
        tasks.spawn(supervised(
            "shift_subscriber",
            run_shift_subscriber(storage.clone(), opts.shift_log_root.clone(), shutdown_rx.clone()),
        ));
        tasks.spawn(supervised(
            "self_optimization",
            run_self_optimization_scheduler(storage.clone(), opts.heal_config.clone(), shutdown_rx.clone()),
        ));
        tasks.spawn(supervised(
            "nightly_gc",
            run_nightly_gc_scheduler(storage.clone(), opts.gc_config.clone(), shutdown_rx.clone()),
        ));
        tasks.spawn(supervised(
            "weekly_eval",
            run_weekly_eval_scheduler(storage.clone(), opts.eval_config.clone(), shutdown_rx.clone()),
        ));
    }
    
    // Signal handlers
    let sig = signal::ctrl_c();
    tokio::pin!(sig);
    sig.await?;
    let _ = shutdown_tx.send(true);
    
    // Drain
    while let Some(res) = tasks.join_next().await {
        if let Err(e) = res {
            tracing::warn!(?e, "task exited with error");
        }
    }
    Ok(())
}

async fn supervised<F, T>(name: &'static str, fut: F) -> Result<T>
where F: std::future::Future<Output = Result<T>>,
{
    let mut backoff = Duration::from_millis(100);
    loop {
        let res = AssertUnwindSafe(fut).catch_unwind().await;
        match res {
            Ok(Ok(t)) => return Ok(t),
            Ok(Err(e)) => {
                tracing::error!(task = name, ?e, "task returned error");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(60));
            }
            Err(panic) => {
                tracing::error!(task = name, ?panic, "task panicked; restarting");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(60));
                if restart_count_in_window(name) > 5 {
                    write_alert(AlertSeverity::Catastrophic, format!("MEJEPA_SCHEDULER_FLAPPING: {}", name));
                }
            }
        }
    }
}
```

---

## Test outcome capture hook

```bash
#!/usr/bin/env bash
# scripts/cc/hook_post_tool_use_test_capture.sh
# Wired in .claude/settings.json under hooks.PostToolUse.Bash

set -euo pipefail

TOOL_OUTPUT="${CLAUDE_TOOL_OUTPUT:-}"
SESSION_ID="${CLAUDE_SESSION_ID:-default}"
COMMAND="${CLAUDE_TOOL_INPUT_COMMAND:-}"

# Detect test framework
case "$COMMAND" in
    *pytest*)         FRAMEWORK="pytest" ;;
    *"cargo test"*)   FRAMEWORK="cargo-test" ;;
    *"jest"*)         FRAMEWORK="jest" ;;
    *"go test"*)      FRAMEWORK="go-test" ;;
    *)                exit 0 ;;  # not a test command
esac

# Parse and write events
echo "$TOOL_OUTPUT" | python3 scripts/cc/parse_test_output.py \
    --framework "$FRAMEWORK" \
    --session-id "$SESSION_ID" \
    | while read -r event_json; do
        # Append to test-outcome JSONL (consumed by daemon's observe_tick)
        echo "$event_json" >> "/mnt/d/contextgraph/runtime/test-outcomes/$SESSION_ID.jsonl"
    done
```

---

## Sampling-weight bump implementation

```rust
// crates/context-graph-mejepa-train/src/sampler/batch_sampler.rs (extension)
pub fn compute_sampling_weight(
    ex: &TrainingExample,
    feedback: Option<&SurpriseEvent>,
) -> f32 {
    let base = 1.0_f32;
    let l_step = ex.l_step;
    let operator_override = if ex.operator_override { 6.0 } else { 1.0 };
    let surprise_bump = if let Some(f) = feedback {
        let severity = match f.severity {
            SurpriseSeverity::Low => 0.2,
            SurpriseSeverity::Medium => 0.5,
            SurpriseSeverity::High => 0.8,
            SurpriseSeverity::Catastrophic => 1.0,
        };
        1.0 + 2.0 * severity
    } else { 1.0 };
    let age_days = ex.age_in_days as f32;
    let decay = 0.995_f32.powf(age_days);
    
    base * l_step * operator_override * surprise_bump * decay
}
```

---

## CF schemas

```rust
// crates/context-graph-mejepa-cf/src/lib.rs (extension)
pub const CF_MEJEPA_DDA_SIGNALS: &str = "mejepa_dda_signals";
pub const CF_MEJEPA_FAILURE_EXEMPLARS: &str = "mejepa_failure_exemplars";
pub const CF_MEJEPA_AGENT_FEEDBACK: &str = "mejepa_agent_feedback";

// CF_MEJEPA_DDA_SIGNALS
//   key: bincode((PanelId, ChunkId))
//   value: serde_json(DdaSignals)
pub struct DdaSignals {
    pub per_embedder_cosine: Vec<f32>,           // length = N (active embedders)
    pub pairwise_cosine_upper: Vec<f32>,         // length = N*(N-1)/2 (upper triangle)
    pub pairwise_mi_upper: Vec<f32>,             // cached weekly
    pub blind_spot_z_scores: Vec<f32>,           // length = N*(N-1)/2
}

// CF_MEJEPA_FAILURE_EXEMPLARS
//   key: bincode((FailureModeClass, EmbedderViolationSignature))
//   value: serde_json(Vec<ExemplarRef>)
pub struct ExemplarRef {
    pub task_id: SwebenchTaskId,
    pub mutation_kind: MutationKind,
    pub chunk_id: ChunkId,
    pub oracle_outcome: OracleVerdict,
    pub witness_hash: WitnessHash,
}

// CF_MEJEPA_AGENT_FEEDBACK
//   key: bincode((PredictionId, AgentId, MillisSinceEpoch))
//   value: serde_json(SurpriseEvent)
```

---

## Asynchronous oracle binding

```rust
// crates/context-graph-mejepa/src/heal/scheduler.rs (within observe_tick handler)
async fn drain_pending_oracle_outcomes(state: &mut SchedulerState) -> Result<()> {
    let outcomes = read_pending_test_outcomes(&state.storage)?;
    for outcome in outcomes {
        // Find the most recent prediction for this session that includes this test
        let pred = lookup_prediction_for_test(&state.storage, &outcome.session_id, &outcome.test_id)?;
        let Some((pred_id, pred, panel)) = pred else { continue };
        
        // Build the panel after the test (typically the current panel for the session)
        let panel_after = lookup_latest_panel_for_session(&state.storage, &outcome.session_id)?;
        
        // Compute correctness
        let was_correct = pred.predicted_failed_tests.iter()
            .find(|t| t.test_id == outcome.test_id)
            .map(|t| t.predicted_outcome == outcome.outcome)
            .unwrap_or(false);
        
        // Call observe()
        state.compiler.observe(&panel, &pred.action, &panel_after, OracleOutcome::from(outcome.clone())).await?;
        
        // If wrong + high severity → enqueue active-learning
        if !was_correct && pred.confidence_interval.upper > 0.85 {
            state.queue_active_learning(ActiveLearningEntry {
                kind: ActiveLearningKind::PredictionMissed,
                prediction_id: pred_id,
                ...
            }).await?;
        }
        
        // Mark the outcome consumed
        mark_outcome_consumed(&state.storage, &outcome)?;
    }
    Ok(())
}
```

---

## systemd unit template

```ini
# scripts/systemd/mejepa-daemon.service
# Install: cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
#          systemctl --user daemon-reload
#          systemctl --user enable --now mejepa-daemon
[Unit]
Description=ContextGraph ME-JEPA Daemon
After=network.target

[Service]
Type=simple
ExecStartPre=/bin/sh -c 'test -d /mnt/d/contextgraph || (echo "MEJEPA_D_DRIVE_UNAVAILABLE" && exit 1)'
ExecStart=/home/cabdru/contextgraph/target/release/context-graph-mcp --daemon --d-root /mnt/d/contextgraph
WorkingDirectory=/home/cabdru/contextgraph
Restart=on-failure
RestartSec=5s
StandardOutput=append:/mnt/d/contextgraph/logs/mejepa-daemon.stdout.log
StandardError=append:/mnt/d/contextgraph/logs/mejepa-daemon.stderr.log
Environment=RUST_LOG=info,context_graph_mejepa=debug

[Install]
WantedBy=default.target
```

---

## Error taxonomy

```rust
pub enum FlywheelError {
    DDriveUnavailable { path: PathBuf, hint: String },
    DDriveNotWritable { source: io::Error },
    DDriveSubdirCreateFailed { path: PathBuf, source: io::Error },
    SchedulerFlapping { task_name: String, restart_count: u32 },
    SubscriberLagAlert { unprocessed_shifts: u32, window_secs: u32 },
    HookDiskFull,
    OracleBindingFailed { test_id: TestId, reason: String },
    FeedbackPredictionNotFound { prediction_id: PredictionId },
    FeedbackInvalidOutcomeShape { prediction_id: PredictionId, kind: String },
    FeedbackRateLimited { agent_id: AgentId },
}
```

All map to `MEJEPA_*` error codes returned via the MCP error response format.

---

## FSV strategy

| FSV | What it proves | Output path |
|---|---|---|
| `daemon_startup_fsv` | --daemon flag works; D-drive validation fires | `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/startup.json` |
| `supervisor_panic_recovery_fsv` | scheduler task panic → restart with backoff | `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/panic_recovery.json` |
| `daemon_supervision_fsv` | real daemon tasks run scheduled GC/eval, write/readback job-run ledger, shutdown cleanly, reopen RocksDB, and verify edge failures | `/mnt/d/contextgraph/fsv/mcp-daemon-supervision-fsv/daemon-supervision-evidence.json` |
| `oracle_binding_fsv` | TestOutcomeEvent → prediction binding → observe() called | `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/oracle_binding.json` |
| `agent_feedback_fsv` | mejepa_record_agent_feedback → SurpriseEvent persisted → sampling weight bumped | `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/agent_feedback.json` |
| `subscriber_lag_alert_fsv` | inject 1000 shifts in 30s → lag alert fires | `/mnt/d/contextgraph/fsv/mcp-daemon-fsv/lag_alert.json` |
| `cf_round_trip_fsv` | DdaSignals + ExemplarRef + SurpriseEvent serialize/deserialize losslessly | `/mnt/d/contextgraph/fsv/mejepa-cf-fsv/round_trip.json` |
