#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-cell-status-fsv"
SEED_JSON="$FSV_ROOT/seed-output.json"
PASS_JSON="$FSV_ROOT/pass-output.json"
BELOW_JSON="$FSV_ROOT/below-threshold-output.json"
MISSING_JSON="$FSV_ROOT/missing-cell-output.json"
REPORT_JSON="$FSV_ROOT/cell_status_fsv.json"

rm -rf "$FSV_ROOT"
mkdir -p "$FSV_ROOT"

cd "$REPO_ROOT"
cargo run -p context-graph-cli --example phase_g_cell_status_seed_fsv -- "$FSV_ROOT" > "$SEED_JSON"
cargo build -p context-graph-cli --bin context-graph-cli

BIN="$REPO_ROOT/target/debug/context-graph-cli"
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/pass/infer-rocksdb" --threshold 0.95 > "$PASS_JSON"

below_exit=0
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/below-threshold/infer-rocksdb" --threshold 0.95 > "$BELOW_JSON" || below_exit=$?

missing_exit=0
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/missing-cell/infer-rocksdb" --threshold 0.95 > "$MISSING_JSON" || missing_exit=$?

python3 - "$SEED_JSON" "$PASS_JSON" "$BELOW_JSON" "$MISSING_JSON" "$REPORT_JSON" "$below_exit" "$missing_exit" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

seed_path = Path(sys.argv[1])
pass_path = Path(sys.argv[2])
below_path = Path(sys.argv[3])
missing_path = Path(sys.argv[4])
report_path = Path(sys.argv[5])
below_exit = int(sys.argv[6])
missing_exit = int(sys.argv[7])

seed = json.loads(seed_path.read_text())
passed = json.loads(pass_path.read_text())
below = json.loads(below_path.read_text())
missing = json.loads(missing_path.read_text())

def find_cell(output, cell):
    for row in output["cells"]:
        if row["cell"] == cell:
            return row
    raise AssertionError(f"missing cell row: {cell}")

below_cell = find_cell(below, "off_by_one::rust")
missing_cell = find_cell(missing, "known_good::ruby")

assertions = {
    "seed_readback_equal": all(s["readback_equal"] for s in seed["scenarios"]),
    "seed_cf_counts": all(s["cf_count"] == 1 for s in seed["scenarios"]),
    "pass_exit_zero": True,
    "below_exit_one": below_exit == 1,
    "missing_exit_one": missing_exit == 1,
    "pass_lists_all_80_cells": passed["requiredCellCount"] == 80 and len(passed["cells"]) == 80,
    "below_lists_all_80_cells": below["requiredCellCount"] == 80 and len(below["cells"]) == 80,
    "missing_lists_all_80_cells": missing["requiredCellCount"] == 80 and len(missing["cells"]) == 80,
    "pass_counts": (
        passed["passed"] is True
        and passed["passingCellCount"] == 80
        and passed["belowThresholdCellCount"] == 0
        and passed["missingCellCount"] == 0
        and passed["failingCellCount"] == 0
    ),
    "below_counts": (
        below["passed"] is False
        and below["passingCellCount"] == 79
        and below["belowThresholdCellCount"] == 1
        and below["missingCellCount"] == 0
        and below["failingCellCount"] == 1
    ),
    "missing_counts": (
        missing["passed"] is False
        and missing["passingCellCount"] == 79
        and missing["belowThresholdCellCount"] == 0
        and missing["missingCellCount"] == 1
        and missing["failingCellCount"] == 1
    ),
    "below_cell_detail": (
        below_cell["status"] == "below_threshold"
        and abs(below_cell["correlation"] - 0.91) < 0.0001
        and 0.039 < below_cell["distanceToThreshold"] < 0.041
        and below_cell["holdoutCount"] == 800
    ),
    "missing_cell_detail": (
        missing_cell["status"] == "missing"
        and missing_cell["correlation"] is None
        and missing_cell["distanceToThreshold"] is None
        and missing_cell["holdoutCount"] == 800
    ),
    "source_of_truth_cf": (
        passed["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
        and below["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
        and missing["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
    ),
}

head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
report = {
    "schema_version": 1,
    "build_release_sha": head,
    "fsv_name": "phase-g-cell-status-fsv",
    "seed_output_sha256": hashlib.sha256(seed_path.read_bytes()).hexdigest(),
    "pass_output_sha256": hashlib.sha256(pass_path.read_bytes()).hexdigest(),
    "below_threshold_output_sha256": hashlib.sha256(below_path.read_bytes()).hexdigest(),
    "missing_cell_output_sha256": hashlib.sha256(missing_path.read_bytes()).hexdigest(),
    "seed": seed,
    "command_outputs": {
        "pass": passed,
        "below_threshold": below,
        "missing_cell": missing,
    },
    "exit_codes": {
        "pass": 0,
        "below_threshold": below_exit,
        "missing_cell": missing_exit,
    },
    "assertions": assertions,
    "all_passed": all(assertions.values()),
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
if not report["all_passed"]:
    raise SystemExit(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps(report, indent=2, sort_keys=True))
PY

sha256sum "$REPORT_JSON"
