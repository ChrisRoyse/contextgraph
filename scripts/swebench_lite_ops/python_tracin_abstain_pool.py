from __future__ import annotations

import json
import math
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now
from .python_per_cell_smooth_ece import (
    brier,
    metric_block,
    pearson_float_or_none,
    pearson_or_none,
    rank_values,
    reject_mounted_drive,
    require_array,
    require_object,
    required_bool,
    required_score,
    required_string,
    smooth_ece,
    write_json_checked,
)


DEFAULT_ROW_DIAGNOSTICS_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_TRAINING_ARRAY_ROOT = Path(
    "/zfs/archive/contextgraph/state/training-arrays/constellation-training-arrays-v1-realinput"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/state/tracin/post-fix-real-input-v1")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-123-tracin-abstain-pool-fsv")
DEFAULT_EXACT_GRADIENT_ROOT = Path("/zfs/archive/contextgraph/state/tracin/post-fix-real-input-v2")
WEAK_CELLS = {"delete_test_call", "off_by_one", "subtle_flip"}
TARGET_CORRELATION = 0.95


@dataclass(frozen=True)
class DiagnosticRow:
    row_index: int
    task_id: str
    category: str
    patch_path: str
    label: int
    score: float
    active_after_abstain: bool | None
    evidence_keys: tuple[str, ...]
    features: dict[str, float]
    foundationality_proxy: float


@dataclass(frozen=True)
class InfluenceRow:
    holdout_id: str
    train_id: str
    holdout_category: str
    train_category: str
    influence: float
    similarity: float
    gradient_alignment: float
    train_label: int
    holdout_label: int
    train_score: float
    holdout_score: float


def add_arguments(parser: Any) -> None:
    parser.add_argument("--row-diagnostics-report", type=Path, default=DEFAULT_ROW_DIAGNOSTICS_REPORT)
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--training-array-root", type=Path, default=DEFAULT_TRAINING_ARRAY_ROOT)
    parser.add_argument("--exact-gradient-root", type=Path, default=DEFAULT_EXACT_GRADIENT_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--top-k", type=int, default=8)
    parser.add_argument("--expected-calibration-count", type=int, default=240)
    parser.add_argument("--expected-holdout-count", type=int, default=240)


def command_python_tracin_abstain_pool(args: Any) -> dict[str, Any]:
    row_diagnostics_report = args.row_diagnostics_report.expanduser().resolve(strict=False)
    ship_gate_report = args.ship_gate_report.expanduser().resolve(strict=False)
    training_array_root = args.training_array_root.expanduser().resolve(strict=False)
    exact_gradient_root = args.exact_gradient_root.expanduser().resolve(strict=False)
    output_root = args.output_root.expanduser().resolve(strict=False)
    for name, path in (
        ("row_diagnostics_report", row_diagnostics_report),
        ("ship_gate_report", ship_gate_report),
        ("training_array_root", training_array_root),
        ("exact_gradient_root", exact_gradient_root),
        ("output_root", output_root),
    ):
        reject_mounted_drive(name, path)
    if args.top_k <= 0:
        raise SwebenchOpsError(
            "PY_G_123_TOP_K_INVALID",
            "--top-k must be positive",
            "Use a small positive top-k for influence readback.",
            {"top_k": args.top_k},
        )

    row_payload = read_json_object(row_diagnostics_report, "row_diagnostics_report")
    calibration = load_diagnostic_rows(
        row_payload,
        "score_sharpness_abstain_calibration_row_diagnostics",
        expected=args.expected_calibration_count,
    )
    holdout = load_diagnostic_rows(
        row_payload,
        "score_sharpness_abstain_holdout_row_diagnostics",
        expected=args.expected_holdout_count,
    )
    ship_payload = read_json_object(ship_gate_report, "ship_gate_report")
    ship_execute = require_object(ship_payload.get("execute_and_inspect"), "execute_and_inspect")
    embedded_active = ship_execute.get("task_py_g_392_active_abstain_ship_gate_diagnostic")
    target_rows = [
        row for row in holdout if row.active_after_abstain is False or row.category in WEAK_CELLS
    ]
    inactive_rows = [row for row in holdout if row.active_after_abstain is False]
    weak_rows = [row for row in holdout if row.category in WEAK_CELLS]
    if not target_rows:
        raise SwebenchOpsError(
            "PY_G_123_TARGET_ROWS_EMPTY",
            "no abstain or weak-cell holdout rows were found",
            "Use the current #392 row diagnostics report with active/abstain fields.",
            {"row_diagnostics_report": str(row_diagnostics_report)},
        )

    exact_gradient_artifact = load_exact_gradient_artifact(exact_gradient_root)
    if exact_gradient_artifact["available"]:
        influence_rows, per_holdout = compute_exact_projected_influence(
            calibration,
            target_rows,
            exact_gradient_artifact,
            top_k=args.top_k,
        )
        influence_method = "exact_projected_gradient_rows_v2"
    else:
        influence_rows, per_holdout = compute_influence(calibration, target_rows, top_k=args.top_k)
        influence_method = "projected_gradient_evidence_key_proxy_v1"
    top_bottom = top_bottom_counterfactual(per_holdout, top_k=args.top_k)
    influence_stats = aggregate_influence_stats(calibration, influence_rows)
    foundationality_corr = foundationality_correlation(calibration, influence_rows)
    training_array = summarize_training_array(training_array_root, exact_gradient_artifact)
    recommendation = binary_recommendation(top_bottom, foundationality_corr, training_array)

    output_root.mkdir(parents=True, exist_ok=True)
    influence_jsonl = output_root / "influence_rows.jsonl"
    write_jsonl(influence_jsonl, [row_to_json(row) for row in influence_rows])
    report = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_123_tracin_abstain_pool",
        "githubIssue": 577,
        "ok": True,
        "rowDiagnosticsReport": str(row_diagnostics_report),
        "rowDiagnosticsReportSha256": sha256_file(row_diagnostics_report),
        "shipGateReport": str(ship_gate_report),
        "shipGateReportSha256": sha256_file(ship_gate_report),
        "trainingArrayRoot": str(training_array_root),
        "trainingArray": training_array,
        "exactGradientRoot": str(exact_gradient_root),
        "exactGradientArtifact": exact_gradient_artifact,
        "method": {
            "name": influence_method,
            "exactTracInGradientsAvailable": exact_gradient_artifact["available"],
            "exactTracInBlockedReason": None
            if exact_gradient_artifact["available"]
            else "No retained exact projected gradient artifact was present at the configured aiwonder root.",
            "calibrationRowsUsedAsInfluenceSource": len(calibration),
            "holdoutRowsUsedForEvaluation": len(holdout),
            "featureSpace": "evidence_key_sample + category + ece_bin + score/cause scalar diagnostics",
            "influenceFormula": "-dot((score-label)*x_train, (score-label)*x_holdout)",
            "targetRows": "active_after_abstain=false OR category in delete_test_call/off_by_one/subtle_flip",
            "topK": args.top_k,
        },
        "targetPopulation": {
            "targetRowCount": len(target_rows),
            "inactiveAbstainRowCount": len(inactive_rows),
            "weakCellRowCount": len(weak_rows),
            "targetRowsByCategory": dict(sorted(Counter(row.category for row in target_rows).items())),
            "inactiveRowsByCategory": dict(sorted(Counter(row.category for row in inactive_rows).items())),
            "weakCells": sorted(WEAK_CELLS),
            "embeddedActiveDiagnostic": summarize_embedded_active(embedded_active),
            "targetRowSample": [diagnostic_row_summary(row) for row in target_rows[:20]],
        },
        "influenceStats": influence_stats,
        "foundationality": foundationality_corr,
        "topBottomCounterfactual": top_bottom,
        "outputArtifacts": {
            "influenceRowsJsonl": str(influence_jsonl),
            "influenceRowsSha256": sha256_file(influence_jsonl),
        },
        "recommendation": recommendation,
        "pathPolicy": {
            "aiwonderOnly": str(row_diagnostics_report).startswith("/zfs/archive/contextgraph/")
            and str(ship_gate_report).startswith("/zfs/archive/contextgraph/")
            and str(output_root).startswith("/zfs/archive/contextgraph/"),
            "mntDActiveReadsOrWrites": False,
        },
    }
    report_path = output_root / "tracin_abstain_pool_report.json"
    report["reportPath"] = str(report_path)
    write_json_checked(report_path, report)
    report["reportSha256"] = sha256_file(report_path)
    return report


def read_json_object(path: Path, label: str) -> dict[str, Any]:
    if not path.is_file():
        raise SwebenchOpsError(
            "PY_G_123_INPUT_MISSING",
            f"{label} missing: {path}",
            "Use retained aiwonder FSV artifacts only.",
            {"path": str(path), "label": label},
        )
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SwebenchOpsError(
            "PY_G_123_BAD_JSON",
            f"{label} is not valid JSON: {path}",
            "Regenerate or replace the retained aiwonder artifact.",
            {"path": str(path), "label": label, "error": str(exc)},
        ) from exc
    return require_object(value, label)


def load_diagnostic_rows(payload: dict[str, Any], key: str, *, expected: int) -> list[DiagnosticRow]:
    rows = require_array(payload.get(key), key)
    if len(rows) != expected:
        raise SwebenchOpsError(
            "PY_G_123_ROW_COUNT_MISMATCH",
            "diagnostic row count does not match expected count",
            "Use the current 240-row calibration/holdout diagnostics or pass the expected count.",
            {"key": key, "expected": expected, "actual": len(rows)},
        )
    out = []
    for index, value in enumerate(rows):
        row = require_object(value, f"{key}[{index}]")
        patch_path = required_string(row, "patch_path")
        reject_mounted_drive("patch_path", Path(patch_path))
        score = required_score(row, "predicted_oracle_pass")
        label = 1 if required_bool(row, "actual_oracle_pass") else 0
        features = feature_vector(row)
        out.append(
            DiagnosticRow(
                row_index=int(row.get("row_index", index)),
                task_id=required_string(row, "task_id"),
                category=required_string(row, "category"),
                patch_path=patch_path,
                label=label,
                score=score,
                active_after_abstain=row.get("active_after_abstain")
                if isinstance(row.get("active_after_abstain"), bool)
                else None,
                evidence_keys=tuple(str(item) for item in row.get("evidence_key_sample", []) if isinstance(item, str)),
                features=features,
                foundationality_proxy=foundationality_proxy(row),
            )
        )
    return out


def feature_vector(row: dict[str, Any]) -> dict[str, float]:
    out: dict[str, float] = {}
    category = required_string(row, "category")
    out[f"cat={category}"] = 1.0
    out[f"ece_bin={int(row.get('ece_bin_index', -1))}"] = 0.35
    for key in row.get("evidence_key_sample", []):
        if isinstance(key, str) and key:
            out[f"evidence={key}"] = 1.0
    scalar_names = (
        "predicted_oracle_pass",
        "cause_bin_score",
        "current_rbf_score",
        "selected_knn_score",
        "score_sharpness_score",
        "cause_feature_nonzero_count",
        "cause_feature_large_magnitude_count",
        "evidence_key_count",
        "absolute_error",
    )
    for name in scalar_names:
        raw = row.get(name)
        if isinstance(raw, bool) or not isinstance(raw, (int, float)) or not math.isfinite(float(raw)):
            continue
        value = float(raw)
        if name.endswith("_count"):
            value = math.log1p(max(0.0, value)) / 4.0
        out[f"scalar={name}"] = value
    return normalize_sparse(out)


def normalize_sparse(values: dict[str, float]) -> dict[str, float]:
    norm = math.sqrt(sum(value * value for value in values.values()))
    if norm <= 1.0e-12:
        return values
    return {key: value / norm for key, value in values.items()}


def foundationality_proxy(row: dict[str, Any]) -> float:
    evidence = float(row.get("evidence_key_count") or len(row.get("evidence_key_sample") or []))
    nonzero = float(row.get("cause_feature_nonzero_count") or 0.0)
    large = float(row.get("cause_feature_large_magnitude_count") or 0.0)
    return math.log1p(max(0.0, evidence)) + 0.5 * math.log1p(max(0.0, nonzero)) + 0.75 * math.log1p(max(0.0, large))


def compute_influence(
    train_rows: list[DiagnosticRow],
    holdout_rows: list[DiagnosticRow],
    *,
    top_k: int,
) -> tuple[list[InfluenceRow], dict[str, list[InfluenceRow]]]:
    all_rows: list[InfluenceRow] = []
    per_holdout: dict[str, list[InfluenceRow]] = {}
    for holdout in holdout_rows:
        scored = []
        h_grad = holdout.score - holdout.label
        for train in train_rows:
            t_grad = train.score - train.label
            similarity = sparse_dot(train.features, holdout.features)
            gradient_alignment = t_grad * h_grad * similarity
            influence = -gradient_alignment
            scored.append(
                InfluenceRow(
                    holdout_id=row_id(holdout),
                    train_id=row_id(train),
                    holdout_category=holdout.category,
                    train_category=train.category,
                    influence=influence,
                    similarity=similarity,
                    gradient_alignment=gradient_alignment,
                    train_label=train.label,
                    holdout_label=holdout.label,
                    train_score=train.score,
                    holdout_score=holdout.score,
                )
            )
        ranked = sorted(scored, key=lambda item: (-abs(item.influence), -item.similarity, item.train_id))
        selected = ranked[: max(top_k, 1)] + ranked[-max(top_k, 1) :]
        per_holdout[row_id(holdout)] = ranked
        all_rows.extend(selected)
    return all_rows, per_holdout


def compute_exact_projected_influence(
    train_rows: list[DiagnosticRow],
    holdout_rows: list[DiagnosticRow],
    exact_artifact: dict[str, Any],
    *,
    top_k: int,
) -> tuple[list[InfluenceRow], dict[str, list[InfluenceRow]]]:
    gradients = exact_artifact["diagnosticGradientsById"]
    all_rows: list[InfluenceRow] = []
    per_holdout: dict[str, list[InfluenceRow]] = {}
    for holdout in holdout_rows:
        holdout_id = row_id(holdout)
        holdout_gradient = gradients.get(holdout_id)
        if holdout_gradient is None:
            raise SwebenchOpsError(
                "PY_G_123_EXACT_HOLDOUT_GRADIENT_MISSING",
                "exact gradient artifact is missing a target holdout row",
                "Regenerate TASK-PY-G-126 against the same row diagnostics report.",
                {"holdout_id": holdout_id, "exact_gradient_root": exact_artifact.get("root")},
            )
        scored: list[InfluenceRow] = []
        for train in train_rows:
            train_id = row_id(train)
            train_gradient = gradients.get(train_id)
            if train_gradient is None:
                raise SwebenchOpsError(
                    "PY_G_123_EXACT_SOURCE_GRADIENT_MISSING",
                    "exact gradient artifact is missing a calibration source row",
                    "Regenerate TASK-PY-G-126 against the same row diagnostics report.",
                    {"train_id": train_id, "exact_gradient_root": exact_artifact.get("root")},
                )
            gradient_alignment = vector_dot(train_gradient, holdout_gradient)
            influence = -gradient_alignment
            similarity = gradient_cosine(train_gradient, holdout_gradient)
            scored.append(
                InfluenceRow(
                    holdout_id=holdout_id,
                    train_id=train_id,
                    holdout_category=holdout.category,
                    train_category=train.category,
                    influence=influence,
                    similarity=similarity,
                    gradient_alignment=gradient_alignment,
                    train_label=train.label,
                    holdout_label=holdout.label,
                    train_score=train.score,
                    holdout_score=holdout.score,
                )
            )
        ranked = sorted(scored, key=lambda item: (-abs(item.influence), item.train_id))
        selected = ranked[: max(top_k, 1)] + ranked[-max(top_k, 1) :]
        per_holdout[holdout_id] = ranked
        all_rows.extend(selected)
    return all_rows, per_holdout


def sparse_dot(left: dict[str, float], right: dict[str, float]) -> float:
    if len(left) > len(right):
        left, right = right, left
    return sum(value * right.get(key, 0.0) for key, value in left.items())


def vector_dot(left: list[float], right: list[float]) -> float:
    return sum(a * b for a, b in zip(left, right))


def gradient_cosine(left: list[float], right: list[float]) -> float:
    denom = math.sqrt(vector_dot(left, left)) * math.sqrt(vector_dot(right, right))
    if denom <= 1.0e-12:
        return 0.0
    return vector_dot(left, right) / denom


def top_bottom_counterfactual(per_holdout: dict[str, list[InfluenceRow]], *, top_k: int) -> dict[str, Any]:
    top_labels: list[int] = []
    top_scores: list[float] = []
    bottom_labels: list[int] = []
    bottom_scores: list[float] = []
    samples: list[dict[str, Any]] = []
    for holdout_id, rows in sorted(per_holdout.items()):
        top = rows[:top_k]
        bottom = rows[-top_k:]
        top_score = weighted_label_score(top)
        bottom_score = weighted_label_score(bottom)
        label = rows[0].holdout_label
        top_labels.append(label)
        bottom_labels.append(label)
        top_scores.append(top_score)
        bottom_scores.append(bottom_score)
        if len(samples) < 20:
            samples.append(
                {
                    "holdoutId": holdout_id,
                    "category": rows[0].holdout_category,
                    "label": label,
                    "currentScore": rows[0].holdout_score,
                    "topKCounterfactualScore": top_score,
                    "bottomKCounterfactualScore": bottom_score,
                    "topKTrainIds": [row.train_id for row in top[:5]],
                    "bottomKTrainIds": [row.train_id for row in bottom[:5]],
                }
            )
    top_metrics = metric_block(top_labels, top_scores)
    bottom_metrics = metric_block(bottom_labels, bottom_scores)
    top_brier = brier(top_labels, top_scores)
    bottom_brier = brier(bottom_labels, bottom_scores)
    return {
        "topK": top_k,
        "topKMetrics": top_metrics,
        "bottomKMetrics": bottom_metrics,
        "topMinusBottomBrierDelta": None
        if top_brier is None or bottom_brier is None
        else top_brier - bottom_brier,
        "topKBeatsBottomK": bool(top_brier is not None and bottom_brier is not None and top_brier + 0.02 < bottom_brier),
        "sampleRows": samples,
    }


def weighted_label_score(rows: list[InfluenceRow]) -> float:
    weights = [max(1.0e-6, abs(row.influence) + max(0.0, row.similarity) * 0.1) for row in rows]
    denom = sum(weights)
    if denom <= 0.0:
        return sum(row.train_label for row in rows) / len(rows)
    return sum(weight * row.train_label for weight, row in zip(weights, rows)) / denom


def aggregate_influence_stats(train_rows: list[DiagnosticRow], influence_rows: list[InfluenceRow]) -> dict[str, Any]:
    by_train: dict[str, float] = defaultdict(float)
    by_pair: dict[str, float] = defaultdict(float)
    for row in influence_rows:
        by_train[row.train_id] += abs(row.influence)
        by_pair[f"{row.train_category}->{row.holdout_category}"] += abs(row.influence)
    ranked = sorted(by_train.items(), key=lambda item: (-item[1], item[0]))
    return {
        "retainedInfluenceRowCount": len(influence_rows),
        "uniqueTrainRowsWithInfluence": len(by_train),
        "topTrainRows": [{"trainId": key, "absInfluence": value} for key, value in ranked[:20]],
        "categoryPairAbsInfluence": dict(sorted(by_pair.items(), key=lambda item: (-item[1], item[0]))[:30]),
        "trainCategoryCounts": dict(sorted(Counter(row.category for row in train_rows).items())),
    }


def foundationality_correlation(train_rows: list[DiagnosticRow], influence_rows: list[InfluenceRow]) -> dict[str, Any]:
    by_train: dict[str, float] = defaultdict(float)
    for row in influence_rows:
        by_train[row.train_id] += abs(row.influence)
    foundation_by_id = {row_id(row): row.foundationality_proxy for row in train_rows}
    paired = [(foundation_by_id[key], influence) for key, influence in by_train.items() if key in foundation_by_id]
    if len(paired) < 3:
        return {"rowCount": len(paired), "pearson": None, "spearman": None, "source": "foundationality_proxy"}
    xs = [item[0] for item in paired]
    ys = [item[1] for item in paired]
    return {
        "rowCount": len(paired),
        "pearson": pearson_float_or_none(xs, ys),
        "spearman": pearson_float_or_none(rank_values(xs), rank_values(ys)),
        "source": "foundationality_proxy_from_evidence_and_cause_feature_counts",
    }


def load_exact_gradient_artifact(root: Path) -> dict[str, Any]:
    manifest_path = root / "exact_tracin_gradient_manifest.json"
    gradients_path = root / "diagnostic_projected_gradients.jsonl"
    if not manifest_path.is_file() or not gradients_path.is_file():
        return {
            "available": False,
            "root": str(root),
            "reason": "exact_gradient_manifest_or_diagnostic_rows_missing",
        }
    manifest = read_json_object(manifest_path, "exact_gradient_manifest")
    if manifest.get("ok") is not True:
        return {
            "available": False,
            "root": str(root),
            "reason": "exact_gradient_manifest_not_ok",
            "manifestPath": str(manifest_path),
        }
    gradients: dict[str, list[float]] = {}
    split_counts: Counter[str] = Counter()
    category_counts: Counter[str] = Counter()
    with gradients_path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            row = require_object(json.loads(line), f"{gradients_path}:{line_no}")
            row_id_value = required_string(row, "rowId")
            vector = require_array(row.get("projectedGradient"), "projectedGradient")
            gradients[row_id_value] = [float(value) for value in vector]
            split_counts[str(row.get("split"))] += 1
            category_counts[str(row.get("category"))] += 1
    return {
        "available": True,
        "root": str(root),
        "manifestPath": str(manifest_path),
        "manifestSha256": sha256_file(manifest_path),
        "diagnosticGradientsPath": str(gradients_path),
        "diagnosticGradientsSha256": sha256_file(gradients_path),
        "diagnosticGradientsById": gradients,
        "diagnosticGradientRowCount": len(gradients),
        "splitCounts": dict(sorted(split_counts.items())),
        "categoryCounts": dict(sorted(category_counts.items())),
        "method": manifest.get("method"),
    }


def summarize_training_array(root: Path, exact_artifact: dict[str, Any]) -> dict[str, Any]:
    rows_path = root / "constellation_training_rows.jsonl"
    manifest_path = root / "constellation_training_manifest.json"
    certificate_path = root / "constellation_training_certificate.json"
    if not rows_path.is_file():
        return {"present": False, "reason": "constellation_training_rows_missing", "root": str(root)}
    partition_counts: Counter[str] = Counter()
    category_counts: Counter[str] = Counter()
    with rows_path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            row = json.loads(line)
            partition_counts[str(row.get("partition"))] += 1
            category_counts[str(row.get("mutation_category"))] += 1
    return {
        "present": True,
        "root": str(root),
        "rowsPath": str(rows_path),
        "rowsSha256": sha256_file(rows_path),
        "manifestSha256": sha256_file(manifest_path) if manifest_path.is_file() else None,
        "certificateSha256": sha256_file(certificate_path) if certificate_path.is_file() else None,
        "partitionCounts": dict(sorted(partition_counts.items())),
        "categoryCounts": dict(sorted(category_counts.items())),
        "exactTracInReady": bool(exact_artifact.get("available")),
        "exactTracInBlocker": None
        if exact_artifact.get("available")
        else "No retained exact projected gradient rows found with this row set.",
    }


def binary_recommendation(
    top_bottom: dict[str, Any],
    foundationality: dict[str, Any],
    training_array: dict[str, Any],
) -> dict[str, Any]:
    top_metrics = top_bottom["topKMetrics"]
    top_corr = top_metrics.get("prediction_oracle_correlation")
    brier_delta = top_bottom.get("topMinusBottomBrierDelta")
    existing_data_signal = (
        top_bottom.get("topKBeatsBottomK") is True
        and top_corr is not None
        and top_corr >= 0.5
        and foundationality.get("spearman") is not None
    )
    if not training_array.get("exactTracInReady"):
        action = "materialize_exact_gradient_rows_before_broad_feature_expansion"
    elif existing_data_signal:
        action = "existing_data_levers_remain_justified"
    else:
        action = "pivot_to_new_measurement_axes_before_feature_expansion"
    return {
        "existingDataLeversRemainJustified": existing_data_signal,
        "recommendedAction": action,
        "rationale": {
            "topKBeatsBottomK": top_bottom.get("topKBeatsBottomK"),
            "topKCorrelation": top_corr,
            "topMinusBottomBrierDelta": brier_delta,
            "foundationalitySpearman": foundationality.get("spearman"),
            "exactGradientRowsReady": training_array.get("exactTracInReady"),
        },
    }


def summarize_embedded_active(raw: Any) -> dict[str, Any]:
    if not isinstance(raw, dict):
        return {"present": False}
    metrics = raw.get("active_metrics") if isinstance(raw.get("active_metrics"), dict) else {}
    return {
        "present": True,
        "activeRowCount": raw.get("active_row_count"),
        "abstainedRowCount": raw.get("abstained_row_count"),
        "activeCoverage": raw.get("active_coverage"),
        "activeCorrelation": metrics.get("prediction_oracle_correlation"),
        "activeBrier": metrics.get("brier"),
        "activeEce": metrics.get("ece"),
        "blockers": raw.get("active_metric_blockers"),
    }


def diagnostic_row_summary(row: DiagnosticRow) -> dict[str, Any]:
    return {
        "rowIndex": row.row_index,
        "taskId": row.task_id,
        "category": row.category,
        "patchPath": row.patch_path,
        "label": row.label,
        "score": row.score,
        "activeAfterAbstain": row.active_after_abstain,
        "evidenceKeyCount": len(row.evidence_keys),
        "foundationalityProxy": row.foundationality_proxy,
    }


def row_to_json(row: InfluenceRow) -> dict[str, Any]:
    return {
        "holdoutId": row.holdout_id,
        "trainId": row.train_id,
        "holdoutCategory": row.holdout_category,
        "trainCategory": row.train_category,
        "influence": row.influence,
        "similarity": row.similarity,
        "gradientAlignment": row.gradient_alignment,
        "trainLabel": row.train_label,
        "holdoutLabel": row.holdout_label,
        "trainScore": row.train_score,
        "holdoutScore": row.holdout_score,
    }


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    temp = path.with_name(f"{path.name}.tmp")
    with temp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    temp.replace(path)
    count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if line.strip():
                json.loads(line)
                count += 1
    if count != len(rows):
        raise SwebenchOpsError(
            "PY_G_123_JSONL_READBACK_MISMATCH",
            "JSONL readback count mismatch",
            "Check filesystem integrity and concurrent writers.",
            {"path": str(path), "expected": len(rows), "actual": count},
        )


def row_id(row: DiagnosticRow) -> str:
    return f"{row.task_id}|{row.category}|{row.row_index}"
