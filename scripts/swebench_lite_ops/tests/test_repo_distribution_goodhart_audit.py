from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_repo_distribution_goodhart_audit import (
    RepoDistributionAuditError,
    run_audit,
    weighted_pearson,
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
            "holdout_predictions": [
                {**item, "predicted_oracle_pass": 0.99 if item["actual_oracle_pass"] else 0.01}
                for item in holdout_rows
            ],
        }
    }


def write_tasks(root: Path) -> None:
    repos = [
        ("org/repo-a", 4),
        ("org/repo-b", 4),
        ("org/repo-c", 4),
        ("org/repo-d", 4),
        ("org/repo-e", 4),
        ("org/repo-f", 4),
        ("org/repo-g", 4),
        ("org/repo-h", 4),
    ]
    for repo, count in repos:
        for idx in range(count):
            instance_id = f"{repo.replace('/', '__')}-{idx}"
            write_json(root / f"{instance_id}.json", {"instance_id": instance_id, "repo": repo})


def run_fixture(
    tmp_path: Path,
    *,
    calibration_rows: list[dict],
    holdout_rows: list[dict],
    min_generalization_repos: int = 4,
) -> dict:
    fsv = tmp_path / "ship-gate-fsv.json"
    tasks = tmp_path / "tasks"
    write_tasks(tasks)
    write_json(fsv, report(calibration_rows, holdout_rows))
    return run_audit(
        Namespace(
            ship_gate_report=fsv,
            tasks_dir=tasks,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_task_count=32,
            min_generalization_repos=min_generalization_repos,
            holdout_repo_count=4,
            target_holdout_fraction=0.25,
            uniform_drop_threshold=0.05,
            severe_drop_threshold=0.10,
            allow_non_aiwonder_roots=True,
        )
    )


def test_weighted_pearson_balances_repo_mass():
    scores = [0.9, 0.8, 0.2, 0.1]
    labels = [1.0, 1.0, 0.0, 0.0]
    weights = [0.125, 0.125, 0.375, 0.375]
    value = weighted_pearson(scores, labels, weights)
    assert value is not None
    assert value > 0.95


def test_narrow_holdout_coverage_requires_repo_atomic_evaluation(tmp_path: Path):
    calibration = [
        row("a-cal-1", "org/repo-a", 0.9, True),
        row("a-cal-2", "org/repo-a", 0.1, False),
        row("b-cal-1", "org/repo-b", 0.8, True),
        row("b-cal-2", "org/repo-b", 0.2, False),
    ]
    holdout = [
        row("a-hol-1", "org/repo-a", 0.9, True),
        row("a-hol-2", "org/repo-a", 0.8, True),
        row("a-hol-3", "org/repo-a", 0.2, False),
        row("a-hol-4", "org/repo-a", 0.1, False),
        row("b-hol-1", "org/repo-b", 0.7, True),
        row("b-hol-2", "org/repo-b", 0.3, False),
    ]
    result = run_fixture(tmp_path, calibration_rows=calibration, holdout_rows=holdout)
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["repo_coverage"]["holdout_repo_count"] == 2
    assert manifest["repo_coverage"]["current_holdout_generalization_claim_allowed"] is False
    assert manifest["decision"]["full_repo_atomic_disjoint_evaluation_required"] is True
    assert manifest["decision"]["canonical_full_execution_issue"] == "#766"
    assert "current_holdout_repo_coverage_insufficient" in manifest["decision"]["decision_reasons"]
    assert Path(result["per_repo_holdout_metrics"]).is_file()
    assert Path(result["repo_subset_holdout_metrics"]).is_file()


def test_uniform_repo_reweighting_drop_flags_goodhart_risk(tmp_path: Path):
    calibration = [
        row("a-cal-1", "org/repo-a", 0.9, True),
        row("b-cal-1", "org/repo-b", 0.1, False),
    ]
    holdout: list[dict] = []
    for idx in range(20):
        holdout.append(row(f"a-pass-{idx}", "org/repo-a", 0.95, True))
        holdout.append(row(f"a-fail-{idx}", "org/repo-a", 0.05, False))
    holdout.extend(
        [
            row("b-pass", "org/repo-b", 0.10, True),
            row("b-fail", "org/repo-b", 0.90, False),
            row("c-pass", "org/repo-c", 0.20, True),
            row("c-fail", "org/repo-c", 0.80, False),
            row("d-pass", "org/repo-d", 0.30, True),
            row("d-fail", "org/repo-d", 0.70, False),
        ]
    )
    result = run_fixture(
        tmp_path,
        calibration_rows=calibration,
        holdout_rows=holdout,
        min_generalization_repos=4,
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["repo_coverage"]["holdout_repo_count"] == 4
    assert manifest["metrics"]["uniform_repo_reweighting_drop"] > 0.05
    assert manifest["decision"]["repo_distribution_goodhart_risk_detected_on_current_fsv"] is True
    assert "uniform_repo_reweighting_drops_correlation" in manifest["decision"]["decision_reasons"]


def test_degenerate_repo_metric_does_not_crash(tmp_path: Path):
    calibration = [row("cal-1", "org/repo-a", 0.9, True), row("cal-2", "org/repo-a", 0.1, False)]
    holdout = [
        row("a-1", "org/repo-a", 0.9, True),
        row("a-2", "org/repo-a", 0.8, True),
        row("b-1", "org/repo-b", 0.1, False),
        row("b-2", "org/repo-b", 0.2, False),
    ]
    result = run_fixture(tmp_path, calibration_rows=calibration, holdout_rows=holdout, min_generalization_repos=2)
    rows = [
        json.loads(line)
        for line in Path(result["per_repo_holdout_metrics"]).read_text(encoding="utf-8").splitlines()
    ]
    assert {item["pearson_defined"] for item in rows} == {False}


def test_missing_holdout_rows_fail_closed(tmp_path: Path):
    fsv = tmp_path / "ship-gate-fsv.json"
    tasks = tmp_path / "tasks"
    write_tasks(tasks)
    write_json(fsv, {"execute_and_inspect": {"calibration_predictions": []}})
    with pytest.raises(RepoDistributionAuditError, match="REPO_DISTRIBUTION_AUDIT_MISSING_ROWS"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                tasks_dir=tasks,
                output_root=tmp_path / "fsv",
                state_root=tmp_path / "state",
                run_id="run-test",
                expected_task_count=32,
                min_generalization_repos=4,
                holdout_repo_count=4,
                target_holdout_fraction=0.25,
                uniform_drop_threshold=0.05,
                severe_drop_threshold=0.10,
                allow_non_aiwonder_roots=True,
            )
        )


def test_durable_roots_require_aiwonder_without_test_escape(tmp_path: Path):
    fsv = tmp_path / "ship-gate-fsv.json"
    tasks = tmp_path / "tasks"
    write_tasks(tasks)
    write_json(
        fsv,
        report(
            [row("cal-1", "org/repo-a", 0.9, True)],
            [row("hol-1", "org/repo-a", 0.9, True), row("hol-2", "org/repo-a", 0.1, False)],
        ),
    )
    with pytest.raises(RepoDistributionAuditError, match="REPO_DISTRIBUTION_AUDIT_NON_AIWONDER_DURABLE_PATH"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                tasks_dir=tasks,
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/repo-distribution-goodhart-audit"),
                run_id="run-test",
                expected_task_count=32,
                min_generalization_repos=4,
                holdout_repo_count=4,
                target_holdout_fraction=0.25,
                uniform_drop_threshold=0.05,
                severe_drop_threshold=0.10,
                allow_non_aiwonder_roots=False,
            )
        )
