#!/usr/bin/env python3
"""aiwonder capability/readiness FSV for ContextGraph.

This audit is intentionally read-heavy. It verifies that the host capabilities
required by the scheduler profile are present and that the canonical
ContextGraph paths resolve to the expected storage classes.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


TASK_ID = "TASK-AIWONDER-017"
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/task-aiwonder-017-capability-audit")
DEFAULT_REPO_ROOT = Path("/zfs/hot/contextgraph/repo")
DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_RUNNER = Path("/zfs/hot/contextgraph/repo/target/release/examples/python_forward_cache_fsv")
DEFAULT_STORAGE_BLOCKER = "#372"

LEAPABLE_SERVICES = (
    "leapable-marketplace.service",
    "leapable-outbox-relay.service",
    "leapable-postgres-exporter.service",
    "leapable-pgbouncer-exporter.service",
    "leapable-tei-general.service",
    "leapable-tei-legal.service",
    "leapable-tei-reranker.service",
)


def utc_now() -> str:
    return dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def run(cmd: list[str], timeout: int = 30, cwd: Path | None = None, env: dict[str, str] | None = None) -> dict[str, Any]:
    started = time.monotonic()
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd) if cwd else None,
            env=merged_env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return {
            "command": cmd,
            "exitCode": proc.returncode,
            "durationSeconds": round(time.monotonic() - started, 3),
            "stdout": proc.stdout,
            "stderr": proc.stderr,
        }
    except subprocess.TimeoutExpired as exc:
        return {
            "command": cmd,
            "exitCode": 124,
            "durationSeconds": round(time.monotonic() - started, 3),
            "stdout": exc.stdout or "",
            "stderr": exc.stderr or f"TIMEOUT after {timeout}s",
        }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_checked(path: Path, payload: dict[str, Any]) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)
    readback = json.loads(path.read_text(encoding="utf-8"))
    if readback != payload:
        raise RuntimeError(f"JSON readback mismatch for {path}")
    return sha256_file(path)


def findmnt_probe(path: Path) -> dict[str, Any]:
    result = run(["findmnt", "-T", str(path), "-o", "SOURCE,FSTYPE,TARGET,OPTIONS", "-n"])
    parsed: dict[str, Any] = {"source": None, "fstype": None, "target": None, "options": None}
    if result["exitCode"] == 0 and result["stdout"].strip():
        parts = result["stdout"].strip().split(None, 3)
        if len(parts) >= 3:
            parsed = {
                "source": parts[0],
                "fstype": parts[1],
                "target": parts[2],
                "options": parts[3] if len(parts) > 3 else "",
            }
    return {"path": str(path), "exists": path.exists(), "findmnt": result, "parsed": parsed}


def df_probe(paths: list[Path]) -> dict[str, Any]:
    existing = [str(path) for path in paths if path.exists()]
    if not existing:
        return {"command": ["df"], "exitCode": 1, "stdout": "", "stderr": "no existing paths"}
    return run(["df", "-B1", "--output=source,size,used,avail,pcent,target", *existing])


def systemctl_is_active(service: str) -> dict[str, Any]:
    return run(["systemctl", "is-active", service], timeout=15)


def requirement(name: str, passes: bool, expected: Any, actual: Any, blocker: str | None = None) -> dict[str, Any]:
    row = {"name": name, "passes": bool(passes), "expected": expected, "actual": actual}
    if blocker and not passes:
        row["blocker"] = blocker
    return row


def docker_default_runtime(probe: dict[str, Any]) -> str:
    for line in probe["stdout"].splitlines():
        if line.startswith("DefaultRuntime="):
            return line.split("=", 1)[1].strip()
    return ""


def run_optional_smoke(args: argparse.Namespace, storage_ready: bool, run_id: str) -> dict[str, Any]:
    if not args.run_smoke:
        return {"status": "skipped", "reason": "run_smoke not requested"}
    if not storage_ready:
        return {
            "status": "blocked",
            "reason": "canonical ContextGraph roots are not mounted on the expected ZFS pools",
            "blocker": args.storage_blocker_issue,
        }

    smoke_cache_root = args.cache_root / run_id
    smoke_fsv_root = args.output_root / run_id / "scheduler-smoke-fsv"
    smoke_hot_root = args.hot_root / run_id
    smoke_env = {
        "CG_MEJEPA_DURABLE_ROOT": "/zfs/archive/contextgraph",
        "CG_MEJEPA_DISK_TARGETS": "/,/zfs/hot/contextgraph,/zfs/archive/contextgraph,/hotpool,/archive",
        "CG_MEJEPA_HOT_DISK_TARGET": "/zfs/hot/contextgraph",
        "CG_MEJEPA_DURABLE_DISK_TARGET": "/zfs/archive/contextgraph",
        "CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS": str(args.parallel_embedders),
        "CONTEXTGRAPH_MEJEPA_MODELS_CONFIG": "/zfs/hot/contextgraph/models/mejepa_models_config.toml",
        "CG_MEJEPA_PY_INSTRUMENT_REGISTRY_JSONL": (
            "/zfs/hot/contextgraph/corpus/python-frozen-instrument-registry-v1/"
            "python_frozen_instrument_records.jsonl"
        ),
    }
    command = [
        str(args.python),
        str(args.repo_root / "scripts/swebench_lite_ops/python_forward_shard_scheduler.py"),
        "--task-id",
        f"{TASK_ID}-SMOKE",
        "--chunks-jsonl",
        str(args.chunks_jsonl),
        "--cache-root",
        str(smoke_cache_root),
        "--fsv-root",
        str(smoke_fsv_root),
        "--hot-root",
        str(smoke_hot_root),
        "--shard-size",
        str(args.smoke_shard_size),
        "--shard-count",
        "1",
        "--max-source-bytes",
        "32768",
        "--true-batch-size",
        "1",
        "--gpu-vram-guard-bytes",
        "30064771072",
        "--io-workers",
        "2",
        "--cpu-workers",
        str(args.cpu_workers),
        "--gpu-workers",
        "1",
        "--cache-store-layout",
        "packed",
        "--require-packed-store",
        "--stage-packed-shards-on-hot",
        "--active-embedder-count",
        "12",
        "--estimated-record-bytes",
        "16384",
        "--hot-stage-budget-bytes",
        "1073741824",
        "--fsv-edge-copy-budget-bytes",
        "268435456",
        "--isolate-embedder-processes",
        "--checkpoint-every-shards",
        "1",
        "--cargo-runner",
        str(args.cargo_runner),
    ]
    result = run(command, timeout=args.smoke_timeout_seconds, cwd=args.repo_root, env=smoke_env)
    fsv_path = smoke_fsv_root / "python_forward_shard_scheduler_fsv.json"
    fsv = None
    if fsv_path.is_file():
        fsv = json.loads(fsv_path.read_text(encoding="utf-8"))
    return {
        "status": "passed" if result["exitCode"] == 0 and fsv and fsv.get("passes") is True else "failed",
        "commandResult": result,
        "schedulerEnv": smoke_env,
        "cacheRoot": str(smoke_cache_root),
        "hotRoot": str(smoke_hot_root),
        "fsvPath": str(fsv_path),
        "fsvSha256": sha256_file(fsv_path) if fsv_path.is_file() else None,
        "fsvSummary": None if not fsv else {
            "passes": fsv.get("passes"),
            "aggregateSummary": fsv.get("aggregate_summary"),
            "runnerControls": fsv.get("runner_controls"),
            "productionRootGuard": fsv.get("host_preflight", {}).get("production_root_guard"),
        },
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Write aiwonder capability/readiness FSV evidence.")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--repo-root", type=Path, default=DEFAULT_REPO_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--cargo-runner", type=Path, default=DEFAULT_RUNNER)
    parser.add_argument("--python", type=Path, default=Path(sys.executable))
    parser.add_argument("--cache-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/task-aiwonder-017-smoke"))
    parser.add_argument("--hot-root", type=Path, default=Path("/zfs/hot/contextgraph/runtime/task-aiwonder-017-smoke"))
    parser.add_argument("--run-id", default="")
    parser.add_argument("--run-smoke", action="store_true")
    parser.add_argument("--smoke-shard-size", type=int, default=1)
    parser.add_argument("--smoke-timeout-seconds", type=int, default=1800)
    parser.add_argument("--parallel-embedders", type=int, default=6)
    parser.add_argument("--cpu-workers", type=int, default=28)
    parser.add_argument("--storage-blocker-issue", default=DEFAULT_STORAGE_BLOCKER)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    run_id = args.run_id or "run-" + dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    output_dir = args.output_root / run_id
    artifact_path = output_dir / "aiwonder_capability_readiness_fsv.json"

    paths = [
        Path("/zfs/archive/contextgraph"),
        Path("/zfs/hot/contextgraph"),
        Path("/archive"),
        Path("/hotpool"),
        Path("/zfs/hot/tmpwork"),
        args.output_root,
    ]
    filesystem = {str(path): findmnt_probe(path) for path in paths}
    host_commands = {
        "hostname": run(["hostname"]),
        "uname": run(["uname", "-a"]),
        "nvidia_smi_query": run(
            [
                "nvidia-smi",
                "--query-gpu=name,driver_version,memory.total,memory.used,utilization.gpu",
                "--format=csv,noheader,nounits",
            ]
        ),
        "mps_service": systemctl_is_active("nvidia-cuda-mps-control.service"),
        "mps_show": run(
            [
                "systemctl",
                "show",
                "nvidia-cuda-mps-control.service",
                "-p",
                "ActiveState",
                "-p",
                "SubState",
                "-p",
                "LoadState",
                "-p",
                "FragmentPath",
            ]
        ),
        "docker_info_default_runtime": run(["docker", "info", "--format", "DefaultRuntime={{.DefaultRuntime}}"]),
        "docker_runtimes": run(["docker", "info", "--format", "Runtimes={{json .Runtimes}}"]),
        "docker_ps_context": run(["docker", "ps", "--format", "{{.Names}} {{.Status}}"]),
        "zpool_list": run(["zpool", "list", "-H", "-o", "name,size,alloc,free,cap,health"]),
        "zfs_list": run(["zfs", "list", "-H", "-o", "name,mountpoint,avail,used"]),
        "df": df_probe(paths),
        "nproc": run(["nproc"]),
    }
    service_states = {service: systemctl_is_active(service) for service in LEAPABLE_SERVICES}

    archive_ctx = filesystem["/zfs/archive/contextgraph"]["parsed"]
    hot_ctx = filesystem["/zfs/hot/contextgraph"]["parsed"]
    archive_pool = filesystem["/archive"]["parsed"]
    hot_pool = filesystem["/hotpool"]["parsed"]
    docker_runtime = docker_default_runtime(host_commands["docker_info_default_runtime"])
    nvidia_ok = host_commands["nvidia_smi_query"]["exitCode"] == 0 and "RTX 5090" in host_commands["nvidia_smi_query"]["stdout"]
    mps_ok = host_commands["mps_service"]["stdout"].strip() == "active"
    leapable_ok = all(state["stdout"].strip() == "active" for state in service_states.values())
    zfs_archive_ok = archive_pool["fstype"] == "zfs" and archive_pool["source"] == "archive"
    zfs_hot_ok = hot_pool["fstype"] == "zfs" and hot_pool["source"] == "hotpool"
    context_archive_on_zfs = archive_ctx["fstype"] == "zfs" and archive_ctx["source"].startswith("archive")
    context_hot_on_zfs = hot_ctx["fstype"] == "zfs" and hot_ctx["source"].startswith("hotpool")

    requirements = [
        requirement("gpu_rtx_5090_visible", nvidia_ok, "nvidia-smi sees RTX 5090", host_commands["nvidia_smi_query"]["stdout"].strip()),
        requirement("mps_service_active", mps_ok, "active", host_commands["mps_service"]["stdout"].strip()),
        requirement("docker_default_runtime_nvidia", docker_runtime == "nvidia", "nvidia", docker_runtime),
        requirement("archive_pool_online", zfs_archive_ok, "archive zfs mounted at /archive", archive_pool),
        requirement("hotpool_online", zfs_hot_ok, "hotpool zfs mounted at /hotpool", hot_pool),
        requirement(
            "contextgraph_archive_root_on_archive_zfs",
            context_archive_on_zfs,
            "findmnt /zfs/archive/contextgraph reports archive-backed zfs",
            archive_ctx,
            args.storage_blocker_issue,
        ),
        requirement(
            "contextgraph_hot_root_on_hotpool_zfs",
            context_hot_on_zfs,
            "findmnt /zfs/hot/contextgraph reports hotpool-backed zfs",
            hot_ctx,
            args.storage_blocker_issue,
        ),
        requirement("leapable_services_active", leapable_ok, "all known resident leapable services active", {
            service: state["stdout"].strip() for service, state in service_states.items()
        }),
    ]

    storage_ready = context_archive_on_zfs and context_hot_on_zfs
    smoke = run_optional_smoke(args, storage_ready, run_id)
    smoke_ok = smoke["status"] == "passed"
    if args.run_smoke:
        requirements.append(
            requirement(
                "bounded_scheduler_smoke",
                smoke_ok,
                "one production-profile scheduler smoke passes",
                smoke["status"],
                args.storage_blocker_issue if smoke["status"] == "blocked" else None,
            )
        )

    readiness_passed = all(row["passes"] for row in requirements)
    payload = {
        "schemaVersion": 1,
        "taskId": TASK_ID,
        "artifactKind": "aiwonder_capability_readiness_fsv",
        "runId": run_id,
        "createdAtUtc": utc_now(),
        "sourceOfTruth": {
            "artifactPath": str(artifact_path),
            "canonicalArchiveRoot": "/zfs/archive/contextgraph",
            "canonicalHotRoot": "/zfs/hot/contextgraph",
            "archivePoolMount": "/archive",
            "hotpoolMount": "/hotpool",
        },
        "schedulerProfile": {
            "expectedEnv": {
                "CG_MEJEPA_DURABLE_ROOT": "/zfs/archive/contextgraph",
                "CG_MEJEPA_DISK_TARGETS": "/,/zfs/hot/contextgraph,/zfs/archive/contextgraph,/hotpool,/archive",
                "CG_MEJEPA_HOT_DISK_TARGET": "/zfs/hot/contextgraph",
                "CG_MEJEPA_DURABLE_DISK_TARGET": "/zfs/archive/contextgraph",
                "CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS": str(args.parallel_embedders),
            },
            "cpuWorkers": args.cpu_workers,
            "ioWorkers": 2,
            "gpuWorkers": 1,
            "gpuVramGuardBytes": 30064771072,
            "packedHotStageRequiredForParallel": True,
        },
        "filesystemReadback": filesystem,
        "hostCommands": host_commands,
        "serviceStates": service_states,
        "requirements": requirements,
        "smoke": smoke,
        "blockedBy": None if readiness_passed else sorted({row.get("blocker") for row in requirements if row.get("blocker")}),
        "allPassed": readiness_passed,
    }
    artifact_sha = write_json_checked(artifact_path, payload)
    print(json.dumps({"artifactPath": str(artifact_path), "artifactSha256": artifact_sha, "allPassed": readiness_passed}, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
