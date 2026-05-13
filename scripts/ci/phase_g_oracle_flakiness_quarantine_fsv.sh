#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-oracle-flakiness-quarantine-fsv"
REPORT="${FSV_ROOT}/oracle_flakiness_quarantine_fsv.json"

cd "${ROOT}"
cargo run --release -p context-graph-cli --example phase_g_oracle_flakiness_quarantine_fsv

test -s "${REPORT}"
sha256sum "${REPORT}"
