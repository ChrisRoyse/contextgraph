"""TASK-PY-G-095 real-corpus constellation smoke over aiwonder forwards.

This command reads the completed packed forward-cache corpus directly and
emits slot-preserving constellation evidence. It never concatenates embedder
vectors into one flat prediction vector; every report row keeps embedder identity
and only compares embedders through typed relationship features.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from itertools import combinations
from pathlib import Path
from typing import Any

import numpy as np

from .forward_cache_lineage import ForwardCacheLineageError, guard_packed_record_lineage
from .smoke_intelligence_matrices import (
    dump_json,
    iter_jsonl,
    read_json,
    row_key_from_fields,
    safe_float,
    sha256_bytes,
    sha256_file,
    tree_state,
    utc_now,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-095"
ARTIFACT_KIND = "python_real_corpus_constellation_smoke"
FORMULA_VERSION = "slot_preserving_constellation_smoke_v1"
DEFAULT_FORWARD_ROOT = Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z")
DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-095-real-corpus-constellation-smoke-readback")
EXPECTED_EMBEDDERS = ("e1", "e6", "e7", "e8", "e10", "e12", "e13", "e14", "e2", "e3", "e4", "e9")
EPS = 1.0e-12


class RealCorpusConstellationSmokeError(RuntimeError):
    pass


@dataclass(frozen=True)
class SampleRecord:
    row_key: str
    embedder_id: str
    vector: np.ndarray
    vector_sha256: str
    record_sha256: str
    cache_key_sha256: str
    vector_len: int
    shard_id: str
    chunk_id: str
    task_instance_id: str
    workspace_state: str
    source_sha256: str
    repo: str
    mutation_category: str
    relative_path: str
    chunk_kind: str
    node_kind: str
    instrument_id: str
    precision_class: str
    model_hash: str
    runtime_model_version: str
    raw_forward_cache_path_metadata_legacy: bool
    canonical_ast_lineage_used: bool
    scalar_features: dict[str, float]


def require(condition: bool, code: str, detail: str | None = None) -> None:
    if not condition:
        raise RealCorpusConstellationSmokeError(code if detail is None else f"{code}: {detail}")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(value: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest()


def finite_float(value: float | np.floating[Any]) -> float:
    out = float(value)
    require(math.isfinite(out), "MEJEPA_CONSTELLATION_SMOKE_NONFINITE_SCALAR")
    return out


def parse_embedder_list(raw: str) -> tuple[str, ...]:
    embedders = tuple(part.strip() for part in raw.split(",") if part.strip())
    require(bool(embedders), "MEJEPA_CONSTELLATION_SMOKE_EMPTY_EMBEDDER_SET")
    require(len(set(embedders)) == len(embedders), "MEJEPA_CONSTELLATION_SMOKE_DUPLICATE_EMBEDDER")
    return embedders


def spread_indices(total: int, selected: int) -> list[int]:
    require(total > 0, "MEJEPA_CONSTELLATION_SMOKE_EMPTY_SELECTION_DOMAIN")
    require(selected > 0, "MEJEPA_CONSTELLATION_SMOKE_INVALID_SELECTION_COUNT")
    if selected >= total:
        return list(range(total))
    if selected == 1:
        return [0]
    out = sorted({round(idx * (total - 1) / (selected - 1)) for idx in range(selected)})
    cursor = 0
    while len(out) < selected:
        if cursor not in out:
            out.append(cursor)
        cursor += 1
    return sorted(out[:selected])


def quantile_bins(values: np.ndarray, bins: int) -> tuple[np.ndarray | None, str | None]:
    finite = values[np.isfinite(values)]
    if finite.size < bins * 3:
        return None, "TOO_FEW_FINITE_VALUES"
    quantiles = np.unique(np.quantile(finite, np.linspace(0.0, 1.0, bins + 1)[1:-1]))
    if quantiles.size < 2:
        return None, "DEGENERATE_QUANTILES"
    return np.digitize(values, quantiles, right=False), None


def normalized_mutual_info(x: np.ndarray, y: np.ndarray, bins: int) -> tuple[float | None, str | None]:
    x_bins, x_reason = quantile_bins(x, bins)
    y_bins, y_reason = quantile_bins(y, bins)
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


def pearson(x: np.ndarray, y: np.ndarray) -> tuple[float | None, str | None]:
    if x.size < 3:
        return None, "TOO_FEW_FINITE_VALUES"
    x_std = float(np.std(x))
    y_std = float(np.std(y))
    if x_std <= EPS or y_std <= EPS:
        return None, "DEGENERATE_VARIANCE"
    return safe_float(float(np.corrcoef(x, y)[0, 1])), None


def vector_scalar_features(vector: np.ndarray) -> dict[str, float]:
    abs_values = np.abs(vector.astype(np.float64, copy=False))
    total_abs = float(abs_values.sum())
    entropy = 0.0
    if total_abs > EPS:
        probs = abs_values / total_abs
        entropy = float(-(probs * np.log(probs + EPS)).sum() / math.log(max(2, probs.size)))
    return {
        "norm_l2": finite_float(float(np.linalg.norm(vector))),
        "mean": finite_float(float(vector.mean())),
        "std": finite_float(float(vector.std())),
        "abs_mean": finite_float(float(abs_values.mean())),
        "zero_fraction": finite_float(float(np.mean(abs_values <= EPS))),
        "entropy_proxy": finite_float(entropy),
    }


def cell_key_for_meta(row: dict[str, Any]) -> str:
    mutation_category = str(row.get("mutation_category") or "unknown")
    return f"python:{mutation_category}"


def select_completed_shards(forward_root: Path, selected_shard_count: int) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require(forward_root.exists(), "MEJEPA_CONSTELLATION_SMOKE_FORWARD_ROOT_MISSING", str(forward_root))
    checkpoint_path = forward_root / "python_forward_cache_checkpoint.json"
    plan_path = forward_root / "python_forward_cache_shard_plan.json"
    corpus_manifest_path = forward_root / "python_forward_cache_corpus_manifest.json"
    checkpoint = read_json(checkpoint_path)
    plan = read_json(plan_path)
    corpus_manifest = read_json(corpus_manifest_path)
    require(checkpoint.get("passes") is True, "MEJEPA_CONSTELLATION_SMOKE_CHECKPOINT_NOT_PASSING")
    require(corpus_manifest.get("passes") is True, "MEJEPA_CONSTELLATION_SMOKE_CORPUS_MANIFEST_NOT_PASSING")
    require(plan.get("artifact_kind") == "python_forward_shard_scheduler_plan", "MEJEPA_CONSTELLATION_SMOKE_BAD_SHARD_PLAN")
    completed = int((checkpoint.get("progress") or {}).get("completed_shards") or 0)
    total = int((checkpoint.get("progress") or {}).get("total_shards") or completed)
    require(completed == total, "MEJEPA_CONSTELLATION_SMOKE_INCOMPLETE_CORPUS", f"completed={completed} total={total}")
    plan_shards = list(plan.get("shards") or [])
    require(len(plan_shards) >= completed, "MEJEPA_CONSTELLATION_SMOKE_PLAN_SHORTER_THAN_CHECKPOINT")
    shard_indices = spread_indices(completed, selected_shard_count)
    selected = [plan_shards[idx] for idx in shard_indices]
    for entry in selected:
        shard_id = str(entry.get("shard_id") or "")
        require(bool(shard_id), "MEJEPA_CONSTELLATION_SMOKE_SHARD_ID_MISSING")
        require(
            (forward_root / "shards" / shard_id / "python_forward_cache_packed_manifest.json").exists(),
            "MEJEPA_CONSTELLATION_SMOKE_PACKED_MANIFEST_MISSING",
            shard_id,
        )
    return selected, {
        "forward_root": str(forward_root),
        "checkpoint_path": str(checkpoint_path),
        "checkpoint_sha256": sha256_file(checkpoint_path),
        "shard_plan_path": str(plan_path),
        "shard_plan_sha256": sha256_file(plan_path),
        "corpus_manifest_path": str(corpus_manifest_path),
        "corpus_manifest_sha256": sha256_file(corpus_manifest_path),
        "completed_shards": completed,
        "total_shards": total,
        "selected_shard_count": len(selected),
        "selected_shard_indices": shard_indices,
        "selected_shard_ids": [str(entry.get("shard_id")) for entry in selected],
        "checkpoint_summary": checkpoint.get("summary") or {},
        "corpus_manifest_summary": corpus_manifest.get("summary") or {},
    }


def load_chunk_manifest_window(path: Path, shards: list[dict[str, Any]]) -> tuple[dict[str, dict[str, Any]], dict[str, Any]]:
    require(path.exists(), "MEJEPA_CONSTELLATION_SMOKE_CHUNKS_JSONL_MISSING", str(path))
    windows: list[tuple[int, int]] = []
    for shard in shards:
        start = int(shard.get("chunk_offset") or 0)
        size = int(shard.get("max_chunks") or 0)
        require(size > 0, "MEJEPA_CONSTELLATION_SMOKE_INVALID_SHARD_WINDOW", str(shard.get("shard_id")))
        windows.append((start, start + size))
    max_needed = max(end for _, end in windows)
    by_key: dict[str, dict[str, Any]] = {}
    counts = {
        "rows": 0,
        "repos": Counter(),
        "mutation_categories": Counter(),
        "chunk_kinds": Counter(),
        "workspace_states": Counter(),
    }
    for idx, (_, row) in enumerate(iter_jsonl(path)):
        if idx >= max_needed:
            break
        if not any(start <= idx < end for start, end in windows):
            continue
        task_instance_id = str(row.get("task_instance_id") or "")
        workspace_state = str(row.get("workspace_state") or "")
        chunk_id = str(row.get("chunk_id") or "")
        source_sha256 = str(row.get("source_sha256") or "")
        require(
            task_instance_id and workspace_state and chunk_id and source_sha256,
            "MEJEPA_CONSTELLATION_SMOKE_CHUNK_STABLE_KEY_MISSING",
            f"{path}:{idx + 1}",
        )
        key = row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)
        require(key not in by_key, "MEJEPA_CONSTELLATION_SMOKE_DUPLICATE_CHUNK_KEY", key)
        by_key[key] = row
        counts["rows"] += 1
        counts["repos"][str(row.get("repo") or "unknown")] += 1
        counts["mutation_categories"][str(row.get("mutation_category") or "unknown")] += 1
        counts["chunk_kinds"][str(row.get("chunk_kind") or "unknown")] += 1
        counts["workspace_states"][str(row.get("workspace_state") or "unknown")] += 1
    planned_window_rows = sum(end - start for start, end in windows)
    require(counts["rows"] > 0, "MEJEPA_CONSTELLATION_SMOKE_CHUNK_WINDOW_EMPTY", str(path))
    return by_key, {
        "path": str(path),
        "sha256": sha256_file(path),
        "selected_window_rows": counts["rows"],
        "planned_window_rows": planned_window_rows,
        "partial_final_window_observed": counts["rows"] < planned_window_rows,
        "repos": dict(sorted(counts["repos"].items())),
        "mutation_categories": dict(sorted(counts["mutation_categories"].items())),
        "chunk_kinds": dict(sorted(counts["chunk_kinds"].items())),
        "workspace_states": dict(sorted(counts["workspace_states"].items())),
    }


def read_selected_jsonl(path: Path, wanted_indices: list[int]) -> tuple[dict[int, dict[str, Any]], int]:
    require(path.exists(), "MEJEPA_CONSTELLATION_SMOKE_JSONL_MISSING", str(path))
    wanted = set(wanted_indices)
    selected: dict[int, dict[str, Any]] = {}
    line_count = 0
    with path.open("rb") as fh:
        for idx, raw in enumerate(fh):
            if not raw.strip():
                continue
            line_count = idx + 1
            if idx in wanted:
                try:
                    row = json.loads(raw)
                except json.JSONDecodeError as exc:
                    raise RealCorpusConstellationSmokeError(
                        f"MEJEPA_CONSTELLATION_SMOKE_BAD_JSONL: {path}:{idx + 1}: {exc}"
                    ) from exc
                row["_packed_line_sha256"] = "sha256:" + hashlib.sha256(raw.rstrip(b"\n")).hexdigest()
                selected[idx] = row
    missing = sorted(wanted - set(selected))
    require(not missing, "MEJEPA_CONSTELLATION_SMOKE_SELECTED_ROWS_MISSING", f"{path} missing={missing[:10]}")
    return selected, line_count


def build_sample_record(
    shard_id: str,
    embedder_id: str,
    record_row: dict[str, Any],
    index_row: dict[str, Any],
    manifest_by_key: dict[str, dict[str, Any]],
    record_path: Path,
    line_index: int,
) -> SampleRecord:
    cache_key = str(record_row.get("cache_key_sha256") or "")
    require(cache_key == str(index_row.get("cache_key_sha256") or ""), "MEJEPA_CONSTELLATION_SMOKE_CACHE_KEY_MISMATCH", f"{record_path}:{line_index + 1}")
    require(
        str(record_row.get("_packed_line_sha256") or "") == str(index_row.get("record_sha256") or ""),
        "MEJEPA_CONSTELLATION_SMOKE_RECORD_SHA_MISMATCH",
        f"{record_path}:{line_index + 1}",
    )
    key = record_row.get("key") or {}
    chunk = record_row.get("chunk") or {}
    output = record_row.get("output") or {}
    require(str(key.get("embedder_id") or embedder_id) == embedder_id, "MEJEPA_CONSTELLATION_SMOKE_EMBEDDER_ID_MISMATCH", f"{record_path}:{line_index + 1}")
    vector = np.asarray(output.get("vector"), dtype="<f4")
    require(vector.ndim == 1 and vector.size > 0, "MEJEPA_CONSTELLATION_SMOKE_VECTOR_MISSING", f"{record_path}:{line_index + 1}")
    require(int(index_row.get("vector_len") or 0) == int(vector.size), "MEJEPA_CONSTELLATION_SMOKE_VECTOR_LEN_MISMATCH", f"{record_path}:{line_index + 1}")
    computed_vector_sha = sha256_bytes(vector.astype("<f4", copy=False).tobytes())
    require(computed_vector_sha == str(index_row.get("vector_sha256") or ""), "MEJEPA_CONSTELLATION_SMOKE_VECTOR_SHA_MISMATCH", f"{record_path}:{line_index + 1}")
    require(
        str(output.get("vector_sha256") or computed_vector_sha) == computed_vector_sha,
        "MEJEPA_CONSTELLATION_SMOKE_OUTPUT_VECTOR_SHA_MISMATCH",
        f"{record_path}:{line_index + 1}",
    )
    chunk_id = str(key.get("chunk_id") or "")
    task_instance_id = str(chunk.get("task_instance_id") or "")
    workspace_state = str(chunk.get("workspace_state") or "")
    source_sha256 = str(key.get("source_sha256") or "")
    row_key = row_key_from_fields(task_instance_id, workspace_state, chunk_id, source_sha256)
    meta = manifest_by_key.get(row_key)
    require(meta is not None, "MEJEPA_CONSTELLATION_SMOKE_RECORD_WITHOUT_CHUNK_META", row_key)
    assert meta is not None
    lineage = guard_packed_record_lineage(
        record_row,
        meta,
        context=f"{record_path}:{line_index + 1}",
    )
    return SampleRecord(
        row_key=row_key,
        embedder_id=embedder_id,
        vector=vector,
        vector_sha256=computed_vector_sha,
        record_sha256=str(index_row.get("record_sha256") or ""),
        cache_key_sha256=cache_key,
        vector_len=int(vector.size),
        shard_id=shard_id,
        chunk_id=chunk_id,
        task_instance_id=task_instance_id,
        workspace_state=workspace_state,
        source_sha256=source_sha256,
        repo=str(meta.get("repo") or chunk.get("repo") or "unknown"),
        mutation_category=str(meta.get("mutation_category") or "unknown"),
        relative_path=str(meta.get("relative_path") or chunk.get("relative_path") or "unknown"),
        chunk_kind=str(meta.get("chunk_kind") or "unknown"),
        node_kind=str(meta.get("node_kind") or "unknown"),
        instrument_id=str(record_row.get("instrument_id") or "unknown"),
        precision_class=str(output.get("precision_class") or "unknown"),
        model_hash=str(key.get("model_hash") or "unknown"),
        runtime_model_version=str(output.get("runtime_model_version") or "unknown"),
        raw_forward_cache_path_metadata_legacy=bool(lineage["raw_packed_path_metadata_legacy"]),
        canonical_ast_lineage_used=bool(lineage["canonical_ast_lineage_used"]),
        scalar_features=vector_scalar_features(vector),
    )


def load_sample_records(
    forward_root: Path,
    shard_entries: list[dict[str, Any]],
    manifest_by_key: dict[str, dict[str, Any]],
    required_embedders: tuple[str, ...],
    sample_rows_per_embedder_per_shard: int,
) -> tuple[dict[str, dict[str, SampleRecord]], dict[str, Any]]:
    require(sample_rows_per_embedder_per_shard > 0, "MEJEPA_CONSTELLATION_SMOKE_INVALID_SAMPLE_LIMIT")
    by_embedder: dict[str, dict[str, SampleRecord]] = defaultdict(dict)
    vector_dims: dict[str, set[int]] = defaultdict(set)
    row_count_readback: dict[str, dict[str, int]] = {}
    manifest_row_counts: dict[str, dict[str, int]] = {}
    index_hash_mismatches = 0
    sampled_record_count = 0
    source_readback_unverified = 0
    raw_forward_cache_legacy_path_metadata = 0
    per_embedder_model_hashes: dict[str, set[str]] = defaultdict(set)
    per_embedder_instruments: dict[str, set[str]] = defaultdict(set)

    for shard in shard_entries:
        shard_id = str(shard.get("shard_id") or "")
        shard_root = forward_root / "shards" / shard_id
        packed_manifest = read_json(shard_root / "python_forward_cache_packed_manifest.json")
        require(packed_manifest.get("passes") is True, "MEJEPA_CONSTELLATION_SMOKE_PACKED_MANIFEST_NOT_PASSING", shard_id)
        per_embedder = packed_manifest.get("per_embedder") or {}
        missing = [embedder_id for embedder_id in required_embedders if embedder_id not in per_embedder]
        require(not missing, "MEJEPA_CONSTELLATION_SMOKE_MISSING_EMBEDDER", f"{shard_id}: {','.join(missing)}")
        row_count_readback[shard_id] = {}
        manifest_row_counts[shard_id] = {}
        for embedder_id in required_embedders:
            entry = per_embedder[embedder_id]
            row_count = int(entry.get("row_count") or 0)
            require(row_count > 0, "MEJEPA_CONSTELLATION_SMOKE_EMPTY_EMBEDDER_RECORDS", f"{shard_id}:{embedder_id}")
            manifest_row_counts[shard_id][embedder_id] = row_count
            wanted = spread_indices(row_count, min(sample_rows_per_embedder_per_shard, row_count))
            record_path = Path(entry.get("record_path") or "")
            index_path = Path(entry.get("index_path") or "")
            record_rows, record_line_count = read_selected_jsonl(record_path, wanted)
            index_rows, index_line_count = read_selected_jsonl(index_path, wanted)
            require(record_line_count == row_count, "MEJEPA_CONSTELLATION_SMOKE_RECORD_ROW_COUNT_MISMATCH", f"{record_path}: manifest={row_count} read={record_line_count}")
            require(index_line_count == row_count, "MEJEPA_CONSTELLATION_SMOKE_INDEX_ROW_COUNT_MISMATCH", f"{index_path}: manifest={row_count} read={index_line_count}")
            row_count_readback[shard_id][embedder_id] = record_line_count
            for line_index in wanted:
                record = build_sample_record(
                    shard_id,
                    embedder_id,
                    record_rows[line_index],
                    index_rows[line_index],
                    manifest_by_key,
                    record_path,
                    line_index,
                )
                if record.row_key in by_embedder[embedder_id]:
                    continue
                by_embedder[embedder_id][record.row_key] = record
                vector_dims[embedder_id].add(record.vector_len)
                per_embedder_model_hashes[embedder_id].add(record.model_hash)
                per_embedder_instruments[embedder_id].add(record.instrument_id)
                source_readback_unverified += 0 if bool(record_rows[line_index].get("source_readback_verified")) else 1
                raw_forward_cache_legacy_path_metadata += 1 if record.raw_forward_cache_path_metadata_legacy else 0
                index_hash_mismatches += 0 if record.vector_sha256 == str(index_rows[line_index].get("vector_sha256") or "") else 1
                sampled_record_count += 1

    require(set(by_embedder) == set(required_embedders), "MEJEPA_CONSTELLATION_SMOKE_EMBEDDER_SET_READBACK_MISMATCH")
    return by_embedder, {
        "sampled_record_count": sampled_record_count,
        "source_readback_unverified": source_readback_unverified,
        "raw_forward_cache_legacy_path_metadata": raw_forward_cache_legacy_path_metadata,
        "forward_cache_lineage_policy": "packed record chunk.absolute_path/dataschema is legacy provenance only; active lineage comes from canonical AST rows",
        "index_hash_mismatches": index_hash_mismatches,
        "row_count_readback": row_count_readback,
        "manifest_row_counts": manifest_row_counts,
        "vector_dims": {key: sorted(value) for key, value in sorted(vector_dims.items())},
        "per_embedder_model_hashes": {key: sorted(value) for key, value in sorted(per_embedder_model_hashes.items())},
        "per_embedder_instruments": {key: sorted(value) for key, value in sorted(per_embedder_instruments.items())},
    }


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            fh.write("\n")
    tmp.replace(path)


def cell_centroid_rows(records: dict[str, dict[str, SampleRecord]]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, str], list[SampleRecord]] = defaultdict(list)
    for embedder_id, rows in records.items():
        for record in rows.values():
            grouped[(cell_key_for_meta({"mutation_category": record.mutation_category}), embedder_id)].append(record)
    out: list[dict[str, Any]] = []
    for (cell_key, embedder_id), items in sorted(grouped.items()):
        matrix = np.stack([item.vector for item in items], axis=0).astype(np.float64, copy=False)
        mean = matrix.mean(axis=0)
        norm = float(np.linalg.norm(mean))
        require(norm > EPS, "MEJEPA_CONSTELLATION_SMOKE_ZERO_CENTROID", f"{cell_key}:{embedder_id}")
        centroid = (mean / norm).astype("<f4")
        diffs = matrix - centroid.astype(np.float64, copy=False)
        per_row_distance = np.linalg.norm(diffs, axis=1)
        variance = finite_float(float(np.mean(per_row_distance * per_row_distance)))
        out.append(
            {
                "artifact_kind": "python_real_corpus_cell_centroid",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "cell_key": cell_key,
                "language": "python",
                "mutation_category": cell_key.split(":", 1)[1],
                "embedder_id": embedder_id,
                "row_count": len(items),
                "dim": int(centroid.size),
                "centroid_vector": [finite_float(v) for v in centroid.astype(float).tolist()],
                "centroid_vector_sha256": sha256_bytes(centroid.tobytes()),
                "centroid_preview": [finite_float(v) for v in centroid[:8].astype(float).tolist()],
                "variance_mean_squared_distance": variance,
                "distance_mean": finite_float(float(per_row_distance.mean())),
                "distance_max": finite_float(float(per_row_distance.max())),
                "source_row_key_sample": [item.row_key for item in items[:5]],
            }
        )
    return out


def scalar_array(rows: dict[str, SampleRecord], keys: list[str], feature: str) -> np.ndarray:
    return np.asarray([rows[key].scalar_features[feature] for key in keys], dtype=np.float64)


def centroid_cosine_if_equal_dim(left: list[SampleRecord], right: list[SampleRecord]) -> tuple[float | None, str | None]:
    left_matrix = np.stack([item.vector for item in left], axis=0).astype(np.float64, copy=False)
    right_matrix = np.stack([item.vector for item in right], axis=0).astype(np.float64, copy=False)
    if left_matrix.shape[1] != right_matrix.shape[1]:
        return None, "DIMENSION_MISMATCH"
    left_centroid = left_matrix.mean(axis=0)
    right_centroid = right_matrix.mean(axis=0)
    denom = float(np.linalg.norm(left_centroid) * np.linalg.norm(right_centroid))
    if denom <= EPS:
        return None, "DEGENERATE_ZERO_NORM"
    return safe_float(float(np.dot(left_centroid, right_centroid) / denom)), None


def pairwise_relationship_rows(
    records: dict[str, dict[str, SampleRecord]],
    min_overlap: int,
    mi_bins: int,
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for left_id, right_id in combinations(sorted(records), 2):
        left_rows = records[left_id]
        right_rows = records[right_id]
        keys = sorted(set(left_rows) & set(right_rows))
        row: dict[str, Any] = {
            "artifact_kind": "python_real_corpus_pairwise_embedder_relationship",
            "schema_version": SCHEMA_VERSION,
            "formula_version": FORMULA_VERSION,
            "left_embedder_id": left_id,
            "right_embedder_id": right_id,
            "overlap_rows": len(keys),
            "left_rows": len(left_rows),
            "right_rows": len(right_rows),
            "missing_from_left": len(set(right_rows) - set(left_rows)),
            "missing_from_right": len(set(left_rows) - set(right_rows)),
            "valid": len(keys) >= min_overlap,
            "invalid_reason": None if len(keys) >= min_overlap else "TOO_FEW_OVERLAP_ROWS",
            "metric_policy": "slot-preserving scalar/correlation relationships; raw vectors are never concatenated",
        }
        if len(keys) < min_overlap:
            rows.append(row)
            continue
        feature_metrics: dict[str, dict[str, Any]] = {}
        redundancy_inputs: list[float] = []
        disagreement_inputs: list[float] = []
        for feature in ("norm_l2", "std", "abs_mean", "entropy_proxy", "zero_fraction"):
            x = scalar_array(left_rows, keys, feature)
            y = scalar_array(right_rows, keys, feature)
            corr, corr_reason = pearson(x, y)
            nmi, nmi_reason = normalized_mutual_info(x, y, mi_bins)
            delta = finite_float(float(np.mean(np.abs(x - y))))
            feature_metrics[feature] = {
                "pearson": corr,
                "pearson_validity": corr_reason,
                "abs_pearson": safe_float(abs(corr)) if corr is not None else None,
                "quantile_nmi": nmi,
                "quantile_nmi_validity": nmi_reason,
                "mean_abs_delta": delta,
            }
            if corr is not None:
                redundancy_inputs.append(abs(corr))
            if nmi is not None:
                redundancy_inputs.append(nmi)
            disagreement_inputs.append(delta)
        centroid_cosine, centroid_reason = centroid_cosine_if_equal_dim(
            [left_rows[key] for key in keys],
            [right_rows[key] for key in keys],
        )
        if centroid_cosine is not None:
            redundancy_inputs.append(max(0.0, centroid_cosine))
        redundancy_score = float(np.mean(redundancy_inputs)) if redundancy_inputs else None
        disagreement_score = float(np.mean(disagreement_inputs)) if disagreement_inputs else None
        row.update(
            {
                "left_dim": next(iter(left_rows.values())).vector_len if left_rows else None,
                "right_dim": next(iter(right_rows.values())).vector_len if right_rows else None,
                "feature_metrics": feature_metrics,
                "same_dim_centroid_cosine": centroid_cosine,
                "same_dim_centroid_cosine_validity": centroid_reason,
                "redundancy_score": safe_float(redundancy_score) if redundancy_score is not None else None,
                "disagreement_score": safe_float(disagreement_score) if disagreement_score is not None else None,
                "relationship_hash": sha256_text(
                    {
                        "left": left_id,
                        "right": right_id,
                        "overlap": len(keys),
                        "features": feature_metrics,
                    }
                ),
            }
        )
        rows.append(row)
    return rows


def top_rows(rows: list[dict[str, Any]], key: str, limit: int, descending: bool) -> list[dict[str, Any]]:
    valid = [row for row in rows if isinstance(row.get(key), (int, float))]
    valid.sort(key=lambda row: (row[key], row.get("left_embedder_id", ""), row.get("right_embedder_id", "")), reverse=descending)
    return valid[:limit]


def highest_variance_slots(centroids: list[dict[str, Any]], limit: int) -> list[dict[str, Any]]:
    rows = sorted(
        centroids,
        key=lambda row: (
            row.get("variance_mean_squared_distance") or 0.0,
            row.get("cell_key") or "",
            row.get("embedder_id") or "",
        ),
        reverse=True,
    )
    return [
        {
            "cell_key": row["cell_key"],
            "embedder_id": row["embedder_id"],
            "row_count": row["row_count"],
            "dim": row["dim"],
            "variance_mean_squared_distance": row["variance_mean_squared_distance"],
            "centroid_vector_sha256": row["centroid_vector_sha256"],
            "centroid_preview": row["centroid_preview"],
        }
        for row in rows[:limit]
    ]


def candidate_failure_fingerprints(records: dict[str, dict[str, SampleRecord]], limit: int) -> list[dict[str, Any]]:
    distribution: dict[tuple[str, str], dict[str, tuple[float, float]]] = {}
    for embedder_id, rows in records.items():
        for feature in ("std", "abs_mean", "entropy_proxy"):
            values = np.asarray([row.scalar_features[feature] for row in rows.values()], dtype=np.float64)
            distribution[(embedder_id, feature)] = (float(values.mean()), max(EPS, float(values.std())))

    row_keys = sorted(set().union(*(set(rows) for rows in records.values())))
    candidates: list[dict[str, Any]] = []
    for row_key in row_keys:
        present = [(embedder_id, rows[row_key]) for embedder_id, rows in sorted(records.items()) if row_key in rows]
        if len(present) < 2:
            continue
        z_values: list[float] = []
        embedder_scores: list[dict[str, Any]] = []
        for embedder_id, record in present:
            feature_z = {}
            for feature in ("std", "abs_mean", "entropy_proxy"):
                mean, std = distribution[(embedder_id, feature)]
                z = (record.scalar_features[feature] - mean) / std
                feature_z[feature] = finite_float(z)
                z_values.append(abs(float(z)))
            embedder_scores.append(
                {
                    "embedder_id": embedder_id,
                    "vector_len": record.vector_len,
                    "feature_z": feature_z,
                    "vector_sha256": record.vector_sha256,
                }
            )
        if not z_values:
            continue
        first = present[0][1]
        score = finite_float(float(np.mean(z_values) + np.std(z_values)))
        candidates.append(
            {
                "artifact_kind": "python_real_corpus_candidate_failure_shape_fingerprint",
                "schema_version": SCHEMA_VERSION,
                "formula_version": FORMULA_VERSION,
                "candidate_id": sha256_text({"row_key": row_key, "score": round(score, 8)}),
                "row_key": row_key,
                "score": score,
                "present_embedder_count": len(present),
                "chunk_id": first.chunk_id,
                "task_instance_id": first.task_instance_id,
                "repo": first.repo,
                "mutation_category": first.mutation_category,
                "workspace_state": first.workspace_state,
                "relative_path": first.relative_path,
                "chunk_kind": first.chunk_kind,
                "node_kind": first.node_kind,
                "embedder_scores": sorted(embedder_scores, key=lambda row: row["embedder_id"]),
            }
        )
    candidates.sort(key=lambda row: (row["score"], row["row_key"]), reverse=True)
    return candidates[:limit]


def row_constellation_features(
    row_key: str,
    records: dict[str, dict[str, SampleRecord]],
    distributions: dict[tuple[str, str], tuple[float, float]],
) -> dict[str, Any] | None:
    present = [(embedder_id, rows[row_key]) for embedder_id, rows in sorted(records.items()) if row_key in rows]
    if len(present) < 2:
        return None
    z_abs_values: list[float] = []
    std_values: list[float] = []
    abs_mean_values: list[float] = []
    entropy_values: list[float] = []
    for embedder_id, record in present:
        std_values.append(record.scalar_features["std"])
        abs_mean_values.append(record.scalar_features["abs_mean"])
        entropy_values.append(record.scalar_features["entropy_proxy"])
        for feature in ("std", "abs_mean", "entropy_proxy"):
            mean, std = distributions[(embedder_id, feature)]
            z_abs_values.append(abs(float((record.scalar_features[feature] - mean) / std)))
    first = present[0][1]
    return {
        "row_key": row_key,
        "label_known_bad": 0 if first.mutation_category == "known_good" else 1,
        "mutation_category": first.mutation_category,
        "slot_coverage_count": len(present),
        "mean_slot_abs_z": finite_float(float(np.mean(z_abs_values))),
        "max_slot_abs_z": finite_float(float(np.max(z_abs_values))),
        "pairwise_std_dispersion": finite_float(float(np.std(std_values))),
        "pairwise_abs_mean_dispersion": finite_float(float(np.std(abs_mean_values))),
        "pairwise_entropy_dispersion": finite_float(float(np.std(entropy_values))),
    }


def split_train_test(row: dict[str, Any]) -> str:
    digest = hashlib.sha256(str(row["row_key"]).encode("utf-8")).hexdigest()
    return "test" if int(digest[:8], 16) % 5 == 0 else "train"


def evaluate_threshold(rows: list[dict[str, Any]], feature: str, threshold: float, direction: str) -> dict[str, Any]:
    tp = tn = fp = fn = 0
    for row in rows:
        value = float(row[feature])
        pred = 1 if (value >= threshold if direction == ">=" else value <= threshold) else 0
        label = int(row["label_known_bad"])
        if pred == 1 and label == 1:
            tp += 1
        elif pred == 0 and label == 0:
            tn += 1
        elif pred == 1 and label == 0:
            fp += 1
        else:
            fn += 1
    total = max(1, tp + tn + fp + fn)
    return {
        "tp": tp,
        "tn": tn,
        "fp": fp,
        "fn": fn,
        "accuracy": finite_float((tp + tn) / total),
    }


def tiny_training_inference_smoke(records: dict[str, dict[str, SampleRecord]]) -> dict[str, Any]:
    distributions: dict[tuple[str, str], tuple[float, float]] = {}
    for embedder_id, rows in records.items():
        for feature in ("std", "abs_mean", "entropy_proxy"):
            values = np.asarray([row.scalar_features[feature] for row in rows.values()], dtype=np.float64)
            distributions[(embedder_id, feature)] = (float(values.mean()), max(EPS, float(values.std())))
    row_keys = sorted(set().union(*(set(rows) for rows in records.values())))
    features = [row_constellation_features(row_key, records, distributions) for row_key in row_keys]
    features = [row for row in features if row is not None]
    train = [row for row in features if split_train_test(row) == "train"]
    test = [row for row in features if split_train_test(row) == "test"]
    label_counts = {
        "train": dict(sorted(Counter(row["label_known_bad"] for row in train).items())),
        "test": dict(sorted(Counter(row["label_known_bad"] for row in test).items())),
    }
    feature_names = [
        "slot_coverage_count",
        "mean_slot_abs_z",
        "max_slot_abs_z",
        "pairwise_std_dispersion",
        "pairwise_abs_mean_dispersion",
        "pairwise_entropy_dispersion",
    ]
    require(len(train) >= 12, "MEJEPA_CONSTELLATION_SMOKE_TRAIN_ROWS_TOO_LOW", str(len(train)))
    require(len(test) >= 3, "MEJEPA_CONSTELLATION_SMOKE_TEST_ROWS_TOO_LOW", str(len(test)))
    require(set(row["label_known_bad"] for row in train) == {0, 1}, "MEJEPA_CONSTELLATION_SMOKE_TRAIN_LABELS_DEGENERATE", str(label_counts["train"]))
    require(set(row["label_known_bad"] for row in test) == {0, 1}, "MEJEPA_CONSTELLATION_SMOKE_TEST_LABELS_DEGENERATE", str(label_counts["test"]))
    best: dict[str, Any] | None = None
    for feature in feature_names[1:]:
        values = sorted({float(row[feature]) for row in train})
        if len(values) == 1:
            thresholds = values
        else:
            thresholds = [(left + right) / 2.0 for left, right in zip(values, values[1:])]
        for threshold in thresholds:
            for direction in (">=", "<="):
                train_eval = evaluate_threshold(train, feature, threshold, direction)
                candidate = {
                    "feature": feature,
                    "threshold": finite_float(threshold),
                    "direction": direction,
                    "train": train_eval,
                }
                if best is None or train_eval["accuracy"] > best["train"]["accuracy"]:
                    best = candidate
    require(best is not None, "MEJEPA_CONSTELLATION_SMOKE_NO_TINY_MODEL")
    assert best is not None
    test_eval = evaluate_threshold(test, str(best["feature"]), float(best["threshold"]), str(best["direction"]))
    return {
        "artifact_kind": "python_real_corpus_tiny_training_inference_smoke",
        "schema_version": SCHEMA_VERSION,
        "formula_version": FORMULA_VERSION,
        "passes": True,
        "feature_policy": "trained on slot-level z-scores and pairwise scalar dispersion; no flat concatenated vector feature is present",
        "feature_names": feature_names,
        "row_count": len(features),
        "train_rows": len(train),
        "test_rows": len(test),
        "label_counts": label_counts,
        "best_threshold_model": best,
        "test": test_eval,
        "feature_row_hash": sha256_text([{name: row[name] for name in feature_names} | {"label": row["label_known_bad"]} for row in features[:50]]),
    }


def built_in_edge_cases() -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for name, action in (
        ("empty_embedder_set_fails_closed", lambda: parse_embedder_list("")),
        ("zero_selection_count_fails_closed", lambda: spread_indices(10, 0)),
        ("empty_selection_domain_fails_closed", lambda: spread_indices(0, 1)),
    ):
        try:
            action()
            cases.append({"name": name, "passes": False, "error": None})
        except RealCorpusConstellationSmokeError as exc:
            cases.append({"name": name, "passes": True, "error": str(exc)})
    corr, reason = pearson(np.asarray([1.0, 1.0, 1.0]), np.asarray([2.0, 2.0, 2.0]))
    cases.append(
        {
            "name": "degenerate_correlation_emits_reason",
            "passes": corr is None and reason == "DEGENERATE_VARIANCE",
            "result": {"pearson": corr, "reason": reason},
        }
    )
    legacy_record = {
        "key": {"chunk_id": "chunk-1", "source_sha256": "sha256:abc"},
        "chunk": {
            "task_instance_id": "task-1",
            "workspace_state": "after",
            "absolute_path": "/mnt/d/contextgraph/corpus/old.py",
            "dataschema": "/mnt/d/contextgraph/corpus/old.schema.json",
        },
    }
    canonical_chunk = {
        "task_instance_id": "task-1",
        "workspace_state": "after",
        "chunk_id": "chunk-1",
        "source_sha256": "sha256:abc",
        "absolute_path": "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/task-1/after/old.py",
        "dataschema": "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.schema.json",
    }
    lineage = guard_packed_record_lineage(legacy_record, canonical_chunk, context="synthetic_legacy_payload")
    cases.append(
        {
            "name": "legacy_packed_path_allowed_only_with_canonical_ast_lineage",
            "passes": lineage["raw_packed_path_metadata_legacy"] is True and lineage["canonical_ast_lineage_used"] is True,
            "result": lineage,
        }
    )
    stale_canonical = dict(canonical_chunk)
    stale_canonical["absolute_path"] = "/mnt/d/contextgraph/corpus/old.py"
    try:
        guard_packed_record_lineage(legacy_record, stale_canonical, context="synthetic_stale_canonical")
        cases.append({"name": "legacy_canonical_ast_path_fails_closed", "passes": False, "error": None})
    except ForwardCacheLineageError as exc:
        cases.append(
            {
                "name": "legacy_canonical_ast_path_fails_closed",
                "passes": exc.code == "MEJEPA_FORWARD_CACHE_CANONICAL_LINEAGE_LEGACY_MOUNTED_PATH",
                "error": str(exc),
            }
        )
    return cases


def command_python_real_corpus_constellation_smoke(args: argparse.Namespace) -> int:
    started = utc_now()
    required_embedders = parse_embedder_list(args.expected_embedders)
    require(args.expected_embedder_count == len(required_embedders), "MEJEPA_CONSTELLATION_SMOKE_EXPECTED_COUNT_MISMATCH")
    require(args.sample_shards > 0, "MEJEPA_CONSTELLATION_SMOKE_INVALID_SAMPLE_SHARDS")
    require(args.sample_rows_per_embedder_per_shard > 0, "MEJEPA_CONSTELLATION_SMOKE_INVALID_SAMPLE_LIMIT")
    output_root = args.output_root or (args.readback_root / "artifacts")
    before = {
        "output_root": tree_state(output_root),
        "readback_root": tree_state(args.readback_root),
    }
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    selected_shards, source_summary = select_completed_shards(args.forward_root, args.sample_shards)
    manifest_by_key, chunk_summary = load_chunk_manifest_window(args.chunks_jsonl, selected_shards)
    records, load_summary = load_sample_records(
        args.forward_root,
        selected_shards,
        manifest_by_key,
        required_embedders,
        args.sample_rows_per_embedder_per_shard,
    )
    require(len(records) == args.expected_embedder_count, "MEJEPA_CONSTELLATION_SMOKE_ACTIVE_EMBEDDER_COUNT_MISMATCH")
    centroids = cell_centroid_rows(records)
    pairwise = pairwise_relationship_rows(records, args.min_overlap, args.mi_bins)
    candidates = candidate_failure_fingerprints(records, args.top_k)
    training_smoke = tiny_training_inference_smoke(records)
    edge_cases = built_in_edge_cases()

    top_redundant = top_rows(pairwise, "redundancy_score", args.top_k, descending=True)
    top_disagreement = top_rows(pairwise, "disagreement_score", args.top_k, descending=True)
    top_variance = highest_variance_slots(centroids, args.top_k)
    expected_pairwise = len(required_embedders) * (len(required_embedders) - 1) // 2

    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "output_root": str(output_root),
        },
        "source_summary": source_summary,
        "chunk_summary": chunk_summary,
        "load_summary": load_summary,
        "constellation_policy": {
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "relationship_features": [
                "per-embedder per-cell centroids",
                "per-embedder per-cell variance",
                "pairwise scalar pearson",
                "pairwise scalar quantile_nmi",
                "same-dimension centroid cosine only when dimensions match",
                "candidate disagreement fingerprints from slot z-scores",
            ],
        },
        "summary": {
            "embedder_count": len(records),
            "expected_embedder_count": args.expected_embedder_count,
            "sampled_record_count": load_summary["sampled_record_count"],
            "cell_centroid_rows": len(centroids),
            "pairwise_rows": len(pairwise),
            "pairwise_valid_rows": sum(1 for row in pairwise if row.get("valid") is True),
            "candidate_failure_shape_fingerprints": len(candidates),
            "tiny_training_rows": training_smoke["train_rows"],
            "tiny_test_rows": training_smoke["test_rows"],
        },
        "top_redundant_pairs": top_redundant,
        "top_disagreement_pairs": top_disagreement,
        "highest_variance_slots_by_cell": top_variance,
        "candidate_failure_shape_fingerprints": candidates,
        "tiny_training_inference_smoke": training_smoke,
        "edge_cases": edge_cases,
    }

    write_jsonl(output_root / "cell_centroids.jsonl", centroids)
    write_jsonl(output_root / "pairwise_relationships.jsonl", pairwise)
    write_jsonl(output_root / "candidate_failure_shape_fingerprints.jsonl", candidates)
    dump_json(output_root / "tiny_training_inference_smoke.json", training_smoke)
    dump_json(output_root / "constellation_preview_report.json", report)

    readback_report = read_json(output_root / "constellation_preview_report.json")
    readback_centroids = sum(1 for _ in iter_jsonl(output_root / "cell_centroids.jsonl"))
    readback_pairwise = sum(1 for _ in iter_jsonl(output_root / "pairwise_relationships.jsonl"))
    readback_candidates = sum(1 for _ in iter_jsonl(output_root / "candidate_failure_shape_fingerprints.jsonl"))
    stable_hashes = {
        "constellation_preview_report": sha256_file(output_root / "constellation_preview_report.json"),
        "cell_centroids": sha256_file(output_root / "cell_centroids.jsonl"),
        "pairwise_relationships": sha256_file(output_root / "pairwise_relationships.jsonl"),
        "candidate_failure_shape_fingerprints": sha256_file(output_root / "candidate_failure_shape_fingerprints.jsonl"),
        "tiny_training_inference_smoke": sha256_file(output_root / "tiny_training_inference_smoke.json"),
    }
    passes = (
        readback_report.get("summary", {}).get("embedder_count") == args.expected_embedder_count
        and readback_centroids == len(centroids)
        and readback_pairwise == expected_pairwise
        and readback_candidates == len(candidates)
        and readback_candidates > 0
        and bool(top_redundant)
        and bool(top_disagreement)
        and bool(top_variance)
        and training_smoke.get("passes") is True
        and all(case.get("passes") is True for case in edge_cases)
        and load_summary["index_hash_mismatches"] == 0
        and load_summary["source_readback_unverified"] == 0
    )
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "output_root": str(output_root),
            "report": str(output_root / "constellation_preview_report.json"),
            "cell_centroids": str(output_root / "cell_centroids.jsonl"),
            "pairwise_relationships": str(output_root / "pairwise_relationships.jsonl"),
            "candidate_failure_shape_fingerprints": str(output_root / "candidate_failure_shape_fingerprints.jsonl"),
            "tiny_training_inference_smoke": str(output_root / "tiny_training_inference_smoke.json"),
            "readback": str(args.readback_root / f"{ARTIFACT_KIND}_readback.json"),
        },
        "before_state": before,
        "after_state": {
            "output_root": tree_state(output_root),
            "readback_root": tree_state(args.readback_root),
        },
        "readback": {
            "report_task_id": readback_report.get("task_id"),
            "report_embedder_count": readback_report.get("summary", {}).get("embedder_count"),
            "centroid_rows": readback_centroids,
            "pairwise_rows": readback_pairwise,
            "candidate_rows": readback_candidates,
            "expected_pairwise_rows": expected_pairwise,
            "top_redundant_first": top_redundant[0] if top_redundant else None,
            "top_disagreement_first": top_disagreement[0] if top_disagreement else None,
            "highest_variance_first": top_variance[0] if top_variance else None,
            "stable_hashes": stable_hashes,
        },
        "source_summary": source_summary,
        "chunk_summary": chunk_summary,
        "load_summary": load_summary,
        "edge_cases": edge_cases,
    }
    readback_path = args.readback_root / f"{ARTIFACT_KIND}_readback.json"
    dump_json(readback_path, readback)
    final_readback = read_json(readback_path)
    require(final_readback.get("passes") is True, f"MEJEPA_CONSTELLATION_SMOKE_READBACK_FAILED: {readback_path}")
    print(json.dumps(final_readback, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--output-root", type=Path, default=None)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--expected-embedder-count", type=int, default=len(EXPECTED_EMBEDDERS))
    parser.add_argument("--sample-shards", type=int, default=7)
    parser.add_argument("--sample-rows-per-embedder-per-shard", type=int, default=32)
    parser.add_argument("--min-overlap", type=int, default=24)
    parser.add_argument("--mi-bins", type=int, default=8)
    parser.add_argument("--top-k", type=int, default=20)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_real_corpus_constellation_smoke(args)
    except RealCorpusConstellationSmokeError as exc:
        print(f"PYTHON_REAL_CORPUS_CONSTELLATION_SMOKE_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
