"""End-to-end test for honest_pearson_cli against a synthetic FSV-shaped report.

Verifies that the CLI consumes the same JSON shape that
python_calibration_active_gate.py reads, runs honest_predictor_report,
and writes an honest-pearson sibling JSON that downstream consumers can
parse.
"""

from __future__ import annotations

import json
import math
import tempfile
from pathlib import Path

import numpy as np
import pytest

from ..honest_pearson_cli import build_report, main


def _row(predicted: float, actual: bool, rank_lift: float | None = None) -> dict:
    row = {
        "task_id": f"task-{predicted}-{actual}",
        "repo": "synthetic/repo",
        "category": "synthetic_category",
        "patch_path": "/zfs/archive/contextgraph/test/synthetic.patch",
        "patch_sha256": "deadbeef" * 8,
        "predicted_oracle_pass": predicted,
        "actual_oracle_pass": actual,
    }
    if rank_lift is not None:
        row["rank_lift_score"] = rank_lift
    return row


def _fsv_report(calibration_rows: list[dict], holdout_rows: list[dict]) -> dict:
    return {
        "execute_and_inspect": {
            "calibration_predictions": calibration_rows,
            "holdout_predictions": holdout_rows,
        }
    }


def test_build_report_with_rank_correlated_predictions(tmp_path):
    rng = np.random.default_rng(seed=695)
    n = 40
    actual = rng.integers(0, 2, size=n).astype(bool)
    raw_predictor = actual.astype(np.float64) * 0.6 + rng.normal(0.2, 0.1, size=n)
    rank_lift = raw_predictor * 0.3 + actual.astype(np.float64) * 0.7 + rng.normal(0, 0.05, size=n)
    rows = [
        _row(float(p), bool(a), rank_lift=float(rl))
        for p, a, rl in zip(raw_predictor, actual, rank_lift)
    ]
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(rows[:20], rows[20:])))
    report = build_report(fsv_path)
    assert report["schema_version"] == 1
    assert report["artifact_kind"] == "honest_pearson_ship_gate_report"
    assert report["ship_gate_report_path"] == str(fsv_path)
    assert isinstance(report["ship_gate_report_sha256"], str)
    cal = report["calibration"]
    hol = report["holdout"]
    assert cal["sample_count"] == 20
    assert hol["sample_count"] == 20
    # Per #695: predictor is meaningfully correlated with actual; rank-lift
    # composite adds extra correlation from non-predictor features.
    assert cal["raw_pearson"] > 0.3
    assert hol["raw_pearson"] > 0.3
    assert cal["rank_lift_minus_raw_pearson"] is not None
    assert hol["rank_lift_minus_raw_pearson"] is not None
    # By construction the rank-lift composite is at least as correlated as
    # the raw predictor in expectation.
    assert cal["rank_lift_minus_raw_pearson"] >= -0.1
    assert hol["rank_lift_minus_raw_pearson"] >= -0.1


def test_build_report_rejects_non_object_execute(tmp_path):
    fsv_path = tmp_path / "bad.json"
    fsv_path.write_text(json.dumps({"execute_and_inspect": []}))
    with pytest.raises(Exception) as exc:
        build_report(fsv_path)
    # SwebenchOpsError surfaces the structured code on .code (the str() form
    # only shows the message).
    assert getattr(exc.value, "code", "") == "HONEST_PEARSON_BAD_INPUT"


def test_build_report_rejects_row_missing_actual_oracle_pass(tmp_path):
    bad_row = {
        "predicted_oracle_pass": 0.5,
        "task_id": "missing-actual",
    }
    good_row = _row(0.5, True)
    fsv_path = tmp_path / "bad-row.json"
    fsv_path.write_text(
        json.dumps(_fsv_report([good_row, bad_row], [good_row, good_row]))
    )
    with pytest.raises(Exception) as exc:
        build_report(fsv_path)
    assert getattr(exc.value, "code", "") == "HONEST_PEARSON_BAD_ROW"


def test_main_writes_json(tmp_path):
    rows = [_row(0.1, False), _row(0.9, True), _row(0.2, False), _row(0.8, True)]
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(rows, rows)))
    out_path = tmp_path / "honest-pearson.json"
    rc = main(
        [
            "--ship-gate-report",
            str(fsv_path),
            "--output-path",
            str(out_path),
        ]
    )
    assert rc == 0
    written = json.loads(out_path.read_text(encoding="utf-8"))
    assert written["artifact_kind"] == "honest_pearson_ship_gate_report"
    assert written["calibration"]["sample_count"] == 4
    assert written["holdout"]["sample_count"] == 4
    # 4 rows with strong rank-monotone predictions → raw Pearson high.
    assert written["calibration"]["raw_pearson"] > 0.8


def test_predictor_ablation_isolates_noise(tmp_path):
    """When the predictor is pure noise but rank-lift adds a strong signal,
    the honest report should make the noise visible: raw_pearson near 0 and
    rank_lift_minus_raw_pearson large positive."""
    rng = np.random.default_rng(seed=42)
    n = 60
    actual = rng.integers(0, 2, size=n).astype(bool)
    noise = rng.normal(0.5, 0.1, size=n)
    rank_lift_with_signal = actual.astype(np.float64) * 0.7 + rng.normal(0.1, 0.05, size=n)
    rows = [
        _row(float(p), bool(a), rank_lift=float(rl))
        for p, a, rl in zip(noise, actual, rank_lift_with_signal)
    ]
    fsv_path = tmp_path / "noise-fsv.json"
    fsv_path.write_text(json.dumps(_fsv_report(rows[:30], rows[30:])))
    report = build_report(fsv_path)
    cal = report["calibration"]
    # Predictor near zero correlation; rank-lift carries the signal.
    assert abs(cal["raw_pearson"]) < 0.35
    assert cal["rank_lift_minus_raw_pearson"] > 0.3


def test_build_report_prefers_holdout_raw_predictions_over_rank_lift_projection(tmp_path):
    """The 2026-05-21 ship-gate FSV stores rank-lift in holdout_predictions
    and the predictor scores in holdout_raw_predictions. The honest report
    must not count the rank-lift projection as raw Pearson."""
    actual = [False, False, True, True, False, True, False, True]
    raw = [0.45, 0.55, 0.40, 0.60, 0.52, 0.48, 0.50, 0.51]
    rank_lift = [0.05, 0.10, 0.88, 0.91, 0.20, 0.86, 0.15, 0.94]
    rank_safe = [0.20, 0.25, 0.70, 0.75, 0.30, 0.80, 0.35, 0.85]
    raw_rows = [_row(p, a) for p, a in zip(raw, actual)]
    rank_lift_rows = [_row(p, a) for p, a in zip(rank_lift, actual)]
    rank_safe_rows = [_row(p, a) for p, a in zip(rank_safe, actual)]
    fsv_path = tmp_path / "ship-gate-fsv.json"
    fsv_path.write_text(
        json.dumps(
            {
                "execute_and_inspect": {
                    "calibration_predictions": raw_rows,
                    "calibration_rank_lift_predictions": rank_lift_rows,
                    "calibration_rank_safe_predictions": rank_safe_rows,
                    "holdout_raw_predictions": raw_rows,
                    "holdout_predictions": rank_lift_rows,
                    "holdout_rank_safe_predictions": rank_safe_rows,
                }
            }
        )
    )
    report = build_report(fsv_path)
    assert report["raw_prediction_sources"]["holdout"] == "holdout_raw_predictions"
    # Raw is weak/noisy; rank-lift is strongly label-aligned.
    assert abs(report["holdout"]["raw_pearson"]) < 0.35
    assert report["holdout"]["rank_lift_minus_raw_pearson"] > 0.6
    assert report["holdout"]["rank_safe_minus_raw_pearson"] > 0.4
