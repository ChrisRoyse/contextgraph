"""Compute representation-intelligence matrices for the 1,000 chunk smoke run."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np


SCHEMA_VERSION = 1
DEFAULT_FORWARD_ROOT = Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/forward-cache")
DEFAULT_SMOKE_MANIFEST = Path(
    "/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices"
)
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv")
DEFAULT_TASK_ID = "TASK-PY-DATA-062"
DEFAULT_MIN_OVERLAP = 20
DEFAULT_NEIGHBOR_K = 10
CCA_MAX_DIM_SUM = 900
EPS = 1.0e-12


class SmokeMatrixError(RuntimeError):
    pass


@dataclass(frozen=True)
class VectorRecord:
    row_key: str
    embedder_id: str
    vector: np.ndarray
    chunk_id: str
    task_instance_id: str
    workspace_state: str
    repo: str
    relative_path: str
    mutation_category: str
    chunk_kind: str
    node_kind: str
    oracle_all_passed: str
    oracle_resolved: str
    precision_class: str
    model_hash: str
    runtime_model_version: str
    instrument_id: str
    vector_sha256: str
    record_sha256: str
    cache_key_sha256: str
    shard_id: str
    source_sha256: str


def sha256_bytes(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def utc_now() -> str:
    import datetime as _dt

    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeMatrixError(message)


def read_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as fh:
            return json.load(fh)
    except FileNotFoundError as exc:
        raise SmokeMatrixError(f"required JSON file missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SmokeMatrixError(f"invalid JSON in {path}: {exc}") from exc


def iter_jsonl(path: Path):
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                yield line_no, json.loads(line)
            except json.JSONDecodeError as exc:
                raise SmokeMatrixError(f"invalid JSONL at {path}:{line_no}: {exc}") from exc


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(value, fh, sort_keys=True, indent=2, separators=(",", ": "))
        fh.write("\n")
    tmp.replace(path)


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            fh.write("\n")
    tmp.replace(path)


def write_csv(path: Path, rows: list[dict[str, Any]], fieldnames: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=fieldnames, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow(row)
    tmp.replace(path)


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "file_count": 0, "byte_count": 0, "hash_sample": []}
    if path.is_file():
        return {
            "path": str(path),
            "exists": True,
            "file_count": 1,
            "byte_count": path.stat().st_size,
            "hash_sample": [f"{path.name}:{sha256_file(path)}"],
        }
    file_count = 0
    byte_count = 0
    samples: list[str] = []
    for child in sorted(path.rglob("*")):
        if not child.is_file():
            continue
        file_count += 1
        byte_count += child.stat().st_size
        if len(samples) < 20:
            samples.append(f"{child.relative_to(path)}:{sha256_file(child)}")
    return {
        "path": str(path),
        "exists": True,
        "file_count": file_count,
        "byte_count": byte_count,
        "hash_sample": samples,
        "hash_sample_truncated": file_count > len(samples),
    }


def row_key_from_fields(task_instance_id: str, workspace_state: str, chunk_id: str, source_sha256: str) -> str:
    return f"{task_instance_id}|{workspace_state}|{chunk_id}|{source_sha256}"


def load_smoke_manifest(path: Path) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    by_key: dict[str, dict[str, Any]] = {}
    counts = {
        "rows": 0,
        "repos": Counter(),
        "mutation_categories": Counter(),
        "chunk_kinds": Counter(),
        "oracle_all_passed": Counter(),
        "oracle_resolved": Counter(),
    }
    for _, row in iter_jsonl(path):
        task_instance_id = str(row.get("task_instance_id") or "")
        workspace_state = str(row.get("workspace_state") or "")
        chunk_id = str(row.get("chunk_id") or "")
        source_sha256 = str(row.get("source_sha256") or "")
        require(task_instance_id and workspace_state and chunk_id and source_sha256, f"manifest row missing stable key fields: {path}")
        key = row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)
        require(key not in by_key, f"duplicate smoke manifest row key: {key}")
        oracle = row.get("oracle_linkage") or {}
        row["_oracle_all_passed_str"] = str(oracle.get("all_passed"))
        row["_oracle_resolved_str"] = str(oracle.get("resolved"))
        by_key[key] = row
        counts["rows"] += 1
        counts["repos"][str(row.get("repo") or "unknown")] += 1
        counts["mutation_categories"][str(row.get("mutation_category") or "unknown")] += 1
        counts["chunk_kinds"][str(row.get("chunk_kind") or "unknown")] += 1
        counts["oracle_all_passed"][row["_oracle_all_passed_str"]] += 1
        counts["oracle_resolved"][row["_oracle_resolved_str"]] += 1
    summary = {
        "rows": counts["rows"],
        "repos": dict(sorted(counts["repos"].items())),
        "mutation_categories": dict(sorted(counts["mutation_categories"].items())),
        "chunk_kinds": dict(sorted(counts["chunk_kinds"].items())),
        "oracle_all_passed": dict(sorted(counts["oracle_all_passed"].items())),
        "oracle_resolved": dict(sorted(counts["oracle_resolved"].items())),
        "sha256": sha256_file(path),
        "path": str(path),
    }
    return by_key, summary


def load_quarantine_counts(shard_root: Path) -> Counter:
    counts: Counter = Counter()
    path = shard_root / "python_forward_cache_quarantine.jsonl"
    if not path.exists():
        return counts
    for _, row in iter_jsonl(path):
        counts[str(row.get("embedder_id") or "unknown")] += 1
    return counts


def load_records(forward_root: Path, manifest_by_key: dict[str, dict[str, Any]]) -> tuple[dict[str, dict[str, VectorRecord]], dict[str, Any]]:
    corpus_manifest = read_json(forward_root / "python_forward_cache_corpus_manifest.json")
    require(corpus_manifest.get("passes") is True, "corpus manifest must have passes=true")
    records: dict[str, dict[str, VectorRecord]] = defaultdict(dict)
    telemetry_rows = 0
    quarantine_counts: Counter = Counter()
    verified_record_rows = 0
    index_rows_verified = 0
    vector_dims: dict[str, set[int]] = defaultdict(set)
    shard_ids: list[str] = []
    for shard in sorted(corpus_manifest.get("shards") or [], key=lambda s: str(s.get("shard_id"))):
        shard_id = str(shard.get("shard_id") or "")
        shard_ids.append(shard_id)
        shard_root = forward_root / "shards" / shard_id
        packed_manifest = read_json(shard_root / "python_forward_cache_packed_manifest.json")
        require(packed_manifest.get("passes") is True, f"{shard_id} packed manifest must pass")
        quarantine_counts.update(load_quarantine_counts(shard_root))
        telemetry_path = shard_root / "python_forward_cache_batch_telemetry.jsonl"
        if telemetry_path.exists():
            telemetry_rows += sum(1 for _ in iter_jsonl(telemetry_path))
        for embedder_id, entry in sorted((packed_manifest.get("per_embedder") or {}).items()):
            index_path = Path(entry["index_path"])
            record_path = Path(entry["record_path"])
            index_by_cache_key: dict[str, dict[str, Any]] = {}
            for _, index_row in iter_jsonl(index_path):
                cache_key = str(index_row["cache_key_sha256"])
                require(cache_key not in index_by_cache_key, f"duplicate packed cache key {cache_key} in {index_path}")
                index_by_cache_key[cache_key] = index_row
            index_rows_verified += len(index_by_cache_key)
            seen_cache_keys: set[str] = set()
            with record_path.open("r", encoding="utf-8") as fh:
                for raw_line_no, raw in enumerate(fh, start=1):
                    if not raw.strip():
                        continue
                    row = json.loads(raw)
                    cache_key = str(row.get("cache_key_sha256") or "")
                    index_row = index_by_cache_key.get(cache_key)
                    require(index_row is not None, f"packed record cache key missing from {index_path}: {cache_key}")
                    seen_cache_keys.add(cache_key)
                    key = row.get("key") or {}
                    output = row.get("output") or {}
                    chunk = row.get("chunk") or {}
                    chunk_id = str(key.get("chunk_id") or row.get("chunk_id") or "")
                    task_instance_id = str(chunk.get("task_instance_id") or row.get("task_instance_id") or "")
                    workspace_state = str(chunk.get("workspace_state") or row.get("workspace_state") or "")
                    source_sha256 = str(key.get("source_sha256") or row.get("source_sha256") or "")
                    stable_key = row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)
                    manifest_row = manifest_by_key.get(stable_key)
                    require(manifest_row is not None, f"forward record has no smoke manifest row: {stable_key}")
                    vector = np.asarray(output.get("vector"), dtype=np.float32)
                    require(vector.ndim == 1 and vector.size > 0, f"vector missing or invalid at {record_path}:{raw_line_no}")
                    require(int(index_row["vector_len"]) == int(vector.size), f"vector_len mismatch at {record_path}:{raw_line_no}")
                    vector_sha = sha256_bytes(vector.astype("<f4", copy=False).tobytes())
                    require(vector_sha == str(index_row.get("vector_sha256")), f"vector sha mismatch at {record_path}:{raw_line_no}")
                    if stable_key in records[str(embedder_id)]:
                        raise SmokeMatrixError(f"duplicate vector row for {embedder_id} {stable_key}")
                    vector_dims[str(embedder_id)].add(int(vector.size))
                    records[str(embedder_id)][stable_key] = VectorRecord(
                        row_key=stable_key,
                        embedder_id=str(embedder_id),
                        vector=vector,
                        chunk_id=chunk_id,
                        task_instance_id=task_instance_id,
                        workspace_state=workspace_state,
                        repo=str(manifest_row.get("repo") or chunk.get("repo") or "unknown"),
                        relative_path=str(manifest_row.get("relative_path") or chunk.get("relative_path") or "unknown"),
                        mutation_category=str(manifest_row.get("mutation_category") or "unknown"),
                        chunk_kind=str(manifest_row.get("chunk_kind") or "unknown"),
                        node_kind=str(manifest_row.get("node_kind") or "unknown"),
                        oracle_all_passed=str(manifest_row.get("_oracle_all_passed_str") or "unknown"),
                        oracle_resolved=str(manifest_row.get("_oracle_resolved_str") or "unknown"),
                        precision_class=str(output.get("precision_class") or "unknown"),
                        model_hash=str(key.get("model_hash") or "unknown"),
                        runtime_model_version=str(output.get("runtime_model_version") or "unknown"),
                        instrument_id=str(row.get("instrument_id") or key.get("instrument_id") or "unknown"),
                        vector_sha256=str(index_row.get("vector_sha256") or row.get("vector_sha256") or "unknown"),
                        record_sha256=str(index_row.get("record_sha256") or "unknown"),
                        cache_key_sha256=str(row.get("cache_key_sha256") or "unknown"),
                        shard_id=shard_id,
                        source_sha256=source_sha256,
                    )
                    verified_record_rows += 1
            require(seen_cache_keys == set(index_by_cache_key), f"packed index rows without record payload in {index_path}")
    load_summary = {
        "corpus_manifest_path": str(forward_root / "python_forward_cache_corpus_manifest.json"),
        "corpus_manifest_sha256": sha256_file(forward_root / "python_forward_cache_corpus_manifest.json"),
        "corpus_manifest_summary": corpus_manifest.get("summary"),
        "embedder_count": len(records),
        "index_rows_verified": index_rows_verified,
        "verified_record_rows": verified_record_rows,
        "telemetry_rows": telemetry_rows,
        "quarantine_counts": dict(sorted(quarantine_counts.items())),
        "vector_dims": {key: sorted(value) for key, value in sorted(vector_dims.items())},
        "shard_ids": shard_ids,
    }
    return records, load_summary


def centered(matrix: np.ndarray) -> np.ndarray:
    return matrix.astype(np.float64, copy=False) - matrix.mean(axis=0, keepdims=True)


def safe_float(value: float | np.floating[Any]) -> float | None:
    out = float(value)
    if math.isfinite(out):
        return out
    return None


def matrix_for_keys(rows: dict[str, VectorRecord], keys: list[str]) -> np.ndarray:
    return np.stack([rows[key].vector for key in keys], axis=0)


def linear_hsic_and_cka(x: np.ndarray, y: np.ndarray) -> tuple[float | None, float | None]:
    x_c = centered(x)
    y_c = centered(y)
    cross = x_c.T @ y_c
    hsic = float(np.sum(cross * cross) / max(1, (x.shape[0] - 1) ** 2))
    x_self = x_c.T @ x_c
    y_self = y_c.T @ y_c
    denom = math.sqrt(float(np.sum(x_self * x_self)) * float(np.sum(y_self * y_self)))
    if denom <= EPS:
        return safe_float(hsic), None
    return safe_float(hsic), safe_float(float(np.sum(cross * cross)) / denom)


def mean_same_dim_cosine(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    if x.shape[1] != y.shape[1]:
        return None, "DIMENSION_MISMATCH"
    x_norm = np.linalg.norm(x, axis=1)
    y_norm = np.linalg.norm(y, axis=1)
    valid = (x_norm > EPS) & (y_norm > EPS)
    if not bool(valid.any()):
        return None, "DEGENERATE_ZERO_NORM"
    values = np.sum(x[valid] * y[valid], axis=1) / (x_norm[valid] * y_norm[valid])
    return safe_float(float(values.mean())), None


def procrustes_similarity(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    if x.shape[1] != y.shape[1]:
        return None, "DIMENSION_MISMATCH"
    x_c = centered(x)
    y_c = centered(y)
    denom = float(np.linalg.norm(x_c, "fro") * np.linalg.norm(y_c, "fro"))
    if denom <= EPS:
        return None, "DEGENERATE_ZERO_NORM"
    singular_values = np.linalg.svd(x_c.T @ y_c, compute_uv=False)
    return safe_float(float(np.sum(singular_values)) / denom), None


def quantile_bins(values: np.ndarray, bins: int = 8) -> tuple[np.ndarray | None, str | None]:
    finite = values[np.isfinite(values)]
    if finite.size < bins * 3:
        return None, "TOO_FEW_FINITE_VALUES"
    quantiles = np.unique(np.quantile(finite, np.linspace(0.0, 1.0, bins + 1)[1:-1]))
    if quantiles.size < 2:
        return None, "DEGENERATE_QUANTILES"
    return np.digitize(values, quantiles, right=False), None


def normalized_mutual_info_from_norms(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    x_bins, x_reason = quantile_bins(np.linalg.norm(x, axis=1))
    y_bins, y_reason = quantile_bins(np.linalg.norm(y, axis=1))
    if x_bins is None or y_bins is None:
        return None, x_reason or y_reason
    joint = Counter(zip(x_bins.tolist(), y_bins.tolist()))
    x_count = Counter(x_bins.tolist())
    y_count = Counter(y_bins.tolist())
    n = float(len(x_bins))
    mi = 0.0
    for (xb, yb), count in joint.items():
        pxy = count / n
        px = x_count[xb] / n
        py = y_count[yb] / n
        mi += pxy * math.log(pxy / (px * py))
    hx = -sum((count / n) * math.log(count / n) for count in x_count.values())
    hy = -sum((count / n) * math.log(count / n) for count in y_count.values())
    denom = math.sqrt(hx * hy)
    if denom <= EPS:
        return None, "DEGENERATE_ENTROPY"
    return safe_float(mi / denom), None


def nearest_neighbors(rows: dict[str, VectorRecord], k: int) -> dict[str, set[str]]:
    keys = sorted(rows)
    if len(keys) <= 1:
        return {key: set() for key in keys}
    matrix = matrix_for_keys(rows, keys).astype(np.float64, copy=False)
    norms = np.linalg.norm(matrix, axis=1, keepdims=True)
    norms[norms <= EPS] = 1.0
    normed = matrix / norms
    sim = normed @ normed.T
    np.fill_diagonal(sim, -np.inf)
    actual_k = min(k, len(keys) - 1)
    order = np.argpartition(-sim, kth=actual_k - 1, axis=1)[:, :actual_k]
    neighbors: dict[str, set[str]] = {}
    for idx, key in enumerate(keys):
        ranked = sorted(order[idx].tolist(), key=lambda j: (-sim[idx, j], keys[j]))
        neighbors[key] = {keys[j] for j in ranked[:actual_k]}
    return neighbors


def neighbor_overlap(
    keys: list[str],
    left_neighbors: dict[str, set[str]],
    right_neighbors: dict[str, set[str]],
    k: int,
) -> tuple[float | None, str | None]:
    if len(keys) < k + 1:
        return None, "TOO_FEW_OVERLAP_ROWS"
    key_set = set(keys)
    values = []
    for key in keys:
        left = {item for item in left_neighbors.get(key, set()) if item in key_set}
        right = {item for item in right_neighbors.get(key, set()) if item in key_set}
        denom = min(k, len(left), len(right))
        if denom <= 0:
            continue
        values.append(len(left & right) / float(denom))
    if not values:
        return None, "NO_VALID_NEIGHBOR_SETS"
    return safe_float(float(np.mean(values))), None


def cca_validity(n: int, dx: int, dy: int) -> tuple[bool, str]:
    if n < DEFAULT_MIN_OVERLAP:
        return False, "TOO_FEW_OVERLAP_ROWS"
    if dx + dy >= min(n - 1, CCA_MAX_DIM_SUM):
        return False, "DIMENSION_SUM_NOT_BELOW_SAMPLE_LIMIT"
    return True, "VALID_BUT_NOT_COMPUTED_IN_SMOKE_V1"


def coverage_rows(records: dict[str, dict[str, VectorRecord]], manifest_summary: dict[str, Any]) -> list[dict[str, Any]]:
    rows = []
    manifest_rows = int(manifest_summary["rows"])
    for embedder_id, embedder_rows in sorted(records.items()):
        dims = Counter(int(row.vector.size) for row in embedder_rows.values())
        precision = Counter(row.precision_class for row in embedder_rows.values())
        models = Counter(row.model_hash for row in embedder_rows.values())
        repos = Counter(row.repo for row in embedder_rows.values())
        workspace_states = Counter(row.workspace_state for row in embedder_rows.values())
        rows.append(
            {
                "embedder_id": embedder_id,
                "row_count": len(embedder_rows),
                "coverage_ratio": round(len(embedder_rows) / manifest_rows, 6),
                "missing_or_quarantined_count": manifest_rows - len(embedder_rows),
                "dimension_counts": dict(sorted((str(k), v) for k, v in dims.items())),
                "precision_classes": dict(sorted(precision.items())),
                "model_hashes": dict(sorted(models.items())),
                "repo_counts": dict(sorted(repos.items())),
                "workspace_state_counts": dict(sorted(workspace_states.items())),
                "unique_vector_sha256_count": len({row.vector_sha256 for row in embedder_rows.values()}),
                "unique_chunk_count": len({row.chunk_id for row in embedder_rows.values()}),
            }
        )
    return rows


def stratified_coverage_rows(records: dict[str, dict[str, VectorRecord]], manifest_by_key: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    strata: list[tuple[str, str]] = []
    for row in manifest_by_key.values():
        oracle = row.get("oracle_linkage") or {}
        values = [
            ("repo", str(row.get("repo") or "unknown")),
            ("mutation_category", str(row.get("mutation_category") or "unknown")),
            ("chunk_kind", str(row.get("chunk_kind") or "unknown")),
            ("node_kind", str(row.get("node_kind") or "unknown")),
            ("workspace_state", str(row.get("workspace_state") or "unknown")),
            ("oracle_all_passed", str(oracle.get("all_passed"))),
            ("oracle_resolved", str(oracle.get("resolved"))),
        ]
        strata.extend(values)
    total_by_stratum = Counter(strata)
    observed: Counter = Counter()
    for embedder_id, embedder_rows in records.items():
        for record in embedder_rows.values():
            observed[(embedder_id, "repo", record.repo)] += 1
            observed[(embedder_id, "mutation_category", record.mutation_category)] += 1
            observed[(embedder_id, "chunk_kind", record.chunk_kind)] += 1
            observed[(embedder_id, "node_kind", record.node_kind)] += 1
            observed[(embedder_id, "workspace_state", record.workspace_state)] += 1
            observed[(embedder_id, "oracle_all_passed", record.oracle_all_passed)] += 1
            observed[(embedder_id, "oracle_resolved", record.oracle_resolved)] += 1
    rows: list[dict[str, Any]] = []
    for embedder_id in sorted(records):
        for (axis, value), total in sorted(total_by_stratum.items()):
            count = observed[(embedder_id, axis, value)]
            rows.append(
                {
                    "embedder_id": embedder_id,
                    "axis": axis,
                    "value": value,
                    "manifest_count": total,
                    "observed_count": count,
                    "coverage_ratio": round(count / total, 6) if total else None,
                }
            )
    return rows


def pairwise_rows(records: dict[str, dict[str, VectorRecord]], neighbor_k: int, min_overlap: int) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    embedder_ids = sorted(records)
    neighbors = {embedder_id: nearest_neighbors(rows, neighbor_k) for embedder_id, rows in records.items()}
    rows: list[dict[str, Any]] = []
    validity_rows: list[dict[str, Any]] = []
    for idx, left_id in enumerate(embedder_ids):
        for right_id in embedder_ids[idx + 1 :]:
            left_rows = records[left_id]
            right_rows = records[right_id]
            keys = sorted(set(left_rows) & set(right_rows))
            n = len(keys)
            row: dict[str, Any] = {
                "left_embedder_id": left_id,
                "right_embedder_id": right_id,
                "overlap_rows": n,
                "left_rows": len(left_rows),
                "right_rows": len(right_rows),
                "missing_from_left": len(set(right_rows) - set(left_rows)),
                "missing_from_right": len(set(left_rows) - set(right_rows)),
                "metric_version": "smoke_intelligence_matrices_v1",
            }
            if n < min_overlap:
                row.update(
                    {
                        "valid": False,
                        "invalid_reason": "TOO_FEW_OVERLAP_ROWS",
                        "linear_cka": None,
                        "linear_hsic": None,
                        "neighbor_overlap_at_k": None,
                        "mean_same_row_cosine": None,
                        "orthogonal_procrustes": None,
                        "norm_quantile_nmi": None,
                        "disagreement_score": None,
                        "redundancy_score": None,
                        "synergy_candidate_score": None,
                    }
                )
                rows.append(row)
                continue
            x = matrix_for_keys(left_rows, keys)
            y = matrix_for_keys(right_rows, keys)
            hsic, cka = linear_hsic_and_cka(x, y)
            cosine, cosine_reason = mean_same_dim_cosine(x, y)
            procrustes, procrustes_reason = procrustes_similarity(x, y)
            overlap, overlap_reason = neighbor_overlap(keys, neighbors[left_id], neighbors[right_id], neighbor_k)
            nmi, nmi_reason = normalized_mutual_info_from_norms(x, y)
            dx = int(x.shape[1])
            dy = int(y.shape[1])
            cca_ok, cca_reason = cca_validity(n, dx, dy)
            validity_rows.extend(
                [
                    {
                        "left_embedder_id": left_id,
                        "right_embedder_id": right_id,
                        "metric": "mean_same_row_cosine",
                        "valid": cosine_reason is None,
                        "reason": cosine_reason,
                        "n": n,
                        "left_dim": dx,
                        "right_dim": dy,
                    },
                    {
                        "left_embedder_id": left_id,
                        "right_embedder_id": right_id,
                        "metric": "orthogonal_procrustes",
                        "valid": procrustes_reason is None,
                        "reason": procrustes_reason,
                        "n": n,
                        "left_dim": dx,
                        "right_dim": dy,
                    },
                    {
                        "left_embedder_id": left_id,
                        "right_embedder_id": right_id,
                        "metric": "neighbor_overlap_at_k",
                        "valid": overlap_reason is None,
                        "reason": overlap_reason,
                        "n": n,
                        "left_dim": dx,
                        "right_dim": dy,
                    },
                    {
                        "left_embedder_id": left_id,
                        "right_embedder_id": right_id,
                        "metric": "norm_quantile_nmi",
                        "valid": nmi_reason is None,
                        "reason": nmi_reason,
                        "n": n,
                        "left_dim": dx,
                        "right_dim": dy,
                    },
                    {
                        "left_embedder_id": left_id,
                        "right_embedder_id": right_id,
                        "metric": "cca_family",
                        "valid": cca_ok,
                        "reason": cca_reason,
                        "n": n,
                        "left_dim": dx,
                        "right_dim": dy,
                    },
                ]
            )
            redundancy = cka
            disagreement = None if overlap is None else 1.0 - overlap
            synergy = None
            if cka is not None and overlap is not None:
                synergy = max(0.0, (1.0 - cka) * (1.0 - overlap))
            row.update(
                {
                    "valid": True,
                    "invalid_reason": None,
                    "left_dim": dx,
                    "right_dim": dy,
                    "linear_cka": cka,
                    "linear_hsic": hsic,
                    "neighbor_overlap_at_k": overlap,
                    "neighbor_k": neighbor_k,
                    "neighbor_overlap_validity": overlap_reason,
                    "mean_same_row_cosine": cosine,
                    "mean_same_row_cosine_validity": cosine_reason,
                    "orthogonal_procrustes": procrustes,
                    "orthogonal_procrustes_validity": procrustes_reason,
                    "norm_quantile_nmi": nmi,
                    "norm_quantile_nmi_validity": nmi_reason,
                    "cca_family_validity": cca_reason,
                    "redundancy_score": redundancy,
                    "disagreement_score": disagreement,
                    "synergy_candidate_score": synergy,
                }
            )
            rows.append(row)
    return rows, validity_rows


def top_rows(pairwise: list[dict[str, Any]], key: str, descending: bool = True, limit: int = 20) -> list[dict[str, Any]]:
    valid = [row for row in pairwise if isinstance(row.get(key), (int, float))]
    valid.sort(key=lambda row: (row[key], row["left_embedder_id"], row["right_embedder_id"]), reverse=descending)
    return valid[:limit]


def per_embedder_projection_rows(records: dict[str, dict[str, VectorRecord]]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for embedder_id, embedder_rows in sorted(records.items()):
        keys = sorted(embedder_rows)
        matrix = matrix_for_keys(embedder_rows, keys).astype(np.float64, copy=False)
        norms = np.linalg.norm(matrix, axis=1)
        rows.append(
            {
                "embedder_id": embedder_id,
                "row_count": len(keys),
                "dim": int(matrix.shape[1]),
                "norm_mean": safe_float(float(norms.mean())),
                "norm_std": safe_float(float(norms.std())),
                "norm_min": safe_float(float(norms.min())),
                "norm_max": safe_float(float(norms.max())),
                "zero_norm_count": int((norms <= EPS).sum()),
                "unique_vector_sha256_count": len({embedder_rows[key].vector_sha256 for key in keys}),
            }
        )
    return rows


def artifact_state(root: Path) -> dict[str, Any]:
    rows = []
    for child in sorted(root.rglob("*")):
        if child.is_file():
            rows.append({"path": str(child), "bytes": child.stat().st_size, "sha256": sha256_file(child)})
    return {"root": str(root), "file_count": len(rows), "byte_count": sum(row["bytes"] for row in rows), "files": rows}


def command_smoke_intelligence(args: argparse.Namespace) -> int:
    started = utc_now()
    before = {
        "output_root": tree_state(args.output_root),
        "fsv_root": tree_state(args.fsv_root),
    }
    if args.clean_output:
        if args.output_root.exists():
            shutil.rmtree(args.output_root)
        if args.fsv_root.exists():
            shutil.rmtree(args.fsv_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    manifest_by_key, manifest_summary = load_smoke_manifest(args.smoke_manifest)
    records, load_summary = load_records(args.forward_root, manifest_by_key)
    require(len(records) == args.expected_embedder_count, f"expected {args.expected_embedder_count} embedders, found {len(records)}")
    require(manifest_summary["rows"] == args.expected_chunk_count, f"expected {args.expected_chunk_count} manifest rows")

    coverage = coverage_rows(records, manifest_summary)
    stratified = stratified_coverage_rows(records, manifest_by_key)
    pairwise, validity = pairwise_rows(records, args.neighbor_k, args.min_overlap)
    projections = per_embedder_projection_rows(records)

    top_alignment = top_rows(pairwise, "linear_cka", descending=True)
    top_disagreement = top_rows(pairwise, "disagreement_score", descending=True)
    top_synergy = top_rows(pairwise, "synergy_candidate_score", descending=True)
    top_redundancy = top_rows(pairwise, "redundancy_score", descending=True)

    summary = {
        "schema_version": SCHEMA_VERSION,
        "task_id": DEFAULT_TASK_ID,
        "artifact_kind": "smoke_intelligence_matrices_summary",
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "smoke_manifest": str(args.smoke_manifest),
            "output_root": str(args.output_root),
        },
        "manifest": manifest_summary,
        "load_summary": load_summary,
        "matrix_summary": {
            "embedder_count": len(records),
            "coverage_rows": len(coverage),
            "pairwise_rows": len(pairwise),
            "pairwise_valid_rows": sum(1 for row in pairwise if row.get("valid") is True),
            "estimator_validity_rows": len(validity),
            "stratified_coverage_rows": len(stratified),
            "projection_rows": len(projections),
        },
        "metric_policy": {
            "linear_cka": "computed on centered real vectors for every pair with sufficient overlap; cross-dimensional valid",
            "linear_hsic": "unnormalized linear HSIC from centered real vectors",
            "neighbor_overlap_at_k": "top-k cosine neighbor overlap from each embedder's available row field",
            "mean_same_row_cosine": "computed only for equal-dimensional embedder pairs",
            "orthogonal_procrustes": "computed only for equal-dimensional embedder pairs",
            "norm_quantile_nmi": "quantile-binned norm mutual-information proxy with validity flags",
            "cca_family": "not emitted as a numeric score in smoke v1 unless validity preconditions are met by dimensions/sample count",
        },
        "top_alignment": top_alignment,
        "top_disagreement": top_disagreement,
        "top_synergy_candidates": top_synergy,
        "top_redundancy": top_redundancy,
    }

    write_jsonl(args.output_root / "embedder_coverage.jsonl", coverage)
    write_csv(
        args.output_root / "embedder_coverage.csv",
        coverage,
        ["embedder_id", "row_count", "coverage_ratio", "missing_or_quarantined_count", "unique_vector_sha256_count", "unique_chunk_count"],
    )
    write_jsonl(args.output_root / "pairwise_metrics.jsonl", pairwise)
    write_csv(
        args.output_root / "pairwise_metrics.csv",
        pairwise,
        [
            "left_embedder_id",
            "right_embedder_id",
            "overlap_rows",
            "left_dim",
            "right_dim",
            "linear_cka",
            "linear_hsic",
            "neighbor_overlap_at_k",
            "mean_same_row_cosine",
            "orthogonal_procrustes",
            "norm_quantile_nmi",
            "redundancy_score",
            "disagreement_score",
            "synergy_candidate_score",
            "cca_family_validity",
        ],
    )
    write_jsonl(args.output_root / "estimator_validity.jsonl", validity)
    write_jsonl(args.output_root / "stratified_coverage.jsonl", stratified)
    write_jsonl(args.output_root / "embedder_projection_stats.jsonl", projections)
    dump_json(args.output_root / "top_alignment.json", top_alignment)
    dump_json(args.output_root / "top_disagreement.json", top_disagreement)
    dump_json(args.output_root / "top_synergy_candidates.json", top_synergy)
    dump_json(args.output_root / "summary.json", summary)

    artifact_hashes = artifact_state(args.output_root)
    after = {
        "output_root": tree_state(args.output_root),
        "fsv_root": tree_state(args.fsv_root),
    }
    readback_summary = read_json(args.output_root / "summary.json")
    readback_pairwise = sum(1 for _ in iter_jsonl(args.output_root / "pairwise_metrics.jsonl"))
    readback_coverage = sum(1 for _ in iter_jsonl(args.output_root / "embedder_coverage.jsonl"))
    readback_validity = sum(1 for _ in iter_jsonl(args.output_root / "estimator_validity.jsonl"))
    stable_hashes = {
        "summary": sha256_file(args.output_root / "summary.json"),
        "pairwise_metrics": sha256_file(args.output_root / "pairwise_metrics.jsonl"),
        "embedder_coverage": sha256_file(args.output_root / "embedder_coverage.jsonl"),
        "estimator_validity": sha256_file(args.output_root / "estimator_validity.jsonl"),
        "stratified_coverage": sha256_file(args.output_root / "stratified_coverage.jsonl"),
    }
    passes = (
        readback_summary.get("task_id") == DEFAULT_TASK_ID
        and readback_summary.get("matrix_summary", {}).get("pairwise_rows") == 66
        and readback_pairwise == 66
        and readback_coverage == args.expected_embedder_count
        and readback_validity >= 66
        and all(row.get("coverage_ratio", 0.0) > 0 for row in coverage)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": DEFAULT_TASK_ID,
        "artifact_kind": "smoke_intelligence_matrices_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "smoke_manifest": str(args.smoke_manifest),
            "output_root": str(args.output_root),
            "summary": str(args.output_root / "summary.json"),
            "pairwise_metrics": str(args.output_root / "pairwise_metrics.jsonl"),
            "embedder_coverage": str(args.output_root / "embedder_coverage.jsonl"),
            "estimator_validity": str(args.output_root / "estimator_validity.jsonl"),
            "stratified_coverage": str(args.output_root / "stratified_coverage.jsonl"),
            "fsv": str(args.fsv_root / "smoke_intelligence_matrices_fsv.json"),
        },
        "before_state": before,
        "after_state": after,
        "readback": {
            "summary_task_id": readback_summary.get("task_id"),
            "summary_pairwise_rows": readback_summary.get("matrix_summary", {}).get("pairwise_rows"),
            "pairwise_rows": readback_pairwise,
            "coverage_rows": readback_coverage,
            "estimator_validity_rows": readback_validity,
            "coverage_min_ratio": min(row["coverage_ratio"] for row in coverage),
            "coverage_max_ratio": max(row["coverage_ratio"] for row in coverage),
            "top_alignment_first": top_alignment[0] if top_alignment else None,
            "top_disagreement_first": top_disagreement[0] if top_disagreement else None,
            "stable_hashes": stable_hashes,
        },
        "artifact_hashes": artifact_hashes,
        "research_basis": [
            {
                "name": "linear CKA / HSIC",
                "source": "Kornblith et al. 2019; use CKA for representation similarity and guard CCA-family dimensional validity",
            },
            {
                "name": "CCA-family validity",
                "source": "SVCCA/PWCCA literature; emit typed invalid rows instead of fake metrics when sample/dimension preconditions are not met",
            },
            {
                "name": "multi-view intelligence extraction",
                "source": "multi-view representation practice; preserve redundancy, unique/disagreement, and synergy-candidate summaries separately",
            },
        ],
    }
    dump_json(args.fsv_root / "smoke_intelligence_matrices_fsv.json", fsv)
    final_readback = read_json(args.fsv_root / "smoke_intelligence_matrices_fsv.json")
    require(final_readback.get("passes") is True, f"FSV failed; see {args.fsv_root / 'smoke_intelligence_matrices_fsv.json'}")
    print(json.dumps(final_readback, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--smoke-manifest", type=Path, default=DEFAULT_SMOKE_MANIFEST)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--expected-embedder-count", type=int, default=12)
    parser.add_argument("--min-overlap", type=int, default=DEFAULT_MIN_OVERLAP)
    parser.add_argument("--neighbor-k", type=int, default=DEFAULT_NEIGHBOR_K)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_smoke_intelligence(args)
    except SmokeMatrixError as exc:
        print(f"SMOKE_INTELLIGENCE_MATRICES_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
