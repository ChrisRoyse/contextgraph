#!/usr/bin/env python3
"""INV-043 bounded causal-delta feasibility audit.

This runner consumes the slot-sliced INV-032 panel/oracle dataset and builds
known-good -> mutation delta rows. It is measurement-only: it decides whether a
full CDE training run is worth spending aiwonder time on, not whether any CDE
slot should be promoted.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
from collections import Counter, defaultdict
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TASK_ID = "INV-043"
ARTIFACT_KIND = "causal_delta_mutation_feasibility"
SCHEMA_VERSION = 1

DEFAULT_DATASET_ROOT = Path(
    "/zfs/archive/contextgraph/state/mi-audit/inv-032-v1/"
    "run-20260529T234900Z/full-candidate-map"
)
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/inv-043-cde-mutation-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/cde-mutation/inv-043-v1")
DEFAULT_BASELINE_CATEGORY = "known_good"
EPS = 1.0e-12


class CausalDeltaFeasibilityError(RuntimeError):
    pass


@dataclass(frozen=True)
class Dataset:
    X: Any
    y: Any
    rows: list[dict[str, Any]]
    manifest: dict[str, Any]
    feature_slices: dict[str, slice]
    dataset_npz: Path
    rows_path: Path


@dataclass(frozen=True)
class DeltaRow:
    row_index: int
    task_instance_id: str
    mutation_category: str
    baseline_row_index: int
    mutation_row_index: int
    oracle_all_passed: bool
    delta_sha256: str
    feature_vector: tuple[float, ...]
    active_slot_count: int
    active_slots: tuple[str, ...]
    slot_metrics: dict[str, dict[str, float]]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise CausalDeltaFeasibilityError(f"{code}{suffix}")


def require_aiwonder_path(path: Path, name: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/"),
        "CAUSAL_DELTA_NON_AIWONDER_PATH",
        {name: text},
    )
    require("/mnt/c/" not in text and "/mnt/d/" not in text, "CAUSAL_DELTA_MOUNTED_DRIVE_PATH", {name: text})


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise CausalDeltaFeasibilityError(f"CAUSAL_DELTA_WRITE_READBACK_MISMATCH: {path}")


def write_json(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    lines = []
    count = 0
    for row in rows:
        lines.append(canonical_json(row) + "\n")
        count += 1
    write_text_checked(path, "".join(lines))
    return count


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False}
    files = [item for item in path.rglob("*") if item.is_file()]
    return {
        "path": str(path),
        "exists": True,
        "file_count": len(files),
        "byte_count": sum(item.stat().st_size for item in files),
        "sample": [str(item.relative_to(path)) for item in sorted(files)[:24]],
    }


def median(values: list[float]) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    midpoint = len(ordered) // 2
    if len(ordered) % 2:
        return float(ordered[midpoint])
    return float((ordered[midpoint - 1] + ordered[midpoint]) / 2.0)


def percentile(values: list[float], q: float) -> float | None:
    if not values:
        return None
    require(0.0 <= q <= 1.0, "CAUSAL_DELTA_PERCENTILE_INVALID", q)
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, int(math.ceil(q * len(ordered)) - 1)))
    return float(ordered[index])


def load_dataset(dataset_root: Path, *, allow_non_aiwonder_roots: bool) -> Dataset:
    import numpy as np

    require_aiwonder_path(dataset_root, "dataset_root", allow_non_aiwonder_roots)
    manifest_path = dataset_root / "dataset_manifest.json"
    require(manifest_path.is_file(), "CAUSAL_DELTA_DATASET_MANIFEST_MISSING", manifest_path)
    manifest = read_json(manifest_path)
    require(manifest.get("passes") is True, "CAUSAL_DELTA_DATASET_NOT_PASSING", manifest_path)
    require(manifest.get("slot_identity_preserved") is True, "CAUSAL_DELTA_SLOT_IDENTITY_NOT_PRESERVED")
    require(manifest.get("flat_vector_concat_used") is False, "CAUSAL_DELTA_FLAT_VECTOR_CONCAT_USED")

    artifacts = manifest.get("artifacts") or {}
    dataset_npz = Path(str(artifacts.get("dataset_npz") or dataset_root / "panel_oracle_dataset.npz"))
    rows_path = Path(
        str(
            artifacts.get("state_dataset_rows_jsonl")
            or artifacts.get("dataset_rows_jsonl")
            or dataset_root / "dataset_rows.jsonl"
        )
    )
    require_aiwonder_path(dataset_npz, "dataset_npz", allow_non_aiwonder_roots)
    require_aiwonder_path(rows_path, "dataset_rows_jsonl", allow_non_aiwonder_roots)
    require(dataset_npz.is_file(), "CAUSAL_DELTA_DATASET_NPZ_MISSING", dataset_npz)
    require(rows_path.is_file(), "CAUSAL_DELTA_DATASET_ROWS_MISSING", rows_path)

    data = np.load(dataset_npz, allow_pickle=False)
    require(set(data.files) == {"X", "y"}, "CAUSAL_DELTA_DATASET_NPZ_KEYS", data.files)
    X = np.asarray(data["X"], dtype=np.float32)
    y = np.asarray(data["y"], dtype=np.int64)
    require(X.ndim == 2 and y.ndim == 1 and X.shape[0] == y.shape[0], "CAUSAL_DELTA_DATASET_SHAPE", (X.shape, y.shape))
    require(bool(np.all(np.isfinite(X))), "CAUSAL_DELTA_DATASET_NONFINITE_FEATURES")
    require(set(int(v) for v in y.tolist()).issubset({0, 1}), "CAUSAL_DELTA_DATASET_NON_BINARY_LABELS")

    rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    require(len(rows) == int(X.shape[0]), "CAUSAL_DELTA_ROW_COUNT_MISMATCH", (len(rows), int(X.shape[0])))
    for index, row in enumerate(rows):
        require(int(row.get("row_index")) == index, "CAUSAL_DELTA_ROW_INDEX_MISMATCH", (index, row.get("row_index")))

    raw_slices = manifest.get("feature_slices") or {}
    active = [str(slot).lower() for slot in (manifest.get("active_embedder_ids") or [])]
    require(active, "CAUSAL_DELTA_ACTIVE_EMBEDDERS_MISSING")
    feature_slices: dict[str, slice] = {}
    for slot in active:
        info = raw_slices.get(slot)
        require(isinstance(info, dict), "CAUSAL_DELTA_ACTIVE_SLICE_MISSING", slot)
        start = int(info.get("start"))
        stop = int(info.get("stop"))
        require(0 <= start < stop <= X.shape[1], "CAUSAL_DELTA_SLICE_OUT_OF_RANGE", {slot: (start, stop), "feature_dim": X.shape[1]})
        feature_slices[slot] = slice(start, stop)
    return Dataset(X=X, y=y, rows=rows, manifest=manifest, feature_slices=feature_slices, dataset_npz=dataset_npz, rows_path=rows_path)


def task_key(row: dict[str, Any], row_index: int) -> str:
    return str(row.get("task_instance_id") or row.get("task_id") or f"row:{row_index}")


def category_key(row: dict[str, Any]) -> str:
    return str(row.get("mutation_category") or row.get("category") or "")


def grouped_holdout_split(delta_rows: list[DeltaRow], holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    require(0.05 <= holdout_fraction <= 0.5, "CAUSAL_DELTA_HOLDOUT_FRACTION_INVALID", holdout_fraction)
    by_task: dict[str, list[int]] = defaultdict(list)
    for index, row in enumerate(delta_rows):
        by_task[row.task_instance_id].append(index)
    tasks = sorted(by_task)
    require(len(tasks) >= 4, "CAUSAL_DELTA_TOO_FEW_TASKS", len(tasks))
    ranked = sorted(tasks, key=lambda task: hashlib.sha256(f"{seed}:{task}".encode("utf-8")).hexdigest())
    holdout_task_count = max(1, int(round(len(ranked) * holdout_fraction)))
    holdout_tasks = set(ranked[:holdout_task_count])
    train_idx: list[int] = []
    holdout_idx: list[int] = []
    for task in tasks:
        target = holdout_idx if task in holdout_tasks else train_idx
        target.extend(by_task[task])
    require(train_idx and holdout_idx, "CAUSAL_DELTA_SPLIT_EMPTY")
    return sorted(train_idx), sorted(holdout_idx)


def build_delta_rows(
    dataset: Dataset,
    *,
    baseline_category: str,
    delta_epsilon: float,
    active_slot_threshold_fraction: float,
) -> list[DeltaRow]:
    import numpy as np

    require(delta_epsilon >= 0.0, "CAUSAL_DELTA_EPSILON_INVALID", delta_epsilon)
    require(0.0 < active_slot_threshold_fraction <= 1.0, "CAUSAL_DELTA_ACTIVE_SLOT_THRESHOLD_INVALID", active_slot_threshold_fraction)
    baseline_by_task: dict[str, int] = {}
    for index, row in enumerate(dataset.rows):
        if category_key(row) != baseline_category:
            continue
        key = task_key(row, index)
        require(key not in baseline_by_task, "CAUSAL_DELTA_DUPLICATE_BASELINE_ROW", key)
        baseline_by_task[key] = index
    require(baseline_by_task, "CAUSAL_DELTA_BASELINE_ROWS_MISSING", baseline_category)

    deltas: list[DeltaRow] = []
    for index, row in enumerate(dataset.rows):
        category = category_key(row)
        if category == baseline_category:
            continue
        require(category, "CAUSAL_DELTA_MUTATION_CATEGORY_MISSING", index)
        key = task_key(row, index)
        require(key in baseline_by_task, "CAUSAL_DELTA_BASELINE_ROW_MISSING_FOR_TASK", {"task": key, "category": category})
        baseline_index = baseline_by_task[key]
        delta = np.asarray(dataset.X[index] - dataset.X[baseline_index], dtype=np.float32)
        max_l2 = 0.0
        slot_metrics: dict[str, dict[str, float]] = {}
        feature_values: list[float] = []
        for slot in sorted(dataset.feature_slices):
            sl = dataset.feature_slices[slot]
            block = np.asarray(delta[sl.start : sl.stop], dtype=np.float64)
            abs_block = np.abs(block)
            l2 = float(np.sqrt(np.square(block).sum()))
            mean_abs = float(abs_block.mean()) if abs_block.size else 0.0
            max_abs = float(abs_block.max()) if abs_block.size else 0.0
            nonzero_fraction = float((abs_block > delta_epsilon).sum() / abs_block.size) if abs_block.size else 0.0
            slot_metrics[slot] = {
                "l2_delta": l2,
                "mean_abs_delta": mean_abs,
                "max_abs_delta": max_abs,
                "nonzero_fraction": nonzero_fraction,
            }
            feature_values.extend([math.log1p(l2), mean_abs, nonzero_fraction])
            max_l2 = max(max_l2, l2)
        threshold = max_l2 * active_slot_threshold_fraction
        active_slots = tuple(
            slot.upper()
            for slot in sorted(dataset.feature_slices)
            if max_l2 > 0.0 and slot_metrics[slot]["l2_delta"] >= threshold and slot_metrics[slot]["l2_delta"] > 0.0
        )
        deltas.append(
            DeltaRow(
                row_index=len(deltas),
                task_instance_id=key,
                mutation_category=category,
                baseline_row_index=baseline_index,
                mutation_row_index=index,
                oracle_all_passed=bool(int(dataset.y[index])),
                delta_sha256=sha256_bytes(delta.astype("<f4", copy=False).tobytes()),
                feature_vector=tuple(float(v) for v in feature_values),
                active_slot_count=len(active_slots),
                active_slots=active_slots,
                slot_metrics=slot_metrics,
            )
        )
    require(deltas, "CAUSAL_DELTA_NO_MUTATION_ROWS")
    return deltas


def standardize(train: Any, holdout: Any) -> tuple[Any, Any, dict[str, Any]]:
    import numpy as np

    train_arr = np.asarray(train, dtype=np.float64)
    holdout_arr = np.asarray(holdout, dtype=np.float64)
    mean = train_arr.mean(axis=0)
    std = train_arr.std(axis=0)
    keep = std > 1.0e-10
    require(bool(np.any(keep)), "CAUSAL_DELTA_FEATURES_CONSTANT")
    train_z = (train_arr[:, keep] - mean[keep]) / std[keep]
    holdout_z = (holdout_arr[:, keep] - mean[keep]) / std[keep]
    return train_z, holdout_z, {"input_dim": int(train_arr.shape[1]), "nonconstant_dim": int(train_z.shape[1])}


def fit_ridge(Z_train: Any, y_train: Any, ridge_alpha: float) -> tuple[Any, float]:
    import numpy as np

    require(ridge_alpha > 0.0 and math.isfinite(ridge_alpha), "CAUSAL_DELTA_RIDGE_ALPHA_INVALID", ridge_alpha)
    Z = np.asarray(Z_train, dtype=np.float64)
    y = np.asarray(y_train, dtype=np.float64)
    ones = np.ones((Z.shape[0], 1), dtype=np.float64)
    design = np.concatenate([ones, Z], axis=1)
    penalty = np.eye(design.shape[1], dtype=np.float64) * ridge_alpha
    penalty[0, 0] = 0.0
    try:
        coef = np.linalg.solve(design.T @ design + penalty, design.T @ y)
    except np.linalg.LinAlgError:
        coef = np.linalg.pinv(design.T @ design + penalty) @ design.T @ y
    return coef[1:], float(coef[0])


def predict_ridge(Z: Any, coefficients: Any, intercept: float) -> Any:
    import numpy as np

    return np.asarray(Z, dtype=np.float64) @ np.asarray(coefficients, dtype=np.float64) + float(intercept)


def pearson_corr(y_true: Any, y_score: Any) -> float | None:
    import numpy as np

    y = np.asarray(y_true, dtype=np.float64)
    s = np.asarray(y_score, dtype=np.float64)
    if y.size < 3:
        return None
    yc = y - y.mean()
    sc = s - s.mean()
    denom = math.sqrt(float(np.dot(yc, yc) * np.dot(sc, sc)))
    if denom <= EPS:
        return None
    value = float(np.dot(yc, sc) / denom)
    return value if math.isfinite(value) else None


def auc_score(y_true: Any, y_score: Any) -> float | None:
    import numpy as np

    y = np.asarray(y_true, dtype=np.int64)
    scores = np.asarray(y_score, dtype=np.float64)
    positives = scores[y == 1]
    negatives = scores[y == 0]
    if positives.size == 0 or negatives.size == 0:
        return None
    order = np.argsort(scores, kind="mergesort")
    ranks = np.empty(scores.size, dtype=np.float64)
    sorted_scores = scores[order]
    start = 0
    while start < scores.size:
        stop = start + 1
        while stop < scores.size and sorted_scores[stop] == sorted_scores[start]:
            stop += 1
        avg_rank = (start + 1 + stop) / 2.0
        ranks[order[start:stop]] = avg_rank
        start = stop
    pos_rank_sum = float(ranks[y == 1].sum())
    return float((pos_rank_sum - positives.size * (positives.size + 1) / 2.0) / (positives.size * negatives.size))


def brier_score(y_true: Any, y_score: Any) -> float | None:
    import numpy as np

    y = np.asarray(y_true, dtype=np.float64)
    if y.size == 0:
        return None
    clipped = np.clip(np.asarray(y_score, dtype=np.float64), 0.0, 1.0)
    return float(np.mean(np.square(clipped - y)))


def fit_centroids(Z_train: Any, categories: list[str]) -> dict[str, Any]:
    import numpy as np

    by_category: dict[str, list[int]] = defaultdict(list)
    for index, category in enumerate(categories):
        by_category[category].append(index)
    require(len(by_category) >= 2, "CAUSAL_DELTA_TOO_FEW_MUTATION_CATEGORIES", sorted(by_category))
    centroids = {}
    for category, indexes in by_category.items():
        centroids[category] = np.asarray(Z_train[indexes], dtype=np.float64).mean(axis=0)
    return centroids


def centroid_scores(Z: Any, centroids: dict[str, Any]) -> list[dict[str, float]]:
    import numpy as np

    rows: list[dict[str, float]] = []
    ordered = sorted(centroids)
    for vector in np.asarray(Z, dtype=np.float64):
        scores = {}
        for category in ordered:
            distance = float(np.linalg.norm(vector - centroids[category]))
            scores[category] = -distance
        rows.append(scores)
    return rows


def evaluate_category_separability(delta_rows: list[DeltaRow], train_idx: list[int], holdout_idx: list[int], ridge_alpha: float) -> dict[str, Any]:
    import numpy as np

    features = np.asarray([row.feature_vector for row in delta_rows], dtype=np.float64)
    train_features = features[train_idx]
    holdout_features = features[holdout_idx]
    Z_train, Z_holdout, reduction = standardize(train_features, holdout_features)
    train_categories = [delta_rows[index].mutation_category for index in train_idx]
    holdout_categories = [delta_rows[index].mutation_category for index in holdout_idx]
    centroids = fit_centroids(Z_train, train_categories)
    scores = centroid_scores(Z_holdout, centroids)
    predictions = [max(row_scores.items(), key=lambda item: (item[1], item[0]))[0] for row_scores in scores]
    accuracy = sum(1 for expected, predicted in zip(holdout_categories, predictions, strict=True) if expected == predicted) / len(predictions)

    category_rows: list[dict[str, Any]] = []
    auc_values: list[float] = []
    for category in sorted(set(train_categories) | set(holdout_categories)):
        y_true = np.asarray([1 if value == category else 0 for value in holdout_categories], dtype=np.int64)
        y_score = np.asarray([row_scores.get(category, -1.0e9) for row_scores in scores], dtype=np.float64)
        auc = auc_score(y_true, y_score)
        if auc is not None:
            auc_values.append(float(auc))
        support = int(y_true.sum())
        row_count = int(y_true.size)
        category_rows.append(
            {
                "mutation_category": category,
                "holdout_rows": row_count,
                "positive_holdout_rows": support,
                "one_vs_rest_auc": auc,
                "valid_auc": auc is not None,
                "predicted_count": int(sum(1 for value in predictions if value == category)),
            }
        )

    labels = np.asarray([1 if delta_rows[index].oracle_all_passed else 0 for index in train_idx], dtype=np.int64)
    holdout_labels = np.asarray([1 if delta_rows[index].oracle_all_passed else 0 for index in holdout_idx], dtype=np.int64)
    oracle_assoc: dict[str, Any]
    if len(set(labels.tolist())) == 2:
        coefficients, intercept = fit_ridge(Z_train, labels, ridge_alpha)
        oracle_scores = predict_ridge(Z_holdout, coefficients, intercept)
        oracle_assoc = {
            "measured": True,
            "pearson": pearson_corr(holdout_labels, oracle_scores),
            "auc": auc_score(holdout_labels, oracle_scores),
            "brier": brier_score(holdout_labels, oracle_scores),
            "holdout_label_counts": dict(sorted(Counter(str(int(v)) for v in holdout_labels.tolist()).items())),
        }
    else:
        oracle_assoc = {
            "measured": False,
            "reason": "train_oracle_labels_degenerate",
            "holdout_label_counts": dict(sorted(Counter(str(int(v)) for v in holdout_labels.tolist()).items())),
        }

    return {
        "feature_reduction": reduction,
        "holdout_accuracy": float(accuracy),
        "macro_one_vs_rest_auc": float(sum(auc_values) / len(auc_values)) if auc_values else None,
        "valid_auc_category_count": len(auc_values),
        "category_rows": category_rows,
        "oracle_association": oracle_assoc,
    }


def summarize_sparsity(delta_rows: list[DeltaRow]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    active_counts = [float(row.active_slot_count) for row in delta_rows]
    summary = {
        "median_active_slot_count": median(active_counts),
        "p90_active_slot_count": percentile(active_counts, 0.90),
        "max_active_slot_count": max(active_counts) if active_counts else None,
        "row_count": len(delta_rows),
    }
    by_category: dict[str, list[DeltaRow]] = defaultdict(list)
    for row in delta_rows:
        by_category[row.mutation_category].append(row)
    category_rows: list[dict[str, Any]] = []
    for category, rows in sorted(by_category.items()):
        values = [float(row.active_slot_count) for row in rows]
        pass_count = sum(1 for row in rows if row.oracle_all_passed)
        category_rows.append(
            {
                "mutation_category": category,
                "delta_rows": len(rows),
                "oracle_pass_rows": pass_count,
                "oracle_fail_rows": len(rows) - pass_count,
                "median_active_slot_count": median(values),
                "p90_active_slot_count": percentile(values, 0.90),
                "max_active_slot_count": max(values) if values else None,
                "top_active_slot_sets": [
                    {"active_slots": list(slots), "count": count}
                    for slots, count in Counter(row.active_slots for row in rows).most_common(5)
                ],
            }
        )
    return summary, category_rows


def delta_row_to_json(row: DeltaRow, *, include_slot_metrics: bool) -> dict[str, Any]:
    payload: dict[str, Any] = {
        "row_index": row.row_index,
        "task_instance_id": row.task_instance_id,
        "mutation_category": row.mutation_category,
        "baseline_row_index": row.baseline_row_index,
        "mutation_row_index": row.mutation_row_index,
        "oracle_all_passed": row.oracle_all_passed,
        "delta_sha256": row.delta_sha256,
        "active_slot_count": row.active_slot_count,
        "active_slots": list(row.active_slots),
        "feature_vector": list(row.feature_vector),
    }
    if include_slot_metrics:
        payload["slot_metrics"] = row.slot_metrics
    return payload


def artifact_hashes(paths: dict[str, Path]) -> dict[str, str]:
    return {name: sha256_file(path) for name, path in sorted(paths.items())}


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    dataset_root = Path(args.dataset_root)
    output_parent = Path(args.output_root)
    state_parent = Path(args.state_root)
    run_id = args.run_id or f"run-{utc_stamp()}-feasibility"
    output_root = output_parent / run_id
    state_root = state_parent / run_id
    allow_non_aiwonder_roots = bool(args.allow_non_aiwonder_roots)
    clean = bool(args.clean)

    require_aiwonder_path(dataset_root, "dataset_root", allow_non_aiwonder_roots)
    require_aiwonder_path(output_parent, "output_root", allow_non_aiwonder_roots)
    require_aiwonder_path(state_parent, "state_root", allow_non_aiwonder_roots)
    if clean:
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "CAUSAL_DELTA_OUTPUT_ROOT_EXISTS", output_root)
    require(not state_root.exists(), "CAUSAL_DELTA_STATE_ROOT_EXISTS", state_root)
    output_root.mkdir(parents=True)
    state_root.mkdir(parents=True)

    before_state = {
        "dataset_root": tree_state(dataset_root),
        "output_root": tree_state(output_root),
        "state_root": tree_state(state_root),
    }
    dataset = load_dataset(dataset_root, allow_non_aiwonder_roots=allow_non_aiwonder_roots)
    deltas = build_delta_rows(
        dataset,
        baseline_category=str(args.baseline_category),
        delta_epsilon=float(args.delta_epsilon),
        active_slot_threshold_fraction=float(args.active_slot_threshold_fraction),
    )
    require(len(deltas) >= int(args.min_delta_rows), "CAUSAL_DELTA_TOO_FEW_DELTA_ROWS", {"actual": len(deltas), "minimum": int(args.min_delta_rows)})
    train_idx, holdout_idx = grouped_holdout_split(deltas, float(args.holdout_fraction), int(args.seed))
    separability = evaluate_category_separability(deltas, train_idx, holdout_idx, float(args.ridge_alpha))
    sparsity_summary, sparsity_rows = summarize_sparsity(deltas)

    blockers: list[str] = []
    macro_auc = separability["macro_one_vs_rest_auc"]
    if macro_auc is None or macro_auc < float(args.min_category_macro_auc):
        blockers.append("mutation_category_macro_auc_below_threshold")
    median_active = sparsity_summary["median_active_slot_count"]
    if median_active is None or median_active > float(args.max_median_active_slots):
        blockers.append("delta_not_sparse_enough_by_active_slot_count")
    if separability["holdout_accuracy"] < float(args.min_category_accuracy):
        blockers.append("mutation_category_accuracy_below_threshold")

    proceed = not blockers
    delta_rows_path = output_root / "mutation_delta_rows.jsonl"
    state_delta_rows_path = state_root / "mutation_delta_rows.jsonl"
    category_metrics_path = output_root / "mutation_category_metrics.jsonl"
    state_category_metrics_path = state_root / "mutation_category_metrics.jsonl"
    sparsity_path = output_root / "mutation_delta_sparsity_by_category.jsonl"
    state_sparsity_path = state_root / "mutation_delta_sparsity_by_category.jsonl"
    source_samples_path = output_root / "source_delta_samples.jsonl"
    state_source_samples_path = state_root / "source_delta_samples.jsonl"

    delta_payloads = [delta_row_to_json(row, include_slot_metrics=False) for row in deltas]
    write_jsonl(delta_rows_path, delta_payloads)
    write_jsonl(state_delta_rows_path, delta_payloads)
    write_jsonl(category_metrics_path, separability["category_rows"])
    write_jsonl(state_category_metrics_path, separability["category_rows"])
    write_jsonl(sparsity_path, sparsity_rows)
    write_jsonl(state_sparsity_path, sparsity_rows)
    samples = [delta_row_to_json(row, include_slot_metrics=True) for row in deltas[:40]]
    write_jsonl(source_samples_path, samples)
    write_jsonl(state_source_samples_path, samples)

    artifacts = {
        "mutation_delta_rows": delta_rows_path,
        "mutation_category_metrics": category_metrics_path,
        "mutation_delta_sparsity_by_category": sparsity_path,
        "source_delta_samples": source_samples_path,
    }
    state_artifacts = {
        "state_mutation_delta_rows": state_delta_rows_path,
        "state_mutation_category_metrics": state_category_metrics_path,
        "state_mutation_delta_sparsity_by_category": state_sparsity_path,
        "state_source_delta_samples": state_source_samples_path,
    }
    manifest = {
        "artifact_kind": ARTIFACT_KIND,
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "run_id": run_id,
        "created_at_utc": utc_now(),
        "passes": True,
        "allPassed": True,
        "measurement_only": True,
        "cde_embedder_promotion_allowed": False,
        "predictor_weights_changed": False,
        "production_checkpoint_modified": False,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "target_labels_used_as_live_inputs": False,
        "oracle_outcomes_used_for_offline_reporting_only": True,
        "dataset": {
            "dataset_root": str(dataset_root),
            "dataset_npz": str(dataset.dataset_npz),
            "rows_path": str(dataset.rows_path),
            "row_count": int(len(dataset.rows)),
            "feature_dim": int(dataset.X.shape[1]),
            "active_embedder_ids": sorted(dataset.feature_slices),
            "source_manifest_artifact_kind": dataset.manifest.get("artifact_kind"),
        },
        "delta_construction": {
            "baseline_category": str(args.baseline_category),
            "delta_rows": len(deltas),
            "train_delta_rows": len(train_idx),
            "holdout_delta_rows": len(holdout_idx),
            "feature_vector": "per-slot [log1p(l2_delta), mean_abs_delta, nonzero_fraction]; no oracle fields",
            "delta_epsilon": float(args.delta_epsilon),
            "active_slot_threshold_fraction": float(args.active_slot_threshold_fraction),
        },
        "sparsity": sparsity_summary,
        "separability": {
            "holdout_accuracy": separability["holdout_accuracy"],
            "macro_one_vs_rest_auc": separability["macro_one_vs_rest_auc"],
            "valid_auc_category_count": separability["valid_auc_category_count"],
            "feature_reduction": separability["feature_reduction"],
        },
        "oracle_association": separability["oracle_association"],
        "thresholds": {
            "min_category_macro_auc": float(args.min_category_macro_auc),
            "min_category_accuracy": float(args.min_category_accuracy),
            "max_median_active_slots": float(args.max_median_active_slots),
            "min_delta_rows": int(args.min_delta_rows),
        },
        "decision": {
            "feasibility_measured": True,
            "full_cde_training_recommended": proceed,
            "promotion_allowed": False,
            "promotion_blockers": ["full_cde_training_not_run", "per_cell_utility_lift_not_measured"],
            "full_training_blockers": blockers,
        },
        "artifacts": {name: str(path) for name, path in artifacts.items()},
        "state_artifacts": {name: str(path) for name, path in state_artifacts.items()},
        "before_state": before_state,
        "after_state": {
            "output_root": tree_state(output_root),
            "state_root": tree_state(state_root),
        },
    }
    manifest_path = output_root / "causal_delta_feasibility_manifest.json"
    state_manifest_path = state_root / "causal_delta_feasibility_manifest.json"
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)
    hashes = artifact_hashes({**artifacts, "manifest": manifest_path})
    state_hashes = artifact_hashes({**state_artifacts, "state_manifest": state_manifest_path})
    hash_payload = {"artifacts": hashes, "state_artifacts": state_hashes}
    write_json(output_root / "artifact_hashes.json", hash_payload)
    write_json(state_root / "artifact_hashes.json", hash_payload)
    return {
        "passes": True,
        "manifest": str(manifest_path),
        "state_manifest": str(state_manifest_path),
        "output_root": str(output_root),
        "state_root": str(state_root),
        "delta_rows": len(deltas),
        "full_cde_training_recommended": proceed,
        "full_training_blockers": blockers,
    }


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--baseline-category", default=DEFAULT_BASELINE_CATEGORY)
    parser.add_argument("--holdout-fraction", type=float, default=0.20)
    parser.add_argument("--seed", type=int, default=773)
    parser.add_argument("--ridge-alpha", type=float, default=1.0)
    parser.add_argument("--delta-epsilon", type=float, default=1.0e-8)
    parser.add_argument("--active-slot-threshold-fraction", type=float, default=0.10)
    parser.add_argument("--min-category-macro-auc", type=float, default=0.70)
    parser.add_argument("--min-category-accuracy", type=float, default=0.50)
    parser.add_argument("--max-median-active-slots", type=float, default=4.0)
    parser.add_argument("--min-delta-rows", type=int, default=100)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)
    try:
        result = run_audit(args)
    except CausalDeltaFeasibilityError as exc:
        print(str(exc), flush=True)
        return 2
    print(canonical_json(result), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
