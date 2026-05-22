from __future__ import annotations

import json
import math
from collections import defaultdict
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .python_pair_feature_audit import (
    DEFAULT_CODE_STATES_JSONL,
    DEFAULT_ROW_DIAGNOSTICS_REPORT,
    load_code_states,
    load_diagnostic_rows,
    pair_feature_row,
    read_json_object,
    write_jsonl,
)
from .python_per_cell_smooth_ece import metric_block, reject_mounted_drive


DEFAULT_PAIR_CANDIDATES_JSONL = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-124-cross-embedder-pair-features-fsv/"
    "run-20260522T010513Z/pair_feature_candidates.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-124-cross-embedder-pair-features-fsv"
)


def add_arguments(parser: Any) -> None:
    parser.add_argument("--code-states-jsonl", type=Path, default=DEFAULT_CODE_STATES_JSONL)
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--pair-candidates-jsonl", type=Path, default=DEFAULT_PAIR_CANDIDATES_JSONL)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--top-k", type=int, default=12)
    parser.add_argument("--epochs", type=int, default=2500)
    parser.add_argument("--learning-rate", type=float, default=0.08)
    parser.add_argument("--l2", type=float, default=0.02)
    parser.add_argument("--expected-code-state-count", type=int, default=3000)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)


def command_python_pair_feature_gate_probe(args: Any) -> dict[str, Any]:
    code_states_jsonl = args.code_states_jsonl.expanduser().resolve(strict=False)
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    pair_candidates_jsonl = args.pair_candidates_jsonl.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("code_states_jsonl", code_states_jsonl),
        ("row_diagnostics_report", row_diagnostics_report),
        ("pair_candidates_jsonl", pair_candidates_jsonl),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.top_k <= 0:
        raise SwebenchOpsError(
            "PY_G_124_PROBE_TOP_K_INVALID",
            "--top-k must be positive",
            "Use a small positive top-k selected from the retained pair audit.",
            {"top_k": args.top_k},
        )
    if args.epochs <= 0 or args.learning_rate <= 0.0 or args.l2 < 0.0:
        raise SwebenchOpsError(
            "PY_G_124_PROBE_OPTIMIZER_INVALID",
            "optimizer parameters must be positive, with non-negative L2",
            "Use deterministic calibration-only optimizer settings.",
            {"epochs": args.epochs, "learning_rate": args.learning_rate, "l2": args.l2},
        )

    states = load_code_states(code_states_jsonl)
    if len(states) != args.expected_code_state_count:
        raise SwebenchOpsError(
            "PY_G_124_PROBE_CODE_STATE_COUNT_MISMATCH",
            "code-state constellation count does not match expected 3000 rows",
            "Use the current aiwonder label root or update the expected count after proving the corpus changed.",
            {"expected": args.expected_code_state_count, "actual": len(states)},
        )
    diagnostics = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration_rows = load_diagnostic_rows(
        diagnostics,
        "score_sharpness_abstain_calibration_row_diagnostics",
        expected=args.expected_calibration_count,
    )
    holdout_rows = load_diagnostic_rows(
        diagnostics,
        "score_sharpness_abstain_holdout_row_diagnostics",
        expected=args.expected_holdout_count,
    )
    candidate_rows = load_pair_candidates(pair_candidates_jsonl)
    selected_features = select_candidate_features(candidate_rows, args.top_k)
    calibration = [pair_feature_row(row, states) for row in calibration_rows]
    holdout = [pair_feature_row(row, states) for row in holdout_rows]

    baseline_calibration = score_rows(calibration_rows, [row["predicted_oracle_pass"] for row in calibration_rows])
    baseline_holdout = score_rows(holdout_rows, [row["predicted_oracle_pass"] for row in holdout_rows])
    feature_stats = fit_feature_stats(calibration, selected_features)
    score_only_model = fit_logistic_model(
        calibration,
        [],
        feature_stats,
        epochs=args.epochs,
        learning_rate=args.learning_rate,
        l2=args.l2,
    )
    pair_model = fit_logistic_model(
        calibration,
        selected_features,
        feature_stats,
        epochs=args.epochs,
        learning_rate=args.learning_rate,
        l2=args.l2,
    )
    score_only_calibration = apply_model(calibration_rows, calibration, [], feature_stats, score_only_model)
    score_only_holdout = apply_model(holdout_rows, holdout, [], feature_stats, score_only_model)
    pair_calibration = apply_model(calibration_rows, calibration, selected_features, feature_stats, pair_model)
    pair_holdout = apply_model(holdout_rows, holdout, selected_features, feature_stats, pair_model)

    output_root.mkdir(parents=True, exist_ok=True)
    pair_rows_path = output_root / "pair_feature_gate_probe_holdout_rows.jsonl"
    write_jsonl(pair_rows_path, pair_holdout)
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_124_pair_feature_gate_probe",
        "githubIssue": 578,
        "ok": True,
        "codeStatesJsonl": str(code_states_jsonl),
        "codeStatesJsonlSha256": sha256_file(code_states_jsonl),
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "pairCandidatesJsonl": str(pair_candidates_jsonl),
        "pairCandidatesSha256": sha256_file(pair_candidates_jsonl),
        "selectedFeatures": selected_features,
        "optimizer": {
            "fitSplit": "calibration only",
            "holdoutUse": "reporting only",
            "epochs": args.epochs,
            "learningRate": args.learning_rate,
            "l2": args.l2,
            "model": "logistic_regression_over_existing_score_plus_selected_pair_features",
        },
        "calibration": {
            "baseline392": split_report(baseline_calibration),
            "scoreOnlyLogistic": split_report(score_only_calibration),
            "scorePlusPairTopK": split_report(pair_calibration),
        },
        "holdout": {
            "baseline392": split_report(baseline_holdout),
            "scoreOnlyLogistic": split_report(score_only_holdout),
            "scorePlusPairTopK": split_report(pair_holdout),
        },
        "deltaVsBaselineHoldout": metric_delta(
            split_report(baseline_holdout)["global"],
            split_report(pair_holdout)["global"],
        ),
        "activeSubsetReportingOnly": {
            "baseline392": split_report([row for row in baseline_holdout if row["active_after_abstain"] is True]),
            "scorePlusPairTopK": split_report([row for row in pair_holdout if row["active_after_abstain"] is True]),
        },
        "promotionRecommendation": promotion_recommendation(baseline_holdout, pair_holdout),
        "outputArtifacts": {
            "pairFeatureGateProbeHoldoutRowsJsonl": str(pair_rows_path),
            "pairFeatureGateProbeHoldoutRowsSha256": sha256_file(pair_rows_path),
        },
        "pathPolicy": {
            "aiwonderOnly": str(code_states_jsonl).startswith("/zfs/archive/contextgraph/")
            and str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(pair_candidates_jsonl).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    report_path = output_root / "pair_feature_gate_probe_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def load_pair_candidates(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_124_PAIR_CANDIDATES_MISSING",
            f"pair candidates missing: {path}",
            "Run python-pair-feature-audit first and use its retained candidates JSONL.",
            {"path": str(path)},
        )
    rows = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            row = json.loads(line)
            feature = row.get("feature")
            if not isinstance(feature, str) or not feature:
                raise SwebenchOpsError(
                    "PY_G_124_PAIR_CANDIDATE_SCHEMA_INVALID",
                    "pair candidate row missing feature",
                    "Regenerate the candidate audit before gate probing.",
                    {"line_number": line_number, "row": row},
                )
            rows.append(row)
    return rows


def select_candidate_features(rows: list[dict[str, Any]], top_k: int) -> list[str]:
    features = []
    seen = set()
    for row in rows:
        feature = str(row["feature"])
        if feature in seen:
            continue
        seen.add(feature)
        features.append(feature)
        if len(features) >= top_k:
            break
    if len(features) < top_k:
        raise SwebenchOpsError(
            "PY_G_124_PAIR_CANDIDATE_COUNT_LOW",
            "not enough pair candidate features for requested top-k",
            "Lower --top-k or rerun the audit with more selected candidates.",
            {"requested": top_k, "available": len(features)},
        )
    return features


def fit_feature_stats(rows: list[dict[str, Any]], features: list[str]) -> dict[str, dict[str, float]]:
    stats = {}
    for feature in features:
        values = [float(row["features"].get(feature, 0.0)) for row in rows]
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / max(1, len(values) - 1)
        std = math.sqrt(variance)
        stats[feature] = {"mean": mean, "std": std if std > 1.0e-6 else 1.0}
    return stats


def fit_logistic_model(
    rows: list[dict[str, Any]],
    features: list[str],
    feature_stats: dict[str, dict[str, float]],
    *,
    epochs: int,
    learning_rate: float,
    l2: float,
) -> list[float]:
    matrix = [design_vector(row, features, feature_stats) for row in rows]
    labels = [float(row["label"]) for row in rows]
    weights = [0.0 for _ in range(len(matrix[0]))]
    for _ in range(epochs):
        grads = [0.0 for _ in weights]
        for x_row, label in zip(matrix, labels):
            predicted = sigmoid(dot(weights, x_row))
            err = predicted - label
            for index, value in enumerate(x_row):
                grads[index] += err * value
        inv_n = 1.0 / len(matrix)
        for index in range(len(weights)):
            penalty = 0.0 if index == 0 else l2 * weights[index]
            weights[index] -= learning_rate * (grads[index] * inv_n + penalty)
    return weights


def apply_model(
    diagnostic_rows: list[dict[str, Any]],
    feature_rows: list[dict[str, Any]],
    features: list[str],
    feature_stats: dict[str, dict[str, float]],
    weights: list[float],
) -> list[dict[str, Any]]:
    out = []
    for diagnostic, row in zip(diagnostic_rows, feature_rows):
        score = sigmoid(dot(weights, design_vector(row, features, feature_stats)))
        out.append(
            {
                "task_id": diagnostic["task_id"],
                "category": diagnostic["category"],
                "bucket": diagnostic["bucket"],
                "actual": int(row["label"]),
                "score": score,
                "baseline_score": float(diagnostic["predicted_oracle_pass"]),
                "active_after_abstain": diagnostic.get("active_after_abstain"),
            }
        )
    return out


def score_rows(diagnostic_rows: list[dict[str, Any]], scores: list[float]) -> list[dict[str, Any]]:
    out = []
    for diagnostic, score in zip(diagnostic_rows, scores):
        out.append(
            {
                "task_id": diagnostic["task_id"],
                "category": diagnostic["category"],
                "bucket": diagnostic["bucket"],
                "actual": 1 if diagnostic["actual_oracle_pass"] else 0,
                "score": float(score),
                "baseline_score": float(diagnostic["predicted_oracle_pass"]),
                "active_after_abstain": diagnostic.get("active_after_abstain"),
            }
        )
    return out


def design_vector(
    row: dict[str, Any],
    features: list[str],
    feature_stats: dict[str, dict[str, float]],
) -> list[float]:
    base_score = clamp_probability(float(row["score"]))
    values = [1.0, logit(base_score)]
    for feature in features:
        raw = float(row["features"].get(feature, 0.0))
        stats = feature_stats[feature]
        values.append((raw - stats["mean"]) / stats["std"])
    return values


def split_report(rows: list[dict[str, Any]]) -> dict[str, Any]:
    labels = [int(row["actual"]) for row in rows]
    scores = [float(row["score"]) for row in rows]
    by_cell: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        by_cell[f"{row['category']}::python"].append(row)
    return {
        "rowCount": len(rows),
        "global": metric_block(labels, scores),
        "perCell": {
            cell: metric_block(
                [int(row["actual"]) for row in cell_rows],
                [float(row["score"]) for row in cell_rows],
            )
            for cell, cell_rows in sorted(by_cell.items())
        },
    }


def metric_delta(baseline: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        "prediction_oracle_correlation": none_delta(
            baseline.get("prediction_oracle_correlation"),
            candidate.get("prediction_oracle_correlation"),
        ),
        "brier": none_delta(baseline.get("brier"), candidate.get("brier")),
        "smooth_ece": none_delta(baseline.get("smooth_ece"), candidate.get("smooth_ece")),
        "fixed_bin_ece": none_delta(baseline.get("fixed_bin_ece"), candidate.get("fixed_bin_ece")),
    }


def promotion_recommendation(baseline_rows: list[dict[str, Any]], candidate_rows: list[dict[str, Any]]) -> dict[str, Any]:
    baseline = split_report(baseline_rows)
    candidate = split_report(candidate_rows)
    delta = metric_delta(baseline["global"], candidate["global"])
    corr_delta = delta["prediction_oracle_correlation"]
    ece_delta = delta["smooth_ece"]
    brier_delta = delta["brier"]
    blockers = []
    if corr_delta is None or corr_delta <= 0.0:
        blockers.append("NO_HOLDOUT_CORRELATION_LIFT")
    if ece_delta is None or ece_delta > 0.003:
        blockers.append("SMOOTHECE_REGRESSION_OVER_BUDGET")
    if brier_delta is None or brier_delta > 0.0:
        blockers.append("BRIER_REGRESSION")
    weak_cells = []
    for cell, block in candidate["perCell"].items():
        corr = block.get("prediction_oracle_correlation")
        if corr is not None and corr < 0.95:
            weak_cells.append({"cell": cell, "correlation": corr})
    if weak_cells:
        blockers.append("PER_CELL_CORRELATION_STILL_BELOW_GATE")
    return {
        "promote": not blockers,
        "blockers": blockers,
        "deltaVsBaseline": delta,
        "weakCells": weak_cells,
        "nextAction": "wire_selected_topk_pair_lane" if not blockers else "do_not_promote_pair_probe_as_scoring_surface",
    }


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
    if not math.isfinite(value):
        raise SwebenchOpsError(
            "PY_G_124_PROBE_NONFINITE_SCORE",
            "non-finite score in pair gate probe",
            "Fix upstream score production before fitting the probe.",
            {"score": value},
        )
    return min(1.0 - 1.0e-6, max(1.0e-6, value))


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)
