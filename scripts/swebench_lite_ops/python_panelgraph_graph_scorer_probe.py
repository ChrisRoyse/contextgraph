#!/usr/bin/env python3
"""Measure #726 PanelGraph graph-scorer lift against row-level baselines.

This runner is measurement-only. It fits deterministic ridge heads on
calibration rows from the existing #726 Phase 1 eval split and reports holdout
delta for graph-shaped row features against the current row-level predictor and
the per-chunk aggregation baseline.
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


SCHEMA_VERSION = 1
TASK_ID = "BLINDSPOT-004"
ARTIFACT_KIND = "panelgraph_graph_scorer_probe"
FORMULA_VERSION = "panelgraph_graph_scorer_v1"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_PHASE1_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-phase1-fsv/"
    "run-20260603T023200Z-panelgraph-with-e9-chunk-verdict-e57f623d"
)
DEFAULT_CHUNK_VERDICT_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-chunk-verdict-fsv/"
    "run-20260603T023000Z-chunk-verdict-e9-only-e57f623d"
)
DEFAULT_ACTIVE_INDEX = Path(
    "/zfs/archive/contextgraph/state/forward-cache-incremental-v3/"
    "run-20260603T022500Z-panelgraph-e9-merged-active-index-e57f623d/active_index/active_index.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-graph-scorer-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/panelgraph-graph-scorer/blindspot-004-v1")
DEFAULT_ACTIVE_EMBEDDERS = ("e9",)
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_per_test_count",
    "oracle_verdict_sha256",
    "resolved_ids",
    "unresolved_ids",
}


class PanelGraphGraphScorerError(RuntimeError):
    pass


@dataclass(frozen=True)
class EvalRow:
    row_index: int
    split: str
    task_id: str
    repo: str
    category: str
    label: int
    row_level_score: float
    shape_class: str
    patch_touched_file_count: int
    patch_touched_chunk_count: int
    candidate_state_chunk_count: int
    chunk_ids: tuple[str, ...]

    @property
    def key(self) -> tuple[str, str]:
        return (self.task_id, self.category)


@dataclass(frozen=True)
class IndexRecord:
    chunk_id: str
    embedder_id: str
    record_path: Path
    byte_offset: int
    byte_len: int
    record_sha256: str
    vector_sha256: str
    vector_len: int


@dataclass(frozen=True)
class RidgeModel:
    variant: str
    alpha: float
    feature_count: int
    kept_feature_count: int
    train_examples: int
    label_mean: float
    feature_mean: Any
    feature_std: Any
    keep_mask: Any
    coefficients: Any


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise PanelGraphGraphScorerError(f"{code}{suffix}")


def reject_mounted_drive_fragment(label: str, value: str) -> None:
    require(
        "/mnt/c/" not in value and "/mnt/d/" not in value,
        "PANELGRAPH_GRAPH_SCORER_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        text.startswith(AIWONDER_ARCHIVE_ROOT),
        "PANELGRAPH_GRAPH_SCORER_NON_AIWONDER_DURABLE_PATH",
        {label: text, "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )
    reject_mounted_drive_fragment(label, text)


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
    tmp = path.with_suffix(path.suffix + f".tmp-{os.getpid()}")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise PanelGraphGraphScorerError(f"PANELGRAPH_GRAPH_SCORER_WRITE_READBACK_MISMATCH: {path}")


def write_json(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    lines: list[str] = []
    count = 0
    for row in rows:
        lines.append(canonical_json(row) + "\n")
        count += 1
    write_text_checked(path, "".join(lines))
    return count


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    require(path.is_file(), code, str(path))
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise PanelGraphGraphScorerError(f"{code}: invalid JSON at {path}:{line_no}: {exc}") from exc
            require(isinstance(row, dict), f"{code}_ROW_NOT_OBJECT", line_no)
            rows.append(row)
    return rows


def probability(value: Any, label: str) -> float:
    require(type(value) in (int, float) and math.isfinite(float(value)), "PANELGRAPH_GRAPH_SCORER_SCORE_INVALID", {label: value})
    score = float(value)
    require(0.0 <= score <= 1.0, "PANELGRAPH_GRAPH_SCORER_SCORE_OUT_OF_RANGE", {label: value})
    return score


def parse_active_embedders(value: str) -> tuple[str, ...]:
    embedders = tuple(part.strip().lower() for part in value.split(",") if part.strip())
    require(bool(embedders), "PANELGRAPH_GRAPH_SCORER_ACTIVE_EMBEDDERS_EMPTY")
    require(len(set(embedders)) == len(embedders), "PANELGRAPH_GRAPH_SCORER_DUPLICATE_ACTIVE_EMBEDDER", embedders)
    return embedders


def load_eval_rows(path: Path) -> list[EvalRow]:
    rows: list[EvalRow] = []
    seen: set[tuple[str, str, str]] = set()
    for index, raw in enumerate(read_jsonl(path, "PANELGRAPH_GRAPH_SCORER_EVAL_ROWS_MISSING")):
        for key, value in raw.items():
            if isinstance(value, str):
                reject_mounted_drive_fragment(key, value)
        split = str(raw.get("split") or "")
        require(split in {"calibration", "holdout"}, "PANELGRAPH_GRAPH_SCORER_SPLIT_INVALID", {"index": index, "split": split})
        task_id = str(raw.get("task_id") or "")
        repo = str(raw.get("repo") or "")
        category = str(raw.get("category") or "")
        require(task_id and repo and category, "PANELGRAPH_GRAPH_SCORER_EVAL_KEY_MISSING", index)
        label_raw = raw.get("actual_oracle_pass")
        require(isinstance(label_raw, bool), "PANELGRAPH_GRAPH_SCORER_EVAL_LABEL_MISSING", index)
        chunks = raw.get("primary_chunk_ids")
        require(isinstance(chunks, list) and chunks, "PANELGRAPH_GRAPH_SCORER_PRIMARY_CHUNKS_MISSING", {"task_id": task_id, "category": category})
        chunk_ids = tuple(str(chunk) for chunk in chunks)
        require(all(chunk.startswith("sha256:") for chunk in chunk_ids), "PANELGRAPH_GRAPH_SCORER_CHUNK_ID_INVALID", {"task_id": task_id, "category": category})
        require(len(set(chunk_ids)) == len(chunk_ids), "PANELGRAPH_GRAPH_SCORER_DUPLICATE_ROW_CHUNK", {"task_id": task_id, "category": category})
        key = (split, task_id, category)
        require(key not in seen, "PANELGRAPH_GRAPH_SCORER_DUPLICATE_EVAL_ROW", key)
        seen.add(key)
        rows.append(
            EvalRow(
                row_index=index,
                split=split,
                task_id=task_id,
                repo=repo,
                category=category,
                label=1 if label_raw else 0,
                row_level_score=probability(raw.get("predicted_oracle_pass"), f"eval_rows[{index}].predicted_oracle_pass"),
                shape_class=str(raw.get("shape_class") or "unknown"),
                patch_touched_file_count=int(raw.get("patch_touched_file_count") or 0),
                patch_touched_chunk_count=int(raw.get("patch_touched_chunk_count") or len(chunk_ids)),
                candidate_state_chunk_count=int(raw.get("candidate_state_chunk_count") or 0),
                chunk_ids=chunk_ids,
            )
        )
    require(rows, "PANELGRAPH_GRAPH_SCORER_EVAL_ROWS_EMPTY")
    require(any(row.split == "calibration" for row in rows), "PANELGRAPH_GRAPH_SCORER_CALIBRATION_ROWS_EMPTY")
    require(any(row.split == "holdout" for row in rows), "PANELGRAPH_GRAPH_SCORER_HOLDOUT_ROWS_EMPTY")
    require(len({row.label for row in rows if row.split == "calibration"}) == 2, "PANELGRAPH_GRAPH_SCORER_CALIBRATION_LABELS_DEGENERATE")
    return rows


def load_chunk_scores(path: Path, eval_rows: list[EvalRow]) -> tuple[dict[tuple[str, str, str], float], dict[str, Any]]:
    expected = {(row.task_id, row.category, chunk_id) for row in eval_rows for chunk_id in row.chunk_ids}
    scores: dict[tuple[str, str, str], float] = {}
    target_key_violations: list[dict[str, Any]] = []
    for line_no, raw in enumerate(read_jsonl(path, "PANELGRAPH_GRAPH_SCORER_CHUNK_VERDICT_ROWS_MISSING"), start=1):
        leaked = sorted(TARGET_SIDE_KEYS.intersection(raw))
        if leaked:
            target_key_violations.append({"line_no": line_no, "keys": leaked})
            continue
        task_id = str(raw.get("task_id") or "")
        category = str(raw.get("category") or "")
        chunk_id = str(raw.get("chunk_id") or "")
        key = (task_id, category, chunk_id)
        if key not in expected:
            continue
        require(key not in scores, "PANELGRAPH_GRAPH_SCORER_DUPLICATE_CHUNK_SCORE", key)
        scores[key] = probability(raw.get("predicted_oracle_pass"), f"chunk_scores[{line_no}].predicted_oracle_pass")
    require(not target_key_violations, "PANELGRAPH_GRAPH_SCORER_TARGET_KEYS_IN_CHUNK_SCORE_ROWS", target_key_violations[:5])
    missing = sorted(expected - set(scores))
    require(
        not missing,
        "PANELGRAPH_GRAPH_SCORER_CHUNK_SCORE_COVERAGE_INCOMPLETE",
        {"missing_sample": missing[:8], "missing_count": len(missing)},
    )
    return scores, {
        "chunk_score_path": str(path),
        "chunk_score_sha256": sha256_file(path),
        "expected_chunk_score_count": len(expected),
        "matched_chunk_score_count": len(scores),
    }


def load_active_index(
    path: Path,
    *,
    target_chunk_ids: set[str],
    active_embedders: tuple[str, ...],
    allow_non_aiwonder_roots: bool,
) -> tuple[dict[str, dict[str, IndexRecord]], dict[str, Any]]:
    require_archive_path(path, "active_index", allow_non_aiwonder_roots)
    require(path.is_file(), "PANELGRAPH_GRAPH_SCORER_ACTIVE_INDEX_MISSING", str(path))
    active_set = set(active_embedders)
    found: dict[str, dict[str, IndexRecord]] = {chunk_id: {} for chunk_id in target_chunk_ids}
    scanned = 0
    matched = 0
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            scanned += 1
            row = json.loads(line)
            chunk_id = str(row.get("chunk_id") or "")
            if chunk_id not in target_chunk_ids:
                continue
            embedder_id = str(row.get("embedder_id") or "").lower()
            if embedder_id not in active_set:
                continue
            record_path = Path(str(row.get("record_path") or ""))
            require_archive_path(record_path, "record_path", allow_non_aiwonder_roots)
            record = IndexRecord(
                chunk_id=chunk_id,
                embedder_id=embedder_id,
                record_path=record_path,
                byte_offset=int(row.get("byte_offset")),
                byte_len=int(row.get("byte_len")),
                record_sha256=str(row.get("record_sha256") or ""),
                vector_sha256=str(row.get("vector_sha256") or ""),
                vector_len=int(row.get("vector_len")),
            )
            require(record.byte_offset >= 0 and record.byte_len > 0, "PANELGRAPH_GRAPH_SCORER_INDEX_OFFSET_INVALID", {"line": line_no, "row": row})
            require(record.vector_sha256.startswith("sha256:"), "PANELGRAPH_GRAPH_SCORER_INDEX_VECTOR_SHA_MISSING", {"line": line_no, "row": row})
            require(embedder_id not in found[chunk_id], "PANELGRAPH_GRAPH_SCORER_DUPLICATE_ACTIVE_INDEX_ROW", {"chunk_id": chunk_id, "embedder_id": embedder_id})
            found[chunk_id][embedder_id] = record
            matched += 1
    missing = {
        chunk_id: [embedder for embedder in active_embedders if embedder not in per_slot]
        for chunk_id, per_slot in found.items()
        if any(embedder not in per_slot for embedder in active_embedders)
    }
    require(not missing, "PANELGRAPH_GRAPH_SCORER_ACTIVE_INDEX_COVERAGE_INCOMPLETE", {"missing_sample": list(missing.items())[:8], "missing_chunk_count": len(missing)})
    return found, {
        "active_index_path": str(path),
        "active_index_sha256": sha256_file(path),
        "scanned_index_rows": scanned,
        "matched_index_rows": matched,
        "target_chunk_count": len(target_chunk_ids),
        "active_embedder_ids": list(active_embedders),
    }


def load_vectors(index_records: dict[str, dict[str, IndexRecord]]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    import numpy as np

    by_path: dict[Path, list[IndexRecord]] = defaultdict(list)
    for per_slot in index_records.values():
        for record in per_slot.values():
            by_path[record.record_path].append(record)
    vectors: dict[str, dict[str, Any]] = {chunk_id: {} for chunk_id in index_records}
    record_reads = 0
    bytes_read = 0
    dims: dict[str, int] = {}
    for record_path, records in sorted(by_path.items(), key=lambda item: str(item[0])):
        with record_path.open("rb") as handle:
            for record in sorted(records, key=lambda item: item.byte_offset):
                handle.seek(record.byte_offset)
                raw = handle.read(record.byte_len)
                bytes_read += len(raw)
                require(len(raw) == record.byte_len, "PANELGRAPH_GRAPH_SCORER_RECORD_SHORT_READ", {"record_path": str(record_path), "offset": record.byte_offset})
                observed_record_sha = sha256_bytes(raw)
                require(
                    not record.record_sha256 or observed_record_sha == record.record_sha256,
                    "PANELGRAPH_GRAPH_SCORER_RECORD_SHA_MISMATCH",
                    {"record_path": str(record_path), "expected": record.record_sha256, "observed": observed_record_sha},
                )
                payload = json.loads(raw.decode("utf-8"))
                key = payload.get("key") or {}
                output = payload.get("output") or {}
                require(str(key.get("chunk_id") or "") == record.chunk_id, "PANELGRAPH_GRAPH_SCORER_RECORD_CHUNK_MISMATCH", {"expected": record.chunk_id, "record_path": str(record_path)})
                require(str(key.get("embedder_id") or "").lower() == record.embedder_id, "PANELGRAPH_GRAPH_SCORER_RECORD_EMBEDDER_MISMATCH", {"expected": record.embedder_id, "record_path": str(record_path)})
                vector = output.get("vector")
                require(isinstance(vector, list) and vector, "PANELGRAPH_GRAPH_SCORER_RECORD_VECTOR_MISSING", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                arr = np.asarray(vector, dtype=np.float64)
                require(arr.ndim == 1 and arr.size == record.vector_len, "PANELGRAPH_GRAPH_SCORER_RECORD_VECTOR_DIM_MISMATCH", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                require(bool(np.all(np.isfinite(arr))), "PANELGRAPH_GRAPH_SCORER_RECORD_VECTOR_NONFINITE", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                observed_vector_sha = sha256_bytes(np.asarray(arr, dtype="<f4").tobytes())
                require(observed_vector_sha == record.vector_sha256, "PANELGRAPH_GRAPH_SCORER_VECTOR_SHA_MISMATCH", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id, "expected": record.vector_sha256, "observed": observed_vector_sha})
                vectors[record.chunk_id][record.embedder_id] = arr
                dims.setdefault(record.embedder_id, int(arr.size))
                require(dims[record.embedder_id] == int(arr.size), "PANELGRAPH_GRAPH_SCORER_SLOT_DIM_CONFLICT", record.embedder_id)
                record_reads += 1
    return vectors, {
        "record_paths_read": len(by_path),
        "record_reads": record_reads,
        "bytes_read": bytes_read,
        "slot_dims": dict(sorted(dims.items())),
    }


def score_summary_features(row: EvalRow, chunk_scores: dict[tuple[str, str, str], float], shape_classes: tuple[str, ...]) -> list[float]:
    import numpy as np

    scores = np.asarray([chunk_scores[(row.task_id, row.category, chunk_id)] for chunk_id in row.chunk_ids], dtype=np.float64)
    shape_flags = [1.0 if row.shape_class == shape else 0.0 for shape in shape_classes]
    return [
        float(len(scores)),
        float(row.patch_touched_file_count),
        float(row.patch_touched_chunk_count),
        float(row.candidate_state_chunk_count),
        float(scores.mean()),
        float(scores.std()),
        float(scores.min()),
        float(scores.max()),
        float(scores.max() - scores.min()),
        *shape_flags,
    ]


def vector_features_for_row(row: EvalRow, vectors: dict[str, dict[str, Any]], active_embedders: tuple[str, ...]) -> list[float]:
    import numpy as np

    features: list[float] = []
    for embedder_id in active_embedders:
        matrix = np.vstack([vectors[chunk_id][embedder_id] for chunk_id in row.chunk_ids]).astype(np.float64)
        mean_vec = matrix.mean(axis=0)
        std_vec = matrix.std(axis=0)
        range_vec = matrix.max(axis=0) - matrix.min(axis=0)
        root_vec = matrix[0]
        features.extend(mean_vec.tolist())
        features.extend(std_vec.tolist())
        features.extend(range_vec.tolist())
        features.extend(root_vec.tolist())
        pairwise_cos: list[float] = []
        pairwise_l2: list[float] = []
        if matrix.shape[0] > 1:
            norms = np.linalg.norm(matrix, axis=1)
            for left in range(matrix.shape[0]):
                for right in range(left + 1, matrix.shape[0]):
                    denom = float(norms[left] * norms[right])
                    pairwise_cos.append(0.0 if denom == 0.0 else float(np.dot(matrix[left], matrix[right]) / denom))
                    pairwise_l2.append(float(np.linalg.norm(matrix[left] - matrix[right])))
        for values in (pairwise_cos, pairwise_l2):
            arr = np.asarray(values or [0.0], dtype=np.float64)
            features.extend([float(arr.mean()), float(arr.std()), float(arr.min()), float(arr.max())])
    return features


def build_feature_matrices(
    eval_rows: list[EvalRow],
    chunk_scores: dict[tuple[str, str, str], float],
    vectors: dict[str, dict[str, Any]],
    active_embedders: tuple[str, ...],
) -> tuple[dict[str, Any], dict[str, Any]]:
    import numpy as np

    shape_classes = tuple(sorted({row.shape_class for row in eval_rows}))
    rows_by_variant: dict[str, list[list[float]]] = {
        "chunk_score_summary": [],
        "graph_vector_summary": [],
        "graph_vector_no_chunk_scores": [],
    }
    for row in eval_rows:
        score_features = score_summary_features(row, chunk_scores, shape_classes)
        vector_features = vector_features_for_row(row, vectors, active_embedders)
        rows_by_variant["chunk_score_summary"].append(score_features)
        rows_by_variant["graph_vector_summary"].append([*score_features, *vector_features])
        rows_by_variant["graph_vector_no_chunk_scores"].append(
            score_features[:4] + score_features[9:] + vector_features
        )
    matrices = {
        variant: np.asarray(rows, dtype=np.float64)
        for variant, rows in rows_by_variant.items()
    }
    summary = {
        variant: {"row_count": int(matrix.shape[0]), "feature_count": int(matrix.shape[1])}
        for variant, matrix in matrices.items()
    }
    summary["shape_classes"] = list(shape_classes)
    return matrices, summary


def fit_ridge(variant: str, X: Any, y: Any, alpha: float) -> RidgeModel:
    import numpy as np

    require(alpha > 0.0 and math.isfinite(alpha), "PANELGRAPH_GRAPH_SCORER_RIDGE_ALPHA_INVALID", alpha)
    mean = X.mean(axis=0)
    std = X.std(axis=0)
    keep = std > 1.0e-10
    require(bool(np.any(keep)), "PANELGRAPH_GRAPH_SCORER_FEATURES_CONSTANT", variant)
    Z = (X[:, keep] - mean[keep]) / std[keep]
    label_mean = float(y.mean())
    centered = y - label_mean
    if Z.shape[1] <= Z.shape[0]:
        lhs = Z.T @ Z + np.eye(Z.shape[1], dtype=np.float64) * alpha
        rhs = Z.T @ centered
        try:
            coef = np.linalg.solve(lhs, rhs)
        except np.linalg.LinAlgError:
            coef = np.linalg.pinv(lhs) @ rhs
    else:
        lhs = Z @ Z.T + np.eye(Z.shape[0], dtype=np.float64) * alpha
        try:
            dual = np.linalg.solve(lhs, centered)
        except np.linalg.LinAlgError:
            dual = np.linalg.pinv(lhs) @ centered
        coef = Z.T @ dual
    return RidgeModel(
        variant=variant,
        alpha=float(alpha),
        feature_count=int(X.shape[1]),
        kept_feature_count=int(Z.shape[1]),
        train_examples=int(X.shape[0]),
        label_mean=label_mean,
        feature_mean=mean,
        feature_std=std,
        keep_mask=keep,
        coefficients=coef,
    )


def predict(model: RidgeModel, X: Any) -> Any:
    import numpy as np

    Z = (X[:, model.keep_mask] - model.feature_mean[model.keep_mask]) / model.feature_std[model.keep_mask]
    scores = model.label_mean + Z @ model.coefficients
    return np.clip(scores, 0.0, 1.0)


def pearson(labels: list[int], scores: list[float]) -> float | None:
    if len(labels) < 2 or len(labels) != len(scores):
        return None
    lx = [float(v) for v in labels]
    sx = [float(v) for v in scores]
    mean_l = sum(lx) / len(lx)
    mean_s = sum(sx) / len(sx)
    var_l = sum((v - mean_l) ** 2 for v in lx)
    var_s = sum((v - mean_s) ** 2 for v in sx)
    if var_l <= 0.0 or var_s <= 0.0:
        return None
    cov = sum((a - mean_l) * (b - mean_s) for a, b in zip(lx, sx, strict=True))
    return cov / math.sqrt(var_l * var_s)


def brier(labels: list[int], scores: list[float]) -> float | None:
    if not labels:
        return None
    return sum((float(score) - float(label)) ** 2 for label, score in zip(labels, scores, strict=True)) / len(labels)


def accuracy(labels: list[int], scores: list[float]) -> float | None:
    if not labels:
        return None
    return sum(1 for label, score in zip(labels, scores, strict=True) if (score >= 0.5) == bool(label)) / len(labels)


def metric_block(labels: list[int], scores: list[float]) -> dict[str, Any]:
    return {
        "row_count": len(labels),
        "pearson": pearson(labels, scores),
        "brier": brier(labels, scores),
        "accuracy": accuracy(labels, scores),
    }


def metrics_by_split(eval_rows: list[EvalRow], scores: list[float]) -> dict[str, dict[str, Any]]:
    labels: dict[str, list[int]] = defaultdict(list)
    by_split_scores: dict[str, list[float]] = defaultdict(list)
    for row, score in zip(eval_rows, scores, strict=True):
        labels[row.split].append(row.label)
        by_split_scores[row.split].append(float(score))
    return {
        split: metric_block(labels[split], by_split_scores[split])
        for split in sorted(labels)
    }


def evaluate_models(
    eval_rows: list[EvalRow],
    matrices: dict[str, Any],
    chunk_scores: dict[tuple[str, str, str], float],
    ridge_alpha: float,
) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]]]:
    import numpy as np

    train_idx = [index for index, row in enumerate(eval_rows) if row.split == "calibration"]
    y = np.asarray([row.label for row in eval_rows], dtype=np.float64)
    y_train = y[train_idx]
    row_level_scores = [row.row_level_score for row in eval_rows]
    chunk_mean_scores = [
        float(np.mean([chunk_scores[(row.task_id, row.category, chunk_id)] for chunk_id in row.chunk_ids]))
        for row in eval_rows
    ]
    predictions: dict[str, list[float]] = {
        "row_level_baseline": row_level_scores,
        "chunk_mean_baseline": chunk_mean_scores,
    }
    model_rows: list[dict[str, Any]] = []
    for variant, X in matrices.items():
        model = fit_ridge(variant, X[train_idx], y_train, ridge_alpha)
        scores = predict(model, X)
        predictions[variant] = [float(value) for value in scores.tolist()]
        model_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "panelgraph_graph_scorer_model_summary",
                "formula_version": FORMULA_VERSION,
                "variant": variant,
                "ridge_alpha": model.alpha,
                "feature_count": model.feature_count,
                "kept_feature_count": model.kept_feature_count,
                "train_examples": model.train_examples,
                "label_mean": model.label_mean,
                "coefficient_sha256": sha256_bytes(np.asarray(model.coefficients, dtype="<f8").tobytes()),
            }
        )
    metrics = {
        variant: metrics_by_split(eval_rows, scores)
        for variant, scores in predictions.items()
    }
    return predictions, metrics, model_rows


def delta(left: float | None, right: float | None) -> float | None:
    if left is None or right is None:
        return None
    return float(left) - float(right)


def decision_from_metrics(metrics: dict[str, Any], primary_variant: str, min_lift: float) -> dict[str, Any]:
    row_holdout = metrics["row_level_baseline"]["holdout"]
    graph_holdout = metrics[primary_variant]["holdout"]
    pearson_delta = delta(graph_holdout.get("pearson"), row_holdout.get("pearson"))
    brier_delta = delta(graph_holdout.get("brier"), row_holdout.get("brier"))
    lift_observed = (
        pearson_delta is not None
        and pearson_delta >= min_lift
        and (brier_delta is None or brier_delta <= 0.0)
    )
    return {
        "primary_variant": primary_variant,
        "min_holdout_pearson_lift": min_lift,
        "holdout_pearson_delta_vs_row_level": pearson_delta,
        "holdout_brier_delta_vs_row_level": brier_delta,
        "graph_head_holdout_lift_observed": lift_observed,
        "promotion_allowed": False,
        "serving_checkpoint_changed": False,
        "next_required_artifact": (
            "full_9_slot_graph_head_training_fsv"
            if lift_observed
            else "richer_panel_or_execution_axis_before_panelgraph_graph_head_promotion"
        ),
        "phase2_blockers": [] if lift_observed else ["graph_head_no_holdout_lift_vs_row_level_baseline"],
    }


def prediction_rows(eval_rows: list[EvalRow], predictions: dict[str, list[float]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for index, row in enumerate(eval_rows):
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "panelgraph_graph_scorer_prediction_row",
                "formula_version": FORMULA_VERSION,
                "split": row.split,
                "task_id": row.task_id,
                "repo": row.repo,
                "category": row.category,
                "shape_class": row.shape_class,
                "chunk_count": len(row.chunk_ids),
                "scores": {variant: float(values[index]) for variant, values in sorted(predictions.items())},
            }
        )
    return rows


def artifact_hashes(paths: dict[str, Path]) -> dict[str, str]:
    return {name: sha256_file(path) for name, path in sorted(paths.items())}


def copy_to_state(fsv_paths: list[Path], fsv_root: Path, state_root: Path) -> list[Path]:
    copied: list[Path] = []
    for path in fsv_paths:
        target = state_root / path.relative_to(fsv_root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        copied.append(target)
    return copied


def mounted_drive_hits(paths: Iterable[Path]) -> int:
    hits = 0
    for path in paths:
        text = path.read_text(encoding="utf-8")
        hits += text.count("/mnt/c/") + text.count("/mnt/d/")
    return hits


def run_probe(args: argparse.Namespace) -> dict[str, Any]:
    phase1_root = Path(args.phase1_root)
    chunk_verdict_root = Path(args.chunk_verdict_root)
    eval_rows_path = Path(args.phase1_eval_rows or (phase1_root / "panelgraph_phase1_offline_eval_rows.jsonl"))
    chunk_scores_path = Path(args.per_chunk_verdict_rows or (chunk_verdict_root / "panelgraph_per_chunk_verdict_rows.jsonl"))
    active_index = Path(args.active_index)
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    run_id = args.run_id or f"run-{utc_stamp()}-panelgraph-graph-scorer"
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    active_embedders = parse_active_embedders(str(args.active_embedder_ids))
    allow_non_aiwonder_roots = bool(args.allow_non_aiwonder_roots)

    for label, path in (
        ("phase1_root", phase1_root),
        ("chunk_verdict_root", chunk_verdict_root),
        ("phase1_eval_rows", eval_rows_path),
        ("per_chunk_verdict_rows", chunk_scores_path),
        ("active_index", active_index),
        ("output_root", output_root),
        ("state_root", state_root),
    ):
        require_archive_path(path, label, allow_non_aiwonder_roots)
    if args.clean:
        shutil.rmtree(fsv_run_root, ignore_errors=True)
        shutil.rmtree(state_run_root, ignore_errors=True)
    require(not fsv_run_root.exists(), "PANELGRAPH_GRAPH_SCORER_FSV_ROOT_EXISTS", str(fsv_run_root))
    require(not state_run_root.exists(), "PANELGRAPH_GRAPH_SCORER_STATE_ROOT_EXISTS", str(state_run_root))
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    eval_rows = load_eval_rows(eval_rows_path)
    chunk_scores, chunk_score_summary = load_chunk_scores(chunk_scores_path, eval_rows)
    target_chunk_ids = {chunk_id for row in eval_rows for chunk_id in row.chunk_ids}
    index_records, index_summary = load_active_index(
        active_index,
        target_chunk_ids=target_chunk_ids,
        active_embedders=active_embedders,
        allow_non_aiwonder_roots=allow_non_aiwonder_roots,
    )
    vectors, vector_summary = load_vectors(index_records)
    matrices, feature_summary = build_feature_matrices(eval_rows, chunk_scores, vectors, active_embedders)
    predictions, metrics, model_rows = evaluate_models(eval_rows, matrices, chunk_scores, float(args.ridge_alpha))
    primary_variant = str(args.primary_variant)
    require(primary_variant in metrics, "PANELGRAPH_GRAPH_SCORER_PRIMARY_VARIANT_UNKNOWN", primary_variant)
    decision = decision_from_metrics(metrics, primary_variant, float(args.min_holdout_pearson_lift))
    pred_rows = prediction_rows(eval_rows, predictions)
    forbidden_prediction_keys = sorted(TARGET_SIDE_KEYS.intersection(set().union(*(row.keys() for row in pred_rows))))
    require(not forbidden_prediction_keys, "PANELGRAPH_GRAPH_SCORER_TARGET_KEYS_IN_PREDICTIONS", forbidden_prediction_keys)

    predictions_path = fsv_run_root / "panelgraph_graph_scorer_predictions.jsonl"
    model_path = fsv_run_root / "panelgraph_graph_scorer_model_summary.jsonl"
    manifest_path = fsv_run_root / "panelgraph_graph_scorer_manifest.json"
    write_jsonl(predictions_path, pred_rows)
    write_jsonl(model_path, model_rows)
    output_hits_before_manifest = mounted_drive_hits([predictions_path, model_path])
    split_counts = Counter(row.split for row in eval_rows)
    category_counts = Counter(row.category for row in eval_rows)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "issue": 726,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "inputs": {
            "phase1_root": str(phase1_root),
            "chunk_verdict_root": str(chunk_verdict_root),
            "phase1_eval_rows": str(eval_rows_path),
            "per_chunk_verdict_rows": str(chunk_scores_path),
            "active_index": str(active_index),
            "active_embedder_ids": list(active_embedders),
            "ridge_alpha": float(args.ridge_alpha),
            "primary_variant": primary_variant,
        },
        "source": {
            "source_commit": str(getattr(args, "source_commit", "") or ""),
            "runner_path": str(Path(__file__).resolve()),
            "runner_sha256": sha256_file(Path(__file__).resolve()),
        },
        "summary": {
            "eval_row_count": len(eval_rows),
            "split_counts": dict(sorted(split_counts.items())),
            "category_counts": dict(sorted(category_counts.items())),
            "unique_chunk_count": len(target_chunk_ids),
            "chunk_scores": chunk_score_summary,
            "index": index_summary,
            "vectors": vector_summary,
            "features": feature_summary,
            "metrics": metrics,
        },
        "decision": decision,
        "policy": {
            "measurement_only": True,
            "offline_calibration_only": True,
            "target_labels_used_as_live_inputs": False,
            "target_labels_used_for_offline_calibration_only": True,
            "slot_identity_preserved": True,
            "per_slot_graph_summaries": True,
            "flat_vector_concat_used": False,
            "row_level_scores_used_as_features": False,
            "predictor_weights_changed": False,
            "production_checkpoint_modified": False,
            "panel_schema_changed": False,
            "serving_pointer_modified": False,
            "inner_llm_used": False,
        },
        "artifacts": {
            "predictions": str(predictions_path),
            "model_summary": str(model_path),
        },
        "state_run_root": str(state_run_root),
        "mounted_drive_hits_in_outputs_before_manifest": output_hits_before_manifest,
        "passes": output_hits_before_manifest == 0,
    }
    write_json(manifest_path, manifest)
    fsv_paths = [predictions_path, model_path, manifest_path]
    state_paths = copy_to_state(fsv_paths, fsv_run_root, state_run_root)
    hash_path = fsv_run_root / "artifact_hashes.json"
    hash_payload = artifact_hashes({f"fsv/{path.name}": path for path in fsv_paths} | {f"state/{path.name}": path for path in state_paths})
    write_json(hash_path, hash_payload)
    shutil.copy2(hash_path, state_run_root / hash_path.name)
    final_paths = [*fsv_paths, hash_path, *state_paths, state_run_root / hash_path.name]
    final_hits = mounted_drive_hits(final_paths)
    manifest["mounted_drive_hits_in_outputs"] = final_hits
    manifest["artifact_hashes"] = str(hash_path)
    manifest["state_mirror_byte_identical"] = all((state_run_root / path.name).read_bytes() == path.read_bytes() for path in fsv_paths)
    manifest["passes"] = final_hits == 0 and manifest["state_mirror_byte_identical"]
    write_json(manifest_path, manifest)
    shutil.copy2(manifest_path, state_run_root / manifest_path.name)
    hash_payload = artifact_hashes({f"fsv/{path.name}": path for path in fsv_paths} | {f"state/{path.name}": state_run_root / path.name for path in fsv_paths})
    write_json(hash_path, hash_payload)
    shutil.copy2(hash_path, state_run_root / hash_path.name)
    return {
        "manifest": str(manifest_path),
        "state_manifest": str(state_run_root / manifest_path.name),
        "artifact_hashes": str(hash_path),
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "passes": manifest["passes"],
        "decision": decision,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase1-root", type=Path, default=DEFAULT_PHASE1_ROOT)
    parser.add_argument("--chunk-verdict-root", type=Path, default=DEFAULT_CHUNK_VERDICT_ROOT)
    parser.add_argument("--phase1-eval-rows", type=Path, default=None)
    parser.add_argument("--per-chunk-verdict-rows", type=Path, default=None)
    parser.add_argument("--active-index", type=Path, default=DEFAULT_ACTIVE_INDEX)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--active-embedder-ids", default=",".join(DEFAULT_ACTIVE_EMBEDDERS))
    parser.add_argument("--ridge-alpha", type=float, default=25.0)
    parser.add_argument("--primary-variant", default="graph_vector_summary")
    parser.add_argument("--min-holdout-pearson-lift", type=float, default=0.005)
    parser.add_argument("--source-commit", default="")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser.parse_args()


def main() -> int:
    try:
        result = run_probe(parse_args())
    except PanelGraphGraphScorerError as exc:
        print(json.dumps({"passes": False, "error": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0 if result["passes"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
