"""Shared builders for TASK-PY-G-088 constellation tensor artifacts."""

from __future__ import annotations

import math
from collections import Counter, defaultdict
from itertools import combinations
from typing import Any

import numpy as np

from .core import SwebenchOpsError
from .python_constellation_training_objective import normalize_category
from .python_real_corpus_constellation_smoke import (
    EPS,
    SampleRecord,
    normalized_mutual_info,
    pearson,
    scalar_array,
    sha256_text,
)
from .smoke_intelligence_matrices import sha256_bytes


SCALAR_FEATURES = ("norm_l2", "std", "abs_mean", "entropy_proxy", "zero_fraction")


def fail(code: str, message: str, details: dict[str, Any] | None = None) -> None:
    raise SwebenchOpsError(
        code,
        message,
        "Fix the tensor materializer input, source substrate, or freshness metadata before re-running #388.",
        details,
    )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        fail(code, message, details)


def finite_float(value: float | np.floating[Any], *, code: str = "MEJEPA_CONSTELLATION_TENSORS_NONFINITE_SCALAR") -> float:
    out = float(value)
    require(math.isfinite(out), code, f"non-finite tensor scalar: {value!r}")
    return out


def parse_embedder_set(raw: str) -> tuple[str, ...]:
    embedders = tuple(part.strip() for part in raw.split(",") if part.strip())
    require(bool(embedders), "MEJEPA_CONSTELLATION_TENSORS_EMPTY_EMBEDDER_SET", "expected at least one embedder id")
    require(len(set(embedders)) == len(embedders), "MEJEPA_CONSTELLATION_TENSORS_DUPLICATE_EMBEDDER", f"duplicate embedder in {raw!r}")
    return embedders


def cell_key(record: SampleRecord) -> str:
    return f"python:{normalize_category(record.mutation_category)}"


def oracle_key(record: SampleRecord) -> tuple[str, str]:
    return (record.task_instance_id, normalize_category(record.mutation_category))


def group_records(
    records: dict[str, dict[str, SampleRecord]],
    oracle_index: dict[tuple[str, str], dict[str, Any]],
    shard_order: dict[str, int],
) -> tuple[dict[tuple[str, str], dict[str, dict[str, SampleRecord]]], dict[str, Any]]:
    grouped: dict[tuple[str, str], dict[str, dict[str, SampleRecord]]] = defaultdict(lambda: defaultdict(dict))
    label_counts = Counter()
    cell_counts = Counter()
    oracle_join_missing = 0
    for embedder_id, rows in records.items():
        for row_key, record in rows.items():
            oracle_entry = oracle_index.get(oracle_key(record))
            if oracle_entry is None:
                oracle_join_missing += 1
                continue
            window_id = f"aiwonder-forward-shard:{record.shard_id}"
            grouped[(cell_key(record), window_id)][embedder_id][row_key] = record
            label_counts["pass" if bool(oracle_entry.get("oracle_all_passed")) else "fail_or_error"] += 1
            cell_counts[cell_key(record)] += 1
    require(oracle_join_missing == 0, "MEJEPA_CONSTELLATION_TENSORS_ORACLE_JOIN_MISSING", f"missing oracle joins: {oracle_join_missing}")
    return grouped, {
        "cell_window_count": len(grouped),
        "label_counts": dict(sorted(label_counts.items())),
        "cell_record_counts": dict(sorted(cell_counts.items())),
        "window_indices": {f"aiwonder-forward-shard:{shard_id}": idx for shard_id, idx in shard_order.items()},
    }


def vector_hash(values: np.ndarray) -> str:
    return sha256_bytes(values.astype("<f4", copy=False).tobytes())


def scalar_feature_stats(items: list[SampleRecord]) -> dict[str, dict[str, float]]:
    out: dict[str, dict[str, float]] = {}
    for feature in SCALAR_FEATURES:
        values = np.asarray([row.scalar_features[feature] for row in items], dtype=np.float64)
        out[feature] = {
            "mean": finite_float(float(values.mean())),
            "std": finite_float(float(values.std())),
            "min": finite_float(float(values.min())),
            "max": finite_float(float(values.max())),
        }
    return out


def slot_distribution_rows(
    grouped: dict[tuple[str, str], dict[str, dict[str, SampleRecord]]],
    required_embedders: tuple[str, ...],
    args: Any,
    created_at_utc: str,
    corpus_id: str,
    run_id: str,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    missing_slots = Counter()
    valid_counts = Counter()
    for (cell, window_id), by_embedder in sorted(grouped.items()):
        shard_id = window_id.split(":", 1)[1]
        for embedder_id in required_embedders:
            items = list(by_embedder.get(embedder_id, {}).values())
            valid = len(items) >= args.min_slot_rows
            row: dict[str, Any] = {
                "artifact_kind": "python_constellation_slot_distribution_tensor",
                "schema_version": args.schema_version,
                "formula_version": args.formula_version,
                "tensor_key": sha256_text({"corpus_id": corpus_id, "run_id": run_id, "cell": cell, "window": window_id, "slot": embedder_id}),
                "created_at_utc": created_at_utc,
                "corpus_id": corpus_id,
                "run_id": run_id,
                "shard_id": shard_id,
                "window_id": window_id,
                "window_index": args.shard_order.get(shard_id, 0),
                "language": "python",
                "cell_key": cell,
                "mutation_category": cell.split(":", 1)[1],
                "slot_id": embedder_id,
                "embedder_id": embedder_id,
                "row_count": len(items),
                "valid": valid,
                "invalid_reason": None if valid else "TOO_FEW_SLOT_ROWS",
                "slot_identity_preserved": True,
                "flat_vector_concat_used": False,
            }
            if not items:
                missing_slots[embedder_id] += 1
                rows.append(row)
                continue
            dims = sorted({item.vector_len for item in items})
            row["dim"] = dims[0] if len(dims) == 1 else None
            row["dim_set"] = dims
            row["source_row_key_sample"] = [item.row_key for item in items[:5]]
            row["vector_sha256_sample"] = [item.vector_sha256 for item in items[:5]]
            if not valid:
                rows.append(row)
                continue
            require(len(dims) == 1, "MEJEPA_CONSTELLATION_TENSORS_SLOT_DIM_MISMATCH", f"{cell}:{window_id}:{embedder_id}:{dims}")
            matrix = np.stack([item.vector for item in items], axis=0).astype(np.float64, copy=False)
            centroid = matrix.mean(axis=0).astype("<f4")
            variance = matrix.var(axis=0).astype("<f4")
            distances = np.linalg.norm(matrix - centroid.astype(np.float64, copy=False), axis=1)
            row.update(
                {
                    "centroid_vector": [finite_float(value) for value in centroid.astype(float).tolist()],
                    "centroid_vector_sha256": vector_hash(centroid),
                    "centroid_preview": [finite_float(value) for value in centroid[:8].astype(float).tolist()],
                    "variance_vector": [finite_float(value) for value in variance.astype(float).tolist()],
                    "variance_vector_sha256": vector_hash(variance),
                    "variance_preview": [finite_float(value) for value in variance[:8].astype(float).tolist()],
                    "variance_mean": finite_float(float(variance.astype(np.float64).mean())),
                    "distance_mean": finite_float(float(distances.mean())),
                    "distance_max": finite_float(float(distances.max())),
                    "scalar_feature_stats": scalar_feature_stats(items),
                }
            )
            valid_counts[cell] += 1
            rows.append(row)
    return rows, {
        "slot_distribution_rows": len(rows),
        "valid_slot_distribution_rows": sum(1 for row in rows if row.get("valid") is True),
        "missing_slot_events": sum(missing_slots.values()),
        "missing_slots_by_embedder": dict(sorted(missing_slots.items())),
        "valid_slot_rows_by_cell": dict(sorted(valid_counts.items())),
    }


def pair_support_state(left_count: int, right_count: int, overlap: int, min_overlap: int) -> dict[str, Any]:
    valid = overlap >= min_overlap
    return {
        "valid": valid,
        "invalid_reason": None if valid else "TOO_FEW_OVERLAP_ROWS",
        "left_rows": left_count,
        "right_rows": right_count,
        "overlap_rows": overlap,
        "min_overlap": min_overlap,
    }


def centroid_cosine(left_items: list[SampleRecord], right_items: list[SampleRecord]) -> tuple[float | None, str | None]:
    left_matrix = np.stack([item.vector for item in left_items], axis=0).astype(np.float64, copy=False)
    right_matrix = np.stack([item.vector for item in right_items], axis=0).astype(np.float64, copy=False)
    if left_matrix.shape[1] != right_matrix.shape[1]:
        return None, "DIMENSION_MISMATCH"
    left_centroid = left_matrix.mean(axis=0)
    right_centroid = right_matrix.mean(axis=0)
    denom = float(np.linalg.norm(left_centroid) * np.linalg.norm(right_centroid))
    if denom <= EPS:
        return None, "DEGENERATE_ZERO_NORM"
    return finite_float(float(np.dot(left_centroid, right_centroid) / denom)), None


def pairwise_tensor_rows(
    grouped: dict[tuple[str, str], dict[str, dict[str, SampleRecord]]],
    required_embedders: tuple[str, ...],
    args: Any,
    created_at_utc: str,
    corpus_id: str,
    run_id: str,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    expected_pairs = len(required_embedders) * (len(required_embedders) - 1) // 2
    for (cell, window_id), by_embedder in sorted(grouped.items()):
        shard_id = window_id.split(":", 1)[1]
        for pair_index, (left_id, right_id) in enumerate(combinations(required_embedders, 2)):
            rows.append(pairwise_tensor_row(by_embedder, cell, window_id, shard_id, pair_index, left_id, right_id, expected_pairs, args, created_at_utc, corpus_id, run_id))
    full_supported_windows = mark_full_overlap_supported_windows(rows, expected_pairs)
    classify_pairwise_surfaces(rows, args.surface_quantile)
    return rows, {
        "pairwise_tensor_rows": len(rows),
        "valid_pairwise_tensor_rows": sum(1 for row in rows if row.get("valid") is True),
        "expected_pairs_per_full_cell_window": expected_pairs,
        "full_supported_cell_windows": full_supported_windows,
        "expected_valid_pairs_for_full_supported_windows": full_supported_windows * expected_pairs,
    }


def pairwise_tensor_row(
    by_embedder: dict[str, dict[str, SampleRecord]],
    cell: str,
    window_id: str,
    shard_id: str,
    pair_index: int,
    left_id: str,
    right_id: str,
    expected_pairs: int,
    args: Any,
    created_at_utc: str,
    corpus_id: str,
    run_id: str,
) -> dict[str, Any]:
    left_rows = by_embedder.get(left_id, {})
    right_rows = by_embedder.get(right_id, {})
    keys = sorted(set(left_rows) & set(right_rows))
    support = pair_support_state(len(left_rows), len(right_rows), len(keys), args.min_overlap)
    row: dict[str, Any] = {
        "artifact_kind": "python_constellation_pairwise_correlation_tensor",
        "schema_version": args.schema_version,
        "formula_version": args.formula_version,
        "tensor_key": sha256_text({"corpus_id": corpus_id, "run_id": run_id, "cell": cell, "window": window_id, "left": left_id, "right": right_id}),
        "created_at_utc": created_at_utc,
        "corpus_id": corpus_id,
        "run_id": run_id,
        "shard_id": shard_id,
        "window_id": window_id,
        "window_index": args.shard_order.get(shard_id, 0),
        "language": "python",
        "cell_key": cell,
        "mutation_category": cell.split(":", 1)[1],
        "pairwise_upper_triangle_index": pair_index,
        "expected_pairs_per_full_cell_window": expected_pairs,
        "left_slot_id": left_id,
        "right_slot_id": right_id,
        "left_embedder_id": left_id,
        "right_embedder_id": right_id,
        "full_cell_window_supported": False,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "raw_vector_comparison_policy": "slot-local vectors are summarized by per-slot distributions and cross-slot tensor metrics, never concatenated",
        **support,
    }
    if not support["valid"]:
        return row
    feature_metrics, redundancy_inputs, disagreement_inputs = pairwise_feature_metrics(left_rows, right_rows, keys, args.mi_bins)
    left_items = [left_rows[key] for key in keys]
    right_items = [right_rows[key] for key in keys]
    cos, cos_reason = centroid_cosine(left_items, right_items)
    if cos is not None:
        redundancy_inputs.append(max(0.0, cos))
    row.update(
        {
            "left_dim": left_items[0].vector_len if left_items else None,
            "right_dim": right_items[0].vector_len if right_items else None,
            "scalar_feature_correlation_tensor": {name: metric["pearson"] for name, metric in feature_metrics.items()},
            "scalar_feature_mi_tensor": {name: metric["quantile_nmi"] for name, metric in feature_metrics.items()},
            "scalar_feature_delta_tensor": {name: metric["mean_abs_delta"] for name, metric in feature_metrics.items()},
            "feature_metrics": feature_metrics,
            "same_dim_centroid_cosine": cos,
            "same_dim_centroid_cosine_validity": cos_reason,
            "redundancy_score": finite_float(float(np.mean(redundancy_inputs))) if redundancy_inputs else None,
            "disagreement_score": finite_float(float(np.mean(disagreement_inputs))) if disagreement_inputs else None,
            "relationship_hash": sha256_text({"left": left_id, "right": right_id, "cell": cell, "window": window_id, "metrics": feature_metrics}),
        }
    )
    return row


def mark_full_overlap_supported_windows(rows: list[dict[str, Any]], expected_pairs: int) -> int:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[(str(row.get("cell_key")), str(row.get("window_id")))].append(row)
    full_supported_windows = 0
    for group_rows in grouped.values():
        full_supported = len(group_rows) == expected_pairs and all(row.get("valid") is True for row in group_rows)
        full_supported_windows += 1 if full_supported else 0
        for row in group_rows:
            row["full_cell_window_supported"] = full_supported
    return full_supported_windows


def pairwise_feature_metrics(
    left_rows: dict[str, SampleRecord],
    right_rows: dict[str, SampleRecord],
    keys: list[str],
    mi_bins: int,
) -> tuple[dict[str, dict[str, Any]], list[float], list[float]]:
    feature_metrics: dict[str, dict[str, Any]] = {}
    redundancy_inputs: list[float] = []
    disagreement_inputs: list[float] = []
    for feature in SCALAR_FEATURES:
        x = scalar_array(left_rows, keys, feature)
        y = scalar_array(right_rows, keys, feature)
        corr, corr_reason = pearson(x, y)
        nmi, nmi_reason = normalized_mutual_info(x, y, mi_bins)
        delta = finite_float(float(np.mean(np.abs(x - y))))
        feature_metrics[feature] = {
            "pearson": corr,
            "pearson_validity": corr_reason,
            "abs_pearson": finite_float(abs(corr)) if corr is not None else None,
            "quantile_nmi": nmi,
            "quantile_nmi_validity": nmi_reason,
            "mean_abs_delta": delta,
        }
        if corr is not None:
            redundancy_inputs.append(abs(corr))
        if nmi is not None:
            redundancy_inputs.append(nmi)
        disagreement_inputs.append(delta)
    return feature_metrics, redundancy_inputs, disagreement_inputs


def percentile(values: list[float], q: float) -> float:
    require(bool(values), "MEJEPA_CONSTELLATION_TENSORS_EMPTY_PERCENTILE", "cannot classify surfaces without values")
    return finite_float(float(np.quantile(np.asarray(values, dtype=np.float64), q)))


def classify_pairwise_surfaces(rows: list[dict[str, Any]], surface_quantile: float) -> None:
    valid = [row for row in rows if row.get("valid") is True]
    if not valid:
        return
    redundancy_values = [float(row["redundancy_score"]) for row in valid if isinstance(row.get("redundancy_score"), (int, float))]
    disagreement_values = [float(row["disagreement_score"]) for row in valid if isinstance(row.get("disagreement_score"), (int, float))]
    high_redundancy = percentile(redundancy_values, surface_quantile) if redundancy_values else float("inf")
    high_disagreement = percentile(disagreement_values, surface_quantile) if disagreement_values else float("inf")
    low_redundancy = percentile(redundancy_values, 1.0 - surface_quantile) if redundancy_values else float("-inf")
    for row in rows:
        if row.get("valid") is not True:
            row["surface_class"] = "unsupported"
            continue
        redundancy = row.get("redundancy_score")
        disagreement = row.get("disagreement_score")
        if isinstance(disagreement, (int, float)) and disagreement >= high_disagreement and (
            not isinstance(redundancy, (int, float)) or redundancy <= high_redundancy
        ):
            row["surface_class"] = "contradictory"
        elif isinstance(redundancy, (int, float)) and redundancy >= high_redundancy:
            row["surface_class"] = "redundant"
        elif isinstance(redundancy, (int, float)) and redundancy <= low_redundancy:
            row["surface_class"] = "independent"
        else:
            row["surface_class"] = "mixed"


def surface_summary(rows: list[dict[str, Any]], top_k: int) -> dict[str, Any]:
    valid = [row for row in rows if row.get("valid") is True]
    by_class = Counter(str(row.get("surface_class") or "unknown") for row in rows)
    by_cell: dict[str, Counter[str]] = defaultdict(Counter)
    for row in rows:
        by_cell[str(row.get("cell_key") or "unknown")][str(row.get("surface_class") or "unknown")] += 1
    redundant = sorted(valid, key=lambda row: (row.get("redundancy_score") or -1.0, row.get("tensor_key") or ""), reverse=True)[:top_k]
    disagreement = sorted(valid, key=lambda row: (row.get("disagreement_score") or -1.0, row.get("tensor_key") or ""), reverse=True)[:top_k]
    contradictory = [row for row in disagreement if row.get("surface_class") == "contradictory"][:top_k]
    return {
        "surface_counts": dict(sorted(by_class.items())),
        "surface_counts_by_cell": {cell: dict(sorted(counter.items())) for cell, counter in sorted(by_cell.items())},
        "top_redundant_pairs": summarize_pairs(redundant),
        "top_disagreement_pairs": summarize_pairs(disagreement),
        "top_contradictory_pairs": summarize_pairs(contradictory),
    }


def summarize_pairs(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    keys = (
        "cell_key",
        "window_id",
        "left_embedder_id",
        "right_embedder_id",
        "overlap_rows",
        "redundancy_score",
        "disagreement_score",
        "surface_class",
        "relationship_hash",
    )
    return [{key: row.get(key) for key in keys} for row in rows]
