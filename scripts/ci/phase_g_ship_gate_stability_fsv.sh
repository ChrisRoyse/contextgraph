#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/zfs/archive/contextgraph/fsv/phase-g-ship-gate-stability-fsv/ship_gate_stability_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mcp --example phase_g_ship_gate_stability_fsv
sha256sum "$REPORT_JSON"
