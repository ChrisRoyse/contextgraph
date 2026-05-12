# 22 — Operator Runbook

Source files covered: `docs/futurebuild/19_OPERATOR_RUNBOOK.md`, `scripts/systemd/mejepa-daemon.service`, `scripts/ensure_daemon.sh`.

> Day-to-day, week-to-week, incident-response operations for ME-JEPA-as-truth-system.

---

## 1. First-time setup (one-time)

```bash
# 1. Mount D drive
mkdir -p /mnt/d   # Windows host should auto-mount D: at /mnt/d under WSL

# 2. Create ContextGraph root tree
mkdir -p /mnt/d/contextgraph/{storage/{contextgraph-rocksdb,replay,cold,witness,train-certs,live-predictions,embedder-cache},runtime/{cgreality-shift-log,test-outcomes},logs,state/{locks,cgreality,gold-labels},fsv,exports/{active-learning,eval,pairwise-mi,load-tests,test-runs}}
mkdir -p /mnt/d/cabdru/contextgraph/models

# 3. Place SHA-pinned model artifacts under /mnt/d/cabdru/contextgraph/models/
#    (Operator-provided per the per-embedder digest list)

# 4. Build
cd /home/cabdru/contextgraph
cargo build --release --workspace

# 5. Install systemd unit
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now mejepa-daemon

# 6. Verify
systemctl --user status mejepa-daemon
ls -la /mnt/d/contextgraph/state/locks/
tail /mnt/d/contextgraph/logs/mejepa-daemon.stdout.log

# 7. MCP smoke test
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"mcp__cgreality__mejepa_quota_status","arguments":{}}}' \
    | nc -U /mnt/d/contextgraph/state/locks/mcp.sock
```

---

## 2. Daily ops (2–5 minutes)

```bash
# 2.1 Daemon health
mcp_call mejepa_daemon_status
# subscriber.lag_ms < 5000, heal.last_drift_class != Catastrophic,
# quota.used_gb < 110, vram.peak_gb < budget_gb

# 2.2 Operator alerts
ls /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/ 2>/dev/null

# 2.3 Active-learning queue
context-graph-cli mejepa active-learning list --top-n 5
```

---

## 3. Weekly ops (15–30 minutes)

```bash
# Mondays
less /mnt/d/contextgraph/exports/eval/$(date +%Y-%m-%d)/weekly.md
# Look for: per-(category × language) ship-gate status, drift events,
# promotions/rollbacks, agent-feedback summary, storage trend

context-graph-cli mejepa active-learning list --top-n 20
# Label or dismiss each entry

# Every other week:
xdg-open /mnt/d/contextgraph/exports/pairwise-mi/$(ls /mnt/d/contextgraph/exports/pairwise-mi/ | tail -1)
# If pair MI > 0.85 sustained 2 audits: retire / recalibrate / accept

du -sh /mnt/d/contextgraph/
mcp_call mejepa_quota_status
```

---

## 4. Monthly ops (30–60 minutes)

```bash
ls -la /mnt/d/contextgraph-backups/  # backups < 7 days old
context-graph-cli mejepa hygiene cold-audit
mcp_call optimizer_witness_chain_verify --depth full
# Quarterly soak test
git log --since="1 month ago" --oneline -- docs/futurebuild/ docs/ruvectorfindings/
```

---

## 5. Incident response

### 5.1 `MEJEPA_DAEMON_FLAPPING` (> 5 restarts/5 min)

```bash
journalctl --user -u mejepa-daemon -n 200
tail -100 /mnt/d/contextgraph/logs/mejepa-daemon.stderr.log
```

Identify the flapping task, investigate, write blocker if unknown, restart.

### 5.2 `MEJEPA_WITNESS_CHAIN_CORRUPTION`

```bash
mcp_call optimizer_witness_chain_diff --since-last-verified
# If auto-repair failed:
mcp_call optimizer_witness_chain_repair_legacy --force
# Last resort: restore last known-good RocksDB checkpoint
```

### 5.3 `MEJEPA_DRIFT_CATASTROPHIC`

```bash
mcp_call mejepa_heal_status
less /mnt/d/contextgraph/exports/eval/.../weekly.md
# If retrain reasonable:
mcp_call mejepa_promote_approval --promotion-id <id> --approve true
# If suspicious:
mcp_call mejepa_promote_approval --promotion-id <id> --approve false --reason "..."
```

### 5.4 `MEJEPA_QUOTA_EXCEEDED`

```bash
mcp_call mejepa_quota_status
mcp_call mejepa_gc_run --force-eviction true
context-graph-cli mejepa hygiene aggressive-cold-tier-down
```

### 5.5 OOM / RSS climbing

```bash
systemctl --user restart mejepa-daemon
# Profile with valgrind during repro
# Write high-severity blocker
```

### 5.6 `MEJEPA_D_DRIVE_UNAVAILABLE`

```bash
mount | grep /mnt/d
# If unmounted: remount; restart daemon
# If corrupted: see 23_disaster_recovery.md §3
```

### 5.7 Silently wrong predictions

```bash
mcp_call mejepa_ship_gate_status
# If specific cell regressing:
mcp_call mejepa_constellation_inspect --force-refit --category <c> --language <l>
# If pervasive: file blocker; consider full retrain
```

---

## 6. Configuration tuning

| Knob | Default | Where | When to change |
|---|---|---|---|
| Drift thresholds | 88/85/80% | daemon config | when production calibrates |
| Subscriber tick | 250 ms | daemon config | rarely; tighten if lag > 5s p50 |
| Promote tick | 3600 s | daemon config | rarely |
| Storage hard quota | 115 GB | daemon config | hardware change |
| GC schedule | nightly 03:00 local | systemd timer | operator preference |
| Active-learning surface | top-N=10, period=600s | daemon config | operator capacity |
| Per-head λ_h | per `10 §8` defaults | trained-in | only via supersession |

Configuration in `~/.config/mejepa/daemon.toml` (futurebuild). Edits require daemon restart.

---

## 7. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Daemon won't start | D-drive missing | mount + verify with `ls /mnt/d/contextgraph/` |
| Daemon won't start | model file missing | check digest report; place under `/mnt/d/cabdru/contextgraph/models/` |
| Daemon won't start | pid lock held | `pkill -f context-graph-mcp.*--daemon` |
| Predictions never appear | subscriber lagged | `mejepa_subscriber_status`; check shift-log dir; check tokio runtime |
| Predictions appear but agent doesn't see them | injection hook misconfigured | check `.claude/settings.json hooks.UserPromptSubmit`; test manually |
| `mejepa_record_agent_feedback` rate-limited | > 100/hr/agent | wait or use different agent_id |
| Sampling weight not bumping | scheduler not consuming queue | check active-learning tick firing; check CF |
| Disk filling fast | shift-log retention broken | check GC logs; `mcp_call mejepa_gc_run` |
| Wrong predictions on Python, Rust fine | per-language drift | `mejepa_ship_gate_status --by-language python`; per-language LoRA refresh |
| `nvidia-smi` reports N/A | WSL passthrough | per `TECH-OBSERVABILITY §5` |

---

## 8. Every operator MCP tool (reference)

See [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md). Key tools: `mejepa_daemon_status`, `mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`, `mejepa_vram_budget_report`, `mejepa_eval_run`, `mejepa_ship_gate_status`, `mejepa_active_learning_queue`, `mejepa_eval_build_graph`, `mejepa_audit_pairwise_mi`, `mejepa_constellation_inspect`, `mejepa_gc_run`, `mejepa_witness_compress`, `mejepa_promote_approval`, `mejepa_rollback_to`, `optimizer_witness_chain_verify`, `optimizer_recall_recommendations`.

---

## 9. Every operator CLI command (reference)

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

---

## 10. Escalation path

If §5 doesn't resolve within 1 day:

1. Write high-severity blocker in `memory/blockers/<date>--<short-name>.md` with severity, reproducible scenario, attempted fixes.
2. If user-facing degradation: `mcp_call mejepa_pause_predictions --duration-mins 60`; agent gets `Verdict::Abstain` until resumed.
3. Investigate root cause; fix; resume.
4. Systemic: file project-repo issue with daemon logs, training certs, failing prediction's witness hash.

---

## 11. Cross-references

[`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md), [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md), [`23_disaster_recovery.md`](23_disaster_recovery.md), [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).
