from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    HOLDOUT_KEY,
    attach_pair_features,
    effect_for_feature,
    grouped_by_cell,
    load_code_states,
    load_diagnostic_rows,
    pair_id_from_feature,
)
from .python_per_cell_smooth_ece import metric_block, power_blockers, reject_mounted_drive


DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-124b-conditional-pair-admission-fsv")
WEAK_CELLS = ("delete_test_call::python", "off_by_one::python", "subtle_flip::python")


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--min-calibration-support", type=int, default=5)
    parser.add_argument("--max-feature-fraction", type=float, default=0.90)
    parser.add_argument("--max-selected-features", type=int, default=40)
    parser.add_argument("--min-calibration-active-count", type=int, default=180)


def command_python_pair_conditional_admission_probe(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.min_calibration_support <= 0 or args.max_selected_features <= 0:
        raise SwebenchOpsError(
            "PY_G_124B_ADMISSION_INVALID_CONFIG",
            "support and feature limits must be positive",
            "Use positive admission-probe configuration.",
            {"min_calibration_support": args.min_calibration_support, "max_selected_features": args.max_selected_features},
        )
    if not (0.0 < float(args.max_feature_fraction) <= 1.0):
        raise SwebenchOpsError(
            "PY_G_124B_ADMISSION_INVALID_FRACTION",
            "max-feature-fraction must be in (0, 1]",
            "Use a valid fraction cap.",
            {"max_feature_fraction": args.max_feature_fraction},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = attach_pair_features(
        load_diagnostic_rows(diagnostics, CALIBRATION_KEY, expected=int(args.expected_calibration_count)),
        states,
    )
    holdout = attach_pair_features(
        load_diagnostic_rows(diagnostics, HOLDOUT_KEY, expected=int(args.expected_holdout_count)),
        states,
    )

    selected_features = select_pair_weights(
        calibration,
        min_support=int(args.min_calibration_support),
        max_fraction=float(args.max_feature_fraction),
        max_features=int(args.max_selected_features),
    )
    scored_calibration = score_rows(calibration, selected_features)
    scored_holdout = score_rows(holdout, selected_features)
    policies = evaluate_policy_grid(scored_calibration, int(args.min_calibration_active_count))
    if not policies:
        raise SwebenchOpsError(
            "PY_G_124B_ADMISSION_NO_POLICY",
            "no admission policy satisfied calibration constraints",
            "Lower min-calibration-active-count only after recording why the power floor changed.",
            {"min_calibration_active_count": args.min_calibration_active_count},
        )
    chosen = policies[0]
    calibration_application = apply_policy(scored_calibration, chosen["policy"])
    holdout_application = apply_policy(scored_holdout, chosen["policy"])
    baseline_calibration = split_report_with_cells(scored_calibration, "active_after_abstain")
    baseline_holdout = split_report_with_cells(scored_holdout, "active_after_abstain")
    calibration_delta = report_delta(baseline_calibration, calibration_application["report"])
    holdout_delta = report_delta(baseline_holdout, holdout_application["report"])

    selected_path = output_root / "selected_pair_admission_features.jsonl"
    policies_path = output_root / "pair_admission_policy_grid.jsonl"
    calibration_rows_path = output_root / "pair_admission_calibration_rows.jsonl"
    holdout_rows_path = output_root / "pair_admission_holdout_rows.jsonl"
    report_path = output_root / "pair_conditional_admission_probe_report.json"
    write_jsonl(selected_path, selected_features)
    write_jsonl(policies_path, policies)
    write_jsonl(calibration_rows_path, row_projection(calibration_application["rows"]))
    write_jsonl(holdout_rows_path, row_projection(holdout_application["rows"]))

    report = {
        "artifactKind": "task_py_g_124b_conditional_pair_admission_probe",
        "checkedAt": utc_now(),
        "githubIssue": 583,
        "ok": True,
        "inputArtifacts": {
            "codeStatesJsonl": str(code_states_jsonl),
            "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
            "labelManifest": str(label_manifest),
            "labelManifestSha256": sha256_file(label_manifest),
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "selectionPolicy": {
            "selectionSplit": "calibration_only",
            "holdoutUse": "reporting_only",
            "minCalibrationSupport": int(args.min_calibration_support),
            "maxFeatureFraction": float(args.max_feature_fraction),
            "maxSelectedFeatures": int(args.max_selected_features),
            "minCalibrationActiveCount": int(args.min_calibration_active_count),
            "scoreChange": "none_admission_only",
        },
        "selectedFeatureSummary": {
            "count": len(selected_features),
            "features": selected_features[:20],
        },
        "chosenPolicy": chosen,
        "baseline": {
            "calibration": baseline_calibration,
            "holdout": baseline_holdout,
        },
        "afterPolicy": {
            "calibration": calibration_application["report"],
            "holdoutReportingOnly": holdout_application["report"],
        },
        "deltaVsBaseline": {
            "calibration": calibration_delta,
            "holdoutReportingOnly": holdout_delta,
        },
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "policy": {
            "noNewEmbedders": True,
            "noFlatVectorShortcut": True,
            "preservesSlotIdentity": True,
            "noHoldoutTuning": True,
        },
        "outputArtifacts": {
            "selectedPairAdmissionFeaturesJsonl": str(selected_path),
            "selectedPairAdmissionFeaturesSha256": sha256_file(selected_path),
            "pairAdmissionPolicyGridJsonl": str(policies_path),
            "pairAdmissionPolicyGridSha256": sha256_file(policies_path),
            "pairAdmissionCalibrationRowsJsonl": str(calibration_rows_path),
            "pairAdmissionCalibrationRowsSha256": sha256_file(calibration_rows_path),
            "pairAdmissionHoldoutRowsJsonl": str(holdout_rows_path),
            "pairAdmissionHoldoutRowsSha256": sha256_file(holdout_rows_path),
        },
        "recommendation": recommendation(calibration_delta, holdout_delta, holdout_application["report"]),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def select_pair_weights(
    rows: list[dict[str, Any]],
    *,
    min_support: int,
    max_fraction: float,
    max_features: int,
) -> list[dict[str, Any]]:
    features = sorted({feature for row in rows for feature in row["pair_features"]})
    selected = []
    for feature in features:
        effect = effect_for_feature(rows, feature)
        support = int(effect["support"])
        fraction = float(effect["nonzeroFraction"] or 0.0)
        if support < min_support or fraction > max_fraction or fraction <= 0.0:
            continue
        brier_delta = effect.get("brierDeltaPresentMinusAbsent")
        brier_corr = effect.get("brierCorrelation")
        if brier_delta is None or brier_corr is None or abs(float(brier_corr)) < 0.02:
            continue
        magnitude = abs(float(brier_corr)) * math.sqrt(support / max(1, len(rows)))
        selected.append(
            {
                "feature": feature,
                "pairId": pair_id_from_feature(feature),
                "featureKind": feature.split("::", 1)[0],
                "support": support,
                "nonzeroFraction": finite_float(fraction),
                "riskWeight": finite_float(max(0.0, float(brier_delta)) * magnitude),
                "safetyWeight": finite_float(max(0.0, -float(brier_delta)) * magnitude),
                "brierDeltaPresentMinusAbsent": brier_delta,
                "brierCorrelation": brier_corr,
                "activeCorrelation": effect.get("activeCorrelation"),
                "labelCorrelation": effect.get("labelCorrelation"),
            }
        )
    selected.sort(
        key=lambda row: (
            -(float(row["riskWeight"]) + float(row["safetyWeight"])),
            row["feature"],
        )
    )
    return selected[:max_features]


def score_rows(rows: list[dict[str, Any]], features: list[dict[str, Any]]) -> list[dict[str, Any]]:
    feature_map = {row["feature"]: row for row in features}
    out = []
    for row in rows:
        risk = 0.0
        safety = 0.0
        fired = []
        for feature in row["pair_features"]:
            weights = feature_map.get(feature)
            if not weights:
                continue
            risk += float(weights["riskWeight"])
            safety += float(weights["safetyWeight"])
            fired.append(feature)
        out.append(
            {
                **row,
                "pair_admission_risk": finite_float(risk),
                "pair_admission_safety": finite_float(safety),
                "pair_admission_margin": finite_float(safety - risk),
                "selected_pair_feature_count": len(fired),
                "selected_pair_feature_sample": sorted(fired)[:12],
            }
        )
    return out


def evaluate_policy_grid(rows: list[dict[str, Any]], min_active_count: int) -> list[dict[str, Any]]:
    risk_values = [float(row["pair_admission_risk"]) for row in rows]
    safety_values = [float(row["pair_admission_safety"]) for row in rows]
    risk_thresholds = quantiles(risk_values, (0.50, 0.60, 0.70, 0.80, 0.90, 0.95, 1.0))
    admit_thresholds = quantiles(risk_values, (0.05, 0.10, 0.20, 0.30, 0.40, 0.50))
    safety_thresholds = quantiles(safety_values, (0.0, 0.25, 0.50, 0.75))
    policies = []
    for risk_threshold in risk_thresholds:
        for admit_threshold in admit_thresholds:
            for safety_threshold in safety_thresholds:
                policy = {
                    "activeRiskMax": risk_threshold,
                    "abstainAdmitRiskMax": admit_threshold,
                    "abstainAdmitSafetyMin": safety_threshold,
                }
                applied = apply_policy(rows, policy)
                active_n = applied["report"]["rowCounts"]["activePolicy"]
                if active_n < min_active_count:
                    continue
                policies.append(
                    {
                        "policy": policy,
                        "selectionObjective": selection_objective(applied["report"]),
                        "calibrationReport": applied["report"],
                    }
                )
    policies.sort(key=lambda row: (-float(row["selectionObjective"]), json.dumps(row["policy"], sort_keys=True)))
    return policies[:40]


def apply_policy(rows: list[dict[str, Any]], policy: dict[str, float]) -> dict[str, Any]:
    projected = []
    for row in rows:
        baseline_active = bool(row["active_after_abstain"])
        risk = float(row["pair_admission_risk"])
        safety = float(row["pair_admission_safety"])
        active = (
            (baseline_active and risk <= float(policy["activeRiskMax"]))
            or (
                not baseline_active
                and risk <= float(policy["abstainAdmitRiskMax"])
                and safety >= float(policy["abstainAdmitSafetyMin"])
            )
        )
        projected.append({**row, "active_after_pair_policy": active})
    return {"rows": projected, "report": split_report_with_cells(projected, "active_after_pair_policy")}


def split_report_with_cells(rows: list[dict[str, Any]], active_key: str) -> dict[str, Any]:
    active = [row for row in rows if row[active_key] is True]
    abstain = [row for row in rows if row[active_key] is False]
    return {
        "full": metrics(rows),
        "activePolicy": metrics(active),
        "abstainPool": metrics(abstain),
        "rowCounts": {"full": len(rows), "activePolicy": len(active), "abstainPool": len(abstain)},
        "activeCoverage": finite_float(len(active) / len(rows)) if rows else None,
        "perCellActive": per_cell_metrics(active),
        "perCellAbstain": per_cell_metrics(abstain),
    }


def metrics(rows: list[dict[str, Any]]) -> dict[str, Any]:
    block = metric_block([row["actual"] for row in rows], [row["predicted_oracle_pass"] for row in rows])
    block["powerBlockers"] = power_blockers(
        [row["actual"] for row in rows],
        [row["predicted_oracle_pass"] for row in rows],
    )
    return block


def per_cell_metrics(rows: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell, cell_rows in grouped_by_cell(rows).items():
        out[cell] = metrics(cell_rows)
    return dict(sorted(out.items()))


def selection_objective(report: dict[str, Any]) -> float:
    active = report["activePolicy"]
    corr = active.get("prediction_oracle_correlation")
    if corr is None:
        return -1.0e9
    smooth = active.get("smooth_ece") or 1.0
    brier = active.get("brier") or 1.0
    weak_corrs = [
        report["perCellActive"].get(cell, {}).get("prediction_oracle_correlation")
        for cell in WEAK_CELLS
    ]
    weak_corr_values = [float(value) for value in weak_corrs if value is not None]
    weak_mean = sum(weak_corr_values) / len(weak_corr_values) if weak_corr_values else 0.0
    return finite_float(float(corr) + 0.25 * weak_mean + 0.10 * float(report["activeCoverage"]) - 2.0 * smooth - brier)


def report_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    return {
        "activeRowDelta": after["rowCounts"]["activePolicy"] - before["rowCounts"]["activePolicy"],
        "abstainRowDelta": after["rowCounts"]["abstainPool"] - before["rowCounts"]["abstainPool"],
        "activeCorrDelta": metric_delta(before["activePolicy"], after["activePolicy"], "prediction_oracle_correlation"),
        "activeBrierDelta": metric_delta(before["activePolicy"], after["activePolicy"], "brier"),
        "activeSmoothEceDelta": metric_delta(before["activePolicy"], after["activePolicy"], "smooth_ece"),
        "weakCellActiveCorrAfter": {
            cell: after["perCellActive"].get(cell, {}).get("prediction_oracle_correlation")
            for cell in WEAK_CELLS
        },
    }


def metric_delta(before: dict[str, Any], after: dict[str, Any], key: str) -> float | None:
    left = before.get(key)
    right = after.get(key)
    if left is None or right is None:
        return None
    return finite_float(float(right) - float(left))


def row_projection(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        out.append(
            {
                "task_id": row["task_id"],
                "category": row["category"],
                "actual": row["actual"],
                "predicted_oracle_pass": row["predicted_oracle_pass"],
                "baseline_active": row["active_after_abstain"],
                "pair_policy_active": row["active_after_pair_policy"],
                "pair_admission_risk": row["pair_admission_risk"],
                "pair_admission_safety": row["pair_admission_safety"],
                "selected_pair_feature_count": row["selected_pair_feature_count"],
                "selected_pair_feature_sample": row["selected_pair_feature_sample"],
            }
        )
    return out


def recommendation(
    calibration_delta: dict[str, Any],
    holdout_delta: dict[str, Any],
    holdout: dict[str, Any],
) -> dict[str, Any]:
    active_corr = holdout["activePolicy"].get("prediction_oracle_correlation")
    active_smooth = holdout["activePolicy"].get("smooth_ece")
    changed_rows = bool(
        calibration_delta.get("activeRowDelta") != 0
        or holdout_delta.get("activeRowDelta") != 0
        or calibration_delta.get("abstainRowDelta") != 0
        or holdout_delta.get("abstainRowDelta") != 0
    )
    if not changed_rows:
        action = "retain_negative_no_pair_admission_policy_beat_baseline"
    elif active_corr is not None and active_corr >= 0.95 and (active_smooth is None or active_smooth <= 0.024):
        action = "consider_named_pair_admission_wiring_under_392"
    else:
        action = "retain_as_diagnostic_or_refine_with_pair_interactions"
    return {
        "recommendedAction": action,
        "changedRows": changed_rows,
        "calibrationDeltaSummary": calibration_delta,
        "holdoutActiveCorr": active_corr,
        "holdoutActiveSmoothEce": active_smooth,
        "holdoutActiveRowCount": holdout["rowCounts"]["activePolicy"],
        "holdoutDeltaSummary": holdout_delta,
    }


def quantiles(values: list[float], qs: tuple[float, ...]) -> list[float]:
    cleaned = sorted({finite_float(value) for value in values if math.isfinite(float(value))})
    if not cleaned:
        return [0.0]
    out = []
    for q in qs:
        idx = min(len(cleaned) - 1, max(0, int(round(q * (len(cleaned) - 1)))))
        out.append(cleaned[idx])
    return sorted(set(out))
