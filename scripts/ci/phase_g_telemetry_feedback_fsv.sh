#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-telemetry-feedback-fsv"
REPORT_JSON="$FSV_ROOT/telemetry_feedback_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mejepa --example phase_g_telemetry_feedback_fsv
test -s "$REPORT_JSON"
sha256sum "$REPORT_JSON"
