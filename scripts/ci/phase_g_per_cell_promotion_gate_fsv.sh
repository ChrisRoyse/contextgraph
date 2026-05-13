#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/mnt/d/contextgraph/fsv/phase-g-per-cell-promotion-gate-fsv/per_cell_promotion_gate_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mejepa --example phase_g_per_cell_promotion_gate_fsv
test -s "$REPORT_JSON"
sha256sum "$REPORT_JSON"
