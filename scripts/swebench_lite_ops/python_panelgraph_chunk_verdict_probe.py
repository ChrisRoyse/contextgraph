#!/usr/bin/env python3
"""Generate #726 PanelGraph per-chunk verdict rows from active chunk vectors.

This runner is measurement-only. It fits deterministic per-slot ridge heads on
calibration chunks from the existing #726 Phase 1 eval rows, predicts per-AST-
chunk pass scores for the same calibration/holdout rows, and writes the narrow
`task_id, category, chunk_id, predicted_oracle_pass` contract consumed by
`python_panelgraph_phase1_audit.py`.

It does not clone row-level scores into chunks, does not promote a PanelGraph
schema, and does not change any production predictor or checkpoint.
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
ARTIFACT_KIND = "panelgraph_chunk_verdict_probe"
FORMULA_VERSION = "panelgraph_chunk_verdict_v1"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_PHASE1_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-phase1-fsv/"
    "run-20260602T232844Z-panelgraph-verdict-contract-91adb824"
)
DEFAULT_ACTIVE_INDEX = Path(
    "/zfs/archive/contextgraph/state/forward-cache-incremental-v3/"
    "run-20260603T011900Z-full-active-index-1683a41b/active_index/active_index.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-chunk-verdict-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/panelgraph-chunk-verdict/blindspot-004-v1")
DEFAULT_ACTIVE_EMBEDDERS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_per_test_count",
    "oracle_verdict_sha256",
    "resolved_ids",
    "unresolved_ids",
}


class PanelGraphChunkVerdictError(RuntimeError):
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
    primary_chunk_ids: tuple[str, ...]


@dataclass(frozen=True)
class ChunkInstance:
    instance_index: int
    row: EvalRow
    chunk_id: str


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
class SlotModel:
    embedder_id: str
    input_dim: int
    kept_dim: int
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
        raise PanelGraphChunkVerdictError(f"{code}{suffix}")


def reject_mounted_drive_fragment(label: str, value: str) -> None:
    require(
        "/mnt/c/" not in value and "/mnt/d/" not in value,
        "PANELGRAPH_CHUNK_VERDICT_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    text = str(path)
    require(
        text.startswith(AIWONDER_ARCHIVE_ROOT),
        "PANELGRAPH_CHUNK_VERDICT_NON_AIWONDER_DURABLE_PATH",
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
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise PanelGraphChunkVerdictError(f"PANELGRAPH_CHUNK_VERDICT_WRITE_READBACK_MISMATCH: {path}")


def write_json(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    count = 0
    lines: list[str] = []
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
                raise PanelGraphChunkVerdictError(f"{code}: invalid JSON at {path}:{line_no}: {exc}") from exc
            require(isinstance(row, dict), f"{code}_ROW_NOT_OBJECT", line_no)
            rows.append(row)
    return rows


def probability(value: Any, label: str) -> float:
    require(type(value) in (int, float) and math.isfinite(float(value)), "PANELGRAPH_CHUNK_VERDICT_SCORE_INVALID", {label: value})
    score = float(value)
    require(0.0 <= score <= 1.0, "PANELGRAPH_CHUNK_VERDICT_SCORE_OUT_OF_RANGE", {label: value})
    return score


def parse_active_embedders(value: str) -> tuple[str, ...]:
    embedders = tuple(part.strip().lower() for part in value.split(",") if part.strip())
    require(bool(embedders), "PANELGRAPH_CHUNK_VERDICT_ACTIVE_EMBEDDERS_EMPTY")
    require(len(set(embedders)) == len(embedders), "PANELGRAPH_CHUNK_VERDICT_DUPLICATE_ACTIVE_EMBEDDER", embedders)
    return embedders


def load_eval_rows(path: Path) -> list[EvalRow]:
    rows = []
    seen: set[tuple[str, str, str]] = set()
    for index, raw in enumerate(read_jsonl(path, "PANELGRAPH_CHUNK_VERDICT_EVAL_ROWS_MISSING")):
        for key, value in raw.items():
            if isinstance(value, str):
                reject_mounted_drive_fragment(key, value)
        split = str(raw.get("split") or "")
        require(split in {"calibration", "holdout"}, "PANELGRAPH_CHUNK_VERDICT_SPLIT_INVALID", {"index": index, "split": split})
        task_id = str(raw.get("task_id") or "")
        repo = str(raw.get("repo") or "")
        category = str(raw.get("category") or "")
        require(task_id and repo and category, "PANELGRAPH_CHUNK_VERDICT_EVAL_KEY_MISSING", index)
        label_raw = raw.get("actual_oracle_pass")
        require(isinstance(label_raw, bool), "PANELGRAPH_CHUNK_VERDICT_EVAL_LABEL_MISSING", index)
        score = probability(raw.get("predicted_oracle_pass"), f"eval_rows[{index}].predicted_oracle_pass")
        chunks = raw.get("primary_chunk_ids")
        require(isinstance(chunks, list) and chunks, "PANELGRAPH_CHUNK_VERDICT_PRIMARY_CHUNKS_MISSING", {"task_id": task_id, "category": category})
        chunk_ids = tuple(str(chunk) for chunk in chunks)
        require(all(chunk.startswith("sha256:") for chunk in chunk_ids), "PANELGRAPH_CHUNK_VERDICT_CHUNK_ID_INVALID", {"task_id": task_id, "category": category})
        require(len(set(chunk_ids)) == len(chunk_ids), "PANELGRAPH_CHUNK_VERDICT_DUPLICATE_ROW_CHUNK", {"task_id": task_id, "category": category})
        unique_key = (split, task_id, category)
        require(unique_key not in seen, "PANELGRAPH_CHUNK_VERDICT_DUPLICATE_EVAL_ROW", unique_key)
        seen.add(unique_key)
        rows.append(
            EvalRow(
                row_index=index,
                split=split,
                task_id=task_id,
                repo=repo,
                category=category,
                label=1 if label_raw else 0,
                row_level_score=score,
                primary_chunk_ids=chunk_ids,
            )
        )
    require(rows, "PANELGRAPH_CHUNK_VERDICT_EVAL_ROWS_EMPTY")
    require(any(row.split == "calibration" for row in rows), "PANELGRAPH_CHUNK_VERDICT_CALIBRATION_ROWS_EMPTY")
    require(any(row.split == "holdout" for row in rows), "PANELGRAPH_CHUNK_VERDICT_HOLDOUT_ROWS_EMPTY")
    return rows


def build_chunk_instances(rows: list[EvalRow]) -> list[ChunkInstance]:
    instances: list[ChunkInstance] = []
    seen: set[tuple[str, str, str]] = set()
    for row in rows:
        for chunk_id in row.primary_chunk_ids:
            key = (row.task_id, row.category, chunk_id)
            require(key not in seen, "PANELGRAPH_CHUNK_VERDICT_DUPLICATE_CHUNK_INSTANCE", key)
            seen.add(key)
            instances.append(ChunkInstance(instance_index=len(instances), row=row, chunk_id=chunk_id))
    require(instances, "PANELGRAPH_CHUNK_VERDICT_CHUNK_INSTANCES_EMPTY")
    return instances


def load_active_index(
    path: Path,
    *,
    target_chunk_ids: set[str],
    active_embedders: tuple[str, ...],
    allow_non_aiwonder_roots: bool,
) -> tuple[dict[str, dict[str, IndexRecord]], dict[str, Any]]:
    require_archive_path(path, "active_index", allow_non_aiwonder_roots)
    require(path.is_file(), "PANELGRAPH_CHUNK_VERDICT_ACTIVE_INDEX_MISSING", str(path))
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
            require(record.byte_offset >= 0 and record.byte_len > 0, "PANELGRAPH_CHUNK_VERDICT_INDEX_OFFSET_INVALID", {"line": line_no, "row": row})
            require(record.vector_sha256.startswith("sha256:"), "PANELGRAPH_CHUNK_VERDICT_INDEX_VECTOR_SHA_MISSING", {"line": line_no, "row": row})
            require(embedder_id not in found[chunk_id], "PANELGRAPH_CHUNK_VERDICT_DUPLICATE_ACTIVE_INDEX_ROW", {"chunk_id": chunk_id, "embedder_id": embedder_id})
            found[chunk_id][embedder_id] = record
            matched += 1
    missing = {
        chunk_id: [embedder for embedder in active_embedders if embedder not in per_slot]
        for chunk_id, per_slot in found.items()
        if any(embedder not in per_slot for embedder in active_embedders)
    }
    require(not missing, "PANELGRAPH_CHUNK_VERDICT_ACTIVE_INDEX_COVERAGE_INCOMPLETE", {"missing_sample": list(missing.items())[:8], "missing_chunk_count": len(missing)})
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
                require(len(raw) == record.byte_len, "PANELGRAPH_CHUNK_VERDICT_RECORD_SHORT_READ", {"record_path": str(record_path), "offset": record.byte_offset})
                observed_record_sha = sha256_bytes(raw)
                require(
                    not record.record_sha256 or observed_record_sha == record.record_sha256,
                    "PANELGRAPH_CHUNK_VERDICT_RECORD_SHA_MISMATCH",
                    {"record_path": str(record_path), "expected": record.record_sha256, "observed": observed_record_sha},
                )
                payload = json.loads(raw.decode("utf-8"))
                key = payload.get("key") or {}
                output = payload.get("output") or {}
                require(str(key.get("chunk_id") or "") == record.chunk_id, "PANELGRAPH_CHUNK_VERDICT_RECORD_CHUNK_MISMATCH", {"expected": record.chunk_id, "record_path": str(record_path)})
                require(str(key.get("embedder_id") or "").lower() == record.embedder_id, "PANELGRAPH_CHUNK_VERDICT_RECORD_EMBEDDER_MISMATCH", {"expected": record.embedder_id, "record_path": str(record_path)})
                vector = output.get("vector")
                require(isinstance(vector, list) and vector, "PANELGRAPH_CHUNK_VERDICT_RECORD_VECTOR_MISSING", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                arr = np.asarray(vector, dtype=np.float64)
                require(arr.ndim == 1 and arr.size == record.vector_len, "PANELGRAPH_CHUNK_VERDICT_RECORD_VECTOR_DIM_MISMATCH", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                require(bool(np.all(np.isfinite(arr))), "PANELGRAPH_CHUNK_VERDICT_RECORD_VECTOR_NONFINITE", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id})
                observed_vector_sha = sha256_bytes(np.asarray(arr, dtype="<f4").tobytes())
                require(observed_vector_sha == record.vector_sha256, "PANELGRAPH_CHUNK_VERDICT_VECTOR_SHA_MISMATCH", {"chunk_id": record.chunk_id, "embedder_id": record.embedder_id, "expected": record.vector_sha256, "observed": observed_vector_sha})
                vectors[record.chunk_id][record.embedder_id] = arr
                dims.setdefault(record.embedder_id, int(arr.size))
                require(dims[record.embedder_id] == int(arr.size), "PANELGRAPH_CHUNK_VERDICT_SLOT_DIM_CONFLICT", record.embedder_id)
                record_reads += 1
    return vectors, {
        "record_paths_read": len(by_path),
        "record_reads": record_reads,
        "bytes_read": bytes_read,
        "slot_dims": dict(sorted(dims.items())),
    }


def fit_slot_model(
    *,
    embedder_id: str,
    vectors: dict[str, dict[str, Any]],
    training_instances: list[ChunkInstance],
    ridge_alpha: float,
) -> SlotModel:
    import numpy as np

    require(ridge_alpha > 0.0 and math.isfinite(ridge_alpha), "PANELGRAPH_CHUNK_VERDICT_RIDGE_ALPHA_INVALID", ridge_alpha)
    X = np.vstack([vectors[instance.chunk_id][embedder_id] for instance in training_instances]).astype(np.float64)
    y = np.asarray([instance.row.label for instance in training_instances], dtype=np.float64)
    require(len(set(int(value) for value in y.tolist())) == 2, "PANELGRAPH_CHUNK_VERDICT_CALIBRATION_LABELS_DEGENERATE", embedder_id)
    mean = X.mean(axis=0)
    std = X.std(axis=0)
    keep = std > 1.0e-10
    require(bool(np.any(keep)), "PANELGRAPH_CHUNK_VERDICT_SLOT_FEATURES_CONSTANT", embedder_id)
    Z = (X[:, keep] - mean[keep]) / std[keep]
    label_mean = float(y.mean())
    centered = y - label_mean
    alpha = float(ridge_alpha)
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
    return SlotModel(
        embedder_id=embedder_id,
        input_dim=int(X.shape[1]),
        kept_dim=int(Z.shape[1]),
        train_examples=int(X.shape[0]),
        label_mean=label_mean,
        feature_mean=mean,
        feature_std=std,
        keep_mask=keep,
        coefficients=coef,
    )


def predict_slot(model: SlotModel, vector: Any) -> float:
    import numpy as np

    arr = np.asarray(vector, dtype=np.float64)
    z = (arr[model.keep_mask] - model.feature_mean[model.keep_mask]) / model.feature_std[model.keep_mask]
    score = model.label_mean + float(z @ model.coefficients)
    return min(1.0, max(0.0, score))


def metric_block(labels: list[int], scores: list[float]) -> dict[str, Any]:
    return {
        "row_count": len(labels),
        "pearson": pearson(labels, scores),
        "brier": brier(labels, scores),
        "accuracy": accuracy(labels, scores),
    }


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


def score_instances(
    *,
    instances: list[ChunkInstance],
    vectors: dict[str, dict[str, Any]],
    models: dict[str, SlotModel],
    active_embedders: tuple[str, ...],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    verdict_rows: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    clone_exact = 0
    clone_near = 0
    per_row_scores: dict[tuple[str, str], list[float]] = defaultdict(list)
    for instance in instances:
        slot_scores = [predict_slot(models[embedder_id], vectors[instance.chunk_id][embedder_id]) for embedder_id in active_embedders]
        score = sum(slot_scores) / len(slot_scores)
        row = instance.row
        key = (row.task_id, row.category)
        per_row_scores[key].append(score)
        if score == row.row_level_score:
            clone_exact += 1
        if abs(score - row.row_level_score) <= 1.0e-12:
            clone_near += 1
        verdict_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "panelgraph_per_chunk_verdict_row",
                "formula_version": FORMULA_VERSION,
                "task_id": row.task_id,
                "category": row.category,
                "chunk_id": instance.chunk_id,
                "predicted_oracle_pass": score,
                "score_source": "calibration_only_per_slot_ridge_chunk_head",
            }
        )
        diagnostics.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "panelgraph_chunk_verdict_diagnostic_row",
                "formula_version": FORMULA_VERSION,
                "split": row.split,
                "task_id": row.task_id,
                "repo": row.repo,
                "category": row.category,
                "chunk_id": instance.chunk_id,
                "predicted_oracle_pass": score,
                "row_level_predicted_oracle_pass": row.row_level_score,
                "abs_delta_vs_row_level_score": abs(score - row.row_level_score),
                "slot_score_count": len(slot_scores),
                "min_slot_score": min(slot_scores),
                "max_slot_score": max(slot_scores),
            }
        )
    all_row_level_cloned = 0
    for row_scores in per_row_scores.values():
        if row_scores and len(set(round(score, 15) for score in row_scores)) == 1:
            all_row_level_cloned += 1
    return verdict_rows, diagnostics, {
        "chunk_instance_count": len(instances),
        "exact_row_score_equal_chunk_count": clone_exact,
        "near_row_score_equal_chunk_count": clone_near,
        "near_row_score_equal_fraction": clone_near / len(instances) if instances else 0.0,
        "rows_with_constant_chunk_scores": all_row_level_cloned,
        "row_count": len(per_row_scores),
        "row_level_scores_used_as_features": False,
        "row_level_scores_cloned_to_chunks": False,
    }


def summarize_models(models: dict[str, SlotModel], active_embedders: tuple[str, ...], training_instances: list[ChunkInstance]) -> list[dict[str, Any]]:
    import numpy as np

    label_counts = Counter(str(instance.row.label) for instance in training_instances)
    rows = []
    for embedder_id in active_embedders:
        model = models[embedder_id]
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "panelgraph_chunk_slot_model_summary",
                "formula_version": FORMULA_VERSION,
                "embedder_id": embedder_id,
                "input_dim": model.input_dim,
                "kept_dim": model.kept_dim,
                "train_examples": model.train_examples,
                "train_label_counts": dict(sorted(label_counts.items())),
                "label_mean": model.label_mean,
                "coefficient_sha256": sha256_bytes(np.asarray(model.coefficients, dtype="<f8").tobytes()),
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
    phase1_eval_rows = Path(args.phase1_eval_rows or (phase1_root / "panelgraph_phase1_offline_eval_rows.jsonl"))
    active_index = Path(args.active_index)
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    run_id = args.run_id or f"run-{utc_stamp()}-chunk-verdict"
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    active_embedders = parse_active_embedders(str(args.active_embedder_ids))
    allow_non_aiwonder_roots = bool(args.allow_non_aiwonder_roots)

    for label, path in (
        ("phase1_root", phase1_root),
        ("phase1_eval_rows", phase1_eval_rows),
        ("active_index", active_index),
        ("output_root", output_root),
        ("state_root", state_root),
    ):
        require_archive_path(path, label, allow_non_aiwonder_roots)
    if args.clean:
        shutil.rmtree(fsv_run_root, ignore_errors=True)
        shutil.rmtree(state_run_root, ignore_errors=True)
    require(not fsv_run_root.exists(), "PANELGRAPH_CHUNK_VERDICT_FSV_ROOT_EXISTS", str(fsv_run_root))
    require(not state_run_root.exists(), "PANELGRAPH_CHUNK_VERDICT_STATE_ROOT_EXISTS", str(state_run_root))
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    eval_rows = load_eval_rows(phase1_eval_rows)
    instances = build_chunk_instances(eval_rows)
    training_instances = [instance for instance in instances if instance.row.split == "calibration"]
    require(training_instances, "PANELGRAPH_CHUNK_VERDICT_TRAINING_INSTANCES_EMPTY")
    require(len(set(instance.row.label for instance in training_instances)) == 2, "PANELGRAPH_CHUNK_VERDICT_TRAINING_LABELS_DEGENERATE")
    target_chunk_ids = {instance.chunk_id for instance in instances}
    index_records, index_summary = load_active_index(
        active_index,
        target_chunk_ids=target_chunk_ids,
        active_embedders=active_embedders,
        allow_non_aiwonder_roots=allow_non_aiwonder_roots,
    )
    vectors, vector_summary = load_vectors(index_records)
    models = {
        embedder_id: fit_slot_model(
            embedder_id=embedder_id,
            vectors=vectors,
            training_instances=training_instances,
            ridge_alpha=float(args.ridge_alpha),
        )
        for embedder_id in active_embedders
    }
    verdict_rows, diagnostics, clone_audit = score_instances(
        instances=instances,
        vectors=vectors,
        models=models,
        active_embedders=active_embedders,
    )
    forbidden_verdict_keys = sorted(TARGET_SIDE_KEYS.intersection(set().union(*(row.keys() for row in verdict_rows))))
    require(not forbidden_verdict_keys, "PANELGRAPH_CHUNK_VERDICT_TARGET_KEYS_IN_OUTPUT", forbidden_verdict_keys)
    model_rows = summarize_models(models, active_embedders, training_instances)

    labels_by_split: dict[str, list[int]] = defaultdict(list)
    scores_by_split: dict[str, list[float]] = defaultdict(list)
    for instance, diagnostic in zip(instances, diagnostics, strict=True):
        labels_by_split[instance.row.split].append(instance.row.label)
        scores_by_split[instance.row.split].append(float(diagnostic["predicted_oracle_pass"]))
    metrics = {
        split: metric_block(labels_by_split[split], scores_by_split[split])
        for split in sorted(labels_by_split)
    }

    verdict_path = fsv_run_root / "panelgraph_per_chunk_verdict_rows.jsonl"
    diagnostics_path = fsv_run_root / "panelgraph_chunk_score_diagnostics.jsonl"
    model_path = fsv_run_root / "panelgraph_chunk_slot_model_summary.jsonl"
    manifest_path = fsv_run_root / "panelgraph_chunk_verdict_manifest.json"
    write_jsonl(verdict_path, verdict_rows)
    write_jsonl(diagnostics_path, diagnostics)
    write_jsonl(model_path, model_rows)

    output_hits_before_manifest = mounted_drive_hits([verdict_path, diagnostics_path, model_path])
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
            "phase1_eval_rows": str(phase1_eval_rows),
            "active_index": str(active_index),
            "ridge_alpha": float(args.ridge_alpha),
            "active_embedder_ids": list(active_embedders),
        },
        "summary": {
            "eval_row_count": len(eval_rows),
            "split_counts": dict(sorted(split_counts.items())),
            "category_counts": dict(sorted(category_counts.items())),
            "chunk_instance_count": len(instances),
            "unique_chunk_count": len(target_chunk_ids),
            "training_chunk_instance_count": len(training_instances),
            "verdict_row_count": len(verdict_rows),
            "index": index_summary,
            "vectors": vector_summary,
            "clone_audit": clone_audit,
            "chunk_metrics_reporting_only": metrics,
        },
        "policy": {
            "measurement_only": True,
            "per_slot_heads": True,
            "late_fusion_policy": "uniform_mean_of_per_slot_chunk_scores",
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "target_labels_used_for_offline_calibration_only": True,
            "row_level_scores_used_as_features": False,
            "row_level_scores_cloned_to_chunks": False,
            "predictor_weights_changed": False,
            "production_checkpoint_modified": False,
            "panel_schema_changed": False,
            "inner_llm_used": False,
        },
        "artifacts": {
            "per_chunk_verdict_rows": str(verdict_path),
            "chunk_score_diagnostics": str(diagnostics_path),
            "slot_model_summary": str(model_path),
        },
        "state_run_root": str(state_run_root),
        "mounted_drive_hits_in_outputs_before_manifest": output_hits_before_manifest,
        "passes": output_hits_before_manifest == 0,
    }
    write_json(manifest_path, manifest)
    fsv_paths = [verdict_path, diagnostics_path, model_path, manifest_path]
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
        "per_chunk_verdict_rows": str(verdict_path),
        "passes": manifest["passes"],
        "verdict_row_count": len(verdict_rows),
        "unique_chunk_count": len(target_chunk_ids),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase1-root", type=Path, default=DEFAULT_PHASE1_ROOT)
    parser.add_argument("--phase1-eval-rows", type=Path, default=None)
    parser.add_argument("--active-index", type=Path, default=DEFAULT_ACTIVE_INDEX)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--active-embedder-ids", default=",".join(DEFAULT_ACTIVE_EMBEDDERS))
    parser.add_argument("--ridge-alpha", type=float, default=25.0)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser.parse_args()


def main() -> int:
    try:
        result = run_probe(parse_args())
    except PanelGraphChunkVerdictError as exc:
        print(str(exc), flush=True)
        return 2
    print(canonical_json(result), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
