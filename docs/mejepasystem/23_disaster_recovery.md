# 23 — Disaster Recovery + Backup

Source files covered: `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md`, `docs/futurebuild/21_COLD_START_AND_BOOTSTRAP.md`, `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`.

> Backup strategy + restore procedures + DR drill for the D-drive state.

---

## 1. What lives on D-drive (recovery scope)

All bulk durable state per [`16_hygiene_tier_compression.md §4`](16_hygiene_tier_compression.md) and `docs/futurebuild/16_STORAGE_LOCATION_POLICY.md`:

| Class | Recoverability | Backup priority |
|---|---|---|
| RocksDB workspace | Critical — predictor weights + corpus + train certs + witness chain + agent feedback | **DAILY** |
| Witness chain (`storage/witness/`) | Sacred — cryptographic provenance | **DAILY** + verified |
| Phase 0 mutation corpus (`corpus/`) | Regenerable from `tasks/swebench-lite/*.json` + operators | **WEEKLY** |
| Operator gold labels (`state/gold-labels/`) | Critical — permanent operator labels | **DAILY** |
| Embedder models (`/mnt/d/cabdru/contextgraph/models/`) | Recoverable via SHA-pinned re-download | **WEEKLY** (or on-change) |
| Shift logs (`runtime/cgreality-shift-log/`) | Recent recoverable; older expendable | rolling 7 days |
| FSV evidence (`fsv/`) | Regenerable via re-running tests | not backed up (rolling) |
| Eval reports (`exports/eval/`) | Historical — keep for trend analysis | **WEEKLY** |
| Daemon logs (`logs/`) | Operational history | rolling 30 days |

---

## 2. Backup strategy

### 2.1 Daily snapshot

```bash
# Nightly cron (operator-scheduled)
rsync -aL --delete \
    /mnt/d/contextgraph/storage/ \
    /mnt/d/contextgraph-backups/$(date +%Y-%m-%d)/storage/

rsync -aL --delete \
    /mnt/d/contextgraph/state/gold-labels/ \
    /mnt/d/contextgraph-backups/$(date +%Y-%m-%d)/gold-labels/

rsync -aL --delete \
    /mnt/d/contextgraph/state/cgreality/ \
    /mnt/d/contextgraph-backups/$(date +%Y-%m-%d)/cgreality/

# Verify after copy
mcp_call optimizer_witness_chain_verify --depth full
```

Daily snapshots kept 30 days; one weekly snapshot kept 1 year.

### 2.2 RocksDB checkpoint

Use RocksDB's atomic `CreateCheckpoint` API:

```rust
let checkpoint = rocksdb::checkpoint::Checkpoint::new(&db)?;
checkpoint.create_checkpoint(&backup_path)?;
```

This creates hardlinks where possible (cheap, atomic).

### 2.3 Witness chain integrity verification post-backup

```bash
mcp_call optimizer_witness_chain_verify --backup-path /mnt/d/contextgraph-backups/$(date +%Y-%m-%d)/storage/witness/
```

Must return `valid=true`; else discard backup + alert.

### 2.4 Corpus backup (weekly)

```bash
tar -cz --use-compress-program="zstd -9" \
    -f /mnt/d/contextgraph-backups/$(date +%Y-%V)/corpus-swebench-lite.tar.zst \
    /mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-*/
```

---

## 3. Restore procedures

### 3.1 Full restore (D-drive failure)

```bash
# 1. Stop daemon
systemctl --user stop mejepa-daemon

# 2. Replace /mnt/d/contextgraph/ with last good snapshot
rsync -aL --delete \
    /mnt/d/contextgraph-backups/<date>/storage/ \
    /mnt/d/contextgraph/storage/

# 3. Verify witness chain
mcp_call optimizer_witness_chain_verify --depth full

# 4. Restart daemon
systemctl --user start mejepa-daemon

# 5. Verify health
mcp_call mejepa_daemon_status
```

### 3.2 Single-CF corruption

```bash
# Stop daemon
systemctl --user stop mejepa-daemon

# Restore single CF from RocksDB checkpoint
# (requires offline rocksdb tool)
rocksdb_repair <path>

# Or selectively restore
# Replace specific CF directory in checkpoint with snapshot version

systemctl --user start mejepa-daemon
mcp_call optimizer_witness_chain_verify
```

### 3.3 Model artifact missing

```bash
# Daemon refuses to start with MEJEPA_EMBEDDER_DIGEST_MISMATCH
# Re-download SHA-pinned model:
operator_provided_script <embedder_id> <expected_sha>
# Place under /mnt/d/cabdru/contextgraph/models/<embedder_id>/
systemctl --user start mejepa-daemon
```

### 3.4 Corpus regeneration

```bash
# Phase 0 corpus deterministic via SplitMix64 seed
cargo run --release -p context-graph-mejepa-corpus -- generate \
    --corpus-id <new-corpus-id> \
    --d-root /mnt/d/contextgraph \
    --tasks-dir tasks/swebench-lite \
    --output-root /mnt/d/contextgraph/corpus \
    --seed <seed-from-provenance>
```

---

## 4. Cold start (fresh deploy, no history)

Per `docs/futurebuild/21_COLD_START_AND_BOOTSTRAP.md`:

| State | Behavior |
|---|---|
| `CF_MEJEPA_PANELS` empty | Subscriber-empty path; `mejepa_predict_latest` returns deterministic placeholder verdict |
| `CF_MEJEPA_WEIGHT_BLOBS` empty | Load baseline weights from `models/baseline-mejepa-checkpoint/` (operator-provided); seed Stage 0 cert |
| Constellation empty | Refuse Gτ guard checks; `Verdict::OutOfDistribution` until first constellation built (after corpus generated) |
| Conformal calibration N=0 | Refuse with `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES`; pause prediction emission |
| No SMAP `./memory/` | Auto-create on first turn (per loop.md §9) |
| No `tasks/swebench-lite/` | Recoverable from upstream SWE-bench Lite release (per `BOOT-002`) |

**Cold-start widening**: per `TASK-BOOT-001`, the conformal interval is **artificially widened** during the first 1000 attempts until calibration accumulates. Predictions are made but flagged `confidence_interval.empirical_coverage = cold_start_placeholder`.

---

## 5. DR drill (quarterly)

Per `docs/futurebuild/20_DISASTER_RECOVERY_AND_BACKUP.md §5`:

```
1. Pick a recent snapshot (1-7 days old)
2. Provision a fresh D-drive volume
3. Restore from snapshot
4. Boot daemon
5. Verify:
   - witness chain valid
   - mejepa_predict_latest returns answers
   - mejepa_ship_gate_status returns reasonable per-cell status
   - 100 synthetic verify() calls succeed
   - drift detector reports valid baseline
6. Compare restored state vs source: should be byte-equivalent (RocksDB) or hash-equivalent (everything else)
```

Schedule: 1st Monday of each quarter. Document outcome in `memory/journal/<date>--dr-drill.md`.

---

## 6. Operator gold labels (special handling)

`state/gold-labels/` is **permanent + critical**. Operator-confirmed labels (per active-learning queue) are gold-standard:

- Always backed up daily.
- Never auto-deleted (`gold-labels.ttl = never` regardless of hygiene policy).
- Sampling weight × 6 in BatchSampler.
- Loss against operator-override examples gets Fisher-protected.
- Treat as the bedrock of model behavior.

---

## 7. Witness chain auto-repair

`optimizer_witness_chain_repair_legacy` handles pre-v2 format migration. For active-format corruption:

```bash
# 1. Identify corruption point
mcp_call optimizer_witness_chain_verify
# Returns issues: [{offset, error}]

# 2. Attempt auto-repair
mcp_call optimizer_witness_chain_repair_legacy --offset <corruption_offset> --dry_run true
# If safe to apply:
mcp_call optimizer_witness_chain_repair_legacy --offset <corruption_offset> --dry_run false

# 3. If repair fails, restore from backup
```

---

## 8. Cross-references

[`15_witness_chain.md`](15_witness_chain.md), [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md), [`22_operator_runbook.md`](22_operator_runbook.md), [`14_corpus_phase0.md`](14_corpus_phase0.md).
