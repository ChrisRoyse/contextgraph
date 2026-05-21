from __future__ import annotations

import json
import math
from collections import defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now


DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-121-per-cell-smooth-ece-fsv"
)
COVERAGE_POINTS = (0.50, 0.60, 0.70, 0.80, 0.85, 0.90, 0.95, 1.00)
ECE_BINS = 10
SHIP_GATE_TARGET = 0.95


def add_arguments(parser: Any) -> None:
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-holdout-count", type=int, default=240)


def command_python_per_cell_smooth_ece(args: Any) -> dict[str, Any]:
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    reject_mounted_drive("ship_gate_report", ship_gate_report)
    reject_mounted_drive("output_root", output_root)
    if args.expected_holdout_count <= 0:
        raise SwebenchOpsError(
            "PY_G_121_EXPECTED_COUNT_INVALID",
            "--expected-holdout-count must be positive",
            "Pass the current holdout row count, normally 240.",
            {"expected_holdout_count": args.expected_holdout_count},
        )
    payload = read_json_object(ship_gate_report)
    execute = require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    holdout_rows = require_array(execute.get("holdout_predictions"), "holdout_predictions")
    if len(holdout_rows) != args.expected_holdout_count:
        raise SwebenchOpsError(
            "PY_G_121_HOLDOUT_COUNT_MISMATCH",
            "ship-gate holdout row count does not match expected count",
            "Use the current aiwonder ship-gate FSV or pass the expected count explicitly.",
            {"expected": args.expected_holdout_count, "actual": len(holdout_rows)},
        )
    rows = [prediction_row(row, index) for index, row in enumerate(holdout_rows)]
    cells: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        cells[cell_key(row)].append(row)

    per_cell = {
        cell: metrics_payload(cell_rows, coverage_points=COVERAGE_POINTS)
        for cell, cell_rows in sorted(cells.items())
    }
    global_metrics = metrics_payload(rows, coverage_points=COVERAGE_POINTS)
    active_diag = parse_active_abstain_diagnostic(execute)
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_121_per_cell_smooth_ece",
        "githubIssue": 575,
        "ok": True,
        "shipGateReport": str(ship_gate_report),
        "shipGateReportSha256": sha256_file(ship_gate_report),
        "rowCount": len(rows),
        "cellCount": len(per_cell),
        "global": global_metrics,
        "perCell": per_cell,
        "activeAbstainDiagnostic": active_diag,
        "coveragePolicy": {
            "coveragePoints": list(COVERAGE_POINTS),
            "rowOrdering": "descending abs(predicted_oracle_pass - 0.5)",
            "holdoutThresholdSelectionUsed": False,
            "purpose": "observability only; #576 owns calibration/selection fitting",
        },
        "smoothEcePolicy": {
            "estimator": "gaussian_kernel_score_space_v1",
            "bandwidth": "silverman_over_scores_clamped_0.05_0.25_per_group",
            "fixedBinEceBins": ECE_BINS,
        },
        "pathPolicy": {
            "mntDActiveReadsOrWrites": False,
            "aiwonderOnly": str(ship_gate_report).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
        },
        "gateInterpretation": {
            "shipGateTarget": SHIP_GATE_TARGET,
            "activeSubsetAlreadyClearsTarget": active_diag.get("active_metrics", {}).get(
                "prediction_oracle_correlation"
            ),
            "nextIssue": "#576 uses this report to fit calibration/coverage using train/calibration only",
        },
    }
    output_root.mkdir(parents=True, exist_ok=True)
    report_path = output_root / "per_cell_smooth_ece_report.json"
    write_json_checked(report_path, report)
    report["reportPath"] = str(report_path)
    report["reportSha256"] = sha256_file(report_path)
    return report


def prediction_row(value: Any, index: int) -> dict[str, Any]:
    row = require_object(value, f"holdout_predictions[{index}]")
    task_id = required_string(row, "task_id")
    category = required_string(row, "category")
    patch_path = required_string(row, "patch_path")
    reject_mounted_drive("patch_path", Path(patch_path))
    score = required_score(row, "predicted_oracle_pass")
    actual = required_bool(row, "actual_oracle_pass")
    return {
        "row_index": index,
        "task_id": task_id,
        "repo": required_string(row, "repo"),
        "category": category,
        "language": "python",
        "patch_path": patch_path,
        "patch_sha256": required_string(row, "patch_sha256"),
        "actual": 1 if actual else 0,
        "score": score,
        "predicted_pass": score >= 0.5,
        "confidence_margin": abs(score - 0.5),
    }


def cell_key(row: dict[str, Any]) -> str:
    return f"{row['category']}::python"


def metrics_payload(rows: list[dict[str, Any]], *, coverage_points: tuple[float, ...]) -> dict[str, Any]:
    labels = [int(row["actual"]) for row in rows]
    scores = [float(row["score"]) for row in rows]
    sorted_rows = sorted(rows, key=lambda row: (-float(row["confidence_margin"]), row["row_index"]))
    coverage_curve = []
    for coverage in coverage_points:
        take = max(1, min(len(sorted_rows), math.ceil(len(sorted_rows) * coverage)))
        subset = sorted_rows[:take]
        coverage_curve.append(
            {
                "coverage": coverage,
                "selectedRowCount": take,
                "metrics": metric_block(
                    [int(row["actual"]) for row in subset],
                    [float(row["score"]) for row in subset],
                ),
            }
        )
    block = metric_block(labels, scores)
    return {
        **block,
        "passCount": sum(labels),
        "failCount": len(labels) - sum(labels),
        "predictedPassCount": sum(1 for score in scores if score >= 0.5),
        "predictedFailCount": sum(1 for score in scores if score < 0.5),
        "powerBlockers": power_blockers(labels, scores),
        "coverageCurve": coverage_curve,
    }


def metric_block(labels: list[int], scores: list[float]) -> dict[str, Any]:
    return {
        "n": len(labels),
        "prediction_oracle_correlation": pearson_or_none(labels, scores),
        "spearman_prediction_oracle_correlation": spearman_or_none(labels, scores),
        "brier": brier(labels, scores),
        "fixed_bin_ece": fixed_bin_ece(labels, scores, ECE_BINS),
        "smooth_ece": smooth_ece(labels, scores),
        "accuracy_at_half": accuracy_at_half(labels, scores),
        "mean_score": sum(scores) / len(scores) if scores else None,
        "mean_label": sum(labels) / len(labels) if labels else None,
    }


def power_blockers(labels: list[int], scores: list[float]) -> list[str]:
    blockers = []
    if len(labels) < 3:
        blockers.append("TOO_FEW_ROWS_FOR_CORRELATION")
    if len(set(labels)) < 2:
        blockers.append("DEGENERATE_ORACLE_VARIANCE")
    if len({round(score, 8) for score in scores}) < 2:
        blockers.append("DEGENERATE_SCORE_VARIANCE")
    if len(labels) < 30:
        blockers.append("LOW_CELL_N_LT_30")
    return blockers


def pearson_or_none(labels: list[int], scores: list[float]) -> float | None:
    if len(labels) < 3:
        return None
    x_mean = sum(float(value) for value in labels) / len(labels)
    y_mean = sum(scores) / len(scores)
    x_var = sum((float(value) - x_mean) ** 2 for value in labels)
    y_var = sum((score - y_mean) ** 2 for score in scores)
    if x_var <= 1.0e-12 or y_var <= 1.0e-12:
        return None
    cov = sum((float(label) - x_mean) * (score - y_mean) for label, score in zip(labels, scores))
    return cov / math.sqrt(x_var * y_var)


def spearman_or_none(labels: list[int], scores: list[float]) -> float | None:
    if len(labels) < 3 or len(set(labels)) < 2 or len(set(scores)) < 2:
        return None
    return pearson_or_none([int(value) for value in rank_values([float(v) for v in labels])], rank_values(scores))


def rank_values(values: list[float]) -> list[float]:
    indexed = sorted(enumerate(values), key=lambda item: item[1])
    ranks = [0.0] * len(values)
    cursor = 0
    while cursor < len(indexed):
        end = cursor + 1
        while end < len(indexed) and indexed[end][1] == indexed[cursor][1]:
            end += 1
        rank = (cursor + 1 + end) / 2.0
        for idx in range(cursor, end):
            ranks[indexed[idx][0]] = rank
        cursor = end
    return ranks


def brier(labels: list[int], scores: list[float]) -> float | None:
    if not labels:
        return None
    return sum((score - float(label)) ** 2 for label, score in zip(labels, scores)) / len(labels)


def accuracy_at_half(labels: list[int], scores: list[float]) -> float | None:
    if not labels:
        return None
    hits = sum(1 for label, score in zip(labels, scores) if (1 if score >= 0.5 else 0) == label)
    return hits / len(labels)


def fixed_bin_ece(labels: list[int], scores: list[float], bins: int) -> float | None:
    if not labels:
        return None
    total = 0.0
    for bin_idx in range(bins):
        low = bin_idx / bins
        high = (bin_idx + 1) / bins
        selected = [
            (label, score)
            for label, score in zip(labels, scores)
            if (score >= low and (score < high or (bin_idx == bins - 1 and score <= high)))
        ]
        if not selected:
            continue
        mean_conf = sum(score for _label, score in selected) / len(selected)
        accuracy = sum(label for label, _score in selected) / len(selected)
        total += (len(selected) / len(labels)) * abs(mean_conf - accuracy)
    return total


def smooth_ece(labels: list[int], scores: list[float]) -> float | None:
    if not labels:
        return None
    bandwidth = smooth_bandwidth(scores)
    total = 0.0
    for center in scores:
        weights = [math.exp(-((score - center) ** 2) / (2.0 * bandwidth * bandwidth)) for score in scores]
        weight_sum = sum(weights)
        if weight_sum <= 0.0:
            continue
        smooth_conf = sum(weight * score for weight, score in zip(weights, scores)) / weight_sum
        smooth_acc = sum(weight * label for weight, label in zip(weights, labels)) / weight_sum
        total += abs(smooth_conf - smooth_acc)
    return total / len(scores)


def smooth_bandwidth(scores: list[float]) -> float:
    if len(scores) < 2:
        return 0.25
    mean = sum(scores) / len(scores)
    variance = sum((score - mean) ** 2 for score in scores) / (len(scores) - 1)
    std = math.sqrt(max(variance, 1.0e-12))
    silverman = 1.06 * std * (len(scores) ** (-1.0 / 5.0))
    return min(0.25, max(0.05, silverman))


def parse_active_abstain_diagnostic(execute: dict[str, Any]) -> dict[str, Any]:
    raw = execute.get("task_py_g_392_active_abstain_ship_gate_diagnostic")
    if not isinstance(raw, dict):
        return {"present": False}
    return {
        "present": True,
        "measurement_valid": raw.get("measurement_valid"),
        "active_row_count": raw.get("active_row_count"),
        "abstained_row_count": raw.get("abstained_row_count"),
        "active_coverage": raw.get("active_coverage"),
        "active_metrics": raw.get("active_metrics"),
        "active_per_category_metrics": raw.get("active_per_category_metrics"),
        "per_category_application": raw.get("per_category_application"),
        "active_metric_gate_passed": raw.get("active_metric_gate_passed"),
        "active_power_gate_passed": raw.get("active_power_gate_passed"),
        "active_four_window_gate_passed": raw.get("active_four_window_gate_passed"),
        "full_active_abstain_ship_gate_passed": raw.get("full_active_abstain_ship_gate_passed"),
        "active_metric_blockers": raw.get("active_metric_blockers"),
        "active_statistical_power_blockers": raw.get("active_statistical_power_blockers"),
    }


def read_json_object(path: Path) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_121_REPORT_MISSING",
            f"required ship-gate report missing: {path}",
            "Run or point at the current aiwonder ship-gate FSV report.",
            {"path": str(path)},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "PY_G_121_REPORT_BAD_JSON",
            f"ship-gate report is not valid JSON: {path}",
            "Regenerate the FSV report before consuming it.",
            {"path": str(path), "error": str(exc)},
        ) from exc
    return require_object(value, str(path))


def require_object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "PY_G_121_OBJECT_REQUIRED",
            f"{name} must be a JSON object",
            "Use a current ship-gate FSV report.",
            {"name": name, "type": type(value).__name__},
        )
    return value


def require_array(value: Any, name: str) -> list[Any]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "PY_G_121_ARRAY_REQUIRED",
            f"{name} must be a JSON array",
            "Use a current ship-gate FSV report.",
            {"name": name, "type": type(value).__name__},
        )
    return value


def required_string(value: dict[str, Any], key: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise SwebenchOpsError(
            "PY_G_121_STRING_REQUIRED",
            f"required string missing: {key}",
            "Use a current ship-gate FSV report.",
            {"key": key, "value": raw},
        )
    return raw.strip()


def required_bool(value: dict[str, Any], key: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise SwebenchOpsError(
            "PY_G_121_BOOL_REQUIRED",
            f"required bool missing: {key}",
            "Use a current ship-gate FSV report.",
            {"key": key, "value": raw},
        )
    return raw


def required_score(value: dict[str, Any], key: str) -> float:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, (int, float)):
        raise SwebenchOpsError(
            "PY_G_121_SCORE_REQUIRED",
            f"required numeric score missing: {key}",
            "Use a current ship-gate FSV report.",
            {"key": key, "value": raw},
        )
    score = float(raw)
    if not math.isfinite(score) or score < 0.0 or score > 1.0:
        raise SwebenchOpsError(
            "PY_G_121_SCORE_OUT_OF_RANGE",
            f"score must be finite and in [0, 1]: {key}",
            "Fix the source report before consuming it.",
            {"key": key, "value": raw},
        )
    return score


def reject_mounted_drive(field: str, path: Path) -> None:
    if str(path).startswith("/mnt/d/"):
        raise SwebenchOpsError(
            "PY_G_121_MNT_D_FORBIDDEN",
            f"{field} cannot point at mounted D drive: {path}",
            "Use aiwonder /zfs roots.",
            {"field": field, "path": str(path)},
        )


def write_json_checked(path: Path, value: dict[str, Any]) -> None:
    temp = path.with_name(f"{path.name}.tmp")
    temp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temp.replace(path)
    readback = json.loads(path.read_text(encoding="utf-8"))
    if readback != value:
        raise SwebenchOpsError(
            "PY_G_121_JSON_READBACK_MISMATCH",
            f"JSON readback mismatch at {path}",
            "Check filesystem integrity and concurrent writers.",
            {"path": str(path)},
        )
