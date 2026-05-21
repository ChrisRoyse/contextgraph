"""TASK-PY-G-097 calibrated constellation ablations.

The suite consumes the real aiwonder TASK-PY-G-096 training arrays, the
TASK-PY-G-088 slot/pair tensor store, and the TASK-PY-G-089 per-slot
calibration/OOD state. Flat-vector features remain comparison-only evidence;
ship-gate-eligible paths operate on named slots, named pairs, calibrated guard
state, and temporal windows without merging the panel into one compensable
space.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

import numpy as np

from .core import (
    DEFAULT_CONSTELLATION_TRAINING_ARRAY_ROOT,
    HISTORICAL_BASELINE_CORRELATION,
    SwebenchOpsError,
    accuracy_at_half,
    brier,
    calibration_ece,
    pearson_or_none,
    validate_constellation_training_artifact_root,
)
from .python_real_corpus_constellation_smoke import finite_float, sha256_text, write_jsonl
from .smoke_intelligence_matrices import dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-097"
ARTIFACT_KIND = "python_constellation_ablation_with_calibration"
FORMULA_VERSION = "slot_pair_calibrated_constellation_ablation_v1"
DEFAULT_ARTIFACT_ROOT = DEFAULT_CONSTELLATION_TRAINING_ARRAY_ROOT
DEFAULT_TENSOR_ROOT = Path("/zfs/archive/contextgraph/corpus/constellation-tensors-v1")
DEFAULT_CALIBRATION_ROOT = Path("/zfs/archive/contextgraph/state/calibration/per-slot-v1")
DEFAULT_CALIBRATION_FSV = Path("/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-calibration-ood-fsv/python_per_slot_calibration_ood_fsv.json")
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-097-constellation-ablation-with-calibration-fsv")
EPS = 1.0e-9


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
    use_slot_calibration: bool
    use_pair_calibration: bool
    use_contradiction_consensus: bool
    use_anti_compensation: bool


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise ConstellationAblationSuiteError(code if detail is None else f"{code}: {detail}")


def parse_utc(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ConstellationAblationSuiteError(f"MEJEPA_CONSTELLATION_ABLATION_BAD_TIMESTAMP: {value}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


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


def load_artifacts(root: Path) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any], dict[str, str]]:
    try:
        artifact_guard = validate_constellation_training_artifact_root(
            root,
            consumer="python_constellation_ablation_suite",
        )
    except SwebenchOpsError as exc:
        raise ConstellationAblationSuiteError(f"{exc.code}: {exc.message}") from exc
    rows_path = root / "constellation_training_rows.jsonl"
    slots_path = root / "constellation_slot_vectors.jsonl"
    pair_obs_path = root / "constellation_pairwise_observations.jsonl"
    certificate_path = root / "constellation_training_certificate.json"
    rows = load_required_jsonl(rows_path, "MEJEPA_CONSTELLATION_ABLATION_TRAINING_ROWS_MISSING")
    slots = load_required_jsonl(slots_path, "MEJEPA_CONSTELLATION_ABLATION_SLOT_VECTORS_MISSING")
    pair_observations = load_required_jsonl(pair_obs_path, "MEJEPA_CONSTELLATION_ABLATION_PAIR_OBSERVATIONS_MISSING")
    certificate = artifact_guard["certificate"]
    hashes = {
        "training_rows": sha256_file(rows_path),
        "slot_vectors": sha256_file(slots_path),
        "pairwise_observations": sha256_file(pair_obs_path),
        "pairwise_relationships": artifact_guard["hashes"]["constellation_pairwise_relationships.jsonl"],
        "training_certificate": artifact_guard["hashes"]["constellation_training_certificate.json"],
    }
    return rows, slots, pair_observations, certificate, hashes


def normalize_pair(left: str, right: str) -> tuple[str, str]:
    return tuple(sorted((str(left), str(right))))


def load_tensor_context(root: Path) -> dict[str, Any]:
    require(root.exists(), "MEJEPA_CONSTELLATION_ABLATION_TENSOR_ROOT_MISSING", str(root))
    slot_path = root / "slot_distribution_tensors.jsonl"
    pair_path = root / "pairwise_correlation_tensors.jsonl"
    report_path = root / "constellation_tensor_report.json"
    manifest_path = root / "constellation_tensor_manifest.json"
    require(report_path.exists(), "MEJEPA_CONSTELLATION_ABLATION_TENSOR_REPORT_MISSING", str(report_path))
    require(manifest_path.exists(), "MEJEPA_CONSTELLATION_ABLATION_TENSOR_MANIFEST_MISSING", str(manifest_path))
    slot_tensors = load_required_jsonl(slot_path, "MEJEPA_CONSTELLATION_ABLATION_SLOT_TENSORS_MISSING")
    pair_tensors = load_required_jsonl(pair_path, "MEJEPA_CONSTELLATION_ABLATION_PAIR_TENSORS_MISSING")
    report = read_json(report_path)
    manifest = read_json(manifest_path)
    require(report.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_REPORT_NOT_PASSING")
    policy = report.get("constellation_policy") or {}
    require(policy.get("slot_identity_preserved") is True, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_SLOT_IDENTITY_NOT_PRESERVED")
    require(policy.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_FLAT_VECTOR_USED")
    freshness = report.get("freshness") or {}
    require(parse_utc(str(freshness.get("stale_after_utc"))) > datetime.now(timezone.utc), "MEJEPA_CONSTELLATION_ABLATION_TENSOR_STALE")

    slot_support = Counter()
    for row in slot_tensors:
        if row.get("valid") is True:
            slot_support[str(row.get("embedder_id"))] += 1

    pair_surfaces: dict[tuple[str, str], Counter] = defaultdict(Counter)
    pair_valid_support = Counter()
    for row in pair_tensors:
        key = normalize_pair(str(row.get("left_embedder_id")), str(row.get("right_embedder_id")))
        pair_surfaces[key][str(row.get("surface_class") or "unknown")] += 1
        if row.get("valid") is True:
            pair_valid_support[key] += 1

    surface_weight = {"redundant": 0.0, "independent": 0.25, "mixed": 0.5, "contradictory": 1.0, "unsupported": 0.75}
    pair_surface_risk = {}
    for key, counts in pair_surfaces.items():
        total = sum(counts.values())
        risk = 0.0 if total == 0 else sum(surface_weight.get(name, 0.5) * count for name, count in counts.items()) / total
        pair_surface_risk[key] = finite_float(risk)

    return {
        "root": str(root),
        "slot_support": dict(sorted(slot_support.items())),
        "pair_surface_counts": {"|".join(key): dict(sorted(counts.items())) for key, counts in sorted(pair_surfaces.items())},
        "pair_valid_support": {"|".join(key): count for key, count in sorted(pair_valid_support.items())},
        "pair_surface_risk": pair_surface_risk,
        "report": report,
        "manifest": manifest,
        "paths": {
            "slot_distribution_tensors": str(slot_path),
            "pairwise_correlation_tensors": str(pair_path),
            "constellation_tensor_report": str(report_path),
            "constellation_tensor_manifest": str(manifest_path),
        },
        "hashes": {
            "slot_distribution_tensors": sha256_file(slot_path),
            "pairwise_correlation_tensors": sha256_file(pair_path),
            "constellation_tensor_report": sha256_file(report_path),
            "constellation_tensor_manifest": sha256_file(manifest_path),
        },
    }


def load_calibration_state(root: Path, calibration_fsv: Path | None, tensor_context: dict[str, Any]) -> dict[str, Any]:
    require(root.exists(), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_ROOT_MISSING", str(root))
    paths = {
        "per_slot_calibration": root / "per_slot_calibration.jsonl",
        "per_cell_slot_calibration": root / "per_cell_slot_calibration.jsonl",
        "per_pair_calibration": root / "per_pair_calibration.jsonl",
        "ood_guard_policy": root / "ood_guard_policy.json",
        "per_slot_calibration_report": root / "per_slot_calibration_report.json",
    }
    per_slot = load_required_jsonl(paths["per_slot_calibration"], "MEJEPA_CONSTELLATION_ABLATION_PER_SLOT_CALIBRATION_MISSING")
    per_cell = load_required_jsonl(paths["per_cell_slot_calibration"], "MEJEPA_CONSTELLATION_ABLATION_PER_CELL_CALIBRATION_MISSING")
    per_pair = load_required_jsonl(paths["per_pair_calibration"], "MEJEPA_CONSTELLATION_ABLATION_PER_PAIR_CALIBRATION_MISSING")
    policy = read_json(paths["ood_guard_policy"])
    report = read_json(paths["per_slot_calibration_report"])
    fsv = None
    fsv_hash = None
    if calibration_fsv is not None:
        require(calibration_fsv.exists(), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_FSV_MISSING", str(calibration_fsv))
        fsv = read_json(calibration_fsv)
        fsv_hash = sha256_file(calibration_fsv)
    state = {
        "root": str(root),
        "policy": policy,
        "report": report,
        "fsv": fsv,
        "per_slot_rows": per_slot,
        "per_cell_rows": per_cell,
        "per_pair_rows": per_pair,
        "tensor_report": tensor_context["report"],
        "paths": {key: str(path) for key, path in paths.items()} | ({"calibration_fsv": str(calibration_fsv)} if calibration_fsv is not None else {}),
        "hashes": {key: sha256_file(path) for key, path in paths.items()} | ({"calibration_fsv": fsv_hash} if fsv_hash is not None else {}),
    }
    validate_calibration_state(state, datetime.now(timezone.utc))
    state["required_slots"] = tuple(str(slot) for slot in policy["required_slots"])
    state["per_slot_by_slot"] = {str(row["embedder_id"]): row for row in per_slot}
    state["per_pair_by_pair"] = {normalize_pair(str(row["left_slot_id"]), str(row["right_slot_id"])): row for row in per_pair}
    return state


def validate_calibration_state(state: dict[str, Any], now: datetime) -> None:
    policy = state["policy"]
    report = state["report"]
    tensor_report = state["tensor_report"]
    fsv = state.get("fsv")
    require(policy.get("strict_conjunction") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_NOT_STRICT")
    require(policy.get("aggregate_score_never_overrides_slot_violation") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_ALLOWS_AGGREGATE_OVERRIDE")
    require(policy.get("reject_missing_or_stale_calibration") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_DOES_NOT_REJECT_STALE")
    require(policy.get("global_slot_calibration_ready") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_NOT_GLOBAL_READY")
    require(parse_utc(str(policy.get("stale_after_utc"))) > now, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_POLICY_STALE")
    require(report.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_REPORT_NOT_PASSING")
    require(report.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_REPORT_FLAT_VECTOR_USED")
    require(report.get("slot_identity_preserved") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_IDENTITY_NOT_PRESERVED")
    if fsv is not None:
        require(fsv.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_FSV_NOT_PASSING")
    tensor_policy = tensor_report.get("constellation_policy") or {}
    require(tensor_report.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_REPORT_NOT_PASSING")
    require(tensor_policy.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_REPORT_FLAT_VECTOR_USED")
    require(parse_utc(str((tensor_report.get("freshness") or {}).get("stale_after_utc"))) > now, "MEJEPA_CONSTELLATION_ABLATION_TENSOR_REPORT_STALE")

    required = tuple(str(slot) for slot in policy.get("required_slots") or [])
    require(bool(required), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_REQUIRED_SLOTS_EMPTY")
    by_slot = {str(row.get("embedder_id")): row for row in state["per_slot_rows"]}
    require(tuple(sorted(by_slot)) == tuple(sorted(required)), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_SET_MISMATCH")
    for slot_id, row in by_slot.items():
        require(row.get("valid") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_INVALID", slot_id)
        require(row.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_FLAT_VECTOR_USED", slot_id)
        require(parse_utc(str(row.get("stale_after_utc"))) > now, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_STALE", slot_id)
        require(isinstance((row.get("thresholds") or {}).get("ood_tau"), (int, float)), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_TAU_MISSING", slot_id)
        require(row.get("model") is not None, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_SLOT_MODEL_MISSING", slot_id)
    pair_rows = state["per_pair_rows"]
    pair_keys = [normalize_pair(str(row.get("left_slot_id")), str(row.get("right_slot_id"))) for row in pair_rows]
    require(len(pair_rows) == 66, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_COUNT_INVALID", str(len(pair_rows)))
    require(len(pair_keys) == len(set(pair_keys)), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_DUPLICATE")
    for row, key in zip(pair_rows, pair_keys):
        name = "|".join(key)
        require(row.get("valid") is True, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_INVALID", name)
        require(row.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_FLAT_VECTOR_USED", name)
        require(parse_utc(str(row.get("stale_after_utc"))) > now, "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_STALE", name)
        require(isinstance((row.get("thresholds") or {}).get("disagreement_tau"), (int, float)), "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_PAIR_TAU_MISSING", name)


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


def slot_for(row: dict[str, Any], embedder_id: str) -> dict[str, Any] | None:
    for slot in row.get("slots") or []:
        if str(slot.get("embedder_id")) == embedder_id:
            return slot
    return None


def sigmoid(value: float) -> float:
    return finite_float(1.0 / (1.0 + math.exp(max(-40.0, min(40.0, -value)))))


def calibrated_slot_probability(score: float, row: dict[str, Any]) -> float:
    model = row["model"]
    return sigmoid(float(model["direction"]) * (score - float(model["center"])) / max(EPS, float(model["scale"])))


def pair_observation_score(row: dict[str, Any]) -> float:
    deltas = row.get("scalar_feature_abs_delta") or {}
    values = [float(value) for value in deltas.values() if isinstance(value, (int, float)) and math.isfinite(float(value))]
    require(bool(values), "MEJEPA_CONSTELLATION_ABLATION_PAIR_OBSERVATION_EMPTY")
    return finite_float(float(np.mean(values)))


def build_pair_observation_index(pair_observations: list[dict[str, Any]]) -> dict[str, dict[tuple[str, str], float]]:
    index: dict[str, dict[tuple[str, str], float]] = defaultdict(dict)
    for row in pair_observations:
        require(row.get("flat_vector_concat_used") is False, "MEJEPA_CONSTELLATION_ABLATION_PAIR_OBSERVATION_FLAT_FLAGGED")
        key = normalize_pair(str(row.get("left_embedder_id")), str(row.get("right_embedder_id")))
        index[str(row.get("row_key"))][key] = pair_observation_score(row)
    return index


def attach_calibrated_features(rows: list[dict[str, Any]], pair_observations: list[dict[str, Any]], calibration: dict[str, Any], tensor_context: dict[str, Any]) -> None:
    pair_index = build_pair_observation_index(pair_observations)
    required_slots = tuple(calibration["required_slots"])
    per_slot = calibration["per_slot_by_slot"]
    per_pair = calibration["per_pair_by_pair"]
    pair_surface_risk = tensor_context["pair_surface_risk"]
    for row in rows:
        slot_norms = []
        slot_probs = []
        slot_violations = 0
        missing_slots = 0
        calibrated_slots = []
        for slot_id in required_slots:
            cal = per_slot[slot_id]
            slot = slot_for(row, slot_id)
            if slot is None:
                missing_slots += 1
                slot_norms.append(2.0)
                slot_probs.append(1.0)
                calibrated_slots.append({"slot_id": slot_id, "present": False, "guard_rejects": True, "reason": "MISSING_SLOT_OBSERVATION"})
                continue
            score = finite_float(float(slot["slot_anomaly_score"]))
            tau = finite_float(float(cal["thresholds"]["ood_tau"]))
            normalized = finite_float(score / max(EPS, tau))
            probability = calibrated_slot_probability(score, cal)
            guard_rejects = score > tau
            slot_violations += 1 if guard_rejects else 0
            slot_norms.append(normalized)
            slot_probs.append(probability)
            calibrated_slots.append(
                {
                    "slot_id": slot_id,
                    "present": True,
                    "score": score,
                    "ood_tau": tau,
                    "normalized_score": normalized,
                    "probability_known_bad": probability,
                    "guard_rejects": guard_rejects,
                }
            )

        row_pair_scores = pair_index.get(str(row.get("row_key")), {})
        pair_norms = []
        pair_probs = []
        pair_surface_values = []
        pair_violations = 0
        missing_pairs = 0
        calibrated_pairs = []
        for key, cal in sorted(per_pair.items()):
            tau = finite_float(float(cal["thresholds"]["disagreement_tau"]))
            score = row_pair_scores.get(key)
            if score is None:
                missing_pairs += 1
                pair_norms.append(1.0)
                pair_probs.append(0.5)
                calibrated_pairs.append({"pair_id": "|".join(key), "present": False, "reason": "PAIR_OBSERVATION_MISSING"})
                continue
            normalized = finite_float(score / max(EPS, tau))
            probability = sigmoid(normalized - 1.0)
            violates = score > tau
            pair_violations += 1 if violates else 0
            pair_norms.append(normalized)
            pair_probs.append(probability)
            risk = finite_float(float(pair_surface_risk.get(key, 0.5)))
            pair_surface_values.append(risk)
            calibrated_pairs.append(
                {
                    "pair_id": "|".join(key),
                    "present": True,
                    "score": score,
                    "disagreement_tau": tau,
                    "normalized_score": normalized,
                    "probability_known_bad": probability,
                    "surface_risk": risk,
                    "guard_rejects": violates,
                }
            )

        required_slot_count = len(required_slots)
        required_pair_count = len(per_pair)
        objective = row.setdefault("objective_features", {})
        objective.update(
            {
                "slot_calibration_mean_probability": finite_float(float(np.mean(slot_probs))),
                "slot_calibration_max_probability": finite_float(float(np.max(slot_probs))),
                "slot_calibration_mean_normalized_score": finite_float(float(np.mean(slot_norms))),
                "slot_calibration_max_normalized_score": finite_float(float(np.max(slot_norms))),
                "slot_calibration_violation_fraction": finite_float(slot_violations / required_slot_count),
                "slot_calibration_missing_fraction": finite_float(missing_slots / required_slot_count),
                "pair_calibration_mean_probability": finite_float(float(np.mean(pair_probs))),
                "pair_calibration_mean_normalized_disagreement": finite_float(float(np.mean(pair_norms))),
                "pair_calibration_max_normalized_disagreement": finite_float(float(np.max(pair_norms))),
                "pair_calibration_violation_fraction": finite_float(pair_violations / required_pair_count),
                "pair_calibration_missing_fraction": finite_float(missing_pairs / required_pair_count),
                "pair_tensor_surface_risk_mean": finite_float(float(np.mean(pair_surface_values))) if pair_surface_values else 0.5,
                "anti_compensation_guard_rejects": 1.0 if slot_violations > 0 or missing_slots > 0 else 0.0,
                "anti_compensation_margin": finite_float(max(0.0, float(np.max(slot_norms)) - 1.0)),
            }
        )
        row["calibrated_slots"] = calibrated_slots
        row["calibrated_pairs"] = calibrated_pairs


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
        scale=finite_float(max(EPS, float(np.std(values)))),
        direction=finite_float(1.0 if bad_mean >= good_mean else -1.0),
        train_good_mean=finite_float(good_mean),
        train_bad_mean=finite_float(bad_mean),
    )


def scalar_probability(row: dict[str, Any], model: ScalarModel) -> float:
    return sigmoid(model.direction * (feature_value(row, model.feature) - model.center) / model.scale)


def temporal_probability_maps(train: list[dict[str, Any]]) -> tuple[float, dict[str, float]]:
    global_rate = finite_float(float(np.mean([label(row) for row in train])))
    grouped: dict[str, list[int]] = defaultdict(list)
    for row in train:
        grouped[temporal_key(row)].append(label(row))
    rates = {key: finite_float((sum(values) + global_rate) / (len(values) + 1.0)) for key, values in grouped.items()}
    return global_rate, rates


def temporal_key(row: dict[str, Any]) -> str:
    return f"{row.get('cell_key')}|{int(row.get('temporal', {}).get('window_index', 0))}"


def evaluate_spec(spec: AblationSpec, train: list[dict[str, Any]], test: list[dict[str, Any]], ece_bins: int) -> dict[str, Any]:
    models = [fit_scalar_model(train, feature) for feature in spec.features]
    global_rate, temporal_rates = temporal_probability_maps(train)
    train_probs = [row_probability(row, models, spec, global_rate, temporal_rates) for row in train]
    test_probs = [row_probability(row, models, spec, global_rate, temporal_rates) for row in test]
    metrics = metric_block([label(row) for row in test], test_probs, ece_bins)
    return {
        "name": spec.name,
        "schema_version": SCHEMA_VERSION,
        "input_policy": spec.input_policy,
        "features": list(spec.features),
        "include_temporal": spec.include_temporal,
        "flat_vector_concat_used": spec.flat_vector_baseline,
        "ship_gate_eligible": spec.ship_gate_eligible,
        "component_flags": {
            "slot_calibration": spec.use_slot_calibration,
            "pair_calibration": spec.use_pair_calibration,
            "contradiction_consensus": spec.use_contradiction_consensus,
            "temporal_trajectory": spec.include_temporal,
            "anti_compensation": spec.use_anti_compensation,
        },
        "model_count": len(models) + (1 if spec.include_temporal else 0) + (1 if spec.use_anti_compensation else 0),
        "models": [model.__dict__ for model in models],
        "train_metrics": metric_block([label(row) for row in train], train_probs, ece_bins),
        "test_metrics": metrics,
        "per_cell_metrics": per_cell_metrics(test, test_probs, ece_bins),
        "anti_compensation": anti_compensation_check(spec, test, test_probs),
    }


def row_probability(row: dict[str, Any], models: list[ScalarModel], spec: AblationSpec, global_rate: float, temporal_rates: dict[str, float]) -> float:
    probs = [scalar_probability(row, model) for model in models]
    if spec.include_temporal:
        probs.append(finite_float(temporal_rates.get(temporal_key(row), global_rate)))
    require(bool(probs), "MEJEPA_CONSTELLATION_ABLATION_NO_MODEL_COMPONENTS")
    probability = finite_float(float(np.mean(probs)))
    if spec.use_anti_compensation and feature_value(row, "anti_compensation_guard_rejects") >= 0.5:
        probability = finite_float(max(probability, 0.99))
    return probability


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


def anti_compensation_check(spec: AblationSpec, rows: list[dict[str, Any]], probabilities: list[float]) -> dict[str, Any]:
    guarded = [(row, prob) for row, prob in zip(rows, probabilities) if feature_value(row, "anti_compensation_guard_rejects") >= 0.5]
    bad_guarded = [(row, prob) for row, prob in guarded if label(row) == 1]
    recall_value = None if not bad_guarded else finite_float(sum(1 for _, prob in bad_guarded if prob >= 0.5) / len(bad_guarded))
    passes = (not spec.ship_gate_eligible) or (spec.use_anti_compensation and (recall_value is None or recall_value >= 0.5))
    return {
        "guarded_rows": len(guarded),
        "bad_guarded_rows": len(bad_guarded),
        "bad_guarded_recall_at_0_5": recall_value,
        "uses_anti_compensation_guard": spec.use_anti_compensation,
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
    raw_slot = ("slot_mean_abs_z", "slot_max_abs_z", "slot_spread_abs_z", "slot_uncertainty_mean")
    slot_cal = (
        "slot_calibration_mean_probability",
        "slot_calibration_max_probability",
        "slot_calibration_mean_normalized_score",
        "slot_calibration_max_normalized_score",
        "slot_calibration_violation_fraction",
    )
    raw_pair = ("pairwise_disagreement_score", "pairwise_max_scalar_delta")
    pair_cal = (
        "pair_calibration_mean_probability",
        "pair_calibration_mean_normalized_disagreement",
        "pair_calibration_max_normalized_disagreement",
        "pair_calibration_violation_fraction",
        "pair_tensor_surface_risk_mean",
    )
    contradiction = ("contradiction_score", "consensus_score")
    flat = ("flat_concat_norm_l2", "flat_concat_abs_mean", "flat_concat_signed_mean")
    full = slot_cal + pair_cal + contradiction
    return [
        AblationSpec("flat_vector_baseline_comparison_only", flat, False, True, False, "comparison baseline; concatenates slots and is never ship-gate eligible", False, False, False, False),
        AblationSpec("calibrated_full_constellation", full, True, False, True, "slot calibration plus pair calibration plus contradiction/consensus plus temporal trajectory plus anti-compensation", True, True, True, True),
        AblationSpec("disable_slot_calibration", raw_slot + pair_cal + contradiction, True, False, True, "raw slot summaries replace per-slot calibration; anti-compensation still guards #389 OOD violations", False, True, True, True),
        AblationSpec("disable_pair_calibration", slot_cal + raw_pair + contradiction, True, False, True, "per-slot calibration remains, but cross-embedder pair calibration is replaced by raw #396 pair summaries", True, False, True, True),
        AblationSpec("disable_contradiction_consensus", slot_cal + pair_cal, True, False, True, "calibrated slot/pair inputs without contradiction or consensus features", True, True, False, True),
        AblationSpec("disable_temporal_trajectory", full, False, False, True, "calibrated slot/pair/contradiction inputs without aiwonder shard/window trajectory rates", True, True, True, True),
        AblationSpec("disable_anti_compensation", full, True, False, False, "comparison-only ablation; measures what happens when #389 strict one-bad-slot rejection is removed", True, True, True, False),
    ]


def per_slot_contribution_metrics(test: list[dict[str, Any]], calibration: dict[str, Any], ece_bins: int) -> dict[str, Any]:
    out = {}
    for slot_id in calibration["required_slots"]:
        values = []
        for row in test:
            for item in row.get("calibrated_slots") or []:
                if item.get("slot_id") == slot_id and item.get("present") is True:
                    values.append((label(row), float(item["probability_known_bad"]), bool(item["guard_rejects"])))
        require(bool(values), "MEJEPA_CONSTELLATION_ABLATION_SLOT_CONTRIBUTION_EMPTY", slot_id)
        out[slot_id] = {
            "support_rows": len(values),
            "metrics": metric_block([item[0] for item in values], [item[1] for item in values], ece_bins),
            "ood_violation_rows": sum(1 for _label, _prob, rejected in values if rejected),
            "tensor_support_rows": calibration.get("tensor_slot_support", {}).get(slot_id),
        }
    return out


def per_pair_contribution_metrics(test: list[dict[str, Any]], calibration: dict[str, Any], ece_bins: int) -> dict[str, Any]:
    grouped: dict[str, list[tuple[int, float, bool, float]]] = defaultdict(list)
    for row in test:
        for item in row.get("calibrated_pairs") or []:
            if item.get("present") is True:
                grouped[str(item["pair_id"])].append((label(row), float(item["probability_known_bad"]), bool(item["guard_rejects"]), float(item["surface_risk"])))
    out = {}
    for key in sorted(calibration["per_pair_by_pair"]):
        pair_id = "|".join(key)
        values = grouped.get(pair_id, [])
        if len(values) < 2:
            out[pair_id] = {
                "support_rows": len(values),
                "measured": False,
                "insufficient_support_reason": "TOO_FEW_TEST_ROWS_FOR_PAIR_METRIC",
                "metrics": None,
                "disagreement_violation_rows": sum(1 for _label, _prob, rejected, _risk in values if rejected),
                "mean_tensor_surface_risk": finite_float(float(np.mean([item[3] for item in values]))) if values else None,
            }
            continue
        out[pair_id] = {
            "support_rows": len(values),
            "measured": True,
            "metrics": metric_block([item[0] for item in values], [item[1] for item in values], ece_bins),
            "disagreement_violation_rows": sum(1 for _label, _prob, rejected, _risk in values if rejected),
            "mean_tensor_surface_risk": finite_float(float(np.mean([item[3] for item in values]))),
        }
    return out


def measurement_gaps(rows: list[dict[str, Any]]) -> dict[str, Any]:
    q4_present = sum(1 for row in rows if row.get("labels", {}).get("q4_labels", {}).get("present") is True)
    q5_present = sum(1 for row in rows if row.get("labels", {}).get("q5_reality_impact", {}).get("present") is True)
    return {
        "ood_recall": {"measured": False, "reason": "#389 provides a guard policy but harvested OOD labels are still absent", "blocked_by": ["#276", "#129"]},
        "q4_calibration": {"measured": q4_present > 0, "present_rows": q4_present, "reason": None if q4_present else "no Q4 labels exist in the #396 training rows", "blocked_by": ["#266", "#267", "#268", "#269", "#127"]},
        "q5_replay_lift": {"measured": q5_present > 0, "present_rows": q5_present, "reason": None if q5_present else "no Q5 replay labels exist in the #396 training rows", "blocked_by": ["#264", "#128"]},
    }


def rank_and_verdicts(ablations: list[dict[str, Any]], baselines: list[dict[str, Any]], historical: float) -> dict[str, Any]:
    best_local_brier = min(float(item["test_metrics"]["brier"]) for item in baselines)
    ranked = sorted(ablations, key=lambda item: float(item["test_metrics"]["brier"]))
    full = next(item for item in ablations if item["name"] == "calibrated_full_constellation")
    full_metrics = full["test_metrics"]
    verdicts = []
    for item in ranked:
        metrics = item["test_metrics"]
        corr = metrics.get("prediction_oracle_correlation")
        delta_brier = finite_float(float(metrics["brier"]) - best_local_brier)
        delta_corr = None if corr is None else finite_float(float(corr) - historical)
        status = "demote"
        if not item["ship_gate_eligible"]:
            status = "comparison_only"
        elif delta_brier <= 0.0 and delta_corr is not None and delta_corr > 0.0 and item["anti_compensation"]["passes"] is True:
            status = "candidate_lift"
        elif abs(delta_brier) <= 0.01:
            status = "neutral"
        verdicts.append({"name": item["name"], "status": status, "delta_brier_vs_best_local": delta_brier, "delta_correlation_vs_historical": delta_corr})

    component_deltas = {}
    disabled_component_by_name = {
        "disable_slot_calibration": "slot_calibration",
        "disable_pair_calibration": "pair_calibration",
        "disable_contradiction_consensus": "contradiction_consensus",
        "disable_temporal_trajectory": "temporal_trajectory",
        "disable_anti_compensation": "anti_compensation",
    }
    for item in ablations:
        component = disabled_component_by_name.get(str(item["name"]))
        if component is None:
            continue
        item_corr = item["test_metrics"].get("prediction_oracle_correlation")
        full_corr = full_metrics.get("prediction_oracle_correlation")
        component_deltas[component] = {
            "disabled_variant": item["name"],
            "brier_delta_when_disabled_vs_full": finite_float(float(item["test_metrics"]["brier"]) - float(full_metrics["brier"])),
            "correlation_delta_when_disabled_vs_full": None if item_corr is None or full_corr is None else finite_float(float(item_corr) - float(full_corr)),
            "positive_if_brier_delta_when_disabled_is_gt_zero": True,
        }

    recommendation = next_predictor_recommendation(full, best_local_brier, historical, component_deltas)
    return {
        "best_local_baseline_brier": finite_float(best_local_brier),
        "ranked_ablation_names": [item["name"] for item in ranked],
        "feature_verdicts": verdicts,
        "component_deltas": component_deltas,
        "next_predictor_change_recommendation": recommendation,
    }


def next_predictor_recommendation(full: dict[str, Any], best_local_brier: float, historical: float, component_deltas: dict[str, Any]) -> dict[str, Any]:
    metrics = full["test_metrics"]
    corr = metrics.get("prediction_oracle_correlation")
    full_beats_local = float(metrics["brier"]) <= best_local_brier
    full_beats_historical = corr is not None and float(corr) > historical
    if not full_beats_local or not full_beats_historical or full["anti_compensation"]["passes"] is not True:
        return {
            "decision": "no_promotion",
            "reason": "calibrated_full_constellation did not beat both the local baseline ladder and configured correlation baseline",
            "candidate_components": [],
            "full_constellation_brier": metrics["brier"],
            "full_constellation_correlation": corr,
            "best_local_baseline_brier": best_local_brier,
            "historical_baseline_correlation": historical,
        }
    useful = [
        component
        for component, delta in sorted(component_deltas.items())
        if component != "anti_compensation" and float(delta["brier_delta_when_disabled_vs_full"]) > 0.0
    ]
    if not useful:
        return {"decision": "no_promotion", "reason": "full model lifted but no individual calibrated component showed positive ablation contribution", "candidate_components": []}
    return {
        "decision": "promote_calibrated_components",
        "reason": "full calibrated constellation beat local and historical baselines",
        "candidate_components": useful,
        "mandatory_guard": "anti_compensation",
    }


def built_in_edge_cases(calibration_state: dict[str, Any]) -> list[dict[str, Any]]:
    cases = [
        edge_case_missing_root(),
        edge_case_empty_metrics(),
        edge_case_flat_promotion_refused(),
        edge_case_nonfinite_feature(),
        edge_case_missing_calibration_root(),
        edge_case_stale_calibration_rejects(calibration_state),
        edge_case_not_global_ready_rejects(calibration_state),
        edge_case_one_bad_slot_rejects_even_when_aggregate_passes(calibration_state),
    ]
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


def edge_case_missing_calibration_root() -> dict[str, Any]:
    try:
        load_calibration_state(Path("/tmp/contextgraph-definitely-missing-per-slot-calibration"), None, {"report": {"passes": True, "constellation_policy": {"flat_vector_concat_used": False}, "freshness": {"stale_after_utc": "2999-01-01T00:00:00+00:00"}}})
        return {"name": "missing_calibration_root_fails_closed", "passes": False}
    except ConstellationAblationSuiteError as exc:
        return {"name": "missing_calibration_root_fails_closed", "passes": "MEJEPA_CONSTELLATION_ABLATION_CALIBRATION_ROOT_MISSING" in str(exc), "error": str(exc)}


def edge_case_stale_calibration_rejects(state: dict[str, Any]) -> dict[str, Any]:
    stale = copy.deepcopy(state)
    stale["policy"]["stale_after_utc"] = "2000-01-01T00:00:00+00:00"
    try:
        validate_calibration_state(stale, datetime.now(timezone.utc))
        return {"name": "stale_calibration_rejects", "passes": False}
    except ConstellationAblationSuiteError as exc:
        return {"name": "stale_calibration_rejects", "passes": "CALIBRATION_POLICY_STALE" in str(exc), "error": str(exc)}


def edge_case_not_global_ready_rejects(state: dict[str, Any]) -> dict[str, Any]:
    not_ready = copy.deepcopy(state)
    not_ready["policy"]["global_slot_calibration_ready"] = False
    try:
        validate_calibration_state(not_ready, datetime.now(timezone.utc))
        return {"name": "not_globally_ready_calibration_rejects", "passes": False}
    except ConstellationAblationSuiteError as exc:
        return {"name": "not_globally_ready_calibration_rejects", "passes": "CALIBRATION_NOT_GLOBAL_READY" in str(exc), "error": str(exc)}


def evaluate_calibration_guard(observation: dict[str, float], calibrations: list[dict[str, Any]], now: datetime) -> dict[str, Any]:
    violations = []
    normalized = []
    for row in sorted(calibrations, key=lambda item: str(item["embedder_id"])):
        slot_id = str(row["embedder_id"])
        if parse_utc(str(row["stale_after_utc"])) <= now:
            violations.append({"slot_id": slot_id, "reason": "CALIBRATION_STALE"})
            continue
        if slot_id not in observation:
            violations.append({"slot_id": slot_id, "reason": "MISSING_SLOT_OBSERVATION"})
            continue
        score = float(observation[slot_id])
        if not math.isfinite(score):
            violations.append({"slot_id": slot_id, "reason": "NONFINITE_SLOT_SCORE"})
            continue
        tau = float(row["thresholds"]["ood_tau"])
        normalized.append(score / max(EPS, tau))
        if score > tau:
            violations.append({"slot_id": slot_id, "reason": "SLOT_OOD_TAU_EXCEEDED", "score": finite_float(score), "tau": finite_float(tau)})
    aggregate_mean = finite_float(float(np.mean(normalized))) if normalized else 1.0
    return {"approved": not violations, "violations": violations, "aggregate_mean_normalized_score": aggregate_mean, "aggregate_mean_would_pass": aggregate_mean <= 1.0}


def edge_case_one_bad_slot_rejects_even_when_aggregate_passes(state: dict[str, Any]) -> dict[str, Any]:
    rows = [row for row in state["per_slot_rows"] if row.get("valid") is True]
    nominal = {str(row["embedder_id"]): float(row["reference_good_score"]) for row in rows}
    bad = dict(nominal)
    bad_slot = str(rows[0]["embedder_id"])
    bad[bad_slot] = float(rows[0]["thresholds"]["ood_tau"]) * 1.25
    decision = evaluate_calibration_guard(bad, rows, datetime.now(timezone.utc))
    return {
        "name": "one_bad_slot_rejects_even_when_aggregate_passes",
        "passes": decision["approved"] is False and decision["aggregate_mean_would_pass"] is True,
        "bad_slot": bad_slot,
        "guard_decision": decision,
    }


def command_python_constellation_ablation_suite(args: argparse.Namespace) -> int:
    started = utc_now()
    output_root = args.output_root or (args.fsv_root / "artifacts")
    before = {
        "artifact_root": tree_state(args.artifact_root),
        "tensor_root": tree_state(args.tensor_root),
        "calibration_root": tree_state(args.calibration_root),
        "calibration_fsv": tree_state(args.calibration_fsv) if args.calibration_fsv is not None else None,
        "output_root": tree_state(output_root),
        "fsv_root": tree_state(args.fsv_root),
    }
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    rows, slot_vectors, pair_observations, certificate, artifact_hashes = load_artifacts(args.artifact_root)
    tensor_context = load_tensor_context(args.tensor_root)
    calibration_state = load_calibration_state(args.calibration_root, args.calibration_fsv, tensor_context)
    calibration_state["tensor_slot_support"] = tensor_context["slot_support"]
    attach_flat_baseline_features(rows, slot_vectors)
    attach_calibrated_features(rows, pair_observations, calibration_state, tensor_context)
    dataset = validate_rows(rows, args.min_train_rows, args.min_test_rows)
    train, test = split_rows(rows)
    baselines = [
        evaluate_rate_baseline("global_label_rate", train, test, lambda _row: "global", args.ece_bins),
        evaluate_rate_baseline("per_cell_label_rate", train, test, lambda row: str(row.get("cell_key")), args.ece_bins),
        evaluate_rate_baseline("per_temporal_window_label_rate", train, test, temporal_key, args.ece_bins),
    ]
    ablations = [evaluate_spec(spec, train, test, args.ece_bins) for spec in ablation_specs()]
    per_slot_metrics = per_slot_contribution_metrics(test, calibration_state, args.ece_bins)
    per_pair_metrics = per_pair_contribution_metrics(test, calibration_state, args.ece_bins)
    summary = rank_and_verdicts(ablations, baselines, float(args.historical_baseline_correlation))
    edge_cases = built_in_edge_cases(calibration_state)
    require(all(case.get("passes") is True for case in edge_cases), "MEJEPA_CONSTELLATION_ABLATION_EDGE_CASE_FAILED")
    require(not any(item["flat_vector_concat_used"] and item["ship_gate_eligible"] for item in ablations), "MEJEPA_CONSTELLATION_ABLATION_FLAT_BASELINE_ELIGIBLE")

    report = build_report(
        args,
        started,
        dataset,
        certificate,
        artifact_hashes,
        tensor_context,
        calibration_state,
        baselines,
        ablations,
        per_slot_metrics,
        per_pair_metrics,
        summary,
        edge_cases,
    )
    metrics_path = output_root / "constellation_ablation_with_calibration_metrics.jsonl"
    report_path = output_root / "constellation_ablation_with_calibration_report.json"
    write_jsonl(metrics_path, baselines + ablations)
    dump_json(report_path, report)
    fsv = build_fsv(args, before, output_root, report_path, metrics_path, report, len(baselines), len(ablations))
    fsv_path = args.fsv_root / f"{ARTIFACT_KIND}_fsv.json"
    dump_json(fsv_path, fsv)
    final = read_json(fsv_path)
    require(final.get("passes") is True, "MEJEPA_CONSTELLATION_ABLATION_FSV_FAILED", str(fsv_path))
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_report(
    args: argparse.Namespace,
    started: str,
    dataset: dict[str, Any],
    certificate: dict[str, Any],
    artifact_hashes: dict[str, str],
    tensor_context: dict[str, Any],
    calibration_state: dict[str, Any],
    baselines: list[dict[str, Any]],
    ablations: list[dict[str, Any]],
    per_slot_metrics: dict[str, Any],
    per_pair_metrics: dict[str, Any],
    summary: dict[str, Any],
    edge_cases: list[dict[str, Any]],
) -> dict[str, Any]:
    ship_gate_eligible = [item for item in ablations if item.get("ship_gate_eligible") is True]
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": True,
        "source": {
            "artifact_root": str(args.artifact_root),
            "tensor_root": str(args.tensor_root),
            "calibration_root": str(args.calibration_root),
            "calibration_fsv": str(args.calibration_fsv) if args.calibration_fsv is not None else None,
            "upstream_certificate_sha256": artifact_hashes["training_certificate"],
            "tensor_paths": tensor_context["paths"],
            "calibration_paths": calibration_state["paths"],
        },
        "source_hashes": {"artifacts": artifact_hashes, "tensors": tensor_context["hashes"], "calibration": calibration_state["hashes"]},
        "policy": {
            "human_intent_inference_used": False,
            "flat_vector_baseline_ship_gate_eligible": False,
            "flat_vector_concat_used_for_ship_gate_eligible_paths": not all(item.get("flat_vector_concat_used") is False for item in ship_gate_eligible),
            "slot_identity_preserved": True,
            "slot_calibration_required": True,
            "pair_calibration_required": True,
            "strict_anti_compensation_required_for_ship_gate_eligible_paths": True,
            "aggregate_score_never_overrides_slot_violation": calibration_state["policy"].get("aggregate_score_never_overrides_slot_violation"),
        },
        "dataset_summary": dataset,
        "upstream_objective_loss": certificate.get("objective_loss"),
        "tensor_summary": tensor_context["report"].get("summary"),
        "calibration_summary": calibration_state["report"].get("calibration_summary"),
        "baselines": baselines,
        "ablations": ablations,
        "per_slot_contribution_metrics": per_slot_metrics,
        "per_pair_contribution_metrics": per_pair_metrics,
        "summary": summary,
        "measurement_gaps": measurement_gaps_from_report_rows(args.artifact_root),
        "edge_cases": edge_cases,
    }


def measurement_gaps_from_report_rows(artifact_root: Path) -> dict[str, Any]:
    rows = [row for _line_no, row in iter_jsonl(artifact_root / "constellation_training_rows.jsonl")]
    return measurement_gaps(rows)


def build_fsv(args: argparse.Namespace, before: dict[str, Any], output_root: Path, report_path: Path, metrics_path: Path, report: dict[str, Any], baseline_count: int, ablation_count: int) -> dict[str, Any]:
    readback_report = read_json(report_path)
    metrics_count = sum(1 for _ in iter_jsonl(metrics_path))
    hashes = {"report": sha256_file(report_path), "metrics": sha256_file(metrics_path)}
    ship_eligible = [item for item in readback_report.get("ablations", []) if item.get("ship_gate_eligible") is True]
    flat_used_for_ship_eligible = any(item.get("flat_vector_concat_used") is True for item in ship_eligible)
    edge_cases = readback_report.get("edge_cases", [])
    independent = {
        "metrics_rows": metrics_count,
        "expected_metrics_rows": baseline_count + ablation_count,
        "report_passes": readback_report.get("passes"),
        "flat_vector_concat_used_for_ship_gate_eligible_paths": flat_used_for_ship_eligible,
        "calibration_global_slot_ready": readback_report.get("calibration_summary", {}).get("global_calibrated_slots") == readback_report.get("calibration_summary", {}).get("global_slot_rows"),
        "per_slot_contribution_metric_count": len(readback_report.get("per_slot_contribution_metrics", {})),
        "per_pair_contribution_metric_count": len(readback_report.get("per_pair_contribution_metrics", {})),
        "next_predictor_change_recommendation": readback_report.get("summary", {}).get("next_predictor_change_recommendation"),
        "stable_hashes": hashes,
    }
    external_edges = {"edge_cases": edge_cases, "all_edge_cases_pass": all(case.get("passes") is True for case in edge_cases)}
    dump_json(args.fsv_root / "independent_readback.json", independent)
    dump_json(args.fsv_root / "external_edge_case_readback.json", external_edges)
    anti_case = next((case for case in edge_cases if case.get("name") == "one_bad_slot_rejects_even_when_aggregate_passes"), {})
    passes = (
        readback_report.get("passes") is True
        and metrics_count == baseline_count + ablation_count
        and ablation_count >= 6
        and flat_used_for_ship_eligible is False
        and readback_report.get("policy", {}).get("flat_vector_baseline_ship_gate_eligible") is False
        and readback_report.get("policy", {}).get("aggregate_score_never_overrides_slot_violation") is True
        and external_edges["all_edge_cases_pass"] is True
        and anti_case.get("passes") is True
        and independent["per_slot_contribution_metric_count"] >= 12
        and independent["per_pair_contribution_metric_count"] >= 60
        and readback_report.get("summary", {}).get("next_predictor_change_recommendation", {}).get("decision") in {"no_promotion", "promote_calibrated_components"}
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "input_artifact_root": str(args.artifact_root),
            "tensor_root": str(args.tensor_root),
            "calibration_root": str(args.calibration_root),
            "calibration_fsv": str(args.calibration_fsv) if args.calibration_fsv is not None else None,
            "output_root": str(output_root),
            "report": str(report_path),
            "metrics": str(metrics_path),
            "independent_readback": str(args.fsv_root / "independent_readback.json"),
            "external_edge_case_readback": str(args.fsv_root / "external_edge_case_readback.json"),
        },
        "before_state": before,
        "after_state": {"output_root": tree_state(output_root), "fsv_root": tree_state(args.fsv_root)},
        "readback": {
            "metrics_rows": metrics_count,
            "expected_metrics_rows": baseline_count + ablation_count,
            "report_passes": readback_report.get("passes"),
            "stable_hashes": hashes,
            "report_summary_hash": sha256_text(readback_report.get("summary")),
        },
        "independent_readback": independent,
        "external_edge_case_readback": external_edges,
        "summary": readback_report.get("summary"),
        "measurement_gaps": readback_report.get("measurement_gaps"),
        "edge_cases": edge_cases,
    }


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--artifact-root", type=Path, default=DEFAULT_ARTIFACT_ROOT)
    parser.add_argument("--tensor-root", type=Path, default=DEFAULT_TENSOR_ROOT)
    parser.add_argument("--calibration-root", type=Path, default=DEFAULT_CALIBRATION_ROOT)
    parser.add_argument("--calibration-fsv", type=Path, default=DEFAULT_CALIBRATION_FSV)
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
