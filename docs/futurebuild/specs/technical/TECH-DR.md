# TECH-DR — Technical Specification

> **Level 3.** Implements [`SPEC-DR.md`](../functional/SPEC-DR.md). Implementation reference: [`20_DISASTER_RECOVERY_AND_BACKUP.md`](../../20_DISASTER_RECOVERY_AND_BACKUP.md).

## Module layout

```
scripts/dr/
  local_snapshot.sh                # REQ-DR-01
  offmachine_snapshot.sh           # REQ-DR-02
  verify_backup.sh                 # REQ-DR-10
  dr_drill.sh                      # REQ-DR-11
scripts/systemd/
  mejepa-backup-local.service      # systemd timer for local
  mejepa-backup-local.timer
  mejepa-backup-offmachine.service
  mejepa-backup-offmachine.timer
  mejepa-backup-verify.service     # monthly
  mejepa-backup-verify.timer
crates/context-graph-cli/
  src/commands/
    mejepa_storage_verify.rs       # REQ-DR-06
    mejepa_storage_restore_cf.rs   # REQ-DR-07
    mejepa_models_verify_fetch.rs  # REQ-DR-05
config/
  model_digests.json               # REQ-DR-04
```

## Snapshot scripts

```bash
#!/usr/bin/env bash
# scripts/dr/local_snapshot.sh
set -euo pipefail
source ~/.config/mejepa/backup.toml.exports     # reads exported env vars from toml

restic -r "$LOCAL_REPO" snapshots > /dev/null || \
    restic -r "$LOCAL_REPO" init
restic -r "$LOCAL_REPO" backup /mnt/d/contextgraph \
    --exclude '/mnt/d/contextgraph/runtime/cgreality-shift-log/*' \
    --tag "automated-hourly"
restic -r "$LOCAL_REPO" forget --keep-hourly 24 --keep-daily 7 --keep-weekly 4 --prune \
    --tag "automated-hourly"

# Audit entry
context-graph-cli mejepa heal-report write \
    --event backup_local --result success \
    --metadata "{\"snapshot_id\":\"$(restic -r "$LOCAL_REPO" snapshots --last 1 --json | jq -r '.[].id')\"}"
```

```bash
#!/usr/bin/env bash
# scripts/dr/offmachine_snapshot.sh
set -euo pipefail
source ~/.config/mejepa/backup.toml.exports
restic -r "$OFFMACHINE_REPO" backup /mnt/d/contextgraph \
    --exclude '/mnt/d/contextgraph/runtime/cgreality-shift-log/*' \
    --exclude '/mnt/d/contextgraph/storage/embedder-cache/*' \
    --tag "automated-daily"
restic -r "$OFFMACHINE_REPO" forget --keep-daily 14 --keep-weekly 8 --keep-monthly 6 --prune \
    --tag "automated-daily"
context-graph-cli mejepa heal-report write \
    --event backup_offmachine --result success
```

## systemd timers

```ini
# scripts/systemd/mejepa-backup-local.timer
[Unit]
Description=Hourly local backup

[Timer]
OnCalendar=hourly
Persistent=true
Unit=mejepa-backup-local.service

[Install]
WantedBy=timers.target

# scripts/systemd/mejepa-backup-local.service
[Unit]
Description=Local backup

[Service]
Type=oneshot
ExecStart=/home/cabdru/contextgraph/scripts/dr/local_snapshot.sh
StandardOutput=append:/mnt/d/contextgraph/logs/backup-local.log
```

## Storage verify CLI

```rust
// crates/context-graph-cli/src/commands/mejepa_storage_verify.rs
pub struct StorageVerifyCmd {
    #[arg(long)] root: PathBuf,
    #[arg(long)] cf: Option<String>,
}

impl StorageVerifyCmd {
    pub fn run(&self) -> Result<()> {
        let storage = Storage::open(&self.root)?;
        let mut report = StorageVerifyReport::default();
        
        // RocksDB integrity check
        report.rocksdb_check = storage.verify_integrity()?;
        
        // Witness chain verify
        report.witness_chain_check = optimizer_witness_chain_verify(&storage, depth = "full")?;
        
        // Per-CF row count
        for cf_name in storage.cf_names() {
            if self.cf.as_ref().map_or(true, |c| c == &cf_name) {
                report.cf_row_counts.insert(cf_name.clone(), storage.cf_count(&cf_name)?);
            }
        }
        
        println!("{}", serde_json::to_string_pretty(&report)?);
        if !report.passed() { std::process::exit(1) }
        Ok(())
    }
}
```

## Restore-CF CLI

```rust
pub struct StorageRestoreCfCmd {
    #[arg(long)] cf_name: String,
    #[arg(long)] source: PathBuf,
    #[arg(long)] target_root: PathBuf,
}

impl StorageRestoreCfCmd {
    pub fn run(&self) -> Result<()> {
        // Confirm daemon stopped
        confirm_daemon_stopped()?;
        
        // Verify the source CF
        let src_storage = Storage::open(&self.source)?;
        let src_count = src_storage.cf_count(&self.cf_name)?;
        eprintln!("Source CF {} has {} rows", self.cf_name, src_count);
        
        // Drop target CF, recreate, copy entries
        let mut dst = Storage::open(&self.target_root)?;
        dst.drop_cf(&self.cf_name)?;
        dst.create_cf(&self.cf_name)?;
        for (k, v) in src_storage.iter_cf(&self.cf_name)? {
            dst.put_cf(&self.cf_name, &k, &v)?;
        }
        let dst_count = dst.cf_count(&self.cf_name)?;
        eprintln!("Restored {} rows to target", dst_count);
        
        // Audit
        context_graph_cli::heal_report::write(...)?;
        Ok(())
    }
}
```

## Model digest list

```json
{
  "version": 1,
  "models": [
    {
      "embedder_id": "E1",
      "name": "intfloat/e5-large-v2.gguf",
      "path_relative": "e5-large-v2.gguf",
      "sha256": "abc123...",
      "size_bytes": 600000000,
      "url": "https://huggingface.co/intfloat/e5-large-v2/resolve/main/...",
      "license": "MIT"
    },
    {
      "embedder_id": "E7",
      "name": "Qodo/Qodo-Embed-1-1.5B.gguf",
      "path_relative": "qodo-embed-1.5b.gguf",
      "sha256": "def456...",
      "size_bytes": 1500000000,
      "url": "https://...",
      "license": "Apache-2.0"
    }
    // ... per embedder ...
  ]
}
```

The verify-and-fetch CLI checks each entry, fetches if missing, verifies digest, fails closed on mismatch.

## DR drill

```bash
#!/usr/bin/env bash
# scripts/dr/dr_drill.sh
set -euo pipefail

echo "DR DRILL — quarterly"

# 1. Stop daemon
systemctl --user stop mejepa-daemon

# 2. Snapshot before drill
restic -r /mnt/d/contextgraph-backups/local backup /mnt/d/contextgraph --tag "pre-drill"

# 3. Simulate corruption: rename a CF directory
mv /mnt/d/contextgraph/storage/contextgraph-rocksdb /mnt/d/contextgraph/storage/contextgraph-rocksdb-corrupt

# 4. Restore from latest snapshot
restic -r /mnt/d/contextgraph-backups/local restore latest --target /mnt/d/

# 5. Verify
context-graph-cli mejepa storage verify --root /mnt/d/contextgraph/storage/contextgraph-rocksdb

# 6. Restart daemon
systemctl --user start mejepa-daemon

# 7. Validate operations
sleep 30
context-graph-cli mejepa daemon status

# 8. Cleanup
rm -rf /mnt/d/contextgraph/storage/contextgraph-rocksdb-corrupt

echo "DR DRILL — completed within RTO ($SECONDS seconds)"
```

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `local_snapshot_fsv` | hourly snapshot creates bounded restic repo | `/mnt/d/contextgraph/fsv/dr-fsv/local_snapshot.json` |
| `restore_round_trip_fsv` | restore to scratch + verify passes | `/mnt/d/contextgraph/fsv/dr-fsv/restore_round_trip.json` |
| `dr_drill_fsv` | full corruption + restore drill within RTO | `/mnt/d/contextgraph/fsv/dr-fsv/drill.json` |
| `model_verify_fsv` | model digest verification | `/mnt/d/contextgraph/fsv/dr-fsv/model_verify.json` |
