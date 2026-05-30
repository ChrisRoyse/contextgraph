from __future__ import annotations

import json

import pytest

from ..sc_cp_cli import build_report, main
from ..sc_cp_metrics import (
    classification_set,
    conformal_tau,
    venn_abers_predict,
)


def _row(score: float, actual: bool, category: str = "off_by_one") -> dict:
    return {
        "task_id": f"task-{score}-{actual}",
        "repo": "synthetic/repo",
        "category": category,
        "patch_path": "/zfs/archive/contextgraph/test/synthetic.patch",
        "patch_sha256": "abc123" * 10,
        "predicted_oracle_pass": score,
        "actual_oracle_pass": actual,
    }


def _fsv_report(calibration_rows: list[dict], holdout_rows: list[dict]) -> dict:
    return {
        "execute_and_inspect": {
            "calibration_predictions": calibration_rows,
            "holdout_raw_predictions": holdout_rows,
            "holdout_predictions": [_row(0.99 if r["actual_oracle_pass"] else 0.01, r["actual_oracle_pass"], r["category"]) for r in holdout_rows],
        }
    }


def test_conformal_tau_uses_finite_sample_quantile():
    scores = [0.01, 0.02, 0.03, 0.40, 0.50]
    # ceil((5 + 1) * 0.9) - 1 = 5, clamped to last index.
    assert conformal_tau(scores, 0.10) == pytest.approx(0.50)


def test_venn_abers_prediction_is_ordered_and_bounded():
    prediction = venn_abers_predict(
        [0.1, 0.2, 0.8, 0.9],
        [0, 0, 1, 1],
        0.85,
    )
    assert 0.0 <= prediction.lower <= prediction.upper <= 1.0
    assert 0.0 <= prediction.point <= 1.0
    assert prediction.point > 0.5


def test_classification_set_can_be_empty_singleton_or_both():
    assert classification_set(0.50, 0.40) == ()
    assert classification_set(0.90, 0.20) == (1,)
    assert classification_set(0.50, 0.60) == (0, 1)


def test_build_report_prefers_holdout_raw_predictions(tmp_path):
    calibration = [
        _row(0.10, False),
        _row(0.20, False),
        _row(0.80, True),
        _row(0.90, True),
    ] * 8
    holdout = [
        _row(0.45, False, "off_by_one"),
        _row(0.55, True, "off_by_one"),
        _row(0.48, False, "subtle_flip"),
        _row(0.52, True, "subtle_flip"),
    ] * 6
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(calibration, holdout)), encoding="utf-8")
    report = build_report(
        fsv_path,
        alpha=0.10,
        bin_count=4,
        min_bin_size=2,
        min_cell_rows=2,
    )
    assert report["raw_prediction_sources"]["holdout"] == "holdout_raw_predictions"
    sc_cp = report["sc_cp"]
    assert sc_cp["policy"]["holdout_used_for_fitting"] is False
    assert sc_cp["calibration_fit"]["sample_count"] == len(calibration)
    assert sc_cp["holdout"]["sample_count"] == len(holdout)
    assert set(sc_cp["per_cell"]) == {"off_by_one::python", "subtle_flip::python"}
    assert sc_cp["row_predictions"][0]["venn_abers"]["point"] < 0.99


def test_main_writes_sc_cp_json(tmp_path):
    calibration = [_row(0.1, False), _row(0.2, False), _row(0.8, True), _row(0.9, True)] * 8
    holdout = [_row(0.15, False), _row(0.85, True), _row(0.25, False), _row(0.75, True)] * 4
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(calibration, holdout)), encoding="utf-8")
    out_path = tmp_path / "sc-cp.json"
    rc = main(
        [
            "--ship-gate-report",
            str(fsv_path),
            "--output-path",
            str(out_path),
            "--prediction-bin-count",
            "4",
            "--min-bin-size",
            "2",
            "--min-cell-rows",
            "2",
        ]
    )
    assert rc == 0
    written = json.loads(out_path.read_text(encoding="utf-8"))
    assert written["artifact_kind"] == "inv_017_sc_cp_ship_gate_report"
    assert written["sc_cp"]["holdout"]["sample_count"] == len(holdout)
    assert "promotion_allowed" in written["sc_cp"]["decision"]
