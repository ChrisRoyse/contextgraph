#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/zfs/archive/contextgraph/fsv/phase-g-convergence-eta-fsv"
REPORT="${FSV_ROOT}/convergence_eta_fsv.json"

cd "${ROOT}"
cargo run --release -p context-graph-mcp --example phase_g_convergence_eta_fsv

test -s "${REPORT}"
sha256sum "${REPORT}"
