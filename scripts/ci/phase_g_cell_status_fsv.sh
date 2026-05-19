#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${CONTEXTGRAPH_CELL_STATUS_FSV_ROOT:-/zfs/archive/contextgraph/fsv/phase-g-cell-status-fsv}"
SEED_JSON="$FSV_ROOT/seed-output.json"
PASS_JSON="$FSV_ROOT/pass-output.json"
BELOW_JSON="$FSV_ROOT/below-threshold-output.json"
MISSING_JSON="$FSV_ROOT/missing-correlation-output.json"
STALE_STDOUT="$FSV_ROOT/stale-80-cell-grid-output.json"
STALE_STDERR="$FSV_ROOT/stale-80-cell-grid-error.txt"
OVER_LIMIT_STDOUT="$FSV_ROOT/over-limit-cell-output.json"
OVER_LIMIT_STDERR="$FSV_ROOT/over-limit-cell-error.txt"
EMPTY_STDOUT="$FSV_ROOT/empty-db-output.json"
EMPTY_STDERR="$FSV_ROOT/empty-db-error.txt"
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
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/missing-correlation/infer-rocksdb" --threshold 0.95 > "$MISSING_JSON" || missing_exit=$?

stale_exit=0
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/stale-80-cell-grid/infer-rocksdb" --threshold 0.95 > "$STALE_STDOUT" 2> "$STALE_STDERR" || stale_exit=$?

over_limit_exit=0
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/over-limit-cell/infer-rocksdb" --threshold 0.95 > "$OVER_LIMIT_STDOUT" 2> "$OVER_LIMIT_STDERR" || over_limit_exit=$?

empty_exit=0
"$BIN" mejepa cell-status --db-path "$FSV_ROOT/empty-db/infer-rocksdb" --threshold 0.95 > "$EMPTY_STDOUT" 2> "$EMPTY_STDERR" || empty_exit=$?

python3 - \
  "$SEED_JSON" \
  "$PASS_JSON" \
  "$BELOW_JSON" \
  "$MISSING_JSON" \
  "$STALE_STDOUT" \
  "$STALE_STDERR" \
  "$OVER_LIMIT_STDOUT" \
  "$OVER_LIMIT_STDERR" \
  "$EMPTY_STDOUT" \
  "$EMPTY_STDERR" \
  "$REPORT_JSON" \
  "$below_exit" \
  "$missing_exit" \
  "$stale_exit" \
  "$over_limit_exit" \
  "$empty_exit" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

seed_path = Path(sys.argv[1])
pass_path = Path(sys.argv[2])
below_path = Path(sys.argv[3])
missing_path = Path(sys.argv[4])
stale_stdout_path = Path(sys.argv[5])
stale_stderr_path = Path(sys.argv[6])
over_limit_stdout_path = Path(sys.argv[7])
over_limit_stderr_path = Path(sys.argv[8])
empty_stdout_path = Path(sys.argv[9])
empty_stderr_path = Path(sys.argv[10])
report_path = Path(sys.argv[11])
below_exit = int(sys.argv[12])
missing_exit = int(sys.argv[13])
stale_exit = int(sys.argv[14])
over_limit_exit = int(sys.argv[15])
empty_exit = int(sys.argv[16])

seed = json.loads(seed_path.read_text())
passed = json.loads(pass_path.read_text())
below = json.loads(below_path.read_text())
missing = json.loads(missing_path.read_text())
stale_stderr = stale_stderr_path.read_text()
over_limit_stderr = over_limit_stderr_path.read_text()
empty_stderr = empty_stderr_path.read_text()

def find_cell(output, cell):
    for row in output["cells"]:
        if row["cell"] == cell:
            return row
    raise AssertionError(f"missing cell row: {cell}")

below_cell = find_cell(below, "off_by_one::python")
missing_cell = find_cell(missing, "known_good::python")

scenario_by_name = {scenario["name"]: scenario for scenario in seed["scenarios"]}

assertions = {
    "seed_readback_equal": all(s["readback_equal"] for s in seed["scenarios"]),
    "seed_required_cell_count_is_8": seed["required_cell_count"] == 8,
    "seed_active_gate_is_python": seed["active_gate"] == "python_swebench_lite_300x8",
    "seed_cf_counts": all(
        (s["cf_count"] == 0 if s["name"] == "empty-db" else s["cf_count"] == 1)
        for s in seed["scenarios"]
    ),
    "pass_exit_zero": True,
    "below_exit_one": below_exit == 1,
    "missing_exit_one": missing_exit == 1,
    "stale_80_exit_one": stale_exit == 1,
    "over_limit_exit_one": over_limit_exit == 1,
    "empty_exit_one": empty_exit == 1,
    "pass_lists_all_8_python_cells": passed["requiredCellCount"] == 8 and len(passed["cells"]) == 8,
    "below_lists_all_8_python_cells": below["requiredCellCount"] == 8 and len(below["cells"]) == 8,
    "missing_lists_all_8_python_cells": missing["requiredCellCount"] == 8 and len(missing["cells"]) == 8,
    "pass_cells_python_only": all(row["language"] == "python" and row["cell"].endswith("::python") for row in passed["cells"]),
    "below_cells_python_only": all(row["language"] == "python" and row["cell"].endswith("::python") for row in below["cells"]),
    "missing_cells_python_only": all(row["language"] == "python" and row["cell"].endswith("::python") for row in missing["cells"]),
    "pass_counts": (
        passed["passed"] is True
        and passed["passingCellCount"] == 8
        and passed["belowThresholdCellCount"] == 0
        and passed["missingCellCount"] == 0
        and passed["failingCellCount"] == 0
    ),
    "below_counts": (
        below["passed"] is False
        and below["passingCellCount"] == 7
        and below["belowThresholdCellCount"] == 1
        and below["missingCellCount"] == 0
        and below["failingCellCount"] == 1
    ),
    "missing_counts": (
        missing["passed"] is False
        and missing["passingCellCount"] == 7
        and missing["belowThresholdCellCount"] == 0
        and missing["missingCellCount"] == 1
        and missing["failingCellCount"] == 1
    ),
    "below_cell_detail": (
        below_cell["status"] == "below_threshold"
        and abs(below_cell["correlation"] - 0.91) < 0.0001
        and 0.039 < below_cell["distanceToThreshold"] < 0.041
        and below_cell["holdoutCount"] == 2400
    ),
    "missing_cell_detail": (
        missing_cell["status"] == "missing"
        and missing_cell["correlation"] is None
        and missing_cell["distanceToThreshold"] is None
        and missing_cell["holdoutCount"] == 2400
    ),
    "source_of_truth_cf": (
        passed["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
        and below["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
        and missing["sourceOfTruth"]["column_family"] == "CF_MEJEPA_EVAL_REPORTS"
    ),
    "source_of_truth_active_gate_python": (
        passed["sourceOfTruth"]["active_gate"] == "python_swebench_lite_300x8"
        and below["sourceOfTruth"]["required_grid"] == "8 mutation categories x python"
        and missing["sourceOfTruth"]["required_grid"] == "8 mutation categories x python"
    ),
    "stale_80_seed_has_old_grid": scenario_by_name["stale-80-cell-grid"]["expected_required_cell_count"] == 8,
    "stale_80_error_is_fail_closed": (
        "MEJEPA_RUNBOOK_STALE_SHIP_GATE_GRID" in stale_stderr
        and "MEJEPA_ACTIVE_PYTHON_GATE_STALE_GRID" in stale_stderr
        and stale_stdout_path.stat().st_size == 0
    ),
    "over_limit_error_is_fail_closed": (
        "MEJEPA_RUNBOOK_STALE_SHIP_GATE_GRID" in over_limit_stderr
        and "MEJEPA_ACTIVE_PYTHON_GATE_STALE_GRID" in over_limit_stderr
        and over_limit_stdout_path.stat().st_size == 0
    ),
    "empty_db_error_is_fail_closed": (
        "MEJEPA_RUNBOOK_LATEST_REPORT_MISSING" in empty_stderr
        and empty_stdout_path.stat().st_size == 0
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
    "stale_80_error_sha256": hashlib.sha256(stale_stderr_path.read_bytes()).hexdigest(),
    "over_limit_error_sha256": hashlib.sha256(over_limit_stderr_path.read_bytes()).hexdigest(),
    "empty_db_error_sha256": hashlib.sha256(empty_stderr_path.read_bytes()).hexdigest(),
    "seed": seed,
    "command_outputs": {
        "pass": passed,
        "below_threshold": below,
        "missing_cell": missing,
        "stale_80_error": stale_stderr,
        "over_limit_error": over_limit_stderr,
        "empty_db_error": empty_stderr,
    },
    "exit_codes": {
        "pass": 0,
        "below_threshold": below_exit,
        "missing_cell": missing_exit,
        "stale_80": stale_exit,
        "over_limit": over_limit_exit,
        "empty_db": empty_exit,
    },
    "boundary_cases": [
        {"name": "happy_path", "trigger": "cell-status over 8 Python cells", "expected": "exit 0 and 8 python rows", "actual": {"exit": 0, "requiredCellCount": passed["requiredCellCount"], "rows": len(passed["cells"])}},
        {"name": "below_threshold", "trigger": "off_by_one::python correlation 0.91", "expected": "exit 1 with one below-threshold cell", "actual": {"exit": below_exit, "cell": below_cell}},
        {"name": "missing_correlation", "trigger": "known_good::python correlation null", "expected": "exit 1 with one missing cell", "actual": {"exit": missing_exit, "cell": missing_cell}},
        {"name": "stale_80_grid", "trigger": "80 non-Python cells", "expected": "fail closed before JSON status output", "actual": {"exit": stale_exit, "stderr": stale_stderr}},
        {"name": "over_limit_cell", "trigger": "9 cells including known_good::rust", "expected": "fail closed before JSON status output", "actual": {"exit": over_limit_exit, "stderr": over_limit_stderr}},
        {"name": "empty_db", "trigger": "no CF_MEJEPA_EVAL_REPORTS rows", "expected": "fail closed before JSON status output", "actual": {"exit": empty_exit, "stderr": empty_stderr}},
    ],
    "assertions": assertions,
    "all_passed": all(assertions.values()),
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
if not report["all_passed"]:
    raise SystemExit(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps(report, indent=2, sort_keys=True))
PY

sha256sum "$REPORT_JSON"
