#!/usr/bin/env python3
"""Fail-closed #255 gate before relaunching the full Python forward-cache run."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-064"
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-064-post249-full-run-relaunch-gate-fsv")
DEFAULT_CHUNKS_JSONL = Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_CACHE_ROOT = Path("/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-20260514")
DEFAULT_RUN_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-016-full-20260514-fsv")
DEFAULT_HOT_ROOT = Path("/home/cabdru/.cache/contextgraph/task-py-data-016-full-20260514")
DEFAULT_SMOKE_MANIFEST_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-060-post249-smoke-manifest-fsv/smoke_manifest_fsv.json")
DEFAULT_SMOKE_FORWARD_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json"
)
DEFAULT_SMOKE_INTELLIGENCE_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"
)
DEFAULT_SMOKE_STORAGE_DESIGN_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-063-post249-smoke-storage-design-fsv/smoke_storage_design_fsv.json"
)
DEFAULT_STORAGE_DESIGN_GO_NO_GO = Path(
    "/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/go_no_go.json"
)
DEFAULT_STORAGE_POLICY = Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/storage_policy.json")
DEFAULT_STORAGE_ESTIMATES = Path(
    "/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/design/full_scale_storage_estimates.json"
)
DEFAULT_FULL_AST_CORPUS_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json")
DEFAULT_CARGO_RUNNER = "target/release/examples/python_forward_cache_fsv"


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
        "--fsv-root",
        str(args.run_fsv_root),
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
        "--cargo-runner",
        args.cargo_runner,
        "--require-smoke-gate",
        "--full-ast-corpus-fsv",
        str(args.full_ast_corpus_fsv),
    ]
    if mode:
        command.extend(["--mode", mode])
    return command


def validate_relaunch_policy(args: argparse.Namespace, command: list[str]) -> dict[str, Any]:
    hot_root = args.hot_root.resolve(strict=False)
    cache_root = args.cache_root.resolve(strict=False)
    fsv_root = args.run_fsv_root.resolve(strict=False)
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
            "name": "durable_cache_on_d_drive",
            "passes": path_is_under(cache_root, Path("/mnt/d/contextgraph")),
            "actual": str(cache_root),
        },
        {
            "name": "fsv_on_d_drive",
            "passes": path_is_under(fsv_root, Path("/mnt/d/contextgraph")),
            "actual": str(fsv_root),
        },
        {"name": "packed_store_required", "passes": "--require-packed-store" in command_set, "actual": command},
        {"name": "hot_stage_enabled", "passes": "--stage-packed-shards-on-hot" in command_set, "actual": command},
        {"name": "resume_completed_enabled", "passes": "--resume-completed-shards" in command_set, "actual": command},
        {"name": "partial_retry_enabled", "passes": "--retry-failed-partial-shards" in command_set, "actual": command},
        {"name": "smoke_gate_required", "passes": "--require-smoke-gate" in command_set, "actual": command},
        {"name": "full_ast_corpus_gate_required", "passes": "--full-ast-corpus-fsv" in command_set, "actual": command},
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
            "name": "storage_estimate_fits_d_drive",
            "passes": int(estimates.get("estimated_total_design_bytes") or 0) < int(estimates.get("d_drive_free_bytes") or 0),
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


def validate_checkpoint(args: argparse.Namespace) -> dict[str, Any]:
    cache_checkpoint = args.cache_root / "python_forward_cache_checkpoint.json"
    fsv_checkpoint = args.run_fsv_root / "python_forward_cache_checkpoint.json"
    cache_state = file_state(cache_checkpoint)
    fsv_state = file_state(fsv_checkpoint)
    cache_payload = read_json(cache_checkpoint)
    fsv_payload = read_json(fsv_checkpoint)
    checks = [
        {"name": "cache_checkpoint_present", "passes": cache_checkpoint.is_file(), "actual": cache_state},
        {"name": "fsv_checkpoint_present", "passes": fsv_checkpoint.is_file(), "actual": fsv_state},
        {"name": "checkpoint_hash_equal", "passes": cache_state.get("sha256") == fsv_state.get("sha256"), "actual": {"cache": cache_state, "fsv": fsv_state}},
        {"name": "checkpoint_task", "passes": cache_payload.get("task_id") == "TASK-PY-DATA-016", "actual": cache_payload.get("task_id")},
        {"name": "checkpoint_passes", "passes": cache_payload.get("passes") is True, "actual": cache_payload.get("passes")},
        {
            "name": "checkpoint_next_shard_expected",
            "passes": (cache_payload.get("progress") or {}).get("next_shard", {}).get("shard_id") == args.expected_next_shard,
            "actual": (cache_payload.get("progress") or {}).get("next_shard"),
        },
        {
            "name": "checkpoint_mirror_equal",
            "passes": cache_payload == fsv_payload,
            "actual": {"cache_path": str(cache_checkpoint), "fsv_path": str(fsv_checkpoint)},
        },
    ]
    return {
        "cache_checkpoint": cache_payload,
        "fsv_checkpoint": fsv_payload,
        "checks": checks,
        "passes": all(check["passes"] is True for check in checks),
    }


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
    pre_manifest_path = args.fsv_root / "hot_stage_pre_cleanup_manifest.json"
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
    cleanup_path = args.fsv_root / "hot_stage_cleanup_manifest.json"
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
        args.fsv_root / "edge_inputs" / "missing_required_artifact.json",
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


def build_fsv(args: argparse.Namespace) -> dict[str, Any]:
    started = utc_now()
    if args.clean_output and args.fsv_root.exists():
        shutil.rmtree(args.fsv_root)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    child_artifacts = [
        validate_required_artifact("smoke_manifest", 251, "TASK-PY-DATA-060", args.smoke_manifest_fsv, expected_chunks_jsonl=args.chunks_jsonl),
        validate_required_artifact("smoke_forward_cache", 252, "TASK-PY-DATA-061", args.smoke_forward_fsv),
        validate_required_artifact("smoke_intelligence_matrices", 253, "TASK-PY-DATA-062", args.smoke_intelligence_fsv),
        validate_required_artifact("smoke_storage_design", 254, "TASK-PY-DATA-063", args.smoke_storage_design_fsv, expected_chunks_jsonl=args.chunks_jsonl),
        validate_required_artifact("full_ast_corpus", 249, "TASK-PY-DATA-058", args.full_ast_corpus_fsv, expected_chunks_jsonl=args.chunks_jsonl),
    ]
    storage_design = validate_storage_design(args)
    checkpoint = validate_checkpoint(args)
    cleanup = cleanup_hot_stage(args, started)
    run_command = relaunch_command(args)
    preflight_command = relaunch_command(args, mode="preflight-resume")
    relaunch_policy = validate_relaunch_policy(args, run_command)
    edge_cases = edge_case_audit(args, cleanup)

    passes = (
        all(item.get("passes") is True for item in child_artifacts)
        and storage_design.get("passes") is True
        and checkpoint.get("passes") is True
        and cleanup.get("passes") is True
        and relaunch_policy.get("passes") is True
        and all(item.get("passes") is True for item in edge_cases)
    )
    fsv_path = args.fsv_root / "full_run_relaunch_gate_fsv.json"
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "full_run_relaunch_gate_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "ready_for_full_run_preflight": passes,
        "relaunch_started": False,
        "source_of_truth": {
            "fsv": str(fsv_path),
            "fsv_root": str(args.fsv_root),
            "hot_root": str(args.hot_root),
            "hot_stage_root": str(args.hot_root / "packed-shard-stage"),
            "cache_root": str(args.cache_root),
            "run_fsv_root": str(args.run_fsv_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "full_ast_corpus_fsv": str(args.full_ast_corpus_fsv),
            "hot_stage_pre_cleanup_manifest": str(args.fsv_root / "hot_stage_pre_cleanup_manifest.json"),
            "hot_stage_cleanup_manifest": str(args.fsv_root / "hot_stage_cleanup_manifest.json"),
        },
        "disk_snapshot": {
            "wsl_ext4_root": disk_snapshot(Path("/")),
            "windows_c_mount_forbidden": disk_snapshot(Path("/mnt/c")),
            "d_drive": disk_snapshot(Path("/mnt/d")),
        },
        "child_smoke_artifacts": child_artifacts,
        "storage_design": storage_design,
        "checkpoint_readback": {
            "passes": checkpoint["passes"],
            "checks": checkpoint["checks"],
            "progress": checkpoint["cache_checkpoint"].get("progress"),
            "summary": checkpoint["cache_checkpoint"].get("summary"),
        },
        "readback": {
            "full_chunks_state": file_state(args.chunks_jsonl),
        },
        "hot_stage_cleanup": cleanup,
        "relaunch_policy": relaunch_policy,
        "commands": {
            "preflight_resume": preflight_command,
            "full_relaunch": run_command,
        },
        "edge_case_results": edge_cases,
    }
    dump_json(fsv_path, fsv)
    readback = read_json(fsv_path)
    require(readback.get("passes") is True, "FULL_RUN_RELAUNCH_FSV_FAILED", str(fsv_path))
    return {
        **readback,
        "artifact_hashes": artifact_hashes(args.fsv_root),
        "fsv_sha256": sha256_file(fsv_path),
    }


def command_full_run_relaunch_gate(args: argparse.Namespace) -> int:
    fsv = build_fsv(args)
    print(json.dumps(fsv, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--cache-root", type=Path, default=DEFAULT_CACHE_ROOT)
    parser.add_argument("--run-fsv-root", type=Path, default=DEFAULT_RUN_FSV_ROOT)
    parser.add_argument("--hot-root", type=Path, default=DEFAULT_HOT_ROOT)
    parser.add_argument("--smoke-manifest-fsv", type=Path, default=DEFAULT_SMOKE_MANIFEST_FSV)
    parser.add_argument("--smoke-forward-fsv", type=Path, default=DEFAULT_SMOKE_FORWARD_FSV)
    parser.add_argument("--smoke-intelligence-fsv", type=Path, default=DEFAULT_SMOKE_INTELLIGENCE_FSV)
    parser.add_argument("--smoke-storage-design-fsv", type=Path, default=DEFAULT_SMOKE_STORAGE_DESIGN_FSV)
    parser.add_argument("--storage-design-go-no-go", type=Path, default=DEFAULT_STORAGE_DESIGN_GO_NO_GO)
    parser.add_argument("--storage-policy", type=Path, default=DEFAULT_STORAGE_POLICY)
    parser.add_argument("--storage-estimates", type=Path, default=DEFAULT_STORAGE_ESTIMATES)
    parser.add_argument("--full-ast-corpus-fsv", type=Path, default=DEFAULT_FULL_AST_CORPUS_FSV)
    parser.add_argument("--shard-size", type=int, default=128)
    parser.add_argument("--shard-count", type=int, default=2933)
    parser.add_argument("--active-embedder-count", type=int, default=12)
    parser.add_argument("--true-batch-size", type=int, default=8)
    parser.add_argument("--io-workers", type=int, default=2)
    parser.add_argument("--cpu-workers", type=int, default=30)
    parser.add_argument("--gpu-workers", type=int, default=1)
    parser.add_argument("--jsonl-sync-every-rows", type=int, default=512)
    parser.add_argument("--checkpoint-every-shards", type=int, default=1)
    parser.add_argument("--cargo-runner", default=DEFAULT_CARGO_RUNNER)
    parser.add_argument("--expected-next-shard", default="shard-000028-offset-000003584")
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
