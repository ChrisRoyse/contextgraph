# SPEC-DR — Functional Specification

> **Level 2.** Disaster recovery + backup.
> Implementation reference: [`20_DISASTER_RECOVERY_AND_BACKUP.md`](../../20_DISASTER_RECOVERY_AND_BACKUP.md).

## User stories

- **As an operator**, I want hourly local snapshots and daily off-machine backups so I never lose more than 1 hour of state.
- **As an operator**, I want documented restore procedures that I can run from memory in an incident.
- **As a maintainer**, I want backup verification monthly so I know the backups actually work.

## Requirements

### REQ-DR-01 — Hourly local snapshot
`restic` (or btrfs) snapshots at `/mnt/d/contextgraph-backups/local`, hourly via systemd timer. Excludes ephemeral shift-log files.

### REQ-DR-02 — Daily off-machine snapshot
Same `restic` to a separate disk / NAS / cloud. Excludes ephemeral + embedder-cache (rebuildable).

### REQ-DR-03 — Retention policy
Local: 24h + 7d + 4w. Off-machine: 14d + 8w + 6m. `restic forget --prune` enforces.

### REQ-DR-04 — Model digest list
SHA-pinned digests committed at `config/model_digests.json` so DR can re-fetch without ambiguity.

### REQ-DR-05 — Model verify-and-fetch CLI
`context-graph-cli mejepa models verify-and-fetch --digest-list <path>` confirms digests + fetches missing.

### REQ-DR-06 — Storage verification CLI
`context-graph-cli mejepa storage verify --root <path>` runs RocksDB integrity checks + witness-chain verification on a restored backup.

### REQ-DR-07 — CF-restoration CLI
`context-graph-cli mejepa storage restore-cf <cf-name> --source <path>` replaces a single CF's SST files atomically.

### REQ-DR-08 — Restore audit trail
Every `restore-cf` or full restore writes a `CF_MEJEPA_HEAL_REPORTS` entry tagged `event=manual_restore` with operator name + target version.

### REQ-DR-09 — Backup encryption
`restic` repo passphrase stored at `~/.config/mejepa/backup-passphrase` (mode 0600).

### REQ-DR-10 — Backup verification monthly
`scripts/dr/verify_backup.sh` runs monthly, restores latest snapshot to scratch location, runs storage verify, alerts on failure.

### REQ-DR-11 — DR drill quarterly
Chaos-test variant simulates D-drive corruption + verifies full restore + asserts daemon resumes operations within RTO.

### REQ-DR-12 — Recovery time objectives
Local snapshot restore: RTO ≤ 30 min, RPO ≤ 1 hour. Off-machine restore: RTO ≤ 4 hours, RPO ≤ 24 hours.

## Acceptance criteria

1. Hourly snapshot timer is running; check with `systemctl --user list-timers | grep mejepa-backup`.
2. `restic snapshots` shows ≥ 24 local snapshots after 24 hours of operation.
3. Test restore succeeds: scratch restore + storage verify passes.
4. Quarterly DR drill completes within RTO.

## Non-functional

- Backup operation < 5 min per hourly snapshot (deduplication keeps it small).
- Off-machine snapshot < 30 min daily.
- Verify operation < 10 min.
