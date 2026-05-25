"""CLI bridge: read a ship-gate FSV report, write an honest-pearson sibling JSON.

#695 follow-up to honest_pearson_metrics.py. Consumes the same shape of
JSON that python_calibration_active_gate.py reads (an `execute_and_inspect`
dict with per-row prediction arrays) and writes a `*.honest_pearson.json`
artifact next to it carrying the three-number gate-metric trio (raw
Pearson / isotonic Pearson / predictor ablation) plus Brier and ECE.

Per #695 Done When #3: 'FSV reports all three numbers, not just rank-lift.'

Usage:
    python3 -m scripts.swebench_lite_ops.honest_pearson_cli \
        --ship-gate-report /zfs/archive/contextgraph/fsv/.../ship-gate-fsv.json \
        --output-path /zfs/archive/contextgraph/fsv/.../honest-pearson.json
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .honest_pearson_metrics import honest_predictor_report


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "HONEST_PEARSON_BAD_INPUT",
            f"{label} must be a JSON object",
            "Pass a ship-gate report whose execute_and_inspect is an object.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "HONEST_PEARSON_BAD_INPUT",
            f"{label} must be a JSON array",
            "Pass a ship-gate report with per-row prediction arrays.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def _extract_rows(
    rows: list[dict[str, Any]], split_label: str
) -> tuple[list[float], list[float]]:
    predictions: list[float] = []
    labels: list[float] = []
    for idx, raw in enumerate(rows):
        if not isinstance(raw, dict):
            raise SwebenchOpsError(
                "HONEST_PEARSON_BAD_ROW",
                f"{split_label} row {idx} must be an object",
                "Each ship-gate prediction row must be a JSON object.",
                {"split": split_label, "index": idx},
            )
        score = raw.get("predicted_oracle_pass")
        actual = raw.get("actual_oracle_pass")
        if not isinstance(score, (int, float)):
            raise SwebenchOpsError(
                "HONEST_PEARSON_BAD_ROW",
                f"{split_label} row {idx} missing or non-numeric predicted_oracle_pass",
                "Every prediction row must include predicted_oracle_pass : float.",
                {"split": split_label, "index": idx, "predicted_oracle_pass": score},
            )
        if not isinstance(actual, (bool, int, float)):
            raise SwebenchOpsError(
                "HONEST_PEARSON_BAD_ROW",
                f"{split_label} row {idx} missing or non-numeric actual_oracle_pass",
                "Every prediction row must include actual_oracle_pass : bool|float.",
                {"split": split_label, "index": idx, "actual_oracle_pass": actual},
            )
        predictions.append(float(score))
        labels.append(1.0 if bool(actual) else 0.0)
    return predictions, labels


def _raw_prediction_rows(
    execute: dict[str, Any], split_label: str
) -> tuple[str, list[dict[str, Any]]]:
    for key in (
        f"{split_label}_raw_predictions",
        f"{split_label}_predictions",
        f"{split_label}_rank_lift_predictions",
    ):
        if key in execute and execute.get(key) is not None:
            return key, _require_array(execute.get(key), key)
    raise SwebenchOpsError(
        "HONEST_PEARSON_BAD_INPUT",
        f"{split_label} raw prediction rows are missing",
        "Pass a ship-gate report with raw per-row prediction arrays.",
        {"split": split_label},
    )


def _maybe_extract_rank_lift(rows: list[dict[str, Any]]) -> list[float] | None:
    if not rows:
        return None
    if "rank_lift_score" not in rows[0] and "rank_lift_predicted_oracle_pass" not in rows[0]:
        return None
    out: list[float] = []
    for idx, raw in enumerate(rows):
        if not isinstance(raw, dict):
            return None
        value = raw.get("rank_lift_score")
        if value is None:
            value = raw.get("rank_lift_predicted_oracle_pass")
        if not isinstance(value, (int, float)):
            return None
        out.append(float(value))
    return out


def _extract_comparator_scores(
    value: Any,
    raw_labels: list[float],
    label: str,
) -> list[float] | None:
    if value is None:
        return None
    rows = _require_array(value, label)
    if not rows:
        return None
    scores, labels = _extract_rows(rows, label)
    if len(scores) != len(raw_labels) or labels != raw_labels:
        raise SwebenchOpsError(
            "HONEST_PEARSON_COMPARATOR_MISMATCH",
            f"{label} does not align with raw prediction labels",
            "Rank-lift/rank-safe comparator rows must align with the raw prediction split.",
            {
                "label": label,
                "comparator_count": len(scores),
                "raw_count": len(raw_labels),
            },
        )
    return scores


def build_report(report_path: Path) -> dict[str, Any]:
    payload = json.loads(report_path.read_text(encoding="utf-8"))
    execute = _require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_source, calibration_rows = _raw_prediction_rows(execute, "calibration")
    holdout_source, holdout_rows = _raw_prediction_rows(execute, "holdout")
    cal_p, cal_y = _extract_rows(calibration_rows, "calibration")
    hol_p, hol_y = _extract_rows(holdout_rows, "holdout")
    cal_rank_lift = _extract_comparator_scores(
        execute.get("calibration_rank_lift_predictions"),
        cal_y,
        "calibration_rank_lift_predictions",
    ) or _maybe_extract_rank_lift(calibration_rows)
    hol_rank_lift = _extract_comparator_scores(
        execute.get("holdout_rank_lift_predictions"),
        hol_y,
        "holdout_rank_lift_predictions",
    )
    if hol_rank_lift is None and holdout_source == "holdout_raw_predictions":
        hol_rank_lift = _extract_comparator_scores(
            execute.get("holdout_predictions"),
            hol_y,
            "holdout_predictions",
        )
    if hol_rank_lift is None:
        hol_rank_lift = _maybe_extract_rank_lift(holdout_rows)
    cal_rank_safe = _extract_comparator_scores(
        execute.get("calibration_rank_safe_predictions"),
        cal_y,
        "calibration_rank_safe_predictions",
    )
    hol_rank_safe = _extract_comparator_scores(
        execute.get("holdout_rank_safe_predictions"),
        hol_y,
        "holdout_rank_safe_predictions",
    )
    cal_report = honest_predictor_report(
        cal_p,
        cal_y,
        rank_lift_predictions=cal_rank_lift,
        rank_safe_predictions=cal_rank_safe,
    )
    hol_report = honest_predictor_report(
        hol_p,
        hol_y,
        rank_lift_predictions=hol_rank_lift,
        rank_safe_predictions=hol_rank_safe,
    )
    return {
        "schema_version": 1,
        "artifact_kind": "honest_pearson_ship_gate_report",
        "created_at_utc": utc_now(),
        "ship_gate_report_path": str(report_path),
        "ship_gate_report_sha256": sha256_file(report_path),
        "raw_prediction_sources": {
            "calibration": calibration_source,
            "holdout": holdout_source,
        },
        "calibration": cal_report.to_dict(),
        "holdout": hol_report.to_dict(),
        "notes": (
            "Per #695: raw Pearson is the gate-honest metric. Isotonic should "
            "preserve Pearson within numerical noise on a held-out split (this "
            "report fits and measures on the same split, so isotonic Pearson can "
            "rise slightly from rank-tie pooling). predictor_ablation replaces "
            "every prediction with 0.5 and recomputes raw Pearson; if the "
            "ablated correlation is meaningfully positive, the gate-relevant "
            "signal is not coming from the predictor."
        ),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compute #695 honest predictor metrics from a ship-gate FSV report"
    )
    parser.add_argument(
        "--ship-gate-report",
        type=Path,
        required=True,
        help="Path to ship-gate-fsv.json (or equivalent execute_and_inspect-shaped report).",
    )
    parser.add_argument(
        "--output-path",
        type=Path,
        required=True,
        help="Output path for the honest-pearson sibling JSON.",
    )
    args = parser.parse_args(argv)
    report = build_report(args.ship_gate_report)
    write_json_checked(args.output_path, report)
    print(json.dumps({"ok": True, "wrote": str(args.output_path)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
