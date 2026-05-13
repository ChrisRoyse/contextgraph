#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/mnt/d/contextgraph/fsv/phase-g-conformal-recalibration-fsv/conformal_recalibration_promotion_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mejepa --example phase_g_conformal_recalibration_promotion_fsv
sha256sum "$REPORT_JSON"
