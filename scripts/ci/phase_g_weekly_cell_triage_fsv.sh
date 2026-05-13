#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-weekly-cell-triage-fsv"
REPORT="${FSV_ROOT}/weekly_cell_triage_fsv.json"

cd "${ROOT}"
cargo run --release -p context-graph-mcp --example phase_g_weekly_cell_triage_fsv

test -s "${REPORT}"
sha256sum "${REPORT}"
