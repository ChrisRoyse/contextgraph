from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path

from .. import python_forward_shard_scheduler as scheduler


def _write_fake_forward_runner(path: Path) -> None:
    path.write_text(
        r'''
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True))
            handle.write("\n")


cache_root = Path(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_ROOT"])
readback_root = Path(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_READBACK_ROOT"])
slug = os.environ.get("CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER", "e1")
chunk_offset = int(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET"])
max_chunks = int(os.environ["CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS"])
chunks_jsonl = os.environ["CG_MEJEPA_PY_CHUNKS_JSONL"]

packed_root = cache_root / "packed_records"
packed_root.mkdir(parents=True, exist_ok=True)
record_path = packed_root / f"{slug}.jsonl"
packed_index_path = packed_root / f"{slug}.index.jsonl"
input_sha = sha256(f"input:{slug}".encode("utf-8"))
vector_sha = sha256(f"vector:{slug}".encode("utf-8"))
cache_key_sha = sha256(f"cache:{slug}".encode("utf-8"))
record = {
    "schema_version": 1,
    "cache_key_sha256": cache_key_sha,
    "embedder_id": slug,
    "input_sha256": input_sha,
    "source_sha256": input_sha,
    "vector_len": 3,
    "vector_sha256": vector_sha,
}
record_payload = json.dumps(record, sort_keys=True).encode("utf-8")
write_jsonl(record_path, [record])
index_row = {
    "schema_version": 1,
    "cache_key_sha256": cache_key_sha,
    "embedder_id": slug,
    "input_sha256": input_sha,
    "source_sha256": input_sha,
    "packed_record_path": str(record_path),
    "byte_offset": 0,
    "byte_len": len(record_payload),
    "record_sha256": sha256(record_payload),
    "vector_len": 3,
    "vector_sha256": vector_sha,
}
write_jsonl(packed_index_path, [index_row])
index_path = cache_root / "python_forward_cache_index.jsonl"
quarantine_path = cache_root / "python_forward_cache_quarantine.jsonl"
telemetry_path = cache_root / "python_forward_cache_batch_telemetry.jsonl"
write_jsonl(index_path, [index_row])
write_jsonl(quarantine_path, [])
write_jsonl(telemetry_path, [{"batch_id": f"batch:{slug}", "batch_size": 1}])
packed_manifest_path = cache_root / "python_forward_cache_packed_manifest.json"
write_json(
    packed_manifest_path,
    {
        "schema_version": 1,
        "artifact_kind": "python_forward_cache_packed_manifest",
        "layout": "packed-jsonl",
        "packed_record_root": str(packed_root),
        "row_count": 1,
        "per_embedder": {
            slug: {
                "record_path": str(record_path),
                "record_sha256": sha256(record_path.read_bytes()),
                "byte_count": record_path.stat().st_size,
                "index_path": str(packed_index_path),
                "index_sha256": sha256(packed_index_path.read_bytes()),
                "row_count": 1,
            }
        },
        "passes": True,
    },
)
summary = {
    "active_embedder_count": 1,
    "batch_telemetry_row_count": 1,
    "cache_record_count": 1,
    "chunk_count": 1,
    "first_pass_hits": 0,
    "first_pass_writes": 1,
    "global_cache_record_count_at_manifest_write": 1,
    "quarantine_row_count": 0,
    "second_pass_hits": 1,
}
write_json(
    cache_root / "python_forward_cache_manifest.json",
    {
        "schema_version": 1,
        "status": "passed",
        "passes": True,
        "inputs": {
            "cache_store_layout": "packed-jsonl",
            "chunk_offset": chunk_offset,
            "chunks_jsonl": chunks_jsonl,
            "max_chunks": max_chunks,
            "only_embedder": slug,
        },
        "source_of_truth": {
            "batch_telemetry_jsonl": str(telemetry_path),
            "index_jsonl": str(index_path),
            "packed_manifest": str(packed_manifest_path),
            "packed_record_root": str(packed_root),
            "quarantine_jsonl": str(quarantine_path),
        },
        "summary": summary,
        "per_embedder_coverage": {slug: {"embedder_id": slug, "row_count": 1}},
    },
)
write_json(
    readback_root / "python_forward_cache_readback.json",
    {
        "active_embedder_count": 1,
        "active_embedder_ids": [slug],
        "passes": True,
        "summary": summary,
    },
)
'''.lstrip(),
        encoding="utf-8",
    )


def _scheduler_args(tmp_path: Path, fake_runner: Path) -> argparse.Namespace:
    chunks_jsonl = tmp_path / "chunks.jsonl"
    chunks_jsonl.write_text(json.dumps({"chunk_id": "synthetic"}) + "\n", encoding="utf-8")
    return argparse.Namespace(
        active_embedder_count=1,
        cache_root=tmp_path / "durable-cache",
        cache_store_layout="packed",
        cargo_runner=f"{sys.executable} {fake_runner}",
        chunks_jsonl=chunks_jsonl,
        cpu_workers=1,
        embedder_batch_size_map={},
        embedder_max_padded_source_byte_map={},
        gpu_vram_guard_bytes=1,
        gpu_workers=1,
        hot_root=tmp_path / "hot",
        io_workers=1,
        isolate_embedder_processes=True,
        jsonl_sync_every_rows=1,
        max_source_bytes=32768,
        readback_root=tmp_path / "durable-readback",
        require_packed_store=True,
        runner_timeout_seconds=30,
        stage_packed_shards_on_hot=True,
        true_batch_max_padded_source_bytes=None,
        true_batch_size=1,
    )


def _read_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def test_e8_dispatch_groups_serialize_load_sensitive_slug(monkeypatch):
    monkeypatch.setattr(scheduler, "ISOLATED_FORWARD_SLUGS", ("e1", "e8", "e9"))

    serialized, parallel = scheduler.isolated_forward_dispatch_groups(4)

    assert serialized == ("e8",)
    assert parallel == ("e1", "e9")
    assert scheduler.isolated_forward_dispatch_groups(1) == ((), ("e1", "e8", "e9"))


def test_isolated_hot_stage_subrun_does_not_publish_composite_readback(tmp_path, monkeypatch):
    monkeypatch.setattr(
        scheduler,
        "resource_snapshot",
        lambda _args, phase: {"phase": phase, "synthetic": True},
    )
    monkeypatch.setattr(scheduler, "ISOLATED_FORWARD_SLUGS", ("e1",))
    fake_runner = tmp_path / "fake_forward_runner.py"
    _write_fake_forward_runner(fake_runner)
    args = _scheduler_args(tmp_path, fake_runner)
    shard = scheduler.ShardSpec(
        "shard-000000-offset-000000000",
        0,
        1,
        args.cache_root / "shards" / "shard-000000-offset-000000000",
        args.readback_root / "shards" / "shard-000000-offset-000000000",
    )
    event_path = args.readback_root / "scheduler_events.jsonl"
    run_log_name = f"{shard.shard_id}.e1.json"

    subrun = scheduler.run_shard(
        args,
        shard,
        event_path,
        {"synthetic_preflight": True},
        extra_env=scheduler.isolated_embedder_extra_env(args, "e1"),
        run_log_name=run_log_name,
        event_phase="shard_embedder_subrun_complete",
        require_packed_store=False,
        seed_hot_stage_from_durable=False,
        promote_hot_stage=False,
    )

    assert not shard.manifest_path().exists()
    assert not shard.readback_path().exists()
    assert subrun["summary"]["active_embedder_count"] == 1

    stage_shard = scheduler.hot_stage_shard(args, shard, run_log_name)
    stage_root = scheduler.hot_stage_root(args, shard, run_log_name)
    assert stage_shard.manifest_path().is_file()
    assert stage_shard.readback_path().is_file()
    probe_path = tmp_path / "fsv_partial_publication_probe.json"
    pre_consolidation_probe = {
        "durable_manifest_exists_after_subrun": shard.manifest_path().exists(),
        "durable_readback_exists_after_subrun": shard.readback_path().exists(),
        "stage_manifest_exists_after_subrun": stage_shard.manifest_path().exists(),
        "stage_readback_exists_after_subrun": stage_shard.readback_path().exists(),
        "stage_manifest_path": str(stage_shard.manifest_path()),
        "stage_readback_path": str(stage_shard.readback_path()),
    }

    snapshot = scheduler.snapshot_isolated_subrun_rows(args, stage_shard, "e1", subrun)
    shutil.rmtree(stage_root)
    assert not stage_root.exists()

    consolidation = scheduler.consolidate_isolated_subruns(args, shard, [snapshot], [subrun])

    assert consolidation["passes"] is True
    assert shard.manifest_path().is_file()
    assert shard.readback_path().is_file()
    readback = scheduler.read_json(shard.readback_path())
    assert readback["passes"] is True
    assert readback["summary"]["active_embedder_count"] == 1
    probe = {
        "pre_consolidation": pre_consolidation_probe,
        "post_consolidation": {
            "durable_manifest_exists": shard.manifest_path().exists(),
            "durable_readback_exists": shard.readback_path().exists(),
            "durable_readback_path": str(shard.readback_path()),
            "durable_active_embedder_count": readback["summary"]["active_embedder_count"],
            "stage_root_exists_after_snapshot_cleanup": stage_root.exists(),
        },
        "passes": True,
    }
    probe_path.write_text(json.dumps(probe, indent=2, sort_keys=True), encoding="utf-8")
    probe_readback = json.loads(probe_path.read_text(encoding="utf-8"))
    assert probe_readback == probe


def test_parallel_isolated_scheduler_runs_e8_outside_worker_pool(tmp_path, monkeypatch):
    monkeypatch.setattr(
        scheduler,
        "resource_snapshot",
        lambda _args, phase: {"phase": phase, "synthetic": True},
    )
    monkeypatch.setattr(scheduler, "ISOLATED_FORWARD_SLUGS", ("e1", "e8"))
    monkeypatch.setenv("CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS", "2")
    fake_runner = tmp_path / "fake_forward_runner.py"
    _write_fake_forward_runner(fake_runner)
    args = _scheduler_args(tmp_path, fake_runner)
    args.active_embedder_count = 2
    shard = scheduler.ShardSpec(
        "shard-000000-offset-000000000",
        0,
        1,
        args.cache_root / "shards" / "shard-000000-offset-000000000",
        args.readback_root / "shards" / "shard-000000-offset-000000000",
    )
    event_path = args.readback_root / "scheduler_events.jsonl"

    run = scheduler.run_shard_embedder_isolated(args, shard, event_path, {"synthetic_preflight": True})

    events = _read_jsonl(event_path)
    serialized_events = [
        row for row in events if row.get("phase") == "shard_embedder_isolation_serialized_subruns"
    ]
    assert len(serialized_events) == 1
    serialized_event = serialized_events[0]["detail"]
    assert serialized_event["serialized_embedder_slugs"] == ["e8"]
    assert serialized_event["parallel_embedder_slugs"] == ["e1"]
    assert run["summary"]["active_embedder_count"] == 2
    assert run["summary"]["accounted_pairs"] == 2
    assert scheduler.read_json(shard.readback_path())["summary"]["active_embedder_count"] == 2
    assert "e8" in {
        row["embedder_id"]
        for row in run["isolated_embedder_subruns"]
    }
    probe = {
        "serialized_event": serialized_event,
        "final_readback": scheduler.read_json(shard.readback_path()),
        "passes": True,
    }
    probe_path = tmp_path / "fsv_e8_serialized_dispatch_probe.json"
    probe_path.write_text(json.dumps(probe, indent=2, sort_keys=True), encoding="utf-8")
    assert json.loads(probe_path.read_text(encoding="utf-8")) == probe
