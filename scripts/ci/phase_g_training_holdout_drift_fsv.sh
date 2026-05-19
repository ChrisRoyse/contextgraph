#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/zfs/archive/contextgraph/fsv/phase-g-training-holdout-drift-fsv/training_holdout_drift_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mejepa --example phase_g_training_holdout_drift_fsv
test -s "$REPORT_JSON"
sha256sum "$REPORT_JSON"
