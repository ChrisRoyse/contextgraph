#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FSV_ROOT="/mnt/d/contextgraph/fsv/phase-g-training-pipeline-orchestrator-fsv"
DB_PATH="$FSV_ROOT/infer-rocksdb"
PY_CORPUS="$FSV_ROOT/corpus-python"
NONPY_CORPUS="$FSV_ROOT/corpus-non-python"
OUTPUT_JSON="$FSV_ROOT/orchestrator-output.json"
REPORT_JSON="$FSV_ROOT/training_pipeline_orchestrator_fsv.json"
PY_REAL="/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json"
NONPY_REAL="/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/index.json"

rm -rf "$FSV_ROOT"
mkdir -p \
  "$PY_CORPUS/mutations/py__fixture-0001" \
  "$PY_CORPUS/mutations/py__fixture-0002" \
  "$NONPY_CORPUS/mutations/rust__fixture-0001" \
  "$NONPY_CORPUS/mutations/javascript__fixture-0002"

printf 'diff --git a/app.py b/app.py\n+print("known good")\n' \
  > "$PY_CORPUS/mutations/py__fixture-0001/known_good.patch"
printf 'diff --git a/app.py b/app.py\n+raise RuntimeError("oracle fail")\n' \
  > "$PY_CORPUS/mutations/py__fixture-0002/compile_error.patch"
printf 'fn main() { println!("known good"); }\n' \
  > "$NONPY_CORPUS/mutations/rust__fixture-0001/known_good.rs"
printf 'export function answer() { return 41 }\n' \
  > "$NONPY_CORPUS/mutations/javascript__fixture-0002/subtle_flip.js"

cat > "$PY_CORPUS/index.json" <<'JSON'
{
  "schema_version": 1,
  "corpus_version": "phase-g-fsv-python",
  "corpus_sha256": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
  "entries": [
    {
      "bucket": "train",
      "category": "known_good",
      "mutation_note": "FSV known-good Python row",
      "oracle_all_passed": true,
      "oracle_exception": null,
      "oracle_per_test_count": 2,
      "oracle_verdict_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "patch_path": "mutations/py__fixture-0001/known_good.patch",
      "patch_sha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "repo": "fixture/python",
      "task_id": "py__fixture-0001"
    },
    {
      "bucket": "train",
      "category": "compile_error",
      "mutation_note": "FSV failing Python row",
      "oracle_all_passed": false,
      "oracle_exception": "RuntimeError",
      "oracle_per_test_count": 1,
      "oracle_verdict_sha256": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
      "patch_path": "mutations/py__fixture-0002/compile_error.patch",
      "patch_sha256": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
      "repo": "fixture/python",
      "task_id": "py__fixture-0002"
    }
  ]
}
JSON

cat > "$NONPY_CORPUS/index.json" <<'JSON'
{
  "schema_version": 1,
  "corpus_version": "phase-g-fsv-non-python",
  "corpus_sha256": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
  "entries": [
    {
      "bucket": "train",
      "category": "known_good",
      "language": "rust",
      "mutation_note": "FSV known-good Rust row",
      "oracle_all_passed": true,
      "oracle_exception": null,
      "oracle_per_test_count": 1,
      "oracle_verdict_sha256": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      "patch_path": "mutations/rust__fixture-0001/known_good.rs",
      "patch_sha256": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      "repo": "fixture/rust",
      "task_id": "rust__fixture-0001"
    },
    {
      "bucket": "train",
      "category": "subtle_flip",
      "language": "java_script",
      "mutation_note": "FSV failing JavaScript row",
      "oracle_all_passed": false,
      "oracle_exception": null,
      "oracle_per_test_count": 1,
      "oracle_verdict_sha256": "sha256:9999999999999999999999999999999999999999999999999999999999999999",
      "patch_path": "mutations/javascript__fixture-0002/subtle_flip.js",
      "patch_sha256": "sha256:8888888888888888888888888888888888888888888888888888888888888888",
      "repo": "fixture/javascript",
      "task_id": "javascript__fixture-0002"
    }
  ]
}
JSON

cd "$REPO_ROOT"
cargo run -p context-graph-cli --bin context-graph-cli -- \
  mejepa train \
  --db-path "$DB_PATH" \
  --corpus "$PY_CORPUS" \
  --corpus "$NONPY_CORPUS" \
  --split train \
  > "$OUTPUT_JSON"

python3 - "$OUTPUT_JSON" "$REPORT_JSON" "$PY_REAL" "$NONPY_REAL" <<'PY'
import hashlib
import json
import subprocess
import sys
from pathlib import Path

output_path = Path(sys.argv[1])
report_path = Path(sys.argv[2])
py_real = Path(sys.argv[3])
nonpy_real = Path(sys.argv[4])

output = json.loads(output_path.read_text())

def real_count(path):
    if not path.exists():
        return None
    data = json.loads(path.read_text())
    return len(data["entries"])

assertions = {
    "input_task_count_is_4": output["inputTaskCount"] == 4,
    "panel_count_matches_input": output["outputPanelCount"] == output["inputTaskCount"],
    "dda_rows_match_input": output["ddaSignalRowCount"] == output["inputTaskCount"],
    "logical_dda_div_462_matches_input": (
        output["logicalDdaSignalCount"] // output["logicalDdaSignalsPerPanel"]
        == output["inputTaskCount"]
    ),
    "predictions_match_input": output["predictionCount"] == output["inputTaskCount"],
    "oracle_verdicts_match_input": output["oracleVerdictCount"] == output["inputTaskCount"],
    "train_certs_match_input": output["trainCertCount"] == output["inputTaskCount"],
    "readback_equal": output["readbackEqual"] is True,
    "active_embedder_forwards_12": output["activeContentEmbedderForwards"] == 12,
    "dda_surfaces_21": output["ddaEmbedderSurfaces"] == 21,
    "instrument_slots_15": output["instrumentSlotCount"] == 15,
    "panel_cf_is_canonical": output["sourceOfTruth"]["panel_cf"] == "CF_MEJEPA_PANELS",
}

head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
report = {
    "schema_version": 1,
    "build_release_sha": head,
    "fsv_name": "phase-g-training-pipeline-orchestrator-fsv",
    "command_output_sha256": hashlib.sha256(output_path.read_bytes()).hexdigest(),
    "synthetic_command_output": output,
    "real_corpus_expected_counts": {
        "python_300x8_entries": real_count(py_real),
        "non_python_fixture_entries": real_count(nonpy_real),
        "combined_entries": (
            real_count(py_real) + real_count(nonpy_real)
            if real_count(py_real) is not None and real_count(nonpy_real) is not None
            else None
        ),
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
