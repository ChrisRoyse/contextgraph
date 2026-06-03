from __future__ import annotations

import argparse
import hashlib
import json
import stat
import textwrap
from pathlib import Path

import pytest

from ..python_forward_cache_incremental_index import (
    IncrementalIndexError,
    run_missing_key_plan,
    run_pipeline,
    run_promote_active_model,
    run_recompute_missing,
)


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


def plan_args_for(
    tmp_path: Path,
    index_root: Path,
    *,
    target_embedder_id: str = "e1",
    target_model_sha256: str | None = None,
    target_preprocessing_sha256: str | None = None,
    active_models_path: Path | None = None,
) -> argparse.Namespace:
    return argparse.Namespace(
        index_root=index_root,
        active_models_path=active_models_path,
        output_root=tmp_path / "plan",
        output_parent=tmp_path,
        run_id="run-plan",
        clean_output=False,
        allow_non_aiwonder_roots=True,
        target_embedder_id=target_embedder_id,
        target_model_sha256=target_model_sha256 or fixture_sha(f"model:{target_embedder_id}:next"),
        target_preprocessing_sha256=target_preprocessing_sha256,
        allow_empty_plan=False,
    )


def recompute_args_for(
    tmp_path: Path,
    index_root: Path,
    inventory_path: Path,
    chunks_jsonl: Path,
    runner: Path,
    *,
    output_root: Path | None = None,
    target_embedder_id: str | None = "e1",
    target_model_sha256: str | None = None,
) -> argparse.Namespace:
    return argparse.Namespace(
        index_root=index_root,
        missing_key_inventory=inventory_path,
        chunks_jsonl=chunks_jsonl,
        output_root=output_root or tmp_path / "recompute",
        clean_output=False,
        allow_non_aiwonder_roots=True,
        target_embedder_id=target_embedder_id,
        target_model_sha256=target_model_sha256,
        runner_command=str(runner),
        runner_timeout_seconds=30,
        max_source_bytes=32768,
        true_batch_size=1,
        gpu_vram_guard_bytes=0,
        models_config=None,
    )


def promote_args_for(
    tmp_path: Path,
    index_root: Path,
    recomputed_index_root: Path,
    *,
    output_root: Path | None = None,
    target_embedder_id: str = "e1",
    target_model_sha256: str | None = None,
) -> argparse.Namespace:
    return argparse.Namespace(
        index_root=index_root,
        recomputed_index_root=recomputed_index_root,
        active_models_path=None,
        output_root=output_root or tmp_path / "promotion",
        output_parent=tmp_path,
        run_id="run-promotion",
        clean_output=False,
        allow_non_aiwonder_roots=True,
        target_embedder_id=target_embedder_id,
        target_model_sha256=target_model_sha256 or fixture_sha(f"model:{target_embedder_id}:next"),
    )


def write_chunks_jsonl(path: Path, chunk_ids: list[str]) -> None:
    rows = []
    for chunk_id in chunk_ids:
        source_sha256 = fixture_sha(f"source:{chunk_id}")
        rows.append(
            {
                "chunk_id": chunk_id,
                "source_text": f"def {chunk_id.replace('-', '_')}():\n    return {len(chunk_id)}\n",
                "source_sha256": source_sha256,
                "file_sha256": fixture_sha(f"file:{chunk_id}"),
                "task_id": "task-py-g-733",
                "task_instance_id": f"fixture-{chunk_id}",
                "repo": "fixture/repo",
                "workspace_state": "mutated",
                "absolute_path": f"/zfs/archive/contextgraph/source-files/{fixture_sha(f'candidate:{chunk_id}')[7:]}/pkg/example.py",
                "relative_path": "pkg/example.py",
                "mutation_category": "subtle_flip",
            }
        )
    write_jsonl(path, rows)


def write_fake_recompute_runner(path: Path) -> None:
    path.write_text(
        textwrap.dedent(
            r'''
            #!/usr/bin/env python3
            import hashlib
            import json
            import os
            from pathlib import Path

            def sha256_bytes(payload):
                return "sha256:" + hashlib.sha256(payload).hexdigest()

            def sha256_json(value):
                return sha256_bytes(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8"))

            def write_json(path, value):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")

            def write_jsonl(path, rows):
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open("w", encoding="utf-8") as handle:
                    for row in rows:
                        handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
                        handle.write("\n")

            cache_root = Path(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_ROOT"])
            readback_root = Path(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_READBACK_ROOT"])
            chunks_jsonl = Path(os.environ["CG_MEJEPA_PY_CHUNKS_JSONL"])
            offset = int(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET"])
            max_chunks = int(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS"])
            embedder = os.environ["CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER"]
            model_sha = os.environ["FAKE_FORWARD_MODEL_SHA256"]
            preprocessing_sha = os.environ["FAKE_FORWARD_PREPROCESSING_SHA256"]
            mutate_path = os.environ.get("FAKE_MUTATE_UNCHANGED_RECORD")
            if mutate_path:
                Path(mutate_path).write_text(Path(mutate_path).read_text(encoding="utf-8") + "\n", encoding="utf-8")

            all_rows = [json.loads(line) for line in chunks_jsonl.read_text(encoding="utf-8").splitlines() if line.strip()]
            selected = all_rows[offset:offset + max_chunks]
            packed_root = cache_root / "packed_records"
            record_path = packed_root / f"{embedder}.jsonl"
            index_path = packed_root / f"{embedder}.index.jsonl"
            records = []
            index_rows = []
            offset_bytes = 0
            for idx, chunk in enumerate(selected):
                key = {
                    "schema_version": 1,
                    "artifact_kind": "python_ast_chunk_forward",
                    "chunk_id": chunk["chunk_id"],
                    "source_sha256": chunk["source_sha256"],
                    "source_text_sha256": chunk["source_sha256"],
                    "embedder_id": embedder,
                    "model_hash": model_sha,
                    "preprocessing_hash": preprocessing_sha,
                    "input_sha256": chunk["source_sha256"],
                }
                vector = [float(idx), 1.0]
                record = {
                    "schema_version": 1,
                    "cache_key_sha256": sha256_json(key),
                    "key": key,
                    "chunk": {
                        "task_id": chunk["task_id"],
                        "task_instance_id": chunk["task_instance_id"],
                        "repo": chunk["repo"],
                        "workspace_state": chunk["workspace_state"],
                        "mutation_category": chunk.get("mutation_category"),
                        "candidate_row_id": f"row-{chunk['chunk_id']}",
                        "relative_path": chunk["relative_path"],
                        "chunk_line_number": offset + idx + 1,
                    },
                    "output": {"vector": vector, "precision_class": "fake_real_forward"},
                    "vector_len": len(vector),
                    "vector_sha256": sha256_bytes(json.dumps(vector).encode("utf-8")),
                }
                payload = json.dumps(record, sort_keys=True, separators=(",", ":")).encode("utf-8")
                records.append(record)
                index_rows.append({
                    "schema_version": 1,
                    "cache_key_sha256": record["cache_key_sha256"],
                    "embedder_id": embedder,
                    "packed_record_path": str(record_path),
                    "byte_offset": offset_bytes,
                    "byte_len": len(payload),
                    "record_sha256": sha256_bytes(payload),
                    "vector_len": record["vector_len"],
                    "vector_sha256": record["vector_sha256"],
                })
                offset_bytes += len(payload) + 1
            write_jsonl(record_path, records)
            write_jsonl(index_path, index_rows)
            packed_manifest_path = cache_root / "python_forward_cache_packed_manifest.json"
            write_json(packed_manifest_path, {
                "schema_version": 1,
                "artifact_kind": "python_forward_cache_packed_manifest",
                "layout": "packed-jsonl",
                "row_count": len(records),
                "per_embedder": {
                    embedder: {
                        "record_path": str(record_path),
                        "record_sha256": sha256_bytes(record_path.read_bytes()),
                        "index_path": str(index_path),
                        "index_sha256": sha256_bytes(index_path.read_bytes()),
                        "row_count": len(records),
                    }
                },
                "passes": True,
            })
            manifest_path = cache_root / "python_forward_cache_manifest.json"
            manifest = {
                "schema_version": 1,
                "status": "passed",
                "passes": True,
                "inputs": {
                    "chunks_jsonl": str(chunks_jsonl),
                    "chunk_offset": offset,
                    "max_chunks": max_chunks,
                    "only_embedder": embedder,
                    "cache_store_layout": "packed-jsonl",
                    "corpus_profile": os.environ.get("CG_MEJEPA_PY_FORWARD_CORPUS_PROFILE", "static_ast_python"),
                },
                "source_of_truth": {
                    "manifest": str(manifest_path),
                    "packed_manifest": str(packed_manifest_path),
                    "packed_record_root": str(packed_root),
                    "readback": str(readback_root / "python_forward_cache_readback.json"),
                },
                "summary": {
                    "chunk_count": len(selected),
                    "active_embedder_count": 1,
                    "active_embedder_ids": [embedder],
                    "cache_record_count": len(records),
                    "quarantine_row_count": 0,
                    "expected_pairs": len(selected),
                    "accounted_pairs": len(records),
                },
                "per_embedder_coverage": {embedder: {"embedder_id": embedder, "cache_record_count": len(records), "passes": True}},
            }
            write_json(manifest_path, manifest)
            write_json(readback_root / "python_forward_cache_readback.json", {
                "schema_version": 1,
                "artifact_kind": "python_forward_cache_readback",
                "passes": True,
                "source_of_truth": {"manifest": str(manifest_path), "packed_record_root": str(packed_root)},
                "summary": manifest["summary"],
            })
            '''
        ).lstrip(),
        encoding="utf-8",
    )
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


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


def test_v3_index_generator_derives_legacy_missing_source_text_sha(tmp_path: Path):
    record = base_record("e1", "chunk-a")
    del record["key"]["source_text_sha256"]
    forward_root = write_forward_root(tmp_path / "forward", {"e1": [record]})

    readback = run_pipeline(args_for(tmp_path, forward_root))
    index_row = json.loads((tmp_path / "out" / "global_index" / "shard-000000-offset-000000000.jsonl").read_text(encoding="utf-8"))

    assert readback["passes"] is True
    assert index_row["source_text_sha256"] == index_row["source_sha256"]
    assert index_row["legacy_source_text_sha256_derived_from_source_sha256"] is True


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


def test_missing_key_planner_selects_only_target_embedder_new_model(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a"), base_record("e1", "chunk-b")],
            "e6": [base_record("e6", "chunk-a"), base_record("e6", "chunk-b")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))

    readback = run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    inventory_rows = [
        json.loads(line)
        for line in (tmp_path / "plan" / "missing_key_inventory.jsonl").read_text(encoding="utf-8").splitlines()
    ]

    assert readback["passes"] is True
    assert readback["summary"]["eligible_chunk_count"] == 2
    assert readback["summary"]["missing_key_count"] == 2
    assert readback["summary"]["unchanged_embedder_selected_count"] == 0
    assert readback["summary"]["unchanged_embedder_selection_counts"] == {"e6": 0}
    assert {row["embedder_id"] for row in inventory_rows} == {"e1"}
    assert {row["model_sha256"] for row in inventory_rows} == {fixture_sha("model:e1:next")}
    assert all(row["reason"] == "target_model_row_missing" for row in inventory_rows)
    assert all(case["pass"] is True for case in readback["cases"])


def test_missing_key_planner_rejects_missing_active_model_pointer(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    active_models_path = tmp_path / "out" / "active_models.json"
    active_models = json.loads(active_models_path.read_text(encoding="utf-8"))
    del active_models["active_models"]["e1"]
    write_json(active_models_path, active_models)

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_MODEL_POINTER_MISSING"):
        run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))


def test_missing_key_planner_rejects_chunker_mismatch(tmp_path: Path):
    forward_root = write_forward_root(tmp_path / "forward", {"e1": [base_record("e1", "chunk-a")]})
    run_pipeline(args_for(tmp_path, forward_root))
    active_models_path = tmp_path / "out" / "active_models.json"
    active_models = json.loads(active_models_path.read_text(encoding="utf-8"))
    active_models["chunker_sha256"] = fixture_sha("different-chunker")
    write_json(active_models_path, active_models)

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNKER_MISMATCH"):
        run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))


def test_missing_key_planner_rejects_empty_selected_row_set(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a"), base_record("e1", "chunk-b")],
            "e6": [base_record("e6", "chunk-a"), base_record("e6", "chunk-b")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_EMPTY_SELECTED_ROW_SET"):
        run_missing_key_plan(
            plan_args_for(
                tmp_path,
                tmp_path / "out",
                target_model_sha256=fixture_sha("model:e1"),
            )
        )


def test_recompute_missing_runs_target_only_and_preserves_unchanged_records(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a"), base_record("e1", "chunk-b")],
            "e6": [base_record("e6", "chunk-a"), base_record("e6", "chunk-b")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a", "chunk-b"])
    inventory_path = tmp_path / "plan" / "missing_key_inventory.jsonl"
    active_e1_rows = [
        json.loads(line)
        for line in inventory_path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    monkeypatch.setenv("FAKE_FORWARD_MODEL_SHA256", fixture_sha("model:e1:next"))
    monkeypatch.setenv("FAKE_FORWARD_PREPROCESSING_SHA256", active_e1_rows[0]["preprocessing_sha256"])

    readback = run_recompute_missing(
        recompute_args_for(
            tmp_path,
            tmp_path / "out",
            inventory_path,
            tmp_path / "chunks.jsonl",
            runner,
        )
    )

    assert readback["passes"] is True
    assert readback["summary"]["target_embedder_id"] == "e1"
    assert readback["summary"]["missing_key_count"] == 2
    assert readback["summary"]["recomputed_v3_row_count"] == 2
    assert readback["summary"]["unchanged_record_file_count"] == 1
    assert readback["summary"]["unchanged_record_changed_count"] == 0
    assert all(case["pass"] is True for case in readback["cases"])


def test_promote_active_model_writes_generation_and_rollback(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a"), base_record("e1", "chunk-b")],
            "e6": [base_record("e6", "chunk-a"), base_record("e6", "chunk-b")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    inventory_path = tmp_path / "plan" / "missing_key_inventory.jsonl"
    inventory_rows = [json.loads(line) for line in inventory_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a", "chunk-b"])
    monkeypatch.setenv("FAKE_FORWARD_MODEL_SHA256", fixture_sha("model:e1:next"))
    monkeypatch.setenv("FAKE_FORWARD_PREPROCESSING_SHA256", inventory_rows[0]["preprocessing_sha256"])
    run_recompute_missing(
        recompute_args_for(
            tmp_path,
            tmp_path / "out",
            inventory_path,
            tmp_path / "chunks.jsonl",
            runner,
        )
    )

    readback = run_promote_active_model(
        promote_args_for(
            tmp_path,
            tmp_path / "out",
            tmp_path / "recompute" / "recomputed_v3",
        )
    )
    promoted = json.loads((tmp_path / "promotion" / "active_models.promoted.json").read_text(encoding="utf-8"))
    rollback = json.loads((tmp_path / "promotion" / "active_models.rollback.json").read_text(encoding="utf-8"))

    assert readback["passes"] is True
    assert readback["summary"]["active_model_changed_count"] == 1
    assert readback["summary"]["eligible_chunk_count"] == 2
    assert readback["summary"]["recomputed_target_row_count"] == 2
    assert readback["summary"]["unchanged_pointer_changed_count"] == 0
    assert readback["summary"]["unchanged_record_changed_count"] == 0
    assert promoted["active_models"]["e1"] == fixture_sha("model:e1:next")
    assert promoted["active_models"]["e6"] == fixture_sha("model:e6")
    assert rollback["active_models"]["e1"] == fixture_sha("model:e1")
    assert all(case["pass"] is True for case in readback["cases"])


def test_promote_active_model_rejects_noop_pointer(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_PROMOTION_NOOP"):
        run_promote_active_model(
            promote_args_for(
                tmp_path,
                tmp_path / "out",
                tmp_path / "out",
                target_model_sha256=fixture_sha("model:e1"),
            )
        )


def test_recompute_missing_rejects_empty_inventory(tmp_path: Path):
    inventory = tmp_path / "empty.jsonl"
    inventory.write_text("", encoding="utf-8")
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a"])
    forward_root = write_forward_root(tmp_path / "forward", {"e6": [base_record("e6", "chunk-a")]})
    run_pipeline(args_for(tmp_path, forward_root))

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_EMPTY"):
        run_recompute_missing(
            recompute_args_for(tmp_path, tmp_path / "out", inventory, tmp_path / "chunks.jsonl", runner)
        )


def test_recompute_missing_rejects_mixed_target_inventory(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    inventory_path = tmp_path / "plan" / "missing_key_inventory.jsonl"
    rows = [json.loads(line) for line in inventory_path.read_text(encoding="utf-8").splitlines() if line.strip()]
    mixed = dict(rows[0])
    mixed["embedder_id"] = "e6"
    key = {field: str(mixed[field]) for field in (
        "corpus_profile",
        "chunk_id",
        "source_sha256",
        "source_text_sha256",
        "embedder_id",
        "model_sha256",
        "chunker_sha256",
        "preprocessing_sha256",
    )}
    mixed["cache_key_sha256"] = sha256_json(key)
    write_jsonl(inventory_path, [rows[0], mixed])
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a"])

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_MISSING_INVENTORY_TARGET_MISMATCH"):
        run_recompute_missing(
            recompute_args_for(tmp_path, tmp_path / "out", inventory_path, tmp_path / "chunks.jsonl", runner)
        )


def test_recompute_missing_rejects_stale_chunk_source_identity(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    inventory_path = tmp_path / "plan" / "missing_key_inventory.jsonl"
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_jsonl(
        tmp_path / "chunks.jsonl",
        [
            {
                "chunk_id": "chunk-a",
                "source_text": "def stale():\n    return 1\n",
                "source_sha256": fixture_sha("stale-source"),
                "file_sha256": fixture_sha("file:chunk-a"),
                "task_id": "task-py-g-733",
                "task_instance_id": "fixture-chunk-a",
                "repo": "fixture/repo",
                "workspace_state": "mutated",
                "absolute_path": "/zfs/archive/contextgraph/source-files/stale/pkg/example.py",
                "relative_path": "pkg/example.py",
                "mutation_category": "subtle_flip",
            }
        ],
    )

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_CHUNK_SOURCE_MISSING"):
        run_recompute_missing(
            recompute_args_for(tmp_path, tmp_path / "out", inventory_path, tmp_path / "chunks.jsonl", runner)
        )


def test_recompute_missing_rejects_output_index_overlap(tmp_path: Path):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a"])

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_OUTPUT_OVERLAPS_INDEX_ROOT"):
        run_recompute_missing(
            recompute_args_for(
                tmp_path,
                tmp_path / "out",
                tmp_path / "plan" / "missing_key_inventory.jsonl",
                tmp_path / "chunks.jsonl",
                runner,
                output_root=tmp_path / "out" / "nested",
            )
        )


def test_recompute_missing_detects_unchanged_record_rewrite(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    forward_root = write_forward_root(
        tmp_path / "forward",
        {
            "e1": [base_record("e1", "chunk-a")],
            "e6": [base_record("e6", "chunk-a")],
        },
    )
    run_pipeline(args_for(tmp_path, forward_root))
    run_missing_key_plan(plan_args_for(tmp_path, tmp_path / "out"))
    inventory_path = tmp_path / "plan" / "missing_key_inventory.jsonl"
    row = json.loads(inventory_path.read_text(encoding="utf-8"))
    runner = tmp_path / "fake_runner.py"
    write_fake_recompute_runner(runner)
    write_chunks_jsonl(tmp_path / "chunks.jsonl", ["chunk-a"])
    monkeypatch.setenv("FAKE_FORWARD_MODEL_SHA256", fixture_sha("model:e1:next"))
    monkeypatch.setenv("FAKE_FORWARD_PREPROCESSING_SHA256", row["preprocessing_sha256"])
    monkeypatch.setenv(
        "FAKE_MUTATE_UNCHANGED_RECORD",
        str(tmp_path / "forward" / "shards" / "shard-000000-offset-000000000" / "packed_records" / "e6.jsonl"),
    )

    with pytest.raises(IncrementalIndexError, match="MEJEPA_FORWARD_CACHE_INCREMENTAL_UNCHANGED_RECORD_FILE_REWRITTEN"):
        run_recompute_missing(
            recompute_args_for(tmp_path, tmp_path / "out", inventory_path, tmp_path / "chunks.jsonl", runner)
        )
