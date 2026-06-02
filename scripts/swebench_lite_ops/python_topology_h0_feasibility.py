#!/usr/bin/env python3
"""INV-044 bounded H0 topology feasibility audit.

This runner consumes the landed INV-032 / #759 panel-oracle dataset and
computes per-slot local H0 barcode summaries from k-nearest-neighbor death
times. It is a cheap feasibility probe for the TopNets/persistent-homology slot
idea, not a promoted slot and not a full H1/ripser TopNets implementation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
from collections import Counter
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TASK_ID = "INV-044"
ARTIFACT_KIND = "topology_h0_feasibility"
SCHEMA_VERSION = 1

DEFAULT_DATASET_ROOT = Path(
    "/zfs/archive/contextgraph/state/mi-audit/inv-032-v1/"
    "run-20260529T234900Z/full-candidate-map"
)
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/inv-044-topnets-persistent-homology-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/topology-h0-feasibility/inv-044-v1")
EPS = 1.0e-12


class TopologyH0FeasibilityError(RuntimeError):
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
class SplitResult:
    split_index: int
    seed: int
    train_idx: list[int]
    holdout_idx: list[int]
    baseline_metrics: dict[str, float | int | None]
    topology_metrics: dict[str, float | int | None]
    augmented_metrics: dict[str, float | int | None]
    delta_metrics: dict[str, float | int | None]
    topology_feature_summary: dict[str, Any]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise TopologyH0FeasibilityError(f"{code}{suffix}")


def require_aiwonder_path(path: Path, name: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/"),
        "TOPOLOGY_H0_NON_AIWONDER_PATH",
        {name: text},
    )
    require("/mnt/c/" not in text and "/mnt/d/" not in text, "TOPOLOGY_H0_MOUNTED_DRIVE_PATH", {name: text})


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def artifact_hashes(paths: dict[str, Path]) -> dict[str, str]:
    return {name: sha256_file(path) for name, path in sorted(paths.items())}


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise TopologyH0FeasibilityError(f"TOPOLOGY_H0_WRITE_READBACK_MISMATCH: {path}")


def write_json(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    count = 0
    lines = []
    for row in rows:
        lines.append(canonical_json(row) + "\n")
        count += 1
    write_text_checked(path, "".join(lines))
    return count


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


def stable_seed(seed: int, label: str) -> int:
    digest = hashlib.sha256(f"{seed}:{label}".encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "little") & 0x7FFFFFFF


def load_dataset(dataset_root: Path, *, allow_non_aiwonder_roots: bool) -> Dataset:
    import numpy as np

    require_aiwonder_path(dataset_root, "dataset_root", allow_non_aiwonder_roots)
    manifest_path = dataset_root / "dataset_manifest.json"
    require(manifest_path.is_file(), "TOPOLOGY_H0_DATASET_MANIFEST_MISSING", manifest_path)
    manifest = read_json(manifest_path)
    require(manifest.get("passes") is True, "TOPOLOGY_H0_DATASET_NOT_PASSING", manifest_path)
    require(manifest.get("slot_identity_preserved") is True, "TOPOLOGY_H0_SLOT_IDENTITY_NOT_PRESERVED")
    require(manifest.get("flat_vector_concat_used") is False, "TOPOLOGY_H0_FLAT_VECTOR_CONCAT_USED")

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
    require(dataset_npz.is_file(), "TOPOLOGY_H0_DATASET_NPZ_MISSING", dataset_npz)
    require(rows_path.is_file(), "TOPOLOGY_H0_DATASET_ROWS_MISSING", rows_path)

    data = np.load(dataset_npz, allow_pickle=False)
    require(set(data.files) == {"X", "y"}, "TOPOLOGY_H0_DATASET_NPZ_KEYS", data.files)
    X = np.asarray(data["X"], dtype=np.float32)
    y = np.asarray(data["y"], dtype=np.int64)
    require(X.ndim == 2 and y.ndim == 1 and X.shape[0] == y.shape[0], "TOPOLOGY_H0_DATASET_SHAPE", (X.shape, y.shape))
    require(bool(np.all(np.isfinite(X))), "TOPOLOGY_H0_DATASET_NONFINITE_FEATURES")
    require(set(int(v) for v in y.tolist()).issubset({0, 1}), "TOPOLOGY_H0_DATASET_NON_BINARY_LABELS")

    rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    require(len(rows) == int(X.shape[0]), "TOPOLOGY_H0_ROW_COUNT_MISMATCH", (len(rows), int(X.shape[0])))
    for index, row in enumerate(rows):
        require(int(row.get("row_index")) == index, "TOPOLOGY_H0_ROW_INDEX_MISMATCH", (index, row.get("row_index")))

    raw_slices = manifest.get("feature_slices") or {}
    active = [str(slot).lower() for slot in (manifest.get("active_embedder_ids") or [])]
    require(active, "TOPOLOGY_H0_ACTIVE_EMBEDDERS_MISSING")
    feature_slices: dict[str, slice] = {}
    for slot in active:
        info = raw_slices.get(slot)
        require(isinstance(info, dict), "TOPOLOGY_H0_ACTIVE_SLICE_MISSING", slot)
        start = int(info.get("start"))
        stop = int(info.get("stop"))
        require(0 <= start < stop <= X.shape[1], "TOPOLOGY_H0_SLICE_OUT_OF_RANGE", {slot: (start, stop), "feature_dim": X.shape[1]})
        feature_slices[slot] = slice(start, stop)
    return Dataset(X=X, y=y, rows=rows, manifest=manifest, feature_slices=feature_slices, dataset_npz=dataset_npz, rows_path=rows_path)


def grouped_holdout_split(rows: list[dict[str, Any]], holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    require(0.05 <= holdout_fraction <= 0.5, "TOPOLOGY_H0_HOLDOUT_FRACTION_INVALID", holdout_fraction)
    by_task: dict[str, list[int]] = {}
    for index, row in enumerate(rows):
        task = str(row.get("task_instance_id") or f"row:{index}")
        by_task.setdefault(task, []).append(index)
    tasks = sorted(by_task)
    require(len(tasks) >= 8, "TOPOLOGY_H0_TOO_FEW_TASKS", len(tasks))
    ranked = sorted(tasks, key=lambda task: hashlib.sha256(f"{seed}:{task}".encode("utf-8")).hexdigest())
    holdout_task_count = max(1, int(round(len(ranked) * holdout_fraction)))
    holdout_tasks = set(ranked[:holdout_task_count])
    train_idx: list[int] = []
    holdout_idx: list[int] = []
    for task in tasks:
        target = holdout_idx if task in holdout_tasks else train_idx
        target.extend(by_task[task])
    require(train_idx and holdout_idx, "TOPOLOGY_H0_SPLIT_EMPTY")
    return sorted(train_idx), sorted(holdout_idx)


def reduce_slot_blocks(
    dataset: Dataset,
    train_idx: list[int],
    *,
    components_per_slot: int,
    seed: int,
) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    require(components_per_slot >= 1, "TOPOLOGY_H0_COMPONENTS_PER_SLOT_INVALID", components_per_slot)
    all_blocks = []
    reductions: dict[str, Any] = {}
    train_index = np.asarray(train_idx, dtype=np.int64)
    for slot in sorted(dataset.feature_slices):
        sl = dataset.feature_slices[slot]
        raw_all = np.asarray(dataset.X[:, sl.start : sl.stop], dtype=np.float64)
        raw_train = raw_all[train_index]
        mean = raw_train.mean(axis=0)
        std = raw_train.std(axis=0)
        keep = std > 1.0e-10
        require(bool(np.any(keep)), "TOPOLOGY_H0_SLOT_BLOCK_CONSTANT", slot)
        std_all = (raw_all[:, keep] - mean[keep]) / std[keep]
        kept_dim = int(std_all.shape[1])
        out_dim = min(components_per_slot, kept_dim)
        rng = np.random.default_rng(stable_seed(seed, f"reduce:{slot}"))
        projection = rng.normal(0.0, 1.0 / math.sqrt(float(out_dim)), size=(kept_dim, out_dim))
        reduced = np.asarray(std_all @ projection, dtype=np.float32)
        all_blocks.append(reduced)
        reductions[slot] = {
            "source_dim": int(sl.stop - sl.start),
            "kept_dim": kept_dim,
            "components": out_dim,
            "reduction": "train-standardized deterministic gaussian projection per slot",
        }
    return np.concatenate(all_blocks, axis=1).astype(np.float32), reductions


def local_h0_features(
    reduced: Any,
    dataset: Dataset,
    train_idx: list[int],
    *,
    reductions: dict[str, Any],
    components_per_slot: int,
    knn_k: int,
    batch_size: int,
) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    require(knn_k >= 2, "TOPOLOGY_H0_KNN_K_INVALID", knn_k)
    require(batch_size >= 16, "TOPOLOGY_H0_BATCH_SIZE_INVALID", batch_size)
    train_index = np.asarray(train_idx, dtype=np.int64)
    require(len(train_index) > knn_k + 1, "TOPOLOGY_H0_TRAIN_TOO_SMALL_FOR_KNN", len(train_index))
    train_pos = {int(row_index): pos for pos, row_index in enumerate(train_index.tolist())}
    slot_features = []
    feature_summary: dict[str, Any] = {}
    cursor = 0
    for slot in sorted(dataset.feature_slices):
        width = int((reductions.get(slot) or {}).get("components") or 0)
        require(width > 0, "TOPOLOGY_H0_REDUCED_SLOT_WIDTH_MISSING", slot)
        require(width <= components_per_slot, "TOPOLOGY_H0_REDUCED_SLOT_WIDTH_INVALID", {slot: width})
        Z = np.asarray(reduced[:, cursor : cursor + width], dtype=np.float64)
        cursor += width
        Z_train = Z[train_index]
        train_norm = np.sum(Z_train * Z_train, axis=1)
        rows = []
        for start in range(0, Z.shape[0], batch_size):
            stop = min(start + batch_size, Z.shape[0])
            block = Z[start:stop]
            dist2 = np.sum(block * block, axis=1, keepdims=True) + train_norm.reshape(1, -1) - 2.0 * (block @ Z_train.T)
            np.maximum(dist2, 0.0, out=dist2)
            for local, row_index in enumerate(range(start, stop)):
                pos = train_pos.get(row_index)
                if pos is not None:
                    dist2[local, pos] = np.inf
            nearest = np.partition(dist2, kth=knn_k - 1, axis=1)[:, :knn_k]
            nearest = np.sqrt(np.sort(nearest, axis=1))
            rows.append(nearest)
        deaths = np.vstack(rows)
        require(bool(np.all(np.isfinite(deaths))), "TOPOLOGY_H0_NONFINITE_KNN_DEATHS", slot)
        feat = np.column_stack(
            [
                deaths[:, 0],
                deaths[:, min(2, knn_k - 1)],
                deaths.mean(axis=1),
                deaths.std(axis=1),
                deaths[:, -1],
                deaths[:, -1] - deaths[:, 0],
            ]
        ).astype(np.float32)
        slot_features.append(feat)
        variance = feat.var(axis=0).astype(float).tolist()
        feature_summary[slot] = {
            "feature_names": [
                "h0_death_k1",
                "h0_death_k3",
                "h0_death_mean",
                "h0_death_std",
                "h0_death_kmax",
                "h0_death_gap",
            ],
            "feature_variance": variance,
            "nondegenerate_feature_count": int(sum(value > 1.0e-12 for value in variance)),
            "knn_k": int(knn_k),
            "point_cloud": "row-local neighborhood in train-standardized per-slot reduced panel space",
        }
    return np.concatenate(slot_features, axis=1).astype(np.float32), feature_summary


def standardize_from_train(features: Any, train_idx: list[int]) -> Any:
    import numpy as np

    X = np.asarray(features, dtype=np.float64)
    train = X[np.asarray(train_idx, dtype=np.int64)]
    mean = train.mean(axis=0)
    std = train.std(axis=0)
    keep = std > 1.0e-10
    require(bool(np.any(keep)), "TOPOLOGY_H0_FEATURE_MATRIX_CONSTANT")
    return ((X[:, keep] - mean[keep]) / std[keep]).astype(np.float64)


def ridge_scores(features: Any, y: Any, train_idx: list[int], holdout_idx: list[int], ridge_alpha: float) -> Any:
    import numpy as np

    require(ridge_alpha >= 0.0, "TOPOLOGY_H0_RIDGE_ALPHA_INVALID", ridge_alpha)
    X = standardize_from_train(features, train_idx)
    train = np.asarray(train_idx, dtype=np.int64)
    holdout = np.asarray(holdout_idx, dtype=np.int64)
    X_train = np.column_stack([np.ones(len(train), dtype=np.float64), X[train]])
    y_train = np.asarray(y[train], dtype=np.float64)
    reg = np.eye(X_train.shape[1], dtype=np.float64) * float(ridge_alpha)
    reg[0, 0] = 0.0
    lhs = X_train.T @ X_train + reg
    rhs = X_train.T @ y_train
    try:
        coef = np.linalg.solve(lhs, rhs)
    except np.linalg.LinAlgError:
        coef = np.linalg.pinv(lhs) @ rhs
    X_holdout = np.column_stack([np.ones(len(holdout), dtype=np.float64), X[holdout]])
    return X_holdout @ coef


def pearson_corr(y: Any, scores: Any) -> float | None:
    import numpy as np

    yy = np.asarray(y, dtype=np.float64)
    ss = np.asarray(scores, dtype=np.float64)
    if yy.size < 2 or ss.size != yy.size:
        return None
    yy = yy - yy.mean()
    ss = ss - ss.mean()
    denom = float(np.sqrt(np.sum(yy * yy) * np.sum(ss * ss)))
    if denom <= EPS:
        return None
    return float(np.sum(yy * ss) / denom)


def auc_score(y: Any, scores: Any) -> float | None:
    import numpy as np

    yy = np.asarray(y, dtype=np.int64)
    ss = np.asarray(scores, dtype=np.float64)
    pos = int(np.sum(yy == 1))
    neg = int(np.sum(yy == 0))
    if pos == 0 or neg == 0:
        return None
    order = np.argsort(ss, kind="mergesort")
    ranks = np.empty_like(order, dtype=np.float64)
    i = 0
    while i < len(order):
        j = i + 1
        while j < len(order) and ss[order[j]] == ss[order[i]]:
            j += 1
        avg_rank = (i + 1 + j) / 2.0
        ranks[order[i:j]] = avg_rank
        i = j
    rank_sum_pos = float(np.sum(ranks[yy == 1]))
    return float((rank_sum_pos - pos * (pos + 1) / 2.0) / (pos * neg))


def brier_score(y: Any, scores: Any) -> float:
    import numpy as np

    yy = np.asarray(y, dtype=np.float64)
    probs = np.clip(np.asarray(scores, dtype=np.float64), 0.0, 1.0)
    return float(np.mean((probs - yy) ** 2))


def accuracy_score(y: Any, scores: Any) -> float:
    import numpy as np

    yy = np.asarray(y, dtype=np.int64)
    preds = (np.asarray(scores, dtype=np.float64) >= 0.5).astype(np.int64)
    return float(np.mean(preds == yy))


def evaluate_scores(y_holdout: Any, scores: Any) -> dict[str, float | int | None]:
    return {
        "holdout_rows": int(len(y_holdout)),
        "pearson": pearson_corr(y_holdout, scores),
        "auc": auc_score(y_holdout, scores),
        "brier": brier_score(y_holdout, scores),
        "accuracy": accuracy_score(y_holdout, scores),
    }


def mean_optional(values: Iterable[float | int | None]) -> float | None:
    real = [float(value) for value in values if value is not None and math.isfinite(float(value))]
    if not real:
        return None
    return float(sum(real) / len(real))


def run_split(
    dataset: Dataset,
    *,
    split_index: int,
    seed: int,
    holdout_fraction: float,
    components_per_slot: int,
    knn_k: int,
    batch_size: int,
    ridge_alpha: float,
) -> SplitResult:
    import numpy as np

    train_idx, holdout_idx = grouped_holdout_split(dataset.rows, holdout_fraction, seed)
    reduced, reductions = reduce_slot_blocks(dataset, train_idx, components_per_slot=components_per_slot, seed=seed)
    topology, topology_feature_summary = local_h0_features(
        reduced,
        dataset,
        train_idx,
        reductions=reductions,
        components_per_slot=components_per_slot,
        knn_k=knn_k,
        batch_size=batch_size,
    )
    holdout = np.asarray(holdout_idx, dtype=np.int64)
    y_holdout = dataset.y[holdout]
    baseline_scores = ridge_scores(reduced, dataset.y, train_idx, holdout_idx, ridge_alpha)
    topology_scores = ridge_scores(topology, dataset.y, train_idx, holdout_idx, ridge_alpha)
    augmented = np.concatenate([reduced, topology], axis=1)
    augmented_scores = ridge_scores(augmented, dataset.y, train_idx, holdout_idx, ridge_alpha)
    baseline = evaluate_scores(y_holdout, baseline_scores)
    topology_only = evaluate_scores(y_holdout, topology_scores)
    augmented_metrics = evaluate_scores(y_holdout, augmented_scores)
    delta = {
        "pearson_delta": (
            None
            if baseline["pearson"] is None or augmented_metrics["pearson"] is None
            else float(augmented_metrics["pearson"]) - float(baseline["pearson"])
        ),
        "auc_delta": (
            None
            if baseline["auc"] is None or augmented_metrics["auc"] is None
            else float(augmented_metrics["auc"]) - float(baseline["auc"])
        ),
        "brier_delta": float(augmented_metrics["brier"]) - float(baseline["brier"]),
        "accuracy_delta": float(augmented_metrics["accuracy"]) - float(baseline["accuracy"]),
    }
    return SplitResult(
        split_index=split_index,
        seed=seed,
        train_idx=train_idx,
        holdout_idx=holdout_idx,
        baseline_metrics=baseline,
        topology_metrics=topology_only,
        augmented_metrics=augmented_metrics,
        delta_metrics=delta,
        topology_feature_summary=topology_feature_summary,
    )


def split_result_to_json(result: SplitResult) -> dict[str, Any]:
    return {
        "split_index": result.split_index,
        "seed": result.seed,
        "train_rows": len(result.train_idx),
        "holdout_rows": len(result.holdout_idx),
        "baseline_metrics": result.baseline_metrics,
        "topology_only_metrics": result.topology_metrics,
        "augmented_metrics": result.augmented_metrics,
        "delta_metrics": result.delta_metrics,
        "topology_feature_summary": result.topology_feature_summary,
    }


def summarize_results(results: list[SplitResult]) -> dict[str, Any]:
    deltas = [result.delta_metrics for result in results]
    baseline = [result.baseline_metrics for result in results]
    topology = [result.topology_metrics for result in results]
    augmented = [result.augmented_metrics for result in results]
    slot_nondegenerate: Counter[str] = Counter()
    for result in results:
        for slot, summary in result.topology_feature_summary.items():
            if int(summary.get("nondegenerate_feature_count") or 0) > 0:
                slot_nondegenerate[slot] += 1
    return {
        "split_count": len(results),
        "baseline_mean": {
            "pearson": mean_optional(row.get("pearson") for row in baseline),
            "auc": mean_optional(row.get("auc") for row in baseline),
            "brier": mean_optional(row.get("brier") for row in baseline),
            "accuracy": mean_optional(row.get("accuracy") for row in baseline),
        },
        "topology_only_mean": {
            "pearson": mean_optional(row.get("pearson") for row in topology),
            "auc": mean_optional(row.get("auc") for row in topology),
            "brier": mean_optional(row.get("brier") for row in topology),
            "accuracy": mean_optional(row.get("accuracy") for row in topology),
        },
        "augmented_mean": {
            "pearson": mean_optional(row.get("pearson") for row in augmented),
            "auc": mean_optional(row.get("auc") for row in augmented),
            "brier": mean_optional(row.get("brier") for row in augmented),
            "accuracy": mean_optional(row.get("accuracy") for row in augmented),
        },
        "delta_mean": {
            "pearson_delta": mean_optional(row.get("pearson_delta") for row in deltas),
            "auc_delta": mean_optional(row.get("auc_delta") for row in deltas),
            "brier_delta": mean_optional(row.get("brier_delta") for row in deltas),
            "accuracy_delta": mean_optional(row.get("accuracy_delta") for row in deltas),
        },
        "nondegenerate_slots_by_split_count": dict(sorted(slot_nondegenerate.items())),
    }


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    dataset_root = Path(args.dataset_root)
    run_id = str(args.run_id or f"run-{utc_stamp()}")
    output_root = Path(args.output_root) / run_id if Path(args.output_root).name != run_id else Path(args.output_root)
    state_root = Path(args.state_root) / run_id if Path(args.state_root).name != run_id else Path(args.state_root)
    allow_non_aiwonder = bool(args.allow_non_aiwonder_roots)
    require_aiwonder_path(output_root, "output_root", allow_non_aiwonder)
    require_aiwonder_path(state_root, "state_root", allow_non_aiwonder)

    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "TOPOLOGY_H0_OUTPUT_ROOT_EXISTS", output_root)
    require(not state_root.exists(), "TOPOLOGY_H0_STATE_ROOT_EXISTS", state_root)
    before_state = {"output_root": tree_state(output_root), "state_root": tree_state(state_root)}

    dataset = load_dataset(dataset_root, allow_non_aiwonder_roots=allow_non_aiwonder)
    require(int(args.split_count) >= 1, "TOPOLOGY_H0_SPLIT_COUNT_INVALID", args.split_count)
    require(int(args.min_holdout_rows) >= 1, "TOPOLOGY_H0_MIN_HOLDOUT_ROWS_INVALID", args.min_holdout_rows)

    results: list[SplitResult] = []
    for split_index in range(int(args.split_count)):
        split_seed = stable_seed(int(args.seed), f"split:{split_index}")
        result = run_split(
            dataset,
            split_index=split_index,
            seed=split_seed,
            holdout_fraction=float(args.holdout_fraction),
            components_per_slot=int(args.components_per_slot),
            knn_k=int(args.knn_k),
            batch_size=int(args.batch_size),
            ridge_alpha=float(args.ridge_alpha),
        )
        require(len(result.holdout_idx) >= int(args.min_holdout_rows), "TOPOLOGY_H0_HOLDOUT_TOO_SMALL", len(result.holdout_idx))
        results.append(result)

    summary = summarize_results(results)
    mean_delta = summary["delta_mean"]
    topology_mean = summary["topology_only_mean"]
    blockers: list[str] = []
    if mean_delta["pearson_delta"] is None or float(mean_delta["pearson_delta"]) < float(args.min_pearson_delta):
        blockers.append("augmented_pearson_delta_below_threshold")
    if topology_mean["auc"] is None or float(topology_mean["auc"]) < float(args.min_topology_auc):
        blockers.append("topology_only_auc_below_threshold")
    if not summary["nondegenerate_slots_by_split_count"]:
        blockers.append("topology_features_degenerate")
    proceed = not blockers

    output_root.mkdir(parents=True, exist_ok=False)
    state_root.mkdir(parents=True, exist_ok=False)
    split_metrics_path = output_root / "topology_h0_split_metrics.jsonl"
    state_split_metrics_path = state_root / "topology_h0_split_metrics.jsonl"
    feature_summary_path = output_root / "topology_h0_feature_summary.json"
    state_feature_summary_path = state_root / "topology_h0_feature_summary.json"
    rows = [split_result_to_json(result) for result in results]
    write_jsonl(split_metrics_path, rows)
    write_jsonl(state_split_metrics_path, rows)
    write_json(feature_summary_path, summary)
    write_json(state_feature_summary_path, summary)

    artifacts = {
        "split_metrics": split_metrics_path,
        "feature_summary": feature_summary_path,
    }
    state_artifacts = {
        "state_split_metrics": state_split_metrics_path,
        "state_feature_summary": state_feature_summary_path,
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
        "topnets_slot_promotion_allowed": False,
        "full_topnets_slot_build_recommended": proceed,
        "predictor_weights_changed": False,
        "production_checkpoint_modified": False,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "target_labels_used_as_live_inputs": False,
        "oracle_outcomes_used_for_offline_reporting_only": True,
        "topology_method": {
            "homology_dimension": 0,
            "barcode_proxy": "local kNN H0 death-time summaries per slot",
            "why_not_h1": "aiwonder venv lacks gudhi/ripser/scipy; this is the cheapest dependency-free falsification pass",
            "primary_sources": [
                "https://openreview.net/forum?id=ELFZWG9C7l",
                "https://proceedings.mlr.press/v267/xin25b.html",
                "https://arxiv.org/abs/2510.05102",
            ],
        },
        "dataset": {
            "dataset_root": str(dataset_root),
            "dataset_npz": str(dataset.dataset_npz),
            "rows_path": str(dataset.rows_path),
            "row_count": int(len(dataset.rows)),
            "feature_dim": int(dataset.X.shape[1]),
            "active_embedder_ids": sorted(dataset.feature_slices),
            "source_manifest_artifact_kind": dataset.manifest.get("artifact_kind"),
            "source_target_bits": dataset.manifest.get("target_bits"),
            "label_counts": dict(Counter(int(v) for v in dataset.y.tolist())),
        },
        "probe": {
            "split_count": int(args.split_count),
            "holdout_fraction": float(args.holdout_fraction),
            "seed": int(args.seed),
            "components_per_slot": int(args.components_per_slot),
            "knn_k": int(args.knn_k),
            "batch_size": int(args.batch_size),
            "ridge_alpha": float(args.ridge_alpha),
            "baseline_feature": "train-standardized deterministic per-slot random projections",
            "topology_feature": "per-slot local H0 kNN death-time summaries computed against train rows only",
            "augmented_feature": "baseline_feature + topology_feature",
        },
        "thresholds": {
            "min_pearson_delta": float(args.min_pearson_delta),
            "min_topology_auc": float(args.min_topology_auc),
            "min_holdout_rows": int(args.min_holdout_rows),
        },
        "summary": summary,
        "decision": {
            "feasibility_measured": True,
            "full_topnets_slot_build_recommended": proceed,
            "promotion_allowed": False,
            "promotion_blockers": ["full_topnets_slot_not_built", "novelty_gate_not_run", "utility_gate_not_run"],
            "full_build_blockers": blockers,
        },
        "artifacts": {name: str(path) for name, path in artifacts.items()},
        "state_artifacts": {name: str(path) for name, path in state_artifacts.items()},
        "before_state": before_state,
        "after_state": {"output_root": tree_state(output_root), "state_root": tree_state(state_root)},
    }
    manifest_path = output_root / "topology_h0_feasibility_manifest.json"
    state_manifest_path = state_root / "topology_h0_feasibility_manifest.json"
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
        "full_topnets_slot_build_recommended": proceed,
        "full_build_blockers": blockers,
        "mean_pearson_delta": summary["delta_mean"]["pearson_delta"],
        "topology_only_auc": summary["topology_only_mean"]["auc"],
    }


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--holdout-fraction", type=float, default=0.20)
    parser.add_argument("--split-count", type=int, default=3)
    parser.add_argument("--seed", type=int, default=779)
    parser.add_argument("--components-per-slot", type=int, default=16)
    parser.add_argument("--knn-k", type=int, default=8)
    parser.add_argument("--batch-size", type=int, default=256)
    parser.add_argument("--ridge-alpha", type=float, default=1.0)
    parser.add_argument("--min-pearson-delta", type=float, default=0.005)
    parser.add_argument("--min-topology-auc", type=float, default=0.55)
    parser.add_argument("--min-holdout-rows", type=int, default=200)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_arg_parser().parse_args(argv)
    try:
        result = run_audit(args)
    except TopologyH0FeasibilityError as exc:
        print(str(exc), flush=True)
        return 2
    print(canonical_json(result), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
