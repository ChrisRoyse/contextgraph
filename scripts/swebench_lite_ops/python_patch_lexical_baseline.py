#!/usr/bin/env python3
"""INV-005 bounded raw-patch lexical baseline.

This is a cheap triage probe for the heavier ComPass/Quatrain baseline work.
It consumes the same 2400-row panel/oracle dataset used by #759 plus the
SWE-bench Lite corpus patches, then compares reduced current-panel features
with deterministic raw-patch lexical hash features under grouped holdout splits.
No predictor, checkpoint, registry, or panel slot is promoted by this script.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
from collections import Counter, defaultdict
from collections.abc import Iterable
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from .python_per_cell_smooth_ece import metric_block
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_per_cell_smooth_ece import metric_block  # type: ignore


TASK_ID = "INV-005"
ARTIFACT_KIND = "patch_lexical_baseline"
SCHEMA_VERSION = 1
FORMULA_VERSION = "patch_lexical_baseline_v1"
AIWONDER_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")
DEFAULT_DATASET_ROOT = Path(
    "/zfs/archive/contextgraph/state/mi-audit/inv-032-v1/"
    "run-20260529T234900Z/full-candidate-map"
)
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/inv-005-patch-correctness-baseline-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/patch-correctness-baseline/inv-005-v1")
CONTESTED_MIXED_CELLS = ("off_by_one", "subtle_flip", "swap_variable")
TOKEN_RE = re.compile(
    r"[A-Za-z_][A-Za-z0-9_]*|0x[0-9A-Fa-f]+|\d+\.\d+|\d+|==|!=|<=|>=|->|[-+*/%]=|"
    r"[-+*/%<>]=?|[(){}\[\].,:;]"
)
EPS = 1.0e-12


class PatchLexicalBaselineError(RuntimeError):
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
class CorpusEntry:
    task_id: str
    repo: str
    category: str
    patch_path: Path
    patch_sha256: str
    oracle_all_passed: bool


@dataclass(frozen=True)
class SplitResult:
    split_index: int
    seed: int
    train_idx: list[int]
    holdout_idx: list[int]
    panel_metrics: dict[str, Any]
    lexical_metrics: dict[str, Any]
    augmented_metrics: dict[str, Any]
    delta_metrics: dict[str, Any]
    per_cell: list[dict[str, Any]]


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise PatchLexicalBaselineError(f"{code}{suffix}")


def reject_mounted_drive(label: str, path: Path) -> None:
    text = str(path)
    require(
        not (text.startswith("/mnt/c/") or text.startswith("/mnt/d/")),
        "PATCH_LEXICAL_MOUNTED_DRIVE_FORBIDDEN",
        {label: text},
    )


def require_aiwonder_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    reject_mounted_drive(label, path)
    require(
        any(text.startswith(root) for root in AIWONDER_ROOTS),
        "PATCH_LEXICAL_NON_AIWONDER_PATH",
        {label: text, "allowed_roots": AIWONDER_ROOTS},
    )


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
        raise PatchLexicalBaselineError(f"PATCH_LEXICAL_WRITE_READBACK_MISMATCH: {path}")


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


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def load_dataset(dataset_root: Path, *, allow_non_aiwonder_roots: bool) -> Dataset:
    import numpy as np

    require_aiwonder_path(dataset_root, "dataset_root", allow_non_aiwonder_roots)
    manifest_path = dataset_root / "dataset_manifest.json"
    require(manifest_path.is_file(), "PATCH_LEXICAL_DATASET_MANIFEST_MISSING", manifest_path)
    manifest = read_json(manifest_path)
    require(manifest.get("passes") is True, "PATCH_LEXICAL_DATASET_NOT_PASSING", manifest_path)
    require(manifest.get("slot_identity_preserved") is True, "PATCH_LEXICAL_SLOT_IDENTITY_NOT_PRESERVED")
    require(manifest.get("flat_vector_concat_used") is False, "PATCH_LEXICAL_FLAT_VECTOR_CONCAT_USED")

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
    require(dataset_npz.is_file(), "PATCH_LEXICAL_DATASET_NPZ_MISSING", dataset_npz)
    require(rows_path.is_file(), "PATCH_LEXICAL_DATASET_ROWS_MISSING", rows_path)

    data = np.load(dataset_npz, allow_pickle=False)
    require(set(data.files) == {"X", "y"}, "PATCH_LEXICAL_DATASET_NPZ_KEYS", data.files)
    X = np.asarray(data["X"], dtype=np.float32)
    y = np.asarray(data["y"], dtype=np.int64)
    require(X.ndim == 2 and y.ndim == 1 and X.shape[0] == y.shape[0], "PATCH_LEXICAL_DATASET_SHAPE", (X.shape, y.shape))
    require(bool(np.all(np.isfinite(X))), "PATCH_LEXICAL_DATASET_NONFINITE_FEATURES")
    require(set(int(v) for v in y.tolist()).issubset({0, 1}), "PATCH_LEXICAL_DATASET_NON_BINARY_LABELS")

    rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    require(len(rows) == int(X.shape[0]), "PATCH_LEXICAL_ROW_COUNT_MISMATCH", (len(rows), int(X.shape[0])))
    for index, row in enumerate(rows):
        require(int(row.get("row_index")) == index, "PATCH_LEXICAL_ROW_INDEX_MISMATCH", (index, row.get("row_index")))

    raw_slices = manifest.get("feature_slices") or {}
    active = [str(slot).lower() for slot in (manifest.get("active_embedder_ids") or [])]
    require(active, "PATCH_LEXICAL_ACTIVE_EMBEDDERS_MISSING")
    feature_slices: dict[str, slice] = {}
    for slot in active:
        info = raw_slices.get(slot)
        require(isinstance(info, dict), "PATCH_LEXICAL_ACTIVE_SLICE_MISSING", slot)
        start = int(info.get("start"))
        stop = int(info.get("stop"))
        require(0 <= start < stop <= X.shape[1], "PATCH_LEXICAL_SLICE_OUT_OF_RANGE", {slot: (start, stop), "feature_dim": X.shape[1]})
        feature_slices[slot] = slice(start, stop)
    return Dataset(X=X, y=y, rows=rows, manifest=manifest, feature_slices=feature_slices, dataset_npz=dataset_npz, rows_path=rows_path)


def load_corpus_entries(corpus_root: Path, *, allow_non_aiwonder_roots: bool) -> tuple[dict[tuple[str, str], CorpusEntry], dict[str, Any]]:
    require_aiwonder_path(corpus_root, "corpus_root", allow_non_aiwonder_roots)
    index_path = corpus_root / "index.json"
    require(index_path.is_file(), "PATCH_LEXICAL_CORPUS_INDEX_MISSING", index_path)
    payload = read_json(index_path)
    require(payload.get("complete") is True, "PATCH_LEXICAL_CORPUS_INDEX_NOT_COMPLETE")
    entries = payload.get("entries")
    require(isinstance(entries, list) and entries, "PATCH_LEXICAL_CORPUS_ENTRIES_MISSING")
    out: dict[tuple[str, str], CorpusEntry] = {}
    category_counts: Counter[str] = Counter()
    bucket_counts: Counter[str] = Counter()
    for index, value in enumerate(entries):
        require(isinstance(value, dict), "PATCH_LEXICAL_CORPUS_ENTRY_NOT_OBJECT", index)
        task_id = required_string(value, "task_id", index)
        repo = required_string(value, "repo", index)
        category = required_string(value, "category", index)
        rel_patch = required_string(value, "patch_path", index)
        reject_mounted_drive("patch_path", Path(rel_patch))
        patch_path = corpus_root / rel_patch
        require(patch_path.is_file(), "PATCH_LEXICAL_PATCH_FILE_MISSING", {"index": index, "path": str(patch_path)})
        patch_sha256 = required_string(value, "patch_sha256", index)
        actual_sha = sha256_file(patch_path)
        require(actual_sha == patch_sha256, "PATCH_LEXICAL_PATCH_HASH_MISMATCH", {"path": str(patch_path), "expected": patch_sha256, "actual": actual_sha})
        oracle = value.get("oracle_all_passed")
        require(isinstance(oracle, bool), "PATCH_LEXICAL_CORPUS_ORACLE_LABEL_MISSING", {"index": index, "task_id": task_id})
        key = (task_id, category)
        require(key not in out, "PATCH_LEXICAL_CORPUS_DUPLICATE_KEY", key)
        out[key] = CorpusEntry(
            task_id=task_id,
            repo=repo,
            category=category,
            patch_path=patch_path,
            patch_sha256=patch_sha256,
            oracle_all_passed=oracle,
        )
        category_counts[category] += 1
        bucket_counts[str(value.get("bucket") or "unknown")] += 1
    summary = {
        "index_path": str(index_path),
        "index_sha256": sha256_file(index_path),
        "entry_count": len(out),
        "category_counts": dict(sorted(category_counts.items())),
        "bucket_counts": dict(sorted(bucket_counts.items())),
        "corpus_sha256": payload.get("corpus_sha256"),
        "corpus_version": payload.get("corpus_version"),
    }
    return out, summary


def required_string(row: dict[str, Any], key: str, context: Any) -> str:
    value = row.get(key)
    require(isinstance(value, str) and value, "PATCH_LEXICAL_REQUIRED_STRING", {"key": key, "context": context})
    return value


def align_patch_texts(dataset: Dataset, entries: dict[tuple[str, str], CorpusEntry]) -> tuple[list[str], list[dict[str, Any]]]:
    texts: list[str] = []
    rows: list[dict[str, Any]] = []
    for row in dataset.rows:
        task_id = str(row.get("task_instance_id") or "")
        category = str(row.get("mutation_category") or "")
        key = (task_id, category)
        entry = entries.get(key)
        require(entry is not None, "PATCH_LEXICAL_CORPUS_ROW_MISSING", key)
        label = bool(row.get("oracle_all_passed"))
        require(label == entry.oracle_all_passed, "PATCH_LEXICAL_ORACLE_LABEL_MISMATCH", {"key": key})
        text = entry.patch_path.read_text(encoding="utf-8", errors="replace")
        require(text.strip(), "PATCH_LEXICAL_EMPTY_PATCH_TEXT", {"key": key, "path": str(entry.patch_path)})
        texts.append(text)
        rows.append(
            {
                "row_index": int(row["row_index"]),
                "task_id": task_id,
                "repo": entry.repo,
                "category": category,
                "patch_path": str(entry.patch_path),
                "patch_sha256": entry.patch_sha256,
                "oracle_all_passed": entry.oracle_all_passed,
                "patch_byte_count": len(text.encode("utf-8", errors="replace")),
            }
        )
    return texts, rows


def stable_seed(seed: int, label: str) -> int:
    digest = hashlib.sha256(f"{seed}:{label}".encode("utf-8")).digest()
    return int.from_bytes(digest[:8], "little") & 0x7FFFFFFF


def grouped_holdout_split(rows: list[dict[str, Any]], holdout_fraction: float, seed: int) -> tuple[list[int], list[int]]:
    require(0.05 <= holdout_fraction <= 0.5, "PATCH_LEXICAL_HOLDOUT_FRACTION_INVALID", holdout_fraction)
    by_task: dict[str, list[int]] = {}
    for index, row in enumerate(rows):
        task = str(row.get("task_instance_id") or f"row:{index}")
        by_task.setdefault(task, []).append(index)
    tasks = sorted(by_task)
    require(len(tasks) >= 8, "PATCH_LEXICAL_TOO_FEW_TASKS", len(tasks))
    ranked = sorted(tasks, key=lambda task: hashlib.sha256(f"{seed}:{task}".encode("utf-8")).hexdigest())
    holdout_task_count = max(1, int(round(len(ranked) * holdout_fraction)))
    holdout_tasks = set(ranked[:holdout_task_count])
    train_idx: list[int] = []
    holdout_idx: list[int] = []
    for task in tasks:
        target = holdout_idx if task in holdout_tasks else train_idx
        target.extend(by_task[task])
    require(train_idx and holdout_idx, "PATCH_LEXICAL_SPLIT_EMPTY")
    return sorted(train_idx), sorted(holdout_idx)


def reduce_slot_blocks(dataset: Dataset, train_idx: list[int], *, components_per_slot: int, seed: int) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    require(components_per_slot >= 1, "PATCH_LEXICAL_COMPONENTS_PER_SLOT_INVALID", components_per_slot)
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
        require(bool(np.any(keep)), "PATCH_LEXICAL_SLOT_BLOCK_CONSTANT", slot)
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


def lexical_hash_features(texts: list[str], *, hash_dim: int, token_limit: int) -> tuple[Any, dict[str, Any]]:
    import numpy as np

    require(hash_dim >= 64, "PATCH_LEXICAL_HASH_DIM_TOO_SMALL", hash_dim)
    rows = np.zeros((len(texts), hash_dim), dtype=np.float32)
    token_counts: list[int] = []
    unique_counts: list[int] = []
    for row_index, text in enumerate(texts):
        raw_tokens = lexical_tokens(text)
        if token_limit > 0:
            raw_tokens = raw_tokens[:token_limit]
        require(raw_tokens, "PATCH_LEXICAL_NO_TOKENS", row_index)
        token_counts.append(len(raw_tokens))
        unique_counts.append(len(set(raw_tokens)))
        for token in raw_tokens:
            digest = hashlib.sha256(token.encode("utf-8")).digest()
            column = int.from_bytes(digest[:8], "little") % hash_dim
            sign = 1.0 if (digest[8] & 1) == 0 else -1.0
            rows[row_index, column] += sign
        norm = math.sqrt(float(len(raw_tokens)))
        if norm > 0:
            rows[row_index] /= norm
    variance = rows.var(axis=0)
    summary = {
        "hash_dim": int(hash_dim),
        "token_limit": int(token_limit),
        "nonzero_feature_columns": int(np.sum(np.any(rows != 0.0, axis=0))),
        "nondegenerate_feature_columns": int(np.sum(variance > 1.0e-12)),
        "token_count_min": int(min(token_counts)),
        "token_count_max": int(max(token_counts)),
        "token_count_mean": float(sum(token_counts) / len(token_counts)),
        "unique_token_count_mean": float(sum(unique_counts) / len(unique_counts)),
        "feature_policy": "signed sha256 hashing over patch lexical tokens and short token ngrams",
    }
    return rows, summary


def lexical_tokens(text: str) -> list[str]:
    tokens: list[str] = []
    previous: str | None = None
    for raw_line in text.splitlines():
        line = raw_line.rstrip("\n")
        prefix = "ctx"
        body = line
        if line.startswith("+") and not line.startswith("+++"):
            prefix = "add"
            body = line[1:]
        elif line.startswith("-") and not line.startswith("---"):
            prefix = "del"
            body = line[1:]
        elif line.startswith("@@"):
            prefix = "hunk"
        elif line.startswith("diff ") or line.startswith("index ") or line.startswith("---") or line.startswith("+++"):
            prefix = "meta"
        tokens.append(f"line:{prefix}")
        for match in TOKEN_RE.finditer(body.lower()):
            token = match.group(0)
            normalized = normalize_token(token)
            tokens.append(f"{prefix}:{normalized}")
            if previous is not None:
                tokens.append(f"bi:{previous}>{prefix}:{normalized}")
            previous = f"{prefix}:{normalized}"
    return tokens


def normalize_token(token: str) -> str:
    if token.isdigit():
        return "<int>"
    if re.fullmatch(r"\d+\.\d+", token):
        return "<float>"
    if re.fullmatch(r"0x[0-9a-f]+", token):
        return "<hex>"
    return token


def standardize_from_train(features: Any, train_idx: list[int]) -> Any:
    import numpy as np

    X = np.asarray(features, dtype=np.float64)
    train = X[np.asarray(train_idx, dtype=np.int64)]
    mean = train.mean(axis=0)
    std = train.std(axis=0)
    keep = std > 1.0e-10
    require(bool(np.any(keep)), "PATCH_LEXICAL_FEATURE_MATRIX_CONSTANT")
    return ((X[:, keep] - mean[keep]) / std[keep]).astype(np.float64)


def ridge_scores(features: Any, y: Any, train_idx: list[int], holdout_idx: list[int], ridge_alpha: float) -> Any:
    import numpy as np

    require(ridge_alpha >= 0.0, "PATCH_LEXICAL_RIDGE_ALPHA_INVALID", ridge_alpha)
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


def metric_payload(labels: list[int], scores: Any) -> dict[str, Any]:
    return metric_block(labels, [float(max(0.0, min(1.0, value))) for value in scores])


def metric_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, float | None]:
    return {
        "correlation_delta": none_delta(before.get("prediction_oracle_correlation"), after.get("prediction_oracle_correlation")),
        "brier_delta": none_delta(before.get("brier"), after.get("brier")),
        "fixed_bin_ece_delta": none_delta(before.get("fixed_bin_ece"), after.get("fixed_bin_ece")),
        "smooth_ece_delta": none_delta(before.get("smooth_ece"), after.get("smooth_ece")),
        "accuracy_delta": none_delta(before.get("accuracy_at_half"), after.get("accuracy_at_half")),
    }


def none_delta(before: Any, after: Any) -> float | None:
    if before is None or after is None:
        return None
    return float(after) - float(before)


def per_cell_rows(
    dataset: Dataset,
    holdout_idx: list[int],
    panel_scores: Any,
    lexical_scores: Any,
    augmented_scores: Any,
) -> list[dict[str, Any]]:
    import numpy as np

    y = dataset.y[np.asarray(holdout_idx, dtype=np.int64)]
    by_cell: dict[str, list[int]] = defaultdict(list)
    for local, row_index in enumerate(holdout_idx):
        category = str(dataset.rows[row_index].get("mutation_category") or "")
        by_cell[category].append(local)
    rows: list[dict[str, Any]] = []
    for category, local_indexes in sorted(by_cell.items()):
        labels = [int(y[index]) for index in local_indexes]
        panel = [float(panel_scores[index]) for index in local_indexes]
        lexical = [float(lexical_scores[index]) for index in local_indexes]
        augmented = [float(augmented_scores[index]) for index in local_indexes]
        panel_metrics = metric_payload(labels, panel)
        lexical_metrics = metric_payload(labels, lexical)
        augmented_metrics = metric_payload(labels, augmented)
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "patch_lexical_baseline_cell_metric",
                "formula_version": FORMULA_VERSION,
                "cell": f"{category}::python",
                "mutation_category": category,
                "language": "python",
                "holdout_rows": len(local_indexes),
                "oracle_pass_count": sum(labels),
                "oracle_fail_count": len(labels) - sum(labels),
                "panel_reduced": panel_metrics,
                "lexical_only": lexical_metrics,
                "panel_plus_lexical": augmented_metrics,
                "lexical_delta_vs_panel": metric_delta(panel_metrics, lexical_metrics),
                "augmented_delta_vs_panel": metric_delta(panel_metrics, augmented_metrics),
            }
        )
    return rows


def run_split(
    dataset: Dataset,
    lexical_features: Any,
    *,
    split_index: int,
    seed: int,
    holdout_fraction: float,
    components_per_slot: int,
    ridge_alpha: float,
) -> SplitResult:
    import numpy as np

    train_idx, holdout_idx = grouped_holdout_split(dataset.rows, holdout_fraction, seed)
    reduced, _reductions = reduce_slot_blocks(dataset, train_idx, components_per_slot=components_per_slot, seed=seed)
    augmented = np.concatenate([reduced, lexical_features], axis=1)
    holdout = np.asarray(holdout_idx, dtype=np.int64)
    labels = [int(value) for value in dataset.y[holdout].tolist()]
    panel_scores = ridge_scores(reduced, dataset.y, train_idx, holdout_idx, ridge_alpha)
    lexical_scores = ridge_scores(lexical_features, dataset.y, train_idx, holdout_idx, ridge_alpha)
    augmented_scores = ridge_scores(augmented, dataset.y, train_idx, holdout_idx, ridge_alpha)
    panel = metric_payload(labels, panel_scores)
    lexical = metric_payload(labels, lexical_scores)
    augmented_metrics = metric_payload(labels, augmented_scores)
    delta = {
        "lexical_vs_panel": metric_delta(panel, lexical),
        "augmented_vs_panel": metric_delta(panel, augmented_metrics),
    }
    return SplitResult(
        split_index=split_index,
        seed=seed,
        train_idx=train_idx,
        holdout_idx=holdout_idx,
        panel_metrics=panel,
        lexical_metrics=lexical,
        augmented_metrics=augmented_metrics,
        delta_metrics=delta,
        per_cell=per_cell_rows(dataset, holdout_idx, panel_scores, lexical_scores, augmented_scores),
    )


def mean_optional(values: Iterable[Any]) -> float | None:
    real = [float(value) for value in values if isinstance(value, (int, float)) and math.isfinite(float(value))]
    if not real:
        return None
    return float(sum(real) / len(real))


def mean_metric(results: list[SplitResult], attr: str) -> dict[str, Any]:
    metrics = [getattr(result, attr) for result in results]
    keys = sorted({key for row in metrics for key in row if isinstance(row.get(key), (int, float)) or row.get(key) is None})
    return {key: mean_optional(row.get(key) for row in metrics) for key in keys}


def summarize_deltas(results: list[SplitResult]) -> dict[str, Any]:
    lexical = [result.delta_metrics["lexical_vs_panel"] for result in results]
    augmented = [result.delta_metrics["augmented_vs_panel"] for result in results]
    keys = sorted({key for row in lexical + augmented for key in row})
    return {
        "lexical_vs_panel": {key: mean_optional(row.get(key) for row in lexical) for key in keys},
        "augmented_vs_panel": {key: mean_optional(row.get(key) for row in augmented) for key in keys},
    }


def contested_cell_deltas(results: list[SplitResult]) -> dict[str, Any]:
    by_cell: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for result in results:
        for row in result.per_cell:
            by_cell[str(row["mutation_category"])].append(row)
    out: dict[str, Any] = {}
    for category in CONTESTED_MIXED_CELLS:
        rows = by_cell.get(category, [])
        out[f"{category}::python"] = {
            "split_count": len(rows),
            "augmented_correlation_delta_mean": mean_optional(
                ((row.get("augmented_delta_vs_panel") or {}).get("correlation_delta") for row in rows)
            ),
            "lexical_correlation_delta_mean": mean_optional(
                ((row.get("lexical_delta_vs_panel") or {}).get("correlation_delta") for row in rows)
            ),
            "augmented_brier_delta_mean": mean_optional(
                ((row.get("augmented_delta_vs_panel") or {}).get("brier_delta") for row in rows)
            ),
        }
    return out


def split_to_json(result: SplitResult) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "patch_lexical_baseline_split_metric",
        "formula_version": FORMULA_VERSION,
        "split_index": result.split_index,
        "seed": result.seed,
        "train_rows": len(result.train_idx),
        "holdout_rows": len(result.holdout_idx),
        "panel_reduced": result.panel_metrics,
        "lexical_only": result.lexical_metrics,
        "panel_plus_lexical": result.augmented_metrics,
        "deltas": result.delta_metrics,
    }


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    import numpy as np

    run_id = str(args.run_id or f"run-{utc_stamp()}-lexical-baseline")
    output_root = Path(args.output_root) / run_id if Path(args.output_root).name != run_id else Path(args.output_root)
    state_root = Path(args.state_root) / run_id if Path(args.state_root).name != run_id else Path(args.state_root)
    require_aiwonder_path(output_root, "output_root", bool(args.allow_non_aiwonder_roots))
    require_aiwonder_path(state_root, "state_root", bool(args.allow_non_aiwonder_roots))
    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    output_root.mkdir(parents=True, exist_ok=True)
    state_root.mkdir(parents=True, exist_ok=True)

    dataset = load_dataset(Path(args.dataset_root), allow_non_aiwonder_roots=bool(args.allow_non_aiwonder_roots))
    corpus_entries, corpus_summary = load_corpus_entries(Path(args.corpus_root), allow_non_aiwonder_roots=bool(args.allow_non_aiwonder_roots))
    patch_texts, aligned_rows = align_patch_texts(dataset, corpus_entries)
    lexical_features, lexical_summary = lexical_hash_features(
        patch_texts,
        hash_dim=int(args.hash_dim),
        token_limit=int(args.token_limit),
    )
    require(lexical_features.shape[0] == dataset.X.shape[0], "PATCH_LEXICAL_FEATURE_ROW_COUNT_MISMATCH")

    results = []
    for split_index in range(int(args.split_count)):
        split_seed = stable_seed(int(args.seed), f"split:{split_index}")
        results.append(
            run_split(
                dataset,
                lexical_features,
                split_index=split_index,
                seed=split_seed,
                holdout_fraction=float(args.holdout_fraction),
                components_per_slot=int(args.components_per_slot),
                ridge_alpha=float(args.ridge_alpha),
            )
        )

    split_rows = [split_to_json(result) for result in results]
    cell_rows = []
    for result in results:
        for row in result.per_cell:
            enriched = dict(row)
            enriched["split_index"] = result.split_index
            cell_rows.append(enriched)
    source_rows = aligned_rows[: int(args.source_sample_count)]
    summary = {
        "split_count": len(results),
        "panel_reduced_mean": mean_metric(results, "panel_metrics"),
        "lexical_only_mean": mean_metric(results, "lexical_metrics"),
        "panel_plus_lexical_mean": mean_metric(results, "augmented_metrics"),
        "delta_mean": summarize_deltas(results),
        "required_cell_deltas": contested_cell_deltas(results),
    }

    blockers = []
    aug_corr_delta = ((summary.get("delta_mean") or {}).get("augmented_vs_panel") or {}).get("correlation_delta")
    if aug_corr_delta is None or float(aug_corr_delta) < float(args.min_augmented_correlation_delta):
        blockers.append("augmented_global_correlation_delta_below_threshold")
    for cell, cell_summary in summary["required_cell_deltas"].items():
        delta = cell_summary.get("augmented_correlation_delta_mean")
        if delta is None or float(delta) < float(args.min_required_cell_delta):
            blockers.append(f"{cell}:augmented_correlation_delta_below_threshold")
    brier_delta = ((summary.get("delta_mean") or {}).get("augmented_vs_panel") or {}).get("brier_delta")
    if brier_delta is not None and float(brier_delta) > float(args.max_brier_regression):
        blockers.append("augmented_brier_regressed")

    split_metrics_path = output_root / "patch_lexical_split_metrics.jsonl"
    cell_metrics_path = output_root / "patch_lexical_cell_metrics.jsonl"
    source_samples_path = output_root / "patch_lexical_source_samples.jsonl"
    write_jsonl(split_metrics_path, split_rows)
    write_jsonl(cell_metrics_path, cell_rows)
    write_jsonl(source_samples_path, source_rows)

    state_split_metrics_path = state_root / split_metrics_path.name
    state_cell_metrics_path = state_root / cell_metrics_path.name
    state_source_samples_path = state_root / source_samples_path.name
    write_jsonl(state_split_metrics_path, split_rows)
    write_jsonl(state_cell_metrics_path, cell_rows)
    write_jsonl(state_source_samples_path, source_rows)

    category_counts = Counter(str(row.get("mutation_category") or "") for row in dataset.rows)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "created_at": utc_now(),
        "passes": True,
        "run_id": run_id,
        "row_count": int(dataset.X.shape[0]),
        "feature_dim": int(dataset.X.shape[1]),
        "active_embedder_ids": list(dataset.feature_slices),
        "category_counts": dict(sorted(category_counts.items())),
        "dataset": {
            "dataset_root": str(args.dataset_root),
            "dataset_npz": str(dataset.dataset_npz),
            "dataset_npz_sha256": sha256_file(dataset.dataset_npz),
            "dataset_rows_jsonl": str(dataset.rows_path),
            "dataset_rows_jsonl_sha256": sha256_file(dataset.rows_path),
            "dataset_manifest_sha256": sha256_file(Path(args.dataset_root) / "dataset_manifest.json"),
        },
        "corpus": corpus_summary,
        "lexical_feature_summary": lexical_summary,
        "summary": summary,
        "decision": {
            "bounded_triage_complete": True,
            "raw_patch_lexical_surface_promotable": not blockers,
            "heavier_compass_quatrain_run_recommended": not blockers,
            "promotion_allowed": False,
            "promotion_blockers": blockers,
            "next_required_artifact": (
                "full_ComPass_or_text_slot_utility_gate" if not blockers else "new_observation_surface_or_stronger_text_representation"
            ),
        },
        "policy": {
            "measurement_only": True,
            "predictor_weights_changed": False,
            "checkpoint_changed": False,
            "panel_slot_promoted": False,
            "inner_llm_used": False,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "target_labels_used_as_live_inputs": False,
            "oracle_labels_used_only_for_offline_fit_and_report": True,
            "holdout_used_for_fit": False,
        },
        "parameters": {
            "split_count": int(args.split_count),
            "holdout_fraction": float(args.holdout_fraction),
            "seed": int(args.seed),
            "components_per_slot": int(args.components_per_slot),
            "ridge_alpha": float(args.ridge_alpha),
            "hash_dim": int(args.hash_dim),
            "token_limit": int(args.token_limit),
            "min_augmented_correlation_delta": float(args.min_augmented_correlation_delta),
            "min_required_cell_delta": float(args.min_required_cell_delta),
            "max_brier_regression": float(args.max_brier_regression),
        },
        "artifacts": {
            "split_metrics": str(split_metrics_path),
            "cell_metrics": str(cell_metrics_path),
            "source_samples": str(source_samples_path),
            "state_split_metrics": str(state_split_metrics_path),
            "state_cell_metrics": str(state_cell_metrics_path),
            "state_source_samples": str(state_source_samples_path),
        },
    }
    manifest_path = output_root / "patch_lexical_baseline_manifest.json"
    state_manifest_path = state_root / manifest_path.name
    write_json(manifest_path, manifest)
    write_json(state_manifest_path, manifest)
    artifact_hashes_path = output_root / "artifact_hashes.json"
    state_artifact_hashes_path = state_root / artifact_hashes_path.name
    artifacts = {
        "manifest": manifest_path,
        "split_metrics": split_metrics_path,
        "cell_metrics": cell_metrics_path,
        "source_samples": source_samples_path,
    }
    hashes = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": f"{ARTIFACT_KIND}_artifact_hashes",
        "artifacts": {name: {"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size} for name, path in sorted(artifacts.items())},
    }
    write_json(artifact_hashes_path, hashes)
    write_json(state_artifact_hashes_path, hashes)
    require(sha256_file(manifest_path) == sha256_file(state_manifest_path), "PATCH_LEXICAL_STATE_MANIFEST_HASH_MISMATCH")
    require(sha256_file(split_metrics_path) == sha256_file(state_split_metrics_path), "PATCH_LEXICAL_STATE_SPLIT_HASH_MISMATCH")
    require(sha256_file(cell_metrics_path) == sha256_file(state_cell_metrics_path), "PATCH_LEXICAL_STATE_CELL_HASH_MISMATCH")
    return {
        "manifest": str(manifest_path),
        "state_manifest": str(state_manifest_path),
        "artifact_hashes": str(artifact_hashes_path),
        "state_artifact_hashes": str(state_artifact_hashes_path),
        "summary": summary,
        "promotion_blockers": blockers,
        "passes": True,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset-root", type=Path, default=DEFAULT_DATASET_ROOT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--split-count", type=int, default=5)
    parser.add_argument("--holdout-fraction", type=float, default=0.2)
    parser.add_argument("--seed", type=int, default=757)
    parser.add_argument("--components-per-slot", type=int, default=8)
    parser.add_argument("--hash-dim", type=int, default=2048)
    parser.add_argument("--token-limit", type=int, default=8192)
    parser.add_argument("--ridge-alpha", type=float, default=10.0)
    parser.add_argument("--min-augmented-correlation-delta", type=float, default=0.005)
    parser.add_argument("--min-required-cell-delta", type=float, default=0.005)
    parser.add_argument("--max-brier-regression", type=float, default=0.0)
    parser.add_argument("--source-sample-count", type=int, default=32)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    result = run_audit(args)
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
