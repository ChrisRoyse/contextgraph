#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="${FSV_ROOT:-/zfs/archive/contextgraph/fsv/phase-g-label-quality-fsv}"
DB_PATH="$FSV_ROOT/rocksdb"
EMPTY_DB_PATH="$FSV_ROOT/empty-rocksdb"
EXPORT_DIR="${EXPORT_DIR:-/zfs/archive/contextgraph/exports/eval/2026-05-13-label-quality-fsv}"
MARKDOWN_OUT="$EXPORT_DIR/label_quality.md"
SUMMARY_JSON="$FSV_ROOT/label_quality_summary.json"
EMPTY_MARKDOWN_OUT="$FSV_ROOT/empty_label_quality.md"
EMPTY_SUMMARY_JSON="$FSV_ROOT/empty_label_quality_summary.json"
REPORT_JSON="$FSV_ROOT/label_quality_fsv.json"

rm -rf "$FSV_ROOT" "$EXPORT_DIR"
mkdir -p "$FSV_ROOT" "$EXPORT_DIR"

cd "$REPO_ROOT"
python3 -m py_compile scripts/audit_label_quality.py

ldb --db="$DB_PATH" --create_if_missing put __default__ init >/dev/null
ldb --db="$DB_PATH" create_column_family CF_MEJEPA_FAILURE_EXEMPLARS >/dev/null
ldb --db="$EMPTY_DB_PATH" --create_if_missing put __default__ init >/dev/null
ldb --db="$EMPTY_DB_PATH" create_column_family CF_MEJEPA_FAILURE_EXEMPLARS >/dev/null

python3 - "$DB_PATH" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

db = Path(sys.argv[1])
cf = "CF_MEJEPA_FAILURE_EXEMPLARS"

def row(task_id, outcomes, exception=None, language=None):
    payload = {
        "task_id": task_id,
        "mutation_kind": "subtle_flip",
        "chunk_id": f"{task_id}::chunk",
        "oracle_verdict": {
            "per_test": [
                {"test_id": f"{task_id}::test::{idx}", "outcome": outcome, "runtime_ms": 10 + idx}
                for idx, outcome in enumerate(outcomes)
            ],
            "exception": exception,
        },
        "witness_hash": [1] * 32,
    }
    if language is not None:
        payload["language"] = language
    return payload

buckets = {
    "python-bucket": [
        row("python::quality-row-1", ["pass", "fail"]),
        row("python::quality-row-2", ["pass"]),
    ],
    "rust-bucket": [
        row("rust::quality-row-1", [], "compile_error"),
        row("rust::quality-row-2", []),
    ],
    "unknown-bucket": [
        row("repo__ambiguous-1", ["skip"]),
    ],
}

for key, value in buckets.items():
    compact = json.dumps(value, separators=(",", ":"), sort_keys=True)
    subprocess.run(
        ["ldb", f"--db={db}", f"--column_family={cf}", "put", key, compact],
        check=True,
    )
subprocess.run(["ldb", f"--db={db}", f"--column_family={cf}", "compact"], check=True)
PY

python3 scripts/audit_label_quality.py \
  --db "$DB_PATH" \
  --date 2026-05-13-label-quality-fsv \
  --output "$MARKDOWN_OUT" \
  --json-output "$SUMMARY_JSON"

python3 scripts/audit_label_quality.py \
  --db "$EMPTY_DB_PATH" \
  --date 2026-05-13-label-quality-empty-fsv \
  --output "$EMPTY_MARKDOWN_OUT" \
  --json-output "$EMPTY_SUMMARY_JSON"

python3 - "$SUMMARY_JSON" "$MARKDOWN_OUT" "$EMPTY_SUMMARY_JSON" "$EMPTY_MARKDOWN_OUT" "$REPORT_JSON" "$DB_PATH" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

summary_path = Path(sys.argv[1])
markdown_path = Path(sys.argv[2])
empty_summary_path = Path(sys.argv[3])
empty_markdown_path = Path(sys.argv[4])
report_path = Path(sys.argv[5])
db_path = Path(sys.argv[6])

summary = json.loads(summary_path.read_text(encoding="utf-8"))
markdown = markdown_path.read_text(encoding="utf-8")
empty_summary = json.loads(empty_summary_path.read_text(encoding="utf-8"))
empty_markdown = empty_markdown_path.read_text(encoding="utf-8")
languages = {row["language"]: row for row in summary["languages"]}

assertions = {
    "source_cf_is_failure_exemplars": summary["sourceOfTruth"]["columnFamily"] == "CF_MEJEPA_FAILURE_EXEMPLARS",
    "synthetic_cf_bucket_count": summary["cfBucketCount"] == 3,
    "python_stats_match_expected": (
        languages["python"]["rows"] == 2
        and languages["python"]["cfBucketCount"] == 1
        and languages["python"]["labelsPerRowMean"] == 1.5
        and languages["python"]["labelsPerRowMin"] == 1
        and languages["python"]["labelsPerRowMax"] == 2
        and languages["python"]["nonTrivialSignalRatio"] == 0.5
        and languages["python"]["unavailableSignalRows"] == 0
    ),
    "rust_unavailable_boundary_match_expected": (
        languages["rust"]["rows"] == 2
        and languages["rust"]["labelsPerRowMean"] == 0.5
        and languages["rust"]["labelsPerRowMin"] == 0
        and languages["rust"]["labelsPerRowMax"] == 1
        and languages["rust"]["nonTrivialSignalRatio"] == 0.5
        and languages["rust"]["unavailableSignalRows"] == 1
    ),
    "unknown_language_boundary_match_expected": (
        languages["unknown"]["rows"] == 1
        and languages["unknown"]["nonTrivialSignalRatio"] == 1.0
        and languages["unknown"]["unavailableSignalRows"] == 0
    ),
    "markdown_contains_expected_rows": (
        "| `python` | 1 | 2 | 1.500 | 1 | 2 | 0.500 | 0 |" in markdown
        and "| `rust` | 1 | 2 | 0.500 | 0 | 1 | 0.500 | 1 |" in markdown
        and "| `unknown` | 1 | 1 | 1.000 | 1 | 1 | 1.000 | 0 |" in markdown
    ),
    "empty_cf_boundary_is_zero": (
        empty_summary["cfBucketCount"] == 0
        and empty_summary["totalRows"] == 0
        and "| `unknown` | 0 | 0 | 0.000 | 0 | 0 | 0.000 | 0 |" in empty_markdown
    ),
    "no_malformed_rows": summary["malformedRows"] == [] and empty_summary["malformedRows"] == [],
}

sst_files = sorted(str(path) for path in db_path.rglob("*.sst"))
head = subprocess.check_output(["git", "rev-parse", "--short=8", "HEAD"], text=True).strip()
report = {
    "schema_version": 1,
    "task_id": "TASK-G-015",
    "fsv_name": "phase-g-label-quality-fsv",
    "build_release_sha": head,
    "source_of_truth": {
        "rocksdb": str(db_path),
        "column_family": "CF_MEJEPA_FAILURE_EXEMPLARS",
        "reader": "scripts/audit_label_quality.py via ldb scan --hex",
    },
    "happy_path": [
        {
            "case": "synthetic_failure_exemplars_label_quality",
            "sot": "CF_MEJEPA_FAILURE_EXEMPLARS",
            "before": {"cf_rows": 0},
            "after": {"cf_rows": summary["cfBucketCount"], "total_rows": summary["totalRows"]},
            "expected": {"python_mean": 1.5, "rust_unavailable_rows": 1, "unknown_rows": 1},
            "actual": summary,
            "pass": all(assertions.values()),
            "evidence_path": str(markdown_path),
        }
    ],
    "boundary_cases": [
        {"case": "unavailable_signal_row", "expected": 1, "actual": languages["rust"]["unavailableSignalRows"], "pass": assertions["rust_unavailable_boundary_match_expected"]},
        {"case": "unknown_language_row", "expected": 1, "actual": languages["unknown"]["rows"], "pass": assertions["unknown_language_boundary_match_expected"]},
        {"case": "empty_cf", "expected": 0, "actual": empty_summary["totalRows"], "pass": assertions["empty_cf_boundary_is_zero"]},
    ],
    "cf_counts_before": {"CF_MEJEPA_FAILURE_EXEMPLARS": 0},
    "cf_counts_after": {"CF_MEJEPA_FAILURE_EXEMPLARS": summary["cfBucketCount"]},
    "readback_equal": True,
    "physical_artifacts": {
        "markdown": str(markdown_path),
        "summary_json": str(summary_path),
        "empty_markdown": str(empty_markdown_path),
        "rocksdb_sst_files": sst_files,
    },
    "assertions": assertions,
    "all_passed": all(assertions.values()) and bool(sst_files),
}
report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
if not report["all_passed"]:
    raise SystemExit(json.dumps(report, indent=2, sort_keys=True))
print(json.dumps(report, indent=2, sort_keys=True))
PY

sha256sum "$REPORT_JSON"
