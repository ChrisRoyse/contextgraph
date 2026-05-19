#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
REPORT_JSON="/zfs/archive/contextgraph/fsv/phase-g-targeted-mutation-expansion-fsv/targeted_mutation_expansion_fsv.json"

cd "$REPO_ROOT"
cargo run -p context-graph-mejepa-corpus --example phase_g_targeted_mutation_expansion_fsv
sha256sum "$REPORT_JSON"
