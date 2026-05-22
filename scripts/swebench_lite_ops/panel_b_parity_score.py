from __future__ import annotations

import json
import math
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import write_jsonl
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-392-temporal-trajectory-features-readback/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_readback.json"
)
DEFAULT_PANEL_B_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-111f-panel-b-parity-scorer-readback")
DEFAULT_CALIBRATION_PANEL_B_ROWS = DEFAULT_PANEL_B_ROOT / "panel_b_calibration_model_scores.jsonl"
DEFAULT_HOLDOUT_PANEL_B_ROWS = DEFAULT_PANEL_B_ROOT / "panel_b_model_scores.jsonl"
DEFAULT_OUTPUT_ROOT = DEFAULT_PANEL_B_ROOT
COVERAGE_GRID = (1.0, 0.95, 0.9, 0.85, 0.8, 0.75, 0.7, 0.65, 0.6, 0.55, 0.5, 0.4, 0.3)


@dataclass(frozen=True)
class DiagnosticRow:
    split: str
    row_index: int
    task_id: str
    category: str
    patch_path: str
    label: int
    panel_a_score: float
    active_after_abstain: bool | None
    evidence_key_count: float
    cause_feature_nonzero_count: float
    cause_feature_large_magnitude_count: float


@dataclass(frozen=True)
class PanelBRow:
    task_id: str
    category: str
    prediction_id_hex: str
    score_b: float
    accepted_label_ids: tuple[str, ...]
    failure_evidence_set_ids: tuple[str, ...]
    model_scores: dict[str, float]
    transformed_scores: dict[str, float]


@dataclass(frozen=True)
class JoinedRow:
    diagnostic: DiagnosticRow
    panel_b: PanelBRow
    features: dict[str, float]


@dataclass(frozen=True)
class ActiveSelection:
    category: str
    coverage: float
    margin_threshold: float | None
    reason: str
    calibration_metrics: dict[str, Any]


def add_arguments(parser: Any) -> None:
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--calibration-panel-b-rows", type=Path, default=DEFAULT_CALIBRATION_PANEL_B_ROWS)
    parser.add_argument("--holdout-panel-b-rows", type=Path, default=DEFAULT_HOLDOUT_PANEL_B_ROWS)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--max-id-features", type=int, default=96)
    parser.add_argument("--epochs", type=int, default=2200)
    parser.add_argument("--learning-rate", type=float, default=0.08)
    parser.add_argument("--l2", type=float, default=0.03)
    parser.add_argument("--target-correlation", type=float, default=0.95)
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)


def command_panel_b_parity_score(args: Any) -> dict[str, Any]:
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    calibration_panel_b_rows = args.calibration_panel_b_rows.expanduser().resolve(strict=False)
    holdout_panel_b_rows = args.holdout_panel_b_rows.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("row_diagnostics_report", row_diagnostics_report),
        ("calibration_panel_b_rows", calibration_panel_b_rows),
        ("holdout_panel_b_rows", holdout_panel_b_rows),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    validate_positive("max_id_features", args.max_id_features)
    validate_positive("epochs", args.epochs)
    validate_positive_float("learning_rate", args.learning_rate)
    if args.l2 < 0.0:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_L2_INVALID",
            "--l2 must be non-negative",
            "Use a non-negative L2 penalty for deterministic logistic fitting.",
            {"l2": args.l2},
        )

    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_diag = load_diagnostics(
        diagnostics,
        "score_sharpness_abstain_calibration_row_diagnostics",
        split="calibration",
        expected=args.expected_calibration_count,
    )
    holdout_diag = load_diagnostics(
        diagnostics,
        "score_sharpness_abstain_holdout_row_diagnostics",
        split="holdout",
        expected=args.expected_holdout_count,
    )
    calibration_b = load_panel_b_rows(calibration_panel_b_rows, expected=args.expected_calibration_count)
    holdout_b = load_panel_b_rows(holdout_panel_b_rows, expected=args.expected_holdout_count)
    calibration = join_rows(calibration_diag, calibration_b, split="calibration")
    holdout = join_rows(holdout_diag, holdout_b, split="holdout")

    feature_names = select_feature_names(calibration, max_id_features=args.max_id_features)
    feature_stats = fit_feature_stats(calibration, feature_names)
    score_only_weights = fit_logistic_model(
        calibration,
        [],
        feature_stats,
        epochs=args.epochs,
        learning_rate=args.learning_rate,
        l2=args.l2,
    )
    parity_weights = fit_logistic_model(
        calibration,
        feature_names,
        feature_stats,
        epochs=args.epochs,
        learning_rate=args.learning_rate,
        l2=args.l2,
    )
    calibration_panel_b = score_panel_b_baseline(calibration)
    holdout_panel_b = score_panel_b_baseline(holdout)
    calibration_score_only = apply_model(calibration, [], feature_stats, score_only_weights, "panel_b_score_only_logistic")
    holdout_score_only = apply_model(holdout, [], feature_stats, score_only_weights, "panel_b_score_only_logistic")
    calibration_parity = apply_model(calibration, feature_names, feature_stats, parity_weights, "panel_b_parity_logistic")
    holdout_parity = apply_model(holdout, feature_names, feature_stats, parity_weights, "panel_b_parity_logistic")
    holdout_panel_a = score_panel_a_baseline(holdout)
    active_by_392 = [row for row in holdout_parity if row["active_after_abstain"] is True]
    active_by_392_panel_a = [row for row in holdout_panel_a if row["active_after_abstain"] is True]
    selections = fit_active_selections(
        calibration_parity,
        calibration_panel_b,
        target=float(args.target_correlation),
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
    )
    parity_active = apply_active_selections(holdout_parity, selections)

    output_root.mkdir(parents=True, exist_ok=True)
    calibration_rows_path = output_root / "panel_b_parity_calibration_rows.jsonl"
    holdout_rows_path = output_root / "panel_b_parity_holdout_rows.jsonl"
    active_rows_path = output_root / "panel_b_parity_active_rows.jsonl"
    active392_cross_panel_path = output_root / "panel_b_parity_active392_cross_panel_rows.jsonl"
    full_cross_panel_path = output_root / "panel_b_parity_full_cross_panel_rows.jsonl"
    write_jsonl(calibration_rows_path, calibration_parity)
    write_jsonl(holdout_rows_path, holdout_parity)
    write_jsonl(active_rows_path, parity_active)
    write_jsonl(active392_cross_panel_path, cross_panel_rows(active_by_392))
    write_jsonl(full_cross_panel_path, cross_panel_rows(holdout_parity))
    calibration_report = split_report(calibration_parity)
    holdout_report = split_report(holdout_parity)
    panel_a_holdout_report = split_report(holdout_panel_a)
    active_392_report = split_report(active_by_392)
    active_392_panel_a_report = split_report(active_by_392_panel_a)
    parity_active_report = split_report(parity_active)
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_111f_panel_b_parity_score",
        "githubIssue": 582,
        "ok": True,
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "calibrationPanelBRows": str(calibration_panel_b_rows),
        "calibrationPanelBRowsSha256": sha256_file(calibration_panel_b_rows),
        "holdoutPanelBRows": str(holdout_panel_b_rows),
        "holdoutPanelBRowsSha256": sha256_file(holdout_panel_b_rows),
        "fitPartition": {
            "calibrationRowsUsedForFitting": len(calibration),
            "holdoutRowsUsedForFitting": 0,
            "holdoutOracleLabelsUsedForScore": False,
            "targetSideOracleLabelsInFeatureVector": False,
            "panelAScoreUsedAsFeature": False,
        },
        "features": {
            "selectedFeatureCount": len(feature_names),
            "selectedFeatureNames": feature_names,
            "featureFamilies": feature_family_counts(feature_names),
        },
        "optimizer": {
            "model": "logistic_regression_over_panel_b_slot_scores_plus_live_safe_consequence_features",
            "epochs": args.epochs,
            "learningRate": args.learning_rate,
            "l2": args.l2,
        },
        "calibration": {
            "panelBRaw": split_report(calibration_panel_b),
            "panelBScoreOnlyLogistic": split_report(calibration_score_only),
            "panelBParity": calibration_report,
        },
        "holdout": {
            "panelABaseline392": panel_a_holdout_report,
            "panelBRaw": split_report(holdout_panel_b),
            "panelBScoreOnlyLogistic": split_report(holdout_score_only),
            "panelBParity": holdout_report,
            "deltaParityVsPanelBRaw": metric_delta(split_report(holdout_panel_b)["global"], holdout_report["global"]),
            "deltaParityVsPanelA392": metric_delta(panel_a_holdout_report["global"], holdout_report["global"]),
        },
        "active392SubsetReportingOnly": {
            "panelA392": active_392_panel_a_report,
            "panelBParity": active_392_report,
            "absCorrelationDelta": abs_metric_delta(
                active_392_panel_a_report["global"].get("prediction_oracle_correlation"),
                active_392_report["global"].get("prediction_oracle_correlation"),
            ),
        },
        "panelBParityActivePolicy": {
            "selectionFitSplit": "calibration",
            "selections": {
                category: {
                    "coverage": selection.coverage,
                    "marginThreshold": selection.margin_threshold,
                    "reason": selection.reason,
                    "calibrationMetrics": selection.calibration_metrics,
                }
                for category, selection in sorted(selections.items())
            },
            "holdout": parity_active_report,
        },
        "promotionRecommendation": promotion_recommendation(
            panel_a_holdout_report,
            holdout_report,
            active_392_panel_a_report,
            active_392_report,
            parity_active_report,
            target=float(args.target_correlation),
        ),
        "outputArtifacts": {
            "calibrationRowsJsonl": str(calibration_rows_path),
            "calibrationRowsSha256": sha256_file(calibration_rows_path),
            "holdoutRowsJsonl": str(holdout_rows_path),
            "holdoutRowsSha256": sha256_file(holdout_rows_path),
            "activeRowsJsonl": str(active_rows_path),
            "activeRowsSha256": sha256_file(active_rows_path),
            "active392CrossPanelRowsJsonl": str(active392_cross_panel_path),
            "active392CrossPanelRowsSha256": sha256_file(active392_cross_panel_path),
            "fullCrossPanelRowsJsonl": str(full_cross_panel_path),
            "fullCrossPanelRowsSha256": sha256_file(full_cross_panel_path),
        },
        "pathPolicy": {
            "aiwonderOnly": str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(calibration_panel_b_rows).startswith("/zfs/archive/contextgraph/")
            and str(holdout_panel_b_rows).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    report_path = output_root / "panel_b_parity_score_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_diagnostics(payload: dict[str, Any], key: str, *, split: str, expected: int) -> list[DiagnosticRow]:
    rows = payload.get(key)
    if not isinstance(rows, list):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_DIAGNOSTICS_MISSING",
            f"{key} missing from row diagnostics report",
            "Use the current #392 row diagnostics artifact.",
            {"key": key},
        )
    if len(rows) != expected:
        raise_count(key, expected, len(rows))
    out: list[DiagnosticRow] = []
    seen: set[tuple[str, str]] = set()
    for index, raw in enumerate(rows):
        row = require_object(raw, f"{key}[{index}]")
        task_id = required_string(row, "task_id")
        category = required_string(row, "category")
        identity = (task_id, category)
        if identity in seen:
            raise SwebenchOpsError(
                "PANEL_B_PARITY_SCORE_DUPLICATE_DIAGNOSTIC",
                "diagnostic split contains duplicate task/category identity",
                "Regenerate row diagnostics with one row per task/category/split.",
                {"split": split, "task_id": task_id, "category": category},
            )
        seen.add(identity)
        out.append(
            DiagnosticRow(
                split=split,
                row_index=int(row.get("row_index", index)),
                task_id=task_id,
                category=category,
                patch_path=required_string(row, "patch_path"),
                label=1 if required_bool(row, "actual_oracle_pass") else 0,
                panel_a_score=required_float(row, "predicted_oracle_pass"),
                active_after_abstain=optional_bool(row, "active_after_abstain"),
                evidence_key_count=required_float(row, "evidence_key_count"),
                cause_feature_nonzero_count=required_float(row, "cause_feature_nonzero_count"),
                cause_feature_large_magnitude_count=required_float(row, "cause_feature_large_magnitude_count"),
            )
        )
    return out


def load_panel_b_rows(path: Path, *, expected: int) -> list[PanelBRow]:
    rows = []
    for line_number, row in iter_jsonl(path):
        rows.append(
            PanelBRow(
                task_id=required_string(row, "taskId"),
                category=required_string(row, "category"),
                prediction_id_hex=required_string(row, "predictionIdHex"),
                score_b=required_float(row, "scoreB"),
                accepted_label_ids=tuple(require_string_array(row.get("acceptedLabelIds"), "acceptedLabelIds")),
                failure_evidence_set_ids=tuple(
                    require_string_array(row.get("failureEvidenceSetIds"), "failureEvidenceSetIds")
                ),
                model_scores=require_float_map(row.get("panelBModelScores"), "panelBModelScores"),
                transformed_scores=require_float_map(
                    row.get("panelBModelTransformedScores"),
                    "panelBModelTransformedScores",
                ),
            )
        )
    if len(rows) != expected:
        raise_count(str(path), expected, len(rows))
    seen: set[tuple[str, str]] = set()
    for row in rows:
        identity = (row.task_id, row.category)
        if identity in seen:
            raise SwebenchOpsError(
                "PANEL_B_PARITY_SCORE_DUPLICATE_PANEL_B_ROW",
                "Panel B split contains duplicate task/category identity",
                "Regenerate Panel B score rows with one row per task/category/split.",
                {"path": str(path), "task_id": row.task_id, "category": row.category},
            )
        seen.add(identity)
    return rows


def join_rows(diagnostics: list[DiagnosticRow], panel_b_rows: list[PanelBRow], *, split: str) -> list[JoinedRow]:
    by_identity = {(row.task_id, row.category): row for row in panel_b_rows}
    joined = []
    missing = []
    for diagnostic in diagnostics:
        panel_b = by_identity.get((diagnostic.task_id, diagnostic.category))
        if panel_b is None:
            missing.append({"task_id": diagnostic.task_id, "category": diagnostic.category})
            continue
        joined.append(JoinedRow(diagnostic=diagnostic, panel_b=panel_b, features=row_features(diagnostic, panel_b)))
    if missing:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_JOIN_MISSING",
            "Panel B rows are missing diagnostics identities",
            "Regenerate Panel B score rows from the same ship-gate split.",
            {"split": split, "missing_count": len(missing), "missing": missing[:20]},
        )
    return joined


def row_features(diagnostic: DiagnosticRow, panel_b: PanelBRow) -> dict[str, float]:
    features: dict[str, float] = {
        f"category:{diagnostic.category}": 1.0,
        "numeric:evidence_key_count": diagnostic.evidence_key_count,
        "numeric:cause_feature_nonzero_count": diagnostic.cause_feature_nonzero_count,
        "numeric:cause_feature_large_magnitude_count": diagnostic.cause_feature_large_magnitude_count,
    }
    for slot, score in sorted(panel_b.model_scores.items()):
        features[f"slot_raw:{slot}"] = logit(score)
    for slot, score in sorted(panel_b.transformed_scores.items()):
        features[f"slot_transformed:{slot}"] = logit(score)
    for label in panel_b.accepted_label_ids:
        features[f"accepted:{label}"] = 1.0
    for label in panel_b.failure_evidence_set_ids:
        features[f"failure:{label}"] = 1.0
    return features


def select_feature_names(rows: list[JoinedRow], *, max_id_features: int) -> list[str]:
    always: set[str] = set()
    ids = Counter()
    for row in rows:
        for feature in row.features:
            if feature.startswith("accepted:") or feature.startswith("failure:"):
                ids[feature] += 1
            else:
                always.add(feature)
    selected_ids = [
        feature
        for feature, _count in ids.most_common(max_id_features)
    ]
    return sorted(always) + selected_ids


def fit_feature_stats(rows: list[JoinedRow], feature_names: list[str]) -> dict[str, dict[str, float]]:
    stats = {}
    for feature in feature_names:
        values = [float(row.features.get(feature, 0.0)) for row in rows]
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / max(1, len(values) - 1)
        std = math.sqrt(variance)
        stats[feature] = {"mean": mean, "std": std if std > 1.0e-6 else 1.0}
    return stats


def fit_logistic_model(
    rows: list[JoinedRow],
    feature_names: list[str],
    feature_stats: dict[str, dict[str, float]],
    *,
    epochs: int,
    learning_rate: float,
    l2: float,
) -> list[float]:
    matrix = [design_vector(row, feature_names, feature_stats) for row in rows]
    labels = [float(row.diagnostic.label) for row in rows]
    weights = [0.0 for _ in matrix[0]]
    for step in range(epochs):
        grads = [0.0 for _ in weights]
        for x_row, label in zip(matrix, labels):
            predicted = sigmoid(dot(weights, x_row))
            err = predicted - label
            for index, value in enumerate(x_row):
                grads[index] += err * value
        inv_n = 1.0 / len(matrix)
        lr = learning_rate / math.sqrt(1.0 + step / 250.0)
        for index in range(len(weights)):
            penalty = 0.0 if index == 0 else l2 * weights[index]
            weights[index] -= lr * (grads[index] * inv_n + penalty)
            weights[index] = max(-20.0, min(20.0, weights[index]))
    return weights


def apply_model(
    rows: list[JoinedRow],
    feature_names: list[str],
    feature_stats: dict[str, dict[str, float]],
    weights: list[float],
    scorer: str,
) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        score = sigmoid(dot(weights, design_vector(row, feature_names, feature_stats)))
        out.append(scored_row(row, score, scorer))
    return out


def score_panel_b_baseline(rows: list[JoinedRow]) -> list[dict[str, Any]]:
    return [scored_row(row, row.panel_b.score_b, "panel_b_raw") for row in rows]


def score_panel_a_baseline(rows: list[JoinedRow]) -> list[dict[str, Any]]:
    return [scored_row(row, row.diagnostic.panel_a_score, "panel_a_392") for row in rows]


def scored_row(row: JoinedRow, score: float, scorer: str) -> dict[str, Any]:
    return {
        "split": row.diagnostic.split,
        "row_index": row.diagnostic.row_index,
        "task_id": row.diagnostic.task_id,
        "category": row.diagnostic.category,
        "predictionIdHex": row.panel_b.prediction_id_hex,
        "actual": row.diagnostic.label,
        "score": clamp_probability(score),
        "panel_b_raw_score": row.panel_b.score_b,
        "panel_a_392_score": row.diagnostic.panel_a_score,
        "scorer": scorer,
        "active_after_abstain": row.diagnostic.active_after_abstain,
        "acceptedLabelIds": list(row.panel_b.accepted_label_ids),
        "failureEvidenceSetIds": list(row.panel_b.failure_evidence_set_ids),
    }


def cross_panel_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        {
            "predictionIdHex": row["predictionIdHex"],
            "scoreA": float(row["panel_a_392_score"]),
            "scoreB": float(row["score"]),
            "oraclePass": bool(int(row["actual"])),
            "acceptedLabelIds": row["acceptedLabelIds"],
            "failureEvidenceSetIds": row["failureEvidenceSetIds"],
        }
        for row in rows
    ]


def design_vector(
    row: JoinedRow,
    feature_names: list[str],
    feature_stats: dict[str, dict[str, float]],
) -> list[float]:
    values = [1.0, logit(row.panel_b.score_b)]
    for feature in feature_names:
        stats = feature_stats[feature]
        raw = float(row.features.get(feature, 0.0))
        values.append((raw - stats["mean"]) / stats["std"])
    return values


def fit_active_selections(
    calibration_rows: list[dict[str, Any]],
    baseline_rows: list[dict[str, Any]],
    *,
    target: float,
    smooth_ece_regression_budget: float,
) -> dict[str, ActiveSelection]:
    baseline_by_cell = group_by_category(baseline_rows)
    selections = {}
    for category, rows in sorted(group_by_category(calibration_rows).items()):
        baseline_metrics = metric_block(
            [int(row["actual"]) for row in baseline_by_cell[category]],
            [float(row["score"]) for row in baseline_by_cell[category]],
        )
        max_smooth = (baseline_metrics.get("smooth_ece") or 0.0) + smooth_ece_regression_budget
        ordered = sorted(rows, key=lambda row: (-abs(float(row["score"]) - 0.5), int(row["row_index"])))
        best: ActiveSelection | None = None
        failures = []
        for coverage in COVERAGE_GRID:
            take = max(1, min(len(ordered), math.ceil(len(ordered) * coverage)))
            selected = ordered[:take]
            labels = [int(row["actual"]) for row in selected]
            scores = [float(row["score"]) for row in selected]
            metrics = metric_block(labels, scores)
            passed, reason = cell_gate_passed(labels, scores, metrics, target=target, max_smooth_ece=max_smooth)
            if not passed:
                failures.append(f"{coverage:.2f}:{reason}")
                continue
            best = ActiveSelection(
                category=category,
                coverage=coverage,
                margin_threshold=min(abs(float(row["score"]) - 0.5) for row in selected),
                reason="calibration_cell_gate_passed",
                calibration_metrics=metrics,
            )
            break
        if best is None:
            best = ActiveSelection(
                category=category,
                coverage=0.0,
                margin_threshold=None,
                reason="no_calibration_only_policy_met_cell_gate",
                calibration_metrics={"failures": failures[:12]},
            )
        selections[category] = best
    return selections


def apply_active_selections(rows: list[dict[str, Any]], selections: dict[str, ActiveSelection]) -> list[dict[str, Any]]:
    out = []
    for row in rows:
        selection = selections[row["category"]]
        if selection.margin_threshold is None:
            continue
        if abs(float(row["score"]) - 0.5) >= selection.margin_threshold:
            out.append({**row, "panel_b_parity_active": True})
    return out


def split_report(rows: list[dict[str, Any]]) -> dict[str, Any]:
    labels = [int(row["actual"]) for row in rows]
    scores = [float(row["score"]) for row in rows]
    by_category = group_by_category(rows)
    return {
        "rowCount": len(rows),
        "global": metric_block(labels, scores) if rows else empty_metric_block(),
        "perCell": {
            f"{category}::python": metric_block(
                [int(row["actual"]) for row in cell_rows],
                [float(row["score"]) for row in cell_rows],
            )
            for category, cell_rows in sorted(by_category.items())
        },
    }


def group_by_category(rows: list[dict[str, Any]]) -> dict[str, list[dict[str, Any]]]:
    out: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        out[str(row["category"])].append(row)
    return out


def cell_gate_passed(
    labels: list[int],
    scores: list[float],
    metrics: dict[str, Any],
    *,
    target: float,
    max_smooth_ece: float,
) -> tuple[bool, str]:
    if len(labels) < 3:
        return False, "too_few_active_rows"
    smooth = metrics.get("smooth_ece")
    if smooth is None or smooth > max_smooth_ece:
        return False, f"smooth_ece {smooth} > {max_smooth_ece}"
    if len(set(labels)) == 2:
        corr = metrics.get("prediction_oracle_correlation")
        if corr is None or corr < target:
            return False, f"correlation {corr} < {target}"
        return True, "mixed_cell_passed"
    target_label = labels[0]
    recall = sum(1 for label, score in zip(labels, scores) if (score >= 0.5) == bool(target_label)) / len(labels)
    brier_value = sum((score - float(label)) ** 2 for label, score in zip(labels, scores)) / len(labels)
    if recall < target:
        return False, f"single_class_recall {recall} < {target}"
    if brier_value > 0.05:
        return False, f"single_class_brier {brier_value} > 0.05"
    return True, "single_class_cell_passed"


def promotion_recommendation(
    panel_a_full: dict[str, Any],
    panel_b_full: dict[str, Any],
    panel_a_active392: dict[str, Any],
    panel_b_active392: dict[str, Any],
    panel_b_parity_active: dict[str, Any],
    *,
    target: float,
) -> dict[str, Any]:
    blockers = []
    full_delta = abs_metric_delta(
        panel_a_full["global"].get("prediction_oracle_correlation"),
        panel_b_full["global"].get("prediction_oracle_correlation"),
    )
    active_delta = abs_metric_delta(
        panel_a_active392["global"].get("prediction_oracle_correlation"),
        panel_b_active392["global"].get("prediction_oracle_correlation"),
    )
    if active_delta is None or active_delta > 0.05:
        blockers.append(f"ACTIVE_392_CROSS_PANEL_DELTA {active_delta} > 0.05")
    active_corr = panel_b_parity_active["global"].get("prediction_oracle_correlation")
    if active_corr is None or active_corr < target:
        blockers.append(f"PANEL_B_PARITY_ACTIVE_CORRELATION {active_corr} < {target}")
    for cell, metrics in panel_b_parity_active["perCell"].items():
        corr = metrics.get("prediction_oracle_correlation")
        if corr is not None and corr < target:
            blockers.append(f"{cell} active correlation {corr} < {target}")
    return {
        "promote": not blockers,
        "blockers": blockers,
        "fullAbsCorrelationDeltaVsPanelA": full_delta,
        "active392AbsCorrelationDeltaVsPanelA": active_delta,
        "nextAction": "run_cross_panel_triangulation_readback" if not blockers else "inspect_parity_feature_gap_by_cell",
    }


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    return {
        "prediction_oracle_correlation": none_delta(
            before.get("prediction_oracle_correlation"),
            after.get("prediction_oracle_correlation"),
        ),
        "brier": none_delta(before.get("brier"), after.get("brier")),
        "smooth_ece": none_delta(before.get("smooth_ece"), after.get("smooth_ece")),
        "fixed_bin_ece": none_delta(before.get("fixed_bin_ece"), after.get("fixed_bin_ece")),
    }


def feature_family_counts(feature_names: list[str]) -> dict[str, int]:
    counts = Counter(feature.split(":", 1)[0] for feature in feature_names)
    return dict(sorted(counts.items()))


def empty_metric_block() -> dict[str, Any]:
    return {
        "n": 0,
        "prediction_oracle_correlation": None,
        "spearman_prediction_oracle_correlation": None,
        "brier": None,
        "fixed_bin_ece": None,
        "smooth_ece": None,
        "accuracy_at_half": None,
        "mean_label": None,
        "mean_score": None,
    }


def iter_jsonl(path: Path) -> list[tuple[int, dict[str, Any]]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_INPUT_MISSING",
            f"required JSONL missing: {path}",
            "Run panel-b-model-scores with retained calibration and holdout outputs first.",
            {"path": str(path)},
        )
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            rows.append((line_number, require_object(json.loads(line), f"{path}:{line_number}")))
    return rows


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_INPUT_MISSING",
            f"{label} missing: {path}",
            "Use retained aiwonder READBACK artifacts only.",
            {"path": str(path)},
        )
    return require_object(json.loads(path.read_text(encoding="utf-8")), label)


def raise_count(label: str, expected: int, actual: int) -> None:
    raise SwebenchOpsError(
        "PANEL_B_PARITY_SCORE_ROW_COUNT_MISMATCH",
        f"{label} row count mismatch",
        "Use row-aligned 240/240 calibration and holdout artifacts.",
        {"label": label, "expected": expected, "actual": actual},
    )


def validate_positive(name: str, value: int) -> None:
    if value <= 0:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_POSITIVE_INT_REQUIRED",
            f"{name} must be positive",
            "Pass a positive integer.",
            {"name": name, "value": value},
        )


def validate_positive_float(name: str, value: float) -> None:
    if not math.isfinite(float(value)) or float(value) <= 0.0:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_POSITIVE_FLOAT_REQUIRED",
            f"{name} must be positive and finite",
            "Pass a positive finite value.",
            {"name": name, "value": value},
        )


def require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_OBJECT_REQUIRED",
            f"{label} must be an object",
            "Fix the artifact schema before parity scoring.",
            {"label": label, "actual_type": type(value).__name__},
        )
    return value


def require_string_array(value: Any, label: str) -> list[str]:
    if not isinstance(value, list):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_STRING_ARRAY_REQUIRED",
            f"{label} must be a string array",
            "Fix the artifact schema before parity scoring.",
            {"label": label, "actual_type": type(value).__name__},
        )
    out = []
    for item in value:
        if not isinstance(item, str) or not item:
            raise SwebenchOpsError(
                "PANEL_B_PARITY_SCORE_STRING_ARRAY_ITEM_INVALID",
                f"{label} contains an invalid item",
                "Emit non-empty string IDs only.",
                {"label": label, "item": item},
            )
        out.append(item)
    return out


def require_float_map(value: Any, label: str) -> dict[str, float]:
    if not isinstance(value, dict) or not value:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_FLOAT_MAP_REQUIRED",
            f"{label} must be a non-empty object",
            "Emit per-slot Panel B scores before parity scoring.",
            {"label": label, "actual_type": type(value).__name__},
        )
    out = {}
    for key, raw in value.items():
        if not isinstance(key, str) or isinstance(raw, bool) or not isinstance(raw, (int, float)):
            raise SwebenchOpsError(
                "PANEL_B_PARITY_SCORE_FLOAT_MAP_ITEM_INVALID",
                f"{label} contains an invalid item",
                "Emit numeric per-slot scores.",
                {"label": label, "key": key, "value": raw},
            )
        out[key] = clamp_probability(float(raw))
    return out


def required_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_STRING_REQUIRED",
            f"{key} must be a non-empty string",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return value


def required_bool(row: dict[str, Any], key: str) -> bool:
    value = row.get(key)
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_BOOL_REQUIRED",
            f"{key} must be boolean",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return value


def optional_bool(row: dict[str, Any], key: str) -> bool | None:
    value = row.get(key)
    if value is None:
        return None
    if not isinstance(value, bool):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_OPTIONAL_BOOL_INVALID",
            f"{key} must be boolean when present",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return value


def required_float(row: dict[str, Any], key: str) -> float:
    value = row.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(float(value)):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_FLOAT_REQUIRED",
            f"{key} must be finite numeric",
            "Fix the artifact schema before parity scoring.",
            {"key": key, "actual": value},
        )
    return float(value)


def dot(weights: list[float], values: list[float]) -> float:
    return sum(weight * value for weight, value in zip(weights, values))


def sigmoid(value: float) -> float:
    if value >= 50.0:
        return 1.0 - 1.0e-9
    if value <= -50.0:
        return 1.0e-9
    return clamp_probability(1.0 / (1.0 + math.exp(-value)))


def logit(value: float) -> float:
    score = clamp_probability(value)
    return math.log(score / (1.0 - score))


def clamp_probability(value: float) -> float:
    if not math.isfinite(float(value)):
        raise SwebenchOpsError(
            "PANEL_B_PARITY_SCORE_NONFINITE_SCORE",
            "non-finite score in Panel B parity scorer",
            "Fix upstream score production before fitting parity.",
            {"score": value},
        )
    return min(1.0 - 1.0e-6, max(1.0e-6, float(value)))


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)


def abs_metric_delta(before: Any, after: Any) -> float | None:
    delta = none_delta(before, after)
    return None if delta is None else abs(delta)
