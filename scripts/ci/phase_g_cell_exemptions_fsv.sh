#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/mnt/d/contextgraph/fsv/phase-g-cell-exemptions-fsv/cell_exemptions_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mcp --example phase_g_cell_exemptions_fsv
sha256sum "$REPORT_JSON"
