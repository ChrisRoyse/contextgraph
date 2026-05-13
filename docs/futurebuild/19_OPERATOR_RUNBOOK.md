# 19 — Operator Runbook

> **Purpose.** Day-to-day, week-to-week, and incident-response operations for ME-JEPA-as-truth-system. The single source of truth for "what does the operator do?"
>
> Owned by [`SPEC-RUNBOOK.md`](specs/functional/SPEC-RUNBOOK.md) + [`TECH-RUNBOOK.md`](specs/technical/TECH-RUNBOOK.md).

---

## 1. First-time setup (one-time)

```bash
# 1. Mount the D drive (WSL convention)
mkdir -p /mnt/d
# (Windows host should already mount D: at /mnt/d when WSL starts; verify with `mount | grep /mnt/d`)

# 2. Create the ContextGraph root
mkdir -p /mnt/d/contextgraph/{storage/{contextgraph-rocksdb,replay,cold,witness,train-certs,live-predictions,embedder-cache},runtime/{cgreality-shift-log,test-outcomes},logs,state/{locks,cgreality,gold-labels},fsv,exports/{active-learning,eval,pairwise-mi,load-tests,test-runs}}
mkdir -p /mnt/d/cabdru/contextgraph/models

# 3. Place SHA-pinned model artifacts under /mnt/d/cabdru/contextgraph/models/
#    (Operator-provided per the per-embedder digest list; not auto-fetched)

# 4. Build
cd /home/cabdru/contextgraph
cargo build --release --workspace

# 5. Install systemd unit
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mejepa-daemon

# 6. Verify
systemctl --user status mejepa-daemon          # should show "active (running)"
ls -la /mnt/d/contextgraph/state/locks/         # should see context-graph-daemon.pid
tail /mnt/d/contextgraph/logs/mejepa-daemon.stdout.log

# 7. MCP smoke test (from any directory)
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"mcp__cgreality__mejepa_quota_status","arguments":{}}}' \
    | nc -U /mnt/d/contextgraph/state/locks/mcp.sock
# expect: { "result": { "used_gb": ..., "hard_quota_gb": 115.0, ... } }
```

If any step fails, see `§7 Troubleshooting`.

---

## 2. Daily operations (2-5 minutes)

```bash
# Once per day, e.g., as part of morning routine

# 2.1 Daemon health
mcp_call mejepa_daemon_status
# Look for:
# - subscriber.lag_ms < 5000
# - heal.last_drift_class != Catastrophic
# - quota.used_gb < 110
# - vram.peak_gb < budget_gb

# 2.2 Glance at any operator alerts
ls /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/ 2>/dev/null
# If a weekly report exists, glance at the top "Catastrophic events" section

# 2.3 Active-learning queue check (do once per session, not per day)
context-graph-cli mejepa active-learning list --top-n 5
# If anything is gold-standard-ready, label it
```

If anything is degraded, see §6 Incident Response. Otherwise, you're done.

---

## 3. Weekly operations (15-30 minutes)

```bash
# Mondays at start of week

# 3.1 Read the weekly evaluation report
less /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/weekly.md

# Look for:
# - Operator Cell Triage top-10 table; take the suggested action per row
# - Per-(category × language) ship-gate status (≥0.95 met / not met per cell)
# - Drift events of the week
# - Promotions / rollbacks
# - Agent-feedback summary (top surprises)
# - Storage trend

# 3.2 Address active-learning queue
context-graph-cli mejepa active-learning list --top-n 20
# For each entry, decide:
# - Label as gold-standard (operator-override): context-graph-cli mejepa active-learning label <id> <label> "<reason>"
# - Dismiss (not actionable): context-graph-cli mejepa active-learning dismiss <id> "<reason>"

# 3.3 Pairwise MI heatmap inspection (every other week)
xdg-open /mnt/d/contextgraph/exports/pairwise-mi/$(ls /mnt/d/contextgraph/exports/pairwise-mi/ | tail -1)
# If any embedder pair has MI > 0.85 sustained over 2 audits, decide:
# - Retire one slot (mejepa_routing_lookup --retire <embedder_id>)
# - Recalibrate (mejepa_audit_pairwise_mi --force-recalibrate)
# - Accept (no action)

# 3.4 Disk usage check
du -sh /mnt/d/contextgraph/
mcp_call mejepa_quota_status   # expect breakdown by slice
```

---

## 4. Monthly operations (30-60 minutes)

```bash
# 4.1 Backup verification (per 20_DISASTER_RECOVERY_AND_BACKUP.md)
ls -la /mnt/d/contextgraph-backups/   # ensure most recent < 7 days old

# 4.2 Storage cold-tier audit
context-graph-cli mejepa hygiene cold-audit
# Confirm cold tier compression ratios are healthy

# 4.3 Witness chain integrity check (forced)
mcp_call optimizer_witness_chain_verify --depth full
# Should return "valid" with chain_length and last_verified_ts

# 4.4 Soak-test trigger (quarterly per 18_INTEGRATION_AND_E2E_TESTING.md §8)
# only quarterly; skip if not due

# 4.5 Read latest doc updates
git log --since="1 month ago" --oneline -- docs/futurebuild/ docs/ruvectorfindings/
```

---

## 5. Incident response — alert classes

### 5.1 `MEJEPA_SCHEDULER_FLAPPING` — supervised task restarting > 5 times in 5 min

**Cause:** persistent error in a tokio task (panic, OOM, dependency crash).

**Investigate:**
```bash
journalctl --user -u mejepa-daemon -n 200
tail -100 /mnt/d/contextgraph/logs/mejepa-daemon.stderr.log
```

**Resolution:**
```bash
systemctl --user stop mejepa-daemon
systemctl --user restart mejepa-daemon
```

1. Identify which supervised task is flapping (log lines name it).
2. Check if it's a known issue (CF corruption → see 5.2; OOM → see 5.5; etc.).
3. If unknown: `systemctl --user stop mejepa-daemon`; investigate; create blocker in `memory/blockers/`; restart.

### 5.2 `MEJEPA_INFER_WITNESS_CHAIN_BROKEN`

**Cause:** hash chain integrity check failed.

**Investigate:**
```bash
mcp_call optimizer_witness_chain_diff --since-last-verified
journalctl --user -u mejepa-daemon -n 200
```

**Resolution:**
```bash
mcp_call optimizer_witness_chain_repair_legacy --force
context-graph-cli mejepa storage-verify --db-path /mnt/d/contextgraph/state/cgreality/rocksdb
```

1. Auto-repair has already attempted. Check: `mcp_call optimizer_witness_chain_diff --since-last-verified`
2. If auto-repair succeeded: log + carry on.
3. If auto-repair failed: `mcp_call optimizer_witness_chain_repair_legacy --force` and check.
4. Last resort: restore the last known-good RocksDB checkpoint per `20_DISASTER_RECOVERY_AND_BACKUP.md §4`.

### 5.3 `MEJEPA_OBSERVE_DRIFT_DETECTED`

**Cause:** rolling accuracy < 80%.

**Investigate:**
```bash
mcp_call mejepa_heal_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
mcp_call mejepa_promote_approval --promotion-id <id> --approve true
mcp_call mejepa_promote_approval --promotion-id <id> --approve false --reason "rejecting suspicious drift candidate"
```

1. Check `mejepa_heal_status` for the queued full retrain candidate.
2. Read `weekly.md` for context (data shift? Operator-introduced label change?).
3. If retrain looks reasonable: `mcp_call mejepa_promote_approval --promotion-id <id> --approve true`
4. If suspicious: `--approve false --reason "..."`; investigate.

### 5.4 `MEJEPA_HYGIENE_QUOTA_EXCEEDED`

**Cause:** disk usage > 115 GB hard cap.

**Investigate:**
```bash
mcp_call mejepa_quota_status
du -sh /mnt/d/contextgraph/runtime /mnt/d/contextgraph/state /mnt/d/contextgraph/fsv
```

**Resolution:**
```bash
mcp_call mejepa_gc_run --force-eviction true
context-graph-cli mejepa hygiene aggressive-cold-tier-down
```

1. Check `mcp_call mejepa_quota_status` for slice breakdown.
2. Run emergency eviction: `mcp_call mejepa_gc_run --force-eviction true`.
3. If eviction insufficient: `context-graph-cli mejepa hygiene aggressive-cold-tier-down`.
4. If still over: investigate why a specific slice is growing (e.g., shift-log retention misconfigured).

### 5.5 OOM / RSS climbing

**Cause:** memory leak in supervised task.

**Investigate:**
```bash
journalctl --user -u mejepa-daemon -n 200
ps -o pid,rss,comm -C context-graph-mcp
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
context-graph-cli mejepa daemon-status
```

1. `systemctl --user restart mejepa-daemon`
2. Check `valgrind`-style profiling (collect data during repro)
3. Create blocker in `memory/blockers/` with severity and reproducible scenario.

### 5.6 `MEJEPA_D_DRIVE_UNAVAILABLE`

**Cause:** D-drive unmounted or corrupted.

**Investigate:**
```bash
mount | grep /mnt/d
ls -ld /mnt/d/contextgraph
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
context-graph-cli mejepa storage-verify --db-path /mnt/d/contextgraph/state/cgreality/rocksdb
```

1. Check mount: `mount | grep /mnt/d`
2. If unmounted: remount; restart daemon.
3. If corrupted: see `20_DISASTER_RECOVERY_AND_BACKUP.md §3`.

### 5.7 `MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT`

**Cause:** the shift subscriber has sustained pending shift-log lines above its configured threshold, so predictions may be stale.

**Investigate:**
```bash
mcp_call mejepa_subscriber_status
find /mnt/d/contextgraph/runtime/cgreality-shift-log -maxdepth 1 -type f -name "*.jsonl" -printf "%p %s bytes\n"
```

**Resolution:**
```bash
systemctl --user restart mejepa-daemon
mcp_call mejepa_subscriber_status
```

1. Check `mcp_call mejepa_subscriber_status` and confirm `lag_alert_active=true`, pending shift count, and latest watermark.
2. Inspect `/mnt/d/contextgraph/runtime/cgreality-shift-log/` for oversized or malformed JSONL files.
3. If the daemon is alive but not consuming, restart it once: `systemctl --user restart mejepa-daemon`.
4. If lag returns, file a high-severity blocker with the offending session id, shift-log path, and subscriber status payload.

### 5.8 Predictions silently wrong (no alert, but operator notices)

**Cause:** drift below detection threshold; OR data distribution shift the detector hasn't caught; OR head's λ ramped too high prematurely.

**Investigate:**
```bash
mcp_call mejepa_ship_gate_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
mcp_call mejepa_constellation_inspect --force-refit --category <category> --language <language>
context-graph-cli mejepa active-learning list
```

1. Pull `mejepa_ship_gate_status` → check per-cell status.
2. If a specific (category, language) cell is regressing, force constellation refresh: `mcp_call mejepa_constellation_inspect --force-refit --category <c> --language <l>`.
3. If pervasive: file blocker; consider triggering full retrain.

### 5.9 `MEJEPA_TRAINING_HOLDOUT_DRIFT`

**Cause:** the training-batch `(mutation_category, language)` distribution has diverged from the holdout distribution with `KL > 0.5` for at least 100 batches.

**Investigate:**
```bash
mcp_call mejepa_heal_status
context-graph-cli mejepa report-weekly --date "$(date -u +%F)"
```

**Resolution:**
```bash
context-graph-cli mejepa cell-status --threshold 0.95
context-graph-cli mejepa active-learning list
```

1. Read the latest `telemetry_feedback` action and confirm `metric_name=training_holdout_kl_divergence`.
2. Compare weak cells in `cell-status` against recent active-learning and operator-override entries.
3. Reduce the skew by draining high-priority labels or broadening corpus sampling for underrepresented cells before promoting a new model.

---

## 6. Configuration tuning

| Knob | Default | Where set | When to change |
|---|---|---|---|
| Drift soft/hard/catastrophic thresholds | 88/85/80% | `daemon config` | when production data calibrates |
| Subscriber tick period | 250 ms | `daemon config` | rarely; tighten if lag > 5s p50 |
| Promote tick period | 3600 s | `daemon config` | rarely |
| Storage hard quota | 115 GB | `daemon config` | when hardware changes |
| GC schedule | nightly 03:00 local | systemd timer | per operator preference |
| Active-learning surface threshold | top-N=10 / period=600s | `daemon config` | per operator capacity |
| Per-head loss coefficients λ_h | per `10 §8` defaults | trained-in | only via supersession |

Configuration lives in `~/.config/mejepa/daemon.toml` (or daemon args). Edits require daemon restart.

---

## 7. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Daemon won't start | D-drive missing | mount D-drive; verify with `ls /mnt/d/contextgraph/` |
| Daemon won't start | model file missing | check digest report; place model under `/mnt/d/cabdru/contextgraph/models/` |
| Daemon won't start | pid lock held | another instance running; `pkill -f context-graph-mcp.*--daemon` |
| Predictions never appear | subscriber lagged | `mcp_call mejepa_subscriber_status`; check shift-log dir; check tokio runtime |
| Predictions appear but agent doesn't see them | injection hook misconfigured | check `.claude/settings.json hooks.UserPromptSubmit`; test the script manually |
| `mejepa_record_agent_feedback` fails | invalid payload, missing prediction, or storage validation error | inspect the returned `MEJEPA_FEEDBACK_*` code and source-of-truth readback details; fix the payload or storage root |
| Sampling weight not bumping after surprise | scheduler not consuming queue | check active-learning tick is firing; check `CF_MEJEPA_ACTIVE_LEARNING_QUEUE` |
| Disk filling fast | shift-log retention broken | check GC logs; manually run `mcp_call mejepa_gc_run` |
| Wrong predictions on Python tasks but Rust looks fine | per-language drift | `mejepa_ship_gate_status --by-language python`; consider per-language LoRA refresh |
| `nvidia-smi` reports N/A | WSL passthrough issue | per `TECH-OBSERVABILITY §5`: flagged not silently swapped |

---

## 8. Reference: every operator MCP tool

| Tool | When | Notes |
|---|---|---|
| `mejepa_daemon_status` | health check (daily) | shipped in Phase F |
| `mejepa_subscriber_status` | check subscriber | shipped |
| `mejepa_heal_status` | check drift / promotion state | shipped |
| `mejepa_quota_status` | disk usage | shipped + Phase A extension |
| `mejepa_vram_budget_report` | GPU usage | shipped |
| `mejepa_eval_run` | force a holdout-eval run | shipped |
| `mejepa_ship_gate_status` | per-(cat × lang) ship-gate | shipped |
| `mejepa_active_learning_queue` | queue contents | shipped |
| `mejepa_eval_build_graph` | rebuild patch-similarity graph | shipped |
| `mejepa_audit_pairwise_mi` | weekly MI audit | shipped |
| `mejepa_constellation_inspect` | TCT cell inspection / forced refit | shipped |
| `mejepa_gc_run` | force GC / eviction | shipped + Phase F extension |
| `mejepa_witness_compress` | force Merkle compression | shipped |
| `mejepa_promote_approval` | approve catastrophic promotions | NEW Phase E |
| `mejepa_rollback_to` | force rollback to prior model | shipped |
| `optimizer_witness_chain_verify` | integrity check | shipped |
| `optimizer_witness_chain_diff` | drift in past recommendations | shipped |
| `optimizer_witness_chain_repair_legacy` | force repair | shipped |
| `optimizer_recall_recommendations` | recent recommendations | shipped |

---

## 9. Reference: every operator CLI command

```bash
context-graph-cli mejepa active-learning list [--top-n N]
context-graph-cli mejepa active-learning label <id> <label> <reason>
context-graph-cli mejepa active-learning dismiss <id> <reason>
context-graph-cli mejepa hygiene cold-audit
context-graph-cli mejepa hygiene aggressive-cold-tier-down
context-graph-cli mejepa rollback --version <version>
context-graph-cli mejepa eval run --force
context-graph-cli mejepa eval ship-gate
context-graph-cli mejepa daemon status
context-graph-cli mejepa daemon restart
context-graph-cli mejepa daemon stop
```

These wrap the MCP tools for easier shell-side use.

---

## 10. Escalation path

If anything in §5 doesn't resolve within 1 day:

1. Write a high-severity blocker in `memory/blockers/<date>--<short-name>.md` with severity, reproducible scenario, what's been tried.
2. If user-facing degradation: pause prediction emission via `mcp_call mejepa_pause_predictions --duration-mins 60`; the agent receives `Verdict::Abstain` until resumed.
3. Investigate root cause; fix; resume.

For systemic issues: file an issue at the project repo with all artifacts (daemon logs, training certs, the failing prediction's witness hash).
