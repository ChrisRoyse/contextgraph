#!/usr/bin/env python3
"""#733 incremental forward-cache v3 index generator.

This utility reads existing filesystem-backed v2 packed forward-cache records
and emits a v3 global index without recomputing vectors. The packed record
payload remains the source of identity; packed index rows are independent
readback for byte offsets and hashes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import shlex
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


TASK_ID = "TASK-PY-G-733"
SCHEMA_VERSION = 1
ARTIFACT_KIND = "forward_cache_v3_index_generation"
INDEX_ARTIFACT_KIND = "python_ast_chunk_forward_v3_index"
ACTIVE_MODELS_ARTIFACT_KIND = "forward_cache_active_models"
DEFAULT_CORPUS_PROFILE = "static_ast_python"
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/state/forward-cache-incremental-v3")
DEFAULT_FORWARD_RUNNER_COMMAND = "target/release/examples/python_forward_cache_readback"
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
MOUNTED_PATH_MARKERS = ("/mnt/c", "/mnt/d")
V3_KEY_FIELDS = (
    "corpus_profile",
    "chunk_id",
    "source_sha256",
    "source_text_sha256",
    "embedder_id",
    "model_sha256",
    "chunker_sha256",
    "preprocessing_sha256",
)
MISSING_KEY_PLAN_ARTIFACT_KIND = "forward_cache_missing_key_plan"
MISSING_KEY_ARTIFACT_KIND = "forward_cache_missing_key"
RECOMPUTE_ARTIFACT_KIND = "forward_cache_missing_key_recompute"


class IncrementalIndexError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_json(value: Any) -> str:
    return sha256_bytes(canonical_json(value).encode("utf-8"))


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return "sha256:" + digest.hexdigest()


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise IncrementalIndexError(f"{code}{suffix}")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + f".tmp-{os.getpid()}")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + f".tmp-{os.getpid()}")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row))
            handle.write("\n")
            count += 1
    tmp.replace(path)
    return count


def iter_jsonl_with_bytes(path: Path) -> Iterable[tuple[int, int, bytes, dict[str, Any]]]:
    with path.open("rb") as handle:
        while True:
            offset = handle.tell()
            line = handle.readline()
            if not line:
                break
            stripped = line.rstrip(b"\n")
            if not stripped.strip():
                continue
            yield offset, len(stripped), stripped, json.loads(stripped.decode("utf-8"))


def iter_jsonl(path: Path) -> Iterable[tuple[int, dict[str, Any]]]:
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            yield line_no, json.loads(line)


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False, "path": str(path)}
    if path.is_file():
        return {
            "exists": True,
            "kind": "file",
            "path": str(path),
            "byte_count": path.stat().st_size,
            "sha256": sha256_file(path),
        }
    file_count = 0
    total_bytes = 0
    digest = hashlib.sha256()
    for child in sorted(p for p in path.rglob("*") if p.is_file()):
        rel = child.relative_to(path).as_posix()
        digest.update(rel.encode("utf-8"))
        digest.update(b"\0")
        child_hash = sha256_file(child)
        digest.update(child_hash.encode("utf-8"))
        file_count += 1
        total_bytes += child.stat().st_size
    return {
        "exists": True,
        "kind": "directory",
        "path": str(path),
        "file_count": file_count,
        "byte_count": total_bytes,
        "tree_sha256": "sha256:" + digest.hexdigest(),
    }


def file_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False, "path": str(path)}
    require(path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_EXPECTED_FILE", path)
    stat = path.stat()
    return {
        "exists": True,
        "kind": "file",
        "path": str(path),
        "byte_count": stat.st_size,
        "mtime_ns": stat.st_mtime_ns,
        "sha256": sha256_file(path),
    }


def artifact_hashes(root: Path) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for child in sorted(p for p in root.rglob("*") if p.is_file()):
        hashes[child.relative_to(root).as_posix()] = sha256_file(child)
    return hashes


def validate_sha(value: Any, code: str, detail: Any) -> str:
    raw = str(value or "")
    require(bool(SHA256_RE.match(raw)), code, detail)
    return raw


def v3_key_from_mapping(value: dict[str, Any]) -> dict[str, str]:
    return {field: str(value.get(field) or "") for field in V3_KEY_FIELDS}


def validate_v3_key_fields(value: dict[str, Any], code: str, detail: Any) -> dict[str, str]:
    key = v3_key_from_mapping(value)
    require(all(key.values()), code, detail)
    for field in (
        "source_sha256",
        "source_text_sha256",
        "model_sha256",
        "chunker_sha256",
        "preprocessing_sha256",
    ):
        detail_map = dict(detail) if isinstance(detail, dict) else {"detail": detail}
        detail_map["field"] = field
        validate_sha(key[field], code, detail_map)
    return key


def reject_mounted_path(path: Path, code: str) -> None:
    text = path.as_posix()
    require(not any(marker in text for marker in MOUNTED_PATH_MARKERS), code, text)


def resolved(path: Path) -> Path:
    return path.expanduser().resolve(strict=False)


def path_is_under(path: Path, parent: Path) -> bool:
    try:
        resolved(path).relative_to(resolved(parent))
        return True
    except ValueError:
        return False


def paths_overlap(left: Path, right: Path) -> bool:
    left_resolved = resolved(left)
    right_resolved = resolved(right)
    return (
        left_resolved == right_resolved
        or path_is_under(left_resolved, right_resolved)
        or path_is_under(right_resolved, left_resolved)
    )


def require_no_path_overlap(output_root: Path, other: Path, code: str) -> None:
    require(
        not paths_overlap(output_root, other),
        code,
        {"output_root": str(output_root), "other": str(other)},
    )


def require_aiwonder_root(path: Path, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        path.as_posix().startswith("/zfs/archive/contextgraph/"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_NON_AIWONDER_OUTPUT_ROOT",
        path,
    )


def parse_embedder_list(raw: str | None) -> tuple[str, ...]:
    if raw is None or not raw.strip():
        return ()
    values = tuple(item.strip() for item in raw.split(",") if item.strip())
    require(len(values) == len(set(values)), "MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_EMBEDDER_FILTER", raw)
    return values


def compute_chunker_sha256(args: argparse.Namespace) -> tuple[str, dict[str, Any]]:
    if args.chunker_sha256:
        chunker_sha256 = validate_sha(
            args.chunker_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_HASH_INVALID",
            "--chunker-sha256",
        )
        return chunker_sha256, {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "forward_cache_incremental_chunker_manifest",
            "identity_source": "cli_sha256",
            "chunker_sha256": chunker_sha256,
        }
    require(
        args.chunker_manifest_path is not None,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_HASH_MISSING",
        "provide --chunker-sha256 or --chunker-manifest-path",
    )
    manifest_path = args.chunker_manifest_path
    require(manifest_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_MANIFEST_MISSING", manifest_path)
    reject_mounted_path(manifest_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CHUNKER_PATH")
    chunker_sha256 = sha256_file(manifest_path)
    return chunker_sha256, {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "forward_cache_incremental_chunker_manifest",
        "identity_source": "file_sha256",
        "source_path": str(manifest_path),
        "source_sha256": chunker_sha256,
        "chunker_sha256": chunker_sha256,
    }


def discover_shards(cache_root: Path, max_shards: int | None) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    corpus_manifest_path = cache_root / "python_forward_cache_corpus_manifest.json"
    packed_manifest_path = cache_root / "python_forward_cache_packed_manifest.json"
    source_summary: dict[str, Any]
    shards: list[dict[str, Any]]
    if corpus_manifest_path.is_file():
        manifest = read_json(corpus_manifest_path)
        require(manifest.get("passes") is True, "MEJEPA_FORWARD_CACHE_INCREMENTAL_CORPUS_MANIFEST_NOT_PASSING", corpus_manifest_path)
        raw_shards = list(manifest.get("shards") or [])
        if not raw_shards:
            shard_dirs = sorted((cache_root / "shards").glob("*/python_forward_cache_packed_manifest.json"))
            raw_shards = [{"shard_id": item.parent.name, "cache_root": str(item.parent)} for item in shard_dirs]
        shards = []
        for index, shard in enumerate(raw_shards):
            shard_id = str(shard.get("shard_id") or f"shard-{index:06d}")
            shard_root = Path(str(shard.get("cache_root") or cache_root / "shards" / shard_id))
            shards.append({"shard_id": shard_id, "cache_root": str(shard_root), "source": shard})
        source_summary = {
            "source_kind": "corpus_manifest",
            "corpus_manifest_path": str(corpus_manifest_path),
            "corpus_manifest_sha256": sha256_file(corpus_manifest_path),
            "declared_shard_count": len(raw_shards),
        }
    elif packed_manifest_path.is_file():
        shards = [{"shard_id": cache_root.name, "cache_root": str(cache_root), "source": {"cache_root": str(cache_root)}}]
        source_summary = {
            "source_kind": "single_shard_root",
            "packed_manifest_path": str(packed_manifest_path),
            "packed_manifest_sha256": sha256_file(packed_manifest_path),
            "declared_shard_count": 1,
        }
    else:
        shard_dirs = sorted((cache_root / "shards").glob("*/python_forward_cache_packed_manifest.json"))
        require(shard_dirs, "MEJEPA_FORWARD_CACHE_INCREMENTAL_NO_PACKED_SHARDS", cache_root)
        shards = [
            {"shard_id": item.parent.name, "cache_root": str(item.parent), "source": {"cache_root": str(item.parent)}}
            for item in shard_dirs
        ]
        source_summary = {
            "source_kind": "shard_directory_scan",
            "declared_shard_count": len(shard_dirs),
        }
    if max_shards is not None:
        require(max_shards > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MAX_SHARDS_INVALID", max_shards)
        shards = shards[:max_shards]
    require(shards, "MEJEPA_FORWARD_CACHE_INCREMENTAL_EMPTY_SHARD_SELECTION", cache_root)
    return shards, {**source_summary, "selected_shard_count": len(shards)}


def load_index_rows(path: Path) -> list[dict[str, Any]]:
    return [row for _line_no, row in iter_jsonl(path)]


def source_shas(record: dict[str, Any], record_path: Path, line_no: int) -> tuple[str, str, bool]:
    key = record.get("key") or {}
    source_sha256 = validate_sha(
        key.get("source_sha256"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH",
        {"record_path": str(record_path), "line_no": line_no, "field": "key.source_sha256"},
    )
    if not key.get("source_text_sha256"):
        return source_sha256, source_sha256, True
    source_text_sha256 = validate_sha(
        key.get("source_text_sha256"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH",
        {"record_path": str(record_path), "line_no": line_no, "field": "key.source_text_sha256"},
    )
    require(
        source_sha256 == source_text_sha256,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH",
        {
            "record_path": str(record_path),
            "line_no": line_no,
            "source_sha256": source_sha256,
            "source_text_sha256": source_text_sha256,
        },
    )
    return source_sha256, source_text_sha256, False


def row_to_v3_index(
    *,
    corpus_profile: str,
    shard_id: str,
    record_path: Path,
    line_no: int,
    offset: int,
    byte_len: int,
    record_bytes: bytes,
    record: dict[str, Any],
    packed_index: dict[str, Any],
    expected_embedder: str,
    chunker_sha256: str,
) -> tuple[dict[str, Any], str]:
    key = record.get("key") or {}
    output = record.get("output") or {}
    record_embedder = str(key.get("embedder_id") or record.get("embedder_id") or "")
    require(
        record_embedder == expected_embedder,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_EMBEDDER_MISMATCH",
        {"record_path": str(record_path), "line_no": line_no, "expected": expected_embedder, "actual": record_embedder},
    )
    chunk_id = str(key.get("chunk_id") or record.get("chunk_id") or "")
    require(chunk_id, "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_ID_MISSING", {"record_path": str(record_path), "line_no": line_no})
    source_sha256, source_text_sha256, legacy_source_text_sha256_derived = source_shas(record, record_path, line_no)
    model_sha256 = validate_sha(
        key.get("model_hash"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_HASH_MISSING",
        {"record_path": str(record_path), "line_no": line_no, "field": "key.model_hash"},
    )
    preprocessing_sha256 = validate_sha(
        key.get("preprocessing_hash"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_PREPROCESSING_HASH_MISSING",
        {"record_path": str(record_path), "line_no": line_no, "field": "key.preprocessing_hash"},
    )
    vector_sha256 = validate_sha(
        record.get("vector_sha256") or output.get("vector_sha256") or packed_index.get("vector_sha256"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_VECTOR_SHA_MISSING",
        {"record_path": str(record_path), "line_no": line_no},
    )
    if packed_index.get("vector_sha256"):
        require(
            str(packed_index.get("vector_sha256")) == vector_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_VECTOR_SHA_MISMATCH",
            {"record_path": str(record_path), "line_no": line_no},
        )
    record_sha256 = sha256_bytes(record_bytes)
    if packed_index.get("record_sha256"):
        require(
            str(packed_index.get("record_sha256")) == record_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECORD_SHA_MISMATCH",
            {"record_path": str(record_path), "line_no": line_no},
        )
    index_offset = int(packed_index.get("byte_offset", offset) or 0)
    index_len = int(packed_index.get("byte_len", byte_len) or 0)
    require(index_offset == offset, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_OFFSET_MISMATCH", {"record_path": str(record_path), "line_no": line_no})
    require(index_len == byte_len, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_BYTE_LEN_MISMATCH", {"record_path": str(record_path), "line_no": line_no})
    v3_key = {
        "corpus_profile": corpus_profile,
        "chunk_id": chunk_id,
        "source_sha256": source_sha256,
        "source_text_sha256": source_text_sha256,
        "embedder_id": expected_embedder,
        "model_sha256": model_sha256,
        "chunker_sha256": chunker_sha256,
        "preprocessing_sha256": preprocessing_sha256,
    }
    cache_key_sha256 = sha256_json(v3_key)
    vector_len = int(record.get("vector_len") or packed_index.get("vector_len") or 0)
    require(
        vector_len > 0,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_VECTOR_LEN_INVALID",
        {"record_path": str(record_path), "line_no": line_no, "vector_len": vector_len},
    )
    row = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": INDEX_ARTIFACT_KIND,
        "cache_key_sha256": cache_key_sha256,
        "legacy_cache_key_sha256": record.get("cache_key_sha256") or packed_index.get("cache_key_sha256"),
        "corpus_profile": corpus_profile,
        "chunk_id": chunk_id,
        "source_sha256": source_sha256,
        "source_text_sha256": source_text_sha256,
        "legacy_source_text_sha256_derived_from_source_sha256": legacy_source_text_sha256_derived,
        "embedder_id": expected_embedder,
        "model_sha256": model_sha256,
        "chunker_sha256": chunker_sha256,
        "preprocessing_sha256": preprocessing_sha256,
        "source_shard_id": shard_id,
        "record_path": str(record_path),
        "byte_offset": offset,
        "byte_len": byte_len,
        "record_sha256": record_sha256,
        "vector_sha256": vector_sha256,
        "vector_len": vector_len,
    }
    return row, model_sha256


def process_embedder_records(
    *,
    corpus_profile: str,
    shard_id: str,
    embedder_id: str,
    info: dict[str, Any],
    chunker_sha256: str,
    seen_keys: dict[str, dict[str, Any]],
    limit_records: int | None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    record_path = Path(str(info.get("record_path") or ""))
    index_path = Path(str(info.get("index_path") or ""))
    require(record_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECORD_PATH_MISSING", record_path)
    require(index_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_PATH_MISSING", index_path)
    reject_mounted_path(record_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_RECORD_PATH")
    reject_mounted_path(index_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INDEX_PATH")
    expected_rows = int(info.get("row_count") or 0)
    require(expected_rows > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_EMPTY_EMBEDDER_RECORDS", {"embedder_id": embedder_id, "record_path": str(record_path)})
    index_rows = load_index_rows(index_path)
    require(len(index_rows) == expected_rows, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROW_COUNT_MISMATCH", {"index_path": str(index_path), "manifest": expected_rows, "actual": len(index_rows)})
    rows: list[dict[str, Any]] = []
    models: set[str] = set()
    record_count = 0
    for line_index, (offset, byte_len, record_bytes, record) in enumerate(iter_jsonl_with_bytes(record_path), start=1):
        record_count += 1
        if limit_records is not None and len(rows) >= limit_records:
            continue
        require(line_index <= len(index_rows), "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROW_MISSING", {"record_path": str(record_path), "line_no": line_index})
        row, model_sha256 = row_to_v3_index(
            corpus_profile=corpus_profile,
            shard_id=shard_id,
            record_path=record_path,
            line_no=line_index,
            offset=offset,
            byte_len=byte_len,
            record_bytes=record_bytes,
            record=record,
            packed_index=index_rows[line_index - 1],
            expected_embedder=embedder_id,
            chunker_sha256=chunker_sha256,
        )
        existing = seen_keys.get(row["cache_key_sha256"])
        require(
            existing is None,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_ROW",
            {
                "cache_key_sha256": row["cache_key_sha256"],
                "first": existing,
                "second": {"record_path": str(record_path), "line_no": line_index},
            },
        )
        seen_keys[row["cache_key_sha256"]] = {"record_path": str(record_path), "line_no": line_index}
        rows.append(row)
        models.add(model_sha256)
    require(record_count == expected_rows, "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECORD_ROW_COUNT_MISMATCH", {"record_path": str(record_path), "manifest": expected_rows, "actual": record_count})
    require(rows, "MEJEPA_FORWARD_CACHE_INCREMENTAL_SELECTED_ROW_SET_EMPTY", {"embedder_id": embedder_id, "record_path": str(record_path)})
    return rows, {
        "record_path": str(record_path),
        "record_sha256": sha256_file(record_path),
        "index_path": str(index_path),
        "index_sha256": sha256_file(index_path),
        "manifest_row_count": expected_rows,
        "selected_row_count": len(rows),
        "model_sha256_values": sorted(models),
    }


def process_shard(
    *,
    shard: dict[str, Any],
    output_root: Path,
    corpus_profile: str,
    chunker_sha256: str,
    embedder_filter: tuple[str, ...],
    seen_keys: dict[str, dict[str, Any]],
    limit_records_per_embedder: int | None,
) -> tuple[dict[str, Any], dict[str, set[str]], int]:
    shard_id = str(shard["shard_id"])
    shard_root = Path(str(shard["cache_root"]))
    packed_manifest_path = shard_root / "python_forward_cache_packed_manifest.json"
    require(packed_manifest_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_PACKED_MANIFEST_MISSING", packed_manifest_path)
    reject_mounted_path(packed_manifest_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_PACKED_MANIFEST")
    packed_manifest = read_json(packed_manifest_path)
    require(packed_manifest.get("passes") is True, "MEJEPA_FORWARD_CACHE_INCREMENTAL_PACKED_MANIFEST_NOT_PASSING", packed_manifest_path)
    per_embedder = packed_manifest.get("per_embedder") or {}
    require(isinstance(per_embedder, dict) and per_embedder, "MEJEPA_FORWARD_CACHE_INCREMENTAL_PACKED_EMBEDDER_SET_EMPTY", packed_manifest_path)
    selected_embedders = embedder_filter or tuple(sorted(per_embedder))
    missing = sorted(set(selected_embedders) - set(per_embedder))
    require(not missing, "MEJEPA_FORWARD_CACHE_INCREMENTAL_PACKED_EMBEDDER_SET_MISSING", {"shard_id": shard_id, "missing": missing})
    shard_index_dir = output_root / "global_index"
    shard_index_path = shard_index_dir / f"{shard_id}.jsonl"
    shard_rows: list[dict[str, Any]] = []
    models_by_embedder: dict[str, set[str]] = defaultdict(set)
    per_embedder_summary: dict[str, Any] = {}
    for embedder_id in selected_embedders:
        rows, summary = process_embedder_records(
            corpus_profile=corpus_profile,
            shard_id=shard_id,
            embedder_id=embedder_id,
            info=per_embedder[embedder_id],
            chunker_sha256=chunker_sha256,
            seen_keys=seen_keys,
            limit_records=limit_records_per_embedder,
        )
        shard_rows.extend(rows)
        models_by_embedder[embedder_id].update(summary["model_sha256_values"])
        per_embedder_summary[embedder_id] = summary
    written_count = write_jsonl(shard_index_path, shard_rows)
    require(written_count == len(shard_rows), "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_WRITE_COUNT_MISMATCH", shard_index_path)
    return {
        "shard_id": shard_id,
        "source_shard_root": str(shard_root),
        "packed_manifest_path": str(packed_manifest_path),
        "packed_manifest_sha256": sha256_file(packed_manifest_path),
        "v3_index_path": str(shard_index_path),
        "v3_index_sha256": sha256_file(shard_index_path),
        "selected_embedder_count": len(selected_embedders),
        "selected_embedders": list(selected_embedders),
        "selected_row_count": len(shard_rows),
        "per_embedder": per_embedder_summary,
    }, models_by_embedder, len(shard_rows)


def run_pipeline(args: argparse.Namespace) -> dict[str, Any]:
    require(args.cache_root is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_ROOT_MISSING", "--cache-root")
    require(args.cache_root.exists(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_ROOT_MISSING", args.cache_root)
    require(args.max_records_per_embedder is None or args.max_records_per_embedder > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_LIMIT_INVALID", args.max_records_per_embedder)
    require_aiwonder_root(args.output_root, args.allow_non_aiwonder_roots)
    reject_mounted_path(args.cache_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CACHE_ROOT")
    reject_mounted_path(args.output_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_OUTPUT_ROOT")
    before = {"output_root": tree_state(args.output_root)}
    if args.output_root.exists():
        require(args.clean_output, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_EXISTS", args.output_root)
        shutil.rmtree(args.output_root)
    work_root = args.output_root.with_name(f"{args.output_root.name}.tmp-{os.getpid()}")
    if work_root.exists():
        shutil.rmtree(work_root)
    work_root.mkdir(parents=True)
    chunker_sha256, chunker_manifest = compute_chunker_sha256(args)
    write_json(work_root / "chunker_manifest.json", chunker_manifest)
    shards, source_summary = discover_shards(args.cache_root, args.max_shards)
    embedder_filter = parse_embedder_list(args.embedder_ids)
    seen_keys: dict[str, dict[str, Any]] = {}
    models_by_embedder: dict[str, set[str]] = defaultdict(set)
    shard_summaries: list[dict[str, Any]] = []
    total_rows = 0
    for shard in shards:
        summary, shard_models, shard_rows = process_shard(
            shard=shard,
            output_root=work_root,
            corpus_profile=args.corpus_profile,
            chunker_sha256=chunker_sha256,
            embedder_filter=embedder_filter,
            seen_keys=seen_keys,
            limit_records_per_embedder=args.max_records_per_embedder,
        )
        shard_summaries.append(summary)
        total_rows += shard_rows
        for embedder_id, model_values in shard_models.items():
            models_by_embedder[embedder_id].update(model_values)
    active_models: dict[str, str] = {}
    active_model_errors: dict[str, list[str]] = {}
    for embedder_id, model_values in sorted(models_by_embedder.items()):
        if len(model_values) != 1:
            active_model_errors[embedder_id] = sorted(model_values)
        else:
            active_models[embedder_id] = next(iter(model_values))
    require(
        not active_model_errors,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_ACTIVE_MODEL_AMBIGUOUS",
        active_model_errors,
    )
    active_models_payload = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ACTIVE_MODELS_ARTIFACT_KIND,
        "corpus_profile": args.corpus_profile,
        "chunker_sha256": chunker_sha256,
        "active_models": active_models,
        "source": "legacy_v2_packed_records",
        "created_at_utc": utc_now(),
    }
    write_json(work_root / "active_models.json", active_models_payload)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "passes": True,
        "corpus_profile": args.corpus_profile,
        "inputs": {
            "cache_root": str(args.cache_root),
            "output_root": str(args.output_root),
            "chunker_sha256": chunker_sha256,
            "chunker_manifest_path": str(args.chunker_manifest_path) if args.chunker_manifest_path else None,
            "embedder_ids": list(embedder_filter),
            "max_shards": args.max_shards,
            "max_records_per_embedder": args.max_records_per_embedder,
        },
        "source_summary": source_summary,
        "summary": {
            "selected_shard_count": len(shard_summaries),
            "selected_embedder_count": len(active_models),
            "selected_row_count": total_rows,
            "duplicate_v3_key_count": 0,
            "active_models": active_models,
        },
        "shards": shard_summaries,
        "cases": [
            {
                "case": "packed_records_payload_identity_used",
                "before": "legacy packed index rows do not carry all v3 key fields",
                "after": "v3 key built from packed record key payload",
                "pass": True,
            },
            {
                "case": "active_model_pointer_generated",
                "before": "legacy roots have no active_models.json",
                "after": active_models,
                "pass": bool(active_models),
            },
            {
                "case": "duplicate_keys_rejected",
                "before": "seen_keys starts empty",
                "after": {"unique_v3_keys": len(seen_keys), "selected_row_count": total_rows},
                "pass": len(seen_keys) == total_rows,
            },
        ],
    }
    write_json(work_root / "cache_manifest.json", manifest)
    hashes = artifact_hashes(work_root)
    write_json(work_root / "artifact_hashes.json", hashes)
    work_root.replace(args.output_root)
    readback = read_json(args.output_root / "cache_manifest.json")
    after = {"output_root": tree_state(args.output_root)}
    readback["manual_readback"] = {
        "before": before,
        "after": after,
        "cache_manifest_sha256": sha256_file(args.output_root / "cache_manifest.json"),
        "active_models_sha256": sha256_file(args.output_root / "active_models.json"),
        "artifact_hashes_sha256": sha256_file(args.output_root / "artifact_hashes.json"),
    }
    write_json(args.output_root / "cache_manifest.readback.json", readback)
    return readback


def load_active_models(active_models_path: Path) -> tuple[str, dict[str, str], dict[str, Any]]:
    require(active_models_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_ACTIVE_MODELS_MISSING", active_models_path)
    reject_mounted_path(active_models_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_ACTIVE_MODELS_PATH")
    payload = read_json(active_models_path)
    chunker_sha256 = validate_sha(
        payload.get("chunker_sha256"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_HASH_INVALID",
        {"active_models_path": str(active_models_path), "field": "chunker_sha256"},
    )
    active_models_raw = payload.get("active_models") or {}
    require(
        isinstance(active_models_raw, dict) and active_models_raw,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_ACTIVE_MODELS_EMPTY",
        active_models_path,
    )
    active_models: dict[str, str] = {}
    for embedder_id, model_sha256 in sorted(active_models_raw.items()):
        require(str(embedder_id), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_POINTER_MISSING", active_models_path)
        active_models[str(embedder_id)] = validate_sha(
            model_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_HASH_MISSING",
            {"active_models_path": str(active_models_path), "embedder_id": str(embedder_id)},
        )
    return chunker_sha256, active_models, payload


def global_index_files(index_root: Path) -> list[Path]:
    global_index_root = index_root / "global_index"
    require(global_index_root.is_dir(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_GLOBAL_INDEX_MISSING", global_index_root)
    files = sorted(global_index_root.glob("*.jsonl"))
    require(files, "MEJEPA_FORWARD_CACHE_INCREMENTAL_GLOBAL_INDEX_EMPTY", global_index_root)
    return files


def read_v3_index_rows(index_root: Path, active_chunker_sha256: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    reject_mounted_path(index_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INDEX_ROOT")
    rows: list[dict[str, Any]] = []
    files: list[dict[str, Any]] = []
    seen_keys: set[str] = set()
    for index_path in global_index_files(index_root):
        reject_mounted_path(index_path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INDEX_PATH")
        file_rows = 0
        for line_no, row in iter_jsonl(index_path):
            detail = {"index_path": str(index_path), "line_no": line_no}
            key = validate_v3_key_fields(row, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_KEY_INVALID", detail)
            require(
                key["chunker_sha256"] == active_chunker_sha256,
                "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_MISMATCH",
                {**detail, "expected": active_chunker_sha256, "actual": key["chunker_sha256"]},
            )
            cache_key_sha256 = validate_sha(
                row.get("cache_key_sha256"),
                "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_KEY_INVALID",
                detail,
            )
            recomputed_cache_key = sha256_json(key)
            require(
                cache_key_sha256 == recomputed_cache_key,
                "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_KEY_MISMATCH",
                {**detail, "expected": recomputed_cache_key, "actual": cache_key_sha256},
            )
            record_path = row.get("record_path")
            if record_path:
                reject_mounted_path(Path(str(record_path)), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_RECORD_PATH")
            require(
                cache_key_sha256 not in seen_keys,
                "MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_ROW",
                {**detail, "cache_key_sha256": cache_key_sha256},
            )
            seen_keys.add(cache_key_sha256)
            rows.append({**row, **key})
            file_rows += 1
        files.append({"path": str(index_path), "sha256": sha256_file(index_path), "row_count": file_rows})
    require(rows, "MEJEPA_FORWARD_CACHE_INCREMENTAL_GLOBAL_INDEX_EMPTY", index_root)
    return rows, files


def target_preprocessing_sha256(args: argparse.Namespace, active_target_rows: list[dict[str, Any]]) -> str:
    if args.target_preprocessing_sha256:
        return validate_sha(
            args.target_preprocessing_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_PREPROCESSING_HASH_MISSING",
            "--target-preprocessing-sha256",
        )
    values = {str(row["preprocessing_sha256"]) for row in active_target_rows}
    require(
        active_target_rows,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_TARGET_EMBEDDER_MISSING",
        args.target_embedder_id,
    )
    require(
        len(values) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_TARGET_PREPROCESSING_AMBIGUOUS",
        {"target_embedder_id": args.target_embedder_id, "preprocessing_sha256_values": sorted(values)},
    )
    return next(iter(values))


def run_missing_key_plan(args: argparse.Namespace) -> dict[str, Any]:
    require(args.index_root is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROOT_MISSING", "--index-root")
    require(args.index_root.exists(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROOT_MISSING", args.index_root)
    require(args.target_embedder_id, "MEJEPA_FORWARD_CACHE_INCREMENTAL_TARGET_EMBEDDER_MISSING", "--target-embedder-id")
    target_model_sha256 = validate_sha(
        args.target_model_sha256,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_TARGET_MODEL_HASH_INVALID",
        "--target-model-sha256",
    )
    require_aiwonder_root(args.output_root, args.allow_non_aiwonder_roots)
    reject_mounted_path(args.output_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_OUTPUT_ROOT")
    require(
        args.output_root.absolute() != args.index_root.absolute(),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_OVERLAPS_INDEX_ROOT",
        {"index_root": str(args.index_root), "output_root": str(args.output_root)},
    )
    active_models_path = args.active_models_path or args.index_root / "active_models.json"
    chunker_sha256, active_models, active_models_payload = load_active_models(active_models_path)
    before = {"output_root": tree_state(args.output_root)}
    if args.output_root.exists():
        require(args.clean_output, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_EXISTS", args.output_root)
        shutil.rmtree(args.output_root)
    work_root = args.output_root.with_name(f"{args.output_root.name}.tmp-{os.getpid()}")
    if work_root.exists():
        shutil.rmtree(work_root)
    work_root.mkdir(parents=True)
    index_rows, index_files = read_v3_index_rows(args.index_root, chunker_sha256)
    observed_embedders = sorted({str(row["embedder_id"]) for row in index_rows})
    missing_pointers = sorted(embedder_id for embedder_id in observed_embedders if embedder_id not in active_models)
    require(
        not missing_pointers,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_POINTER_MISSING",
        {"active_models_path": str(active_models_path), "missing": missing_pointers},
    )
    target_embedder_id = str(args.target_embedder_id)
    require(
        target_embedder_id in active_models,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_POINTER_MISSING",
        {"active_models_path": str(active_models_path), "target_embedder_id": target_embedder_id},
    )
    active_rows = [
        row for row in index_rows if str(row["model_sha256"]) == active_models[str(row["embedder_id"])]
    ]
    active_target_rows = [
        row for row in active_rows if str(row["embedder_id"]) == target_embedder_id
    ]
    preprocessing_sha256 = target_preprocessing_sha256(args, active_target_rows)
    chunk_identity_by_key: dict[str, dict[str, str]] = {}
    for row in active_rows:
        chunk_identity = {
            "corpus_profile": str(row["corpus_profile"]),
            "chunk_id": str(row["chunk_id"]),
            "source_sha256": str(row["source_sha256"]),
            "source_text_sha256": str(row["source_text_sha256"]),
            "chunker_sha256": str(row["chunker_sha256"]),
        }
        identity_key = sha256_json(chunk_identity)
        existing = chunk_identity_by_key.get(identity_key)
        require(
            existing is None or existing == chunk_identity,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_IDENTITY_CONFLICT",
            {"identity_key": identity_key, "first": existing, "second": chunk_identity},
        )
        chunk_identity_by_key[identity_key] = chunk_identity
    require(
        chunk_identity_by_key,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_EMPTY_SELECTED_ROW_SET",
        {"index_root": str(args.index_root), "reason": "no active chunk identities"},
    )
    existing_keys = {str(row["cache_key_sha256"]) for row in index_rows}
    missing_rows: list[dict[str, Any]] = []
    existing_target_key_count = 0
    for chunk_identity in sorted(chunk_identity_by_key.values(), key=lambda item: (item["chunk_id"], item["source_sha256"])):
        key = {
            **chunk_identity,
            "embedder_id": target_embedder_id,
            "model_sha256": target_model_sha256,
            "preprocessing_sha256": preprocessing_sha256,
        }
        cache_key_sha256 = sha256_json(key)
        if cache_key_sha256 in existing_keys:
            existing_target_key_count += 1
            continue
        missing_rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": MISSING_KEY_ARTIFACT_KIND,
                "cache_key_sha256": cache_key_sha256,
                **key,
                "reason": "target_model_row_missing",
            }
        )
    require(
        args.allow_empty_plan or missing_rows,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_EMPTY_SELECTED_ROW_SET",
        {
            "target_embedder_id": target_embedder_id,
            "target_model_sha256": target_model_sha256,
            "eligible_chunk_count": len(chunk_identity_by_key),
            "existing_target_key_count": existing_target_key_count,
        },
    )
    unchanged_embedder_selection_counts = {
        embedder_id: 0 for embedder_id in observed_embedders if embedder_id != target_embedder_id
    }
    require(
        all(str(row["embedder_id"]) == target_embedder_id for row in missing_rows),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_UNCHANGED_EMBEDDER_SELECTED",
        {"target_embedder_id": target_embedder_id},
    )
    inventory_path = work_root / "missing_key_inventory.jsonl"
    written_count = write_jsonl(inventory_path, missing_rows)
    require(
        written_count == len(missing_rows),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_WRITE_COUNT_MISMATCH",
        inventory_path,
    )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": MISSING_KEY_PLAN_ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "passes": True,
        "inputs": {
            "index_root": str(args.index_root),
            "active_models_path": str(active_models_path),
            "output_root": str(args.output_root),
            "target_embedder_id": target_embedder_id,
            "target_model_sha256": target_model_sha256,
            "target_preprocessing_sha256": preprocessing_sha256,
            "allow_empty_plan": bool(args.allow_empty_plan),
        },
        "source_summary": {
            "active_models_sha256": sha256_file(active_models_path),
            "active_models": active_models,
            "active_models_artifact_kind": active_models_payload.get("artifact_kind"),
            "chunker_sha256": chunker_sha256,
            "global_index_files": index_files,
        },
        "summary": {
            "index_row_count": len(index_rows),
            "observed_embedder_count": len(observed_embedders),
            "observed_embedders": observed_embedders,
            "eligible_chunk_count": len(chunk_identity_by_key),
            "target_embedder_id": target_embedder_id,
            "target_model_sha256": target_model_sha256,
            "target_preprocessing_sha256": preprocessing_sha256,
            "existing_target_key_count": existing_target_key_count,
            "missing_key_count": len(missing_rows),
            "unchanged_embedder_selected_count": sum(unchanged_embedder_selection_counts.values()),
            "unchanged_embedder_selection_counts": unchanged_embedder_selection_counts,
        },
        "cases": [
            {
                "case": "active_model_pointers_complete",
                "expected": "every embedder observed in the index has an active model pointer",
                "actual": {"missing_pointers": missing_pointers},
                "pass": not missing_pointers,
            },
            {
                "case": "chunker_identity_matches",
                "expected": chunker_sha256,
                "actual": sorted({row["chunker_sha256"] for row in index_rows}),
                "pass": True,
            },
            {
                "case": "non_empty_missing_key_set",
                "expected": "missing_key_count > 0 unless allow_empty_plan=true",
                "actual": {"missing_key_count": len(missing_rows), "allow_empty_plan": bool(args.allow_empty_plan)},
                "pass": bool(missing_rows) or bool(args.allow_empty_plan),
            },
            {
                "case": "unchanged_embedders_excluded",
                "expected": 0,
                "actual": sum(unchanged_embedder_selection_counts.values()),
                "pass": sum(unchanged_embedder_selection_counts.values()) == 0,
            },
        ],
    }
    write_json(work_root / "missing_key_plan_manifest.json", manifest)
    write_json(work_root / "missing_key_plan_artifact_hashes.json", artifact_hashes(work_root))
    work_root.replace(args.output_root)
    readback = read_json(args.output_root / "missing_key_plan_manifest.json")
    after = {"output_root": tree_state(args.output_root)}
    readback["manual_readback"] = {
        "before": before,
        "after": after,
        "manifest_sha256": sha256_file(args.output_root / "missing_key_plan_manifest.json"),
        "inventory_sha256": sha256_file(args.output_root / "missing_key_inventory.jsonl"),
        "artifact_hashes_sha256": sha256_file(args.output_root / "missing_key_plan_artifact_hashes.json"),
    }
    write_json(args.output_root / "missing_key_plan.readback.json", readback)
    return readback


def read_missing_inventory(path: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require(path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_MISSING", path)
    reject_mounted_path(path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INVENTORY_PATH")
    rows: list[dict[str, Any]] = []
    seen_keys: set[str] = set()
    for line_no, row in iter_jsonl(path):
        detail = {"inventory_path": str(path), "line_no": line_no}
        require(
            row.get("artifact_kind") == MISSING_KEY_ARTIFACT_KIND,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_KIND_INVALID",
            detail,
        )
        key = validate_v3_key_fields(row, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_KEY_INVALID", detail)
        cache_key_sha256 = validate_sha(
            row.get("cache_key_sha256"),
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_KEY_INVALID",
            detail,
        )
        recomputed = sha256_json(key)
        require(
            cache_key_sha256 == recomputed,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CACHE_KEY_MISMATCH",
            {**detail, "expected": recomputed, "actual": cache_key_sha256},
        )
        require(
            cache_key_sha256 not in seen_keys,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_ROW",
            {**detail, "cache_key_sha256": cache_key_sha256},
        )
        require(
            key["source_text_sha256"] == key["source_sha256"],
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_TEXT_SHA_UNSUPPORTED",
            {
                **detail,
                "source_sha256": key["source_sha256"],
                "source_text_sha256": key["source_text_sha256"],
                "reason": "current Rust forwarder records source_text_sha256 as source_sha256",
            },
        )
        seen_keys.add(cache_key_sha256)
        rows.append({**row, **key, "cache_key_sha256": cache_key_sha256})
    require(rows, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_EMPTY", path)
    embedders = sorted({str(row["embedder_id"]) for row in rows})
    model_values = sorted({str(row["model_sha256"]) for row in rows})
    chunker_values = sorted({str(row["chunker_sha256"]) for row in rows})
    preprocessing_values = sorted({str(row["preprocessing_sha256"]) for row in rows})
    corpus_profiles = sorted({str(row["corpus_profile"]) for row in rows})
    require(
        len(embedders) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_TARGET_MISMATCH",
        {"embedders": embedders},
    )
    require(
        len(model_values) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_MODEL_MISMATCH",
        {"model_sha256_values": model_values},
    )
    require(
        len(chunker_values) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_CHUNKER_MISMATCH",
        {"chunker_sha256_values": chunker_values},
    )
    require(
        len(preprocessing_values) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_PREPROCESSING_MISMATCH",
        {"preprocessing_sha256_values": preprocessing_values},
    )
    require(
        len(corpus_profiles) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_CORPUS_PROFILE_MISMATCH",
        {"corpus_profiles": corpus_profiles},
    )
    summary = {
        "inventory_path": str(path),
        "inventory_sha256": sha256_file(path),
        "row_count": len(rows),
        "target_embedder_id": embedders[0],
        "target_model_sha256": model_values[0],
        "target_chunker_sha256": chunker_values[0],
        "target_preprocessing_sha256": preprocessing_values[0],
        "corpus_profile": corpus_profiles[0],
        "unique_cache_key_count": len(seen_keys),
    }
    return rows, summary


def source_chunk_key(row: dict[str, Any]) -> tuple[str, str]:
    return str(row.get("chunk_id") or ""), str(row.get("source_sha256") or "")


def find_contiguous_chunk_window(
    chunks_jsonl: Path,
    inventory_rows: list[dict[str, Any]],
) -> dict[str, Any]:
    require(chunks_jsonl.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKS_JSONL_MISSING", chunks_jsonl)
    reject_mounted_path(chunks_jsonl, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CHUNKS_PATH")
    needed: dict[tuple[str, str], dict[str, Any]] = {}
    for row in inventory_rows:
        key = source_chunk_key(row)
        require(
            all(key),
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_IDENTITY_INVALID",
            {"row": row},
        )
        existing = needed.get(key)
        require(
            existing is None or existing["cache_key_sha256"] == row["cache_key_sha256"],
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_IDENTITY_CONFLICT",
            {"chunk_key": key},
        )
        needed[key] = row
    matches: dict[tuple[str, str], dict[str, Any]] = {}
    with chunks_jsonl.open("r", encoding="utf-8") as handle:
        for raw_index, line in enumerate(handle):
            if not line.strip():
                continue
            row = json.loads(line)
            chunk_id = str(row.get("chunk_id") or "")
            source_sha256 = str(row.get("source_sha256") or "")
            key = (chunk_id, source_sha256)
            if key not in needed:
                continue
            require(
                key not in matches,
                "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_SOURCE_DUPLICATE",
                {"chunks_jsonl": str(chunks_jsonl), "chunk_id": chunk_id, "source_sha256": source_sha256},
            )
            absolute_path = str(row.get("absolute_path") or "")
            relative_path = str(row.get("relative_path") or "")
            if absolute_path:
                reject_mounted_path(Path(absolute_path), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CHUNK_SOURCE_PATH")
            if relative_path:
                reject_mounted_path(Path(relative_path), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CHUNK_SOURCE_PATH")
            matches[key] = {
                "raw_line_index_zero_based": raw_index,
                "chunk_id": chunk_id,
                "source_sha256": source_sha256,
                "relative_path": relative_path,
                "absolute_path": absolute_path,
            }
    missing = [
        {"chunk_id": key[0], "source_sha256": key[1]}
        for key in sorted(set(needed) - set(matches))
    ]
    require(
        not missing,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_SOURCE_MISSING",
        {"chunks_jsonl": str(chunks_jsonl), "missing": missing[:10], "missing_count": len(missing)},
    )
    ordered = sorted(matches.values(), key=lambda row: int(row["raw_line_index_zero_based"]))
    line_indices = [int(row["raw_line_index_zero_based"]) for row in ordered]
    start = line_indices[0]
    expected = list(range(start, start + len(line_indices)))
    require(
        line_indices == expected,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_NON_CONTIGUOUS_CHUNK_SELECTION",
        {
            "chunks_jsonl": str(chunks_jsonl),
            "line_indices": line_indices,
            "expected_contiguous_indices": expected,
        },
    )
    return {
        "chunks_jsonl": str(chunks_jsonl),
        "chunk_offset": start,
        "max_chunks": len(line_indices),
        "selected_chunk_count": len(line_indices),
        "line_indices": line_indices,
        "selected_chunks": ordered,
    }


def unchanged_record_file_states(index_root: Path, chunker_sha256: str, target_embedder_id: str) -> dict[str, dict[str, Any]]:
    require(index_root is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROOT_MISSING", "--index-root")
    require(index_root.exists(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROOT_MISSING", index_root)
    rows, _files = read_v3_index_rows(index_root, chunker_sha256)
    paths = sorted(
        {
            str(row.get("record_path") or "").split("#", 1)[0]
            for row in rows
            if str(row.get("embedder_id")) != target_embedder_id and str(row.get("record_path") or "")
        }
    )
    require(
        paths,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_UNCHANGED_RECORD_SET_EMPTY",
        {"index_root": str(index_root), "target_embedder_id": target_embedder_id},
    )
    states: dict[str, dict[str, Any]] = {}
    for raw_path in paths:
        path = Path(raw_path)
        reject_mounted_path(path, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_RECORD_PATH")
        states[raw_path] = file_state(path)
    return states


def changed_file_states(before: dict[str, dict[str, Any]], after: dict[str, dict[str, Any]]) -> dict[str, Any]:
    changed: dict[str, Any] = {}
    for path, before_state in before.items():
        after_state = after.get(path)
        if after_state != before_state:
            changed[path] = {"before": before_state, "after": after_state}
    return changed


def runner_env_for_recompute(
    args: argparse.Namespace,
    *,
    cache_root: Path,
    readback_root: Path,
    window: dict[str, Any],
    inventory_summary: dict[str, Any],
) -> dict[str, str]:
    env = os.environ.copy()
    env.update(
        {
            "CG_MEJEPA_PY_FORWARD_CACHE_ROOT": str(cache_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_READBACK_ROOT": str(readback_root),
            "CG_MEJEPA_PY_CHUNKS_JSONL": str(args.chunks_jsonl),
            "CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET": str(window["chunk_offset"]),
            "CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS": str(window["max_chunks"]),
            "CG_MEJEPA_PY_FORWARD_MAX_SOURCE_BYTES": str(args.max_source_bytes),
            "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE": str(args.true_batch_size),
            "CG_MEJEPA_PY_FORWARD_GPU_VRAM_GUARD_BYTES": str(args.gpu_vram_guard_bytes),
            "CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT": "packed",
            "CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER": inventory_summary["target_embedder_id"],
            "CG_MEJEPA_PY_FORWARD_CORPUS_PROFILE": inventory_summary["corpus_profile"],
        }
    )
    if args.models_config:
        env["CG_MEJEPA_MODELS_CONFIG"] = str(args.models_config)
    return env


def run_forward_runner(args: argparse.Namespace, env: dict[str, str]) -> dict[str, Any]:
    command = shlex.split(args.runner_command)
    require(command, "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_COMMAND_EMPTY", args.runner_command)
    started_at = utc_now()
    completed = subprocess.run(
        command,
        text=True,
        capture_output=True,
        env=env,
        timeout=args.runner_timeout_seconds,
        check=False,
    )
    run_log = {
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "command": args.runner_command,
        "exit_code": completed.returncode,
        "stdout_tail": completed.stdout[-8000:],
        "stderr_tail": completed.stderr[-8000:],
    }
    require(
        completed.returncode == 0,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_FAILED",
        run_log,
    )
    return run_log


def validate_runner_manifest(
    runner_cache_root: Path,
    runner_readback_root: Path,
    args: argparse.Namespace,
    inventory_summary: dict[str, Any],
    window: dict[str, Any],
) -> dict[str, Any]:
    manifest_path = runner_cache_root / "python_forward_cache_manifest.json"
    readback_path = runner_readback_root / "python_forward_cache_readback.json"
    require(manifest_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_MANIFEST_MISSING", manifest_path)
    require(readback_path.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_READBACK_MISSING", readback_path)
    manifest = read_json(manifest_path)
    readback = read_json(readback_path)
    summary = manifest.get("summary") or {}
    inputs = manifest.get("inputs") or {}
    require(manifest.get("passes") is True, "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_MANIFEST_NOT_PASSING", manifest_path)
    require(readback.get("passes") is True, "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_READBACK_NOT_PASSING", readback_path)
    require(
        inputs.get("only_embedder") == inventory_summary["target_embedder_id"],
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_TARGET_MISMATCH",
        {"expected": inventory_summary["target_embedder_id"], "actual": inputs.get("only_embedder")},
    )
    require(
        int(inputs.get("chunk_offset", -1)) == int(window["chunk_offset"]),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_WINDOW_MISMATCH",
        {"expected": window["chunk_offset"], "actual": inputs.get("chunk_offset")},
    )
    require(
        int(inputs.get("max_chunks", -1)) == int(window["max_chunks"]),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_WINDOW_MISMATCH",
        {"expected": window["max_chunks"], "actual": inputs.get("max_chunks")},
    )
    require(
        str(inputs.get("chunks_jsonl") or "") == str(args.chunks_jsonl),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_CHUNKS_SOURCE_MISMATCH",
        {"expected": str(args.chunks_jsonl), "actual": inputs.get("chunks_jsonl")},
    )
    require(
        int(summary.get("active_embedder_count", -1)) == 1,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_ACTIVE_EMBEDDER_COUNT_MISMATCH",
        summary,
    )
    require(
        summary.get("active_embedder_ids") == [inventory_summary["target_embedder_id"]],
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_TARGET_MISMATCH",
        summary,
    )
    require(
        int(summary.get("cache_record_count", -1)) == int(inventory_summary["row_count"]),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_RECORD_COUNT_MISMATCH",
        {"expected": inventory_summary["row_count"], "actual": summary.get("cache_record_count")},
    )
    require(
        int(summary.get("quarantine_row_count", 0)) == 0,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_QUARANTINE_NONEMPTY",
        summary,
    )
    return {
        "manifest": manifest,
        "readback": readback,
        "manifest_sha256": sha256_file(manifest_path),
        "readback_sha256": sha256_file(readback_path),
    }


def validate_recomputed_rows(
    *,
    args: argparse.Namespace,
    runner_cache_root: Path,
    output_root: Path,
    inventory_rows: list[dict[str, Any]],
    inventory_summary: dict[str, Any],
) -> dict[str, Any]:
    recomputed_index_root = output_root / "recomputed_v3"
    recomputed_index_root.mkdir(parents=True, exist_ok=True)
    seen_keys: dict[str, dict[str, Any]] = {}
    summary, _models_by_embedder, row_count = process_shard(
        shard={"shard_id": "recomputed-target", "cache_root": str(runner_cache_root)},
        output_root=recomputed_index_root,
        corpus_profile=inventory_summary["corpus_profile"],
        chunker_sha256=inventory_summary["target_chunker_sha256"],
        embedder_filter=(inventory_summary["target_embedder_id"],),
        seen_keys=seen_keys,
        limit_records_per_embedder=None,
    )
    rows = load_index_rows(Path(summary["v3_index_path"]))
    inventory_key_set = {str(row["cache_key_sha256"]) for row in inventory_rows}
    produced_key_set = {str(row["cache_key_sha256"]) for row in rows}
    require(
        produced_key_set == inventory_key_set,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECOMPUTE_OUTPUT_KEY_MISMATCH",
        {
            "missing_from_output": sorted(inventory_key_set - produced_key_set),
            "unexpected_output_keys": sorted(produced_key_set - inventory_key_set),
        },
    )
    require(
        all(str(row["embedder_id"]) == inventory_summary["target_embedder_id"] for row in rows),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECOMPUTE_TARGET_MISMATCH",
        {"target_embedder_id": inventory_summary["target_embedder_id"]},
    )
    require(
        all(str(row["model_sha256"]) == inventory_summary["target_model_sha256"] for row in rows),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECOMPUTE_MODEL_MISMATCH",
        {"target_model_sha256": inventory_summary["target_model_sha256"]},
    )
    require(
        row_count == inventory_summary["row_count"],
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_RECOMPUTE_ROW_COUNT_MISMATCH",
        {"expected": inventory_summary["row_count"], "actual": row_count},
    )
    return {
        "v3_index_summary": summary,
        "produced_row_count": row_count,
        "produced_cache_key_count": len(produced_key_set),
        "produced_index_sha256": sha256_file(Path(summary["v3_index_path"])),
    }


def run_recompute_missing(args: argparse.Namespace) -> dict[str, Any]:
    require(args.index_root is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_INDEX_ROOT_MISSING", "--index-root")
    require(args.missing_key_inventory is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_MISSING", "--missing-key-inventory")
    require(args.chunks_jsonl is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKS_JSONL_MISSING", "--chunks-jsonl")
    require(args.output_root is not None, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_ROOT_MISSING", "--output-root")
    require(args.max_source_bytes > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MAX_SOURCE_BYTES_INVALID", args.max_source_bytes)
    require(args.true_batch_size > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_TRUE_BATCH_SIZE_INVALID", args.true_batch_size)
    require(args.runner_timeout_seconds > 0, "MEJEPA_FORWARD_CACHE_INCREMENTAL_RUNNER_TIMEOUT_INVALID", args.runner_timeout_seconds)
    require_aiwonder_root(args.output_root, args.allow_non_aiwonder_roots)
    reject_mounted_path(args.output_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_OUTPUT_ROOT")
    reject_mounted_path(args.index_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INDEX_ROOT")
    reject_mounted_path(args.chunks_jsonl, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_CHUNKS_PATH")
    reject_mounted_path(args.missing_key_inventory, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_INVENTORY_PATH")
    require_no_path_overlap(args.output_root, args.index_root, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_OVERLAPS_INDEX_ROOT")
    require_no_path_overlap(args.output_root, args.missing_key_inventory, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_OVERLAPS_INVENTORY")
    require_no_path_overlap(args.output_root, args.chunks_jsonl, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_OVERLAPS_CHUNKS")
    if args.models_config:
        reject_mounted_path(args.models_config, "MEJEPA_FORWARD_CACHE_INCREMENTAL_MOUNTED_MODELS_CONFIG")
        require(args.models_config.is_file(), "MEJEPA_FORWARD_CACHE_INCREMENTAL_MODELS_CONFIG_MISSING", args.models_config)

    before = {"output_root": tree_state(args.output_root)}
    if args.output_root.exists():
        require(args.clean_output, "MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_EXISTS", args.output_root)

    inventory_rows, inventory_summary = read_missing_inventory(args.missing_key_inventory)
    if args.target_embedder_id:
        require(
            args.target_embedder_id == inventory_summary["target_embedder_id"],
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_TARGET_MISMATCH",
            {"expected": args.target_embedder_id, "actual": inventory_summary["target_embedder_id"]},
        )
    if args.target_model_sha256:
        target_model_sha256 = validate_sha(
            args.target_model_sha256,
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_TARGET_MODEL_HASH_INVALID",
            "--target-model-sha256",
        )
        require(
            target_model_sha256 == inventory_summary["target_model_sha256"],
            "MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_MODEL_MISMATCH",
            {"expected": target_model_sha256, "actual": inventory_summary["target_model_sha256"]},
        )
    window = find_contiguous_chunk_window(args.chunks_jsonl, inventory_rows)
    unchanged_before = unchanged_record_file_states(
        args.index_root,
        inventory_summary["target_chunker_sha256"],
        inventory_summary["target_embedder_id"],
    )
    if args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=False)
    runner_cache_root = args.output_root / "runner_cache"
    runner_readback_root = args.output_root / "runner_readback"
    env = runner_env_for_recompute(
        args,
        cache_root=runner_cache_root,
        readback_root=runner_readback_root,
        window=window,
        inventory_summary=inventory_summary,
    )
    run_log = run_forward_runner(args, env)
    write_json(args.output_root / "runner_invocation.json", run_log)
    runner_readback = validate_runner_manifest(
        runner_cache_root,
        runner_readback_root,
        args,
        inventory_summary,
        window,
    )
    recomputed = validate_recomputed_rows(
        args=args,
        runner_cache_root=runner_cache_root,
        output_root=args.output_root,
        inventory_rows=inventory_rows,
        inventory_summary=inventory_summary,
    )
    unchanged_after = {
        path: file_state(Path(path))
        for path in unchanged_before
    }
    unchanged_changes = changed_file_states(unchanged_before, unchanged_after)
    require(
        not unchanged_changes,
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_UNCHANGED_RECORD_FILE_REWRITTEN",
        unchanged_changes,
    )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": RECOMPUTE_ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "passes": True,
        "inputs": {
            "index_root": str(args.index_root),
            "missing_key_inventory": str(args.missing_key_inventory),
            "chunks_jsonl": str(args.chunks_jsonl),
            "output_root": str(args.output_root),
            "runner_command": args.runner_command,
            "target_embedder_id": inventory_summary["target_embedder_id"],
            "target_model_sha256": inventory_summary["target_model_sha256"],
            "max_source_bytes": args.max_source_bytes,
            "true_batch_size": args.true_batch_size,
            "gpu_vram_guard_bytes": args.gpu_vram_guard_bytes,
            "models_config": str(args.models_config) if args.models_config else None,
        },
        "source_summary": {
            "inventory": inventory_summary,
            "chunk_window": window,
            "unchanged_record_files_before": unchanged_before,
            "runner_manifest_sha256": runner_readback["manifest_sha256"],
            "runner_readback_sha256": runner_readback["readback_sha256"],
        },
        "summary": {
            "target_embedder_id": inventory_summary["target_embedder_id"],
            "target_model_sha256": inventory_summary["target_model_sha256"],
            "missing_key_count": inventory_summary["row_count"],
            "runner_cache_record_count": runner_readback["manifest"]["summary"]["cache_record_count"],
            "recomputed_v3_row_count": recomputed["produced_row_count"],
            "unchanged_record_file_count": len(unchanged_before),
            "unchanged_record_changed_count": len(unchanged_changes),
            "runner_exit_code": run_log["exit_code"],
        },
        "runner": {
            "invocation": run_log,
            "manifest_path": str(runner_cache_root / "python_forward_cache_manifest.json"),
            "readback_path": str(runner_readback_root / "python_forward_cache_readback.json"),
        },
        "recomputed": recomputed,
        "cases": [
            {
                "case": "inventory_single_target",
                "expected": "one embedder and one target model",
                "actual": {
                    "target_embedder_id": inventory_summary["target_embedder_id"],
                    "target_model_sha256": inventory_summary["target_model_sha256"],
                },
                "pass": True,
            },
            {
                "case": "source_chunk_window_contiguous",
                "expected": list(range(window["chunk_offset"], window["chunk_offset"] + window["max_chunks"])),
                "actual": window["line_indices"],
                "pass": True,
            },
            {
                "case": "runner_target_only",
                "expected": [inventory_summary["target_embedder_id"]],
                "actual": runner_readback["manifest"]["summary"]["active_embedder_ids"],
                "pass": True,
            },
            {
                "case": "recomputed_keys_match_inventory",
                "expected": inventory_summary["row_count"],
                "actual": recomputed["produced_cache_key_count"],
                "pass": recomputed["produced_cache_key_count"] == inventory_summary["row_count"],
            },
            {
                "case": "unchanged_record_files_stable",
                "expected": 0,
                "actual": len(unchanged_changes),
                "pass": len(unchanged_changes) == 0,
            },
        ],
    }
    write_json(args.output_root / "recompute_manifest.json", manifest)
    write_json(args.output_root / "recompute_artifact_hashes.json", artifact_hashes(args.output_root))
    readback = read_json(args.output_root / "recompute_manifest.json")
    after = {"output_root": tree_state(args.output_root)}
    readback["manual_readback"] = {
        "before": before,
        "after": after,
        "manifest_sha256": sha256_file(args.output_root / "recompute_manifest.json"),
        "artifact_hashes_sha256": sha256_file(args.output_root / "recompute_artifact_hashes.json"),
        "unchanged_record_files_after": unchanged_after,
    }
    write_json(args.output_root / "recompute_manifest.readback.json", readback)
    return readback


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("index", "plan-missing", "recompute-missing"), default="index")
    parser.add_argument("--cache-root", type=Path)
    parser.add_argument("--index-root", type=Path)
    parser.add_argument("--active-models-path", type=Path)
    parser.add_argument("--missing-key-inventory", type=Path)
    parser.add_argument("--chunks-jsonl", type=Path)
    parser.add_argument("--output-root", type=Path)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--corpus-profile", default=DEFAULT_CORPUS_PROFILE)
    parser.add_argument("--chunker-sha256")
    parser.add_argument("--chunker-manifest-path", type=Path)
    parser.add_argument("--embedder-ids", help="Comma-separated embedder IDs. Defaults to all embedders in each packed manifest.")
    parser.add_argument("--max-shards", type=int)
    parser.add_argument("--max-records-per-embedder", type=int)
    parser.add_argument("--clean-output", action="store_true")
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--target-embedder-id")
    parser.add_argument("--target-model-sha256")
    parser.add_argument("--target-preprocessing-sha256")
    parser.add_argument("--allow-empty-plan", action="store_true")
    parser.add_argument("--runner-command", default=DEFAULT_FORWARD_RUNNER_COMMAND)
    parser.add_argument("--runner-timeout-seconds", type=int, default=3600)
    parser.add_argument("--max-source-bytes", type=int, default=32768)
    parser.add_argument("--true-batch-size", type=int, default=1)
    parser.add_argument("--gpu-vram-guard-bytes", type=int, default=0)
    parser.add_argument("--models-config", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    run_id = args.run_id or f"run-{utc_stamp()}"
    if args.output_root is None:
        args.output_root = args.output_parent / run_id
    try:
        if args.mode == "index":
            readback = run_pipeline(args)
            print(json.dumps(readback["summary"], indent=2, sort_keys=True))
        elif args.mode == "plan-missing":
            readback = run_missing_key_plan(args)
            print(json.dumps(readback["summary"], indent=2, sort_keys=True))
        else:
            readback = run_recompute_missing(args)
            print(json.dumps(readback["summary"], indent=2, sort_keys=True))
    except IncrementalIndexError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
