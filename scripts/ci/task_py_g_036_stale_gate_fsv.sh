#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${CONTEXTGRAPH_TASK_PY_G_036_FSV_ROOT:-/zfs/archive/contextgraph/fsv/task-py-g-036-stale-gate-fsv}"
CELL_ROOT="$FSV_ROOT/cell-status"
DASHBOARD_ROOT="$FSV_ROOT/dashboard"
REPORT_JSON="$FSV_ROOT/task_py_g_036_stale_gate_fsv.json"

rm -rf "$FSV_ROOT"
mkdir -p "$FSV_ROOT"

cd "$REPO_ROOT"

CONTEXTGRAPH_CELL_STATUS_FSV_ROOT="$CELL_ROOT" bash scripts/ci/phase_g_cell_status_fsv.sh
CONTEXTGRAPH_SHIP_GATE_DASHBOARD_FSV_ROOT="$DASHBOARD_ROOT" bash scripts/ci/phase_g_ship_gate_dashboard_fsv.sh

python3 - "$CELL_ROOT/cell_status_fsv.json" "$DASHBOARD_ROOT/ship_gate_dashboard_fsv.json" "$REPORT_JSON" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

cell_report_path = Path(sys.argv[1])
dashboard_report_path = Path(sys.argv[2])
report_path = Path(sys.argv[3])

cell = json.loads(cell_report_path.read_text(encoding="utf-8"))
dashboard = json.loads(dashboard_report_path.read_text(encoding="utf-8"))

def assertion(report, name):
    return bool(report.get("assertions", {}).get(name))

assertions = {
    "cell_status_all_passed": cell.get("all_passed") is True,
    "dashboard_all_passed": dashboard.get("all_passed") is True,
    "cell_status_uses_8_python_cells": assertion(cell, "pass_lists_all_8_python_cells")
    and assertion(cell, "pass_cells_python_only"),
    "cell_status_rejects_stale_80": assertion(cell, "stale_80_error_is_fail_closed"),
    "cell_status_rejects_over_limit": assertion(cell, "over_limit_error_is_fail_closed"),
    "cell_status_rejects_empty_db": assertion(cell, "empty_db_error_is_fail_closed"),
    "dashboard_renders_only_python": assertion(dashboard, "synthetic_grid_has_8_python_rows")
    and assertion(dashboard, "markdown_rejects_non_python_cells")
    and assertion(dashboard, "html_rejects_non_python_cells"),
    "dashboard_rejects_stale_80": assertion(dashboard, "stale_dashboard_fails_closed"),
    "dashboard_rejects_over_limit": assertion(dashboard, "over_limit_dashboard_fails_closed"),
}

head = subprocess.check_output(["git", "rev-parse", "--short=8", "HEAD"], text=True).strip()
report = {
    "schema_version": 1,
    "task_id": "TASK-PY-G-036",
    "fsv_name": "task-py-g-036-stale-gate-fsv",
    "build_release_sha": head,
    "source_of_truth": {
        "eval_reports": "CF_MEJEPA_EVAL_REPORTS",
        "cell_status_fsv": str(cell_report_path),
        "dashboard_fsv": str(dashboard_report_path),
        "active_gate": "python_swebench_lite_300x8",
        "required_grid": "8 mutation categories x python",
    },
    "artifacts": {
        "cell_status_report": str(cell_report_path),
        "dashboard_report": str(dashboard_report_path),
    },
    "sha256": {
        "cell_status_report": hashlib.sha256(cell_report_path.read_bytes()).hexdigest(),
        "dashboard_report": hashlib.sha256(dashboard_report_path.read_bytes()).hexdigest(),
    },
    "boundary_cases": [
        "happy path: valid 8-cell Python gate status renders/returns source-backed rows",
        "empty input: no CF_MEJEPA_EVAL_REPORTS rows fails closed",
        "invalid metadata: stale 80-cell non-Python grid fails closed",
        "over-limit: 9th non-Python cell fails closed",
        "missing signal: one Python cell with null correlation returns explicit missing status",
        "below threshold: one Python cell below 0.95 returns explicit below_threshold status",
    ],
    "assertions": assertions,
    "all_passed": all(assertions.values()),
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
if not report["all_passed"]:
    raise SystemExit(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps(report, indent=2, sort_keys=True))
PY

sha256sum "$REPORT_JSON"
