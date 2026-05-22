from __future__ import annotations

import math
from collections import defaultdict
from itertools import combinations
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    EXPECTED_DOCUMENTED_SLOTS,
    HOLDOUT_KEY,
    SCALAR_FEATURES,
    derived_pair_records,
    load_code_states,
    load_diagnostic_rows,
    require_state,
    slot_features,
    target_workspace_state,
)
from .python_per_cell_smooth_ece import (
    metric_block,
    pearson_float_or_none,
    power_blockers,
    rank_values,
    reject_mounted_drive,
)


DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-126-per-cell-axis-rotation-fsv")
WEAK_CELLS = ("delete_test_call::python", "off_by_one::python", "subtle_flip::python")
REFERENCE_ACTIVE_CORRELATION = 0.9526350498
REFERENCE_ACTIVE_BRIER = 0.0231730901
REFERENCE_ACTIVE_SMOOTH_ECE = 0.0169885978


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--min-cell-support", type=int, default=30)
    parser.add_argument("--association-threshold", type=float, default=0.35)


def command_python_axis_rotation_diagnostic(args: Any) -> dict[str, Any]:
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
    if int(args.min_cell_support) <= 0:
        raise SwebenchOpsError(
            "PY_G_126_INVALID_MIN_CELL_SUPPORT",
            "min-cell-support must be positive",
            "Use a positive min-cell-support value.",
            {"min_cell_support": args.min_cell_support},
        )
    if float(args.association_threshold) < 0.0 or float(args.association_threshold) > 1.0:
        raise SwebenchOpsError(
            "PY_G_126_INVALID_ASSOCIATION_THRESHOLD",
            "association-threshold must be in [0, 1]",
            "Use a bounded absolute Pearson threshold.",
            {"association_threshold": args.association_threshold},
        )

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = load_diagnostic_rows(
        diagnostics,
        CALIBRATION_KEY,
        expected=int(args.expected_calibration_count),
    )
    holdout = load_diagnostic_rows(
        diagnostics,
        HOLDOUT_KEY,
        expected=int(args.expected_holdout_count),
    )

    feature_schema = build_feature_schema(states)
    calibration_vectors = materialize_vectors(calibration, states, feature_schema)
    holdout_vectors = materialize_vectors(holdout, states, feature_schema)
    score_axis = covariance_direction(calibration_vectors, [row["predicted_oracle_pass"] for row in calibration])
    label_axis = covariance_direction(calibration_vectors, [float(row["actual"]) for row in calibration])
    if vector_norm(score_axis) <= 1.0e-12:
        raise SwebenchOpsError(
            "PY_G_126_ZERO_SCORE_AXIS",
            "calibration rows did not produce a non-zero score direction",
            "Inspect the #392 row diagnostics and feature schema before fitting rotation.",
            {"feature_count": len(feature_schema)},
        )

    calibration_cell_rows = per_cell_axis_rows(
        calibration,
        calibration_vectors,
        score_axis,
        label_axis,
        min_support=int(args.min_cell_support),
    )
    holdout_cell_rows = per_cell_reporting_rows(
        holdout,
        holdout_vectors,
        calibration_cell_rows,
        score_axis,
        label_axis,
    )
    associations = association_report(calibration_cell_rows)
    candidate = rotation_candidate_report(
        calibration,
        holdout,
        calibration_cell_rows,
        association_threshold=float(args.association_threshold),
    )

    calibration_split = split_report(calibration, "predicted_oracle_pass")
    holdout_split = split_report(holdout, "predicted_oracle_pass")
    cell_table_path = output_root / "axis_rotation_cell_table.jsonl"
    row_scores_path = output_root / "axis_rotation_candidate_rows.jsonl"
    report_path = output_root / "axis_rotation_diagnostic_report.json"
    write_jsonl(cell_table_path, calibration_cell_rows)
    write_jsonl(row_scores_path, candidate["rowScoresReportingOnly"])

    report = {
        "artifactKind": "task_py_g_126_per_cell_axis_rotation_diagnostic",
        "checkedAt": utc_now(),
        "githubIssue": 587,
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
        "featureSpace": {
            "basis": "slot_scalars_plus_105_pair_transition_features",
            "featureCount": len(feature_schema),
            "slotCount": len(EXPECTED_DOCUMENTED_SLOTS),
            "documentedPairCount": len(list(combinations(EXPECTED_DOCUMENTED_SLOTS, 2))),
            "preservesSlotIdentity": True,
            "preservesPairIdentity": True,
            "noFlatVectorShortcut": True,
            "schemaSample": feature_schema[:25],
        },
        "baseline": {
            "calibration": calibration_split,
            "holdout": holdout_split,
            "referenceActive392": {
                "prediction_oracle_correlation": REFERENCE_ACTIVE_CORRELATION,
                "brier": REFERENCE_ACTIVE_BRIER,
                "smooth_ece": REFERENCE_ACTIVE_SMOOTH_ECE,
            },
        },
        "axisDiagnostics": {
            "scoreAxisNorm": finite_float(vector_norm(score_axis)),
            "labelAxisNorm": finite_float(vector_norm(label_axis)),
            "scoreLabelAxisCosine": finite_float(cosine(score_axis, label_axis)),
            "calibrationPerCell": calibration_cell_rows,
            "holdoutPerCellReportingOnly": holdout_cell_rows,
            "associations": associations,
            "weakCells": weak_cell_report(calibration_cell_rows),
            "abstainPoolFinding": abstain_pool_finding(calibration_cell_rows),
        },
        "rotationCandidate": candidate_report_without_rows(candidate),
        "policy": {
            "readOnlyDiagnostic": True,
            "selectionSplit": "calibration_only",
            "holdoutUse": "reporting_only",
            "noNewEmbedders": True,
            "noNewModelDownload": True,
            "noPredictorPromotion": True,
            "noHoldoutThresholdTuning": True,
        },
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "outputArtifacts": {
            "axisRotationCellTableJsonl": str(cell_table_path),
            "axisRotationCellTableSha256": sha256_file(cell_table_path),
            "axisRotationCandidateRowsJsonl": str(row_scores_path),
            "axisRotationCandidateRowsSha256": sha256_file(row_scores_path),
        },
        "nextRecommendation": recommendation(associations, calibration_cell_rows, candidate),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def build_feature_schema(states: dict[tuple[str, str], dict[str, Any]]) -> list[str]:
    pairs = [f"{left}/{right}" for left, right in combinations(EXPECTED_DOCUMENTED_SLOTS, 2)]
    schema = []
    for slot in EXPECTED_DOCUMENTED_SLOTS:
        for feature in SCALAR_FEATURES:
            schema.append(f"slot_target::{slot}::{feature}")
            schema.append(f"slot_delta::{slot}::{feature}")
    for pair in pairs:
        for feature in ("targetMaxDelta", "deltaFromBase", "stateOrdinal", "driverChanged"):
            schema.append(f"pair::{pair}::{feature}")
    active_slots = {
        slot
        for row in states.values()
        for slot in slot_features(row).keys()
    }
    if not active_slots:
        raise SwebenchOpsError(
            "PY_G_126_NO_ACTIVE_SLOT_FEATURES",
            "no slot_feature_means were found in code-state rows",
            "Use the #117 live-split label root with per-slot feature means.",
            {"state_count": len(states)},
        )
    return schema


def materialize_vectors(
    rows: list[dict[str, Any]],
    states: dict[tuple[str, str], dict[str, Any]],
    schema: list[str],
) -> list[list[float]]:
    out = []
    for row in rows:
        out.append(vector_for_row(row, states, schema))
    return out


def vector_for_row(
    row: dict[str, Any],
    states: dict[tuple[str, str], dict[str, Any]],
    schema: list[str],
) -> list[float]:
    base = require_state(states, row["task_id"], "base")
    target = require_state(states, row["task_id"], target_workspace_state(row["category"]))
    base_slots = slot_features(base)
    target_slots = slot_features(target)
    base_pairs = derived_pair_records(base)
    target_pairs = derived_pair_records(target)
    values: dict[str, float] = {}
    for slot in EXPECTED_DOCUMENTED_SLOTS:
        for feature in SCALAR_FEATURES:
            target_value = target_slots.get(slot, {}).get(feature, 0.0)
            base_value = base_slots.get(slot, {}).get(feature, 0.0)
            values[f"slot_target::{slot}::{feature}"] = float(target_value)
            values[f"slot_delta::{slot}::{feature}"] = float(target_value) - float(base_value)
    for left, right in combinations(EXPECTED_DOCUMENTED_SLOTS, 2):
        pair = f"{left}/{right}"
        target_pair = target_pairs.get(pair)
        base_pair = base_pairs.get(pair)
        if target_pair is None:
            target_max_delta = 0.0
            state_ordinal = 0.0
            driver_changed = 0.0
        else:
            target_max_delta = float(target_pair["max_delta"])
            state_ordinal = pair_state_ordinal(str(target_pair["state"]))
            driver_changed = (
                0.0
                if base_pair is None or str(target_pair["driver"]) == str(base_pair.get("driver"))
                else 1.0
            )
        base_max_delta = float(base_pair["max_delta"]) if base_pair is not None else 0.0
        values[f"pair::{pair}::targetMaxDelta"] = target_max_delta
        values[f"pair::{pair}::deltaFromBase"] = target_max_delta - base_max_delta
        values[f"pair::{pair}::stateOrdinal"] = state_ordinal
        values[f"pair::{pair}::driverChanged"] = driver_changed
    return [finite_float(values.get(key, 0.0)) for key in schema]


def covariance_direction(vectors: list[list[float]], targets: list[float]) -> list[float]:
    if not vectors or len(vectors) != len(targets):
        raise SwebenchOpsError(
            "PY_G_126_DIRECTION_INPUT_MISMATCH",
            "vectors and targets must be non-empty and aligned",
            "Fix vector materialization before computing axes.",
            {"vector_count": len(vectors), "target_count": len(targets)},
        )
    means = feature_means(vectors)
    target_mean = sum(float(value) for value in targets) / len(targets)
    direction = [0.0 for _ in means]
    for vector, target in zip(vectors, targets):
        weight = float(target) - target_mean
        for idx, value in enumerate(vector):
            direction[idx] += weight * (float(value) - means[idx])
    return direction


def per_cell_axis_rows(
    rows: list[dict[str, Any]],
    vectors: list[list[float]],
    score_axis: list[float],
    label_axis: list[float],
    *,
    min_support: int,
) -> list[dict[str, Any]]:
    global_means = feature_means(vectors)
    grouped: dict[str, list[tuple[dict[str, Any], list[float]]]] = defaultdict(list)
    for row, vector in zip(rows, vectors):
        grouped[cell_key(row)].append((row, vector))
    out = []
    for cell, items in sorted(grouped.items()):
        cell_rows = [row for row, _vector in items]
        cell_vectors = [vector for _row, vector in items]
        centroid = subtract_vectors(feature_means(cell_vectors), global_means)
        score_cos = cosine(centroid, score_axis)
        label_cos = cosine(centroid, label_axis)
        theta = degrees_from_cos(score_cos)
        metrics = metric_block(
            [row["actual"] for row in cell_rows],
            [row["predicted_oracle_pass"] for row in cell_rows],
        )
        active_count = sum(1 for row in cell_rows if row["active_after_abstain"])
        labels = [row["actual"] for row in cell_rows]
        scores = [row["predicted_oracle_pass"] for row in cell_rows]
        out.append(
            {
                "cell": cell,
                "rowCount": len(cell_rows),
                "centroidNorm": finite_float(vector_norm(centroid)),
                "scoreAxisCosine": finite_float(score_cos),
                "scoreAxisThetaDegrees": finite_float(theta),
                "axisMismatchOneMinusAbsCos": finite_float(1.0 - abs(score_cos)),
                "labelAxisCosine": finite_float(label_cos),
                "labelAxisThetaDegrees": finite_float(degrees_from_cos(label_cos)),
                "metrics": metrics,
                "activeCoverage": finite_float(active_count / len(cell_rows)) if cell_rows else None,
                "abstainCoverage": finite_float(1.0 - active_count / len(cell_rows)) if cell_rows else None,
                "meanResidualScoreMinusActual": finite_float(
                    sum(float(score) - float(label) for label, score in zip(labels, scores)) / len(cell_rows)
                )
                if cell_rows
                else None,
                "powerBlockers": power_blockers(labels, scores),
                "axisPowerBlockers": ["LOW_CELL_N_LT_MIN_SUPPORT"]
                if len(cell_rows) < min_support
                else [],
            }
        )
    return out


def per_cell_reporting_rows(
    rows: list[dict[str, Any]],
    vectors: list[list[float]],
    calibration_cells: list[dict[str, Any]],
    score_axis: list[float],
    label_axis: list[float],
) -> list[dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    _ = vectors, score_axis, label_axis
    calibration_by_cell = {row["cell"]: row for row in calibration_cells}
    for row in rows:
        grouped[cell_key(row)].append(row)
    out = []
    for cell, cell_rows in sorted(grouped.items()):
        metrics = metric_block(
            [row["actual"] for row in cell_rows],
            [row["predicted_oracle_pass"] for row in cell_rows],
        )
        active_count = sum(1 for row in cell_rows if row["active_after_abstain"])
        cal_axis = calibration_by_cell.get(cell, {})
        out.append(
            {
                "cell": cell,
                "rowCount": len(cell_rows),
                "calibrationScoreAxisCosine": cal_axis.get("scoreAxisCosine"),
                "calibrationScoreAxisThetaDegrees": cal_axis.get("scoreAxisThetaDegrees"),
                "metrics": metrics,
                "activeCoverage": finite_float(active_count / len(cell_rows)) if cell_rows else None,
                "abstainCoverage": finite_float(1.0 - active_count / len(cell_rows)) if cell_rows else None,
                "powerBlockers": power_blockers(
                    [row["actual"] for row in cell_rows],
                    [row["predicted_oracle_pass"] for row in cell_rows],
                ),
            }
        )
    return out


def rotation_candidate_report(
    calibration: list[dict[str, Any]],
    holdout: list[dict[str, Any]],
    cell_rows: list[dict[str, Any]],
    *,
    association_threshold: float,
) -> dict[str, Any]:
    association = association_report(cell_rows)
    support = association.get("axisMismatchOneMinusAbsCos_vs_smooth_ece", {}).get("pearson")
    evidence_strong = support is not None and float(support) >= association_threshold
    cell_params = fit_cell_rotation_params(calibration, cell_rows)
    calibration_rows = score_rows_with_candidate(calibration, cell_params, "calibration")
    holdout_rows = score_rows_with_candidate(holdout, cell_params, "holdout")
    baseline_cal = split_report(calibration, "predicted_oracle_pass")
    candidate_cal = split_report(calibration_rows, "axis_rotation_score")
    baseline_hold = split_report(holdout, "predicted_oracle_pass")
    candidate_hold = split_report(holdout_rows, "axis_rotation_score")
    promotion_gate = promotion_gate_report(candidate_hold)
    return {
        "candidateEmitted": evidence_strong,
        "candidateStatus": "emitted_for_split_legal_falsification" if evidence_strong else "not_emitted_axis_evidence_below_threshold",
        "associationThreshold": association_threshold,
        "supportingAssociationPearson": support,
        "fitSplit": "calibration_only",
        "holdoutUse": "reporting_only",
        "cellParams": cell_params,
        "baselineCalibration": baseline_cal,
        "candidateCalibration": candidate_cal,
        "baselineHoldout": baseline_hold,
        "candidateHoldoutReportingOnly": candidate_hold,
        "promotionGate": promotion_gate,
        "rowScoresReportingOnly": holdout_rows,
    }


def fit_cell_rotation_params(rows: list[dict[str, Any]], cell_axis_rows: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[cell_key(row)].append(row)
    axis_by_cell = {row["cell"]: row for row in cell_axis_rows}
    out = {}
    for cell, cell_rows in sorted(grouped.items()):
        score_logits = [logit(row["predicted_oracle_pass"]) for row in cell_rows]
        label_rate = (sum(row["actual"] for row in cell_rows) + 0.5) / (len(cell_rows) + 1.0)
        score_mean = sum(score_logits) / len(score_logits)
        target_mean = logit(label_rate)
        axis_cos = float(axis_by_cell.get(cell, {}).get("scoreAxisCosine", 0.0) or 0.0)
        scale = min(1.0, max(0.05, abs(axis_cos)))
        out[cell] = {
            "n": len(cell_rows),
            "axisCosine": finite_float(axis_cos),
            "scaleAbsCosClamped": finite_float(scale),
            "scoreLogitMean": finite_float(score_mean),
            "labelRateJeffreys": finite_float(label_rate),
            "targetLogitMean": finite_float(target_mean),
            "logitBias": finite_float(target_mean - scale * score_mean),
        }
    return out


def score_rows_with_candidate(
    rows: list[dict[str, Any]],
    params: dict[str, dict[str, Any]],
    split: str,
) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        cell = cell_key(row)
        param = params.get(cell)
        if param is None:
            score = row["predicted_oracle_pass"]
            status = "missing_cell_param_baseline_score"
        else:
            score = sigmoid(float(param["logitBias"]) + float(param["scaleAbsCosClamped"]) * logit(row["predicted_oracle_pass"]))
            status = "axis_rotation_score"
        out.append(
            {
                **row,
                "split": split,
                "cell": cell,
                "axis_rotation_score": finite_float(min(1.0, max(0.0, score))),
                "axis_rotation_status": status,
            }
        )
    return out


def split_report(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    active = [row for row in rows if row["active_after_abstain"]]
    abstain = [row for row in rows if not row["active_after_abstain"]]
    return {
        "full": metric_block([row["actual"] for row in rows], [row[score_key] for row in rows]),
        "active392": metric_block([row["actual"] for row in active], [row[score_key] for row in active]) if active else None,
        "abstainPool": metric_block([row["actual"] for row in abstain], [row[score_key] for row in abstain]) if abstain else None,
        "rowCounts": {"full": len(rows), "active392": len(active), "abstainPool": len(abstain)},
        "perCell": per_cell_metric_report(rows, score_key),
    }


def per_cell_metric_report(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[cell_key(row)].append(row)
    out = {}
    for cell, cell_rows in sorted(grouped.items()):
        out[cell] = metric_block(
            [row["actual"] for row in cell_rows],
            [row[score_key] for row in cell_rows],
        )
    return out


def association_report(cell_rows: list[dict[str, Any]]) -> dict[str, Any]:
    metrics = {
        "smooth_ece": lambda row: row["metrics"].get("smooth_ece"),
        "fixed_bin_ece": lambda row: row["metrics"].get("fixed_bin_ece"),
        "brier": lambda row: row["metrics"].get("brier"),
        "activeCoverage": lambda row: row.get("activeCoverage"),
        "abstainCoverage": lambda row: row.get("abstainCoverage"),
        "absMeanResidual": lambda row: abs(float(row.get("meanResidualScoreMinusActual") or 0.0)),
    }
    axes = {
        "scoreAxisThetaDegrees": lambda row: row.get("scoreAxisThetaDegrees"),
        "axisMismatchOneMinusAbsCos": lambda row: row.get("axisMismatchOneMinusAbsCos"),
        "scoreAxisCosine": lambda row: row.get("scoreAxisCosine"),
    }
    out = {}
    for axis_name, axis_getter in axes.items():
        for metric_name, metric_getter in metrics.items():
            pairs = [
                (float(axis_getter(row)), float(metric_getter(row)))
                for row in cell_rows
                if is_number(axis_getter(row)) and is_number(metric_getter(row))
            ]
            key = f"{axis_name}_vs_{metric_name}"
            out[key] = {
                "n": len(pairs),
                "pearson": pearson_float_or_none(
                    [left for left, _right in pairs],
                    [right for _left, right in pairs],
                )
                if len(pairs) >= 3
                else None,
                "spearman": pearson_float_or_none(
                    rank_values([left for left, _right in pairs]),
                    rank_values([right for _left, right in pairs]),
                )
                if len(pairs) >= 3
                else None,
            }
    return out


def weak_cell_report(cell_rows: list[dict[str, Any]]) -> dict[str, Any]:
    by_cell = {row["cell"]: row for row in cell_rows}
    ranked = sorted(cell_rows, key=lambda row: (-float(row["axisMismatchOneMinusAbsCos"]), row["cell"]))
    rank_by_cell = {row["cell"]: idx + 1 for idx, row in enumerate(ranked)}
    out = {}
    for cell in WEAK_CELLS:
        row = by_cell.get(cell)
        out[cell] = {
            "present": row is not None,
            "axisMismatchRankDescending": rank_by_cell.get(cell),
            "row": row,
        }
    return out


def abstain_pool_finding(cell_rows: list[dict[str, Any]]) -> dict[str, Any]:
    pairs = [
        (float(row["axisMismatchOneMinusAbsCos"]), float(row["abstainCoverage"]))
        for row in cell_rows
        if is_number(row.get("axisMismatchOneMinusAbsCos")) and is_number(row.get("abstainCoverage"))
    ]
    return {
        "cellCount": len(pairs),
        "axisMismatchVsAbstainCoveragePearson": pearson_float_or_none(
            [left for left, _right in pairs],
            [right for _left, right in pairs],
        )
        if len(pairs) >= 3
        else None,
        "highestMismatchCells": sorted(
            [
                {
                    "cell": row["cell"],
                    "axisMismatchOneMinusAbsCos": row["axisMismatchOneMinusAbsCos"],
                    "abstainCoverage": row["abstainCoverage"],
                    "smooth_ece": row["metrics"].get("smooth_ece"),
                    "brier": row["metrics"].get("brier"),
                }
                for row in cell_rows
            ],
            key=lambda row: (-float(row["axisMismatchOneMinusAbsCos"]), row["cell"]),
        )[:5],
    }


def promotion_gate_report(candidate_holdout: dict[str, Any]) -> dict[str, Any]:
    active = candidate_holdout.get("active392") or {}
    corr = active.get("prediction_oracle_correlation")
    brier = active.get("brier")
    smooth_ece = active.get("smooth_ece")
    beats = {
        "activeCorrelationBeatsOrEqualsReference": corr is not None and float(corr) >= REFERENCE_ACTIVE_CORRELATION,
        "activeBrierBeatsOrEqualsReference": brier is not None and float(brier) <= REFERENCE_ACTIVE_BRIER,
        "activeSmoothEceBeatsOrEqualsReference": smooth_ece is not None and float(smooth_ece) <= REFERENCE_ACTIVE_SMOOTH_ECE,
    }
    return {
        **beats,
        "promotionEligible": all(beats.values()),
        "referenceActive392": {
            "prediction_oracle_correlation": REFERENCE_ACTIVE_CORRELATION,
            "brier": REFERENCE_ACTIVE_BRIER,
            "smooth_ece": REFERENCE_ACTIVE_SMOOTH_ECE,
        },
    }


def recommendation(
    associations: dict[str, Any],
    cell_rows: list[dict[str, Any]],
    candidate: dict[str, Any],
) -> dict[str, Any]:
    smooth_assoc = associations.get("axisMismatchOneMinusAbsCos_vs_smooth_ece", {}).get("pearson")
    abstain_assoc = associations.get("axisMismatchOneMinusAbsCos_vs_abstainCoverage", {}).get("pearson")
    candidate_gate = candidate.get("promotionGate", {})
    if candidate_gate.get("promotionEligible"):
        return {
            "recommendedAction": "promote_axis_rotation_candidate_to_586_calibration_substrate",
            "reason": "candidate beat or equaled reference active correlation, brier, and smooth ece",
        }
    if smooth_assoc is not None and float(smooth_assoc) >= 0.35:
        return {
            "recommendedAction": "use_axis_mismatch_as_per_cell_calibration_feature_not_as_standalone_candidate",
            "reason": "axis mismatch explains calibration error, but the closed-form candidate is not promotion eligible",
            "smoothEceAssociationPearson": smooth_assoc,
            "abstainAssociationPearson": abstain_assoc,
            "highestMismatchCells": abstain_pool_finding(cell_rows)["highestMismatchCells"],
        }
    if smooth_assoc is not None and float(smooth_assoc) <= -0.35:
        return {
            "recommendedAction": "do_not_prioritize_axis_rotation_as_primary_gate_mover",
            "reason": "axis mismatch is negatively associated with SmoothECE on calibration cells; this falsifies the simple mismatch-causes-ECE hypothesis",
            "smoothEceAssociationPearson": smooth_assoc,
            "abstainAssociationPearson": abstain_assoc,
            "highestMismatchCells": abstain_pool_finding(cell_rows)["highestMismatchCells"],
        }
    return {
        "recommendedAction": "do_not_prioritize_axis_rotation_as_primary_gate_mover",
        "reason": "axis mismatch did not show strong calibration-split association with SmoothECE",
        "smoothEceAssociationPearson": smooth_assoc,
        "abstainAssociationPearson": abstain_assoc,
    }


def candidate_report_without_rows(candidate: dict[str, Any]) -> dict[str, Any]:
    return {key: value for key, value in candidate.items() if key != "rowScoresReportingOnly"}


def feature_means(vectors: list[list[float]]) -> list[float]:
    if not vectors:
        return []
    width = len(vectors[0])
    sums = [0.0] * width
    for vector in vectors:
        if len(vector) != width:
            raise SwebenchOpsError(
                "PY_G_126_VECTOR_WIDTH_MISMATCH",
                "feature vectors have inconsistent widths",
                "Fix the feature schema materializer.",
                {"expected_width": width, "actual_width": len(vector)},
            )
        for idx, value in enumerate(vector):
            sums[idx] += float(value)
    return [value / len(vectors) for value in sums]


def subtract_vectors(left: list[float], right: list[float]) -> list[float]:
    return [float(l) - float(r) for l, r in zip(left, right)]


def vector_norm(vector: list[float]) -> float:
    return math.sqrt(sum(float(value) * float(value) for value in vector))


def cosine(left: list[float], right: list[float]) -> float:
    denom = vector_norm(left) * vector_norm(right)
    if denom <= 1.0e-12:
        return 0.0
    return max(-1.0, min(1.0, sum(float(l) * float(r) for l, r in zip(left, right)) / denom))


def degrees_from_cos(value: float) -> float:
    return math.degrees(math.acos(max(-1.0, min(1.0, float(value)))))


def pair_state_ordinal(state: str) -> float:
    return {
        "near_identity_cross_slot_delta": 0.0,
        "low_cross_slot_delta": 1.0,
        "moderate_cross_slot_delta": 2.0,
        "high_cross_slot_delta": 3.0,
        "extreme_cross_slot_delta": 4.0,
    }.get(state, 0.0)


def cell_key(row: dict[str, Any]) -> str:
    return f"{row['category']}::python"


def is_number(value: Any) -> bool:
    return not isinstance(value, bool) and isinstance(value, (int, float)) and math.isfinite(float(value))


def logit(score: float) -> float:
    score = min(1.0 - 1.0e-9, max(1.0e-9, float(score)))
    return math.log(score / (1.0 - score))


def sigmoid(value: float) -> float:
    if value >= 0.0:
        z = math.exp(-value)
        return 1.0 / (1.0 + z)
    z = math.exp(value)
    return z / (1.0 + z)
