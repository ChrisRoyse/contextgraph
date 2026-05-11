# Disaster recovery domain — all atomic tasks

## TASK-DR-001 — Local snapshot script + systemd timer

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-01,03 + TECH-DR §2-3
- **Outputs:** `scripts/dr/local_snapshot.sh`, `scripts/systemd/mejepa-backup-local.{timer,service}`.
- **DoD:** hourly snapshots write to `/mnt/d/contextgraph-backups/local`; retention enforced.

## TASK-DR-002 — Off-machine snapshot script + systemd timer

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-02,03
- **Outputs:** `scripts/dr/offmachine_snapshot.sh`, systemd unit.
- **DoD:** daily snapshots to off-machine target; retention enforced.

## TASK-DR-003 — `context-graph-cli mejepa storage verify`

- **Phase:** F · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-06 + TECH-DR §6
- **Outputs:** `crates/context-graph-cli/src/commands/mejepa_storage_verify.rs`.
- **DoD:** runs RocksDB integrity + witness chain check; exit 0 on healthy storage.

## TASK-DR-004 — `context-graph-cli mejepa storage restore-cf`

- **Phase:** F · **Effort:** 1d · **Spec:** SPEC-DR REQ-DR-07 + TECH-DR §7
- **Outputs:** `mejepa_storage_restore_cf.rs`.
- **DoD:** restoring a single CF from snapshot replaces SST files atomically; row count matches source.

## TASK-DR-005 — `context-graph-cli mejepa models verify-and-fetch`

- **Phase:** A · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-05 + TECH-DR §8-9
- **Outputs:** `mejepa_models_verify_fetch.rs`; uses `config/model_digests.json`.
- **DoD:** missing models fetched; digest mismatch fails closed; matching digests skip.

## TASK-DR-006 — Model digest list

- **Phase:** A · **Effort:** 0.25d · **Spec:** SPEC-DR REQ-DR-04
- **Outputs:** `config/model_digests.json` with entries for the 12 active embedders.
- **DoD:** all 12 embedders have SHA-256 + URL + size + license entries.

## TASK-DR-007 — Restore verification monthly cron

- **Phase:** F · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-10
- **Outputs:** `scripts/dr/verify_backup.sh`, monthly systemd timer.
- **DoD:** monthly: restore latest to scratch, run storage verify, alert on failure.

## TASK-DR-008 — DR drill quarterly

- **Phase:** F · **Effort:** 0.5d · **Spec:** SPEC-DR REQ-DR-11 + TECH-DR §11
- **Outputs:** `scripts/dr/dr_drill.sh`.
- **DoD:** simulates corruption, restores, verifies daemon resumes within RTO.
