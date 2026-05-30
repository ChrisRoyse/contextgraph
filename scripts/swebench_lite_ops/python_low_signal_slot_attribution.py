#!/usr/bin/env python3
"""TASK-CR-008 Phase 2 slot-attribution slice.

This runner consumes the landed INV-032 / #759 panel-oracle dataset and
measures per-cell oracle-correlation lift by masking each candidate slot block
at inference time in a deterministic held-out ridge probe. It is deliberately
not the Phase 1 retraining ablation from #746, so it never populates
`holdout_corr_delta_when_dropped`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import sys
from collections import Counter
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TASK_ID = "TASK-CR-008"
ARTIFACT_KIND = "low_signal_slot_attribution_phase_2"
SCHEMA_VERSION = 1

DEFAULT_DATASET_ROOT = Path(
    "/zfs/archive/contextgraph/state/mi-audit/inv-032-v1/"
    "run-20260529T234900Z/full-candidate-map"
)
DEFAULT_OUTPUT_PARENT = Path(
    "/zfs/archive/contextgraph/fsv/task-cr-008-low-signal-embedder-audit-fsv/"
    "phase-2-attribution"
)
DEFAULT_STATE_PARENT = Path(
    "/zfs/archive/contextgraph/state/embedder-signal-audit/"
    "task-cr-008-phase-2-attribution-v1"
)

DEFAULT_CANDIDATE_SLOTS = ("e9", "e14", "e2", "e3", "e4", "e11", "e5")
TEMPORAL_SLOTS = {"e2", "e3", "e4"}
EMPTY_SLOTS = {"e5", "e11"}
EPS = 1.0e-12


class SlotAttributionError(RuntimeError):
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
class Probe:
    train_idx: list[int]
    holdout_idx: list[int]
    feature_blocks: dict[str, slice]
    coefficients: Any
    intercept: float
    reduction: dict[str, Any]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


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
        raise SlotAttributionError(f"SLOT_ATTR_WRITE_READBACK_MISMATCH: {path}")


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


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise SlotAttributionError(f"{code}{suffix}")


def require_aiwonder_path(path: Path, name: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/"),
        "SLOT_ATTR_NON_AIWONDER_PATH",
        {name: text},
    )
    require("/mnt/c/" not in text and "/mnt/d/" not in text, "SLOT_ATTR_LEGACY_MOUNT_PATH", {name: text})


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
    require(manifest_path.is_file(), "SLOT_ATTR_DATASET_MANIFEST_MISSING", manifest_path)
    manifest = read_json(manifest_path)
    require(manifest.get("passes") is True, "SLOT_ATTR_DATASET_NOT_PASSING", manifest_path)
    require(manifest.get("slot_identity_preserved") is True, "SLOT_ATTR_SLOT_IDENTITY_NOT_PRESERVED")
    require(manifest.get("flat_vector_concat_used") is False, "SLOT_ATTR_FLAT_VECTOR_CONCAT_USED")

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
    require(dataset_npz.is_file(), "SLOT_ATTR_DATASET_NPZ_MISSING", dataset_npz)
    require(rows_path.is_file(), "SLOT_ATTR_DATASET_ROWS_MISSING", rows_path)

    data = np.load(dataset_npz, allow_pickle=False)
    require(set(data.files) == {"X", "y"}, "SLOT_ATTR_DATASET_NPZ_KEYS", data.files)
    X = np.asarray(data["X"], dtype=np.float32)
    y = np.asarray(data["y"], dtype=np.int64)
    require(X.ndim == 2 and y.ndim == 1 and X.shape[0] == y.shape[0], "SLOT_ATTR_DATASET_SHAPE", (X.shape, y.shape))
    require(bool(np.all(np.isfinite(X))), "SLOT_ATTR_DATASET_NONFINITE_FEATURES")
    require(set(int(v) for v in y.tolist()).issubset({0, 1}), "SLOT_ATTR_DATASET_NON_BINARY_LABELS")

    rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    require(len(rows) == int(X.shape[0]), "SLOT_ATTR_ROW_COUNT_MISMATCH", (len(rows), int(X.shape[0])))
    for index, row in enumerate(rows):
        require(int(row.get("row_index")) == index, "SLOT_ATTR_ROW_INDEX_MISMATCH", (index, row.get("row_index")))

    raw_slices = manifest.get("feature_slices") or {}
    active = [str(slot).lower() for slot in (manifest.get("active_embedder_ids") or [])]
    require(active, "SLOT_ATTR_ACTIVE_EMBEDDERS_MISSING")
    feature_slices: dict[str, slice] = {}
    for slot in active:
        info = raw_slices.get(slot)
        require(isinstance(info, dict), "SLOT_ATTR_ACTIVE_SLICE_MISSING", slot)
        start = int(info.get("start"))
        stop = int(info.get("stop"))
        require(0 <= start < stop <= X.shape[1], "SLOT_ATTR_SLICE_OUT_OF_RANGE", {slot: (start, stop), "feature_dim": X.shape[1]})
        feature_slices[slot] = slice(start, stop)
    return Dataset(X=X, y=y, rows=rows, manifest=manifest, feature_slices=feature_slices, dataset_npz=dataset_npz, rows_path=rows_path)


def grouped_holdout_split(rows: list[dict[str, Any]], holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    require(0.05 <= holdout_fraction <= 0.5, "SLOT_ATTR_HOLDOUT_FRACTION_INVALID", holdout_fraction)
    by_task: dict[str, list[int]] = {}
    for index, row in enumerate(rows):
        task = str(row.get("task_instance_id") or f"row:{index}")
        by_task.setdefault(task, []).append(index)
    tasks = sorted(by_task)
    require(len(tasks) >= 8, "SLOT_ATTR_TOO_FEW_TASKS", len(tasks))
    ranked = sorted(tasks, key=lambda task: hashlib.sha256(f"{seed}:{task}".encode("utf-8")).hexdigest())
    holdout_task_count = max(1, int(round(len(ranked) * holdout_fraction)))
    holdout_tasks = set(ranked[:holdout_task_count])
    train_idx: list[int] = []
    holdout_idx: list[int] = []
    for task in tasks:
        target = holdout_idx if task in holdout_tasks else train_idx
        target.extend(by_task[task])
    require(train_idx and holdout_idx, "SLOT_ATTR_SPLIT_EMPTY")
    return sorted(train_idx), sorted(holdout_idx)


def reduce_slot_blocks(
    X_train: Any,
    X_eval: Any,
    feature_slices: dict[str, slice],
    *,
    components_per_slot: int,
    seed: int,
) -> tuple[Any, Any, dict[str, slice], dict[str, Any]]:
    import numpy as np

    require(components_per_slot >= 1, "SLOT_ATTR_COMPONENTS_PER_SLOT_INVALID", components_per_slot)
    train_blocks = []
    eval_blocks = []
    block_slices: dict[str, slice] = {}
    reductions: dict[str, Any] = {}
    cursor = 0
    for slot in sorted(feature_slices):
        sl = feature_slices[slot]
        train_raw = np.asarray(X_train[:, sl.start : sl.stop], dtype=np.float64)
        eval_raw = np.asarray(X_eval[:, sl.start : sl.stop], dtype=np.float64)
        mean = train_raw.mean(axis=0)
        std = train_raw.std(axis=0)
        keep = std > 1.0e-10
        require(bool(np.any(keep)), "SLOT_ATTR_SLOT_BLOCK_CONSTANT", slot)
        train_std = (train_raw[:, keep] - mean[keep]) / std[keep]
        eval_std = (eval_raw[:, keep] - mean[keep]) / std[keep]
        kept_dim = int(train_std.shape[1])
        if kept_dim > components_per_slot:
            rng = np.random.default_rng(stable_seed(seed, slot))
            projection = rng.normal(
                0.0,
                1.0 / math.sqrt(float(components_per_slot)),
                size=(kept_dim, components_per_slot),
            )
            train_block = train_std @ projection
            eval_block = eval_std @ projection
            method = "per_slot_gaussian_random_projection"
            output_dim = components_per_slot
        else:
            train_block = train_std
            eval_block = eval_std
            method = "per_slot_standardized_nonconstant_features"
            output_dim = kept_dim
        train_block = np.asarray(train_block, dtype=np.float64)
        eval_block = np.asarray(eval_block, dtype=np.float64)
        block_slices[slot] = slice(cursor, cursor + output_dim)
        cursor += output_dim
        train_blocks.append(train_block)
        eval_blocks.append(eval_block)
        reductions[slot] = {
            "input_start": int(sl.start),
            "input_stop": int(sl.stop),
            "input_dim": int(sl.stop - sl.start),
            "nonconstant_dim": kept_dim,
            "output_dim": int(output_dim),
            "method": method,
            "seed": stable_seed(seed, slot) if method == "per_slot_gaussian_random_projection" else None,
        }
    Z_train = np.concatenate(train_blocks, axis=1)
    Z_eval = np.concatenate(eval_blocks, axis=1)
    return (
        Z_train,
        Z_eval,
        block_slices,
        {
            "slot_block_reductions": reductions,
            "output_dim": int(Z_train.shape[1]),
            "components_per_slot": int(components_per_slot),
            "slot_order": sorted(feature_slices),
        },
    )


def fit_ridge_probe(Z_train: Any, y_train: Any, ridge_alpha: float) -> tuple[Any, float]:
    import numpy as np

    require(ridge_alpha > 0.0 and math.isfinite(ridge_alpha), "SLOT_ATTR_RIDGE_ALPHA_INVALID", ridge_alpha)
    Z = np.asarray(Z_train, dtype=np.float64)
    y = np.asarray(y_train, dtype=np.float64)
    require(Z.ndim == 2 and y.ndim == 1 and Z.shape[0] == y.shape[0], "SLOT_ATTR_RIDGE_SHAPE", (Z.shape, y.shape))
    ones = np.ones((Z.shape[0], 1), dtype=np.float64)
    design = np.concatenate([ones, Z], axis=1)
    penalty = np.eye(design.shape[1], dtype=np.float64) * ridge_alpha
    penalty[0, 0] = 0.0
    try:
        coef = np.linalg.solve(design.T @ design + penalty, design.T @ y)
    except np.linalg.LinAlgError:
        coef = np.linalg.pinv(design.T @ design + penalty) @ design.T @ y
    return coef[1:], float(coef[0])


def predict_probe(Z: Any, coefficients: Any, intercept: float) -> Any:
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


def brier_score(y_true: Any, y_score: Any) -> float | None:
    import numpy as np

    y = np.asarray(y_true, dtype=np.float64)
    if y.size == 0:
        return None
    clipped = np.clip(np.asarray(y_score, dtype=np.float64), 0.0, 1.0)
    return float(np.mean(np.square(clipped - y)))


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


def metric_row(y_true: Any, y_score: Any) -> dict[str, Any]:
    labels = [int(v) for v in y_true.tolist()]
    counts = Counter(labels)
    corr = pearson_corr(y_true, y_score)
    return {
        "row_count": len(labels),
        "label_counts": {str(key): int(value) for key, value in sorted(counts.items())},
        "pearson": corr,
        "brier": brier_score(y_true, y_score),
        "auc": auc_score(y_true, y_score),
        "valid_for_corr": corr is not None,
        "invalid_reason": None if corr is not None else "DEGENERATE_OR_CONSTANT_CELL",
    }


def build_probe(dataset: Dataset, *, holdout_fraction: float, seed: int, components_per_slot: int, ridge_alpha: float) -> tuple[Probe, Any, Any]:
    import numpy as np

    train_idx, holdout_idx = grouped_holdout_split(dataset.rows, holdout_fraction, seed)
    y_train = np.asarray(dataset.y)[train_idx]
    y_holdout = np.asarray(dataset.y)[holdout_idx]
    require(len(set(int(v) for v in y_train.tolist())) == 2, "SLOT_ATTR_TRAIN_LABELS_DEGENERATE")
    Z_train, Z_holdout, blocks, reduction = reduce_slot_blocks(
        np.asarray(dataset.X)[train_idx],
        np.asarray(dataset.X)[holdout_idx],
        dataset.feature_slices,
        components_per_slot=components_per_slot,
        seed=seed,
    )
    coefficients, intercept = fit_ridge_probe(Z_train, y_train, ridge_alpha)
    probe = Probe(
        train_idx=train_idx,
        holdout_idx=holdout_idx,
        feature_blocks=blocks,
        coefficients=coefficients,
        intercept=intercept,
        reduction=reduction,
    )
    return probe, Z_holdout, y_holdout


def summarize_slot(
    *,
    slot: str,
    cell_rows: list[dict[str, Any]],
    feature_blocks: dict[str, slice],
) -> dict[str, Any]:
    values = [
        float(row["oracle_corr_lift_per_slot"])
        for row in cell_rows
        if row.get("scope") == "cell" and isinstance(row.get("oracle_corr_lift_per_slot"), (int, float))
    ]
    brier_values = [
        float(row["brier_delta_when_masked"])
        for row in cell_rows
        if row.get("scope") == "cell" and isinstance(row.get("brier_delta_when_masked"), (int, float))
    ]
    if slot not in feature_blocks:
        reason = (
            "temporal_slot_absent_from_static_ast_dataset"
            if slot in TEMPORAL_SLOTS
            else "empty_or_absent_slot_not_in_759_dataset"
            if slot in EMPTY_SLOTS
            else "slot_absent_from_759_dataset"
        )
        return {
            "slot": slot.upper(),
            "measured": False,
            "reason": reason,
            "per_cell_median_attribution": None,
            "holdout_corr_delta_when_dropped": None,
            "replaceable": False,
            "authoritative_for_replacement": False,
            "decision": "phase_2_not_measurable_in_static_ast_panel_dataset",
            "blockers": ["phase_1_drop_ablation_required"] + (["phase_3_live_temporal_variance_required"] if slot in TEMPORAL_SLOTS else []),
        }
    median = sorted(values)[len(values) // 2] if values else None
    min_value = min(values) if values else None
    max_value = max(values) if values else None
    median_brier = sorted(brier_values)[len(brier_values) // 2] if brier_values else None
    if median is None:
        decision = "phase_2_no_mixed_cells_for_corr"
    elif median >= 0.005:
        decision = "phase_2_keep_signal_detected"
    elif median < 0.001:
        decision = "phase_2_low_signal_candidate_needs_phase_1_confirmation"
    else:
        decision = "phase_2_marginal_signal_needs_phase_1_confirmation"
    return {
        "slot": slot.upper(),
        "measured": True,
        "reason": None,
        "per_cell_median_attribution": median,
        "per_cell_min_attribution": min_value,
        "per_cell_max_attribution": max_value,
        "per_cell_valid_corr_count": len(values),
        "per_cell_median_brier_delta_when_masked": median_brier,
        "holdout_corr_delta_when_dropped": None,
        "replaceable": False,
        "authoritative_for_replacement": False,
        "decision": decision,
        "blockers": ["phase_1_drop_ablation_required"],
    }


def compute_attributions(dataset: Dataset, probe: Probe, Z_holdout: Any, y_holdout: Any, candidate_slots: list[str]) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    import numpy as np

    holdout_rows = [dataset.rows[index] for index in probe.holdout_idx]
    full_scores = predict_probe(Z_holdout, probe.coefficients, probe.intercept)
    categories = sorted({str(row.get("mutation_category") or "") for row in holdout_rows})
    category_indexes = {
        category: [idx for idx, row in enumerate(holdout_rows) if str(row.get("mutation_category") or "") == category]
        for category in categories
    }
    global_metrics = metric_row(y_holdout, full_scores)
    per_cell_rows: list[dict[str, Any]] = []
    summary_rows: list[dict[str, Any]] = []
    for slot in candidate_slots:
        slot = slot.lower()
        slot_cell_rows: list[dict[str, Any]] = []
        if slot in probe.feature_blocks:
            masked = np.array(Z_holdout, copy=True)
            block = probe.feature_blocks[slot]
            masked[:, block.start : block.stop] = 0.0
            masked_scores = predict_probe(masked, probe.coefficients, probe.intercept)
            full_global = metric_row(y_holdout, full_scores)
            masked_global = metric_row(y_holdout, masked_scores)
            global_lift = (
                full_global["pearson"] - masked_global["pearson"]
                if full_global["pearson"] is not None and masked_global["pearson"] is not None
                else None
            )
            global_brier_delta = (
                masked_global["brier"] - full_global["brier"]
                if full_global["brier"] is not None and masked_global["brier"] is not None
                else None
            )
            slot_cell_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "slot": slot.upper(),
                    "scope": "global",
                    "cell": "python::all",
                    "mutation_category": "all",
                    "language": "python",
                    "measured": True,
                    "full_metrics": full_global,
                    "masked_metrics": masked_global,
                    "oracle_corr_lift_per_slot": global_lift,
                    "brier_delta_when_masked": global_brier_delta,
                    "interpretation": "positive lift means full model correlation exceeds same model with this slot block masked",
                }
            )
            for category, indexes in category_indexes.items():
                idx = np.asarray(indexes, dtype=np.int64)
                full_cell = metric_row(y_holdout[idx], full_scores[idx])
                masked_cell = metric_row(y_holdout[idx], masked_scores[idx])
                lift = (
                    full_cell["pearson"] - masked_cell["pearson"]
                    if full_cell["pearson"] is not None and masked_cell["pearson"] is not None
                    else None
                )
                brier_delta = (
                    masked_cell["brier"] - full_cell["brier"]
                    if full_cell["brier"] is not None and masked_cell["brier"] is not None
                    else None
                )
                slot_cell_rows.append(
                    {
                        "schema_version": SCHEMA_VERSION,
                        "slot": slot.upper(),
                        "scope": "cell",
                        "cell": f"{category}::python",
                        "mutation_category": category,
                        "language": "python",
                        "measured": True,
                        "full_metrics": full_cell,
                        "masked_metrics": masked_cell,
                        "oracle_corr_lift_per_slot": lift,
                        "brier_delta_when_masked": brier_delta,
                        "interpretation": "positive lift means full model correlation exceeds same model with this slot block masked",
                    }
                )
        else:
            slot_cell_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "slot": slot.upper(),
                    "scope": "global",
                    "cell": "python::all",
                    "mutation_category": "all",
                    "language": "python",
                    "measured": False,
                    "full_metrics": global_metrics,
                    "masked_metrics": None,
                    "oracle_corr_lift_per_slot": None,
                    "brier_delta_when_masked": None,
                    "invalid_reason": (
                        "temporal_slot_absent_from_static_ast_dataset"
                        if slot in TEMPORAL_SLOTS
                        else "empty_or_absent_slot_not_in_759_dataset"
                        if slot in EMPTY_SLOTS
                        else "slot_absent_from_759_dataset"
                    ),
                }
            )
        per_cell_rows.extend(slot_cell_rows)
        summary_rows.append(summarize_slot(slot=slot, cell_rows=slot_cell_rows, feature_blocks=probe.feature_blocks))
    diagnostics = {
        "holdout_row_count": len(probe.holdout_idx),
        "train_row_count": len(probe.train_idx),
        "holdout_label_counts": {str(key): int(value) for key, value in sorted(Counter(int(v) for v in y_holdout.tolist()).items())},
        "holdout_category_counts": dict(sorted(Counter(str(row.get("mutation_category") or "") for row in holdout_rows).items())),
        "full_holdout_metrics": global_metrics,
    }
    return per_cell_rows, summary_rows, diagnostics


def validation_cases(per_cell_rows: list[dict[str, Any]], summary_rows: list[dict[str, Any]], candidate_slots: list[str], doctrine: dict[str, Any]) -> list[dict[str, Any]]:
    measured = {row["slot"] for row in summary_rows if row.get("measured") is True}
    unmeasured = {row["slot"] for row in summary_rows if row.get("measured") is False}
    active_expected = {slot.upper() for slot in candidate_slots if slot.lower() in {"e9", "e14"}}
    temporal_expected = {slot.upper() for slot in candidate_slots if slot.lower() in TEMPORAL_SLOTS}
    active_replaceable = [
        row["slot"]
        for row in summary_rows
        if row.get("replaceable") is True and row["slot"].lower() not in EMPTY_SLOTS
    ]
    return [
        {
            "case": "slot_identity_preserved",
            "expected": True,
            "actual": doctrine.get("slot_identity_preserved"),
            "pass": doctrine.get("slot_identity_preserved") is True,
        },
        {
            "case": "no_flat_vector_identity_erasure",
            "expected": False,
            "actual": doctrine.get("flat_vector_concat_used"),
            "pass": doctrine.get("flat_vector_concat_used") is False,
        },
        {
            "case": "active_static_candidates_measured",
            "expected": sorted(active_expected),
            "actual": sorted(measured & active_expected),
            "pass": active_expected.issubset(measured),
        },
        {
            "case": "temporal_slots_not_fabricated_from_static_dataset",
            "expected": sorted(temporal_expected),
            "actual": sorted(unmeasured & temporal_expected),
            "pass": temporal_expected.issubset(unmeasured),
        },
        {
            "case": "phase_1_drop_ablation_not_claimed",
            "expected": "all summary holdout_corr_delta_when_dropped values are null",
            "actual": {row["slot"]: row.get("holdout_corr_delta_when_dropped") for row in summary_rows},
            "pass": all(row.get("holdout_corr_delta_when_dropped") is None for row in summary_rows),
        },
        {
            "case": "active_slots_not_marked_replaceable_from_phase_2_only",
            "expected": [],
            "actual": active_replaceable,
            "pass": not active_replaceable,
        },
        {
            "case": "per_cell_rows_written_for_measured_slots",
            "expected": "at least one cell row per measured slot",
            "actual": dict(sorted(Counter(row["slot"] for row in per_cell_rows if row.get("scope") == "cell").items())),
            "pass": all(any(row["slot"] == slot and row.get("scope") == "cell" for row in per_cell_rows) for slot in measured),
        },
    ]


def artifact_hashes(paths: dict[str, Path]) -> dict[str, str]:
    return {name: sha256_file(path) for name, path in sorted(paths.items()) if path.is_file()}


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    candidate_slots = [slot.lower() for slot in args.candidate_slot]
    run_id = args.run_id or f"run-{utc_stamp()}"
    output_root = args.output_root or (args.output_parent / run_id)
    state_root = args.state_root or (args.state_parent / run_id)
    require_aiwonder_path(output_root, "output_root", args.allow_non_aiwonder_roots)
    require_aiwonder_path(state_root, "state_root", args.allow_non_aiwonder_roots)
    before = {"output_root": tree_state(output_root), "state_root": tree_state(state_root)}
    if args.clean:
        if output_root.exists():
            shutil.rmtree(output_root)
        if state_root.exists() and args.allow_clean_state_root:
            shutil.rmtree(state_root)
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)

    dataset = load_dataset(args.dataset_root, allow_non_aiwonder_roots=args.allow_non_aiwonder_roots)
    probe, Z_holdout, y_holdout = build_probe(
        dataset,
        holdout_fraction=args.holdout_fraction,
        seed=args.seed,
        components_per_slot=args.components_per_slot,
        ridge_alpha=args.ridge_alpha,
    )
    per_cell_rows, summary_rows, diagnostics = compute_attributions(dataset, probe, Z_holdout, y_holdout, candidate_slots)

    doctrine = {
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "slot_block_late_fusion_used": True,
        "target_labels_used_as_live_inputs": False,
        "target_labels_used_as_audit_supervision": True,
        "no_inner_llm_used": True,
        "phase_1_retraining_ablation_run": False,
        "phase_3_live_temporal_variance_run": False,
        "aiwonder_durable_root_required": (
            str(output_root).startswith("/zfs/archive/contextgraph/")
            and str(state_root).startswith("/zfs/archive/contextgraph/")
        )
        if not args.allow_non_aiwonder_roots
        else True,
    }
    cases = validation_cases(per_cell_rows, summary_rows, candidate_slots, doctrine)
    all_passed = all(case["pass"] for case in cases)

    per_cell_path = output_root / "per_cell_slot_attribution.jsonl"
    summary_path = output_root / "slot_attribution_summary.json"
    manifest_path = output_root / "low_signal_slot_attribution_manifest.json"
    readback_path = output_root / "manual_readback_summary.json"
    hashes_path = output_root / "artifact_hashes.json"
    state_summary_path = state_root / "slot_attribution_summary.json"
    state_per_cell_path = state_root / "per_cell_slot_attribution.jsonl"

    per_cell_count = write_jsonl(per_cell_path, per_cell_rows)
    write_jsonl(state_per_cell_path, per_cell_rows)
    summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "low_signal_slot_attribution_summary",
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "summary_rows": summary_rows,
        "diagnostics": diagnostics,
        "decision_limits": {
            "phase_2_only": True,
            "holdout_corr_delta_when_dropped_not_measured": True,
            "temporal_slots_require_live_variance": True,
            "active_slot_replacement_requires_phase_1_confirmation": True,
        },
    }
    write_json(summary_path, summary)
    write_json(state_summary_path, summary)
    source_of_truth = {
        "output_root": str(output_root),
        "state_root": str(state_root),
        "dataset_root": str(args.dataset_root),
        "dataset_npz": str(dataset.dataset_npz),
        "dataset_rows_jsonl": str(dataset.rows_path),
        "per_cell_slot_attribution_jsonl": str(per_cell_path),
        "slot_attribution_summary_json": str(summary_path),
        "state_per_cell_slot_attribution_jsonl": str(state_per_cell_path),
        "state_slot_attribution_summary_json": str(state_summary_path),
        "manifest_json": str(manifest_path),
        "manual_readback_summary_json": str(readback_path),
        "artifact_hashes_json": str(hashes_path),
    }
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "passes": all_passed,
        "allPassed": all_passed,
        "sourceOfTruth": source_of_truth,
        "trigger": " ".join(sys.argv),
        "expected": "Phase 2 per-cell slot-attribution lift over #759 slot-sliced panel dataset; no Phase 1 retraining ablation claim",
        "actual": {
            "candidate_slots": [slot.upper() for slot in candidate_slots],
            "measured_slots": [row["slot"] for row in summary_rows if row.get("measured") is True],
            "unmeasured_slots": [row["slot"] for row in summary_rows if row.get("measured") is False],
            "per_cell_row_count": per_cell_count,
            "summary_rows": summary_rows,
        },
        "cases": cases,
        "doctrine": doctrine,
        "probe": {
            "estimator": "per_slot_random_projection_plus_ridge_linear_probe",
            "seed": args.seed,
            "holdout_fraction": args.holdout_fraction,
            "ridge_alpha": args.ridge_alpha,
            "train_row_count": len(probe.train_idx),
            "holdout_row_count": len(probe.holdout_idx),
            "reduction": probe.reduction,
        },
        "source_artifacts": {
            "dataset_manifest": {
                "path": str(args.dataset_root / "dataset_manifest.json"),
                "sha256": sha256_file(args.dataset_root / "dataset_manifest.json"),
            },
            "dataset_npz": {"path": str(dataset.dataset_npz), "sha256": sha256_file(dataset.dataset_npz)},
            "dataset_rows_jsonl": {"path": str(dataset.rows_path), "sha256": sha256_file(dataset.rows_path)},
        },
        "before": before,
        "after": {"output_root": tree_state(output_root), "state_root": tree_state(state_root)},
    }
    write_json(manifest_path, manifest)
    readback = {
        "passes": all_passed,
        "allPassed": all_passed,
        "run_id": run_id,
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "summary_path": str(summary_path),
        "summary_sha256": sha256_file(summary_path),
        "per_cell_path": str(per_cell_path),
        "per_cell_sha256": sha256_file(per_cell_path),
        "per_cell_row_count": per_cell_count,
        "summary_rows": summary_rows,
        "cases": cases,
    }
    write_json(readback_path, readback)
    hashes = artifact_hashes(
        {
            "low_signal_slot_attribution_manifest.json": manifest_path,
            "manual_readback_summary.json": readback_path,
            "per_cell_slot_attribution.jsonl": per_cell_path,
            "slot_attribution_summary.json": summary_path,
            "state_per_cell_slot_attribution.jsonl": state_per_cell_path,
            "state_slot_attribution_summary.json": state_summary_path,
        }
    )
    write_json(hashes_path, hashes)
    if not all_passed:
        raise SlotAttributionError(f"SLOT_ATTR_VALIDATION_FAILED: {manifest_path}")
    print(json.dumps(readback, indent=2, sort_keys=True))
    return readback


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--state-root", type=Path)
    parser.add_argument("--run-id", default=f"run-{utc_stamp()}")
    parser.add_argument("--candidate-slot", action="append", default=list(DEFAULT_CANDIDATE_SLOTS))
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--components-per-slot", type=int, default=32)
    parser.add_argument("--ridge-alpha", type=float, default=10.0)
    parser.add_argument("--seed", type=int, default=746002)
    parser.add_argument("--clean", action="store_true")
    parser.add_argument("--allow-clean-state-root", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_pipeline(args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SlotAttributionError as err:
        print(f"SLOT_ATTR_ERROR: {err}", file=sys.stderr)
        raise SystemExit(2)
