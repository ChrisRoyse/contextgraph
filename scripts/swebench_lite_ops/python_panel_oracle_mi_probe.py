#!/usr/bin/env python3
"""Run the INV-032 / #759 panel-sufficiency mutual-information probe.

The runner reads the #715 v2 static-AST forward cache, aggregates packed
per-slot chunk vectors into one slot-preserving code-state feature row per
`(task_instance_id, mutation_category, workspace_state)`, and estimates
`I(panel; oracle)` against the SWE-bench Lite oracle labels.

The command writes artifacts for manual source-of-truth readback. It never
modifies the forward cache.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import random
import sys
import time
from collections import Counter
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_FORWARD_ROOT = Path(
    "/zfs/archive/contextgraph/corpus/python-forward-cache-v2-static-ast-e7lang-fa703b89-20260523T204324Z"
)
DEFAULT_ORACLE_INDEX = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/inv-032-panel-sufficiency-mi-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/mi-audit/inv-032-v1")
ACTIVE_EMBEDDERS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
RETIRED_EMBEDDERS = ("e2", "e3", "e4")
EPS = 1.0e-12


class PanelMiError(RuntimeError):
    pass


@dataclass
class SlotAccumulator:
    dim: int
    count: int = 0
    sum: Any = None
    sumsq: Any = None


@dataclass
class CodeStateAccumulator:
    task_instance_id: str
    mutation_category: str
    workspace_state: str
    oracle_all_passed: bool
    repo: str = ""
    row_count: int = 0
    slots: dict[str, SlotAccumulator] = field(default_factory=dict)


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise PanelMiError(f"PANEL_MI_WRITE_READBACK_MISMATCH: {path}")


def write_json_checked(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    write_text_checked(path, "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def require(condition: bool, code: str, details: Any = None) -> None:
    if not condition:
        raise PanelMiError(f"{code}: {details}")


def require_aiwonder_path(path: Path, label: str) -> Path:
    text = str(path)
    if not (text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/")):
        raise PanelMiError(f"PANEL_MI_NOT_AIWONDER_PATH: {label}={path}")
    if "/mnt/c/" in text or "/mnt/d/" in text:
        raise PanelMiError(f"PANEL_MI_LEGACY_MOUNT_PATH: {label}={path}")
    return path


def load_oracle_index(path: Path) -> tuple[dict[tuple[str, str], bool], dict[str, Any]]:
    payload = read_json(path)
    entries = payload.get("entries")
    require(isinstance(entries, list) and entries, "PANEL_MI_ORACLE_INDEX_EMPTY", path)
    labels: dict[tuple[str, str], bool] = {}
    counts: Counter[str] = Counter()
    label_counts: Counter[str] = Counter()
    for row in entries:
        task_id = str(row.get("task_id") or "")
        category = str(row.get("category") or "")
        require(task_id and category, "PANEL_MI_ORACLE_ROW_KEY_MISSING", row)
        key = (task_id, category)
        label = bool(row.get("oracle_all_passed"))
        if key in labels and labels[key] != label:
            raise PanelMiError(f"PANEL_MI_ORACLE_LABEL_CONFLICT: {key}")
        labels[key] = label
        counts[category] += 1
        label_counts["pass" if label else "fail"] += 1
    return labels, {
        "path": str(path),
        "sha256": sha256_file(path),
        "entry_count": len(entries),
        "unique_task_category_count": len(labels),
        "category_counts": dict(sorted(counts.items())),
        "label_counts": dict(sorted(label_counts.items())),
    }


def discover_forward_cache(forward_root: Path, max_shards: int | None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require_aiwonder_path(forward_root, "forward_root")
    corpus_manifest_path = forward_root / "python_forward_cache_corpus_manifest.json"
    shard_plan_path = forward_root / "python_forward_cache_shard_plan.json"
    checkpoint_path = forward_root / "python_forward_cache_checkpoint.json"
    for path in (corpus_manifest_path, shard_plan_path, checkpoint_path):
        require(path.is_file(), "PANEL_MI_FORWARD_CACHE_FILE_MISSING", path)

    corpus_manifest = read_json(corpus_manifest_path)
    shard_plan = read_json(shard_plan_path)
    checkpoint = read_json(checkpoint_path)
    require(corpus_manifest.get("passes") is True, "PANEL_MI_CORPUS_MANIFEST_NOT_PASSING", corpus_manifest_path)
    require(checkpoint.get("passes") is True, "PANEL_MI_CHECKPOINT_NOT_PASSING", checkpoint_path)
    summary = corpus_manifest.get("summary") or {}
    active = tuple(summary.get("active_embedder_ids") or [])
    retired = tuple(summary.get("retired_embedder_ids") or [])
    require(active == ACTIVE_EMBEDDERS, "PANEL_MI_ACTIVE_EMBEDDER_SET_MISMATCH", active)
    require(retired == RETIRED_EMBEDDERS, "PANEL_MI_RETIRED_EMBEDDER_SET_MISMATCH", retired)
    shards = list(shard_plan.get("shards") or [])
    require(bool(shards), "PANEL_MI_SHARD_PLAN_EMPTY")
    if max_shards is not None:
        require(max_shards > 0, "PANEL_MI_MAX_SHARDS_INVALID", max_shards)
        shards = shards[:max_shards]

    return shards, {
        "forward_root": str(forward_root),
        "corpus_manifest_path": str(corpus_manifest_path),
        "corpus_manifest_sha256": sha256_file(corpus_manifest_path),
        "shard_plan_path": str(shard_plan_path),
        "shard_plan_sha256": sha256_file(shard_plan_path),
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "summary": summary,
        "selected_shard_count": len(shards),
        "full_shard_count": len(shard_plan.get("shards") or []),
    }


def code_key(task_instance_id: str, mutation_category: str, workspace_state: str) -> tuple[str, str, str]:
    return (task_instance_id, mutation_category, workspace_state)


def workspace_state_matches(category: str, workspace_state: str, selector: str) -> bool:
    if selector == "all":
        return True
    if selector == "candidate":
        expected = "test_patch" if category == "known_good" else f"mutation_{category}"
        return workspace_state == expected
    return workspace_state == selector


def vector_sha256(values: Any) -> str:
    import numpy as np

    arr = np.asarray(values, dtype="<f4")
    return sha256_bytes(arr.tobytes())


def update_slot(slot: SlotAccumulator, values: Any) -> None:
    import numpy as np

    vector = np.asarray(values, dtype=np.float32)
    require(vector.ndim == 1 and vector.size == slot.dim, "PANEL_MI_VECTOR_DIM_MISMATCH", (vector.size, slot.dim))
    require(bool(np.all(np.isfinite(vector))), "PANEL_MI_VECTOR_NONFINITE")
    if slot.sum is None:
        slot.sum = np.zeros(slot.dim, dtype=np.float64)
        slot.sumsq = np.zeros(slot.dim, dtype=np.float64)
    slot.sum += vector.astype(np.float64)
    slot.sumsq += np.square(vector.astype(np.float64))
    slot.count += 1


def ingest_forward_cache(
    *,
    shards: list[dict[str, Any]],
    oracle_labels: dict[tuple[str, str], bool],
    max_records_per_embedder: int | None,
    workspace_state_selector: str,
) -> tuple[dict[tuple[str, str, str], CodeStateAccumulator], dict[str, Any]]:
    accumulators: dict[tuple[str, str, str], CodeStateAccumulator] = {}
    counts: Counter[str] = Counter()
    category_counts: Counter[str] = Counter()
    workspace_counts: Counter[str] = Counter()
    slot_counts: Counter[str] = Counter()
    slot_dims: dict[str, int] = {}
    shard_rows: list[dict[str, Any]] = []
    samples: list[dict[str, Any]] = []

    for shard_idx, shard in enumerate(shards):
        shard_root = Path(str(shard.get("cache_root") or ""))
        require_aiwonder_path(shard_root, f"shard[{shard_idx}].cache_root")
        packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
        shard_manifest_path = shard_root / "python_forward_cache_manifest.json"
        require(packed_manifest_path.is_file(), "PANEL_MI_PACKED_MANIFEST_MISSING", packed_manifest_path)
        require(shard_manifest_path.is_file(), "PANEL_MI_SHARD_MANIFEST_MISSING", shard_manifest_path)
        packed_manifest = read_json(packed_manifest_path)
        shard_manifest = read_json(shard_manifest_path)
        require(packed_manifest.get("passes") is True, "PANEL_MI_PACKED_MANIFEST_NOT_PASSING", packed_manifest_path)
        require(shard_manifest.get("passes") is True, "PANEL_MI_SHARD_MANIFEST_NOT_PASSING", shard_manifest_path)
        per_embedder = packed_manifest.get("per_embedder") or {}
        require(tuple(sorted(per_embedder)) == ACTIVE_EMBEDDERS, "PANEL_MI_SHARD_ACTIVE_EMBEDDER_MISMATCH", shard_root)

        shard_record_count = 0
        for embedder_id in ACTIVE_EMBEDDERS:
            info = per_embedder[embedder_id]
            record_path = Path(str(info.get("record_path") or ""))
            require_aiwonder_path(record_path, f"{shard_root}:{embedder_id}.record_path")
            require(record_path.is_file(), "PANEL_MI_RECORD_PATH_MISSING", record_path)
            manifest_row_count = int(info.get("row_count") or 0)
            local_row_count = 0
            used_row_count = 0
            with record_path.open("r", encoding="utf-8") as handle:
                for line_no, line in enumerate(handle, start=1):
                    if not line.strip():
                        continue
                    local_row_count += 1
                    if max_records_per_embedder is not None and used_row_count >= max_records_per_embedder:
                        continue
                    record = json.loads(line)
                    key = record.get("key") or {}
                    chunk = record.get("chunk") or {}
                    output = record.get("output") or {}
                    actual_embedder = str(key.get("embedder_id") or "")
                    require(actual_embedder == embedder_id, "PANEL_MI_RECORD_EMBEDDER_MISMATCH", (record_path, line_no))
                    vector = output.get("vector")
                    require(isinstance(vector, list) and vector, "PANEL_MI_RECORD_VECTOR_MISSING", (record_path, line_no))
                    expected_sha = str(output.get("vector_sha256") or record.get("vector_sha256") or "")
                    observed_sha = vector_sha256(vector)
                    require(expected_sha == observed_sha, "PANEL_MI_VECTOR_SHA_MISMATCH", (record_path, line_no, expected_sha, observed_sha))

                    task_id = str(chunk.get("task_instance_id") or "")
                    category = str(chunk.get("mutation_category") or "")
                    workspace_state = str(chunk.get("workspace_state") or "")
                    require(task_id and category and workspace_state, "PANEL_MI_RECORD_CODE_KEY_MISSING", (record_path, line_no))
                    if not workspace_state_matches(category, workspace_state, workspace_state_selector):
                        counts["workspace_state_filter_skipped_records"] += 1
                        continue
                    oracle_key = (task_id, category)
                    if oracle_key not in oracle_labels:
                        counts["oracle_missing_skipped_records"] += 1
                        continue
                    ck = code_key(task_id, category, workspace_state)
                    if ck not in accumulators:
                        accumulators[ck] = CodeStateAccumulator(
                            task_instance_id=task_id,
                            mutation_category=category,
                            workspace_state=workspace_state,
                            oracle_all_passed=oracle_labels[oracle_key],
                            repo=str(chunk.get("repo") or ""),
                        )
                    acc = accumulators[ck]
                    dim = len(vector)
                    if embedder_id in slot_dims and slot_dims[embedder_id] != dim:
                        raise PanelMiError(f"PANEL_MI_SLOT_DIM_CONFLICT: {embedder_id} {slot_dims[embedder_id]} {dim}")
                    slot_dims.setdefault(embedder_id, dim)
                    slot = acc.slots.setdefault(embedder_id, SlotAccumulator(dim=dim))
                    update_slot(slot, vector)
                    acc.row_count += 1
                    used_row_count += 1
                    shard_record_count += 1
                    counts["used_records"] += 1
                    category_counts[category] += 1
                    workspace_counts[workspace_state] += 1
                    slot_counts[embedder_id] += 1
                    if len(samples) < 16:
                        samples.append(
                            {
                                "task_instance_id": task_id,
                                "mutation_category": category,
                                "workspace_state": workspace_state,
                                "repo": str(chunk.get("repo") or ""),
                                "embedder_id": embedder_id,
                                "dim": dim,
                                "vector_sha256": observed_sha,
                                "record_path": str(record_path),
                                "line_no": line_no,
                            }
                        )
            require(local_row_count == manifest_row_count, "PANEL_MI_RECORD_ROW_COUNT_MISMATCH", (record_path, local_row_count, manifest_row_count))
        shard_rows.append({"shard_id": shard.get("shard_id"), "used_record_count": shard_record_count})

    return accumulators, {
        "record_counts": dict(sorted(counts.items())),
        "category_record_counts": dict(sorted(category_counts.items())),
        "workspace_record_counts": dict(sorted(workspace_counts.items())),
        "slot_record_counts": dict(sorted(slot_counts.items())),
        "slot_dims": dict(sorted(slot_dims.items())),
        "shards": shard_rows,
        "samples": samples,
        "code_state_count": len(accumulators),
        "workspace_state_selector": workspace_state_selector,
        "workspace_state_policy": "candidate=test_patch for known_good, mutation_<category> for mutation rows",
    }


def finalize_dataset(
    *,
    accumulators: dict[tuple[str, str, str], CodeStateAccumulator],
    output_root: Path,
    state_root: Path,
    run_id: str,
    input_manifest: dict[str, Any],
    ingest_manifest: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, slice]]:
    import numpy as np

    rows = []
    feature_slices: dict[str, slice] = {}
    cursor = 0
    slot_dims = {embedder_id: int(ingest_manifest["slot_dims"][embedder_id]) for embedder_id in ACTIVE_EMBEDDERS}
    for embedder_id in ACTIVE_EMBEDDERS:
        width = slot_dims[embedder_id] * 2 + 1
        feature_slices[embedder_id] = slice(cursor, cursor + width)
        cursor += width
    feature_dim = cursor

    ordered = sorted(accumulators.values(), key=lambda acc: (acc.task_instance_id, acc.mutation_category, acc.workspace_state))
    require(bool(ordered), "PANEL_MI_DATASET_EMPTY")
    X = np.zeros((len(ordered), feature_dim), dtype=np.float32)
    y = np.zeros((len(ordered),), dtype=np.int64)
    category_counts: Counter[str] = Counter()
    workspace_counts: Counter[str] = Counter()
    label_counts: Counter[str] = Counter()
    missing_slot_rows: Counter[str] = Counter()
    metadata_rows: list[dict[str, Any]] = []

    for row_idx, acc in enumerate(ordered):
        y[row_idx] = 1 if acc.oracle_all_passed else 0
        category_counts[acc.mutation_category] += 1
        workspace_counts[acc.workspace_state] += 1
        label_counts["pass" if acc.oracle_all_passed else "fail"] += 1
        slot_counts: dict[str, int] = {}
        for embedder_id in ACTIVE_EMBEDDERS:
            sl = feature_slices[embedder_id]
            dim = slot_dims[embedder_id]
            slot = acc.slots.get(embedder_id)
            if slot is None or slot.count == 0:
                missing_slot_rows[embedder_id] += 1
                continue
            mean = slot.sum / float(slot.count)
            var = np.maximum((slot.sumsq / float(slot.count)) - np.square(mean), 0.0)
            std = np.sqrt(var)
            X[row_idx, sl.start : sl.start + dim] = mean.astype(np.float32)
            X[row_idx, sl.start + dim : sl.start + dim * 2] = std.astype(np.float32)
            X[row_idx, sl.stop - 1] = math.log1p(slot.count)
            slot_counts[embedder_id] = int(slot.count)
        metadata_rows.append(
            {
                "row_index": row_idx,
                "task_instance_id": acc.task_instance_id,
                "mutation_category": acc.mutation_category,
                "workspace_state": acc.workspace_state,
                "repo": acc.repo,
                "oracle_all_passed": acc.oracle_all_passed,
                "slot_counts": slot_counts,
                "present_embedder_count": len(slot_counts),
                "feature_sha256": sha256_bytes(X[row_idx].astype("<f4", copy=False).tobytes()),
            }
        )

    dataset_npz = output_root / "panel_oracle_dataset.npz"
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(dataset_npz, X=X, y=y)
    rows_path = output_root / "dataset_rows.jsonl"
    write_jsonl_checked(rows_path, metadata_rows)
    state_rows_path = state_root / "dataset_rows.jsonl"
    write_jsonl_checked(state_rows_path, metadata_rows)
    feature_slices_json = {
        embedder_id: {
            "start": sl.start,
            "stop": sl.stop,
            "mean_start": sl.start,
            "mean_stop": sl.start + slot_dims[embedder_id],
            "std_start": sl.start + slot_dims[embedder_id],
            "std_stop": sl.stop - 1,
            "count_feature_index": sl.stop - 1,
            "source_dim": slot_dims[embedder_id],
        }
        for embedder_id, sl in feature_slices.items()
    }
    manifest = {
        "artifact_kind": "inv032_panel_oracle_mi_dataset",
        "schema_version": 1,
        "run_id": run_id,
        "created_at_utc": utc_now(),
        "passes": True,
        "input_manifest": input_manifest,
        "ingest_manifest": ingest_manifest,
        "row_count": int(X.shape[0]),
        "feature_dim": int(X.shape[1]),
        "target_bits": entropy_bits(y.tolist()),
        "category_counts": dict(sorted(category_counts.items())),
        "workspace_state_counts": dict(sorted(workspace_counts.items())),
        "label_counts": dict(sorted(label_counts.items())),
        "missing_slot_rows": dict(sorted(missing_slot_rows.items())),
        "active_embedder_ids": list(ACTIVE_EMBEDDERS),
        "retired_embedder_ids": list(RETIRED_EMBEDDERS),
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "feature_construction": "per-code-state per-slot mean+std+log_count blocks; slot slices retained for drop-one ablation",
        "feature_slices": feature_slices_json,
        "artifacts": {
            "dataset_npz": str(dataset_npz),
            "dataset_npz_sha256": sha256_file(dataset_npz),
            "dataset_rows_jsonl": str(rows_path),
            "dataset_rows_jsonl_sha256": sha256_file(rows_path),
            "state_dataset_rows_jsonl": str(state_rows_path),
            "state_dataset_rows_jsonl_sha256": sha256_file(state_rows_path),
        },
    }
    write_json_checked(output_root / "dataset_manifest.json", manifest)
    write_json_checked(state_root / "dataset_manifest.json", manifest)
    return manifest, feature_slices


def entropy_bits(labels: list[int] | Any) -> float:
    values = [int(v) for v in labels]
    if not values:
        return 0.0
    counts = Counter(values)
    n = float(len(values))
    total = 0.0
    for count in counts.values():
        p = count / n
        if p > 0:
            total -= p * math.log2(p)
    return float(total)


def stratified_split(y: Any, holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    groups: dict[int, list[int]] = {}
    for idx, value in enumerate(y.tolist()):
        groups.setdefault(int(value), []).append(idx)
    rng = random.Random(seed)
    train: list[int] = []
    holdout: list[int] = []
    for _, indexes in sorted(groups.items()):
        rng.shuffle(indexes)
        n_holdout = max(1, int(round(len(indexes) * holdout_fraction))) if len(indexes) > 1 else 0
        holdout.extend(indexes[:n_holdout])
        train.extend(indexes[n_holdout:])
    rng.shuffle(train)
    rng.shuffle(holdout)
    return train, holdout


def standardize_and_reduce(X: Any, y: Any, max_components: int, seed: int) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    X64 = np.asarray(X, dtype=np.float64)
    mean = X64.mean(axis=0)
    std = X64.std(axis=0)
    keep = std > 1.0e-10
    Xs = (X64[:, keep] - mean[keep]) / std[keep]
    info: dict[str, Any] = {
        "input_dim": int(X.shape[1]),
        "nonconstant_dim": int(Xs.shape[1]),
        "method": "standardize_only",
        "output_dim": int(Xs.shape[1]),
    }
    if max_components > 0 and Xs.shape[1] > max_components and Xs.shape[0] > max_components + 2:
        try:
            from sklearn.decomposition import PCA

            pca = PCA(n_components=max_components, svd_solver="randomized", random_state=seed)
            Xr = pca.fit_transform(Xs).astype(np.float32)
            info.update(
                {
                    "method": "sklearn_randomized_pca",
                    "output_dim": int(Xr.shape[1]),
                    "explained_variance_ratio_sum": float(pca.explained_variance_ratio_.sum()),
                }
            )
            return Xr, info
        except Exception as err:
            info["pca_error"] = repr(err)
        rng = np.random.default_rng(seed)
        projection = rng.normal(0.0, 1.0 / math.sqrt(float(max_components)), size=(Xs.shape[1], max_components))
        Xr = (Xs @ projection).astype(np.float32)
        info.update(
            {
                "method": "numpy_gaussian_random_projection",
                "output_dim": int(Xr.shape[1]),
                "random_projection_seed": seed,
            }
        )
        return Xr, info
    return Xs.astype(np.float32), info


def mine_estimate(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    epochs: int,
    batch_size: int,
    learning_rate: float,
    hidden_dim: int,
    holdout_fraction: float,
    device_name: str,
) -> dict[str, Any]:
    import numpy as np
    import torch
    import torch.nn as nn
    import torch.nn.functional as F

    labels = [int(v) for v in y.tolist()]
    counts = Counter(labels)
    if len(counts) < 2:
        return {
            "label": label,
            "valid": False,
            "invalid_reason": "DEGENERATE_ORACLE_LABELS",
            "label_counts": dict(sorted(counts.items())),
        }
    if len(labels) < 32:
        return {
            "label": label,
            "valid": False,
            "invalid_reason": "LOW_SUPPORT",
            "label_counts": dict(sorted(counts.items())),
        }
    torch.manual_seed(seed)
    np.random.seed(seed)
    random.seed(seed)
    device = torch.device(device_name if device_name != "auto" else ("cuda" if torch.cuda.is_available() else "cpu"))
    Xr, reduction = standardize_and_reduce(X, y, max_components=min(256, max(8, len(labels) // 3)), seed=seed)
    train_idx, val_idx = stratified_split(y, holdout_fraction, seed)
    if not train_idx or not val_idx:
        return {
            "label": label,
            "valid": False,
            "invalid_reason": "SPLIT_FAILED",
            "label_counts": dict(sorted(counts.items())),
        }
    x_train = torch.tensor(Xr[train_idx], dtype=torch.float32, device=device)
    y_train = torch.tensor(np.asarray(y)[train_idx].reshape(-1, 1), dtype=torch.float32, device=device)
    x_val = torch.tensor(Xr[val_idx], dtype=torch.float32, device=device)
    y_val = torch.tensor(np.asarray(y)[val_idx].reshape(-1, 1), dtype=torch.float32, device=device)
    input_dim = x_train.shape[1] + 1

    class MineNet(nn.Module):
        def __init__(self) -> None:
            super().__init__()
            self.net = nn.Sequential(
                nn.Linear(input_dim, hidden_dim),
                nn.ELU(),
                nn.Linear(hidden_dim, hidden_dim),
                nn.ELU(),
                nn.Linear(hidden_dim, 1),
            )

        def forward(self, x_tensor: Any, y_tensor: Any) -> Any:
            return self.net(torch.cat([x_tensor, y_tensor], dim=1))

    model = MineNet().to(device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=learning_rate, weight_decay=1.0e-4)
    history: list[dict[str, Any]] = []
    n_train = x_train.shape[0]
    effective_batch = min(batch_size, n_train)
    if effective_batch < 8:
        return {
            "label": label,
            "valid": False,
            "invalid_reason": "BATCH_TOO_SMALL",
            "label_counts": dict(sorted(counts.items())),
        }

    def bound_bits(x_tensor: Any, y_tensor: Any) -> float:
        model.eval()
        with torch.no_grad():
            t_joint = model(x_tensor, y_tensor)
            perm = torch.randperm(y_tensor.shape[0], device=device)
            t_marg = model(x_tensor, y_tensor[perm])
            dv = t_joint.mean() - torch.logsumexp(t_marg.squeeze(1), dim=0) + math.log(float(y_tensor.shape[0]))
            return float((dv / math.log(2.0)).detach().cpu().item())

    best_val = -1.0e9
    best_epoch = -1
    start = time.time()
    for epoch in range(1, epochs + 1):
        model.train()
        perm = torch.randperm(n_train, device=device)
        epoch_losses = []
        for start_idx in range(0, n_train, effective_batch):
            batch_idx = perm[start_idx : start_idx + effective_batch]
            if batch_idx.shape[0] < 8:
                continue
            xb = x_train[batch_idx]
            yb = y_train[batch_idx]
            y_perm = yb[torch.randperm(yb.shape[0], device=device)]
            t_joint = model(xb, yb)
            t_marg = model(xb, y_perm).squeeze(1)
            dv = t_joint.mean() - torch.logsumexp(t_marg, dim=0) + math.log(float(yb.shape[0]))
            loss = -dv
            if not torch.isfinite(loss):
                raise PanelMiError(f"PANEL_MI_MINE_LOSS_NONFINITE: {label} epoch={epoch}")
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 5.0)
            optimizer.step()
            epoch_losses.append(float(loss.detach().cpu().item()))
        if epoch == 1 or epoch % max(1, epochs // 20) == 0 or epoch == epochs:
            train_bits = bound_bits(x_train, y_train)
            val_bits = bound_bits(x_val, y_val)
            best_val = max(best_val, val_bits)
            if best_val == val_bits:
                best_epoch = epoch
            history.append(
                {
                    "epoch": epoch,
                    "mean_loss": float(np.mean(epoch_losses)) if epoch_losses else None,
                    "train_bound_bits": train_bits,
                    "validation_bound_bits": val_bits,
                }
            )

    final_train = bound_bits(x_train, y_train)
    final_val = bound_bits(x_val, y_val)
    target_bits = entropy_bits(labels)
    return {
        "label": label,
        "valid": True,
        "estimator": "MINE_DV",
        "seed": seed,
        "device": str(device),
        "sample_count": len(labels),
        "train_count": len(train_idx),
        "validation_count": len(val_idx),
        "input_feature_dim": int(X.shape[1]),
        "reduction": reduction,
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": target_bits,
        "final_train_lower_bound_bits": final_train,
        "final_validation_lower_bound_bits": final_val,
        "best_validation_lower_bound_bits": best_val,
        "best_epoch": best_epoch,
        "bounded_final_validation_bits": max(0.0, min(target_bits, final_val)),
        "bounded_best_validation_bits": max(0.0, min(target_bits, best_val)),
        "epochs": epochs,
        "batch_size": batch_size,
        "learning_rate": learning_rate,
        "hidden_dim": hidden_dim,
        "elapsed_seconds": time.time() - start,
        "history": history,
    }


def classifier_lower_bound(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    holdout_fraction: float,
) -> dict[str, Any]:
    import numpy as np

    labels = [int(v) for v in y.tolist()]
    counts = Counter(labels)
    if len(counts) < 2:
        return {"label": label, "valid": False, "invalid_reason": "DEGENERATE_ORACLE_LABELS", "label_counts": dict(sorted(counts.items()))}
    try:
        from sklearn.linear_model import LogisticRegression
        from sklearn.metrics import log_loss, roc_auc_score
    except Exception as err:
        return torch_classifier_lower_bound(
            X=X,
            y=y,
            label=label,
            seed=seed,
            holdout_fraction=holdout_fraction,
            sklearn_error=repr(err),
        )
    Xr, reduction = standardize_and_reduce(X, y, max_components=min(256, max(8, len(labels) // 3)), seed=seed)
    train_idx, val_idx = stratified_split(y, holdout_fraction, seed)
    model = LogisticRegression(max_iter=2000, class_weight="balanced", random_state=seed)
    model.fit(Xr[train_idx], np.asarray(y)[train_idx])
    probs = model.predict_proba(Xr[val_idx])[:, 1]
    ce_nats = float(log_loss(np.asarray(y)[val_idx], probs, labels=[0, 1], normalize=True))
    ce_bits = ce_nats / math.log(2.0)
    h_bits = entropy_bits(labels)
    lower_bound = max(0.0, min(h_bits, h_bits - ce_bits))
    try:
        auc = float(roc_auc_score(np.asarray(y)[val_idx], probs))
    except Exception:
        auc = None
    return {
        "label": label,
        "valid": True,
        "estimator": "heldout_logistic_cross_entropy_lower_bound",
        "seed": seed,
        "sample_count": len(labels),
        "train_count": len(train_idx),
        "validation_count": len(val_idx),
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": h_bits,
        "cross_entropy_bits": ce_bits,
        "lower_bound_bits": lower_bound,
        "validation_auc": auc,
        "reduction": reduction,
    }


def torch_classifier_lower_bound(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    holdout_fraction: float,
    sklearn_error: str,
) -> dict[str, Any]:
    import numpy as np
    import torch
    import torch.nn as nn
    import torch.nn.functional as F

    labels = [int(v) for v in y.tolist()]
    counts = Counter(labels)
    Xr, reduction = standardize_and_reduce(X, y, max_components=min(256, max(8, len(labels) // 3)), seed=seed)
    train_idx, val_idx = stratified_split(y, holdout_fraction, seed)
    if not train_idx or not val_idx:
        return {
            "label": label,
            "valid": False,
            "invalid_reason": "SPLIT_FAILED",
            "label_counts": dict(sorted(counts.items())),
            "sklearn_error": sklearn_error,
        }
    torch.manual_seed(seed)
    np.random.seed(seed)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    x_train = torch.tensor(Xr[train_idx], dtype=torch.float32, device=device)
    y_train = torch.tensor(np.asarray(y)[train_idx].reshape(-1, 1), dtype=torch.float32, device=device)
    x_val = torch.tensor(Xr[val_idx], dtype=torch.float32, device=device)
    y_val = torch.tensor(np.asarray(y)[val_idx].reshape(-1, 1), dtype=torch.float32, device=device)
    model = nn.Linear(x_train.shape[1], 1).to(device)
    positives = float(y_train.sum().detach().cpu().item())
    negatives = float(y_train.shape[0] - positives)
    pos_weight = torch.tensor([max(1.0, negatives / max(1.0, positives))], dtype=torch.float32, device=device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=1.0e-2, weight_decay=1.0e-4)
    history: list[dict[str, Any]] = []
    for epoch in range(1, 1601):
        model.train()
        logits = model(x_train)
        loss = F.binary_cross_entropy_with_logits(logits, y_train, pos_weight=pos_weight)
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        optimizer.step()
        if epoch in (1, 100, 200, 400, 800, 1200, 1600):
            model.eval()
            with torch.no_grad():
                val_logits = model(x_val)
                val_loss = F.binary_cross_entropy_with_logits(val_logits, y_val)
                history.append(
                    {
                        "epoch": epoch,
                        "train_weighted_bce": float(loss.detach().cpu().item()),
                        "validation_bce": float(val_loss.detach().cpu().item()),
                    }
                )
    model.eval()
    with torch.no_grad():
        probs = torch.sigmoid(model(x_val))
        ce_nats = float(F.binary_cross_entropy(probs, y_val).detach().cpu().item())
    ce_bits = ce_nats / math.log(2.0)
    h_bits = entropy_bits(labels)
    lower_bound = max(0.0, min(h_bits, h_bits - ce_bits))
    return {
        "label": label,
        "valid": True,
        "estimator": "torch_linear_cross_entropy_lower_bound",
        "seed": seed,
        "device": str(device),
        "sample_count": len(labels),
        "train_count": len(train_idx),
        "validation_count": len(val_idx),
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": h_bits,
        "cross_entropy_bits": ce_bits,
        "lower_bound_bits": lower_bound,
        "validation_auc": None,
        "reduction": reduction,
        "sklearn_error": sklearn_error,
        "history": history,
    }


def kraskov_crosscheck(*, X: Any, y: Any, label: str, seed: int, n_neighbors: int) -> dict[str, Any]:
    import numpy as np

    labels = [int(v) for v in y.tolist()]
    counts = Counter(labels)
    if len(counts) < 2:
        return {"label": label, "valid": False, "invalid_reason": "DEGENERATE_ORACLE_LABELS", "label_counts": dict(sorted(counts.items()))}
    try:
        from sklearn.feature_selection import mutual_info_classif
    except Exception as err:
        return ross_kraskov_fallback(X=X, y=y, label=label, seed=seed, n_neighbors=n_neighbors, sklearn_error=repr(err))
    Xr, reduction = standardize_and_reduce(X, y, max_components=min(128, max(8, len(labels) // 4)), seed=seed)
    mi_nats = mutual_info_classif(Xr, np.asarray(y), discrete_features=False, n_neighbors=n_neighbors, random_state=seed)
    h_bits = entropy_bits(labels)
    per_component_bits = [float(v / math.log(2.0)) for v in mi_nats.tolist()]
    summed = float(sum(v for v in per_component_bits if math.isfinite(v)))
    return {
        "label": label,
        "valid": True,
        "estimator": "sklearn_ross_kraskov_per_component_crosscheck",
        "note": "Per-component Ross/Kraskov estimate on PCA components; not a joint multivariate MI pass/fail statistic.",
        "seed": seed,
        "sample_count": len(labels),
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": h_bits,
        "per_component_sum_bits": summed,
        "per_component_sum_capped_to_target_bits": max(0.0, min(h_bits, summed)),
        "max_component_bits": max(per_component_bits) if per_component_bits else 0.0,
        "nonzero_component_count": sum(1 for value in per_component_bits if value > 0.0),
        "n_neighbors": n_neighbors,
        "reduction": reduction,
    }


def digamma_approx(value: float) -> float:
    try:
        from scipy.special import digamma

        return float(digamma(value))
    except Exception:
        x = float(value)
        result = 0.0
        while x < 8.0:
            result -= 1.0 / x
            x += 1.0
        inv = 1.0 / x
        inv2 = inv * inv
        return result + math.log(x) - 0.5 * inv - inv2 / 12.0 + inv2 * inv2 / 120.0


def radius_counts_chebyshev(X: Any, points: Any, radii: Any) -> Any:
    import numpy as np

    try:
        from scipy.spatial import cKDTree

        tree = cKDTree(X)
        counts = tree.query_ball_point(points, radii, p=np.inf, return_length=True)
        return np.asarray(counts, dtype=np.int64)
    except Exception:
        counts = []
        chunk = 128
        for start in range(0, points.shape[0], chunk):
            block = points[start : start + chunk]
            block_radii = radii[start : start + chunk]
            distances = np.max(np.abs(block[:, None, :] - X[None, :, :]), axis=2)
            counts.extend((distances <= block_radii[:, None]).sum(axis=1).tolist())
        return np.asarray(counts, dtype=np.int64)


def ross_kraskov_fallback(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    n_neighbors: int,
    sklearn_error: str,
) -> dict[str, Any]:
    import numpy as np

    labels = np.asarray(y, dtype=np.int64)
    counts = Counter(labels.tolist())
    if len(counts) < 2:
        return {"label": label, "valid": False, "invalid_reason": "DEGENERATE_ORACLE_LABELS", "label_counts": dict(sorted(counts.items()))}
    min_class = min(counts.values())
    if min_class <= 1:
        return {"label": label, "valid": False, "invalid_reason": "LOW_CLASS_SUPPORT_FOR_KNN", "label_counts": dict(sorted(counts.items()))}
    k = min(n_neighbors, min_class - 1)
    Xr, reduction = standardize_and_reduce(X, y, max_components=min(64, max(4, len(labels) // 8)), seed=seed)
    Xr = np.asarray(Xr, dtype=np.float64)
    n = Xr.shape[0]
    radii = np.zeros(n, dtype=np.float64)
    local_k = np.zeros(n, dtype=np.int64)
    label_cardinality = np.zeros(n, dtype=np.int64)

    for class_value, class_count in sorted(counts.items()):
        idx = np.where(labels == class_value)[0]
        class_points = Xr[idx]
        class_k = min(k, len(idx) - 1)
        label_cardinality[idx] = len(idx)
        local_k[idx] = class_k
        try:
            from scipy.spatial import cKDTree

            tree = cKDTree(class_points)
            dists, _ = tree.query(class_points, k=class_k + 1, p=np.inf)
            kth = dists[:, -1]
        except Exception:
            distances = np.max(np.abs(class_points[:, None, :] - class_points[None, :, :]), axis=2)
            distances.sort(axis=1)
            kth = distances[:, class_k]
        radii[idx] = np.nextafter(kth, 0.0)

    neighbor_counts = radius_counts_chebyshev(Xr, Xr, radii)
    neighbor_counts = np.maximum(neighbor_counts - 1, 1)
    mi_nats = (
        digamma_approx(float(n))
        + float(np.mean([digamma_approx(float(v)) for v in local_k]))
        - float(np.mean([digamma_approx(float(v)) for v in label_cardinality]))
        - float(np.mean([digamma_approx(float(v)) for v in neighbor_counts]))
    )
    mi_bits = max(0.0, mi_nats / math.log(2.0))
    h_bits = entropy_bits(labels.tolist())
    return {
        "label": label,
        "valid": True,
        "estimator": "ross_kraskov_discrete_continuous_fallback",
        "note": "Approximate Ross/Kraskov kNN estimate after deterministic random projection; used because sklearn is unavailable in the aiwonder venv.",
        "seed": seed,
        "sample_count": int(n),
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": h_bits,
        "joint_estimate_bits": mi_bits,
        "joint_estimate_capped_to_target_bits": max(0.0, min(h_bits, mi_bits)),
        "n_neighbors": int(k),
        "reduction": reduction,
        "sklearn_error": sklearn_error,
    }


def feature_subset(X: Any, feature_slices: dict[str, slice], drop_embedder: str | None) -> Any:
    import numpy as np

    if drop_embedder is None:
        return X
    sl = feature_slices[drop_embedder]
    keep = np.ones((X.shape[1],), dtype=bool)
    keep[sl.start : sl.stop] = False
    return X[:, keep]


def load_dataset(output_root: Path) -> tuple[Any, Any, list[dict[str, Any]], dict[str, slice], dict[str, Any]]:
    import numpy as np

    manifest = read_json(output_root / "dataset_manifest.json")
    data = np.load(output_root / "panel_oracle_dataset.npz")
    rows = [json.loads(line) for line in (output_root / "dataset_rows.jsonl").read_text(encoding="utf-8").splitlines() if line.strip()]
    slices = {
        embedder_id: slice(int(info["start"]), int(info["stop"]))
        for embedder_id, info in (manifest.get("feature_slices") or {}).items()
    }
    return data["X"], data["y"], rows, slices, manifest


def run_estimators(args: argparse.Namespace, output_root: Path, state_root: Path) -> dict[str, Any]:
    import numpy as np

    X, y, rows, feature_slices, dataset_manifest = load_dataset(output_root)
    configs: list[tuple[str, Any, Any]] = [("global", X, y)]
    for embedder_id in ACTIVE_EMBEDDERS:
        configs.append((f"drop_one:{embedder_id}", feature_subset(X, feature_slices, embedder_id), y))
    categories = sorted({str(row["mutation_category"]) for row in rows})
    for category in categories:
        idx = [int(row["row_index"]) for row in rows if row["mutation_category"] == category]
        configs.append((f"cell:{category}", X[idx, :], y[idx]))

    mine_rows: list[dict[str, Any]] = []
    classifier_rows: list[dict[str, Any]] = []
    kraskov_rows: list[dict[str, Any]] = []
    existing_mine: dict[str, dict[str, Any]] = {}
    existing_mine_path = output_root / "mine_estimates.jsonl"
    if args.skip_mine and existing_mine_path.is_file():
        for line in existing_mine_path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                row = json.loads(line)
                existing_mine[str(row.get("label"))] = row
    for label, X_sub, y_sub in configs:
        if args.skip_mine:
            mine_rows.append(existing_mine.get(label, {"label": label, "valid": False, "invalid_reason": "SKIPPED"}))
        else:
            mine_rows.append(
                mine_estimate(
                    X=X_sub,
                    y=y_sub,
                    label=label,
                    seed=args.seed,
                    epochs=args.epochs,
                    batch_size=args.batch_size,
                    learning_rate=args.learning_rate,
                    hidden_dim=args.hidden_dim,
                    holdout_fraction=args.holdout_fraction,
                    device_name=args.device,
                )
            )
        classifier_rows.append(classifier_lower_bound(X=X_sub, y=y_sub, label=label, seed=args.seed, holdout_fraction=args.holdout_fraction))
        kraskov_rows.append(kraskov_crosscheck(X=X_sub, y=y_sub, label=label, seed=args.seed, n_neighbors=args.kraskov_neighbors))
        write_json_checked(output_root / "partial_estimator_readback.json", {"updated_at_utc": utc_now(), "completed_labels": [row["label"] for row in mine_rows]})

    global_mine = next(row for row in mine_rows if row["label"] == "global")
    global_classifier = next(row for row in classifier_rows if row["label"] == "global")
    drop_rows = [row for row in mine_rows if row["label"].startswith("drop_one:")]
    global_bits = global_mine.get("bounded_best_validation_bits") if global_mine.get("valid") else None
    drop_summary = []
    for row in drop_rows:
        bits = row.get("bounded_best_validation_bits") if row.get("valid") else None
        drop_summary.append(
            {
                "embedder_id": row["label"].split(":", 1)[1],
                "drop_one_bounded_best_bits": bits,
                "delta_from_global_bits": (global_bits - bits) if global_bits is not None and bits is not None else None,
                "valid": row.get("valid"),
                "invalid_reason": row.get("invalid_reason"),
            }
        )

    write_jsonl_checked(output_root / "mine_estimates.jsonl", mine_rows)
    write_jsonl_checked(output_root / "classifier_lower_bounds.jsonl", classifier_rows)
    write_jsonl_checked(output_root / "kraskov_crosschecks.jsonl", kraskov_rows)
    write_jsonl_checked(state_root / "mine_estimates.jsonl", mine_rows)
    write_jsonl_checked(state_root / "classifier_lower_bounds.jsonl", classifier_rows)
    write_jsonl_checked(state_root / "kraskov_crosschecks.jsonl", kraskov_rows)
    manifest = {
        "artifact_kind": "inv032_panel_oracle_mi_estimator_manifest",
        "schema_version": 1,
        "created_at_utc": utc_now(),
        "passes": True,
        "dataset_manifest_sha256": sha256_file(output_root / "dataset_manifest.json"),
        "global_mine": global_mine,
        "global_classifier_lower_bound": global_classifier,
        "drop_one_mine_summary": drop_summary,
        "cell_mine_summary": [
            {
                "cell": row["label"].split(":", 1)[1],
                "valid": row.get("valid"),
                "bounded_best_validation_bits": row.get("bounded_best_validation_bits"),
                "target_entropy_bits": row.get("target_entropy_bits"),
                "invalid_reason": row.get("invalid_reason"),
                "label_counts": row.get("label_counts"),
            }
            for row in mine_rows
            if row["label"].startswith("cell:")
        ],
        "decision_rule": {
            "target_bits": 0.95,
            "panel_sufficient": bool(global_bits is not None and global_bits >= 0.95),
            "classification": (
                "panel_sufficient"
                if global_bits is not None and global_bits >= 0.95
                else "marginal_or_insufficient"
                if global_bits is not None and global_bits >= 0.7
                else "panel_provably_insufficient_or_estimator_lower_bound_low"
                if global_bits is not None
                else "unknown"
            ),
        },
        "artifacts": {
            "mine_estimates_jsonl": str(output_root / "mine_estimates.jsonl"),
            "mine_estimates_sha256": sha256_file(output_root / "mine_estimates.jsonl"),
            "classifier_lower_bounds_jsonl": str(output_root / "classifier_lower_bounds.jsonl"),
            "classifier_lower_bounds_sha256": sha256_file(output_root / "classifier_lower_bounds.jsonl"),
            "kraskov_crosschecks_jsonl": str(output_root / "kraskov_crosschecks.jsonl"),
            "kraskov_crosschecks_sha256": sha256_file(output_root / "kraskov_crosschecks.jsonl"),
            "state_root": str(state_root),
        },
        "dataset_row_count": dataset_manifest.get("row_count"),
        "dataset_feature_dim": dataset_manifest.get("feature_dim"),
    }
    write_json_checked(output_root / "panel_oracle_mi_manifest.json", manifest)
    write_json_checked(state_root / "panel_oracle_mi_manifest.json", manifest)
    return manifest


def synthetic_self_test(output_root: Path) -> None:
    import numpy as np

    rng = np.random.default_rng(7)
    y = rng.integers(0, 2, size=256, dtype=np.int64)
    x_signal = y.astype(np.float32).reshape(-1, 1) + rng.normal(0.0, 0.05, size=(256, 1)).astype(np.float32)
    X = np.concatenate([x_signal, rng.normal(0.0, 1.0, size=(256, 15)).astype(np.float32)], axis=1)
    weak_y = rng.integers(0, 2, size=256, dtype=np.int64)
    weak_X = rng.normal(0.0, 1.0, size=(256, 16)).astype(np.float32)
    strong = classifier_lower_bound(X=X, y=y, label="synthetic_signal", seed=11, holdout_fraction=0.25)
    weak = classifier_lower_bound(X=weak_X, y=weak_y, label="synthetic_noise", seed=11, holdout_fraction=0.25)
    require(strong.get("valid") is True, "PANEL_MI_SELF_TEST_SIGNAL_INVALID", strong)
    require(weak.get("valid") is True, "PANEL_MI_SELF_TEST_NOISE_INVALID", weak)
    require(float(strong["lower_bound_bits"]) > 0.45, "PANEL_MI_SELF_TEST_SIGNAL_TOO_LOW", strong)
    require(float(weak["lower_bound_bits"]) < 0.35, "PANEL_MI_SELF_TEST_NOISE_TOO_HIGH", weak)
    write_json_checked(output_root / "self_test_readback.json", {"passes": True, "strong": strong, "weak": weak})


def artifact_hashes(root: Path) -> dict[str, Any]:
    rows = []
    for path in sorted(root.rglob("*")):
        if path.is_file():
            rows.append({"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)})
    return {"root": str(root), "file_count": len(rows), "byte_count": sum(row["bytes"] for row in rows), "files": rows}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--oracle-index", type=Path, default=DEFAULT_ORACLE_INDEX)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-root", type=Path)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default=f"run-{utc_stamp()}")
    parser.add_argument("--max-shards", type=int)
    parser.add_argument("--max-records-per-embedder", type=int)
    parser.add_argument(
        "--workspace-state",
        default="candidate",
        help="Code-state selector: candidate, all, or an exact workspace_state.",
    )
    parser.add_argument("--dataset-only", action="store_true")
    parser.add_argument("--estimators-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--skip-mine", action="store_true")
    parser.add_argument("--epochs", type=int, default=1200)
    parser.add_argument("--batch-size", type=int, default=256)
    parser.add_argument("--learning-rate", type=float, default=1.0e-4)
    parser.add_argument("--hidden-dim", type=int, default=512)
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--seed", type=int, default=759032)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--kraskov-neighbors", type=int, default=5)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    output_root = args.output_root or (args.output_parent / args.run_id)
    state_root = args.state_root or (args.state_parent / args.run_id)
    require_aiwonder_path(output_root, "output_root")
    require_aiwonder_path(state_root, "state_root")
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)

    if args.self_test:
        synthetic_self_test(output_root)
        write_json_checked(output_root / "artifact_hashes.json", artifact_hashes(output_root))
        return 0

    if not args.estimators_only:
        require_aiwonder_path(args.forward_root, "forward_root")
        require_aiwonder_path(args.oracle_index, "oracle_index")
        oracle_labels, oracle_manifest = load_oracle_index(args.oracle_index)
        shards, forward_manifest = discover_forward_cache(args.forward_root, args.max_shards)
        accumulators, ingest_manifest = ingest_forward_cache(
            shards=shards,
            oracle_labels=oracle_labels,
            max_records_per_embedder=args.max_records_per_embedder,
            workspace_state_selector=args.workspace_state,
        )
        input_manifest = {
            "oracle": oracle_manifest,
            "forward_cache": forward_manifest,
        }
        finalize_dataset(
            accumulators=accumulators,
            output_root=output_root,
            state_root=state_root,
            run_id=args.run_id,
            input_manifest=input_manifest,
            ingest_manifest=ingest_manifest,
        )

    if not args.dataset_only:
        run_estimators(args, output_root, state_root)

    write_json_checked(output_root / "artifact_hashes.json", artifact_hashes(output_root))
    write_json_checked(state_root / "artifact_hashes.json", artifact_hashes(state_root))
    write_json_checked(
        output_root / "run_status.json",
        {
            "status": "complete",
            "updated_at_utc": utc_now(),
            "output_root": str(output_root),
            "state_root": str(state_root),
        },
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except PanelMiError as err:
        print(f"PANEL_MI_ERROR: {err}", file=sys.stderr)
        raise SystemExit(2)
