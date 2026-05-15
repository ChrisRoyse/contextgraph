#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${CONTEXTGRAPH_SHIP_GATE_DASHBOARD_FSV_ROOT:-/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv}"
INPUT_JSON="$FSV_ROOT/synthetic_cell_status.json"
STALE_JSON="$FSV_ROOT/stale_80_cell_status.json"
STALE_STDOUT="$FSV_ROOT/stale_80_dashboard_stdout.json"
STALE_STDERR="$FSV_ROOT/stale_80_dashboard_error.txt"
OVER_LIMIT_JSON="$FSV_ROOT/over_limit_cell_status.json"
OVER_LIMIT_STDOUT="$FSV_ROOT/over_limit_dashboard_stdout.json"
OVER_LIMIT_STDERR="$FSV_ROOT/over_limit_dashboard_error.txt"
PREDICTIONS_DIR="$FSV_ROOT/failing-predictions"
MARKDOWN_OUT="$FSV_ROOT/ship_gate_dashboard.md"
HTML_OUT="$FSV_ROOT/ship_gate_dashboard.html"
REPORT_JSON="$FSV_ROOT/ship_gate_dashboard_fsv.json"

rm -rf "$FSV_ROOT"
mkdir -p "$PREDICTIONS_DIR"

cd "$REPO_ROOT"
python3 -m py_compile scripts/render_ship_gate_dashboard.py

python3 - "$INPUT_JSON" "$STALE_JSON" "$OVER_LIMIT_JSON" "$PREDICTIONS_DIR" <<'PY'
import json
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
stale_path = Path(sys.argv[2])
over_limit_path = Path(sys.argv[3])
predictions_dir = Path(sys.argv[4])

categories = [
    "known_good",
    "subtle_flip",
    "off_by_one",
    "swap_variable",
    "delete_test_call",
    "wrong_file",
    "over_engineer",
    "compile_error",
]
active_languages = ["python"]
stale_languages = [
    "rust",
    "javascript",
    "typescript",
    "go",
    "java",
    "c",
    "cpp",
    "c_sharp",
    "ruby",
    "php",
]

rows = []
for category_index, category in enumerate(categories):
    for language_index, language in enumerate(active_languages):
        cell = f"{category}::{language}"
        correlation = 0.971
        status = "pass"
        if cell == "off_by_one::python":
            correlation = 0.912
            status = "below_threshold"
        elif cell == "compile_error::python":
            correlation = None
            status = "missing"
        rows.append(
            {
                "cell": cell,
                "mutationCategory": category,
                "language": language,
                "correlation": correlation,
                "holdoutCount": 2400,
                "distanceToThreshold": None if correlation is None else max(0.95 - correlation, 0.0),
                "lastUpdateUnixMs": 1778680800000 + category_index * 1000 + language_index,
                "status": status,
            }
        )
        safe = cell.replace("::", "__")
        (predictions_dir / f"{safe}.md").write_text(
            f"# Representative failing predictions for {cell}\n\n"
            f"- prediction_id: synthetic-{safe}-001\n"
            f"- expected_status: {status}\n",
            encoding="utf-8",
)

payload = {
    "latestReportDate": "2026-05-15-task-py-g-036-dashboard-fsv",
    "generatedAtUnixMs": 1778680800000,
    "threshold": 0.95,
    "activeGate": "python_swebench_lite_300x8",
    "requiredCellCount": 8,
    "observedCellCount": 7,
    "passingCellCount": 6,
    "belowThresholdCellCount": 1,
    "missingCellCount": 1,
    "failingCellCount": 2,
    "passed": False,
    "cells": rows,
    "sourceOfTruth": {
        "db_path": "/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv/synthetic-db",
        "column_family": "CF_MEJEPA_EVAL_REPORTS",
        "selector": "load_latest_report()",
        "required_grid": "8 mutation categories x python",
        "threshold": 0.95,
    },
}
input_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

stale_rows = []
for category_index, category in enumerate(categories):
    for language_index, language in enumerate(stale_languages):
        cell = f"{category}::{language}"
        stale_rows.append(
            {
                "cell": cell,
                "mutationCategory": category,
                "language": language,
                "correlation": 0.971,
                "holdoutCount": 800 + category_index * 10 + language_index,
                "distanceToThreshold": 0.0,
                "lastUpdateUnixMs": 1778680800000 + category_index * 1000 + language_index,
                "status": "pass",
            }
        )
stale_payload = {
    "latestReportDate": "2026-05-15-task-py-g-036-stale-dashboard-fsv",
    "generatedAtUnixMs": 1778680800000,
    "threshold": 0.95,
    "requiredCellCount": 80,
    "observedCellCount": 80,
    "passingCellCount": 80,
    "belowThresholdCellCount": 0,
    "missingCellCount": 0,
    "failingCellCount": 0,
    "passed": True,
    "cells": stale_rows,
    "sourceOfTruth": {
        "db_path": "/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv/stale-db",
        "column_family": "CF_MEJEPA_EVAL_REPORTS",
        "selector": "load_latest_report()",
        "required_grid": "8 mutation categories x 10 non-Python fixture languages",
        "threshold": 0.95,
    },
}
stale_path.write_text(json.dumps(stale_payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

over_limit_payload = dict(payload)
over_limit_rows = list(rows)
over_limit_rows.append(
    {
        "cell": "known_good::rust",
        "mutationCategory": "known_good",
        "language": "rust",
        "correlation": 0.971,
        "holdoutCount": 2400,
        "distanceToThreshold": 0.0,
        "lastUpdateUnixMs": 1778680809999,
        "status": "pass",
    }
)
over_limit_payload["cells"] = over_limit_rows
over_limit_payload["observedCellCount"] = 8
over_limit_path.write_text(
    json.dumps(over_limit_payload, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PY

python3 scripts/render_ship_gate_dashboard.py \
  --input "$INPUT_JSON" \
  --output "$MARKDOWN_OUT" \
  --format markdown \
  --failing-predictions-dir "$PREDICTIONS_DIR"

python3 scripts/render_ship_gate_dashboard.py \
  --input "$INPUT_JSON" \
  --output "$HTML_OUT" \
  --format html \
  --failing-predictions-dir "$PREDICTIONS_DIR"

stale_exit=0
python3 scripts/render_ship_gate_dashboard.py \
  --input "$STALE_JSON" \
  --output "$FSV_ROOT/stale_80_dashboard.md" \
  --format markdown \
  --failing-predictions-dir "$PREDICTIONS_DIR" > "$STALE_STDOUT" 2> "$STALE_STDERR" || stale_exit=$?

over_limit_exit=0
python3 scripts/render_ship_gate_dashboard.py \
  --input "$OVER_LIMIT_JSON" \
  --output "$FSV_ROOT/over_limit_dashboard.md" \
  --format markdown \
  --failing-predictions-dir "$PREDICTIONS_DIR" > "$OVER_LIMIT_STDOUT" 2> "$OVER_LIMIT_STDERR" || over_limit_exit=$?

python3 - \
  "$INPUT_JSON" \
  "$STALE_JSON" \
  "$OVER_LIMIT_JSON" \
  "$MARKDOWN_OUT" \
  "$HTML_OUT" \
  "$PREDICTIONS_DIR" \
  "$STALE_STDOUT" \
  "$STALE_STDERR" \
  "$OVER_LIMIT_STDOUT" \
  "$OVER_LIMIT_STDERR" \
  "$REPORT_JSON" \
  "$stale_exit" \
  "$over_limit_exit" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
stale_path = Path(sys.argv[2])
over_limit_path = Path(sys.argv[3])
markdown_path = Path(sys.argv[4])
html_path = Path(sys.argv[5])
predictions_dir = Path(sys.argv[6])
stale_stdout_path = Path(sys.argv[7])
stale_stderr_path = Path(sys.argv[8])
over_limit_stdout_path = Path(sys.argv[9])
over_limit_stderr_path = Path(sys.argv[10])
report_path = Path(sys.argv[11])
stale_exit = int(sys.argv[12])
over_limit_exit = int(sys.argv[13])

payload = json.loads(input_path.read_text(encoding="utf-8"))
stale_payload = json.loads(stale_path.read_text(encoding="utf-8"))
over_limit_payload = json.loads(over_limit_path.read_text(encoding="utf-8"))
markdown = markdown_path.read_text(encoding="utf-8")
html = html_path.read_text(encoding="utf-8")
cells = [row["cell"] for row in payload["cells"]]
stale_stderr = stale_stderr_path.read_text(encoding="utf-8")
over_limit_stderr = over_limit_stderr_path.read_text(encoding="utf-8")

def contains_all_cells(text):
    return all(cell in text for cell in cells)

def safe(cell):
    return cell.replace("::", "__")

assertions = {
    "synthetic_grid_has_8_python_rows": len(cells) == 8 and len(set(cells)) == 8,
    "synthetic_grid_python_only": all(cell.endswith("::python") for cell in cells),
    "markdown_exists": markdown_path.is_file() and markdown_path.stat().st_size > 0,
    "html_exists": html_path.is_file() and html_path.stat().st_size > 0,
    "markdown_contains_all_8_python_cells": contains_all_cells(markdown),
    "html_contains_all_8_python_cells": contains_all_cells(html),
    "markdown_rejects_non_python_cells": "::rust" not in markdown and "::php" not in markdown,
    "html_rejects_non_python_cells": "::rust" not in html and "::php" not in html,
    "markdown_heatmap_contains_expected_low_cell": "| `off_by_one` |" in markdown and "0.912 below_threshold" in markdown,
    "markdown_heatmap_contains_missing_cell": "| `compile_error` |" in markdown and "missing" in markdown,
    "markdown_has_expected_holdout_count": "`off_by_one::python` | `off_by_one` | `python` | 0.912 | 2400 |" in markdown,
    "markdown_has_expected_last_update": "1778680802000" in markdown,
    "markdown_links_representative_prediction": f"failing-predictions/{safe('off_by_one::python')}.md" in markdown,
    "html_links_representative_prediction": f"failing-predictions/{safe('compile_error::python')}.md" in html,
    "prediction_drilldown_files_for_every_cell": all((predictions_dir / f"{safe(cell)}.md").is_file() for cell in cells),
    "source_of_truth_rendered": "CF_MEJEPA_EVAL_REPORTS" in markdown and "CF_MEJEPA_EVAL_REPORTS" in html,
    "active_gate_metadata_python": payload["activeGate"] == "python_swebench_lite_300x8" and payload["requiredCellCount"] == 8,
    "stale_input_is_80_non_python_rows": len(stale_payload["cells"]) == 80 and stale_payload["requiredCellCount"] == 80,
    "stale_dashboard_fails_closed": (
        stale_exit == 1
        and "MEJEPA_SHIP_GATE_DASHBOARD_STALE_REQUIRED_CELL_COUNT" in stale_stderr
        and stale_stdout_path.stat().st_size == 0
    ),
    "over_limit_input_has_extra_non_python_row": len(over_limit_payload["cells"]) == 9 and any(row["cell"] == "known_good::rust" for row in over_limit_payload["cells"]),
    "over_limit_dashboard_fails_closed": (
        over_limit_exit == 1
        and "MEJEPA_SHIP_GATE_DASHBOARD_STALE_GATE_LANGUAGE" in over_limit_stderr
        and over_limit_stdout_path.stat().st_size == 0
    ),
}

head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
report = {
    "schema_version": 1,
    "task_id": "TASK-G-008",
    "fsv_name": "phase-g-ship-gate-dashboard-fsv",
    "build_release_sha": head,
    "source_of_truth": {
        "input": "synthetic context-graph-cli mejepa cell-status JSON",
        "backing_store": "CF_MEJEPA_EVAL_REPORTS",
        "prediction_drilldown_dir": str(predictions_dir),
        "active_gate": "python_swebench_lite_300x8",
        "required_grid": "8 mutation categories x python",
    },
    "artifacts": {
        "input_json": str(input_path),
        "stale_json": str(stale_path),
        "over_limit_json": str(over_limit_path),
        "markdown": str(markdown_path),
        "html": str(html_path),
        "stale_stderr": str(stale_stderr_path),
        "over_limit_stderr": str(over_limit_stderr_path),
    },
    "sha256": {
        "input_json": hashlib.sha256(input_path.read_bytes()).hexdigest(),
        "stale_json": hashlib.sha256(stale_path.read_bytes()).hexdigest(),
        "over_limit_json": hashlib.sha256(over_limit_path.read_bytes()).hexdigest(),
        "markdown": hashlib.sha256(markdown_path.read_bytes()).hexdigest(),
        "html": hashlib.sha256(html_path.read_bytes()).hexdigest(),
        "stale_stderr": hashlib.sha256(stale_stderr_path.read_bytes()).hexdigest(),
        "over_limit_stderr": hashlib.sha256(over_limit_stderr_path.read_bytes()).hexdigest(),
    },
    "boundary_cases": [
        {"name": "happy_path", "trigger": "render 8 Python cell-status rows", "expected": "markdown/html render all and only Python cells", "actual": {"cell_count": len(cells), "markdown_bytes": markdown_path.stat().st_size, "html_bytes": html_path.stat().st_size}},
        {"name": "stale_80_grid", "trigger": "render 80 non-Python fixture rows", "expected": "fail closed before output", "actual": {"exit": stale_exit, "stderr": stale_stderr}},
        {"name": "over_limit_non_python_cell", "trigger": "render 9 rows including known_good::rust", "expected": "fail closed before output", "actual": {"exit": over_limit_exit, "stderr": over_limit_stderr}},
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
