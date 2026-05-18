# 20 — Disaster Recovery and Backup

> **Purpose.** What happens when D:\ fails, RocksDB corrupts, or the daemon's storage gets into an unrecoverable state. The shipped substrate has hash-chained provenance + permanent merge history + EWC++ Fisher snapshots, but no spec describes the operational backup + restore strategy.
>
> Owned by [`SPEC-DR.md`](specs/functional/SPEC-DR.md) + [`TECH-DR.md`](specs/technical/TECH-DR.md).

---

## 1. Threat model

| Threat | Likelihood | Recovery target |
|---|---|---|
| D:\ filesystem corruption | low (modern hardware) | restore from snapshot ≤ 1 hour |
| RocksDB SST corruption (bug / power loss) | medium | repair via RocksDB tools or restore from snapshot |
| Witness chain corruption (hash mismatch) | low | auto-repair; if fail, restore segment from snapshot |
| Operator deletes a CF accidentally | low (lock files exist) | restore from snapshot |
| Catastrophic model regression after a promotion | medium | rollback via `mejepa_rollback_to` (immediate) + restore weights from snapshot |
| Machine total loss (hardware failure) | low | restore everything from off-machine backup |
| Ransomware / malware encrypts D:\ | low | restore from off-machine backup |

---

## 2. Backup strategy

### 2.1 Local snapshot (hourly)

Tool: `restic` (or btrfs snapshots if filesystem supports). Schedule: hourly via systemd timer.

```bash
# scripts/dr/local_snapshot.sh
restic -r /mnt/d/contextgraph-backups/local backup /mnt/d/contextgraph \
    --exclude '/mnt/d/contextgraph/runtime/cgreality-shift-log/*'  # ephemeral
restic -r /mnt/d/contextgraph-backups/local forget --keep-hourly 24 --keep-daily 7 --keep-weekly 4 --prune
```

Retention: 24 hours + 7 days + 4 weeks. Total local snapshot footprint typically < 30 GB (deduplicated).

### 2.2 Off-machine backup (daily)

Tool: `restic` to a separate disk / NAS / cloud. Schedule: nightly at 04:00 (after nightly GC at 03:00).

```bash
# scripts/dr/offmachine_snapshot.sh
restic -r <off-machine-repo> backup /mnt/d/contextgraph \
    --exclude '/mnt/d/contextgraph/runtime/cgreality-shift-log/*' \
    --exclude '/mnt/d/contextgraph/storage/embedder-cache/*'  # rebuildable
restic -r <off-machine-repo> forget --keep-daily 14 --keep-weekly 8 --keep-monthly 6 --prune
```

Retention: 14 days + 8 weeks + 6 months. Off-machine target is operator-chosen.

### 2.3 Model artifact backup

Models under `/mnt/d/cabdru/contextgraph/models/` are large (~6.5 GB) but reproducible from upstream. Backup strategy:
- SHA-pinned digest list committed to git (so we know what to fetch).
- Models themselves NOT in restic (reduces backup cost).
- Disaster recovery re-fetches per the digest list.

### 2.4 Witness-chain Merkle bundles

Per `06 HYGIENE` (shipped REQ-HYG-03), witness chains > 90 days are Merkle-compressed. These bundles are part of the regular backup. Verification:
- `optimizer_witness_chain_verify` runs against restored bundle to confirm hash chain integrity post-restore.

---

## 3. Restore procedures

### 3.1 Full restore from local snapshot

```bash
# 1. Stop daemon
systemctl --user stop mejepa-daemon

# 2. Move corrupt state aside
mv /mnt/d/contextgraph /mnt/d/contextgraph-corrupt-$(date +%Y%m%d-%H%M)

# 3. Restore latest snapshot
restic -r /mnt/d/contextgraph-backups/local restore latest --target /mnt/d/

# 4. Verify
ls -la /mnt/d/contextgraph/state/locks/
# pid file may be stale; remove if so
rm -f /mnt/d/contextgraph/state/locks/context-graph-daemon.pid

# 5. Start daemon
systemctl --user start mejepa-daemon

# 6. Verify operations
mcp_call mejepa_daemon_status
mcp_call optimizer_witness_chain_verify --depth full
```

Recovery time objective (RTO): ≤ 30 minutes for full restore.
Recovery point objective (RPO): ≤ 1 hour (last hourly snapshot).

### 3.2 Partial CF restore

If only one CF is corrupt:

```bash
# 1. Stop daemon
systemctl --user stop mejepa-daemon

# 2. Restore single CF from snapshot
mkdir -p /tmp/restore
restic -r /mnt/d/contextgraph-backups/local restore latest --target /tmp/restore/ --include '/mnt/d/contextgraph/storage/contextgraph-rocksdb/<cf-files>'

# 3. RocksDB-side repair (replace SST files for the CF)
# (Use the CF-restoration helper)
context-graph-cli mejepa storage restore-cf <cf-name> --source /tmp/restore/...

# 4. Restart + verify
systemctl --user start mejepa-daemon
mcp_call mejepa_daemon_status
```

### 3.3 Off-machine restore (machine total loss)

```bash
# On a fresh machine:
# 1. Install Rust + build context-graph-mcp
# 2. Mount the D drive (or equivalent path)
# 3. Restore from off-machine backup
restic -r <off-machine-repo> restore latest --target /mnt/d/

# 4. Re-fetch model artifacts per digest list
context-graph-cli mejepa models verify-and-fetch --digest-list config/model_digests.json

# 5. Install systemd unit + start daemon
cp scripts/systemd/mejepa-daemon.service ~/.config/systemd/user/
systemctl --user enable --now mejepa-daemon

# 6. Verify
mcp_call mejepa_daemon_status
```

RTO for total-loss recovery: ≤ 4 hours (depends on download speed for off-machine restore + model re-fetch).

### 3.4 Model rollback (operational, not DR)

If a recent promotion regresses:

```bash
# Find the previous active version
mcp_call optimizer_recall_recommendations --top-n 10
# Identify the prior weights' SHA

# Rollback
mcp_call mejepa_rollback_to --target-version <version_id>

# Verify
mcp_call mejepa_ship_gate_status
```

This is not a backup operation — it uses the in-CF retention of prior weights (30-day window per `09_SELF_HEALING_AUTONOMY.md §4`).

---

## 4. Backup verification

Monthly per `19_OPERATOR_RUNBOOK.md §4`:

```bash
# Test restore to a scratch location (not /mnt/d/contextgraph)
restic -r /mnt/d/contextgraph-backups/local restore latest --target /tmp/restore-test/

# Run integrity check on the restored RocksDB
context-graph-cli mejepa storage verify --root /tmp/restore-test/mnt/d/contextgraph/storage/contextgraph-rocksdb

# Cleanup
rm -rf /tmp/restore-test/
```

If verification fails: alert; rotate to off-machine snapshot for the next month while investigating.

---

## 5. What's intentionally NOT backed up

| Item | Reason |
|---|---|
| `runtime/cgreality-shift-log/*.jsonl` | ephemeral; replayable from CF_MEJEPA_PANELS via watermark |
| `storage/embedder-cache/` | rebuildable from chunks (slow but free) |
| `target/release/` | rebuildable from source |
| `/tmp/*` | ephemeral by definition |
| Operator-side personal config (`~/.config/`) | operator's own backup responsibility |

---

## 6. Backup encryption + access control

- Local snapshots: `restic` repo with passphrase stored in `~/.config/mejepa/backup-passphrase` (mode 0600, in operator's home)
- Off-machine snapshots: same passphrase or per-target passphrase
- Backup access: limited to operator's user account
- Audit: every restore writes to `CF_MEJEPA_HEAL_REPORTS` with `event=manual_restore`, operator name, target version

---

## 7. Cross-machine recovery considerations

- Witness chains are independent per machine (each machine has its own session_id space). After restore, the chain continues from the last entry — no merge logic needed.
- Active-learning queue items survive restore (they're CF entries).
- Operator-override gold labels survive restore.
- Model versions / pointers survive restore.
- Drift window state survives restore (rolling-1k window resets but recalibrates within minutes).

---

## 8. Configuration

```toml
# ~/.config/mejepa/dr.toml
[backup.local]
repo = "/mnt/d/contextgraph-backups/local"
schedule = "hourly"
retention = { hourly = 24, daily = 7, weekly = 4 }

[backup.offmachine]
repo = "/mnt/external-disk/contextgraph-backups"  # or s3://...
schedule = "daily"
schedule_time = "04:00"
retention = { daily = 14, weekly = 8, monthly = 6 }

[restore]
verify_after_restore = true
verify_witness_chain = true
```

---

## 9. What this commits us to

Tasks (in `tasks/dr/`):

- `TASK-DR-001` — local snapshot script + systemd timer
- `TASK-DR-002` — off-machine snapshot script + systemd timer
- `TASK-DR-003` — `context-graph-cli mejepa storage verify` command
- `TASK-DR-004` — `context-graph-cli mejepa storage restore-cf` command
- `TASK-DR-005` — `context-graph-cli mejepa models verify-and-fetch` command
- `TASK-DR-006` — model digest list (`config/model_digests.json`)
- `TASK-DR-007` — restore verification monthly cron
- `TASK-DR-008` — DR drill (chaos-test variant: simulate corruption + verify restore)
