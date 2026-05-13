#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-failure-mode-class-metrics-fsv"
REPORT="${FSV_ROOT}/failure_mode_class_metrics_fsv.json"

cd "${ROOT}"
cargo run --release -p context-graph-mcp --example phase_g_failure_mode_class_metrics_fsv

test -s "${REPORT}"
sha256sum "${REPORT}"
