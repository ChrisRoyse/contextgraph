from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_temporal_goodhart_audit import (
    TemporalGoodhartAuditError,
    run_audit,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def row(task_id: str, repo: str, score: float, actual: bool, category: str = "off_by_one") -> dict:
    return {
        "task_id": task_id,
        "repo": repo,
        "category": category,
        "predicted_oracle_pass": score,
        "actual_oracle_pass": actual,
    }


def report(calibration_rows: list[dict], holdout_rows: list[dict]) -> dict:
    return {
        "execute_and_inspect": {
            "calibration_predictions": calibration_rows,
            "holdout_raw_predictions": holdout_rows,
        }
    }


def write_tasks(root: Path, count: int = 12) -> list[str]:
    task_ids = []
    for idx in range(count):
        task_id = f"org__repo-{idx:02d}"
        task_ids.append(task_id)
        write_json(root / f"{task_id}.json", {"instance_id": task_id, "repo": "org/repo"})
    return task_ids


def write_metadata(path: Path, task_ids: list[str], *, omit: str | None = None) -> None:
    tasks = []
    for idx, task_id in enumerate(task_ids):
        if task_id == omit:
            continue
        year = 2018 + (idx % 6)
        tasks.append(
            {
                "task_id": task_id,
                "repo": "org/repo",
                "created_at": f"{year}-01-01T00:00:00Z",
                "source_url": f"https://github.com/org/repo/issues/{idx}",
            }
        )
    write_json(path, {"schema_version": 1, "tasks": tasks})


def run_fixture(
    tmp_path: Path,
    *,
    calibration_rows: list[dict],
    holdout_rows: list[dict],
    metadata_omit: str | None = None,
    min_years: int = 4,
) -> dict:
    tasks_dir = tmp_path / "tasks"
    task_ids = write_tasks(tasks_dir)
    metadata_path = tmp_path / "metadata.json"
    write_metadata(metadata_path, task_ids, omit=metadata_omit)
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(fsv, report(calibration_rows, holdout_rows))
    return run_audit(
        Namespace(
            ship_gate_report=fsv,
            task_metadata_json=metadata_path,
            tasks_dir=tasks_dir,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_task_count=12,
            newest_task_count=3,
            temporal_holdout_task_count=4,
            min_years=min_years,
            drop_threshold=0.05,
            severe_drop_threshold=0.10,
            allow_non_aiwonder_roots=True,
        )
    )


def test_candidate_temporal_holdout_missing_from_current_fsv_requires_full_eval(tmp_path: Path):
    calibration = [row("org__repo-00", "org/repo", 0.9, True), row("org__repo-01", "org/repo", 0.1, False)]
    holdout = [
        row("org__repo-00", "org/repo", 0.9, True),
        row("org__repo-00", "org/repo", 0.1, False),
        row("org__repo-01", "org/repo", 0.8, True),
        row("org__repo-01", "org/repo", 0.2, False),
    ]
    result = run_fixture(tmp_path, calibration_rows=calibration, holdout_rows=holdout)
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["decision"]["full_temporal_disjoint_evaluation_required"] is True
    assert manifest["decision"]["canonical_full_execution_issue"] == "#769"
    assert "candidate_temporal_holdout_not_covered_by_current_holdout" in manifest["decision"]["decision_reasons"]
    assert manifest["temporal_candidate_split"]["candidate_holdout_tasks_missing_from_current_holdout_count"] > 0


def test_newest_window_drop_flags_temporal_risk(tmp_path: Path):
    calibration = [row("org__repo-00", "org/repo", 0.9, True), row("org__repo-01", "org/repo", 0.1, False)]
    holdout = []
    for idx in range(6):
        task_id = f"org__repo-{idx:02d}"
        if idx >= 3:
            holdout.extend([row(task_id, "org/repo", 0.1, True), row(task_id, "org/repo", 0.9, False)])
        else:
            holdout.extend([row(task_id, "org/repo", 0.9, True), row(task_id, "org/repo", 0.1, False)])
    result = run_fixture(tmp_path, calibration_rows=calibration, holdout_rows=holdout, min_years=2)
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["metrics"]["newest_current_holdout_drop"] > 0.05
    assert manifest["decision"]["temporal_goodhart_risk_detected_on_current_fsv"] is True
    assert "newest_current_holdout_window_drops_correlation" in manifest["decision"]["decision_reasons"]


def test_metadata_missing_for_report_task_fails_closed(tmp_path: Path):
    calibration = [row("org__repo-00", "org/repo", 0.9, True)]
    holdout = [row("org__repo-01", "org/repo", 0.1, False), row("org__repo-01", "org/repo", 0.9, True)]
    with pytest.raises(TemporalGoodhartAuditError, match="TEMPORAL_GOODHART_AUDIT_METADATA_INCOMPLETE"):
        run_fixture(
            tmp_path,
            calibration_rows=calibration,
            holdout_rows=holdout,
            metadata_omit="org__repo-01",
        )


def test_bad_created_at_fails_closed(tmp_path: Path):
    tasks_dir = tmp_path / "tasks"
    task_ids = write_tasks(tasks_dir)
    metadata_path = tmp_path / "metadata.json"
    write_metadata(metadata_path, task_ids)
    doc = json.loads(metadata_path.read_text(encoding="utf-8"))
    doc["tasks"][0]["created_at"] = "not-a-date"
    write_json(metadata_path, doc)
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(
        fsv,
        report(
            [row("org__repo-00", "org/repo", 0.9, True)],
            [row("org__repo-01", "org/repo", 0.1, False), row("org__repo-01", "org/repo", 0.9, True)],
        ),
    )
    with pytest.raises(TemporalGoodhartAuditError, match="TEMPORAL_GOODHART_AUDIT_CREATED_AT_INVALID"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                task_metadata_json=metadata_path,
                tasks_dir=tasks_dir,
                output_root=tmp_path / "fsv",
                state_root=tmp_path / "state",
                run_id="run-test",
                expected_task_count=12,
                newest_task_count=3,
                temporal_holdout_task_count=4,
                min_years=4,
                drop_threshold=0.05,
                severe_drop_threshold=0.10,
                allow_non_aiwonder_roots=True,
            )
        )


def test_durable_roots_require_aiwonder_without_test_escape(tmp_path: Path):
    tasks_dir = tmp_path / "tasks"
    task_ids = write_tasks(tasks_dir)
    metadata_path = tmp_path / "metadata.json"
    write_metadata(metadata_path, task_ids)
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(
        fsv,
        report(
            [row("org__repo-00", "org/repo", 0.9, True)],
            [row("org__repo-01", "org/repo", 0.1, False), row("org__repo-01", "org/repo", 0.9, True)],
        ),
    )
    with pytest.raises(TemporalGoodhartAuditError, match="TEMPORAL_GOODHART_AUDIT_NON_AIWONDER_DURABLE_PATH"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                task_metadata_json=metadata_path,
                tasks_dir=tasks_dir,
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/temporal-goodhart-audit"),
                run_id="run-test",
                expected_task_count=12,
                newest_task_count=3,
                temporal_holdout_task_count=4,
                min_years=4,
                drop_threshold=0.05,
                severe_drop_threshold=0.10,
                allow_non_aiwonder_roots=False,
            )
        )
