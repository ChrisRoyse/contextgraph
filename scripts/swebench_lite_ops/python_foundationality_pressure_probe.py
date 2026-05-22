from __future__ import annotations

import json
import math
from collections import defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-392-temporal-trajectory-features-readback/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_readback.json"
)
DEFAULT_TRACIN_REPORT = Path(
    "/zfs/archive/contextgraph/state/tracin/post-fix-real-input-v2-rerun2/"
    "tracin_abstain_pool_report.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-125-foundationality-cp-entropy-readback"
)

CALIBRATION_KEY = "score_sharpness_abstain_calibration_row_diagnostics"
HOLDOUT_KEY = "score_sharpness_abstain_holdout_row_diagnostics"


def add_arguments(parser: Any) -> None:
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--tracin-report", type=Path, default=DEFAULT_TRACIN_REPORT)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--epochs", type=int, default=1800)
    parser.add_argument("--learning-rate", type=float, default=0.08)
    parser.add_argument("--l2", type=float, default=0.015)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)


def command_python_foundationality_pressure_probe(args: Any) -> dict[str, Any]:
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    tracin_report = args.tracin_report.expanduser().resolve(strict=False)
    repo_root = args.repo_root.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("row_diagnostics_report", row_diagnostics_report),
        ("tracin_report", tracin_report),
        ("repo_root", repo_root),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.epochs <= 0 or args.learning_rate <= 0.0 or args.l2 < 0.0:
        raise SwebenchOpsError(
            "PY_G_125_FOUNDATIONALITY_OPTIMIZER_INVALID",
            "optimizer parameters must be positive, with non-negative L2",
            "Use deterministic calibration-only optimizer settings.",
            {"epochs": args.epochs, "learning_rate": args.learning_rate, "l2": args.l2},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    source = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    tracin = read_json_object(tracin_report, "tracin_report")
    calibration = load_rows(source, CALIBRATION_KEY, args.expected_calibration_count)
    holdout = load_rows(source, HOLDOUT_KEY, args.expected_holdout_count)
    norm = fit_foundationality_normalizer(calibration)

    candidates = []
    for name, mode in (
        ("base_logit_calibration", "base"),
        ("foundationality_global", "global"),
        ("foundationality_per_cell", "per_cell"),
    ):
        model = fit_model(
            calibration,
            feature_mode=mode,
            norm=norm,
            epochs=int(args.epochs),
            learning_rate=float(args.learning_rate),
            l2=float(args.l2),
        )
        candidates.append(score_candidate(name, mode, model, calibration, holdout, norm))

    baseline = {
        "calibration": split_report(calibration, score_key="predicted_oracle_pass"),
        "holdout": split_report(holdout, score_key="predicted_oracle_pass"),
    }
    wiring = rust_wiring_audit(repo_root)
    decision = promotion_decision(
        baseline["holdout"]["full"],
        candidates,
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
    )

    row_path = output_root / "foundationality_pressure_candidate_rows.jsonl"
    write_jsonl(row_path, candidate_rows(candidates))
    report_path = output_root / "foundationality_pressure_probe_report.json"
    report = {
        "artifactKind": "task_py_g_125_foundationality_pressure_probe",
        "checkedAt": utc_now(),
        "githubIssue": 579,
        "ok": True,
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "tracinReport": str(tracin_report),
        "tracinReportSha256": sha256_file(tracin_report),
        "splitCounts": {"calibration": len(calibration), "holdout": len(holdout)},
        "foundationalityNormalizer": norm,
        "rustWiringAudit": wiring,
        "tracinFoundationality": tracin.get("foundationality"),
        "tracinRecommendation": tracin.get("recommendation"),
        "baseline": baseline,
        "candidates": candidates,
        "decision": decision,
        "candidateRowsJsonl": str(row_path),
        "candidateRowsSha256": sha256_file(row_path),
        "reportPath": str(report_path),
        "noHoldoutSelectedThresholds": True,
        "usesNewEmbedder": False,
        "usesMountedDrive": False,
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_rows(payload: dict[str, Any], key: str, expected: int) -> list[dict[str, Any]]:
    raw = payload.get(key)
    if not isinstance(raw, list):
        raise SwebenchOpsError(
            "PY_G_125_DIAGNOSTIC_ROWS_MISSING",
            f"{key} missing or not an array",
            "Use the retained #392 active/abstain row diagnostics artifact.",
            {"key": key},
        )
    if len(raw) != expected:
        raise SwebenchOpsError(
            "PY_G_125_DIAGNOSTIC_ROW_COUNT_MISMATCH",
            f"{key} row count mismatch",
            "Use the current #392 report or update the expected count after proving the split changed.",
            {"key": key, "expected": expected, "actual": len(raw)},
        )
    return [normalize_row(row, key, index) for index, row in enumerate(raw)]


def normalize_row(row: Any, key: str, index: int) -> dict[str, Any]:
    if not isinstance(row, dict):
        raise SwebenchOpsError(
            "PY_G_125_DIAGNOSTIC_ROW_OBJECT_REQUIRED",
            f"{key}[{index}] must be an object",
            "Fix the source row diagnostics before probing foundationality.",
            {"key": key, "index": index, "actual": type(row).__name__},
        )
    task_id = required_string(row, "task_id")
    category = required_string(row, "category")
    score = required_float(row, "predicted_oracle_pass")
    actual = 1 if required_bool(row, "actual_oracle_pass") else 0
    active = row.get("active_after_abstain")
    if not isinstance(active, bool):
        raise SwebenchOpsError(
            "PY_G_125_ACTIVE_FLAG_REQUIRED",
            f"{key}[{index}] active_after_abstain must be boolean",
            "Use score_sharpness_abstain row diagnostics, not legal_best rows.",
            {"key": key, "index": index, "actual": active},
        )
    out = {
        "row_index": int(row.get("row_index", index)),
        "task_id": task_id,
        "category": category,
        "patch_path": required_string(row, "patch_path"),
        "actual": actual,
        "predicted_oracle_pass": clamp_prob(score),
        "active_after_abstain": active,
        "foundationality_proxy": foundationality_proxy(row),
        "cause_feature_nonzero_count": numeric_or_zero(row.get("cause_feature_nonzero_count")),
        "cause_feature_large_magnitude_count": numeric_or_zero(row.get("cause_feature_large_magnitude_count")),
        "evidence_key_count": numeric_or_zero(row.get("evidence_key_count")),
    }
    return out


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "PY_G_125_STRING_REQUIRED",
            f"{key} must be a non-empty string",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PY_G_125_BOOL_REQUIRED",
            f"{key} must be boolean",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return value


def required_float(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise SwebenchOpsError(
            "PY_G_125_FLOAT_REQUIRED",
            f"{key} must be a finite number",
            "Fix the source artifact schema before using it for gate evidence.",
            {"key": key, "actual": value},
        )
    return float(value)


def numeric_or_zero(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        return 0.0
    return float(value)


def foundationality_proxy(row: dict[str, Any]) -> float:
    evidence = numeric_or_zero(row.get("evidence_key_count"))
    nonzero = numeric_or_zero(row.get("cause_feature_nonzero_count"))
    large = numeric_or_zero(row.get("cause_feature_large_magnitude_count"))
    return finite_float(
        math.log1p(max(0.0, evidence))
        + 0.5 * math.log1p(max(0.0, nonzero))
        + 0.75 * math.log1p(max(0.0, large))
    )


def fit_foundationality_normalizer(rows: list[dict[str, Any]]) -> dict[str, float]:
    values = [float(row["foundationality_proxy"]) for row in rows]
    mean = sum(values) / len(values)
    variance = sum((value - mean) ** 2 for value in values) / max(1, len(values) - 1)
    std = math.sqrt(max(variance, 1.0e-12))
    return {"mean": finite_float(mean), "std": finite_float(std)}


def fit_model(
    rows: list[dict[str, Any]],
    *,
    feature_mode: str,
    norm: dict[str, float],
    epochs: int,
    learning_rate: float,
    l2: float,
) -> dict[str, Any]:
    names = feature_names(rows, feature_mode)
    weights = {name: 0.0 for name in names}
    for _ in range(epochs):
        grad = {name: 0.0 for name in names}
        for row in rows:
            features = feature_vector(row, feature_mode, norm, names)
            pred = sigmoid(sum(weights[name] * features.get(name, 0.0) for name in names))
            err = pred - float(row["actual"])
            for name in names:
                grad[name] += err * features.get(name, 0.0)
        scale = 1.0 / len(rows)
        for name in names:
            penalty = 0.0 if name == "intercept" else l2 * weights[name]
            weights[name] -= learning_rate * (grad[name] * scale + penalty)
    return {
        "featureMode": feature_mode,
        "features": names,
        "weights": {name: finite_float(value) for name, value in weights.items()},
    }


def feature_names(rows: list[dict[str, Any]], feature_mode: str) -> list[str]:
    names = ["intercept", "base_logit"]
    if feature_mode in {"global", "per_cell"}:
        names.append("foundationality_proxy_z")
    if feature_mode == "per_cell":
        for category in sorted({str(row["category"]) for row in rows}):
            names.append(f"foundationality_proxy_z::{category}")
    return names


def feature_vector(
    row: dict[str, Any],
    feature_mode: str,
    norm: dict[str, float],
    names: list[str],
) -> dict[str, float]:
    score = clamp_prob(float(row["predicted_oracle_pass"]))
    z = (float(row["foundationality_proxy"]) - norm["mean"]) / norm["std"]
    features = {"intercept": 1.0, "base_logit": logit(score)}
    if feature_mode in {"global", "per_cell"}:
        features["foundationality_proxy_z"] = z
    if feature_mode == "per_cell":
        key = f"foundationality_proxy_z::{row['category']}"
        if key in names:
            features[key] = z
    return features


def score_candidate(
    name: str,
    mode: str,
    model: dict[str, Any],
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    norm: dict[str, float],
) -> dict[str, Any]:
    names = list(model["features"])
    scored_calibration = [with_candidate_score(row, model, mode, norm, names) for row in calibration]
    scored_holdout = [with_candidate_score(row, model, mode, norm, names) for row in holdout]
    return {
        "name": name,
        "mode": mode,
        "model": model,
        "calibration": split_report(scored_calibration, score_key="candidate_score"),
        "holdout": split_report(scored_holdout, score_key="candidate_score"),
    }


def with_candidate_score(
    row: dict[str, Any],
    model: dict[str, Any],
    mode: str,
    norm: dict[str, float],
    names: list[str],
) -> dict[str, Any]:
    features = feature_vector(row, mode, norm, names)
    weights = model["weights"]
    score = sigmoid(sum(float(weights[name]) * features.get(name, 0.0) for name in names))
    return {**row, "candidate_score": finite_float(score)}


def split_report(rows: list[dict[str, Any]], *, score_key: str) -> dict[str, Any]:
    active = [row for row in rows if row["active_after_abstain"] is True]
    abstain = [row for row in rows if row["active_after_abstain"] is False]
    return {
        "full": metrics(rows, score_key),
        "active392": metrics(active, score_key),
        "abstainPool": metrics(abstain, score_key),
        "perCell": per_cell_metrics(rows, score_key),
        "rowCounts": {"full": len(rows), "active392": len(active), "abstainPool": len(abstain)},
    }


def metrics(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    if not rows:
        return {
            "n": 0,
            "prediction_oracle_correlation": None,
            "brier": None,
            "smooth_ece": None,
            "accuracy_at_half": None,
        }
    return metric_block(
        [int(row["actual"]) for row in rows],
        [float(row[score_key]) for row in rows],
    )


def per_cell_metrics(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[f"{row['category']}::python"].append(row)
    return {cell: metrics(cell_rows, score_key) for cell, cell_rows in sorted(grouped.items())}


def promotion_decision(
    baseline_full: dict[str, Any],
    candidates: list[dict[str, Any]],
    *,
    smooth_ece_regression_budget: float,
) -> dict[str, Any]:
    blockers = []
    baseline_corr = baseline_full.get("prediction_oracle_correlation")
    baseline_brier = baseline_full.get("brier")
    baseline_smooth = baseline_full.get("smooth_ece")
    scored = []
    for candidate in candidates:
        full = candidate["holdout"]["full"]
        delta = metric_delta(baseline_full, full)
        promotes = (
            baseline_corr is not None
            and full.get("prediction_oracle_correlation") is not None
            and full["prediction_oracle_correlation"] > baseline_corr
            and baseline_brier is not None
            and full.get("brier") is not None
            and full["brier"] <= baseline_brier
            and baseline_smooth is not None
            and full.get("smooth_ece") is not None
            and full["smooth_ece"] <= baseline_smooth + smooth_ece_regression_budget
        )
        scored.append({"name": candidate["name"], "holdoutDelta": delta, "promotable": bool(promotes)})
    if not any(item["promotable"] for item in scored):
        blockers.append("NO_FOUNDATIONALITY_CANDIDATE_IMPROVED_HOLDOUT_CORR_WITHOUT_BRIER_SMOOTHECE_REGRESSION")
    return {
        "promotableCandidateFound": any(item["promotable"] for item in scored),
        "smoothEceRegressionBudget": smooth_ece_regression_budget,
        "candidateDecisions": scored,
        "blockers": blockers,
    }


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    return {
        "prediction_oracle_correlation": none_delta(
            before.get("prediction_oracle_correlation"),
            after.get("prediction_oracle_correlation"),
        ),
        "brier": none_delta(before.get("brier"), after.get("brier")),
        "smooth_ece": none_delta(before.get("smooth_ece"), after.get("smooth_ece")),
        "accuracy_at_half": none_delta(before.get("accuracy_at_half"), after.get("accuracy_at_half")),
    }


def rust_wiring_audit(repo_root: Path) -> dict[str, Any]:
    path = repo_root / "crates/context-graph-mejepa/src/head_projection.rs"
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_125_HEAD_PROJECTION_SOURCE_MISSING",
            f"head projection source missing: {path}",
            "Run from the active ContextGraph source mirror.",
            {"path": str(path)},
        )
    text = path.read_text(encoding="utf-8")
    oracle_block = arm_block(text, "HeadId::Oracle", "HeadId::FailureMode")
    failure_block = arm_block(text, "HeadId::FailureMode", "HeadId::EdgeCase")
    return {
        "sourcePath": str(path),
        "sourceSha256": sha256_file(path),
        "scalarFeatureDoorPresent": "pub scalar_features:" in text
        and "HeadProjectionFeatureSource::Scalar" in text,
        "oracleSpecUsesFoundationality": "foundationality" in oracle_block,
        "failureModeSpecUsesFoundationality": "foundationality" in failure_block,
        "oracleSpecBlock": oracle_block.strip(),
        "failureModeSpecBlock": failure_block.strip(),
    }


def arm_block(text: str, start: str, end: str) -> str:
    start_idx = text.find(start)
    end_idx = text.find(end, start_idx + len(start)) if start_idx >= 0 else -1
    if start_idx < 0 or end_idx < 0:
        return ""
    return text[start_idx:end_idx]


def candidate_rows(candidates: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for candidate in candidates:
        rows.append(
            {
                "candidate": candidate["name"],
                "mode": candidate["mode"],
                "holdoutFull": candidate["holdout"]["full"],
                "holdoutActive392": candidate["holdout"]["active392"],
                "holdoutAbstainPool": candidate["holdout"]["abstainPool"],
            }
        )
    return rows


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return finite_float(float(after) - float(before))


def clamp_prob(value: float) -> float:
    return min(0.999, max(0.001, finite_float(value)))


def logit(value: float) -> float:
    value = clamp_prob(value)
    return math.log(value / (1.0 - value))


def sigmoid(value: float) -> float:
    if value >= 0.0:
        z = math.exp(-value)
        return 1.0 / (1.0 + z)
    z = math.exp(value)
    return z / (1.0 + z)
