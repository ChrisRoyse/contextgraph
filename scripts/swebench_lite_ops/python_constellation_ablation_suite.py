"""TASK-PY-G-093 capability-discovery ablations for constellations.

The suite consumes the real aiwonder TASK-PY-G-096 training arrays and compares
feature families without letting flat-vector baselines become ship-gate evidence.
"""

from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

import numpy as np

from .python_constellation_training_objective import HISTORICAL_BASELINE_CORRELATION
from .python_constellation_training_objective import accuracy_at_half, brier, calibration_ece, pearson_or_none
from .python_real_corpus_constellation_smoke import finite_float, sha256_text, write_jsonl
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-093"
ARTIFACT_KIND = "python_constellation_ablation_suite"
FORMULA_VERSION = "slot_preserving_constellation_ablation_suite_v1"
DEFAULT_ARTIFACT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv/artifacts")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-093-constellation-ablation-suite-fsv")


class ConstellationAblationSuiteError(RuntimeError):
    pass


@dataclass(frozen=True)
class ScalarModel:
    feature: str
    center: float
    scale: float
    direction: float
    train_good_mean: float
    train_bad_mean: float


@dataclass(frozen=True)
class AblationSpec:
    name: str
    features: tuple[str, ...]
    include_temporal: bool
    flat_vector_baseline: bool
    ship_gate_eligible: bool
    input_policy: str


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise ConstellationAblationSuiteError(code if detail is None else f"{code}: {detail}")


def label(row: dict[str, Any]) -> int:
    value = int(row.get("labels", {}).get("oracle_known_bad"))
    require(value in {0, 1}, "MEJEPA_CONSTELLATION_ABLATION_LABEL_INVALID", str(value))
    return value


def feature_value(row: dict[str, Any], feature: str) -> float:
    if feature.startswith("flat_"):
        value = row.get("flat_baseline_features", {}).get(feature)
    else:
        value = row.get("objective_features", {}).get(feature)
    require(isinstance(value, (int, float)), "MEJEPA_CONSTELLATION_ABLATION_FEATURE_MISSING", feature)
    require(math.isfinite(float(value)), "MEJEPA_CONSTELLATION_ABLATION_FEATURE_NONFINITE", feature)
    return finite_float(float(value))


def load_required_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    require(path.exists(), code, str(path))
    rows = [row for _line_no, row in iter_jsonl(path)]
    require(bool(rows), code.replace("MISSING", "EMPTY"), str(path))
    return rows


def load_artifacts(root: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    require(root.exists(), "MEJEPA_CONSTELLATION_ABLATION_ARTIFACT_ROOT_MISSING", str(root))
    rows = load_required_jsonl(root / "constellation_training_rows.jsonl", "MEJEPA_CONSTELLATION_ABLATION_TRAINING_ROWS_MISSING")
    slots = load_required_jsonl(root / "constellation_slot_vectors.jsonl", "MEJEPA_CONSTELLATION_ABLATION_SLOT_VECTORS_MISSING")
    certificate_path = root / "constellation_training_certificate.json"
    require(certificate_path.exists(), "MEJEPA_CONSTELLATION_ABLATION_CERTIFICATE_MISSING", str(certificate_path))
    certificate = read_json(certificate_path)
    require(certificate.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_UPSTREAM_CERTIFICATE_NOT_PASSING")
    policy = certificate.get("objective_policy") or {}
    require(policy.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_UPSTREAM_FLAT_VECTOR_USED")
    return rows, slots, certificate


def validate_rows(rows: list[dict[str, Any]], min_train: int, min_test: int) -> dict[str, Any]:
    partitions = Counter()
    labels_by_partition: dict[str, Counter] = defaultdict(Counter)
    flat_flags = []
    for row in rows:
        partitions[str(row.get("partition"))] += 1
        labels_by_partition[str(row.get("partition"))][label(row)] += 1
        flat_flags.append(bool(row.get("slot_policy", {}).get("flat_vector_concat_used")))
        require(row.get("slot_policy", {}).get("slot_identity_preserved") is True, "MEJEPA_CONSTELLATION_ABLATION_SLOT_IDENTITY_NOT_PRESERVED")
        require(isinstance(row.get("slots"), list) and row["slots"], "MEJEPA_CONSTELLATION_ABLATION_ROW_SLOTS_MISSING")
    require(not any(flat_flags), "MEJEPA_CONSTELLATION_ABLATION_ROW_FLAT_VECTOR_FLAGGED")
    require(partitions["train"] >= min_train, "MEJEPA_CONSTELLATION_ABLATION_TRAIN_ROWS_TOO_LOW", str(partitions["train"]))
    require(partitions["test"] >= min_test, "MEJEPA_CONSTELLATION_ABLATION_TEST_ROWS_TOO_LOW", str(partitions["test"]))
    require(set(labels_by_partition["train"]) == {0, 1}, "MEJEPA_CONSTELLATION_ABLATION_TRAIN_LABELS_DEGENERATE")
    require(set(labels_by_partition["test"]) == {0, 1}, "MEJEPA_CONSTELLATION_ABLATION_TEST_LABELS_DEGENERATE")
    return {
        "row_count": len(rows),
        "partition_counts": dict(sorted(partitions.items())),
        "label_counts_by_partition": {part: dict(sorted(counts.items())) for part, counts in sorted(labels_by_partition.items())},
    }


def attach_flat_baseline_features(rows: list[dict[str, Any]], slot_rows: list[dict[str, Any]]) -> None:
    accum: dict[str, dict[str, float]] = defaultdict(lambda: {"sum_sq": 0.0, "sum_abs": 0.0, "sum": 0.0, "count": 0.0})
    for slot in slot_rows:
        require(slot.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_SLOT_VECTOR_FLAT_FLAGGED")
        vector = slot.get("vector")
        require(isinstance(vector, list) and vector, "MEJEPA_CONSTELLATION_ABLATION_SLOT_VECTOR_MISSING")
        stats = accum[str(slot.get("row_key"))]
        for raw in vector:
            value = float(raw)
            require(math.isfinite(value), "MEJEPA_CONSTELLATION_ABLATION_SLOT_VECTOR_NONFINITE")
            stats["sum_sq"] += value * value
            stats["sum_abs"] += abs(value)
            stats["sum"] += value
            stats["count"] += 1.0
    for row in rows:
        stats = accum.get(str(row.get("row_key")))
        require(stats is not None and stats["count"] > 0, "MEJEPA_CONSTELLATION_ABLATION_FLAT_BASELINE_ROW_MISSING", str(row.get("row_key")))
        row["flat_baseline_features"] = {
            "flat_concat_norm_l2": finite_float(math.sqrt(stats["sum_sq"])),
            "flat_concat_abs_mean": finite_float(stats["sum_abs"] / stats["count"]),
            "flat_concat_signed_mean": finite_float(stats["sum"] / stats["count"]),
        }


def split_rows(rows: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    return [row for row in rows if row["partition"] == "train"], [row for row in rows if row["partition"] == "test"]


def fit_scalar_model(train: list[dict[str, Any]], feature: str) -> ScalarModel:
    good = [feature_value(row, feature) for row in train if label(row) == 0]
    bad = [feature_value(row, feature) for row in train if label(row) == 1]
    require(bool(good) and bool(bad), "MEJEPA_CONSTELLATION_ABLATION_MODEL_LABELS_DEGENERATE", feature)
    values = np.asarray(good + bad, dtype=np.float64)
    good_mean = float(np.mean(good))
    bad_mean = float(np.mean(bad))
    return ScalarModel(
        feature=feature,
        center=finite_float((good_mean + bad_mean) / 2.0),
        scale=finite_float(max(1.0e-9, float(np.std(values)))),
        direction=finite_float(1.0 if bad_mean >= good_mean else -1.0),
        train_good_mean=finite_float(good_mean),
        train_bad_mean=finite_float(bad_mean),
    )


def sigmoid(value: float) -> float:
    return finite_float(1.0 / (1.0 + math.exp(max(-40.0, min(40.0, -value)))))


def scalar_probability(row: dict[str, Any], model: ScalarModel) -> float:
    return sigmoid(model.direction * (feature_value(row, model.feature) - model.center) / model.scale)


def temporal_probability_maps(train: list[dict[str, Any]]) -> tuple[float, dict[str, float]]:
    global_rate = finite_float(float(np.mean([label(row) for row in train])))
    grouped: dict[str, list[int]] = defaultdict(list)
    for row in train:
        key = temporal_key(row)
        grouped[key].append(label(row))
    rates = {key: finite_float((sum(values) + global_rate) / (len(values) + 1.0)) for key, values in grouped.items()}
    return global_rate, rates


def temporal_key(row: dict[str, Any]) -> str:
    return f"{row.get('cell_key')}|{int(row.get('temporal', {}).get('window_index', 0))}"


def evaluate_spec(spec: AblationSpec, train: list[dict[str, Any]], test: list[dict[str, Any]], ece_bins: int) -> dict[str, Any]:
    models = [fit_scalar_model(train, feature) for feature in spec.features]
    global_rate, temporal_rates = temporal_probability_maps(train)
    train_probs = [row_probability(row, models, spec.include_temporal, global_rate, temporal_rates) for row in train]
    test_probs = [row_probability(row, models, spec.include_temporal, global_rate, temporal_rates) for row in test]
    metrics = metric_block([label(row) for row in test], test_probs, ece_bins)
    return {
        "name": spec.name,
        "schema_version": SCHEMA_VERSION,
        "input_policy": spec.input_policy,
        "features": list(spec.features),
        "include_temporal": spec.include_temporal,
        "flat_vector_concat_used": spec.flat_vector_baseline,
        "ship_gate_eligible": spec.ship_gate_eligible,
        "model_count": len(models) + (1 if spec.include_temporal else 0),
        "models": [model.__dict__ for model in models],
        "train_metrics": metric_block([label(row) for row in train], train_probs, ece_bins),
        "test_metrics": metrics,
        "per_cell_metrics": per_cell_metrics(test, test_probs, ece_bins),
        "anti_compensation": anti_compensation_check(spec, train, test, test_probs),
    }


def row_probability(row: dict[str, Any], models: list[ScalarModel], include_temporal: bool, global_rate: float, temporal_rates: dict[str, float]) -> float:
    probs = [scalar_probability(row, model) for model in models]
    if include_temporal:
        probs.append(finite_float(temporal_rates.get(temporal_key(row), global_rate)))
    require(bool(probs), "MEJEPA_CONSTELLATION_ABLATION_NO_MODEL_COMPONENTS")
    return finite_float(float(np.mean(probs)))


def metric_block(labels: list[int], probabilities: list[float], ece_bins: int) -> dict[str, Any]:
    corr, corr_reason = pearson_or_none(labels, probabilities)
    return {
        "row_count": len(labels),
        "brier": brier(labels, probabilities),
        "ece": calibration_ece(labels, probabilities, ece_bins),
        "accuracy_at_0_5": accuracy_at_half(labels, probabilities),
        "precision_at_0_5": precision(labels, probabilities),
        "recall_at_0_5": recall(labels, probabilities),
        "prediction_oracle_correlation": corr,
        "prediction_oracle_correlation_validity": corr_reason,
    }


def precision(labels: list[int], probabilities: list[float]) -> float | None:
    predicted = [idx for idx, prob in enumerate(probabilities) if prob >= 0.5]
    if not predicted:
        return None
    return finite_float(sum(1 for idx in predicted if labels[idx] == 1) / len(predicted))


def recall(labels: list[int], probabilities: list[float]) -> float | None:
    positives = [idx for idx, value in enumerate(labels) if value == 1]
    if not positives:
        return None
    return finite_float(sum(1 for idx in positives if probabilities[idx] >= 0.5) / len(positives))


def per_cell_metrics(rows: list[dict[str, Any]], probabilities: list[float], ece_bins: int) -> dict[str, Any]:
    grouped: dict[str, list[tuple[int, float]]] = defaultdict(list)
    for row, prob in zip(rows, probabilities):
        grouped[str(row.get("cell_key"))].append((label(row), prob))
    return {
        key: metric_block([item[0] for item in values], [item[1] for item in values], ece_bins)
        for key, values in sorted(grouped.items())
        if len(values) >= 2
    }


def anti_compensation_check(spec: AblationSpec, train: list[dict[str, Any]], test: list[dict[str, Any]], test_probs: list[float]) -> dict[str, Any]:
    threshold = finite_float(float(np.mean([feature_value(row, "slot_max_abs_z") for row in train])) + float(np.std([feature_value(row, "slot_max_abs_z") for row in train])))
    stress = [(row, prob) for row, prob in zip(test, test_probs) if feature_value(row, "slot_max_abs_z") >= threshold]
    bad_stress = [(row, prob) for row, prob in stress if label(row) == 1]
    recall_value = None if not bad_stress else finite_float(sum(1 for _, prob in bad_stress if prob >= 0.5) / len(bad_stress))
    has_slot_failure_signal = any(feature in spec.features for feature in ("slot_max_abs_z", "contradiction_score"))
    passes = (not spec.ship_gate_eligible) or (has_slot_failure_signal and (recall_value is None or recall_value >= 0.5))
    return {
        "stress_threshold_slot_max_abs_z": threshold,
        "stress_rows": len(stress),
        "bad_stress_rows": len(bad_stress),
        "bad_stress_recall_at_0_5": recall_value,
        "uses_slot_local_failure_signal": has_slot_failure_signal,
        "passes": passes,
    }


def evaluate_rate_baseline(name: str, train: list[dict[str, Any]], test: list[dict[str, Any]], key_fn: Callable[[dict[str, Any]], str], ece_bins: int) -> dict[str, Any]:
    global_rate = finite_float(float(np.mean([label(row) for row in train])))
    grouped: dict[str, list[int]] = defaultdict(list)
    for row in train:
        grouped[key_fn(row)].append(label(row))
    rates = {key: finite_float((sum(values) + global_rate) / (len(values) + 1.0)) for key, values in grouped.items()}
    probabilities = [finite_float(rates.get(key_fn(row), global_rate)) for row in test]
    return {
        "name": name,
        "ship_gate_eligible": False,
        "flat_vector_concat_used": False,
        "test_metrics": metric_block([label(row) for row in test], probabilities, ece_bins),
        "rate_count": len(rates),
    }


def ablation_specs() -> list[AblationSpec]:
    slot = ("slot_mean_abs_z", "slot_max_abs_z", "slot_spread_abs_z", "slot_uncertainty_mean")
    pairwise = ("pairwise_disagreement_score", "pairwise_max_scalar_delta")
    contradiction = ("contradiction_score", "consensus_score")
    flat = ("flat_concat_norm_l2", "flat_concat_abs_mean", "flat_concat_signed_mean")
    return [
        AblationSpec("flat_vector_baseline_comparison_only", flat, False, True, False, "comparison baseline; concatenates slots and is never ship-gate eligible"),
        AblationSpec("per_slot_independent_only", slot, False, False, True, "separate per-slot scalar summaries"),
        AblationSpec("per_slot_plus_pairwise", slot + pairwise, False, False, True, "per-slot summaries plus cross-embedder relationship scalars"),
        AblationSpec("per_slot_pairwise_temporal", slot + pairwise, True, False, True, "slot plus pairwise plus aiwonder shard/window rates"),
        AblationSpec("per_slot_contradiction_consensus", slot + contradiction, False, False, True, "slot summaries plus contradiction and consensus heads"),
        AblationSpec("full_constellation", slot + pairwise + contradiction, True, False, True, "all available #396 slot, pairwise, contradiction, and temporal signals"),
    ]


def measurement_gaps(rows: list[dict[str, Any]]) -> dict[str, Any]:
    q4_present = sum(1 for row in rows if row.get("labels", {}).get("q4_labels", {}).get("present") is True)
    q5_present = sum(1 for row in rows if row.get("labels", {}).get("q5_reality_impact", {}).get("present") is True)
    return {
        "ood_recall": {"measured": False, "reason": "no OOD labels exist in the #396 training rows", "blocked_by": ["#389", "#276", "#129"]},
        "q4_calibration": {"measured": q4_present > 0, "present_rows": q4_present, "reason": None if q4_present else "no Q4 labels exist in the #396 training rows", "blocked_by": ["#266", "#267", "#268", "#269", "#127"]},
        "q5_replay_lift": {"measured": q5_present > 0, "present_rows": q5_present, "reason": None if q5_present else "no Q5 replay labels exist in the #396 training rows", "blocked_by": ["#264", "#128"]},
    }


def rank_and_verdicts(ablations: list[dict[str, Any]], baselines: list[dict[str, Any]], historical: float) -> dict[str, Any]:
    best_local_brier = min(float(item["test_metrics"]["brier"]) for item in baselines)
    ranked = sorted(ablations, key=lambda item: float(item["test_metrics"]["brier"]))
    verdicts = []
    for item in ranked:
        metrics = item["test_metrics"]
        corr = metrics.get("prediction_oracle_correlation")
        delta_brier = finite_float(float(metrics["brier"]) - best_local_brier)
        delta_corr = None if corr is None else finite_float(float(corr) - historical)
        status = "demote"
        if not item["ship_gate_eligible"]:
            status = "comparison_only"
        elif delta_brier <= 0.0 and delta_corr is not None and delta_corr > 0.0:
            status = "candidate_lift"
        elif abs(delta_brier) <= 0.01:
            status = "neutral"
        verdicts.append({"name": item["name"], "status": status, "delta_brier_vs_best_local": delta_brier, "delta_correlation_vs_historical": delta_corr})
    return {"best_local_baseline_brier": finite_float(best_local_brier), "ranked_ablation_names": [item["name"] for item in ranked], "feature_verdicts": verdicts}


def built_in_edge_cases() -> list[dict[str, Any]]:
    cases = []
    cases.append(edge_case_missing_root())
    cases.append(edge_case_empty_metrics())
    cases.append(edge_case_flat_promotion_refused())
    cases.append(edge_case_nonfinite_feature())
    return cases


def edge_case_missing_root() -> dict[str, Any]:
    try:
        load_artifacts(Path("/tmp/contextgraph-definitely-missing-constellation-ablation-root"))
        return {"name": "missing_artifact_root_fails_closed", "passes": False}
    except ConstellationAblationSuiteError as exc:
        return {"name": "missing_artifact_root_fails_closed", "passes": "MEJEPA_CONSTELLATION_ABLATION_ARTIFACT_ROOT_MISSING" in str(exc), "error": str(exc)}


def edge_case_empty_metrics() -> dict[str, Any]:
    try:
        metric_block([], [], 8)
        return {"name": "empty_metric_set_fails_closed", "passes": False}
    except Exception as exc:
        return {"name": "empty_metric_set_fails_closed", "passes": True, "error": str(exc)}


def edge_case_flat_promotion_refused() -> dict[str, Any]:
    spec = ablation_specs()[0]
    return {"name": "flat_vector_baseline_not_ship_gate_eligible", "passes": spec.flat_vector_baseline and not spec.ship_gate_eligible}


def edge_case_nonfinite_feature() -> dict[str, Any]:
    try:
        feature_value({"objective_features": {"slot_max_abs_z": float("nan")}}, "slot_max_abs_z")
        return {"name": "nonfinite_feature_fails_closed", "passes": False}
    except ConstellationAblationSuiteError as exc:
        return {"name": "nonfinite_feature_fails_closed", "passes": "NONFINITE" in str(exc), "error": str(exc)}


def command_python_constellation_ablation_suite(args: argparse.Namespace) -> int:
    started = utc_now()
    output_root = args.output_root or (args.fsv_root / "artifacts")
    before = {"artifact_root": tree_state(args.artifact_root), "output_root": tree_state(output_root), "fsv_root": tree_state(args.fsv_root)}
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    rows, slot_vectors, certificate = load_artifacts(args.artifact_root)
    attach_flat_baseline_features(rows, slot_vectors)
    dataset = validate_rows(rows, args.min_train_rows, args.min_test_rows)
    train, test = split_rows(rows)
    baselines = [
        evaluate_rate_baseline("global_label_rate", train, test, lambda _row: "global", args.ece_bins),
        evaluate_rate_baseline("per_cell_label_rate", train, test, lambda row: str(row.get("cell_key")), args.ece_bins),
        evaluate_rate_baseline("per_temporal_window_label_rate", train, test, temporal_key, args.ece_bins),
    ]
    ablations = [evaluate_spec(spec, train, test, args.ece_bins) for spec in ablation_specs()]
    summary = rank_and_verdicts(ablations, baselines, float(args.historical_baseline_correlation))
    edge_cases = built_in_edge_cases()
    require(all(case.get("passes") is True for case in edge_cases), "MEJEPA_CONSTELLATION_ABLATION_EDGE_CASE_FAILED")
    require(not any(item["flat_vector_concat_used"] and item["ship_gate_eligible"] for item in ablations), "MEJEPA_CONSTELLATION_ABLATION_FLAT_BASELINE_ELIGIBLE")

    report = build_report(args, started, dataset, certificate, baselines, ablations, summary, edge_cases)
    metrics_path = output_root / "constellation_ablation_metrics.jsonl"
    report_path = output_root / "constellation_ablation_suite_report.json"
    write_jsonl(metrics_path, baselines + ablations)
    dump_json(report_path, report)
    fsv = build_fsv(args, before, output_root, report_path, metrics_path, report, len(baselines), len(ablations))
    fsv_path = args.fsv_root / f"{ARTIFACT_KIND}_fsv.json"
    dump_json(fsv_path, fsv)
    final = read_json(fsv_path)
    require(final.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_FSV_FAILED", str(fsv_path))
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_report(args: argparse.Namespace, started: str, dataset: dict[str, Any], certificate: dict[str, Any], baselines: list[dict[str, Any]], ablations: list[dict[str, Any]], summary: dict[str, Any], edge_cases: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": True,
        "source": {"artifact_root": str(args.artifact_root), "upstream_certificate_sha256": sha256_file(args.artifact_root / "constellation_training_certificate.json")},
        "policy": {"human_intent_inference_used": False, "flat_vector_baseline_ship_gate_eligible": False, "slot_identity_preserved": True},
        "dataset_summary": dataset,
        "upstream_objective_loss": certificate.get("objective_loss"),
        "baselines": baselines,
        "ablations": ablations,
        "summary": summary,
        "measurement_gaps": measurement_gaps_from_report_rows(ablations, args.artifact_root),
        "edge_cases": edge_cases,
    }


def measurement_gaps_from_report_rows(_ablations: list[dict[str, Any]], artifact_root: Path) -> dict[str, Any]:
    rows = [row for _line_no, row in iter_jsonl(artifact_root / "constellation_training_rows.jsonl")]
    return measurement_gaps(rows)


def build_fsv(args: argparse.Namespace, before: dict[str, Any], output_root: Path, report_path: Path, metrics_path: Path, report: dict[str, Any], baseline_count: int, ablation_count: int) -> dict[str, Any]:
    readback_report = read_json(report_path)
    metrics_count = sum(1 for _ in iter_jsonl(metrics_path))
    hashes = {"report": sha256_file(report_path), "metrics": sha256_file(metrics_path)}
    passes = (
        readback_report.get("passes") is True
        and metrics_count == baseline_count + ablation_count
        and ablation_count >= 6
        and readback_report.get("policy", {}).get("flat_vector_baseline_ship_gate_eligible") is False
        and all(item.get("passes") is True for item in readback_report.get("edge_cases", []))
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {"input_artifact_root": str(args.artifact_root), "output_root": str(output_root), "report": str(report_path), "metrics": str(metrics_path)},
        "before_state": before,
        "after_state": {"output_root": tree_state(output_root), "fsv_root": tree_state(args.fsv_root)},
        "readback": {"metrics_rows": metrics_count, "expected_metrics_rows": baseline_count + ablation_count, "report_passes": readback_report.get("passes"), "stable_hashes": hashes, "report_summary_hash": sha256_text(readback_report.get("summary"))},
        "summary": readback_report.get("summary"),
        "measurement_gaps": readback_report.get("measurement_gaps"),
        "edge_cases": readback_report.get("edge_cases"),
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--artifact-root", type=Path, default=DEFAULT_ARTIFACT_ROOT)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--historical-baseline-correlation", type=float, default=HISTORICAL_BASELINE_CORRELATION)
    parser.add_argument("--ece-bins", type=int, default=8)
    parser.add_argument("--min-train-rows", type=int, default=12)
    parser.add_argument("--min-test-rows", type=int, default=3)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_constellation_ablation_suite(args)
    except ConstellationAblationSuiteError as exc:
        print(f"PYTHON_CONSTELLATION_ABLATION_SUITE_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
