#!/usr/bin/env python3
"""Screen candidate embedder/vector sources by panel-oracle MI contribution.

This is the #844 candidate-selection harness. It consumes a completed
slot-identity-preserved #759-style panel/oracle dataset plus one or more
candidate feature-row JSONL files, then reports:

* standalone candidate MI lower bounds,
* baseline-panel plus candidate delta-MI lower bounds,
* per-cell candidate and augmented lower bounds,
* novelty against the current slots, and
* a greedy assembly order subject to the Tier-6 utility/novelty gates.

The output is measurement evidence only. It does not promote a predictor,
change panel schema, or write model/checkpoint state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import random
import re
import shutil
from collections import Counter
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from .python_panel_oracle_mi_probe import (
        classifier_lower_bound,
        entropy_bits,
        kraskov_crosscheck,
        mine_estimate,
    )
    from .python_patch_lexical_baseline import load_dataset
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_panel_oracle_mi_probe import (  # type: ignore
        classifier_lower_bound,
        entropy_bits,
        kraskov_crosscheck,
        mine_estimate,
    )
    from swebench_lite_ops.python_patch_lexical_baseline import load_dataset  # type: ignore


TASK_ID = "INV-032"
CANONICAL_ISSUE = "#844"
ARTIFACT_KIND = "embedder_candidate_mi_screen"
SCHEMA_VERSION = 1
FORMULA_VERSION = "candidate_delta_mi_v1"
AIWONDER_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/embedder-candidate-screening-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/embedder-candidate-screening/inv-032-v1")
DEFAULT_DATASET_ROOT = Path(
    "/zfs/archive/contextgraph/state/mi-audit/inv-032-v1/"
    "run-20260529T234900Z/full-candidate-map"
)
BEHAVIORAL_CELLS = ("compile_error", "delete_test_call", "off_by_one", "over_engineer", "subtle_flip", "wrong_file")
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "completed_ids",
    "completed_instances",
    "error_ids",
    "error_instances",
    "label",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_label",
    "oracle_outcome",
    "oracle_passed",
    "oracle_verdict_sha256",
    "resolved_ids",
    "resolved_instances",
    "target",
    "unresolved_ids",
    "unresolved_instances",
    "y",
}
DIRECT_OUTCOME_KEYS = {
    "captured_exception",
    "execution_outcome",
    "pytest_args",
    "pytest_exit_code",
}
FORBIDDEN_FEATURE_KEYS = TARGET_SIDE_KEYS | DIRECT_OUTCOME_KEYS
FEATURE_KEY_RE = re.compile(r"^[A-Za-z0-9_.:/@|+=, -]{1,240}$")


class CandidateMiScreenError(RuntimeError):
    pass


@dataclass(frozen=True)
class CandidateSpec:
    candidate_id: str
    path: Path


@dataclass(frozen=True)
class CandidateMatrix:
    candidate_id: str
    path: Path
    source_sha256: str
    X: Any
    feature_names: list[str]
    row_count: int
    extra_row_count: int
    nonconstant_dim: int
    feature_matrix_sha256: str
    source_summary: dict[str, Any]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise CandidateMiScreenError(f"{code}{suffix}")


def reject_mounted_drive(label: str, path: Path) -> None:
    text = str(path)
    require(
        not (text.startswith("/mnt/c/") or text.startswith("/mnt/d/")),
        "CANDIDATE_MI_MOUNTED_DRIVE_FORBIDDEN",
        {label: text},
    )


def require_aiwonder_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    reject_mounted_drive(label, path)
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        any(text.startswith(root) for root in AIWONDER_ROOTS),
        "CANDIDATE_MI_NON_AIWONDER_PATH",
        {label: text, "allowed_roots": AIWONDER_ROOTS},
    )


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
        raise CandidateMiScreenError(f"CANDIDATE_MI_WRITE_READBACK_MISMATCH: {path}")


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


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    require(path.is_file(), "CANDIDATE_MI_SOURCE_JSONL_MISSING", str(path))
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateMiScreenError(f"CANDIDATE_MI_SOURCE_JSONL_INVALID: {path}:{line_no}: {exc}") from exc
            require(isinstance(row, dict), "CANDIDATE_MI_SOURCE_ROW_NOT_OBJECT", {"path": str(path), "line_no": line_no})
            rows.append(row)
    require(rows, "CANDIDATE_MI_SOURCE_JSONL_EMPTY", str(path))
    return rows


def parse_candidate_spec(raw: str) -> CandidateSpec:
    require("=" in raw, "CANDIDATE_MI_CANDIDATE_SPEC_INVALID", raw)
    candidate_id, path_text = raw.split("=", 1)
    candidate_id = candidate_id.strip()
    require(bool(candidate_id), "CANDIDATE_MI_CANDIDATE_ID_EMPTY", raw)
    require(re.match(r"^[A-Za-z0-9_.:-]+$", candidate_id) is not None, "CANDIDATE_MI_CANDIDATE_ID_INVALID", candidate_id)
    path = Path(path_text.strip())
    require(str(path), "CANDIDATE_MI_CANDIDATE_PATH_EMPTY", raw)
    return CandidateSpec(candidate_id=candidate_id, path=path)


def row_key(row: dict[str, Any]) -> tuple[str, str]:
    task = str(row.get("task_instance_id") or row.get("task_id") or "")
    category = str(row.get("mutation_category") or row.get("category") or "")
    require(task and category, "CANDIDATE_MI_ROW_KEY_MISSING", row)
    return task, category


def find_forbidden_keys(value: Any, forbidden: set[str], *, max_list_items: int = 10) -> set[str]:
    hits: set[str] = set()

    def visit(item: Any) -> None:
        if isinstance(item, dict):
            for key, child in item.items():
                if key in forbidden:
                    hits.add(key)
                visit(child)
        elif isinstance(item, list):
            for child in item[:max_list_items]:
                visit(child)

    visit(value)
    return hits


def coerce_feature_dict(row: dict[str, Any], *, feature_field: str, path: Path, line_key: tuple[str, str]) -> dict[str, float]:
    forbidden_top_level = sorted(find_forbidden_keys(row, FORBIDDEN_FEATURE_KEYS))
    require(
        not forbidden_top_level,
        "CANDIDATE_MI_FORBIDDEN_KEY_IN_SOURCE_ROW",
        {"path": str(path), "key": line_key, "forbidden_keys": forbidden_top_level[:20]},
    )
    raw_features = row.get(feature_field)
    require(isinstance(raw_features, dict) and raw_features, "CANDIDATE_MI_FEATURE_FIELD_MISSING", {"path": str(path), "key": line_key, "feature_field": feature_field})
    forbidden_features = sorted(find_forbidden_keys(raw_features, FORBIDDEN_FEATURE_KEYS))
    require(
        not forbidden_features,
        "CANDIDATE_MI_FORBIDDEN_FEATURE_KEY",
        {"path": str(path), "key": line_key, "forbidden_keys": forbidden_features[:20]},
    )
    out: dict[str, float] = {}
    for name, value in raw_features.items():
        feature_name = str(name)
        require(FEATURE_KEY_RE.match(feature_name) is not None, "CANDIDATE_MI_FEATURE_NAME_INVALID", {"feature": feature_name})
        require(isinstance(value, (int, float)) and not isinstance(value, bool), "CANDIDATE_MI_FEATURE_VALUE_NON_NUMERIC", {"feature": feature_name, "value": value})
        number = float(value)
        require(math.isfinite(number), "CANDIDATE_MI_FEATURE_VALUE_NONFINITE", {"feature": feature_name, "value": value})
        out[feature_name] = number
    require(out, "CANDIDATE_MI_FEATURE_VECTOR_EMPTY", {"path": str(path), "key": line_key})
    return out


def load_candidate_matrix(
    *,
    spec: CandidateSpec,
    dataset_rows: list[dict[str, Any]],
    feature_field: str,
    allow_non_aiwonder_roots: bool,
) -> CandidateMatrix:
    import numpy as np

    require_aiwonder_path(spec.path, f"candidate:{spec.candidate_id}", allow_non_aiwonder_roots)
    source_rows = read_jsonl(spec.path)
    dataset_keys = [row_key(row) for row in dataset_rows]
    require(len(set(dataset_keys)) == len(dataset_keys), "CANDIDATE_MI_DATASET_KEY_DUPLICATE")
    source_by_key: dict[tuple[str, str], dict[str, float]] = {}
    duplicate_keys: list[tuple[str, str]] = []
    feature_names: set[str] = set()
    feature_counts: Counter[str] = Counter()
    for row in source_rows:
        key = row_key(row)
        if key in source_by_key:
            duplicate_keys.append(key)
            continue
        features = coerce_feature_dict(row, feature_field=feature_field, path=spec.path, line_key=key)
        source_by_key[key] = features
        feature_names.update(features)
        for name in features:
            feature_counts[name] += 1
    require(not duplicate_keys, "CANDIDATE_MI_SOURCE_DUPLICATE_KEYS", {"candidate": spec.candidate_id, "sample": duplicate_keys[:20]})
    missing = [key for key in dataset_keys if key not in source_by_key]
    require(not missing, "CANDIDATE_MI_SOURCE_MISSING_DATASET_ROWS", {"candidate": spec.candidate_id, "missing_count": len(missing), "sample": missing[:20]})
    ordered_features = sorted(feature_names)
    require(ordered_features, "CANDIDATE_MI_SOURCE_FEATURES_EMPTY", spec.candidate_id)
    X = np.zeros((len(dataset_keys), len(ordered_features)), dtype=np.float32)
    feature_index = {name: idx for idx, name in enumerate(ordered_features)}
    for row_idx, key in enumerate(dataset_keys):
        for name, value in source_by_key[key].items():
            X[row_idx, feature_index[name]] = float(value)
    require(bool(np.all(np.isfinite(X))), "CANDIDATE_MI_FEATURE_MATRIX_NONFINITE", spec.candidate_id)
    std = X.astype(np.float64).std(axis=0)
    nonconstant_dim = int((std > 1.0e-10).sum())
    require(nonconstant_dim > 0, "CANDIDATE_MI_FEATURE_MATRIX_CONSTANT", spec.candidate_id)
    source_key_set = set(source_by_key)
    extra = sorted(source_key_set - set(dataset_keys))
    return CandidateMatrix(
        candidate_id=spec.candidate_id,
        path=spec.path,
        source_sha256=sha256_file(spec.path),
        X=X,
        feature_names=ordered_features,
        row_count=int(X.shape[0]),
        extra_row_count=len(extra),
        nonconstant_dim=nonconstant_dim,
        feature_matrix_sha256=sha256_bytes(X.astype("<f4", copy=False).tobytes()),
        source_summary={
            "source_row_count": len(source_rows),
            "matched_row_count": int(X.shape[0]),
            "extra_row_count": len(extra),
            "feature_count": len(ordered_features),
            "nonconstant_feature_count": nonconstant_dim,
            "min_feature_presence": min(feature_counts.values()) if feature_counts else 0,
            "max_feature_presence": max(feature_counts.values()) if feature_counts else 0,
            "extra_key_sample": [{"task_id": task, "category": category} for task, category in extra[:20]],
        },
    )


def estimate_bits(row: dict[str, Any]) -> float | None:
    if row.get("valid") is not True:
        return None
    for key in ("lower_bound_bits", "bounded_best_validation_bits", "joint_estimate_capped_to_target_bits", "per_component_sum_capped_to_target_bits"):
        value = row.get(key)
        if isinstance(value, (int, float)) and math.isfinite(float(value)):
            return float(value)
    return None


def classifier_lower_bound_safe(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    holdout_fraction: float,
) -> dict[str, Any]:
    try:
        return classifier_lower_bound(X=X, y=y, label=label, seed=seed, holdout_fraction=holdout_fraction)
    except ModuleNotFoundError as err:
        return numpy_ridge_classifier_lower_bound(
            X=X,
            y=y,
            label=label,
            seed=seed,
            holdout_fraction=holdout_fraction,
            dependency_error=repr(err),
        )


def numpy_ridge_classifier_lower_bound(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    holdout_fraction: float,
    dependency_error: str,
) -> dict[str, Any]:
    import numpy as np

    labels = [int(v) for v in y.tolist()]
    counts = Counter(labels)
    if len(counts) < 2:
        return {"label": label, "valid": False, "invalid_reason": "DEGENERATE_ORACLE_LABELS", "label_counts": dict(sorted(counts.items()))}
    train_idx, val_idx = stratified_split_local(labels, holdout_fraction, seed)
    if not train_idx or not val_idx:
        return {"label": label, "valid": False, "invalid_reason": "SPLIT_FAILED", "label_counts": dict(sorted(counts.items()))}
    Xr, reduction = numpy_standardize_reduce(X, seed=seed, max_components=min(128, max(8, len(labels) // 3)))
    y_arr = np.asarray(labels, dtype=np.float64)
    x_train = np.column_stack([np.ones(len(train_idx)), Xr[train_idx]])
    x_val = np.column_stack([np.ones(len(val_idx)), Xr[val_idx]])
    y_train = y_arr[train_idx]
    y_val = y_arr[val_idx]
    penalty = np.eye(x_train.shape[1], dtype=np.float64) * 1.0e-2
    penalty[0, 0] = 0.0
    try:
        coeff = np.linalg.solve(x_train.T @ x_train + penalty, x_train.T @ y_train)
    except np.linalg.LinAlgError:
        coeff = np.linalg.pinv(x_train.T @ x_train + penalty) @ x_train.T @ y_train
    probs = np.clip(x_val @ coeff, 0.001, 0.999)
    ce_bits = float(
        -np.mean(y_val * np.log2(probs) + (1.0 - y_val) * np.log2(1.0 - probs))
    )
    h_bits = entropy_bits(labels)
    lower_bound = max(0.0, min(h_bits, h_bits - ce_bits))
    return {
        "label": label,
        "valid": True,
        "estimator": "numpy_ridge_cross_entropy_lower_bound",
        "seed": seed,
        "sample_count": len(labels),
        "train_count": len(train_idx),
        "validation_count": len(val_idx),
        "label_counts": dict(sorted(counts.items())),
        "target_entropy_bits": h_bits,
        "cross_entropy_bits": ce_bits,
        "lower_bound_bits": lower_bound,
        "validation_auc": binary_auc(y_val.tolist(), probs.tolist()),
        "reduction": reduction,
        "dependency_error": dependency_error,
    }


def stratified_split_local(labels: list[int], holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    groups: dict[int, list[int]] = {}
    for idx, value in enumerate(labels):
        groups.setdefault(int(value), []).append(idx)
    rng = random.Random(seed)
    train: list[int] = []
    holdout: list[int] = []
    for _, indexes in sorted(groups.items()):
        indexes = list(indexes)
        rng.shuffle(indexes)
        n_holdout = max(1, int(round(len(indexes) * holdout_fraction))) if len(indexes) > 1 else 0
        holdout.extend(indexes[:n_holdout])
        train.extend(indexes[n_holdout:])
    rng.shuffle(train)
    rng.shuffle(holdout)
    return train, holdout


def numpy_standardize_reduce(X: Any, *, seed: int, max_components: int) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    X64 = np.asarray(X, dtype=np.float64)
    mean = X64.mean(axis=0)
    std = X64.std(axis=0)
    keep = std > 1.0e-10
    Xs = (X64[:, keep] - mean[keep]) / std[keep]
    info: dict[str, Any] = {
        "input_dim": int(X64.shape[1]),
        "nonconstant_dim": int(Xs.shape[1]),
        "method": "numpy_standardize_only",
        "output_dim": int(Xs.shape[1]),
    }
    if max_components > 0 and Xs.shape[1] > max_components:
        rng = np.random.default_rng(seed)
        projection = rng.normal(0.0, 1.0 / math.sqrt(float(max_components)), size=(Xs.shape[1], max_components))
        Xr = Xs @ projection
        info.update(
            {
                "method": "numpy_gaussian_random_projection",
                "output_dim": int(max_components),
                "random_projection_seed": seed,
            }
        )
        return Xr.astype(np.float32), info
    return Xs.astype(np.float32), info


def binary_auc(labels: list[float], scores: list[float]) -> float | None:
    positives = [score for label, score in zip(labels, scores) if int(label) == 1]
    negatives = [score for label, score in zip(labels, scores) if int(label) == 0]
    if not positives or not negatives:
        return None
    wins = 0.0
    total = 0.0
    for pos in positives:
        for neg in negatives:
            total += 1.0
            if pos > neg:
                wins += 1.0
            elif pos == neg:
                wins += 0.5
    return wins / total if total else None


def classifier_bits(row: dict[str, Any]) -> float | None:
    value = row.get("lower_bound_bits")
    if row.get("valid") is True and isinstance(value, (int, float)) and math.isfinite(float(value)):
        return float(value)
    return None


def matrix_subset(X: Any, indexes: list[int]) -> Any:
    import numpy as np

    return np.asarray(X)[indexes, :]


def concatenate_features(matrices: list[Any]) -> Any:
    import numpy as np

    require(bool(matrices), "CANDIDATE_MI_CONCAT_EMPTY")
    if len(matrices) == 1:
        return matrices[0]
    return np.concatenate(matrices, axis=1).astype(np.float32)


def standardize_nonconstant(X: Any) -> Any:
    import numpy as np

    X64 = np.asarray(X, dtype=np.float64)
    std = X64.std(axis=0)
    keep = std > 1.0e-10
    if not np.any(keep):
        return np.zeros((X64.shape[0], 0), dtype=np.float64)
    mean = X64.mean(axis=0)
    return (X64[:, keep] - mean[keep]) / std[keep]


def max_abs_corr_against_slots(panel_X: Any, feature_slices: dict[str, slice], candidate_X: Any) -> dict[str, Any]:
    import numpy as np

    candidate_std = standardize_nonconstant(candidate_X)
    if candidate_std.shape[1] == 0:
        return {
            "max_abs_corr": None,
            "max_abs_corr_slot": None,
            "per_slot": [],
            "valid": False,
            "invalid_reason": "CANDIDATE_CONSTANT",
        }
    denom = max(1.0, float(candidate_std.shape[0] - 1))
    rows: list[dict[str, Any]] = []
    global_max: float | None = None
    global_slot: str | None = None
    for slot, sl in sorted(feature_slices.items()):
        slot_std = standardize_nonconstant(panel_X[:, sl])
        if slot_std.shape[1] == 0:
            rows.append({"slot": slot, "valid": False, "invalid_reason": "SLOT_CONSTANT", "max_abs_corr": None})
            continue
        local_max = 0.0
        for start in range(0, candidate_std.shape[1], 256):
            block = candidate_std[:, start : start + 256]
            corr = np.abs((slot_std.T @ block) / denom)
            if corr.size:
                local_max = max(local_max, float(np.nanmax(corr)))
        rows.append({"slot": slot, "valid": True, "max_abs_corr": local_max})
        if global_max is None or local_max > global_max:
            global_max = local_max
            global_slot = slot
    return {
        "valid": global_max is not None,
        "max_abs_corr": global_max,
        "max_abs_corr_slot": global_slot,
        "per_slot": rows,
    }


def evaluate_feature_set(
    *,
    X: Any,
    y: Any,
    label: str,
    seed: int,
    holdout_fraction: float,
    kraskov_neighbors: int,
    run_mine: bool,
    mine_epochs: int,
    mine_batch_size: int,
    mine_learning_rate: float,
    mine_hidden_dim: int,
    mine_device: str,
) -> dict[str, Any]:
    classifier = classifier_lower_bound_safe(X=X, y=y, label=label, seed=seed, holdout_fraction=holdout_fraction)
    kraskov = kraskov_crosscheck(X=X, y=y, label=label, seed=seed, n_neighbors=kraskov_neighbors)
    mine = None
    if run_mine:
        mine = mine_estimate(
            X=X,
            y=y,
            label=label,
            seed=seed,
            epochs=mine_epochs,
            batch_size=mine_batch_size,
            learning_rate=mine_learning_rate,
            hidden_dim=mine_hidden_dim,
            holdout_fraction=holdout_fraction,
            device_name=mine_device,
        )
    return {
        "label": label,
        "classifier_lower_bound": classifier,
        "kraskov_crosscheck": kraskov,
        "mine": mine,
        "primary_bits": classifier_bits(classifier),
    }


def run_candidate_screen(args: argparse.Namespace) -> dict[str, Any]:
    import numpy as np

    dataset_root = Path(args.dataset_root)
    output_parent = Path(args.output_parent)
    state_parent = Path(args.state_parent)
    require_aiwonder_path(dataset_root, "dataset_root", bool(args.allow_non_aiwonder_roots))
    require_aiwonder_path(output_parent, "output_parent", bool(args.allow_non_aiwonder_roots))
    require_aiwonder_path(state_parent, "state_parent", bool(args.allow_non_aiwonder_roots))
    candidate_specs = [parse_candidate_spec(raw) for raw in args.candidate]
    require(candidate_specs, "CANDIDATE_MI_NO_CANDIDATES")
    candidate_ids = [spec.candidate_id for spec in candidate_specs]
    require(len(set(candidate_ids)) == len(candidate_ids), "CANDIDATE_MI_DUPLICATE_CANDIDATE_ID", candidate_ids)

    run_id = str(args.run_id or f"run-{utc_stamp()}-candidate-mi")
    output_root = output_parent / run_id
    state_root = state_parent / run_id
    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "CANDIDATE_MI_OUTPUT_ROOT_EXISTS", str(output_root))
    require(not state_root.exists(), "CANDIDATE_MI_STATE_ROOT_EXISTS", str(state_root))
    output_root.mkdir(parents=True, exist_ok=False)
    state_root.mkdir(parents=True, exist_ok=False)

    try:
        dataset = load_dataset(dataset_root, allow_non_aiwonder_roots=bool(args.allow_non_aiwonder_roots))
    except Exception as exc:
        raise CandidateMiScreenError(f"CANDIDATE_MI_DATASET_INVALID: {exc}") from exc
    y = np.asarray(dataset.y, dtype=np.int64)
    require(set(int(value) for value in y.tolist()).issubset({0, 1}), "CANDIDATE_MI_NON_BINARY_LABELS")
    candidates = [
        load_candidate_matrix(
            spec=spec,
            dataset_rows=dataset.rows,
            feature_field=str(args.feature_field),
            allow_non_aiwonder_roots=bool(args.allow_non_aiwonder_roots),
        )
        for spec in candidate_specs
    ]

    base_eval = evaluate_feature_set(
        X=dataset.X,
        y=y,
        label="baseline:current_panel",
        seed=int(args.seed),
        holdout_fraction=float(args.holdout_fraction),
        kraskov_neighbors=int(args.kraskov_neighbors),
        run_mine=bool(args.run_mine),
        mine_epochs=int(args.mine_epochs),
        mine_batch_size=int(args.mine_batch_size),
        mine_learning_rate=float(args.mine_learning_rate),
        mine_hidden_dim=int(args.mine_hidden_dim),
        mine_device=str(args.mine_device),
    )
    baseline_bits = base_eval["primary_bits"]
    require(baseline_bits is not None, "CANDIDATE_MI_BASELINE_CLASSIFIER_INVALID", base_eval["classifier_lower_bound"])

    estimate_rows: list[dict[str, Any]] = [base_eval]
    per_cell_rows: list[dict[str, Any]] = []
    candidate_summary_rows: list[dict[str, Any]] = []
    categories = sorted({str(row.get("mutation_category") or row.get("category")) for row in dataset.rows})
    category_indexes = {
        category: [idx for idx, row in enumerate(dataset.rows) if str(row.get("mutation_category") or row.get("category")) == category]
        for category in categories
    }

    for candidate in candidates:
        standalone = evaluate_feature_set(
            X=candidate.X,
            y=y,
            label=f"candidate:{candidate.candidate_id}:standalone",
            seed=int(args.seed),
            holdout_fraction=float(args.holdout_fraction),
            kraskov_neighbors=int(args.kraskov_neighbors),
            run_mine=bool(args.run_mine),
            mine_epochs=int(args.mine_epochs),
            mine_batch_size=int(args.mine_batch_size),
            mine_learning_rate=float(args.mine_learning_rate),
            mine_hidden_dim=int(args.mine_hidden_dim),
            mine_device=str(args.mine_device),
        )
        augmented_X = concatenate_features([dataset.X, candidate.X])
        augmented = evaluate_feature_set(
            X=augmented_X,
            y=y,
            label=f"candidate:{candidate.candidate_id}:baseline_plus_candidate",
            seed=int(args.seed),
            holdout_fraction=float(args.holdout_fraction),
            kraskov_neighbors=int(args.kraskov_neighbors),
            run_mine=bool(args.run_mine),
            mine_epochs=int(args.mine_epochs),
            mine_batch_size=int(args.mine_batch_size),
            mine_learning_rate=float(args.mine_learning_rate),
            mine_hidden_dim=int(args.mine_hidden_dim),
            mine_device=str(args.mine_device),
        )
        novelty = max_abs_corr_against_slots(dataset.X, dataset.feature_slices, candidate.X)
        standalone_bits = standalone["primary_bits"]
        augmented_bits = augmented["primary_bits"]
        delta_bits = None if augmented_bits is None else float(augmented_bits) - float(baseline_bits)
        novelty_value = novelty.get("max_abs_corr")
        utility_gate_pass = bool(delta_bits is not None and delta_bits >= float(args.min_delta_bits))
        novelty_gate_pass = bool(isinstance(novelty_value, (int, float)) and float(novelty_value) < float(args.novelty_max_corr))
        estimate_rows.extend([standalone, augmented])
        cell_delta_by_category: dict[str, float | None] = {}
        for category, indexes in category_indexes.items():
            X_base_cell = matrix_subset(dataset.X, indexes)
            X_candidate_cell = matrix_subset(candidate.X, indexes)
            y_cell = y[indexes]
            cell_base = classifier_lower_bound_safe(
                X=X_base_cell,
                y=y_cell,
                label=f"cell:{category}:baseline",
                seed=int(args.seed),
                holdout_fraction=float(args.holdout_fraction),
            )
            cell_standalone = classifier_lower_bound_safe(
                X=X_candidate_cell,
                y=y_cell,
                label=f"cell:{category}:candidate:{candidate.candidate_id}",
                seed=int(args.seed),
                holdout_fraction=float(args.holdout_fraction),
            )
            cell_augmented = classifier_lower_bound_safe(
                X=concatenate_features([X_base_cell, X_candidate_cell]),
                y=y_cell,
                label=f"cell:{category}:augmented:{candidate.candidate_id}",
                seed=int(args.seed),
                holdout_fraction=float(args.holdout_fraction),
            )
            base_bits = classifier_bits(cell_base)
            aug_bits = classifier_bits(cell_augmented)
            cell_delta = None if base_bits is None or aug_bits is None else aug_bits - base_bits
            cell_delta_by_category[category] = cell_delta
            per_cell_rows.append(
                {
                    "candidate_id": candidate.candidate_id,
                    "category": category,
                    "row_count": len(indexes),
                    "label_counts": dict(sorted(Counter(int(v) for v in y_cell.tolist()).items())),
                    "baseline_classifier_lower_bound": cell_base,
                    "candidate_classifier_lower_bound": cell_standalone,
                    "augmented_classifier_lower_bound": cell_augmented,
                    "augmented_delta_bits_vs_baseline": cell_delta,
                }
            )
        candidate_summary_rows.append(
            {
                "candidate_id": candidate.candidate_id,
                "source_path": str(candidate.path),
                "source_sha256": candidate.source_sha256,
                "row_count": candidate.row_count,
                "feature_count": len(candidate.feature_names),
                "nonconstant_feature_count": candidate.nonconstant_dim,
                "feature_matrix_sha256": candidate.feature_matrix_sha256,
                "standalone_classifier_bits": standalone_bits,
                "augmented_classifier_bits": augmented_bits,
                "augmented_delta_bits_vs_baseline": delta_bits,
                "novelty": novelty,
                "utility_gate_pass": utility_gate_pass,
                "novelty_gate_pass": novelty_gate_pass,
                "tier6_precheck_pass": utility_gate_pass and novelty_gate_pass,
                "behavioral_cell_delta_bits": {
                    category: cell_delta_by_category.get(category)
                    for category in BEHAVIORAL_CELLS
                    if category in category_delta_keys(cell_delta_by_category)
                },
                "source_summary": candidate.source_summary,
            }
        )

    greedy_rows = greedy_selection(
        baseline_X=dataset.X,
        y=y,
        candidates=candidates,
        baseline_bits=float(baseline_bits),
        seed=int(args.seed),
        holdout_fraction=float(args.holdout_fraction),
        min_delta_bits=float(args.min_delta_bits),
        novelty_by_candidate={row["candidate_id"]: row["novelty"] for row in candidate_summary_rows},
        novelty_max_corr=float(args.novelty_max_corr),
    )
    final_bits = greedy_rows[-1]["selected_panel_bits"] if greedy_rows else float(baseline_bits)

    candidate_summary_path = output_root / "candidate_matrix_summaries.jsonl"
    estimates_path = output_root / "candidate_estimates.jsonl"
    per_cell_path = output_root / "candidate_per_cell_estimates.jsonl"
    greedy_path = output_root / "greedy_selection.jsonl"
    write_jsonl(candidate_summary_path, candidate_summary_rows)
    write_jsonl(estimates_path, estimate_rows)
    write_jsonl(per_cell_path, per_cell_rows)
    write_jsonl(greedy_path, greedy_rows)
    matrix_npz_path = output_root / "candidate_feature_matrices.npz"
    np.savez_compressed(matrix_npz_path, **{sanitize_npz_key(candidate.candidate_id): candidate.X for candidate in candidates})
    doctrine_policy = {
        "measurement_only": True,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "target_labels_used_as_live_inputs": False,
        "candidate_features_are_separate_slots": True,
        "predictor_weights_changed": False,
        "inner_llm_used": False,
        "aiwonder_durable_root_required": True,
        "feature_matrix_concat_scope": "offline_mi_estimator_only_with_slot_slices_retained_not_live_predictor_input",
    }
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "canonical_issue": CANONICAL_ISSUE,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "passes": True,
        "dataset_root": str(dataset_root),
        "dataset_manifest_sha256": sha256_file(dataset_root / "dataset_manifest.json"),
        "dataset_row_count": int(dataset.X.shape[0]),
        "dataset_feature_dim": int(dataset.X.shape[1]),
        "target_entropy_bits": entropy_bits(y.tolist()),
        "baseline_classifier_bits": baseline_bits,
        "final_greedy_classifier_bits": final_bits,
        "decision_rule": {
            "target_bits": 0.95,
            "min_delta_bits": float(args.min_delta_bits),
            "novelty_max_corr": float(args.novelty_max_corr),
            "panel_sufficient": bool(final_bits >= 0.95),
            "classification": (
                "panel_sufficient"
                if final_bits >= 0.95
                else "marginal_or_insufficient"
                if final_bits >= 0.7
                else "candidate_pool_insufficient_or_classifier_lower_bound_low"
            ),
        },
        "selected_candidates": [row["selected_candidate_id"] for row in greedy_rows],
        "candidate_count": len(candidates),
        "candidate_ids": [candidate.candidate_id for candidate in candidates],
        "category_counts": dict(sorted(Counter(str(row.get("mutation_category") or row.get("category")) for row in dataset.rows).items())),
        "active_embedder_ids": sorted(dataset.feature_slices),
        "doctrine_policy": doctrine_policy,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "artifacts": {
            "candidate_matrix_summaries": str(candidate_summary_path),
            "candidate_matrix_summaries_sha256": sha256_file(candidate_summary_path),
            "candidate_estimates": str(estimates_path),
            "candidate_estimates_sha256": sha256_file(estimates_path),
            "candidate_per_cell_estimates": str(per_cell_path),
            "candidate_per_cell_estimates_sha256": sha256_file(per_cell_path),
            "greedy_selection": str(greedy_path),
            "greedy_selection_sha256": sha256_file(greedy_path),
            "candidate_feature_matrices_npz": str(matrix_npz_path),
            "candidate_feature_matrices_npz_sha256": sha256_file(matrix_npz_path),
            "state_root": str(state_root),
        },
    }
    manifest_path = output_root / "candidate_mi_screen_manifest.json"
    write_json(manifest_path, manifest)
    copied_paths = mirror_to_state(
        output_root=output_root,
        state_root=state_root,
        paths=[manifest_path, candidate_summary_path, estimates_path, per_cell_path, greedy_path],
    )
    hash_path = write_artifact_hashes(output_root, [manifest_path, candidate_summary_path, estimates_path, per_cell_path, greedy_path, matrix_npz_path, *copied_paths])
    state_hash_path = state_root / hash_path.name
    write_json(state_hash_path, json.loads(hash_path.read_text(encoding="utf-8")))
    return {
        "ok": True,
        "run_id": run_id,
        "output_root": str(output_root),
        "state_root": str(state_root),
        "manifest": str(manifest_path),
        "artifact_hashes": str(hash_path),
        "decision_rule": manifest["decision_rule"],
        "selected_candidates": manifest["selected_candidates"],
    }


def category_delta_keys(values: dict[str, float | None]) -> set[str]:
    return set(values)


def greedy_selection(
    *,
    baseline_X: Any,
    y: Any,
    candidates: list[CandidateMatrix],
    baseline_bits: float,
    seed: int,
    holdout_fraction: float,
    min_delta_bits: float,
    novelty_by_candidate: dict[str, dict[str, Any]],
    novelty_max_corr: float,
) -> list[dict[str, Any]]:
    remaining = {candidate.candidate_id: candidate for candidate in candidates}
    selected_matrices = [baseline_X]
    selected_rows: list[dict[str, Any]] = []
    current_bits = baseline_bits
    step = 0
    while remaining and current_bits < 0.95:
        best: dict[str, Any] | None = None
        for candidate_id, candidate in sorted(remaining.items()):
            novelty = novelty_by_candidate.get(candidate_id) or {}
            novelty_value = novelty.get("max_abs_corr")
            if not isinstance(novelty_value, (int, float)) or float(novelty_value) >= novelty_max_corr:
                continue
            X_trial = concatenate_features([*selected_matrices, candidate.X])
            result = classifier_lower_bound_safe(
                X=X_trial,
                y=y,
                label=f"greedy_step_candidate:{candidate_id}",
                seed=seed,
                holdout_fraction=holdout_fraction,
            )
            bits = classifier_bits(result)
            if bits is None:
                continue
            delta = bits - current_bits
            row = {
                "candidate_id": candidate_id,
                "trial_bits": bits,
                "delta_bits": delta,
                "classifier_lower_bound": result,
            }
            if best is None or bits > best["trial_bits"]:
                best = row
        if best is None or float(best["delta_bits"]) < min_delta_bits:
            break
        candidate = remaining.pop(str(best["candidate_id"]))
        selected_matrices.append(candidate.X)
        current_bits = float(best["trial_bits"])
        step += 1
        selected_rows.append(
            {
                "step": step,
                "selected_candidate_id": candidate.candidate_id,
                "selected_panel_bits": current_bits,
                "delta_bits": float(best["delta_bits"]),
                "target_bits": 0.95,
                "panel_sufficient_after_step": current_bits >= 0.95,
                "classifier_lower_bound": best["classifier_lower_bound"],
            }
        )
    return selected_rows


def sanitize_npz_key(value: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9_]+", "_", value).strip("_")
    return cleaned or "candidate"


def mirror_to_state(*, output_root: Path, state_root: Path, paths: list[Path]) -> list[Path]:
    copied = []
    for path in paths:
        relative = path.relative_to(output_root)
        target = state_root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        require(sha256_file(path) == sha256_file(target), "CANDIDATE_MI_STATE_MIRROR_HASH_MISMATCH", {"source": str(path), "state": str(target)})
        copied.append(target)
    return copied


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument(
        "--candidate",
        action="append",
        default=[],
        help="Candidate feature source as candidate_id=/path/to/features.jsonl. May be repeated.",
    )
    parser.add_argument("--feature-field", default="features")
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--seed", type=int, default=844032)
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--kraskov-neighbors", type=int, default=5)
    parser.add_argument("--min-delta-bits", type=float, default=0.005)
    parser.add_argument("--novelty-max-corr", type=float, default=0.7)
    parser.add_argument("--run-mine", action="store_true")
    parser.add_argument("--mine-epochs", type=int, default=800)
    parser.add_argument("--mine-batch-size", type=int, default=256)
    parser.add_argument("--mine-learning-rate", type=float, default=1.0e-4)
    parser.add_argument("--mine-hidden-dim", type=int, default=512)
    parser.add_argument("--mine-device", default="auto")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        result = run_candidate_screen(args)
    except CandidateMiScreenError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
