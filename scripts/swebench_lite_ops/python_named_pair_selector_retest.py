from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_calibration_active_gate import fit_policies, select_cells
from .python_pair_feature_audit import read_json_object, write_jsonl
from .python_pair_surface_mining import (
    CALIBRATION_KEY,
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_LABEL_MANIFEST,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    HOLDOUT_KEY,
    derived_pair_records,
    load_code_states,
    load_diagnostic_rows,
    require_state,
    target_workspace_state,
)
from .python_per_cell_calibration_substrate import (
    REFERENCE_ACTIVE_BRIER,
    REFERENCE_ACTIVE_CORRELATION,
    REFERENCE_ACTIVE_SMOOTH_ECE,
    apply_selected_policy,
    clipped_logit,
    to_policy_rows,
)
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_CALIBRATION_DECISIONS_JSONL = Path(
    "/zfs/archive/contextgraph/readback/task-py-g-124e-per-cell-calibration-substrate-readback/"
    "run-20260522T040221Z/calibration_key_admission_decisions.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-124c-named-pair-selector-readback")


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--label-manifest", type=Path, default=DEFAULT_LABEL_MANIFEST)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--calibration-decisions-jsonl", type=Path, default=DEFAULT_CALIBRATION_DECISIONS_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)
    parser.add_argument("--target-correlation", type=float, default=0.95)
    parser.add_argument("--smooth-ece-regression-budget", type=float, default=0.003)
    parser.add_argument("--shrinkage-strength", type=float, default=60.0)
    parser.add_argument("--epochs", type=int, default=1800)
    parser.add_argument("--learning-rate", type=float, default=0.04)
    parser.add_argument("--l2", type=float, default=0.08)


def command_python_named_pair_selector_retest(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    label_manifest = args.label_manifest.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    calibration_decisions_jsonl = args.calibration_decisions_jsonl.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("label_manifest", label_manifest),
        ("row_diagnostics_report", row_diagnostics_report),
        ("calibration_decisions_jsonl", calibration_decisions_jsonl),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    validate_args(args)

    output_root.mkdir(parents=True, exist_ok=True)
    label_manifest_payload = read_json_object(label_manifest, "label_manifest")
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    states = load_code_states(code_states_jsonl, expected=int(args.expected_code_state_count))
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
    decisions = load_eligible_decisions(calibration_decisions_jsonl)

    calibration_policy_rows = to_policy_rows(calibration_raw)
    holdout_policy_rows = to_policy_rows(holdout_raw)
    policies = fit_policies(calibration_policy_rows, shrinkage_strength=float(args.shrinkage_strength))
    selections = select_cells(
        calibration_policy_rows,
        policies,
        target=float(args.target_correlation),
        smooth_ece_regression_budget=float(args.smooth_ece_regression_budget),
    )
    calibration_active = apply_selected_policy(calibration_raw, calibration_policy_rows, policies, selections)
    holdout_active = apply_selected_policy(holdout_raw, holdout_policy_rows, policies, selections)
    if len(calibration_active) < 30 or len(holdout_active) < 30:
        raise SwebenchOpsError(
            "PY_G_124C_LOW_ACTIVE_ROWS",
            "substrate active rows are too low for named-pair retest",
            "Repair the calibration substrate before retesting named pairs.",
            {"calibration_active": len(calibration_active), "holdout_active": len(holdout_active)},
        )

    feature_spec = build_feature_spec(decisions)
    calibration_matrix = matrix_for_rows(calibration_active, states, feature_spec)
    holdout_matrix = matrix_for_rows(holdout_active, states, feature_spec)
    stats = feature_stats(calibration_matrix)
    cal_x = standardize_matrix(calibration_matrix, stats)
    hold_x = standardize_matrix(holdout_matrix, stats)
    labels = [int(row["actual"]) for row in calibration_active]
    score_only_columns = [0]
    all_columns = list(range(len(feature_spec)))
    score_only_model = fit_logistic(
        select_columns(cal_x, score_only_columns),
        labels,
        epochs=int(args.epochs),
        learning_rate=float(args.learning_rate),
        l2=float(args.l2),
    )
    pair_model = fit_logistic(
        select_columns(cal_x, all_columns),
        labels,
        epochs=int(args.epochs),
        learning_rate=float(args.learning_rate),
        l2=float(args.l2),
    )
    calibration_score_only = score_rows(
        calibration_active,
        predict_scores(select_columns(cal_x, score_only_columns), score_only_model),
        "substrate_score_only_logistic",
    )
    calibration_pair = score_rows(
        calibration_active,
        predict_scores(select_columns(cal_x, all_columns), pair_model),
        "substrate_plus_named_pair_top20",
    )
    holdout_score_only = score_rows(
        holdout_active,
        predict_scores(select_columns(hold_x, score_only_columns), score_only_model),
        "substrate_score_only_logistic",
    )
    holdout_pair = score_rows(
        holdout_active,
        predict_scores(select_columns(hold_x, all_columns), pair_model),
        "substrate_plus_named_pair_top20",
    )

    decisions_path = output_root / "named_pair_retest_decisions.jsonl"
    holdout_rows_path = output_root / "named_pair_retest_holdout_rows.jsonl"
    report_path = output_root / "named_pair_selector_retest_report.json"
    write_jsonl(decisions_path, decisions)
    write_jsonl(holdout_rows_path, holdout_pair)
    report = {
        "artifactKind": "task_py_g_124c_named_pair_selector_retest",
        "checkedAt": utc_now(),
        "githubIssue": 584,
        "ok": True,
        "inputArtifacts": {
            "codeStatesJsonl": str(code_states_jsonl),
            "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
            "labelManifest": str(label_manifest),
            "labelManifestSha256": sha256_file(label_manifest),
            "rowDiagnosticsReport": str(row_diagnostics_report),
            "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
            "calibrationDecisionsJsonl": str(calibration_decisions_jsonl),
            "calibrationDecisionsJsonlSha256": sha256_file(calibration_decisions_jsonl),
        },
        "labelManifestLineCounts": label_manifest_payload.get("line_counts"),
        "fitPartition": {
            "calibrationActiveRowsUsed": len(calibration_active),
            "holdoutRowsUsedForFitting": 0,
            "holdoutActiveRowsReportingOnly": len(holdout_active),
            "selectionSplit": "calibration_only",
            "decisionsSource": "eligible_for_584_diagnostic_retest_only",
        },
        "featureSpec": {
            "decisionCount": len(decisions),
            "featureCount": len(feature_spec),
            "scoreColumn": feature_spec[0],
            "pairFeatureColumns": feature_spec[1:25],
            "noBroadPairExpansion": True,
        },
        "optimizer": {
            "epochs": int(args.epochs),
            "learningRate": float(args.learning_rate),
            "l2": float(args.l2),
            "model": "logistic_regression_over_substrate_score_plus_eligible_named_pair_measurements",
        },
        "calibration": {
            "substrateBaseline": split_report(calibration_active, "substrate_score"),
            "scoreOnlyLogistic": split_report(calibration_score_only, "retest_score"),
            "scorePlusNamedPairTop20": split_report(calibration_pair, "retest_score"),
        },
        "holdoutReportingOnly": {
            "substrateBaseline": split_report(holdout_active, "substrate_score"),
            "scoreOnlyLogistic": split_report(holdout_score_only, "retest_score"),
            "scorePlusNamedPairTop20": split_report(holdout_pair, "retest_score"),
        },
        "deltaVsSubstrateHoldout": metric_delta(
            split_report(holdout_active, "substrate_score")["global"],
            split_report(holdout_pair, "retest_score")["global"],
        ),
        "promotionGate": promotion_gate(split_report(holdout_pair, "retest_score")["global"]),
        "pathPolicy": {"aiwonderOnly": True, "mntDActiveReadsOrWrites": False},
        "policy": {
            "noHoldoutThresholdTuning": True,
            "noNewEmbedders": True,
            "noNewModelDownload": True,
            "noPredictorPromotion": True,
            "preservesPairIdentity": True,
        },
        "outputArtifacts": {
            "namedPairRetestDecisionsJsonl": str(decisions_path),
            "namedPairRetestDecisionsSha256": sha256_file(decisions_path),
            "namedPairRetestHoldoutRowsJsonl": str(holdout_rows_path),
            "namedPairRetestHoldoutRowsSha256": sha256_file(holdout_rows_path),
        },
        "nextRecommendation": recommendation(split_report(holdout_pair, "retest_score")["global"]),
        "reportPath": str(report_path),
    }
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def validate_args(args: Any) -> None:
    if int(args.epochs) <= 0 or float(args.learning_rate) <= 0.0 or float(args.l2) < 0.0:
        raise SwebenchOpsError(
            "PY_G_124C_BAD_OPTIMIZER_CONFIG",
            "optimizer settings must be positive, with non-negative L2",
            "Use deterministic bounded optimizer settings.",
            {"epochs": args.epochs, "learning_rate": args.learning_rate, "l2": args.l2},
        )


def load_eligible_decisions(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124C_DECISIONS_MISSING",
            f"calibration decisions JSONL missing: {path}",
            "Run #586 per-cell calibration substrate first.",
            {"path": str(path)},
        )
    out = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            if row.get("decision") != "eligible_for_584_diagnostic_retest":
                continue
            pair = row.get("pairId")
            cell = row.get("cell")
            if not isinstance(pair, str) or "/" not in pair or not isinstance(cell, str):
                raise SwebenchOpsError(
                    "PY_G_124C_BAD_DECISION_ROW",
                    "eligible decision row lacks pairId/cell",
                    "Regenerate #586 admission decisions.",
                    {"line": line_number, "row": row},
                )
            out.append(row)
    if not out:
        raise SwebenchOpsError(
            "PY_G_124C_NO_ELIGIBLE_DECISIONS",
            "no eligible named-pair decisions were found",
            "Run #586 and keep #584 blocked if no holes survive.",
            {"path": str(path)},
        )
    return out


def build_feature_spec(decisions: list[dict[str, Any]]) -> list[dict[str, str]]:
    spec = [{"kind": "score_logit", "cell": "*", "head": "*", "pairId": "*", "measure": "substrate_score_logit"}]
    seen = set()
    for decision in decisions:
        identity = (str(decision["cell"]), str(decision["head"]), str(decision["pairId"]))
        if identity in seen:
            continue
        seen.add(identity)
        for measure in ("targetMaxDelta", "deltaFromBase", "stateOrdinal", "driverChanged"):
            spec.append(
                {
                    "kind": "named_pair_measure",
                    "cell": identity[0],
                    "head": identity[1],
                    "pairId": identity[2],
                    "measure": measure,
                }
            )
    return spec


def matrix_for_rows(
    rows: list[dict[str, Any]],
    states: dict[tuple[str, str], dict[str, Any]],
    feature_spec: list[dict[str, str]],
) -> list[list[float]]:
    return [vector_for_row(row, states, feature_spec) for row in rows]


def vector_for_row(
    row: dict[str, Any],
    states: dict[tuple[str, str], dict[str, Any]],
    feature_spec: list[dict[str, str]],
) -> list[float]:
    cell = str(row["cell"])
    base = require_state(states, row["task_id"], "base")
    target = require_state(states, row["task_id"], target_workspace_state(row["category"]))
    base_pairs = derived_pair_records(base)
    target_pairs = derived_pair_records(target)
    values = []
    for spec in feature_spec:
        if spec["kind"] == "score_logit":
            values.append(clipped_logit(float(row["substrate_score"])))
            continue
        if spec["cell"] != cell:
            values.append(0.0)
            continue
        pair_id = spec["pairId"]
        base_pair = base_pairs.get(pair_id)
        target_pair = target_pairs.get(pair_id)
        values.append(pair_measure(target_pair, base_pair, spec["measure"]))
    return [finite_float(value) for value in values]


def pair_measure(target_pair: dict[str, Any] | None, base_pair: dict[str, Any] | None, measure: str) -> float:
    if target_pair is None:
        return 0.0
    target_delta = float(target_pair["max_delta"])
    if measure == "targetMaxDelta":
        return target_delta
    if measure == "deltaFromBase":
        return target_delta - (float(base_pair["max_delta"]) if base_pair is not None else 0.0)
    if measure == "stateOrdinal":
        return pair_state_ordinal(str(target_pair["state"]))
    if measure == "driverChanged":
        return 0.0 if base_pair is None or str(target_pair["driver"]) == str(base_pair.get("driver")) else 1.0
    raise AssertionError(f"unknown measure: {measure}")


def feature_stats(matrix: list[list[float]]) -> list[tuple[float, float]]:
    width = len(matrix[0])
    stats = []
    for col in range(width):
        values = [row[col] for row in matrix]
        mean = sum(values) / len(values)
        var = sum((value - mean) ** 2 for value in values) / max(1, len(values) - 1)
        stats.append((mean, math.sqrt(max(var, 1.0e-12))))
    return stats


def standardize_matrix(matrix: list[list[float]], stats: list[tuple[float, float]]) -> list[list[float]]:
    return [[(value - stats[idx][0]) / stats[idx][1] for idx, value in enumerate(row)] for row in matrix]


def select_columns(matrix: list[list[float]], columns: list[int]) -> list[list[float]]:
    return [[row[col] for col in columns] for row in matrix]


def fit_logistic(
    xs: list[list[float]],
    labels: list[int],
    *,
    epochs: int,
    learning_rate: float,
    l2: float,
) -> list[float]:
    width = len(xs[0]) + 1
    weights = [0.0] * width
    for epoch in range(epochs):
        grads = [0.0] * width
        for row, label in zip(xs, labels):
            features = [1.0, *row]
            pred = sigmoid(sum(weight * feature for weight, feature in zip(weights, features)))
            err = pred - float(label)
            for idx, feature in enumerate(features):
                grads[idx] += err * feature
        lr = learning_rate / math.sqrt(1.0 + epoch / 150.0)
        for idx in range(width):
            penalty = 0.0 if idx == 0 else l2 * weights[idx]
            weights[idx] -= lr * ((grads[idx] / len(xs)) + penalty)
            weights[idx] = max(-10.0, min(10.0, weights[idx]))
    return weights


def predict_scores(xs: list[list[float]], weights: list[float]) -> list[float]:
    scores = []
    for row in xs:
        features = [1.0, *row]
        scores.append(sigmoid(sum(weight * feature for weight, feature in zip(weights, features))))
    return scores


def score_rows(rows: list[dict[str, Any]], scores: list[float], model_name: str) -> list[dict[str, Any]]:
    return [
        {
            **row,
            "retest_score": finite_float(score),
            "retest_model": model_name,
        }
        for row, score in zip(rows, scores)
    ]


def split_report(rows: list[dict[str, Any]], score_key: str) -> dict[str, Any]:
    grouped: dict[str, list[dict[str, Any]]] = {}
    for row in rows:
        grouped.setdefault(str(row["cell"]), []).append(row)
    return {
        "rowCount": len(rows),
        "global": metric_block([row["actual"] for row in rows], [row[score_key] for row in rows]),
        "perCell": {
            cell: metric_block(
                [row["actual"] for row in cell_rows],
                [row[score_key] for row in cell_rows],
            )
            for cell, cell_rows in sorted(grouped.items())
        },
    }


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    keys = ("prediction_oracle_correlation", "brier", "smooth_ece", "fixed_bin_ece", "accuracy_at_half")
    out = {}
    for key in keys:
        if before.get(key) is None or after.get(key) is None:
            out[key] = None
        else:
            out[key] = finite_float(float(after[key]) - float(before[key]))
    return out


def promotion_gate(metrics: dict[str, Any]) -> dict[str, Any]:
    checks = {
        "correlationBeatsOrEqualsReferenceActive392": metrics.get("prediction_oracle_correlation") is not None
        and float(metrics["prediction_oracle_correlation"]) >= REFERENCE_ACTIVE_CORRELATION,
        "brierBeatsOrEqualsReferenceActive392": metrics.get("brier") is not None
        and float(metrics["brier"]) <= REFERENCE_ACTIVE_BRIER,
        "smoothEceBeatsOrEqualsReferenceActive392": metrics.get("smooth_ece") is not None
        and float(metrics["smooth_ece"]) <= REFERENCE_ACTIVE_SMOOTH_ECE,
    }
    return {
        **checks,
        "promotionEligible": all(checks.values()),
        "referenceActive392": {
            "prediction_oracle_correlation": REFERENCE_ACTIVE_CORRELATION,
            "brier": REFERENCE_ACTIVE_BRIER,
            "smooth_ece": REFERENCE_ACTIVE_SMOOTH_ECE,
        },
        "metricsReportingOnly": metrics,
    }


def recommendation(metrics: dict[str, Any]) -> dict[str, Any]:
    gate = promotion_gate(metrics)
    if gate["promotionEligible"]:
        return {
            "recommendedAction": "promote_named_pair_selector_candidate_after_independent_revalidation",
            "reason": "named-pair retest beat or equaled active #392 reference metrics",
        }
    return {
        "recommendedAction": "retain_negative_evidence_and_move_to_next_gate_lever",
        "reason": "targeted named-pair retest did not beat the active #392 reference",
        "promotionGate": gate,
    }


def pair_state_ordinal(state: str) -> float:
    return {
        "near_identity_cross_slot_delta": 0.0,
        "low_cross_slot_delta": 1.0,
        "moderate_cross_slot_delta": 2.0,
        "high_cross_slot_delta": 3.0,
        "extreme_cross_slot_delta": 4.0,
    }.get(state, 0.0)


def sigmoid(value: float) -> float:
    if value >= 0.0:
        z = math.exp(-value)
        return 1.0 / (1.0 + z)
    z = math.exp(value)
    return z / (1.0 + z)
