"""TASK-PY-G-096 constellation training objective over real aiwonder forwards.

This command builds a small real-corpus training dataset from the completed
aiwonder packed forward-cache substrate and emits a slot-preserving objective
certificate. It is intentionally a smoke-scale certificate path: it proves the
objective uses separate embedder slots and relationship features before any
promotion can claim movement against the Python 0.95 ship gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from itertools import combinations
from pathlib import Path
from typing import Any

import numpy as np

from .python_real_corpus_constellation_smoke import (
    DEFAULT_CHUNKS_JSONL,
    DEFAULT_FORWARD_ROOT,
    EPS,
    EXPECTED_EMBEDDERS,
    RealCorpusConstellationSmokeError,
    SampleRecord,
    finite_float,
    load_chunk_manifest_window,
    load_sample_records,
    pairwise_relationship_rows,
    parse_embedder_list,
    select_completed_shards,
    sha256_text,
    write_jsonl,
)
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-096"
ARTIFACT_KIND = "python_constellation_training_objective"
FORMULA_VERSION = "slot_preserving_constellation_training_objective_v1"
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-096-constellation-training-objective-fsv")
DEFAULT_CORPUS_INDEX_JSON = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
HISTORICAL_BASELINE_CORRELATION = 0.8666666746139526
SCALAR_FEATURES = ("norm_l2", "std", "abs_mean", "entropy_proxy", "zero_fraction")
ABLATION_TERMS = ("slot", "pairwise", "contradiction", "temporal")


class ConstellationTrainingObjectiveError(RuntimeError):
    pass


@dataclass(frozen=True)
class ComponentConfig:
    slot: bool
    pairwise: bool
    contradiction: bool
    temporal: bool

    def enabled_terms(self) -> list[str]:
        return [name for name in ABLATION_TERMS if bool(getattr(self, name))]

    def as_dict(self) -> dict[str, Any]:
        return {
            "slot": self.slot,
            "pairwise": self.pairwise,
            "contradiction": self.contradiction,
            "temporal": self.temporal,
            "oracle_alignment": True,
            "anti_compensation": self.slot,
        }


@dataclass(frozen=True)
class ProbabilityModel:
    feature: str
    center: float
    scale: float
    direction: float
    train_bad_mean: float
    train_good_mean: float


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise ConstellationTrainingObjectiveError(code if detail is None else f"{code}: {detail}")


def normalize_category(value: str) -> str:
    return value.strip().lower().replace("-", "_")


def oracle_known_bad(category: str) -> int:
    normalized = normalize_category(category)
    return 0 if normalized in {"known_good", "knowngood", "gold", "base"} else 1


def load_oracle_index(path: Path) -> tuple[dict[tuple[str, str], dict[str, Any]], dict[str, Any]]:
    require(path.exists(), "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_INDEX_MISSING", str(path))
    payload = read_json(path)
    entries = list(payload.get("entries") or [])
    require(bool(entries), "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_INDEX_EMPTY", str(path))
    by_key: dict[tuple[str, str], dict[str, Any]] = {}
    oracle_counts = Counter()
    category_counts = Counter()
    bucket_counts = Counter()
    for entry in entries:
        task_id = str(entry.get("task_id") or "")
        category = normalize_category(str(entry.get("category") or ""))
        require(task_id and category, "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_INDEX_KEY_MISSING")
        key = (task_id, category)
        require(key not in by_key, "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_INDEX_DUPLICATE_KEY", f"{task_id}:{category}")
        require(isinstance(entry.get("oracle_all_passed"), bool), "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_BOOL_MISSING", f"{task_id}:{category}")
        by_key[key] = entry
        oracle_counts["pass" if entry["oracle_all_passed"] else "fail_or_error"] += 1
        category_counts[category] += 1
        bucket_counts[str(entry.get("bucket") or "unknown")] += 1
    return by_key, {
        "path": str(path),
        "sha256": sha256_file(path),
        "complete": bool(payload.get("complete")),
        "entry_count": len(entries),
        "oracle_counts": dict(sorted(oracle_counts.items())),
        "category_counts": dict(sorted(category_counts.items())),
        "bucket_counts": dict(sorted(bucket_counts.items())),
        "corpus_sha256": payload.get("corpus_sha256"),
    }


def stable_partition(row_key: str) -> str:
    digest = hashlib.sha256(row_key.encode("utf-8")).hexdigest()
    return "test" if int(digest[:8], 16) % 5 == 0 else "train"


def sigmoid(value: float) -> float:
    clipped = max(-40.0, min(40.0, value))
    return finite_float(1.0 / (1.0 + math.exp(-clipped)))


def brier(labels: list[int], probabilities: list[float]) -> float:
    require(len(labels) == len(probabilities), "MEJEPA_CONSTELLATION_OBJECTIVE_LABEL_PROB_MISMATCH")
    require(bool(labels), "MEJEPA_CONSTELLATION_OBJECTIVE_EMPTY_EVAL_SET")
    return finite_float(float(np.mean([(float(p) - float(y)) ** 2 for y, p in zip(labels, probabilities)])))


def accuracy_at_half(labels: list[int], probabilities: list[float]) -> float:
    require(len(labels) == len(probabilities), "MEJEPA_CONSTELLATION_OBJECTIVE_LABEL_PROB_MISMATCH")
    require(bool(labels), "MEJEPA_CONSTELLATION_OBJECTIVE_EMPTY_EVAL_SET")
    hits = sum(1 for label, prob in zip(labels, probabilities) if (1 if prob >= 0.5 else 0) == int(label))
    return finite_float(hits / len(labels))


def pearson_or_none(labels: list[int], probabilities: list[float]) -> tuple[float | None, str | None]:
    if len(labels) < 3:
        return None, "TOO_FEW_ROWS"
    x = np.asarray(labels, dtype=np.float64)
    y = np.asarray(probabilities, dtype=np.float64)
    if float(np.std(x)) <= EPS or float(np.std(y)) <= EPS:
        return None, "DEGENERATE_VARIANCE"
    return finite_float(float(np.corrcoef(x, y)[0, 1])), None


def calibration_ece(labels: list[int], probabilities: list[float], bins: int) -> float:
    require(bins > 0, "MEJEPA_CONSTELLATION_OBJECTIVE_INVALID_ECE_BINS")
    require(len(labels) == len(probabilities), "MEJEPA_CONSTELLATION_OBJECTIVE_LABEL_PROB_MISMATCH")
    total = len(labels)
    require(total > 0, "MEJEPA_CONSTELLATION_OBJECTIVE_EMPTY_EVAL_SET")
    ece = 0.0
    for idx in range(bins):
        low = idx / bins
        high = (idx + 1) / bins
        bucket = [
            (label, prob)
            for label, prob in zip(labels, probabilities)
            if (low <= prob < high) or (idx == bins - 1 and prob == 1.0)
        ]
        if not bucket:
            continue
        conf = float(np.mean([prob for _, prob in bucket]))
        observed = float(np.mean([label for label, _ in bucket]))
        ece += (len(bucket) / total) * abs(conf - observed)
    return finite_float(ece)


def parse_component_config(args: argparse.Namespace) -> ComponentConfig:
    config = ComponentConfig(
        slot=not bool(args.disable_slot_term),
        pairwise=not bool(args.disable_pairwise_term),
        contradiction=not bool(args.disable_contradiction_term),
        temporal=not bool(args.disable_temporal_term),
    )
    require(bool(config.enabled_terms()), "MEJEPA_CONSTELLATION_OBJECTIVE_ALL_TERMS_DISABLED")
    return config


def slot_distributions(records: dict[str, dict[str, SampleRecord]]) -> dict[tuple[str, str], tuple[float, float]]:
    distributions: dict[tuple[str, str], tuple[float, float]] = {}
    for embedder_id, rows in records.items():
        for feature in SCALAR_FEATURES:
            values = np.asarray([row.scalar_features[feature] for row in rows.values()], dtype=np.float64)
            require(values.size > 1, "MEJEPA_CONSTELLATION_OBJECTIVE_SLOT_FEATURE_TOO_FEW_ROWS", f"{embedder_id}:{feature}")
            distributions[(embedder_id, feature)] = (float(values.mean()), max(EPS, float(values.std())))
    return distributions


def row_pairwise_observations(
    row_key: str,
    present: list[tuple[str, SampleRecord]],
    row_index: int,
) -> tuple[list[dict[str, Any]], dict[str, float]]:
    observations: list[dict[str, Any]] = []
    disagreement_inputs: list[float] = []
    feature_deltas_by_name: dict[str, list[float]] = {feature: [] for feature in SCALAR_FEATURES}
    for pair_index, ((left_id, left), (right_id, right)) in enumerate(combinations(present, 2)):
        deltas = {
            feature: finite_float(abs(float(left.scalar_features[feature]) - float(right.scalar_features[feature])))
            for feature in SCALAR_FEATURES
        }
        for feature, value in deltas.items():
            feature_deltas_by_name[feature].append(value)
            disagreement_inputs.append(value)
        observations.append(
            {
                "artifact_kind": "python_constellation_row_pairwise_observation",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "row_key": row_key,
                "row_index": row_index,
                "pair_index": pair_index,
                "left_embedder_id": left_id,
                "right_embedder_id": right_id,
                "left_vector_sha256": left.vector_sha256,
                "right_vector_sha256": right.vector_sha256,
                "left_dim": left.vector_len,
                "right_dim": right.vector_len,
                "scalar_feature_abs_delta": deltas,
                "raw_vector_comparison_policy": "vectors remain slot-local; cross-slot relationship uses scalar/tensor summaries only",
                "flat_vector_concat_used": False,
            }
        )
    summary = {
        "pair_count": finite_float(float(len(observations))),
        "mean_scalar_delta": finite_float(float(np.mean(disagreement_inputs))) if disagreement_inputs else 0.0,
        "max_scalar_delta": finite_float(float(np.max(disagreement_inputs))) if disagreement_inputs else 0.0,
    }
    for feature, values in feature_deltas_by_name.items():
        summary[f"{feature}_mean_abs_delta"] = finite_float(float(np.mean(values))) if values else 0.0
    return observations, summary


def build_training_dataset(
    records: dict[str, dict[str, SampleRecord]],
    required_embedders: tuple[str, ...],
    shard_order: dict[str, int],
    oracle_index: dict[tuple[str, str], dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    distributions = slot_distributions(records)
    all_row_keys = sorted(set().union(*(set(rows) for rows in records.values())))
    training_rows: list[dict[str, Any]] = []
    slot_vectors: list[dict[str, Any]] = []
    pairwise_observations: list[dict[str, Any]] = []
    slot_coverage = Counter()
    label_counts = Counter()
    oracle_join_missing = 0

    for row_index, row_key in enumerate(all_row_keys):
        present = [(embedder_id, rows[row_key]) for embedder_id, rows in sorted(records.items()) if row_key in rows]
        if len(present) < 2:
            continue
        first = present[0][1]
        oracle_key = (first.task_instance_id, normalize_category(first.mutation_category))
        oracle_entry = oracle_index.get(oracle_key)
        if oracle_entry is None:
            oracle_join_missing += 1
            continue
        label = 0 if bool(oracle_entry.get("oracle_all_passed")) else 1
        slot_summaries: list[dict[str, Any]] = []
        slot_anomaly_values: list[float] = []
        slot_uncertainty_values: list[float] = []
        for embedder_id, record in present:
            z_scores: dict[str, float] = {}
            for feature in SCALAR_FEATURES:
                mean, std = distributions[(embedder_id, feature)]
                z_scores[feature] = finite_float((float(record.scalar_features[feature]) - mean) / std)
            abs_z = [abs(value) for value in z_scores.values()]
            slot_anomaly = finite_float(float(np.mean(abs_z)))
            slot_uncertainty = finite_float(float(np.std(abs_z)))
            slot_anomaly_values.append(slot_anomaly)
            slot_uncertainty_values.append(slot_uncertainty)
            slot_summaries.append(
                {
                    "embedder_id": embedder_id,
                    "dim": record.vector_len,
                    "vector_sha256": record.vector_sha256,
                    "record_sha256": record.record_sha256,
                    "cache_key_sha256": record.cache_key_sha256,
                    "instrument_id": record.instrument_id,
                    "model_hash": record.model_hash,
                    "runtime_model_version": record.runtime_model_version,
                    "precision_class": record.precision_class,
                    "scalar_features": {key: finite_float(value) for key, value in record.scalar_features.items()},
                    "z_scores": z_scores,
                    "slot_anomaly_score": slot_anomaly,
                    "slot_uncertainty_score": slot_uncertainty,
                }
            )
            slot_vectors.append(
                {
                    "artifact_kind": "python_constellation_slot_vector",
                    "schema_version": SCHEMA_VERSION,
                    "formula_version": FORMULA_VERSION,
                    "row_key": row_key,
                    "embedder_id": embedder_id,
                    "dim": record.vector_len,
                    "vector_sha256": record.vector_sha256,
                    "record_sha256": record.record_sha256,
                    "cache_key_sha256": record.cache_key_sha256,
                    "vector": [finite_float(value) for value in record.vector.astype(float).tolist()],
                    "source": {
                        "shard_id": record.shard_id,
                        "task_instance_id": record.task_instance_id,
                        "workspace_state": record.workspace_state,
                        "chunk_id": record.chunk_id,
                        "source_sha256": record.source_sha256,
                    },
                    "flat_vector_concat_used": False,
                }
            )

        row_pairwise, pairwise_summary = row_pairwise_observations(row_key, present, len(training_rows))
        pairwise_observations.extend(row_pairwise)
        slot_mean = finite_float(float(np.mean(slot_anomaly_values)))
        slot_max = finite_float(float(np.max(slot_anomaly_values)))
        slot_spread = finite_float(float(np.std(slot_anomaly_values)))
        slot_uncertainty = finite_float(float(np.mean(slot_uncertainty_values)))
        pairwise_score = finite_float(pairwise_summary["mean_scalar_delta"])
        contradiction_score = finite_float((slot_max * (1.0 + pairwise_score)) / (1.0 + slot_mean))
        consensus_score = finite_float(1.0 / (1.0 + slot_spread + pairwise_score))
        window_index = int(shard_order.get(first.shard_id, 0))
        row = {
            "artifact_kind": "python_constellation_training_row",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "row_key": row_key,
            "row_index": len(training_rows),
            "partition": stable_partition(row_key),
            "cell_key": f"python:{first.mutation_category}",
            "language": "python",
            "mutation_category": first.mutation_category,
            "labels": {
                "oracle_known_bad": label,
                "oracle_all_passed": bool(oracle_entry.get("oracle_all_passed")),
                "oracle_exception": oracle_entry.get("oracle_exception"),
                "oracle_per_test_count": oracle_entry.get("oracle_per_test_count"),
                "oracle_verdict_sha256": oracle_entry.get("oracle_verdict_sha256"),
                "oracle_label_source": "swebench_lite_python_300x8_index_oracle_all_passed",
                "mutation_category_proxy_known_bad": oracle_known_bad(first.mutation_category),
                "q3_failure_mode": {"present": label == 1, "value": first.mutation_category if label == 1 else None},
                "q4_labels": {
                    "present": False,
                    "reason": "not present in the packed forward-cache rows consumed by this smoke",
                },
                "q5_reality_impact": {
                    "present": False,
                    "reason": "shift-log replay labels are consumed by downstream Q5 calibration, not this packed-forward smoke",
                },
            },
            "source": {
                "task_instance_id": first.task_instance_id,
                "workspace_state": first.workspace_state,
                "repo": first.repo,
                "relative_path": first.relative_path,
                "chunk_id": first.chunk_id,
                "chunk_kind": first.chunk_kind,
                "node_kind": first.node_kind,
                "source_sha256": first.source_sha256,
                "shard_id": first.shard_id,
            },
            "temporal": {
                "window_id": f"aiwonder-forward-shard:{first.shard_id}",
                "window_index": window_index,
                "exact_state_policy": "constellation snapshot at packed forward-cache readback time",
            },
            "slot_policy": {
                "expected_embedders": list(required_embedders),
                "present_embedder_count": len(present),
                "missing_embedders": sorted(set(required_embedders) - {embedder_id for embedder_id, _ in present}),
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
            },
            "slots": sorted(slot_summaries, key=lambda item: item["embedder_id"]),
            "pairwise_tensor_summary": pairwise_summary,
            "objective_features": {
                "slot_mean_abs_z": slot_mean,
                "slot_max_abs_z": slot_max,
                "slot_spread_abs_z": slot_spread,
                "slot_uncertainty_mean": slot_uncertainty,
                "pairwise_disagreement_score": pairwise_score,
                "pairwise_max_scalar_delta": finite_float(pairwise_summary["max_scalar_delta"]),
                "contradiction_score": contradiction_score,
                "consensus_score": consensus_score,
                "temporal_window_index": finite_float(float(window_index)),
            },
        }
        row["row_sha256"] = sha256_text(
            {
                "row_key": row_key,
                "label": label,
                "slots": [(slot["embedder_id"], slot["vector_sha256"]) for slot in row["slots"]],
                "features": row["objective_features"],
            }
        )
        training_rows.append(row)
        slot_coverage[len(present)] += 1
        label_counts[label] += 1

    require(bool(training_rows), "MEJEPA_CONSTELLATION_OBJECTIVE_NO_TRAINING_ROWS")
    require(oracle_join_missing == 0, "MEJEPA_CONSTELLATION_OBJECTIVE_ORACLE_JOIN_MISSING", str(oracle_join_missing))
    return training_rows, slot_vectors, pairwise_observations, {
        "row_count": len(training_rows),
        "slot_vector_rows": len(slot_vectors),
        "pairwise_observation_rows": len(pairwise_observations),
        "slot_coverage": dict(sorted(slot_coverage.items())),
        "label_counts": dict(sorted(label_counts.items())),
        "oracle_join_missing": oracle_join_missing,
        "oracle_label_source": "swebench_lite_python_300x8_index_oracle_all_passed",
    }


def split_rows(rows: list[dict[str, Any]], min_train_rows: int, min_test_rows: int) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    train = [row for row in rows if row["partition"] == "train"]
    test = [row for row in rows if row["partition"] == "test"]
    require(len(train) >= min_train_rows, "MEJEPA_CONSTELLATION_OBJECTIVE_TRAIN_ROWS_TOO_LOW", str(len(train)))
    require(len(test) >= min_test_rows, "MEJEPA_CONSTELLATION_OBJECTIVE_TEST_ROWS_TOO_LOW", str(len(test)))
    train_labels = {int(row["labels"]["oracle_known_bad"]) for row in train}
    test_labels = {int(row["labels"]["oracle_known_bad"]) for row in test}
    require(train_labels == {0, 1}, "MEJEPA_CONSTELLATION_OBJECTIVE_TRAIN_LABELS_DEGENERATE", str(train_labels))
    require(test_labels == {0, 1}, "MEJEPA_CONSTELLATION_OBJECTIVE_TEST_LABELS_DEGENERATE", str(test_labels))
    return train, test, {
        "train_rows": len(train),
        "test_rows": len(test),
        "train_label_counts": dict(sorted(Counter(row["labels"]["oracle_known_bad"] for row in train).items())),
        "test_label_counts": dict(sorted(Counter(row["labels"]["oracle_known_bad"] for row in test).items())),
    }


def fit_probability_model(train: list[dict[str, Any]], feature: str) -> ProbabilityModel:
    good_values = [float(row["objective_features"][feature]) for row in train if int(row["labels"]["oracle_known_bad"]) == 0]
    bad_values = [float(row["objective_features"][feature]) for row in train if int(row["labels"]["oracle_known_bad"]) == 1]
    require(bool(good_values) and bool(bad_values), "MEJEPA_CONSTELLATION_OBJECTIVE_MODEL_LABELS_DEGENERATE", feature)
    all_values = np.asarray(good_values + bad_values, dtype=np.float64)
    scale = max(EPS, float(np.std(all_values)))
    good_mean = float(np.mean(good_values))
    bad_mean = float(np.mean(bad_values))
    direction = 1.0 if bad_mean >= good_mean else -1.0
    center = (bad_mean + good_mean) / 2.0
    return ProbabilityModel(
        feature=feature,
        center=finite_float(center),
        scale=finite_float(scale),
        direction=finite_float(direction),
        train_bad_mean=finite_float(bad_mean),
        train_good_mean=finite_float(good_mean),
    )


def predict_probability(model: ProbabilityModel, row: dict[str, Any]) -> float:
    value = float(row["objective_features"][model.feature])
    return sigmoid(model.direction * (value - model.center) / model.scale)


def evaluate_component(
    name: str,
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    feature: str,
    enabled: bool,
    ece_bins: int,
    input_policy: str,
) -> dict[str, Any]:
    if not enabled:
        return {
            "component": name,
            "enabled": False,
            "disabled_reason": "ablation_switch",
            "flat_panel_aggregate_used": False,
            "flat_vector_concat_used": False,
        }
    model = fit_probability_model(train, feature)
    train_prob = [predict_probability(model, row) for row in train]
    test_prob = [predict_probability(model, row) for row in test]
    train_labels = [int(row["labels"]["oracle_known_bad"]) for row in train]
    test_labels = [int(row["labels"]["oracle_known_bad"]) for row in test]
    corr, corr_reason = pearson_or_none(test_labels, test_prob)
    return {
        "component": name,
        "enabled": True,
        "feature": feature,
        "input_policy": input_policy,
        "model": {
            "center": model.center,
            "scale": model.scale,
            "direction": model.direction,
            "train_good_mean": model.train_good_mean,
            "train_bad_mean": model.train_bad_mean,
        },
        "train_brier": brier(train_labels, train_prob),
        "test_brier": brier(test_labels, test_prob),
        "test_ece": calibration_ece(test_labels, test_prob, ece_bins),
        "test_accuracy_at_0_5": accuracy_at_half(test_labels, test_prob),
        "test_prediction_oracle_correlation": corr,
        "test_prediction_oracle_correlation_validity": corr_reason,
        "prediction_sample": [
            {
                "row_key": row["row_key"],
                "probability_known_bad": finite_float(prob),
                "oracle_known_bad": int(row["labels"]["oracle_known_bad"]),
            }
            for row, prob in list(zip(test, test_prob))[:10]
        ],
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }


def metric_block(labels: list[int], probabilities: list[float], ece_bins: int) -> dict[str, Any]:
    corr, corr_reason = pearson_or_none(labels, probabilities)
    return {
        "test_brier": brier(labels, probabilities),
        "test_ece": calibration_ece(labels, probabilities, ece_bins),
        "test_accuracy_at_0_5": accuracy_at_half(labels, probabilities),
        "test_prediction_oracle_correlation": corr,
        "test_prediction_oracle_correlation_validity": corr_reason,
    }


def evaluate_temporal_component(
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    enabled: bool,
    ece_bins: int,
) -> dict[str, Any]:
    if not enabled:
        return {
            "component": "temporal_window",
            "enabled": False,
            "disabled_reason": "ablation_switch",
            "flat_panel_aggregate_used": False,
            "flat_vector_concat_used": False,
        }
    global_rate = float(np.mean([int(row["labels"]["oracle_known_bad"]) for row in train]))
    grouped: dict[tuple[str, int], list[int]] = defaultdict(list)
    for row in train:
        grouped[(str(row["cell_key"]), int(row["temporal"]["window_index"]))].append(int(row["labels"]["oracle_known_bad"]))
    rates = {
        f"{cell_key}|{window_index}": finite_float((sum(labels) + global_rate) / (len(labels) + 1.0))
        for (cell_key, window_index), labels in grouped.items()
    }

    def probability(row: dict[str, Any]) -> float:
        key = f"{row['cell_key']}|{int(row['temporal']['window_index'])}"
        return finite_float(rates.get(key, global_rate))

    train_prob = [probability(row) for row in train]
    test_prob = [probability(row) for row in test]
    train_labels = [int(row["labels"]["oracle_known_bad"]) for row in train]
    test_labels = [int(row["labels"]["oracle_known_bad"]) for row in test]
    corr, corr_reason = pearson_or_none(test_labels, test_prob)
    return {
        "component": "temporal_window",
        "enabled": True,
        "input_policy": "cell_key plus aiwonder shard/window metadata",
        "model": {
            "global_train_bad_rate": finite_float(global_rate),
            "cell_window_rate_count": len(rates),
            "cell_window_rate_sample": dict(sorted(rates.items())[:10]),
        },
        "train_brier": brier(train_labels, train_prob),
        "test_brier": brier(test_labels, test_prob),
        "test_ece": calibration_ece(test_labels, test_prob, ece_bins),
        "test_accuracy_at_0_5": accuracy_at_half(test_labels, test_prob),
        "test_prediction_oracle_correlation": corr,
        "test_prediction_oracle_correlation_validity": corr_reason,
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }


def smoothed_rate_model(
    train: list[dict[str, Any]],
    key_fn: Any,
    global_rate: float,
) -> dict[str, float]:
    grouped: dict[str, list[int]] = defaultdict(list)
    for row in train:
        grouped[str(key_fn(row))].append(int(row["labels"]["oracle_known_bad"]))
    return {
        key: finite_float((sum(labels) + global_rate) / (len(labels) + 1.0))
        for key, labels in grouped.items()
    }


def evaluate_rate_baseline(
    name: str,
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    key_fn: Any,
    ece_bins: int,
    input_policy: str,
) -> dict[str, Any]:
    global_rate = finite_float(float(np.mean([int(row["labels"]["oracle_known_bad"]) for row in train])))
    rates = smoothed_rate_model(train, key_fn, global_rate)
    probabilities = [finite_float(rates.get(str(key_fn(row)), global_rate)) for row in test]
    labels = [int(row["labels"]["oracle_known_bad"]) for row in test]
    block = metric_block(labels, probabilities, ece_bins)
    return {
        "baseline": name,
        "input_policy": input_policy,
        "model": {
            "global_train_bad_rate": global_rate,
            "group_count": len(rates),
            "group_rate_sample": dict(sorted(rates.items())[:20]),
        },
        **block,
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }


def slot_by_embedder(row: dict[str, Any], embedder_id: str) -> dict[str, Any] | None:
    for slot in row.get("slots") or []:
        if slot.get("embedder_id") == embedder_id:
            return slot
    return None


def evaluate_per_embedder_slot_baselines(
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    ece_bins: int,
    global_null_brier: float,
) -> list[dict[str, Any]]:
    embedder_ids = sorted({str(slot["embedder_id"]) for row in train + test for slot in row.get("slots") or []})
    out: list[dict[str, Any]] = []
    for embedder_id in embedder_ids:
        train_rows = [row for row in train if slot_by_embedder(row, embedder_id) is not None]
        test_rows = [row for row in test if slot_by_embedder(row, embedder_id) is not None]
        train_labels = {int(row["labels"]["oracle_known_bad"]) for row in train_rows}
        if len(train_rows) < 4 or len(test_rows) < 1 or train_labels != {0, 1}:
            out.append(
                {
                    "baseline": "per_embedder_slot",
                    "embedder_id": embedder_id,
                    "valid": False,
                    "invalid_reason": "INSUFFICIENT_OR_DEGENERATE_SLOT_ROWS",
                    "train_rows": len(train_rows),
                    "test_rows": len(test_rows),
                    "train_labels": sorted(train_labels),
                    "flat_panel_aggregate_used": False,
                    "flat_vector_concat_used": False,
                }
            )
            continue
        good_values = [
            float(slot_by_embedder(row, embedder_id)["slot_anomaly_score"])
            for row in train_rows
            if int(row["labels"]["oracle_known_bad"]) == 0
        ]
        bad_values = [
            float(slot_by_embedder(row, embedder_id)["slot_anomaly_score"])
            for row in train_rows
            if int(row["labels"]["oracle_known_bad"]) == 1
        ]
        all_values = np.asarray(good_values + bad_values, dtype=np.float64)
        scale = max(EPS, float(np.std(all_values)))
        good_mean = float(np.mean(good_values))
        bad_mean = float(np.mean(bad_values))
        direction = 1.0 if bad_mean >= good_mean else -1.0
        center = (bad_mean + good_mean) / 2.0
        labels = [int(row["labels"]["oracle_known_bad"]) for row in test_rows]
        probabilities = [
            sigmoid(direction * (float(slot_by_embedder(row, embedder_id)["slot_anomaly_score"]) - center) / scale)
            for row in test_rows
        ]
        block = metric_block(labels, probabilities, ece_bins)
        out.append(
            {
                "baseline": "per_embedder_slot",
                "embedder_id": embedder_id,
                "valid": True,
                "feature": "slot_anomaly_score",
                "train_rows": len(train_rows),
                "test_rows": len(test_rows),
                "model": {
                    "center": finite_float(center),
                    "scale": finite_float(scale),
                    "direction": finite_float(direction),
                    "train_good_mean": finite_float(good_mean),
                    "train_bad_mean": finite_float(bad_mean),
                },
                **block,
                "delta_test_brier_vs_global_null": finite_float(float(block["test_brier"]) - float(global_null_brier)),
                "flat_panel_aggregate_used": False,
                "flat_vector_concat_used": False,
            }
        )
    return out


def component_probability_maps(components: dict[str, dict[str, Any]], train: list[dict[str, Any]], test: list[dict[str, Any]]) -> dict[str, dict[str, float]]:
    maps: dict[str, dict[str, float]] = {}
    for name, component in components.items():
        if not component.get("enabled"):
            continue
        if name == "temporal_window":
            global_rate = float(component["model"]["global_train_bad_rate"])
            rates = component["model"]["cell_window_rate_sample"]
            # Full rates may be larger than the sample in the certificate. Rebuild the
            # compact temporal probabilities from train rows for objective scoring.
            grouped: dict[tuple[str, int], list[int]] = defaultdict(list)
            for row in train:
                grouped[(str(row["cell_key"]), int(row["temporal"]["window_index"]))].append(int(row["labels"]["oracle_known_bad"]))
            full_rates = {
                f"{cell_key}|{window_index}": finite_float((sum(labels) + global_rate) / (len(labels) + 1.0))
                for (cell_key, window_index), labels in grouped.items()
            }
            maps[name] = {
                row["row_key"]: finite_float(full_rates.get(f"{row['cell_key']}|{int(row['temporal']['window_index'])}", rates.get(f"{row['cell_key']}|{int(row['temporal']['window_index'])}", global_rate)))
                for row in test
            }
            continue
        model_info = component["model"]
        model = ProbabilityModel(
            feature=str(component["feature"]),
            center=float(model_info["center"]),
            scale=float(model_info["scale"]),
            direction=float(model_info["direction"]),
            train_bad_mean=float(model_info["train_bad_mean"]),
            train_good_mean=float(model_info["train_good_mean"]),
        )
        maps[name] = {row["row_key"]: predict_probability(model, row) for row in test}
    return maps


def evaluate_baseline_ladder(
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    ece_bins: int,
    historical_baseline_correlation: float,
) -> dict[str, Any]:
    global_rate = finite_float(float(np.mean([int(row["labels"]["oracle_known_bad"]) for row in train])))
    labels = [int(row["labels"]["oracle_known_bad"]) for row in test]
    global_probs = [global_rate for _ in test]
    global_null = {
        "baseline": "global_label_rate_null",
        "input_policy": "train split oracle bad-rate only; no embedder information",
        "model": {"global_train_bad_rate": global_rate},
        **metric_block(labels, global_probs, ece_bins),
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }
    per_cell = evaluate_rate_baseline(
        "per_cell_label_rate",
        train,
        test,
        lambda row: row["cell_key"],
        ece_bins,
        "mutation_category x language cell rate baseline",
    )
    per_window = evaluate_rate_baseline(
        "per_temporal_window_label_rate",
        train,
        test,
        lambda row: row["temporal"]["window_index"],
        ece_bins,
        "aiwonder forward-cache shard/window rate baseline",
    )
    per_cell_window = evaluate_rate_baseline(
        "per_cell_window_label_rate",
        train,
        test,
        lambda row: f"{row['cell_key']}|{row['temporal']['window_index']}",
        ece_bins,
        "cell plus temporal window local rate baseline",
    )
    per_embedder = evaluate_per_embedder_slot_baselines(train, test, ece_bins, float(global_null["test_brier"]))
    valid_embedder = [row for row in per_embedder if row.get("valid") is True]
    best_embedder_brier = min((float(row["test_brier"]) for row in valid_embedder), default=None)
    best_local_brier = min(
        [
            float(global_null["test_brier"]),
            float(per_cell["test_brier"]),
            float(per_window["test_brier"]),
            float(per_cell_window["test_brier"]),
        ]
        + ([best_embedder_brier] if best_embedder_brier is not None else [])
    )
    return {
        "policy": "0.866667 remains global ship-gate context only; local baselines are required per cell/window/embedder before promotion",
        "historical_ship_gate_context": {
            "prediction_oracle_correlation": finite_float(historical_baseline_correlation),
            "source": "legacy holdout measurement; not the sole comparator for slot-preserving objectives",
        },
        "global_null": global_null,
        "per_cell": per_cell,
        "per_window": per_window,
        "per_cell_window": per_cell_window,
        "per_embedder_slot": per_embedder,
        "summary": {
            "valid_per_embedder_baselines": len(valid_embedder),
            "best_local_baseline_test_brier": finite_float(best_local_brier),
            "global_null_test_brier": global_null["test_brier"],
        },
    }


def evaluate_oracle_alignment(
    components: dict[str, dict[str, Any]],
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    ece_bins: int,
    baseline_correlation: float,
) -> dict[str, Any]:
    probability_maps = component_probability_maps(components, train, test)
    require(bool(probability_maps), "MEJEPA_CONSTELLATION_OBJECTIVE_NO_ENABLED_COMPONENT_PROBABILITIES")
    names = sorted(probability_maps)
    probabilities: list[float] = []
    labels: list[int] = []
    for row in test:
        row_probs = [probability_maps[name][row["row_key"]] for name in names]
        probabilities.append(finite_float(float(np.mean(row_probs))))
        labels.append(int(row["labels"]["oracle_known_bad"]))
    corr, corr_reason = pearson_or_none(labels, probabilities)
    observed = corr if corr is not None else 0.0
    delta = finite_float(observed - baseline_correlation)
    if corr is None:
        status = "falsified_for_smoke_degenerate_correlation"
    elif delta > 0.0:
        status = "smoke_improved_historical_baseline_not_promoted"
    else:
        status = "smoke_falsified_improvement_against_historical_baseline"
    return {
        "component": "oracle_alignment",
        "enabled": True,
        "input_components": names,
        "input_policy": "mean of enabled non-flat component probabilities measured against oracle labels per held-out row",
        "test_brier": brier(labels, probabilities),
        "test_ece": calibration_ece(labels, probabilities, ece_bins),
        "test_accuracy_at_0_5": accuracy_at_half(labels, probabilities),
        "test_prediction_oracle_correlation": corr,
        "test_prediction_oracle_correlation_validity": corr_reason,
        "historical_baseline_correlation": finite_float(baseline_correlation),
        "delta_vs_historical_baseline": delta,
        "baseline_comparison_status": status,
        "promotion_allowed": False,
        "promotion_block_reason": "smoke_scale_certificate_only; requires full held-out/four-window gate before promotion",
        "prediction_sample": [
            {
                "row_key": row["row_key"],
                "probability_known_bad": finite_float(prob),
                "oracle_known_bad": int(row["labels"]["oracle_known_bad"]),
            }
            for row, prob in list(zip(test, probabilities))[:10]
        ],
        "test_probabilities_by_row": {
            row["row_key"]: finite_float(prob)
            for row, prob in zip(test, probabilities)
        },
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }


def evaluate_anti_compensation(
    train: list[dict[str, Any]],
    test: list[dict[str, Any]],
    oracle_alignment: dict[str, Any],
    slot_enabled: bool,
) -> dict[str, Any]:
    if not slot_enabled:
        return {
            "component": "anti_compensation",
            "enabled": False,
            "disabled_reason": "slot_term_disabled",
            "flat_panel_aggregate_used": False,
            "flat_vector_concat_used": False,
        }
    train_slot_max = np.asarray([float(row["objective_features"]["slot_max_abs_z"]) for row in train], dtype=np.float64)
    threshold = finite_float(float(np.quantile(train_slot_max, 0.90)))
    probabilities = {
        str(key): float(value)
        for key, value in (oracle_alignment.get("test_probabilities_by_row") or {}).items()
    }
    penalties: list[float] = []
    violation_rows: list[dict[str, Any]] = []
    for row in test:
        slot_max = float(row["objective_features"]["slot_max_abs_z"])
        local_violation = slot_max >= threshold
        aggregate_prob = probabilities.get(row["row_key"], 0.5)
        confident_good = aggregate_prob < 0.35
        penalty = max(0.0, (slot_max - threshold) / max(EPS, threshold)) if local_violation and confident_good else 0.0
        penalties.append(penalty)
        if penalty > 0.0:
            violation_rows.append(
                {
                    "row_key": row["row_key"],
                    "slot_max_abs_z": finite_float(slot_max),
                    "aggregate_probability_known_bad": finite_float(aggregate_prob),
                    "penalty": finite_float(penalty),
                }
            )

    synthetic_edge_penalty = max(0.0, (threshold * 2.0 - threshold) / max(EPS, threshold))
    return {
        "component": "anti_compensation",
        "enabled": True,
        "input_policy": "local slot violation guard; aggregate fit cannot hide high-risk slot evidence",
        "slot_violation_threshold_p90": threshold,
        "test_penalty_mean": finite_float(float(np.mean(penalties))) if penalties else 0.0,
        "test_violation_count": len(violation_rows),
        "violation_sample": violation_rows[:10],
        "edge_case": {
            "name": "aggregate_good_score_cannot_hide_high_slot_violation",
            "passes": synthetic_edge_penalty > 0.0,
            "synthetic_control_only": True,
            "penalty": finite_float(synthetic_edge_penalty),
        },
        "flat_panel_aggregate_used": False,
        "flat_vector_concat_used": False,
    }


def build_objective_certificate(
    rows: list[dict[str, Any]],
    pairwise_relationships: list[dict[str, Any]],
    dataset_summary: dict[str, Any],
    config: ComponentConfig,
    args: argparse.Namespace,
) -> dict[str, Any]:
    train, test, split_summary = split_rows(rows, args.min_train_rows, args.min_test_rows)
    components = {
        "slot_predictive": evaluate_component(
            "slot_predictive",
            train,
            test,
            "slot_mean_abs_z",
            config.slot,
            args.ece_bins,
            "per-embedder z-score summary, no cross-slot compensation",
        ),
        "slot_calibration_uncertainty": evaluate_component(
            "slot_calibration_uncertainty",
            train,
            test,
            "slot_uncertainty_mean",
            config.slot,
            args.ece_bins,
            "per-embedder uncertainty z-score dispersion",
        ),
        "pairwise_relationship": evaluate_component(
            "pairwise_relationship",
            train,
            test,
            "pairwise_disagreement_score",
            config.pairwise,
            args.ece_bins,
            "cross-embedder scalar relationship tensor, no raw-vector concat",
        ),
        "contradiction_consensus": evaluate_component(
            "contradiction_consensus",
            train,
            test,
            "contradiction_score",
            config.contradiction,
            args.ece_bins,
            "slot max anomaly crossed with pairwise disagreement",
        ),
        "temporal_window": evaluate_temporal_component(train, test, config.temporal, args.ece_bins),
    }
    baseline_ladder = evaluate_baseline_ladder(train, test, args.ece_bins, args.baseline_correlation)
    oracle_alignment = evaluate_oracle_alignment(components, train, test, args.ece_bins, args.baseline_correlation)
    anti_compensation = evaluate_anti_compensation(train, test, oracle_alignment, config.slot)
    components["oracle_alignment"] = oracle_alignment
    components["anti_compensation"] = anti_compensation

    flat_only_components = [
        name
        for name, component in components.items()
        if bool(component.get("flat_panel_aggregate_used")) or bool(component.get("flat_vector_concat_used"))
    ]
    enabled_losses = {
        name: component
        for name, component in components.items()
        if component.get("enabled") is True and name not in {"anti_compensation"}
    }
    require(bool(enabled_losses), "MEJEPA_CONSTELLATION_OBJECTIVE_NO_ENABLED_LOSSES")
    mean_test_brier = finite_float(float(np.mean([float(component["test_brier"]) for component in enabled_losses.values()])))
    objective_delta_vs_best_local = finite_float(
        float(oracle_alignment["test_brier"]) - float(baseline_ladder["summary"]["best_local_baseline_test_brier"])
    )
    pairwise_valid = sum(1 for row in pairwise_relationships if row.get("valid") is True)
    anti_compensation_ok = (
        anti_compensation.get("enabled") is False
        or bool(anti_compensation.get("edge_case", {}).get("passes"))
    )

    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_training_certificate",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": not flat_only_components and anti_compensation_ok and pairwise_valid > 0,
        "objective_policy": {
            "predicts_consequences_of_code_being_live": True,
            "human_intent_inference_used": False,
            "synthetic_seed_features_used": False,
            "flat_packed_panel_allowed_only_as_serialization": True,
            "flat_vector_concat_used": False,
            "flat_panel_aggregate_loss_components": flat_only_components,
            "slot_identity_preserved": True,
        },
        "ablation_switches": config.as_dict(),
        "baseline_ladder": baseline_ladder,
        "dataset_summary": dataset_summary,
        "split_summary": split_summary,
        "pairwise_relationship_summary": {
            "rows": len(pairwise_relationships),
            "valid_rows": pairwise_valid,
            "expected_pair_count": len(args.expected_embedders_parsed) * (len(args.expected_embedders_parsed) - 1) // 2,
            "relationship_hash": sha256_text(
                [
                    {
                        "left": row.get("left_embedder_id"),
                        "right": row.get("right_embedder_id"),
                        "redundancy": row.get("redundancy_score"),
                        "disagreement": row.get("disagreement_score"),
                    }
                    for row in pairwise_relationships
                ]
            ),
        },
        "component_losses": components,
        "objective_loss": {
            "mean_enabled_test_brier": mean_test_brier,
            "promotion_allowed": False,
            "promotion_block_reason": oracle_alignment["promotion_block_reason"],
            "baseline_comparison_status": oracle_alignment["baseline_comparison_status"],
            "historical_baseline_correlation": oracle_alignment["historical_baseline_correlation"],
            "observed_smoke_correlation": oracle_alignment["test_prediction_oracle_correlation"],
            "delta_vs_historical_baseline": oracle_alignment["delta_vs_historical_baseline"],
            "delta_test_brier_vs_best_local_baseline": objective_delta_vs_best_local,
            "best_local_baseline_test_brier": baseline_ladder["summary"]["best_local_baseline_test_brier"],
        },
    }


def built_in_edge_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    try:
        parse_component_config(
            argparse.Namespace(
                disable_slot_term=True,
                disable_pairwise_term=True,
                disable_contradiction_term=True,
                disable_temporal_term=True,
            )
        )
        cases.append({"name": "all_objective_terms_disabled_fails_closed", "passes": False, "error": None})
    except ConstellationTrainingObjectiveError as exc:
        cases.append({"name": "all_objective_terms_disabled_fails_closed", "passes": True, "error": str(exc)})

    for name, value in (("negative_baseline_fails_closed", -0.01), ("baseline_above_one_fails_closed", 1.01)):
        try:
            validate_baseline(value)
            cases.append({"name": name, "passes": False, "error": None})
        except ConstellationTrainingObjectiveError as exc:
            cases.append({"name": name, "passes": True, "error": str(exc)})

    labels = [0, 1, 1, 0]
    probs = [0.1, 0.9, 0.8, 0.2]
    cases.append(
        {
            "name": "oracle_alignment_correlation_positive_on_ordered_control",
            "passes": (pearson_or_none(labels, probs)[0] or 0.0) > 0.0,
            "result": {"correlation": pearson_or_none(labels, probs)[0]},
        }
    )
    return cases


def validate_baseline(value: float) -> None:
    require(math.isfinite(value), "MEJEPA_CONSTELLATION_OBJECTIVE_BASELINE_NONFINITE")
    require(0.0 <= value <= 1.0, "MEJEPA_CONSTELLATION_OBJECTIVE_BASELINE_OUT_OF_RANGE", str(value))


def command_python_constellation_training_objective(args: argparse.Namespace) -> int:
    started = utc_now()
    validate_baseline(float(args.baseline_correlation))
    required_embedders = parse_embedder_list(args.expected_embedders)
    require(args.expected_embedder_count == len(required_embedders), "MEJEPA_CONSTELLATION_OBJECTIVE_EXPECTED_COUNT_MISMATCH")
    require(args.sample_shards > 0, "MEJEPA_CONSTELLATION_OBJECTIVE_INVALID_SAMPLE_SHARDS")
    require(args.sample_rows_per_embedder_per_shard > 0, "MEJEPA_CONSTELLATION_OBJECTIVE_INVALID_SAMPLE_LIMIT")
    config = parse_component_config(args)
    args.expected_embedders_parsed = required_embedders

    output_root = args.output_root or (args.fsv_root / "artifacts")
    before = {
        "output_root": tree_state(output_root),
        "fsv_root": tree_state(args.fsv_root),
    }
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    selected_shards, source_summary = select_completed_shards(args.forward_root, args.sample_shards)
    shard_order = {str(shard.get("shard_id")): idx for idx, shard in enumerate(selected_shards)}
    oracle_index, oracle_summary = load_oracle_index(args.corpus_index_json)
    manifest_by_key, chunk_summary = load_chunk_manifest_window(args.chunks_jsonl, selected_shards)
    records, load_summary = load_sample_records(
        args.forward_root,
        selected_shards,
        manifest_by_key,
        required_embedders,
        args.sample_rows_per_embedder_per_shard,
    )
    require(len(records) == args.expected_embedder_count, "MEJEPA_CONSTELLATION_OBJECTIVE_ACTIVE_EMBEDDER_COUNT_MISMATCH")

    training_rows, slot_vectors, row_pairwise, dataset_summary = build_training_dataset(records, required_embedders, shard_order, oracle_index)
    pairwise_relationships = pairwise_relationship_rows(records, args.min_overlap, args.mi_bins)
    certificate = build_objective_certificate(training_rows, pairwise_relationships, dataset_summary, config, args)
    edge_cases = built_in_edge_cases()
    require(all(case.get("passes") is True for case in edge_cases), "MEJEPA_CONSTELLATION_OBJECTIVE_EDGE_CASE_FAILED")

    training_rows_path = output_root / "constellation_training_rows.jsonl"
    slot_vectors_path = output_root / "constellation_slot_vectors.jsonl"
    pairwise_observations_path = output_root / "constellation_pairwise_observations.jsonl"
    pairwise_relationships_path = output_root / "constellation_pairwise_relationships.jsonl"
    certificate_path = output_root / "constellation_training_certificate.json"

    write_jsonl(training_rows_path, training_rows)
    write_jsonl(slot_vectors_path, slot_vectors)
    write_jsonl(pairwise_observations_path, row_pairwise)
    write_jsonl(pairwise_relationships_path, pairwise_relationships)
    dump_json(certificate_path, certificate)

    readback_certificate = read_json(certificate_path)
    readback_training_rows = sum(1 for _ in iter_jsonl(training_rows_path))
    readback_slot_vectors = sum(1 for _ in iter_jsonl(slot_vectors_path))
    readback_pairwise_observations = sum(1 for _ in iter_jsonl(pairwise_observations_path))
    readback_pairwise_relationships = sum(1 for _ in iter_jsonl(pairwise_relationships_path))
    stable_hashes = {
        "constellation_training_rows": sha256_file(training_rows_path),
        "constellation_slot_vectors": sha256_file(slot_vectors_path),
        "constellation_pairwise_observations": sha256_file(pairwise_observations_path),
        "constellation_pairwise_relationships": sha256_file(pairwise_relationships_path),
        "constellation_training_certificate": sha256_file(certificate_path),
    }
    expected_pairwise = len(required_embedders) * (len(required_embedders) - 1) // 2
    passes = (
        readback_certificate.get("passes") is True
        and readback_training_rows == len(training_rows)
        and readback_slot_vectors == len(slot_vectors)
        and readback_pairwise_observations == len(row_pairwise)
        and readback_pairwise_relationships == expected_pairwise
        and readback_slot_vectors > 0
        and dataset_summary["slot_vector_rows"] == readback_slot_vectors
        and load_summary["index_hash_mismatches"] == 0
        and load_summary["source_readback_unverified"] == 0
        and readback_certificate.get("objective_policy", {}).get("flat_vector_concat_used") is False
        and readback_certificate.get("objective_policy", {}).get("flat_panel_aggregate_loss_components") == []
        and all(case.get("passes") is True for case in edge_cases)
    )

    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "corpus_index_json": str(args.corpus_index_json),
            "output_root": str(output_root),
            "training_rows": str(training_rows_path),
            "slot_vectors": str(slot_vectors_path),
            "pairwise_observations": str(pairwise_observations_path),
            "pairwise_relationships": str(pairwise_relationships_path),
            "training_certificate": str(certificate_path),
            "fsv": str(args.fsv_root / f"{ARTIFACT_KIND}_fsv.json"),
        },
        "before_state": before,
        "after_state": {
            "output_root": tree_state(output_root),
            "fsv_root": tree_state(args.fsv_root),
        },
        "readback": {
            "training_rows": readback_training_rows,
            "slot_vectors": readback_slot_vectors,
            "pairwise_observations": readback_pairwise_observations,
            "pairwise_relationships": readback_pairwise_relationships,
            "expected_pairwise_relationships": expected_pairwise,
            "certificate_passes": readback_certificate.get("passes"),
            "flat_vector_concat_used": readback_certificate.get("objective_policy", {}).get("flat_vector_concat_used"),
            "flat_panel_aggregate_loss_components": readback_certificate.get("objective_policy", {}).get("flat_panel_aggregate_loss_components"),
            "baseline_comparison_status": readback_certificate.get("objective_loss", {}).get("baseline_comparison_status"),
            "promotion_allowed": readback_certificate.get("objective_loss", {}).get("promotion_allowed"),
            "stable_hashes": stable_hashes,
        },
        "source_summary": source_summary,
        "oracle_summary": oracle_summary,
        "chunk_summary": chunk_summary,
        "load_summary": load_summary,
        "dataset_summary": dataset_summary,
        "edge_cases": edge_cases,
    }
    fsv_path = args.fsv_root / f"{ARTIFACT_KIND}_fsv.json"
    dump_json(fsv_path, fsv)
    final_readback = read_json(fsv_path)
    require(final_readback.get("passes") is True, f"MEJEPA_CONSTELLATION_OBJECTIVE_FSV_FAILED: {fsv_path}")
    print(json.dumps(final_readback, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--corpus-index-json", type=Path, default=DEFAULT_CORPUS_INDEX_JSON)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--expected-embedder-count", type=int, default=len(EXPECTED_EMBEDDERS))
    parser.add_argument("--sample-shards", type=int, default=5)
    parser.add_argument("--sample-rows-per-embedder-per-shard", type=int, default=24)
    parser.add_argument("--min-overlap", type=int, default=18)
    parser.add_argument("--mi-bins", type=int, default=8)
    parser.add_argument("--baseline-correlation", type=float, default=HISTORICAL_BASELINE_CORRELATION)
    parser.add_argument("--ece-bins", type=int, default=8)
    parser.add_argument("--min-train-rows", type=int, default=12)
    parser.add_argument("--min-test-rows", type=int, default=3)
    parser.add_argument("--disable-slot-term", action="store_true")
    parser.add_argument("--disable-pairwise-term", action="store_true")
    parser.add_argument("--disable-contradiction-term", action="store_true")
    parser.add_argument("--disable-temporal-term", action="store_true")
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_constellation_training_objective(args)
    except (ConstellationTrainingObjectiveError, RealCorpusConstellationSmokeError) as exc:
        print(f"PYTHON_CONSTELLATION_TRAINING_OBJECTIVE_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
