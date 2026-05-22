#!/usr/bin/env python3
"""Fail-closed #255 gate before relaunching the full Python forward-cache run."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-064"
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-064-post249-full-run-relaunch-gate-readback")
DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_CACHE_ROOT = Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515")
DEFAULT_RUN_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-016-full-c8117f15-20260515-readback")
DEFAULT_HOT_ROOT = Path("/home/cabdru/.cache/contextgraph/task-py-data-016-full-c8117f15-20260515")
DEFAULT_STALE_CACHE_ROOT = Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-20260514")
DEFAULT_STALE_RUN_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-016-full-20260514-readback")
DEFAULT_STALE_HOT_ROOT = Path("/home/cabdru/.cache/contextgraph/task-py-data-016-full-20260514")
DEFAULT_SMOKE_MANIFEST_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-060-post249-smoke-manifest-readback/smoke_manifest_readback.json")
DEFAULT_SMOKE_FORWARD_READBACK = Path(
    "/zfs/archive/contextgraph/readback/task-py-data-061-post249-smoke-forward-readback/python_forward_shard_scheduler_readback.json"
)
DEFAULT_SMOKE_INTELLIGENCE_READBACK = Path(
    "/zfs/archive/contextgraph/readback/task-py-data-062-post249-smoke-intelligence-readback/smoke_intelligence_matrices_readback.json"
)
DEFAULT_SMOKE_STORAGE_DESIGN_READBACK = Path(
    "/zfs/archive/contextgraph/readback/task-py-data-063-post249-smoke-storage-design-readback/smoke_storage_design_readback.json"
)
DEFAULT_STORAGE_DESIGN_GO_NO_GO = Path(
    "/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/go_no_go.json"
)
DEFAULT_STORAGE_POLICY = Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/storage_policy.json")
DEFAULT_STORAGE_ESTIMATES = Path(
    "/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/full_scale_storage_estimates.json"
)
DEFAULT_FULL_AST_CORPUS_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-058-full-ast-corpus-readback/full_ast_corpus_readback.json")
DEFAULT_CARGO_RUNNER = "target/release/examples/python_forward_cache_readback"


class FullRunRelaunchGateError(RuntimeError):
    pass


def utc_now() -> str:
    import datetime as _dt

    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def require(condition: bool, code: str, message: str) -> None:
    if not condition:
        raise FullRunRelaunchGateError(f"{code}: {message}")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


def read_json(path: Path) -> Any:
    try:
        with path.open("r", encoding="utf-8") as fh:
            return json.load(fh)
    except FileNotFoundError as exc:
        raise FullRunRelaunchGateError(f"FULL_RUN_RELAUNCH_JSON_MISSING: {path}") from exc
    except json.JSONDecodeError as exc:
        raise FullRunRelaunchGateError(f"FULL_RUN_RELAUNCH_JSON_INVALID: {path}: {exc}") from exc


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(value, fh, sort_keys=True, indent=2, separators=(",", ": "))
        fh.write("\n")
    tmp.replace(path)


def file_state(path: Path, *, hash_file: bool = True) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "is_file": False}
    state: dict[str, Any] = {
        "path": str(path),
        "exists": True,
        "is_file": path.is_file(),
        "byte_count": path.stat().st_size,
    }
    if path.is_file() and hash_file:
        state["sha256"] = sha256_file(path)
    return state


def tree_state(root: Path, *, hash_files: bool = True) -> dict[str, Any]:
    if not root.exists():
        return {
            "path": str(root),
            "exists": False,
            "is_dir": False,
            "dir_count": 0,
            "file_count": 0,
            "byte_count": 0,
            "files": [],
            "children": [],
        }
    if root.is_file():
        return {
            "path": str(root),
            "exists": True,
            "is_dir": False,
            "dir_count": 0,
            "file_count": 1,
            "byte_count": root.stat().st_size,
            "files": [file_state(root, hash_file=hash_files)],
            "children": [],
        }
    files: list[dict[str, Any]] = []
    dir_count = 0
    byte_count = 0
    for child in sorted(root.rglob("*")):
        if child.is_dir():
            dir_count += 1
            continue
        if not child.is_file():
            continue
        stat = child.stat()
        byte_count += stat.st_size
        row = {
            "path": str(child),
            "relative_path": str(child.relative_to(root)),
            "byte_count": stat.st_size,
        }
        if hash_files:
            row["sha256"] = sha256_file(child)
        files.append(row)
    return {
        "path": str(root),
        "exists": True,
        "is_dir": True,
        "dir_count": dir_count,
        "file_count": len(files),
        "byte_count": byte_count,
        "files": files,
        "children": sorted(child.name for child in root.iterdir()),
    }


def artifact_hashes(root: Path) -> dict[str, Any]:
    if not root.exists():
        return {"root": str(root), "file_count": 0, "byte_count": 0, "files": []}
    files = []
    for child in sorted(root.rglob("*")):
        if child.is_file():
            files.append(
                {
                    "path": str(child),
                    "relative_path": str(child.relative_to(root)),
                    "bytes": child.stat().st_size,
                    "sha256": sha256_file(child),
                }
            )
    return {"root": str(root), "file_count": len(files), "byte_count": sum(row["bytes"] for row in files), "files": files}


def disk_snapshot(path: Path) -> dict[str, Any]:
    usage = shutil.disk_usage(path)
    return {"path": str(path), "total": usage.total, "used": usage.used, "free": usage.free}


def path_is_under(path: Path, root: Path) -> bool:
    try:
        path.resolve(strict=False).relative_to(root.resolve(strict=False))
        return True
    except ValueError:
        return False


def validate_required_artifact(name: str, issue: int, task_id: str, path: Path, *, expected_chunks_jsonl: Path | None = None) -> dict[str, Any]:
    state = file_state(path)
    if not path.is_file():
        return {
            "name": name,
            "issue": issue,
            "expected_task_id": task_id,
            "path": str(path),
            "state": state,
            "passes": False,
            "error_code": "FULL_RUN_RELAUNCH_REQUIRED_ARTIFACT_MISSING",
        }
    payload = read_json(path)
    passes = payload.get("task_id") == task_id and payload.get("passes") is True
    chunk_check: dict[str, Any] | None = None
    if expected_chunks_jsonl is not None:
        chunks_state = file_state(expected_chunks_jsonl)
        source_of_truth = payload.get("source_of_truth") or {}
        after_state = payload.get("after_state") or {}
        recommended = payload.get("recommended_full_run") or {}
        payload_paths = {
            source_of_truth.get("chunks_jsonl"),
            source_of_truth.get("source_chunks_jsonl"),
            source_of_truth.get("full_chunks_jsonl"),
            recommended.get("chunks_jsonl"),
        }
        payload_hashes = {
            recommended.get("chunks_sha256"),
            (after_state.get("chunks") or {}).get("sha256"),
            (after_state.get("source_chunks") or {}).get("sha256"),
            ((payload.get("readback") or {}).get("full_chunks_state") or {}).get("sha256"),
        }
        chunk_check = {
            "expected_chunks_jsonl": str(expected_chunks_jsonl),
            "payload_chunks_jsonl_candidates": sorted(str(item) for item in payload_paths if item),
            "expected_chunks_state": chunks_state,
            "payload_chunks_sha256_candidates": sorted(str(item) for item in payload_hashes if item),
            "passes": str(expected_chunks_jsonl) in payload_paths and chunks_state.get("sha256") in payload_hashes,
        }
        passes = passes and chunk_check["passes"]
    return {
        "name": name,
        "issue": issue,
        "expected_task_id": task_id,
        "path": str(path),
        "state": state,
        "payload_task_id": payload.get("task_id"),
        "payload_artifact_kind": payload.get("artifact_kind"),
        "payload_passes": payload.get("passes"),
        "chunk_check": chunk_check,
        "passes": passes,
    }


def relaunch_command(args: argparse.Namespace, *, mode: str | None = None) -> list[str]:
    command = [
        "python3",
        "scripts/swebench_lite_ops/python_forward_shard_scheduler.py",
        "--task-id",
        "TASK-PY-DATA-016",
        "--chunks-jsonl",
        str(args.chunks_jsonl),
        "--cache-root",
        str(args.cache_root),
        "--readback-root",
        str(args.run_readback_root),
        "--hot-root",
        str(args.hot_root),
        "--shard-size",
        str(args.shard_size),
        "--shard-count",
        str(args.shard_count),
        "--active-embedder-count",
        str(args.active_embedder_count),
        "--true-batch-size",
        str(args.true_batch_size),
        "--io-workers",
        str(args.io_workers),
        "--cpu-workers",
        str(args.cpu_workers),
        "--gpu-workers",
        str(args.gpu_workers),
        "--jsonl-sync-every-rows",
        str(args.jsonl_sync_every_rows),
        "--cache-store-layout",
        "packed",
        "--require-packed-store",
        "--stage-packed-shards-on-hot",
        "--resume-completed-shards",
        "--retry-failed-partial-shards",
        "--checkpoint-every-shards",
        str(args.checkpoint_every_shards),
        "--isolate-embedder-processes",
        "--cargo-runner",
        args.cargo_runner,
        "--require-smoke-gate",
        "--full-ast-corpus-readback",
        str(args.full_ast_corpus_readback),
    ]
    if mode:
        command.extend(["--mode", mode])
    return command


def validate_relaunch_policy(args: argparse.Namespace, command: list[str]) -> dict[str, Any]:
    hot_root = args.hot_root.resolve(strict=False)
    cache_root = args.cache_root.resolve(strict=False)
    readback_root = args.run_readback_root.resolve(strict=False)
    command_set = set(command)
    checks = [
        {
            "name": "hot_root_is_wsl_ext4_cache",
            "passes": path_is_under(hot_root, Path("/home/cabdru/.cache/contextgraph")),
            "actual": str(hot_root),
        },
        {
            "name": "hot_root_not_mnt_c",
            "passes": not path_is_under(hot_root, Path("/mnt/c")),
            "actual": str(hot_root),
        },
        {
            "name": "durable_cache_on_aiwonder_archive",
            "passes": path_is_under(cache_root, Path("/zfs/archive/contextgraph")),
            "actual": str(cache_root),
        },
        {
            "name": "readback_on_aiwonder_archive",
            "passes": path_is_under(readback_root, Path("/zfs/archive/contextgraph")),
            "actual": str(readback_root),
        },
        {"name": "packed_store_required", "passes": "--require-packed-store" in command_set, "actual": command},
        {"name": "hot_stage_enabled", "passes": "--stage-packed-shards-on-hot" in command_set, "actual": command},
        {"name": "resume_completed_enabled", "passes": "--resume-completed-shards" in command_set, "actual": command},
        {"name": "partial_retry_enabled", "passes": "--retry-failed-partial-shards" in command_set, "actual": command},
        {"name": "embedder_process_isolation_enabled", "passes": "--isolate-embedder-processes" in command_set, "actual": command},
        {"name": "smoke_gate_required", "passes": "--require-smoke-gate" in command_set, "actual": command},
        {"name": "full_ast_corpus_gate_required", "passes": "--full-ast-corpus-readback" in command_set, "actual": command},
        {"name": "gpu_worker_policy", "passes": args.gpu_workers == 1, "actual": args.gpu_workers},
        {"name": "io_worker_policy", "passes": 1 <= args.io_workers <= 2, "actual": args.io_workers},
        {"name": "cpu_worker_policy", "passes": args.cpu_workers == 30, "actual": args.cpu_workers},
        {"name": "checkpoint_policy", "passes": args.checkpoint_every_shards == 1, "actual": args.checkpoint_every_shards},
    ]
    return {"checks": checks, "passes": all(check["passes"] is True for check in checks)}


def validate_storage_design(args: argparse.Namespace) -> dict[str, Any]:
    go_no_go = read_json(args.storage_design_go_no_go)
    policy = read_json(args.storage_policy)
    estimates = read_json(args.storage_estimates)
    metric_policy = policy.get("metric_storage_policy") or []
    unbounded_ok = all(item.get("unbounded_pairwise_chunk_storage") is False for item in metric_policy)
    checks = [
        {
            "name": "smoke_design_accepted",
            "passes": go_no_go.get("storage_design_accepted") is True,
            "actual": go_no_go,
        },
        {
            "name": "full_run_blocked_before_255",
            "passes": go_no_go.get("may_start_full_run_now") is False and "#255" in (go_no_go.get("blocking_issues") or []),
            "actual": go_no_go,
        },
        {
            "name": "full_run_allowed_after_255",
            "passes": go_no_go.get("may_start_full_run_after_255") is True,
            "actual": go_no_go,
        },
        {
            "name": "unbounded_chunk_pair_storage_forbidden",
            "passes": unbounded_ok,
            "actual": metric_policy,
        },
        {
            "name": "storage_estimate_fits_aiwonder_archive",
            "passes": int(estimates.get("estimated_total_design_bytes") or 0) < int(estimates.get("archive_free_bytes") or 0),
            "actual": estimates,
        },
    ]
    return {
        "go_no_go": go_no_go,
        "storage_policy": policy,
        "storage_estimates": estimates,
        "checks": checks,
        "passes": all(check["passes"] is True for check in checks),
    }


def expected_shard_count(full_chunk_count: int, shard_size: int) -> int:
    require(shard_size > 0, "FULL_RUN_RELAUNCH_INVALID_SHARD_SIZE", f"shard_size={shard_size}")
    return int(math.ceil(full_chunk_count / float(shard_size)))


def expected_shard_id(shard_number: int, shard_size: int) -> str:
    return f"shard-{shard_number:06d}-offset-{shard_number * shard_size:09d}"


def validate_run_state(args: argparse.Namespace, *, full_chunk_count: int) -> dict[str, Any]:
    cache_checkpoint = args.cache_root / "python_forward_cache_checkpoint.json"
    readback_checkpoint = args.run_readback_root / "python_forward_cache_checkpoint.json"
    cache_state = file_state(cache_checkpoint)
    readback_state = file_state(readback_checkpoint)
    cache_tree = tree_state(args.cache_root, hash_files=True)
    readback_tree = tree_state(args.run_readback_root, hash_files=True)
    expected_total_shards = expected_shard_count(full_chunk_count, args.shard_size)
    expected_planned_chunk_capacity = expected_total_shards * args.shard_size

    if not cache_checkpoint.exists() and not readback_checkpoint.exists():
        actual_next_shard = expected_shard_id(0, args.shard_size)
        checks = [
            {"name": "fresh_cache_root_has_no_checkpoint", "passes": True, "actual": cache_state},
            {"name": "fresh_readback_root_has_no_checkpoint", "passes": True, "actual": readback_state},
            {"name": "cache_root_not_stale_legacy_root", "passes": args.cache_root != args.stale_cache_root, "actual": str(args.cache_root)},
            {"name": "readback_root_not_stale_legacy_root", "passes": args.run_readback_root != args.stale_run_readback_root, "actual": str(args.run_readback_root)},
            {
                "name": "cache_root_empty_or_absent_for_fresh_start",
                "passes": cache_tree.get("file_count") == 0,
                "actual": cache_tree,
            },
            {
                "name": "readback_root_empty_or_absent_for_fresh_start",
                "passes": readback_tree.get("file_count") == 0,
                "actual": readback_tree,
            },
            {
                "name": "fresh_next_shard_expected",
                "passes": args.expected_next_shard == "auto" or args.expected_next_shard == actual_next_shard,
                "actual": {"expected_next_shard_arg": args.expected_next_shard, "computed_next_shard": actual_next_shard},
            },
            {
                "name": "shard_count_matches_full_chunk_count",
                "passes": args.shard_count == expected_total_shards,
                "actual": {
                    "arg_shard_count": args.shard_count,
                    "expected_shard_count": expected_total_shards,
                    "full_chunk_count": full_chunk_count,
                    "shard_size": args.shard_size,
                },
            },
            {
                "name": "planned_capacity_covers_full_corpus",
                "passes": args.shard_count * args.shard_size >= full_chunk_count,
                "actual": {"planned_chunk_capacity": args.shard_count * args.shard_size, "full_chunk_count": full_chunk_count},
            },
            {
                "name": "planned_capacity_overrun_is_bounded",
                "passes": args.shard_count * args.shard_size - full_chunk_count < args.shard_size,
                "actual": {"overrun_chunks": args.shard_count * args.shard_size - full_chunk_count, "shard_size": args.shard_size},
            },
        ]
        return {
            "mode": "fresh_start",
            "cache_checkpoint": None,
            "readback_checkpoint": None,
            "cache_tree": cache_tree,
            "readback_tree": readback_tree,
            "expected_next_shard": actual_next_shard,
            "expected_total_shards": expected_total_shards,
            "expected_planned_chunk_capacity": expected_planned_chunk_capacity,
            "checks": checks,
            "passes": all(check["passes"] is True for check in checks),
        }

    cache_payload = read_json(cache_checkpoint)
    readback_payload = read_json(readback_checkpoint)
    progress = cache_payload.get("progress") or {}
    completed_shards = int(progress.get("completed_shards") or 0)
    actual_next_shard = expected_shard_id(completed_shards, args.shard_size)
    checks = [
        {"name": "cache_checkpoint_present", "passes": cache_checkpoint.is_file(), "actual": cache_state},
        {"name": "readback_checkpoint_present", "passes": readback_checkpoint.is_file(), "actual": readback_state},
        {"name": "checkpoint_hash_equal", "passes": cache_state.get("sha256") == readback_state.get("sha256"), "actual": {"cache": cache_state, "readback": readback_state}},
        {"name": "cache_root_not_stale_legacy_root", "passes": args.cache_root != args.stale_cache_root, "actual": str(args.cache_root)},
        {"name": "readback_root_not_stale_legacy_root", "passes": args.run_readback_root != args.stale_run_readback_root, "actual": str(args.run_readback_root)},
        {"name": "checkpoint_task", "passes": cache_payload.get("task_id") == "TASK-PY-DATA-016", "actual": cache_payload.get("task_id")},
        {"name": "checkpoint_passes", "passes": cache_payload.get("passes") is True, "actual": cache_payload.get("passes")},
        {
            "name": "checkpoint_next_shard_expected",
            "passes": (progress.get("next_shard") or {}).get("shard_id") == actual_next_shard
            and (args.expected_next_shard == "auto" or (progress.get("next_shard") or {}).get("shard_id") == args.expected_next_shard),
            "actual": {
                "checkpoint_next_shard": progress.get("next_shard"),
                "computed_next_shard": actual_next_shard,
                "expected_next_shard_arg": args.expected_next_shard,
            },
        },
        {
            "name": "shard_count_matches_full_chunk_count",
            "passes": args.shard_count == expected_total_shards,
            "actual": {
                "arg_shard_count": args.shard_count,
                "expected_shard_count": expected_total_shards,
                "full_chunk_count": full_chunk_count,
                "shard_size": args.shard_size,
            },
        },
        {
            "name": "checkpoint_total_shards_matches_relaunch",
            "passes": progress.get("total_shards") == args.shard_count,
            "actual": {"checkpoint_total_shards": progress.get("total_shards"), "arg_shard_count": args.shard_count},
        },
        {
            "name": "checkpoint_planned_capacity_matches_full_corpus",
            "passes": progress.get("planned_chunk_capacity") == expected_planned_chunk_capacity,
            "actual": {
                "checkpoint_planned_chunk_capacity": progress.get("planned_chunk_capacity"),
                "expected_planned_chunk_capacity": expected_planned_chunk_capacity,
            },
        },
        {
            "name": "checkpoint_mirror_equal",
            "passes": cache_payload == readback_payload,
            "actual": {"cache_path": str(cache_checkpoint), "readback_path": str(readback_checkpoint)},
        },
    ]
    return {
        "mode": "resume_existing",
        "cache_checkpoint": cache_payload,
        "readback_checkpoint": readback_payload,
        "expected_next_shard": actual_next_shard,
        "expected_total_shards": expected_total_shards,
        "expected_planned_chunk_capacity": expected_planned_chunk_capacity,
        "checks": checks,
        "passes": all(check["passes"] is True for check in checks),
    }


def invalidate_stale_roots(args: argparse.Namespace, started_at: str) -> dict[str, Any]:
    stale = {
        "cache_root": tree_state(args.stale_cache_root, hash_files=True),
        "run_readback_root": tree_state(args.stale_run_readback_root, hash_files=True),
        "hot_root": tree_state(args.stale_hot_root, hash_files=True),
    }
    new_paths = {
        "cache_root": str(args.cache_root),
        "run_readback_root": str(args.run_readback_root),
        "hot_root": str(args.hot_root),
    }
    old_paths = {
        "cache_root": str(args.stale_cache_root),
        "run_readback_root": str(args.stale_run_readback_root),
        "hot_root": str(args.stale_hot_root),
    }
    checks = [
        {
            "name": "new_cache_root_differs_from_stale_root",
            "passes": args.cache_root != args.stale_cache_root,
            "actual": {"new": str(args.cache_root), "stale": str(args.stale_cache_root)},
        },
        {
            "name": "new_readback_root_differs_from_stale_root",
            "passes": args.run_readback_root != args.stale_run_readback_root,
            "actual": {"new": str(args.run_readback_root), "stale": str(args.stale_run_readback_root)},
        },
        {
            "name": "new_hot_root_differs_from_stale_root",
            "passes": args.hot_root != args.stale_hot_root,
            "actual": {"new": str(args.hot_root), "stale": str(args.stale_hot_root)},
        },
        {
            "name": "stale_root_manifested_if_present",
            "passes": True,
            "actual": {name: state.get("file_count") for name, state in stale.items()},
        },
    ]
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "stale_partial_roots_invalidation_manifest",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "policy": "stale roots are historical evidence only and are not used by the post-249 relaunch command",
        "stale_roots": old_paths,
        "replacement_roots": new_paths,
        "stale_root_states": stale,
        "checks": checks,
        "passes": all(check["passes"] is True for check in checks),
    }
    path = args.readback_root / "stale_partial_roots_invalidation_manifest.json"
    dump_json(path, manifest)
    readback = read_json(path)
    require(
        readback.get("passes") is True,
        "FULL_RUN_RELAUNCH_STALE_ROOT_INVALIDATION_FAILED",
        str(path),
    )
    return {**readback, "manifest_state": file_state(path)}


def cleanup_hot_stage(args: argparse.Namespace, started_at: str) -> dict[str, Any]:
    hot_root = args.hot_root.resolve(strict=False)
    stage_root = hot_root / "packed-shard-stage"
    require(
        path_is_under(stage_root, Path("/home/cabdru/.cache/contextgraph")),
        "FULL_RUN_RELAUNCH_HOT_STAGE_PATH_UNSAFE",
        f"refusing to clean outside WSL ext4 cache: {stage_root}",
    )
    before = tree_state(stage_root, hash_files=True)
    pre_manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "hot_stage_pre_cleanup_manifest",
        "created_at_utc": utc_now(),
        "started_at_utc": started_at,
        "stage_root": str(stage_root),
        "before_state": before,
        "action": "manifest_written_before_cleanup",
    }
    pre_manifest_path = args.readback_root / "hot_stage_pre_cleanup_manifest.json"
    dump_json(pre_manifest_path, pre_manifest)
    pre_manifest_readback = read_json(pre_manifest_path)
    require(
        pre_manifest_readback.get("before_state", {}).get("path") == str(stage_root),
        "FULL_RUN_RELAUNCH_PRE_CLEANUP_MANIFEST_READBACK_FAILED",
        str(pre_manifest_path),
    )

    removed: list[dict[str, Any]] = []
    if stage_root.exists():
        for child in sorted(stage_root.iterdir()):
            row = {"path": str(child), "name": child.name, "was_dir": child.is_dir(), "was_file": child.is_file()}
            if child.is_dir():
                shutil.rmtree(child)
            elif child.is_file():
                child.unlink()
            else:
                raise FullRunRelaunchGateError(f"FULL_RUN_RELAUNCH_UNSUPPORTED_HOT_STAGE_ENTRY: {child}")
            removed.append(row)
    else:
        stage_root.mkdir(parents=True, exist_ok=True)

    after = tree_state(stage_root, hash_files=True)
    cleanup = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "hot_stage_cleanup_manifest",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "stage_root": str(stage_root),
        "pre_cleanup_manifest": str(pre_manifest_path),
        "pre_cleanup_manifest_sha256": sha256_file(pre_manifest_path),
        "before_state": before,
        "removed_entries": removed,
        "after_state": after,
        "passes": after.get("exists") is True and after.get("file_count") == 0 and after.get("children") == [],
    }
    cleanup_path = args.readback_root / "hot_stage_cleanup_manifest.json"
    dump_json(cleanup_path, cleanup)
    cleanup_readback = read_json(cleanup_path)
    require(
        cleanup_readback.get("passes") is True,
        "FULL_RUN_RELAUNCH_HOT_STAGE_CLEANUP_FAILED",
        str(cleanup_path),
    )
    return {
        **cleanup,
        "pre_cleanup_manifest_state": file_state(pre_manifest_path),
        "cleanup_manifest_state": file_state(cleanup_path),
    }


def edge_case_audit(args: argparse.Namespace, cleanup_result: dict[str, Any]) -> list[dict[str, Any]]:
    missing = validate_required_artifact(
        "synthetic_missing_required_artifact",
        0,
        "TASK-PY-DATA-000",
        args.readback_root / "edge_inputs" / "missing_required_artifact.json",
    )
    windows_mount_rejected = not path_is_under(Path("/mnt/c/contextgraph/hot").resolve(strict=False), Path("/home/cabdru/.cache/contextgraph"))
    hot_stage_after = tree_state(args.hot_root / "packed-shard-stage", hash_files=True)
    return [
        {
            "name": "missing_required_artifact_fails_closed",
            "input": missing["path"],
            "before_state": {"path": missing["path"], "exists": False},
            "after_state": missing,
            "expected": "FULL_RUN_RELAUNCH_REQUIRED_ARTIFACT_MISSING",
            "passes": missing.get("passes") is False
            and missing.get("error_code") == "FULL_RUN_RELAUNCH_REQUIRED_ARTIFACT_MISSING",
        },
        {
            "name": "windows_mount_hot_root_rejected",
            "input": "/mnt/c/contextgraph/hot",
            "before_state": {"candidate_hot_root": "/mnt/c/contextgraph/hot"},
            "after_state": {"accepted_as_hot_root": not windows_mount_rejected},
            "expected": "hot staging must remain under WSL ext4 cache, not /mnt/c",
            "passes": windows_mount_rejected,
        },
        {
            "name": "interrupted_hot_stage_removed",
            "input": cleanup_result["stage_root"],
            "before_state": {
                "file_count": cleanup_result.get("before_state", {}).get("file_count"),
                "byte_count": cleanup_result.get("before_state", {}).get("byte_count"),
                "children": cleanup_result.get("before_state", {}).get("children"),
            },
            "after_state": hot_stage_after,
            "expected": "stage root exists with zero children after cleanup",
            "passes": hot_stage_after.get("exists") is True
            and hot_stage_after.get("file_count") == 0
            and hot_stage_after.get("children") == [],
        },
    ]


def build_readback(args: argparse.Namespace) -> dict[str, Any]:
    started = utc_now()
    if args.clean_output and args.readback_root.exists():
        shutil.rmtree(args.readback_root)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    child_artifacts = [
        validate_required_artifact("smoke_manifest", 251, "TASK-PY-DATA-060", args.smoke_manifest_readback, expected_chunks_jsonl=args.chunks_jsonl),
        validate_required_artifact("smoke_forward_cache", 252, "TASK-PY-DATA-061", args.smoke_forward_readback),
        validate_required_artifact("smoke_intelligence_matrices", 253, "TASK-PY-DATA-062", args.smoke_intelligence_readback),
        validate_required_artifact("smoke_storage_design", 254, "TASK-PY-DATA-063", args.smoke_storage_design_readback, expected_chunks_jsonl=args.chunks_jsonl),
        validate_required_artifact("full_ast_corpus", 249, "TASK-PY-DATA-058", args.full_ast_corpus_readback, expected_chunks_jsonl=args.chunks_jsonl),
    ]
    storage_design = validate_storage_design(args)
    storage_estimates = storage_design.get("storage_estimates") or {}
    full_chunk_count = int(storage_estimates.get("full_chunk_count") or 0)
    checkpoint = validate_run_state(args, full_chunk_count=full_chunk_count)
    stale_invalidation = invalidate_stale_roots(args, started)
    cleanup = cleanup_hot_stage(args, started)
    run_command = relaunch_command(args)
    preflight_command = relaunch_command(args, mode="preflight-resume")
    relaunch_policy = validate_relaunch_policy(args, run_command)
    edge_cases = edge_case_audit(args, cleanup)

    passes = (
        all(item.get("passes") is True for item in child_artifacts)
        and storage_design.get("passes") is True
        and checkpoint.get("passes") is True
        and stale_invalidation.get("passes") is True
        and cleanup.get("passes") is True
        and relaunch_policy.get("passes") is True
        and all(item.get("passes") is True for item in edge_cases)
    )
    readback_path = args.readback_root / "full_run_relaunch_gate_readback.json"
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "full_run_relaunch_gate_readback",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "ready_for_full_run_preflight": passes,
        "relaunch_started": False,
        "source_of_truth": {
            "readback": str(readback_path),
            "readback_root": str(args.readback_root),
            "hot_root": str(args.hot_root),
            "hot_stage_root": str(args.hot_root / "packed-shard-stage"),
            "cache_root": str(args.cache_root),
            "run_readback_root": str(args.run_readback_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "full_ast_corpus_readback": str(args.full_ast_corpus_readback),
            "stale_partial_roots_invalidation_manifest": str(args.readback_root / "stale_partial_roots_invalidation_manifest.json"),
            "hot_stage_pre_cleanup_manifest": str(args.readback_root / "hot_stage_pre_cleanup_manifest.json"),
            "hot_stage_cleanup_manifest": str(args.readback_root / "hot_stage_cleanup_manifest.json"),
        },
        "disk_snapshot": {
            "wsl_ext4_root": disk_snapshot(Path("/")),
            "windows_c_mount_forbidden": {"policy": "forbidden", "disk_probe": False},
            "aiwonder_archive": disk_snapshot(Path("/zfs/archive")),
        },
        "child_smoke_artifacts": child_artifacts,
        "storage_design": storage_design,
        "checkpoint_readback": {
            "passes": checkpoint["passes"],
            "mode": checkpoint["mode"],
            "checks": checkpoint["checks"],
            "expected_next_shard": checkpoint["expected_next_shard"],
            "expected_total_shards": checkpoint["expected_total_shards"],
            "expected_planned_chunk_capacity": checkpoint["expected_planned_chunk_capacity"],
            "progress": (checkpoint["cache_checkpoint"] or {}).get("progress"),
            "summary": (checkpoint["cache_checkpoint"] or {}).get("summary"),
        },
        "readback": {
            "full_chunks_state": file_state(args.chunks_jsonl),
        },
        "stale_partial_roots_invalidation": stale_invalidation,
        "hot_stage_cleanup": cleanup,
        "companion_artifacts_before_readback_write": artifact_hashes(args.readback_root),
        "relaunch_policy": relaunch_policy,
        "commands": {
            "preflight_resume": preflight_command,
            "full_relaunch": run_command,
        },
        "edge_case_results": edge_cases,
    }
    dump_json(readback_path, readback)
    readback = read_json(readback_path)
    require(readback.get("passes") is True, "FULL_RUN_RELAUNCH_READBACK_FAILED", str(readback_path))
    return {
        **readback,
        "artifact_hashes": artifact_hashes(args.readback_root),
        "readback_sha256": sha256_file(readback_path),
    }


def command_full_run_relaunch_gate(args: argparse.Namespace) -> int:
    readback = build_readback(args)
    print(json.dumps(readback, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--cache-root", type=Path, default=DEFAULT_CACHE_ROOT)
    parser.add_argument("--run-readback-root", type=Path, default=DEFAULT_RUN_READBACK_ROOT)
    parser.add_argument("--hot-root", type=Path, default=DEFAULT_HOT_ROOT)
    parser.add_argument("--stale-cache-root", type=Path, default=DEFAULT_STALE_CACHE_ROOT)
    parser.add_argument("--stale-run-readback-root", type=Path, default=DEFAULT_STALE_RUN_READBACK_ROOT)
    parser.add_argument("--stale-hot-root", type=Path, default=DEFAULT_STALE_HOT_ROOT)
    parser.add_argument("--smoke-manifest-readback", type=Path, default=DEFAULT_SMOKE_MANIFEST_READBACK)
    parser.add_argument("--smoke-forward-readback", type=Path, default=DEFAULT_SMOKE_FORWARD_READBACK)
    parser.add_argument("--smoke-intelligence-readback", type=Path, default=DEFAULT_SMOKE_INTELLIGENCE_READBACK)
    parser.add_argument("--smoke-storage-design-readback", type=Path, default=DEFAULT_SMOKE_STORAGE_DESIGN_READBACK)
    parser.add_argument("--storage-design-go-no-go", type=Path, default=DEFAULT_STORAGE_DESIGN_GO_NO_GO)
    parser.add_argument("--storage-policy", type=Path, default=DEFAULT_STORAGE_POLICY)
    parser.add_argument("--storage-estimates", type=Path, default=DEFAULT_STORAGE_ESTIMATES)
    parser.add_argument("--full-ast-corpus-readback", type=Path, default=DEFAULT_FULL_AST_CORPUS_READBACK)
    parser.add_argument("--shard-size", type=int, default=128)
    parser.add_argument("--shard-count", type=int, default=3124)
    parser.add_argument("--active-embedder-count", type=int, default=12)
    parser.add_argument("--true-batch-size", type=int, default=8)
    parser.add_argument("--io-workers", type=int, default=2)
    parser.add_argument("--cpu-workers", type=int, default=30)
    parser.add_argument("--gpu-workers", type=int, default=1)
    parser.add_argument("--jsonl-sync-every-rows", type=int, default=512)
    parser.add_argument("--checkpoint-every-shards", type=int, default=1)
    parser.add_argument("--cargo-runner", default=DEFAULT_CARGO_RUNNER)
    parser.add_argument("--expected-next-shard", default="auto")
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_full_run_relaunch_gate(args)
    except FullRunRelaunchGateError as exc:
        print(f"FULL_RUN_RELAUNCH_GATE_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
