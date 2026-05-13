#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv"
INPUT_JSON="$FSV_ROOT/synthetic_cell_status.json"
PREDICTIONS_DIR="$FSV_ROOT/failing-predictions"
MARKDOWN_OUT="$FSV_ROOT/ship_gate_dashboard.md"
HTML_OUT="$FSV_ROOT/ship_gate_dashboard.html"
REPORT_JSON="$FSV_ROOT/ship_gate_dashboard_fsv.json"

rm -rf "$FSV_ROOT"
mkdir -p "$PREDICTIONS_DIR"

cd "$REPO_ROOT"
python3 -m py_compile scripts/render_ship_gate_dashboard.py

python3 - "$INPUT_JSON" "$PREDICTIONS_DIR" <<'PY'
import json
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
predictions_dir = Path(sys.argv[2])

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
languages = [
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
    for language_index, language in enumerate(languages):
        cell = f"{category}::{language}"
        correlation = 0.971
        status = "pass"
        if cell == "off_by_one::rust":
            correlation = 0.912
            status = "below_threshold"
        elif cell == "compile_error::php":
            correlation = None
            status = "missing"
        rows.append(
            {
                "cell": cell,
                "mutationCategory": category,
                "language": language,
                "correlation": correlation,
                "holdoutCount": 800 + category_index * 10 + language_index,
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
    "latestReportDate": "2026-05-13-phase-g-dashboard-fsv",
    "generatedAtUnixMs": 1778680800000,
    "threshold": 0.95,
    "requiredCellCount": 80,
    "observedCellCount": 79,
    "passingCellCount": 78,
    "belowThresholdCellCount": 1,
    "missingCellCount": 1,
    "failingCellCount": 2,
    "passed": False,
    "cells": rows,
    "sourceOfTruth": {
        "db_path": "/mnt/d/contextgraph/fsv/phase-g-ship-gate-dashboard-fsv/synthetic-db",
        "column_family": "CF_MEJEPA_EVAL_REPORTS",
        "selector": "load_latest_report()",
        "required_grid": "8 mutation categories x 10 non-Python fixture languages",
        "threshold": 0.95,
    },
}
input_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
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

python3 - "$INPUT_JSON" "$MARKDOWN_OUT" "$HTML_OUT" "$PREDICTIONS_DIR" "$REPORT_JSON" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

input_path = Path(sys.argv[1])
markdown_path = Path(sys.argv[2])
html_path = Path(sys.argv[3])
predictions_dir = Path(sys.argv[4])
report_path = Path(sys.argv[5])

payload = json.loads(input_path.read_text(encoding="utf-8"))
markdown = markdown_path.read_text(encoding="utf-8")
html = html_path.read_text(encoding="utf-8")
cells = [row["cell"] for row in payload["cells"]]

def contains_all_cells(text):
    return all(cell in text for cell in cells)

def safe(cell):
    return cell.replace("::", "__")

assertions = {
    "synthetic_grid_has_80_rows": len(cells) == 80 and len(set(cells)) == 80,
    "markdown_exists": markdown_path.is_file() and markdown_path.stat().st_size > 0,
    "html_exists": html_path.is_file() and html_path.stat().st_size > 0,
    "markdown_contains_all_80_cells": contains_all_cells(markdown),
    "html_contains_all_80_cells": contains_all_cells(html),
    "markdown_heatmap_contains_expected_low_cell": "| `off_by_one` |" in markdown and "0.912 below_threshold" in markdown,
    "markdown_heatmap_contains_missing_cell": "| `compile_error` |" in markdown and "missing" in markdown,
    "markdown_has_expected_holdout_count": "`off_by_one::rust` | `off_by_one` | `rust` | 0.912 | 820 |" in markdown,
    "markdown_has_expected_last_update": "1778680802000" in markdown,
    "markdown_links_representative_prediction": f"failing-predictions/{safe('off_by_one::rust')}.md" in markdown,
    "html_links_representative_prediction": f"failing-predictions/{safe('compile_error::php')}.md" in html,
    "prediction_drilldown_files_for_every_cell": all((predictions_dir / f"{safe(cell)}.md").is_file() for cell in cells),
    "source_of_truth_rendered": "CF_MEJEPA_EVAL_REPORTS" in markdown and "CF_MEJEPA_EVAL_REPORTS" in html,
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
    },
    "artifacts": {
        "input_json": str(input_path),
        "markdown": str(markdown_path),
        "html": str(html_path),
    },
    "sha256": {
        "input_json": hashlib.sha256(input_path.read_bytes()).hexdigest(),
        "markdown": hashlib.sha256(markdown_path.read_bytes()).hexdigest(),
        "html": hashlib.sha256(html_path.read_bytes()).hexdigest(),
    },
    "assertions": assertions,
    "all_passed": all(assertions.values()),
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
if not report["all_passed"]:
    raise SystemExit(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps(report, indent=2, sort_keys=True))
PY

sha256sum "$REPORT_JSON"
