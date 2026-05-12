# TECH-TESTING — Technical Specification

> **Level 3.** Implements [`SPEC-TESTING.md`](../functional/SPEC-TESTING.md). Implementation reference: [`18_INTEGRATION_AND_E2E_TESTING.md`](../../18_INTEGRATION_AND_E2E_TESTING.md).

## Module layout

```
tests/
  e2e/
    happy_path.rs                 # REQ-TEST-01
    restart_safety.rs             # REQ-TEST-02
    multi_agent.rs                # REQ-TEST-03
    catastrophic_event.rs         # REQ-TEST-04
    storage_exhaustion.rs         # REQ-TEST-05
  chaos/
    failpoints_supervised.rs      # REQ-TEST-06
  load/
    concurrent_agents.rs          # REQ-TEST-07
  soak/
    seven_day.rs                  # REQ-TEST-08
  toolchain_fail_closed/
    matrix.rs                     # REQ-TEST-09
  fixtures/
    adversarial/                  # REQ-TEST-10 corpus (30 cases)
.github/workflows/
  e2e.yml, chaos.yml, load.yml, soak.yml  # REQ-TEST-11
```

## Test harness

```rust
// tests/e2e/harness.rs
pub struct E2eHarness {
    pub d_root: TempDir,
    pub daemon: Child,                // spawned daemon process
    pub mcp_socket: PathBuf,
    pub fixtures: PathBuf,
}

impl E2eHarness {
    pub async fn boot() -> Result<Self> { /* ... */ }
    pub async fn make_edit(&self, file: &Path, old: &str, new: &str) -> Result<ShiftId> { /* ... */ }
    pub async fn run_test(&self, fixture: &str) -> Result<TestOutcomeEvent> { /* ... */ }
    pub async fn record_feedback(&self, prediction_id: PredictionId, kind: FeedbackKind, severity: SurpriseSeverity) -> Result<FeedbackId> { /* ... */ }
    pub async fn assert_prediction_within(&self, session_id: SessionId, deadline: Duration) -> Result<RealityPrediction> { /* ... */ }
    pub async fn shutdown(self) -> Result<()> { /* ... */ }
}
```

## failpoints integration (chaos)

```rust
// in supervised task code
fn drain_pending_shifts(&self) -> Result<()> {
    fail_point!("subscriber.consume", |_| {
        panic!("synthetic panic for chaos testing")
    });
    /* normal path */
}

// in chaos test
#[test]
fn subscriber_panic_recovers() {
    let _ = fail::cfg("subscriber.consume", "panic").unwrap();
    let harness = E2eHarness::boot().expect("boot");
    harness.make_edit(...).await.expect("edit");
    // wait > backoff window
    harness.assert_prediction_within(session, Duration::from_secs(5)).await.expect("recovers");
    fail::cfg("subscriber.consume", "off").unwrap();
}
```

## Load harness

Use `goose` (Rust load-testing crate) or hand-rolled tokio:

```rust
async fn load_concurrent_agents(n_agents: usize, edits_per_second: f32, duration: Duration) -> LoadReport {
    let mut handles = vec![];
    for agent_id in 0..n_agents {
        let h = tokio::spawn(simulate_agent(agent_id, edits_per_second, duration));
        handles.push(h);
    }
    let results = futures::future::join_all(handles).await;
    LoadReport::aggregate(results)
}
```

## Soak harness

Long-running script that spawns the daemon + a synthetic agent emitter; checks invariants periodically.

```bash
# scripts/soak/run_seven_day_soak.sh
systemctl --user start mejepa-daemon
nohup ./scripts/soak/synthetic_agent.sh &
nohup ./scripts/soak/inject_drift_day_4.sh &
nohup ./scripts/soak/inject_catastrophic_day_5.sh &
# ... wait 7 days ...
./scripts/soak/assert_day_7.sh
```

## CI workflow snippets

```yaml
# .github/workflows/e2e.yml
name: e2e
on: [pull_request]
jobs:
  e2e:
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release --workspace
      - run: cargo test --features e2e --test "*" -- --test-threads 1
      - run: tar cf /tmp/e2e-artifacts.tar /tmp/e2e-d-root
      - uses: actions/upload-artifact@v4
        with: { name: e2e-artifacts, path: /tmp/e2e-artifacts.tar }
```

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `e2e_happy_path_fsv` | full Edit → predict → feedback → observe loop completes | `/mnt/d/contextgraph/fsv/e2e-fsv/happy_path.json` |
| `e2e_restart_safety_fsv` | watermark resume after SIGKILL | `/mnt/d/contextgraph/fsv/e2e-fsv/restart_safety.json` |
| `chaos_subscriber_panic_fsv` | injected panic → recovery | `/mnt/d/contextgraph/fsv/chaos-fsv/subscriber_panic.json` |
| `load_10_agents_fsv` | 10-agent x 10-min run; latency p99 within budget | `/mnt/d/contextgraph/fsv/load-fsv/10_agents.json` |
| `soak_day_7_assertions_fsv` | post-soak invariants hold | `/mnt/d/contextgraph/fsv/soak-fsv/day_7.json` |
| `adversarial_corpus_fsv` | ≥80% non-Pass; 0 false-confident-Pass | `/mnt/d/contextgraph/fsv/test-fsv/adversarial.json` |
