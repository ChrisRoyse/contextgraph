from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_execution_aware_lift_measurement import (
    CONTESTED_MIXED_CELLS,
    ExecutionAwareLiftMeasurementError,
    run_measurement,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def make_rows() -> tuple[list[dict], list[dict], list[dict]]:
    calibration: list[dict] = []
    holdout: list[dict] = []
    corpus: list[dict] = []
    for split, target in (("cal", calibration), ("hold", holdout)):
        for category in CONTESTED_MIXED_CELLS:
            for idx, (actual, base_score, line_count) in enumerate(
                (
                    (False, 0.45, 10),
                    (False, 0.48, 12),
                    (True, 0.52, 300),
                    (True, 0.55, 340),
                )
            ):
                task_id = f"org__repo-{split}-{category}-{idx}"
                row = {
                    "task_id": task_id,
                    "repo": "org/repo",
                    "category": category,
                    "predicted_oracle_pass": base_score,
                    "actual_oracle_pass": actual,
                    "patch_path": f"/zfs/archive/contextgraph/corpus/{task_id}/{category}.patch",
                    "patch_sha256": "sha256:" + ("a" * 64),
                    "_line_count": line_count,
                }
                target.append(row)
                corpus.append(row)
    return calibration, holdout, corpus


def write_ship_gate(path: Path, calibration: list[dict], holdout: list[dict]) -> None:
    def public(row: dict) -> dict:
        return {key: value for key, value in row.items() if not key.startswith("_")}

    write_json(
        path,
        {
            "execute_and_inspect": {
                "calibration_predictions": [public(row) for row in calibration],
                "holdout_predictions": [public(row) for row in holdout],
            }
        },
    )


def write_corpus(root: Path, rows: list[dict]) -> None:
    write_json(
        root / "index.json",
        {
            "entries": [
                {
                    "task_id": row["task_id"],
                    "repo": row["repo"],
                    "category": row["category"],
                    "patch_path": f"mutations/{row['task_id']}/{row['category']}.patch",
                }
                for row in rows
            ]
        },
    )


def write_trace(root: Path, row: dict, *, target_side: bool = False, constant: bool = False) -> None:
    line_count = 100 if constant else int(row["_line_count"])
    payload = {
        "schema_version": 1,
        "producer": "test.trace",
        "task_id": row["task_id"],
        "category": row["category"],
        "execution_outcome": "executed",
        "coverage": {
            "events_recorded": line_count * 3,
            "events_seen": line_count * 4,
            "lines_executed": line_count,
            "files_touched": 2 if line_count < 100 else 8,
            "truncated": False,
            "execution_outcome": "executed",
        },
        "runtime_metrics": {
            "elapsed_ms": line_count * 2,
            "pytest_exit_code": 0,
            "trace_stopped_at_max_events": False,
            "execution_outcome": "executed",
        },
        "steps": {"event_counts": {"call": 2, "line": line_count, "return": 2, "exception": 0}},
        "trace_events": [
            {"event": "line", "file": f"pkg/mod{idx % 3}.py", "function": f"fn{idx % 5}", "line": idx}
            for idx in range(min(line_count, 20))
        ],
        "line_coverage": [{"location": f"pkg/mod.py:{idx}", "count": 1} for idx in range(min(line_count, 20))],
        "variable_states": [{"event_index": idx, "local_names": ["x"]} for idx in range(min(line_count, 20))],
    }
    if target_side:
        payload["actual_oracle_pass"] = row["actual_oracle_pass"]
    write_json(root / row["task_id"] / f"{row['category']}.execution-trace.json", payload)


def run_fixture(tmp_path: Path, *, constant_trace: bool = False, target_side_trace: bool = False) -> dict:
    calibration, holdout, corpus = make_rows()
    ship_gate = tmp_path / "ship-gate-fsv.json"
    corpus_root = tmp_path / "corpus"
    trace_root = tmp_path / "traces"
    write_ship_gate(ship_gate, calibration, holdout)
    write_corpus(corpus_root, corpus)
    for row in corpus:
        write_trace(trace_root, row, constant=constant_trace, target_side=target_side_trace and row is corpus[0])
    return run_measurement(
        Namespace(
            ship_gate_report=ship_gate,
            corpus_root=corpus_root,
            trace_root=trace_root,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test-lift",
            expected_corpus_rows=len(corpus),
            expected_calibration_count=len(calibration),
            expected_holdout_count=len(holdout),
            ridge_alpha=0.01,
            min_correlation_lift=0.005,
            min_trace_coverage=0.95,
            allow_non_aiwonder_roots=True,
        )
    )


def test_trace_lift_is_measured_and_mirrored(tmp_path: Path):
    result = run_fixture(tmp_path)
    manifest_path = Path(result["manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    lift_rows = [
        json.loads(line)
        for line in Path(manifest["artifacts"]["lift_rows"]).read_text(encoding="utf-8").splitlines()
    ]
    state_manifest = Path(result["state_run_root"]) / manifest_path.name

    assert manifest["decision"]["proxy_feature_lift_measured"] is True
    assert manifest["decision"]["promotion_allowed"] is True
    assert manifest["trace_coverage"]["coverage_fraction"] == 1.0
    assert len(lift_rows) == len(CONTESTED_MIXED_CELLS)
    assert state_manifest.read_text(encoding="utf-8") == manifest_path.read_text(encoding="utf-8")
    assert manifest["modeling_policy"]["holdout_rows_used_for_fitting"] == 0
    assert "pytest_exit_code" in manifest["modeling_policy"]["direct_outcome_fields_excluded_from_live_feature_vector"]


def test_no_trace_signal_blocks_promotion(tmp_path: Path):
    result = run_fixture(tmp_path, constant_trace=True)
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["decision"]["proxy_feature_lift_measured"] is True
    assert manifest["decision"]["promotion_allowed"] is False
    assert manifest["decision"]["promotion_blockers"]


def test_target_side_key_in_trace_fails_closed(tmp_path: Path):
    with pytest.raises(ExecutionAwareLiftMeasurementError, match="EXECUTION_AWARE_LIFT_TARGET_SIDE_KEY_IN_TRACE_PAYLOAD"):
        run_fixture(tmp_path, target_side_trace=True)


def test_durable_roots_require_aiwonder_without_escape(tmp_path: Path):
    calibration, holdout, corpus = make_rows()
    ship_gate = tmp_path / "ship-gate-fsv.json"
    corpus_root = tmp_path / "corpus"
    trace_root = tmp_path / "traces"
    write_ship_gate(ship_gate, calibration, holdout)
    write_corpus(corpus_root, corpus)
    for row in corpus:
        write_trace(trace_root, row)

    with pytest.raises(ExecutionAwareLiftMeasurementError, match="EXECUTION_AWARE_LIFT_NON_AIWONDER_DURABLE_PATH"):
        run_measurement(
            Namespace(
                ship_gate_report=ship_gate,
                corpus_root=corpus_root,
                trace_root=trace_root,
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/execution-aware-audit/inv-010-v1"),
                run_id="run-test-lift",
                expected_corpus_rows=len(corpus),
                expected_calibration_count=len(calibration),
                expected_holdout_count=len(holdout),
                ridge_alpha=0.01,
                min_correlation_lift=0.005,
                min_trace_coverage=0.95,
                allow_non_aiwonder_roots=False,
            )
        )
