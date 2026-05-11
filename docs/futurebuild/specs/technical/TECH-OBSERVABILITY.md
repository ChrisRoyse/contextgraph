# TECH-OBSERVABILITY — Technical Specification

> **Level 3.** Defines HOW the production observability surface is built. Implements [`SPEC-OBSERVABILITY.md`](../functional/SPEC-OBSERVABILITY.md).

---

## Module layout

```
crates/context-graph-mejepa/
  src/
    eval/
      report.rs                         # existing; extended for weekly cron writer
      weekly_cron.rs                    # NEW: scheduled cron writer (REQ-OBS-FB-01)
      mi_heatmap_renderer.rs            # NEW: pairwise MI → markdown table + PNG (REQ-OBS-FB-02)
      per_head_calibration_tracker.rs   # NEW: rolling-window ECE per head (REQ-OBS-FB-04)
      slice_quota_breakdown.rs          # NEW: disk usage by slice (REQ-OBS-FB-08)

crates/context-graph-mcp/
  src/
    handlers/tools/
      mejepa_tools.rs                   # extended: mejepa_daemon_status, mejepa_inspect_prediction
  src/
    telemetry/
      gpu_wsl.rs                        # NEW: hardened nvidia-smi wrapper (REQ-OBS-FB-05)

crates/context-graph-cli/
  src/
    commands/
      mejepa_active_learning.rs         # NEW: operator CLI (REQ-OBS-FB-07)

ci/lints/
  no_c_drive_paths.sh                   # NEW (REQ-OBS-FB-09)
```

---

## Weekly evaluation cron

```rust
// crates/context-graph-mejepa/src/eval/weekly_cron.rs (NEW)
pub async fn run_weekly_eval_scheduler(
    storage: Storage,
    config: WeeklyEvalConfig,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    loop {
        let now = chrono::Utc::now();
        let next_run = next_monday_06_utc(now);
        let sleep_dur = (next_run - now).to_std().unwrap_or(Duration::from_secs(60));
        
        tokio::select! {
            _ = tokio::time::sleep(sleep_dur) => {
                if let Err(e) = run_one_weekly_report(&storage, &config).await {
                    tracing::error!(?e, "weekly eval failed");
                }
            }
            res = shutdown.changed() => {
                res?;
                if *shutdown.borrow() { break; }
            }
        }
    }
    Ok(())
}

async fn run_one_weekly_report(storage: &Storage, config: &WeeklyEvalConfig) -> Result<()> {
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let out_dir = config.exports_root.join("eval").join(date.to_string());
    std::fs::create_dir_all(&out_dir)?;
    
    let eval = mejepa_eval::EvalRunner::new(storage.clone()).run_holdout_eval().await?;
    let report = build_weekly_report(&eval, storage)?;
    
    std::fs::write(out_dir.join("weekly.md"), report.markdown)?;
    std::fs::write(out_dir.join("weekly.json"), serde_json::to_string_pretty(&report.json)?)?;
    
    Ok(())
}
```

---

## WSL GPU telemetry hardening

```rust
// crates/context-graph-mcp/src/telemetry/gpu_wsl.rs (NEW)
pub async fn nvidia_smi_query(query: &str) -> Result<GpuTelemetry> {
    for attempt in 0..3 {
        match nvidia_smi_once(query).await {
            Ok(telemetry) => return Ok(telemetry),
            Err(GpuError::Timeout) if attempt < 2 => {
                tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

async fn nvidia_smi_once(query: &str) -> Result<GpuTelemetry> {
    let out = tokio::process::Command::new("nvidia-smi")
        .args(["--query-gpu", query, "--format=csv,noheader,nounits"])
        .output()
        .await
        .map_err(|_| GpuError::Timeout)?;
    
    if !out.status.success() {
        return Err(GpuError::CommandFailed { stderr: String::from_utf8_lossy(&out.stderr).into_owned() });
    }
    
    parse_nvidia_smi_output(&out.stdout, query)
}

fn parse_nvidia_smi_output(stdout: &[u8], query: &str) -> Result<GpuTelemetry> {
    let s = String::from_utf8_lossy(stdout);
    let mut tel = GpuTelemetry::default();
    for line in s.lines() {
        for (col, raw) in query.split(',').zip(line.split(',')) {
            let raw = raw.trim();
            if raw == "[N/A]" || raw == "N/A" {
                tel.unavailable_fields.push(col.trim().to_string());
            } else {
                tel.set_field(col.trim(), raw)?;
            }
        }
    }
    Ok(tel)
}
```

Per `memory/decisions/agent-42-harness-editor--nvidia-smi-na-sentinel-tolerance.md`: N/A is not silently swapped for -1; it's reported as `unavailable: true` for that field.

---

## Per-prediction-class calibration tracker

```rust
// crates/context-graph-mejepa/src/eval/per_head_calibration_tracker.rs (NEW)
pub struct PerHeadCalibrationTracker {
    per_head_window: HashMap<HeadId, RollingWindow<(f32, bool)>>,
}

impl PerHeadCalibrationTracker {
    pub fn record(&mut self, head: HeadId, predicted_confidence: f32, was_correct: bool) {
        self.per_head_window.entry(head).or_insert_with(|| RollingWindow::new(1000)).push((predicted_confidence, was_correct));
    }
    
    pub fn ece(&self, head: HeadId) -> f32 {
        let window = match self.per_head_window.get(&head) {
            Some(w) => w,
            None => return 0.0,
        };
        // Standard ECE: bin by confidence, compute |bin_avg_confidence - bin_accuracy|, weighted sum
        compute_ece(window.iter())
    }
}
```

Surfaced via extension to `mejepa_ship_gate_status` response.

---

## Per-slice quota breakdown

```rust
// crates/context-graph-mejepa/src/eval/slice_quota_breakdown.rs (NEW)
pub fn slice_quota(storage_root: &Path) -> Result<SliceQuotaReport> {
    let slices = vec![
        ("rocksdb", "storage/contextgraph-rocksdb"),
        ("shift_log", "runtime/cgreality-shift-log"),
        ("cold", "storage/cold"),
        ("replay", "storage/replay"),
        ("witness", "storage/witness"),
        ("logs", "logs"),
        ("fsv", "fsv"),
        ("exports", "exports"),
        ("state", "state"),
        ("train_certs", "storage/train-certs"),
        ("live_predictions", "storage/live-predictions"),
        ("embedder_cache", "storage/embedder-cache"),
    ];
    
    let mut by_slice = Vec::new();
    let mut total = 0;
    for (name, path) in slices {
        let p = storage_root.join(path);
        let bytes = du_recursive(&p)?;
        by_slice.push((name.to_string(), bytes));
        total += bytes;
    }
    
    Ok(SliceQuotaReport {
        total_gb: total as f64 / 1e9,
        by_slice,
        hard_quota_gb: 115.0,
    })
}
```

Returned by extension to `mejepa_quota_status`.

---

## CI lint script

See `TECH-LOOP.md §6` (the same lint covers both spec scopes).

---

## Active-learning operator CLI

```rust
// crates/context-graph-cli/src/commands/mejepa_active_learning.rs (NEW)
#[derive(clap::Parser)]
pub struct ActiveLearningCmd {
    #[command(subcommand)]
    pub action: ActiveLearningAction,
}

#[derive(clap::Subcommand)]
pub enum ActiveLearningAction {
    List { #[arg(long)] top_n: Option<usize> },
    Label { id: String, label: String, reason: String },
    Dismiss { id: String, reason: String },
}

impl ActiveLearningCmd {
    pub async fn run(&self, storage: Storage) -> Result<()> {
        let queue = mejepa_eval::queue::ActiveLearningQueueState::open(&storage)?;
        match &self.action {
            ActiveLearningAction::List { top_n } => {
                for entry in queue.high_priority(top_n.unwrap_or(20)) {
                    println!("{}", render_entry(&entry));
                }
            }
            ActiveLearningAction::Label { id, label, reason } => {
                let entry = queue.lookup(id)?;
                queue.set_operator_label(entry, label, reason).await?;
                println!("labeled {}", id);
            }
            ActiveLearningAction::Dismiss { id, reason } => {
                queue.dismiss(id, reason).await?;
                println!("dismissed {}", id);
            }
        }
        Ok(())
    }
}
```

---

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `weekly_report_fsv` | weekly.md contains all required sections + writes to D-drive path | `/mnt/d/contextgraph/fsv/eval-fsv/weekly.json` |
| `mi_heatmap_render_fsv` | renders to markdown + PNG at expected paths | `/mnt/d/contextgraph/fsv/eval-fsv/heatmap.json` |
| `daemon_status_aggregator_fsv` | mejepa_daemon_status returns all sub-statuses < 100ms | `/mnt/d/contextgraph/fsv/mcp-fsv/daemon_status.json` |
| `per_head_calibration_fsv` | rolling ECE updates per recorded sample | `/mnt/d/contextgraph/fsv/eval-fsv/calibration.json` |
| `gpu_wsl_telemetry_fsv` | N/A field flagged as unavailable, not -1 | `/mnt/d/contextgraph/fsv/mcp-fsv/gpu_wsl.json` |
| `inspect_prediction_fsv` | full provenance trace returned | `/mnt/d/contextgraph/fsv/mejepa-fsv/inspect.json` |
| `slice_quota_fsv` | breakdown sums to total; total matches du -sh | `/mnt/d/contextgraph/fsv/eval-fsv/slice_quota.json` |
| `c_drive_lint_fsv` | catches deliberately-introduced /tmp/contextgraph-test/ path | CI step output |
| `cli_active_learning_fsv` | list / label / dismiss work end-to-end | `/mnt/d/contextgraph/fsv/cli-fsv/active_learning.json` |
