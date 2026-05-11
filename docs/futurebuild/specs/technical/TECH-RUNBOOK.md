# TECH-RUNBOOK — Technical Specification

> **Level 3.** Implements [`SPEC-RUNBOOK.md`](../functional/SPEC-RUNBOOK.md). Implementation reference: [`19_OPERATOR_RUNBOOK.md`](../../19_OPERATOR_RUNBOOK.md).

## Module layout

```
crates/context-graph-cli/
  src/
    commands/
      mejepa_daemon.rs              # status, start, stop, restart
      mejepa_active_learning.rs     # list, label, dismiss
      mejepa_hygiene.rs             # cold-audit, aggressive-cold-tier-down
      mejepa_eval.rs                # run, ship-gate
      mejepa_rollback.rs
      mejepa_pause.rs
      mejepa_report.rs              # weekly --date <date>
      mejepa_session.rs             # cleanup --session-id
      mejepa_replay.rs              # cross-version reproduction
      mejepa_models.rs              # verify-and-fetch
      mejepa_storage.rs             # verify, restore-cf, migrate-cf
scripts/setup/
  first_time_setup.sh               # REQ-RUN-01
INSTALL.md                          # quickstart (REQ-RUN-11)
```

## CLI structure

```
context-graph-cli mejepa <subcommand> [options]

Subcommands:
  daemon {start|stop|restart|status}
  active-learning {list|label|dismiss}
  hygiene {cold-audit|aggressive-cold-tier-down}
  eval {run|ship-gate}
  rollback --version <id>
  pause --duration-mins <n>
  report weekly --date <date>
  session cleanup --session-id <id>
  replay --prediction-id <id>
  models verify-and-fetch [--digest-list <path>]
  storage {verify|restore-cf|migrate-cf} [args]
  bootstrap [--baseline <path> | --no-baseline | --recover]
```

Each subcommand wraps an MCP tool call OR a direct file operation, with structured stdout output for both human and machine consumption.

## first_time_setup.sh

```bash
#!/usr/bin/env bash
# scripts/setup/first_time_setup.sh
set -euo pipefail

D_ROOT="${1:-/mnt/d/contextgraph}"
MODELS_ROOT="${2:-/mnt/d/cabdru/contextgraph/models}"

echo "ContextGraph ME-JEPA First-Time Setup"
echo "  D-drive root: $D_ROOT"
echo "  Models root: $MODELS_ROOT"

# 1. Create directory structure
mkdir -p "$D_ROOT"/{storage/{contextgraph-rocksdb,replay,cold,witness,train-certs,live-predictions,embedder-cache},runtime/{cgreality-shift-log,test-outcomes},logs,state/{locks,cgreality,gold-labels},fsv,exports/{active-learning,eval,pairwise-mi,load-tests,test-runs}}
mkdir -p "$MODELS_ROOT"

# 2. Verify model artifacts
echo "Checking model digest list..."
context-graph-cli mejepa models verify-and-fetch --digest-list config/model_digests.json --models-root "$MODELS_ROOT"

# 3. Build
echo "Building..."
cargo build --release --workspace

# 4. Install systemd unit
mkdir -p ~/.config/systemd/user
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
sed -i "s|/mnt/d/contextgraph|$D_ROOT|g" ~/.config/systemd/user/mejepa-daemon.service
systemctl --user daemon-reload

# 5. Start
systemctl --user enable --now mejepa-daemon

# 6. Smoke test
sleep 5
echo "Smoke test: daemon status..."
context-graph-cli mejepa daemon status

echo "Setup complete. Read docs/futurebuild/19_OPERATOR_RUNBOOK.md for daily operations."
```

## Configuration files

```toml
# ~/.config/mejepa/daemon.toml
[paths]
d_root = "/mnt/d/contextgraph"
models_root = "/mnt/d/cabdru/contextgraph/models"
storage_root = "/mnt/d/contextgraph/storage/contextgraph-rocksdb"

[scheduler]
observe_period_ms = 250
drift_period_secs = 60
active_learning_period_secs = 600
promote_period_secs = 3600
continual_backprop_period_secs = 1800
constellation_period_secs = 3600
emergency_eviction_period_secs = 600

[drift]
soft_threshold = 0.88
hard_threshold = 0.85
catastrophic_threshold = 0.80
window_size = 1000

[storage]
hard_quota_gb = 115.0
emergency_evict_threshold_gb = 110.0

[security]
require_two_approvers_for_catastrophic_promotion = false
secret_redaction_enabled = true
```

```toml
# ~/.config/mejepa/backup.toml
[local]
repo = "/mnt/d/contextgraph-backups/local"
schedule = "hourly"
retention = { hourly = 24, daily = 7, weekly = 4 }

[offmachine]
repo = "/mnt/external/mejepa-backups"
schedule = "daily"
schedule_time = "04:00"
retention = { daily = 14, weekly = 8, monthly = 6 }
```

## Pause-predictions implementation

```rust
// crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs (extension)
pub async fn handle_pause_predictions(storage: &Storage, args: PauseArgs) -> Result<PauseResponse> {
    let until = SystemTime::now() + Duration::from_secs(args.duration_mins * 60);
    storage.put_cf(CF_MEJEPA_DAEMON_STATE, b"prediction_pause_until", &serialize_systime(until))?;
    Ok(PauseResponse { paused_until: until })
}
```

The predictor checks this flag at the top of `verify()`; when paused, returns `Verdict::Abstain` with `confidence_interval = [0.0, 1.0]` and a `paused_until` field.

## INSTALL.md (quickstart)

```markdown
# ContextGraph ME-JEPA — Install

## Prerequisites
- WSL2 with D: drive mounted at /mnt/d
- Rust 1.84+
- CUDA 13.2 toolkit (for embedder forwards)
- 10+ GB free on /mnt/d/

## One-line install
```bash
git clone https://github.com/.../contextgraph.git ~/contextgraph
cd ~/contextgraph
bash scripts/setup/first_time_setup.sh
```

After this, the daemon is running. Read `docs/futurebuild/19_OPERATOR_RUNBOOK.md`.
```

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `cli_daemon_status_fsv` | command works against running daemon | `/mnt/d/contextgraph/fsv/cli-fsv/daemon_status.json` |
| `cli_active_learning_fsv` | list/label/dismiss round-trip | `/mnt/d/contextgraph/fsv/cli-fsv/active_learning.json` |
| `first_time_setup_fsv` | fresh-clone install completes ≤ 30 min | manual: timer + final daemon-status check |
