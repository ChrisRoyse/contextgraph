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
SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
MOUNTED_PATH_MARKERS = ("/mnt/c", "/mnt/d")


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


def artifact_hashes(root: Path) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for child in sorted(p for p in root.rglob("*") if p.is_file()):
        hashes[child.relative_to(root).as_posix()] = sha256_file(child)
    return hashes


def validate_sha(value: Any, code: str, detail: Any) -> str:
    raw = str(value or "")
    require(bool(SHA256_RE.match(raw)), code, detail)
    return raw


def reject_mounted_path(path: Path, code: str) -> None:
    text = path.as_posix()
    require(not any(marker in text for marker in MOUNTED_PATH_MARKERS), code, text)


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


def source_shas(record: dict[str, Any], record_path: Path, line_no: int) -> tuple[str, str]:
    key = record.get("key") or {}
    source_sha256 = validate_sha(
        key.get("source_sha256"),
        "MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH",
        {"record_path": str(record_path), "line_no": line_no, "field": "key.source_sha256"},
    )
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
    return source_sha256, source_text_sha256


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
    source_sha256, source_text_sha256 = source_shas(record, record_path, line_no)
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


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache-root", type=Path, required=True)
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
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    run_id = args.run_id or f"run-{utc_stamp()}"
    if args.output_root is None:
        args.output_root = args.output_parent / run_id
    try:
        readback = run_pipeline(args)
    except IncrementalIndexError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    print(json.dumps(readback["summary"], indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
