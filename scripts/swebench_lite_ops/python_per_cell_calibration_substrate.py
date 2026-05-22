from __future__ import annotations

import json
import math
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_calibration_active_gate import (
    CellSelection,
    FittedPolicy,
    Row,
    calibrate,
    fit_policies,
    select_cells,
    split_report as policy_split_report,
    smooth_ece_regressions,
)
from .python_mendeleev_pair_table import WEAK_CELLS
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    HOLDOUT_KEY,
    load_diagnostic_rows,
)
from .python_per_cell_smooth_ece import metric_block, power_blockers, reject_mounted_drive


DEFAULT_MENDELEEV_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-124d-mendeleev-pair-table-readback/"
    "run-20260522T041330Z/mendeleev_pair_table_report.json"
)
DEFAULT_STRUCTURAL_HOLES_JSONL = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-124d-mendeleev-pair-table-readback/"
    "run-20260522T041330Z/structural_hole_rankings.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-124e-per-cell-calibration-substrate-readback")
REFERENCE_ACTIVE_CORRELATION = 0.9526350498
REFERENCE_ACTIVE_BRIER = 0.0231730901
REFERENCE_ACTIVE_SMOOTH_ECE = 0.0169885978
PRIMARY_HEADS = {"panel", "oracle", "failure_mode", "accuracy"}


def add_arguments(parser: Any) -> None:
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--mendeleev-report", type=Path, default=DEFAULT_MENDELEEV_REPORT)
    parser.add_argument("--structural-holes-jsonl", type=Path, default=DEFAULT_STRUCTURAL_HOLES_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--target-correlation", type=float, default=0.95)
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)
    parser.add_argument("--shrinkage-strength", type=float, default=60.0)
    parser.add_argument("--min-active-cell-rows", type=int, default=3)
    parser.add_argument("--max-structural-holes", type=int, default=20)


def command_python_per_cell_calibration_substrate(args: Any) -> dict[str, Any]:
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    mendeleev_report = args.mendeleev_report.expanduser().resolve(strict=False)
    structural_holes_jsonl = args.structural_holes_jsonl.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("mendeleev_report", mendeleev_report),
        ("structural_holes_jsonl", structural_holes_jsonl),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    validate_args(args)

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    mendeleev_payload = read_json_object(mendeleev_report, "mendeleev_report")
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_raw = load_diagnostic_rows(
        diagnostics,
        CALIBRATION_KEY,
        expected=int(args.expected_calibration_count),
    )
    holdout_raw = load_diagnostic_rows(
        diagnostics,
        HOLDOUT_KEY,
        expected=int(args.expected_holdout_count),
    )
    calibration_rows = to_policy_rows(calibration_raw)
    holdout_rows = to_policy_rows(holdout_raw)
    structural_holes = load_jsonl_objects(structural_holes_jsonl, int(args.max_structural_holes))

    policies = fit_policies(calibration_rows, shrinkage_strength=float(args.shrinkage_strength))
    selections = select_cells(
        calibration_rows,
        policies,
        target=float(args.target_correlation),
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
    )
    baseline_calibration = policy_split_report(calibration_rows)
    baseline_holdout = policy_split_report(holdout_rows)
    after_calibration = policy_split_report(calibration_rows, policies=policies, selections=selections)
    after_holdout = policy_split_report(holdout_rows, policies=policies, selections=selections)
    regressions = smooth_ece_regressions(
        before=baseline_calibration["perCell"],
        after=after_calibration["perCell"],
        budget=float(args.smooth_ece_regression_budget),
    )

    calibration_active_rows = apply_selected_policy(calibration_raw, calibration_rows, policies, selections)
    holdout_active_rows = apply_selected_policy(holdout_raw, holdout_rows, policies, selections)
    logit_ece = {
        "baselineCalibration": logit_ece_split(calibration_raw, "predicted_oracle_pass"),
        "baselineHoldout": logit_ece_split(holdout_raw, "predicted_oracle_pass"),
        "afterCalibration": logit_ece_split(calibration_active_rows, "substrate_score"),
        "afterHoldoutReportingOnly": logit_ece_split(holdout_active_rows, "substrate_score"),
    }
    admission_decisions = structural_hole_admission_decisions(
        structural_holes,
        selections,
        after_calibration,
        regressions,
        min_active_cell_rows=int(args.min_active_cell_rows),
    )
    promotion_gate = promotion_gate_report(after_holdout, regressions)

    selections_path = output_root / "per_cell_calibration_selections.jsonl"
    decisions_path = output_root / "calibration_key_admission_decisions.jsonl"
    holdout_rows_path = output_root / "substrate_holdout_active_rows.jsonl"
    report_path = output_root / "per_cell_calibration_substrate_report.json"
    write_jsonl(selections_path, selection_rows(selections))
    write_jsonl(decisions_path, admission_decisions)
    write_jsonl(holdout_rows_path, holdout_active_rows)

    report = {
        "artifactKind": "task_py_g_124e_per_cell_calibration_substrate",
        "checkedAt": utc_now(),
        "githubIssue": 586,
        "ok": True,
        "inputArtifacts": {
            "labelManifest": str(label_manifest),
            "labelManifestSha256": sha256_file(label_manifest),
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
            "mendeleevReport": str(mendeleev_report),
            "mendeleevReportSha256": sha256_file(mendeleev_report),
            "structuralHolesJsonl": str(structural_holes_jsonl),
            "structuralHolesJsonlSha256": sha256_file(structural_holes_jsonl),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "mendeleevSource": {
            "reportOk": mendeleev_payload.get("ok"),
            "reportKind": mendeleev_payload.get("artifactKind"),
            "topHoleCountRead": len(structural_holes),
            "sourceReportSha256": sha256_file(mendeleev_report),
        },
        "fitPartition": {
            "calibrationRowsUsed": len(calibration_rows),
            "holdoutRowsUsedForFitting": 0,
            "holdoutRowsReportingOnly": len(holdout_rows),
            "selectionSplit": "calibration_only",
            "thresholdSource": "score_sharpness_abstain_calibration_row_diagnostics",
        },
        "policy": {
            "candidatePolicies": sorted(policies),
            "targetCorrelation": float(args.target_correlation),
            "smoothEceRegressionBudget": float(args.smooth_ece_regression_budget),
            "shrinkageStrength": float(args.shrinkage_strength),
            "minActiveCellRows": int(args.min_active_cell_rows),
            "noHoldoutThresholdTuning": True,
            "noNewEmbedders": True,
            "noNewModelDownload": True,
            "noPredictorPromotion": True,
        },
        "selectedCells": {
            cell: selection_payload(selection)
            for cell, selection in sorted(selections.items())
        },
        "before": {
            "calibration": baseline_calibration,
            "holdout": baseline_holdout,
        },
        "after": {
            "calibration": after_calibration,
            "holdoutReportingOnly": after_holdout,
        },
        "logitSpaceEce": logit_ece,
        "smoothEceRegressions": regressions,
        "admissionSummary": admission_summary(admission_decisions),
        "weakCellSummary": weak_cell_summary(after_calibration, admission_decisions),
        "promotionGate": promotion_gate,
        "nextRecommendation": recommendation(promotion_gate, admission_decisions),
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "outputArtifacts": {
            "perCellCalibrationSelectionsJsonl": str(selections_path),
            "perCellCalibrationSelectionsSha256": sha256_file(selections_path),
            "calibrationKeyAdmissionDecisionsJsonl": str(decisions_path),
            "calibrationKeyAdmissionDecisionsSha256": sha256_file(decisions_path),
            "substrateHoldoutActiveRowsJsonl": str(holdout_rows_path),
            "substrateHoldoutActiveRowsSha256": sha256_file(holdout_rows_path),
        },
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def validate_args(args: Any) -> None:
    if int(args.expected_calibration_count) <= 0 or int(args.expected_holdout_count) <= 0:
        raise SwebenchOpsError(
            "PY_G_124E_BAD_SPLIT_COUNTS",
            "expected split counts must be positive",
            "Use the retained #392 split sizes unless the source artifact changed.",
            {
                "expected_calibration_count": args.expected_calibration_count,
                "expected_holdout_count": args.expected_holdout_count,
            },
        )
    if not 0.0 < float(args.target_correlation) <= 1.0:
        raise SwebenchOpsError(
            "PY_G_124E_BAD_TARGET",
            "target-correlation must be in (0, 1]",
            "Use the current ship-gate target.",
            {"target_correlation": args.target_correlation},
        )
    if float(args.smooth_ece_regression_budget) < 0.0 or float(args.shrinkage_strength) <= 0.0:
        raise SwebenchOpsError(
            "PY_G_124E_BAD_CALIBRATION_CONFIG",
            "smooth ECE budget must be non-negative and shrinkage strength positive",
            "Use bounded calibration substrate settings.",
            {
                "smooth_ece_regression_budget": args.smooth_ece_regression_budget,
                "shrinkage_strength": args.shrinkage_strength,
            },
        )


def to_policy_rows(rows: list[dict[str, Any]]) -> list[Row]:
    out = []
    for index, row in enumerate(rows):
        task_id = str(row["task_id"])
        repo = task_id.split("__", 1)[0] if "__" in task_id else "unknown"
        out.append(
            Row(
                row_index=int(row.get("row_index", index)),
                task_id=task_id,
                repo=repo,
                category=str(row["category"]),
                patch_path=str(row["patch_path"]),
                patch_sha256="unavailable_in_392_row_diagnostics",
                label=int(row["actual"]),
                score=float(row["predicted_oracle_pass"]),
            )
        )
    return out


def apply_selected_policy(
    raw_rows: list[dict[str, Any]],
    rows: list[Row],
    policies: dict[str, FittedPolicy],
    selections: dict[str, CellSelection],
) -> list[dict[str, Any]]:
    out = []
    for raw, row in zip(raw_rows, rows):
        cell = cell_key(row.category)
        selection = selections[cell]
        if selection.policy_name == "abstain_all":
            active = False
            score = row.score
        else:
            policy = policies[selection.policy_name]
            score = calibrate(policy, cell, row.score)
            active = selection.margin_threshold is not None and abs(score - 0.5) >= selection.margin_threshold
        if active:
            out.append(
                {
                    **raw,
                    "cell": cell,
                    "substrate_score": finite_float(score),
                    "substrate_policy": selection.policy_name,
                    "substrate_margin_threshold": selection.margin_threshold,
                    "substrate_active": True,
                }
            )
    return out


def structural_hole_admission_decisions(
    holes: list[dict[str, Any]],
    selections: dict[str, CellSelection],
    after_calibration: dict[str, Any],
    regressions: list[dict[str, Any]],
    *,
    min_active_cell_rows: int,
) -> list[dict[str, Any]]:
    regression_cells = {row["cell"] for row in regressions}
    out = []
    for rank, hole in enumerate(holes, 1):
        cell = str(hole.get("cell"))
        selection = selections.get(cell)
        cell_report = after_calibration.get("perCell", {}).get(cell, {})
        reasons = []
        if selection is None:
            reasons.append("CELL_SELECTION_MISSING")
        elif selection.policy_name == "abstain_all":
            reasons.append("CELL_ABSTAIN_ALL_UNDER_CALIBRATION")
        if cell in regression_cells:
            reasons.append("CELL_SMOOTH_ECE_REGRESSION")
        if int(cell_report.get("activeRowCount") or 0) < min_active_cell_rows:
            reasons.append("LOW_ACTIVE_CELL_ROWS")
        if int(hole.get("calibrationSupport") or 0) < 30:
            reasons.append("STRUCTURAL_HOLE_POWER_LT_30")
        if float(hole.get("expectedLiftScore") or 0.0) <= 0.0:
            reasons.append("NO_POSITIVE_EXPECTED_LIFT")
        head = str(hole.get("head"))
        if head not in PRIMARY_HEADS:
            reasons.append("NON_PRIMARY_HEAD_SHRINK_ONLY")
        if not reasons:
            decision = "eligible_for_584_diagnostic_retest"
        elif reasons == ["NON_PRIMARY_HEAD_SHRINK_ONLY"]:
            decision = "shrink_only_do_not_wire"
        else:
            decision = "reject_for_584_retest"
        out.append(
            {
                "rank": rank,
                "decision": decision,
                "reasons": reasons,
                "cell": cell,
                "head": head,
                "pairId": hole.get("pairId"),
                "expectedLiftScore": hole.get("expectedLiftScore"),
                "bestDependence": hole.get("bestDependence"),
                "calibrationSupport": hole.get("calibrationSupport"),
                "holdoutSupportReportingOnly": hole.get("holdoutSupportReportingOnly"),
                "selection": selection_payload(selection) if selection else None,
                "cellCalibrationAfter": cell_report,
            }
        )
    return out


def logit_ece_split(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    active_rows = [row for row in rows if row.get("substrate_active", row.get("active_after_abstain", True))]
    return {
        "fullOrSelected": logit_ece_metrics(rows, score_key),
        "active392OrSelected": logit_ece_metrics(active_rows, score_key),
        "perCell": {
            cell: logit_ece_metrics(cell_rows, score_key)
            for cell, cell_rows in sorted(group_raw_by_cell(rows).items())
        },
    }


def logit_ece_metrics(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    if not rows:
        return {"n": 0, "equalWidthLogitEce": None, "quantileLogitEce": None}
    labels = [int(row["actual"]) for row in rows]
    scores = [float(row[score_key]) for row in rows]
    return {
        "n": len(rows),
        "equalWidthLogitEce": logit_width_ece(labels, scores, bins=10),
        "quantileLogitEce": logit_quantile_ece(labels, scores, bins=10),
        "probabilitySpaceMetrics": metric_block(labels, scores),
        "powerBlockers": power_blockers(labels, scores),
    }


def logit_width_ece(labels: list[int], scores: list[float], *, bins: int) -> float | None:
    if not labels:
        return None
    logits = [clipped_logit(score) for score in scores]
    low, high = -12.0, 12.0
    total = 0.0
    for idx in range(bins):
        b_low = low + (high - low) * idx / bins
        b_high = low + (high - low) * (idx + 1) / bins
        selected = [
            (label, score)
            for label, score, value in zip(labels, scores, logits)
            if value >= b_low and (value < b_high or idx == bins - 1)
        ]
        if not selected:
            continue
        total += ece_bin_contribution(selected, len(labels))
    return finite_float(total)


def logit_quantile_ece(labels: list[int], scores: list[float], *, bins: int) -> float | None:
    if not labels:
        return None
    ordered = sorted(zip(labels, scores), key=lambda item: clipped_logit(item[1]))
    chunk = max(1, math.ceil(len(ordered) / bins))
    total = 0.0
    for start in range(0, len(ordered), chunk):
        selected = ordered[start : start + chunk]
        total += ece_bin_contribution(selected, len(labels))
    return finite_float(total)


def ece_bin_contribution(selected: list[tuple[int, float]], total_count: int) -> float:
    mean_conf = sum(score for _label, score in selected) / len(selected)
    mean_acc = sum(label for label, _score in selected) / len(selected)
    return (len(selected) / total_count) * abs(mean_conf - mean_acc)


def promotion_gate_report(after_holdout: dict[str, Any], regressions: list[dict[str, Any]]) -> dict[str, Any]:
    metrics = after_holdout.get("metrics") or {}
    corr = metrics.get("prediction_oracle_correlation")
    brier = metrics.get("brier")
    smooth = metrics.get("smooth_ece")
    checks = {
        "correlationBeatsOrEqualsReferenceActive392": corr is not None and float(corr) >= REFERENCE_ACTIVE_CORRELATION,
        "brierBeatsOrEqualsReferenceActive392": brier is not None and float(brier) <= REFERENCE_ACTIVE_BRIER,
        "smoothEceBeatsOrEqualsReferenceActive392": smooth is not None and float(smooth) <= REFERENCE_ACTIVE_SMOOTH_ECE,
        "noCalibrationSmoothEceRegressions": not regressions,
    }
    return {
        **checks,
        "promotionEligible": all(checks.values()),
        "referenceActive392": {
            "prediction_oracle_correlation": REFERENCE_ACTIVE_CORRELATION,
            "brier": REFERENCE_ACTIVE_BRIER,
            "smooth_ece": REFERENCE_ACTIVE_SMOOTH_ECE,
        },
        "candidateHoldoutMetricsReportingOnly": metrics,
    }


def admission_summary(decisions: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "decisionCounts": dict(Counter(row["decision"] for row in decisions)),
        "eligibleFor584DiagnosticRetestCount": sum(
            1 for row in decisions if row["decision"] == "eligible_for_584_diagnostic_retest"
        ),
        "topEligible": [
            row for row in decisions if row["decision"] == "eligible_for_584_diagnostic_retest"
        ][:12],
        "topRejected": [row for row in decisions if row["decision"] == "reject_for_584_retest"][:12],
    }


def weak_cell_summary(after_calibration: dict[str, Any], decisions: list[dict[str, Any]]) -> dict[str, Any]:
    out = {}
    for cell in sorted(WEAK_CELLS):
        out[cell] = {
            "afterCalibrationCell": after_calibration.get("perCell", {}).get(cell),
            "admissionDecisions": [row for row in decisions if row["cell"] == cell][:10],
        }
    return out


def recommendation(promotion_gate: dict[str, Any], decisions: list[dict[str, Any]]) -> dict[str, Any]:
    admits = [row for row in decisions if row["decision"] == "eligible_for_584_diagnostic_retest"]
    if promotion_gate["promotionEligible"] and admits:
        return {
            "recommendedAction": "unblock_584_with_admitted_structural_holes",
            "reason": "substrate itself is promotion-eligible and has safe structural holes for retest",
            "admittedCount": len(admits),
        }
    if admits:
        return {
            "recommendedAction": "keep_584_blocked_for_promotion_but_use_admitted_holes_for_diagnostic_retest",
            "reason": "substrate can rank safe retest candidates, but the substrate is not promotion-eligible yet",
            "admittedCount": len(admits),
            "promotionGate": promotion_gate,
        }
    return {
        "recommendedAction": "keep_584_blocked_and_repair_calibration_or_feature_surface_first",
        "reason": "no structural holes survived the per-cell calibration substrate",
        "promotionGate": promotion_gate,
    }


def selection_rows(selections: dict[str, CellSelection]) -> list[dict[str, Any]]:
    return [selection_payload(selection) for _cell, selection in sorted(selections.items())]


def selection_payload(selection: CellSelection) -> dict[str, Any]:
    return {
        "cell": selection.cell,
        "policy": selection.policy_name,
        "coverage": finite_float(selection.coverage),
        "marginThreshold": selection.margin_threshold,
        "reason": selection.reason,
        "calibrationMetrics": selection.calibration_metrics,
    }


def load_jsonl_objects(path: Path, limit: int) -> list[dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124E_JSONL_MISSING",
            f"required structural-hole JSONL missing: {path}",
            "Run #585 Mendeleev pair table first.",
            {"path": str(path)},
        )
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            value = json.loads(line)
            if not isinstance(value, dict):
                raise SwebenchOpsError(
                    "PY_G_124E_JSONL_OBJECT_REQUIRED",
                    "structural-hole JSONL row must be an object",
                    "Regenerate #585 evidence.",
                    {"path": str(path), "line": line_number},
                )
            rows.append(value)
            if len(rows) >= limit:
                break
    return rows


def group_raw_by_cell(rows: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[str(row.get("cell") or f"{row['category']}::python")].append(row)
    return grouped


def cell_key(category: str) -> str:
    return f"{category}::python"


def clipped_logit(score: float) -> float:
    score = min(1.0 - 1.0e-9, max(1.0e-9, float(score)))
    return max(-12.0, min(12.0, math.log(score / (1.0 - score))))
