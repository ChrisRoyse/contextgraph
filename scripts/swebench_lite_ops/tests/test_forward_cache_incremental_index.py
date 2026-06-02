from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import pytest

from ..python_forward_cache_incremental_index import IncrementalIndexError, run_pipeline


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_json(value: object) -> str:
    return sha256_bytes(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8"))


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            handle.write("\n")


def fixture_sha(label: str) -> str:
    return sha256_bytes(label.encode("utf-8"))


def base_record(embedder_id: str, chunk_id: str, *, model_hash: str | None = None, source_text_sha256: str | None = None) -> dict:
    source_sha256 = fixture_sha(f"source:{chunk_id}")
    return {
        "schema_version": 1,
        "cache_key_sha256": fixture_sha(f"legacy:{embedder_id}:{chunk_id}"),
        "key": {
            "schema_version": 1,
            "artifact_kind": "python_ast_chunk_forward",
            "chunk_id": chunk_id,
            "source_sha256": source_sha256,
            "source_text_sha256": source_text_sha256 or source_sha256,
            "embedder_id": embedder_id,
            "model_hash": model_hash if model_hash is not None else fixture_sha(f"model:{embedder_id}"),
            "preprocessing_hash": fixture_sha(f"preprocessing:{embedder_id}"),
            "input_sha256": fixture_sha(f"input:{embedder_id}:{chunk_id}"),
        },
        "chunk": {
            "task_id": "task-py-g-733",
            "task_instance_id": f"fixture-{chunk_id}",
            "repo": "fixture/repo",
            "workspace_state": "mutated",
            "mutation_category": "subtle_flip",
            "candidate_row_id": f"row-{chunk_id}",
            "relative_path": "pkg/example.py",
            "chunk_line_number": 1,
        },
        "output": {"vector": [1.0, 0.0, 0.5], "precision_class": "algorithmic_real_forward"},
        "vector_len": 3,
        "vector_sha256": fixture_sha(f"vector:{embedder_id}:{chunk_id}"),
    }


def write_forward_root(root: Path, rows_by_embedder: dict[str, list[dict]]) -> Path:
    shard_root = root / "shards" / "shard-000000-offset-000000000"
    packed_root = shard_root / "packed_records"
    per_embedder = {}
    for embedder_id, rows in rows_by_embedder.items():
        record_path = packed_root / f"{embedder_id}.jsonl"
        index_path = packed_root / f"{embedder_id}.index.jsonl"
        write_jsonl(record_path, rows)
        index_rows = []
        offset = 0
        for row in rows:
            payload = json.dumps(row, sort_keys=True, separators=(",", ":")).encode("utf-8")
            index_rows.append(
                {
                    "schema_version": 1,
                    "cache_key_sha256": row["cache_key_sha256"],
                    "embedder_id": embedder_id,
                    "packed_record_path": str(record_path),
                    "byte_offset": offset,
                    "byte_len": len(payload),
                    "record_sha256": sha256_bytes(payload),
                    "vector_len": row["vector_len"],
                    "vector_sha256": row["vector_sha256"],
                }
            )
            offset += len(payload) + 1
        write_jsonl(index_path, index_rows)
        per_embedder[embedder_id] = {
            "record_path": str(record_path),
            "record_sha256": sha256_bytes(record_path.read_bytes()),
            "index_path": str(index_path),
            "index_sha256": sha256_bytes(index_path.read_bytes()),
            "row_count": len(rows),
        }
    write_json(
        shard_root / "python_forward_cache_packed_manifest.json",
        {
            "schema_version": 1,
            "artifact_kind": "python_forward_cache_packed_manifest",
            "layout": "packed-jsonl",
            "row_count": sum(len(rows) for rows in rows_by_embedder.values()),
            "per_embedder": per_embedder,
            "passes": True,
        },
    )
    write_json(
        root / "python_forward_cache_corpus_manifest.json",
        {
            "schema_version": 1,
            "artifact_kind": "python_forward_cache_corpus_manifest",
            "passes": True,
            "shards": [{"shard_id": "shard-000000-offset-000000000", "cache_root": str(shard_root)}],
        },
    )
    return root


def args_for(tmp_path: Path, root: Path) -> argparse.Namespace:
    return argparse.Namespace(
        cache_root=root,
        output_root=tmp_path / "out",
        output_parent=tmp_path,
        run_id="run-test",
        corpus_profile="static_ast_python",
        chunker_sha256=fixture_sha("chunker"),
        chunker_manifest_path=None,
        embedder_ids=None,
        max_shards=None,
        max_records_per_embedder=None,
        clean_output=False,
        allow_non_aiwonder_roots=True,
    )


def test_v3_index_generator_writes_active_models_and_global_index(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a"), base_record("e1", "chunk-b")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    readback = run_pipeline(args_for(tmp_path, forward_root))
    active_models = json.loads((tmp_path / "out" / "active_models.json").read_text(encoding="utf-8"))
    index_rows = [
        json.loads(line)
        for line in (tmp_path / "out" / "global_index" / "shard-000000-offset-000000000.jsonl").read_text(encoding="utf-8").splitlines()
    ]

    assert readback["passes"] is True
    assert readback["summary"]["selected_row_count"] == 3
    assert active_models["active_models"] == {"e1": fixture_sha("model:e1"), "e6": fixture_sha("model:e6")}
    assert {row["artifact_kind"] for row in index_rows} == {"python_ast_chunk_forward_v3_index"}
    assert {row["embedder_id"] for row in index_rows} == {"e1", "e6"}
    assert all(row["cache_key_sha256"] == sha256_json({key: row[key] for key in (
        "corpus_profile",
        "chunk_id",
        "source_sha256",
        "source_text_sha256",
        "embedder_id",
        "model_sha256",
        "chunker_sha256",
        "preprocessing_sha256",
    )}) for row in index_rows)
    assert all(case["pass"] is True for case in readback["cases"])


def test_v3_index_generator_rejects_duplicate_key(tmp_path: Path):
    duplicate = base_record("e1", "chunk-a")
    forward_root = write_forward_root(tmp_path / "forward", {"e1": [duplicate, dict(duplicate)]})

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_DUPLICATE_ROW"):
        run_pipeline(args_for(tmp_path, forward_root))


def test_v3_index_generator_rejects_missing_model_hash(tmp_path: Path):
    record = base_record("e1", "chunk-a", model_hash="")
    del record["key"]["model_hash"]
    forward_root = write_forward_root(tmp_path / "forward", {"e1": [record]})

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_HASH_MISSING"):
        run_pipeline(args_for(tmp_path, forward_root))


def test_v3_index_generator_rejects_source_sha_mismatch(tmp_path: Path):
    record = base_record("e1", "chunk-a", source_text_sha256=fixture_sha("different-source"))
    forward_root = write_forward_root(tmp_path / "forward", {"e1": [record]})

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_SOURCE_SHA_MISMATCH"):
        run_pipeline(args_for(tmp_path, forward_root))
