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


def _distinctness_rows(embedder_id: str, pairs: list[tuple[str, str]]) -> list[dict[str, str]]:
    return [
        {
            "embedder_id": embedder_id,
            "input_sha256": input_sha256,
            "source_sha256": input_sha256,
            "vector_sha256": vector_sha256,
        }
        for vector_sha256, input_sha256 in pairs
    ]


def test_distinctness_accepts_low_support_duplicate_heavy_identity_preserved_input():
    pairs: list[tuple[str, str]] = []
    for idx in range(56):
        pairs.append((f"sha256:vector-{idx:04}", f"sha256:input-{idx:04}"))
    for idx in range(60):
        source = idx % 56
        pairs.append((f"sha256:vector-{source:04}", f"sha256:input-{source:04}"))

    report = scheduler.evaluate_embedder_distinctness(["e1"], _distinctness_rows("e1", pairs))

    assert report["passes"] is True
    metrics = report["per_embedder_metrics"]["e1"]
    assert metrics["distinctness_gate_status"] == "usable_duplicate_limited"
    assert metrics["forward_record_count"] == 116
    assert metrics["unique_input_sha256_count"] == 56
    assert metrics["unique_vector_sha256_count"] == 56
    assert metrics["vector_collision_group_count"] == 0
    assert metrics["observed_input_identity_preserved"] is True


def test_distinctness_treats_exact_vector_collisions_above_floor_as_benign():
    # 20 of 100 distinct inputs collide into 10 shared vectors (non_colliding = 0.8):
    # below the 0.9 clean threshold but well above the 0.5 catastrophic floor with
    # deterministic inputs. This is the shard-2418 shape (uncased embedder mapping
    # case-only variants together) and must be NON-FATAL.
    pairs: list[tuple[str, str]] = []
    for idx in range(100):
        vector = f"sha256:shared-{idx // 2:04}" if idx < 20 else f"sha256:vector-{idx:04}"
        pairs.append((vector, f"sha256:input-{idx:04}"))

    report = scheduler.evaluate_embedder_distinctness(["e9"], _distinctness_rows("e9", pairs))

    assert report["passes"] is True
    assert report["collapse_failures"] == []
    assert len(report["benign_collisions"]) == 1
    assert report["benign_collisions"][0]["info_code"] == scheduler.INFO_EMBEDDER_BENIGN_COLLISION
    metrics = report["per_embedder_metrics"]["e9"]
    assert metrics["distinctness_gate_status"] == "usable_benign_collision"
    assert metrics["distinctness_fraction"] == 0.9
    assert metrics["vector_collision_group_count"] == 10
    assert metrics["non_colliding_unique_input_fraction"] == 0.8
    # Determinism is preserved (same input -> one vector), so this is not a collapse.
    assert metrics["observed_input_identity_preserved"] is True


def test_distinctness_rejects_nondeterministic_input_identity():
    # The SAME input mapping to two different vectors is a nondeterministic embedder
    # (e.g. batch-position float drift): always fatal.
    pairs = [
        ("sha256:vector-a", "sha256:input-shared"),
        ("sha256:vector-b", "sha256:input-shared"),
        ("sha256:vector-c", "sha256:input-002"),
        ("sha256:vector-d", "sha256:input-003"),
    ]

    report = scheduler.evaluate_embedder_distinctness(["e1"], _distinctness_rows("e1", pairs))

    assert report["passes"] is False
    assert report["error_code"] == scheduler.ERR_EMBEDDER_COLLAPSE
    assert len(report["collapse_failures"]) == 1
    assert report["collapse_failures"][0]["collapse_reason"] == "input_identity_nondeterministic"
    metrics = report["per_embedder_metrics"]["e1"]
    assert metrics["distinctness_gate_status"] == "collapse_nondeterministic"
    assert metrics["input_with_multiple_vectors_count"] == 1


def test_distinctness_rejects_catastrophic_vector_collision_below_floor():
    # 60 of 100 distinct inputs collide (non_colliding = 0.4 < 0.5): genuinely
    # degenerate embedder for this shard. Fatal, fail fast.
    pairs: list[tuple[str, str]] = []
    for idx in range(100):
        vector = f"sha256:shared-{idx // 3:04}" if idx < 60 else f"sha256:vector-{idx:04}"
        pairs.append((vector, f"sha256:input-{idx:04}"))

    report = scheduler.evaluate_embedder_distinctness(["e9"], _distinctness_rows("e9", pairs))

    assert report["passes"] is False
    assert report["error_code"] == scheduler.ERR_EMBEDDER_COLLAPSE
    assert len(report["collapse_failures"]) == 1
    assert report["collapse_failures"][0]["collapse_reason"] == "catastrophic_vector_collision"
    metrics = report["per_embedder_metrics"]["e9"]
    assert metrics["distinctness_gate_status"] == "collapse_catastrophic"


def test_corpus_non_colliding_floor_passes_when_benign_collisions_are_rare():
    # One benign-collision shard (0.80) diluted across a healthy corpus stays above the
    # 0.90 corpus floor: weighted = (99*1.0 + 80*0.80...) here approximated with totals.
    # e1: 990 of 1000 distinct inputs non-colliding corpus-wide = 0.99 -> pass.
    fractions, failures = scheduler.evaluate_corpus_non_colliding_floor(
        unique_input_totals={"e1": 1000, "e7": 1000},
        non_colliding_input_totals={"e1": 990, "e7": 1000},
        benign_collision_shard_counts={"e1": 3},
    )
    assert failures == []
    assert abs(fractions["e1"] - 0.99) < 1e-9
    assert fractions["e7"] == 1.0


def test_corpus_non_colliding_floor_fails_on_corpus_wide_collapse():
    # An embedder that collides corpus-wide (only 60% of distinct inputs non-colliding)
    # is genuinely degenerate and must fail the corpus floor, unlike a few benign shards.
    fractions, failures = scheduler.evaluate_corpus_non_colliding_floor(
        unique_input_totals={"e1": 1000, "e9": 1000},
        non_colliding_input_totals={"e1": 600, "e9": 1000},
        benign_collision_shard_counts={"e1": 400},
    )
    assert len(failures) == 1
    failure = failures[0]
    assert failure["embedder_id"] == "e1"
    assert failure["error_code"] == scheduler.ERR_CORPUS_EMBEDDER_COLLAPSE
    assert abs(failure["corpus_non_colliding_unique_input_fraction"] - 0.6) < 1e-9
    assert failure["corpus_min_non_colliding_unique_input_fraction"] == 0.9
    assert fractions["e9"] == 1.0


def test_corpus_non_colliding_floor_passes_at_exact_threshold():
    # Exactly at the 0.90 floor is acceptable (>=, not >).
    _, failures = scheduler.evaluate_corpus_non_colliding_floor(
        unique_input_totals={"e1": 1000},
        non_colliding_input_totals={"e1": 900},
        benign_collision_shard_counts={},
    )
    assert failures == []


def test_record_failed_shard_writes_durable_manifest(tmp_path):
    args = argparse.Namespace(readback_root=tmp_path / "readback")
    shard = scheduler.ShardSpec(
        shard_id="shard-002418-offset-000309504",
        chunk_offset=309504,
        max_chunks=128,
        cache_root=tmp_path / "cache" / "shard-002418",
        readback_root=tmp_path / "readback" / "shard-002418",
    )
    record = scheduler.record_failed_shard(
        args,
        shard,
        error="MEJEPA_FORWARD_CACHE_EMBEDDER_COLLAPSE: simulated runner failure",
        failure_index=1,
        consecutive_failures=1,
    )
    assert record["shard_id"] == "shard-002418-offset-000309504"
    assert record["chunk_offset"] == 309504
    assert "simulated runner failure" in record["error"]
    # Independently read the durable Source of Truth back.
    manifest_path = scheduler.failed_shards_manifest_path(args)
    assert manifest_path.is_file()
    lines = manifest_path.read_text(encoding="utf-8").strip().splitlines()
    assert len(lines) == 1
    readback = json.loads(lines[0])
    assert readback["shard_id"] == "shard-002418-offset-000309504"
    assert readback["artifact_kind"] == "python_forward_cache_failed_shard"
    # A second failure appends, not overwrites.
    scheduler.record_failed_shard(
        args, shard, error="second failure", failure_index=2, consecutive_failures=2
    )
    assert len(manifest_path.read_text(encoding="utf-8").strip().splitlines()) == 2
