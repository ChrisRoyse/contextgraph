#!/usr/bin/env bash
# TASK-DR-008 — Quarterly disaster-recovery drill.
#
# Simulates RocksDB corruption against a *snapshot copy* of the
# inference DB, then restores from the most recent backup and verifies
# the daemon binary can re-open the restored tree and that storage verify
# passes. Records wall-clock time so the RTO budget can be audited.
#
# The live aiwonder inference DB is NEVER touched: corruption only happens
# inside the scratch root that this script owns.

set -euo pipefail

REPO_ROOT="${MEJEPA_DR_REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
BACKUP_ROOT="${MEJEPA_BACKUP_ROOT:-/zfs/archive/contextgraph/backups/local}"
SCRATCH_ROOT="${MEJEPA_DR_SCRATCH_ROOT:-/zfs/hot/contextgraph/dr-scratch}"
CLI_BIN="${MEJEPA_CLI_BIN:-${REPO_ROOT}/target/release/context-graph-cli}"
EVIDENCE_ROOT="${MEJEPA_DR_EVIDENCE_ROOT:-/zfs/archive/contextgraph/fsv/dr-drill-fsv}"
RTO_BUDGET_SECONDS="${MEJEPA_DR_RTO_BUDGET_SECONDS:-1800}"

if [[ ! -d "${BACKUP_ROOT}" ]]; then
  echo "MEJEPA_DR_DRILL_BACKUP_ROOT_MISSING: ${BACKUP_ROOT} not found" >&2
  exit 1
fi
if [[ ! -x "${CLI_BIN}" ]]; then
  echo "MEJEPA_DR_DRILL_CLI_BIN_MISSING: ${CLI_BIN} not executable" >&2
  exit 1
fi

mkdir -p "${EVIDENCE_ROOT}"
stamp="$(date -u '+%Y%m%dT%H%M%SZ')"
drill_root="${SCRATCH_ROOT}/drill-${stamp}-$$"
mkdir -p "${drill_root}"
trap 'rm -rf "${drill_root}"' EXIT INT TERM

# Pick the most recent snapshot to drill against.
snapshot="$(find "${BACKUP_ROOT}" -mindepth 1 -maxdepth 1 -type d -printf '%T@ %p\n' \
  | sort -nr | head -1 | cut -d' ' -f2- || true)"
if [[ -z "${snapshot}" ]]; then
  echo "MEJEPA_DR_DRILL_NO_SNAPSHOT: ${BACKUP_ROOT} has no snapshots" >&2
  exit 1
fi

evidence="${EVIDENCE_ROOT}/dr_drill_${stamp}.json"
start_ts=$(date -u '+%s')

# 1. Restore the snapshot into the scratch DB root.
restored="${drill_root}/restored-rocksdb"
cp -a "${snapshot}/." "${restored}/"

# 2. Simulate corruption by truncating one SST file in place. Records
#    the original size so the drill is reproducible; the live tree is
#    untouched because we operate only inside ${restored}.
victim="$(find "${restored}" -type f -name '*.sst' | head -1 || true)"
if [[ -z "${victim}" ]]; then
  echo "MEJEPA_DR_DRILL_NO_SST: snapshot has no .sst files to corrupt" >&2
  exit 1
fi
original_size=$(stat --format=%s "${victim}")
: > "${victim}"  # truncate to 0 bytes
corrupted_size=$(stat --format=%s "${victim}")

# 3. Confirm storage-verify fails on the corrupted tree.
set +e
"${CLI_BIN}" mejepa storage-verify --db-path "${restored}" > "${drill_root}/corrupt_verify.json" 2>&1
corrupt_rc=$?
set -e
if [[ "${corrupt_rc}" -eq 0 ]]; then
  echo "MEJEPA_DR_DRILL_CORRUPTION_UNDETECTED: storage-verify reported healthy on truncated SST" >&2
  exit 1
fi

# 4. Re-restore the snapshot over the corruption (the recovery step).
rm -rf "${restored}"
cp -a "${snapshot}/." "${restored}/"

# 5. Verify the recovered tree.
"${CLI_BIN}" mejepa storage-verify --db-path "${restored}" > "${drill_root}/recovered_verify.json"

end_ts=$(date -u '+%s')
elapsed=$((end_ts - start_ts))
within_budget="false"
if [[ "${elapsed}" -le "${RTO_BUDGET_SECONDS}" ]]; then
  within_budget="true"
fi

cat > "${evidence}" <<JSON
{
  "task_id": "TASK-DR-008",
  "snapshot": "${snapshot}",
  "scratch_root": "${drill_root}",
  "victim_sst": "${victim}",
  "original_size_bytes": ${original_size},
  "corrupted_size_bytes": ${corrupted_size},
  "corrupt_verify_rc": ${corrupt_rc},
  "recovered_verify_path": "${drill_root}/recovered_verify.json",
  "elapsed_seconds": ${elapsed},
  "rto_budget_seconds": ${RTO_BUDGET_SECONDS},
  "within_budget": ${within_budget},
  "all_passed": ${within_budget}
}
JSON

if [[ "${within_budget}" != "true" ]]; then
  echo "MEJEPA_DR_DRILL_RTO_EXCEEDED: elapsed=${elapsed}s > budget=${RTO_BUDGET_SECONDS}s; see ${evidence}" >&2
  exit 1
fi

cat "${evidence}"
