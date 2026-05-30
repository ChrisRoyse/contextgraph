from __future__ import annotations

import json

from ..mondrian_conformal_cli import build_report, main
from ..sc_cp_metrics import build_mondrian_conformal_report


def _row(score: float, actual: bool, category: str = "off_by_one") -> dict:
    return {
        "task_id": f"task-{score}-{actual}-{category}",
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
            "holdout_predictions": [
                _row(
                    0.99 if r["actual_oracle_pass"] else 0.01,
                    r["actual_oracle_pass"],
                    r["category"],
                )
                for r in holdout_rows
            ],
        }
    }


def test_build_mondrian_conformal_uses_cell_specific_fits():
    calibration = (
        [_row(0.05, False, "off_by_one"), _row(0.15, False, "off_by_one"), _row(0.85, True, "off_by_one"), _row(0.95, True, "off_by_one")]
        + [_row(0.10, False, "subtle_flip"), _row(0.25, False, "subtle_flip"), _row(0.75, True, "subtle_flip"), _row(0.90, True, "subtle_flip")]
    ) * 4
    holdout = (
        [_row(0.12, False, "off_by_one"), _row(0.88, True, "off_by_one")]
        + [_row(0.18, False, "subtle_flip"), _row(0.82, True, "subtle_flip")]
    ) * 5
    report = build_mondrian_conformal_report(
        calibration,
        holdout,
        alpha=0.10,
        min_cell_rows=4,
        in_band_cell_fraction=0.80,
    )
    assert report["artifact_kind"] == "inv_019_mondrian_conformal_report"
    assert report["policy"]["holdout_used_for_fitting"] is False
    assert report["calibration_fit"]["eligible_cell_fit_count"] == 2
    assert set(report["per_cell"]) == {"off_by_one::python", "subtle_flip::python"}
    assert report["decision"]["support_gap_cells"] == []
    assert all(
        row["probability_interval"]["tau_source"].startswith("mondrian_cell:")
        for row in report["row_predictions"]
    )


def test_insufficient_cell_support_uses_explicit_fallback_and_blocks_promotion():
    calibration = [
        _row(0.05, False, "off_by_one"),
        _row(0.95, True, "off_by_one"),
        _row(0.15, False, "subtle_flip"),
        _row(0.85, True, "subtle_flip"),
    ]
    holdout = [_row(0.20, False, "off_by_one"), _row(0.80, True, "off_by_one")] * 3
    report = build_mondrian_conformal_report(
        calibration,
        holdout,
        alpha=0.10,
        min_cell_rows=4,
    )
    assert report["decision"]["promotion_allowed"] is False
    assert report["decision"]["support_gap_cells"] == ["off_by_one::python"]
    assert report["per_cell"]["off_by_one::python"]["used_global_diagnostic_fallback"] is True
    assert all(
        row["probability_interval"]["tau_source"].startswith("global_diagnostic_fallback:")
        for row in report["row_predictions"]
    )


def test_build_report_prefers_holdout_raw_predictions(tmp_path):
    calibration = (
        [_row(0.05, False, "off_by_one"), _row(0.95, True, "off_by_one")]
        + [_row(0.10, False, "subtle_flip"), _row(0.90, True, "subtle_flip")]
    ) * 8
    holdout = (
        [_row(0.20, False, "off_by_one"), _row(0.80, True, "off_by_one")]
        + [_row(0.25, False, "subtle_flip"), _row(0.75, True, "subtle_flip")]
    ) * 4
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(calibration, holdout)), encoding="utf-8")
    report = build_report(fsv_path, alpha=0.10, min_cell_rows=4)
    assert report["artifact_kind"] == "inv_019_mondrian_conformal_ship_gate_report"
    assert report["raw_prediction_sources"]["holdout"] == "holdout_raw_predictions"
    assert report["mondrian_conformal"]["policy"]["method"] == "mondrian_cell_venn_abers_split_conformal"


def test_main_writes_mondrian_conformal_json(tmp_path):
    calibration = [_row(0.1, False), _row(0.2, False), _row(0.8, True), _row(0.9, True)] * 8
    holdout = [_row(0.15, False), _row(0.85, True), _row(0.25, False), _row(0.75, True)] * 4
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(calibration, holdout)), encoding="utf-8")
    out_path = tmp_path / "mondrian.json"
    rc = main(
        [
            "--ship-gate-report",
            str(fsv_path),
            "--output-path",
            str(out_path),
            "--min-cell-rows",
            "4",
        ]
    )
    assert rc == 0
    written = json.loads(out_path.read_text(encoding="utf-8"))
    assert written["artifact_kind"] == "inv_019_mondrian_conformal_ship_gate_report"
    assert "promotion_allowed" in written["mondrian_conformal"]["decision"]
