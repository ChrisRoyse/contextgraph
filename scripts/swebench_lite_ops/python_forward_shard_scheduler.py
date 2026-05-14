#!/usr/bin/env python3
"""Shard scheduler for Python ME-JEPA-Code forward-cache generation.

This script is intentionally small and fail-closed. It does not embed code
itself; it schedules the Rust forward-cache FSV runner over explicit chunk
windows, then aggregates the physical shard manifests it reads back from disk.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_CHUNKS_JSONL = Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_CACHE_ROOT = Path("/mnt/d/contextgraph/corpus/python-forward-cache-v1")
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-029-sharded-cuda-scheduler-fsv")
DEFAULT_HOT_ROOT = Path("/home/cabdru/.cache/contextgraph/python-forward-shard-scheduler")
DEFAULT_RUNNER_TIMEOUT_SECONDS = 3600
DEFAULT_CARGO_RUNNER = "target/release/examples/python_forward_cache_fsv"
SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-029"
SUPPORTED_TRUE_BATCH_SLUGS = {"e1", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14"}
ISOLATED_FORWARD_SLUGS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
DEFAULT_ACTIVE_EMBEDDER_COUNT = 12
DEFAULT_ESTIMATED_RECORD_BYTES = 16 * 1024
DEFAULT_HOT_STAGE_BUDGET_BYTES = 48 * 1024 * 1024 * 1024
DEFAULT_DURABLE_SLACK_BYTES = 2 * 1024 * 1024 * 1024
DEFAULT_FSV_EDGE_COPY_BUDGET_BYTES = 2 * 1024 * 1024 * 1024
MAX_D_DRIVE_IO_WORKERS = 8
MAX_GPU_WORKERS_PER_DEVICE = 1
SHARD_RUN_LOG_NAME_PATTERN = re.compile(r"^shard-\d{6}-offset-\d{9}\.json$")


class SchedulerError(RuntimeError):
    pass


@dataclass(frozen=True)
class ShardSpec:
    shard_id: str
    chunk_offset: int
    max_chunks: int
    cache_root: Path
    fsv_root: Path

    def manifest_path(self) -> Path:
        return self.cache_root / "python_forward_cache_manifest.json"

    def index_path(self) -> Path:
        return self.cache_root / "python_forward_cache_index.jsonl"

    def quarantine_path(self) -> Path:
        return self.cache_root / "python_forward_cache_quarantine.jsonl"

    def fsv_path(self) -> Path:
        return self.fsv_root / "python_forward_cache_fsv.json"


def parse_args() -> argparse.Namespace:
    default_cpu_workers = detected_cpu_worker_count()
    parser = argparse.ArgumentParser(
        description="Run and aggregate sharded Python forward-cache generation."
    )
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--cache-root", type=Path, default=DEFAULT_CACHE_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--hot-root", type=Path, default=DEFAULT_HOT_ROOT)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--start-offset", type=int, default=0)
    parser.add_argument("--shard-size", type=int, required=True)
    parser.add_argument("--shard-count", type=int, required=True)
    parser.add_argument("--max-source-bytes", type=int, default=32768)
    parser.add_argument("--true-batch-size", type=int, default=8)
    parser.add_argument(
        "--true-batch-max-padded-source-bytes",
        type=int,
        default=None,
        help="Optional global padded source-byte budget per true batch.",
    )
    parser.add_argument(
        "--embedder-batch-sizes",
        default="",
        help="Comma-separated per-embedder row caps, e.g. e1=128,e12=32,e14=8.",
    )
    parser.add_argument(
        "--embedder-max-padded-source-bytes",
        default="",
        help="Comma-separated per-embedder padded source-byte budgets, e.g. e1=4194304,e14=262144.",
    )
    parser.add_argument(
        "--gpu-vram-guard-bytes",
        type=int,
        default=32607 * 1024 * 1024,
        help="Fail shard runners before model forward if their batch VRAM plan exceeds this byte guard.",
    )
    parser.add_argument("--io-workers", type=int, default=2)
    parser.add_argument("--cpu-workers", type=int, default=default_cpu_workers)
    parser.add_argument("--gpu-workers", type=int, default=1)
    parser.add_argument("--jsonl-sync-every-rows", type=int, default=512)
    parser.add_argument("--runner-timeout-seconds", type=int, default=DEFAULT_RUNNER_TIMEOUT_SECONDS)
    parser.add_argument(
        "--cache-store-layout",
        choices=("packed", "legacy-files"),
        default="packed",
        help="Forward-cache store layout to request from shard runners.",
    )
    parser.add_argument(
        "--require-packed-store",
        action="store_true",
        help="Fail closed unless every shard manifest proves packed JSONL store readback.",
    )
    parser.add_argument(
        "--active-embedder-count",
        type=int,
        default=DEFAULT_ACTIVE_EMBEDDER_COUNT,
        help="Expected active content embedder count used for capacity projection before shard manifests exist.",
    )
    parser.add_argument(
        "--estimated-record-bytes",
        type=int,
        default=DEFAULT_ESTIMATED_RECORD_BYTES,
        help="Projected durable bytes per chunk/embedder output for pre-run D-drive capacity checks.",
    )
    parser.add_argument(
        "--projected-output-bytes",
        type=int,
        default=None,
        help="Override projected durable output bytes for capacity edge tests or full-run planning.",
    )
    parser.add_argument(
        "--hot-stage-budget-bytes",
        type=int,
        default=DEFAULT_HOT_STAGE_BUDGET_BYTES,
        help="Required free bytes on WSL ext4 hot storage before any writes.",
    )
    parser.add_argument(
        "--fsv-edge-copy-budget-bytes",
        type=int,
        default=DEFAULT_FSV_EDGE_COPY_BUDGET_BYTES,
        help="Required temporary D-drive bytes reserved for FSV edge/readback artifacts.",
    )
    parser.add_argument(
        "--nvidia-smi-command",
        default="nvidia-smi",
        help="Command used for required CUDA/GPU runtime detection.",
    )
    parser.add_argument(
        "--nvcc-command",
        default="nvcc",
        help="Command used to record CUDA compiler version when present.",
    )
    parser.add_argument(
        "--mode",
        choices=("run", "aggregate-only", "preflight-resume"),
        default="run",
        help=(
            "run executes shard runners first; aggregate-only only reads existing shard manifests; "
            "preflight-resume scans the existing checkpoint next-shard state and archives unsafe partials."
        ),
    )
    parser.add_argument(
        "--resume-completed-shards",
        action="store_true",
        help=(
            "In run mode, skip only shards whose manifest, FSV, telemetry, "
            "index/quarantine, and packed-store readback already pass. "
            "Partial or corrupt shard state still fails closed."
        ),
    )
    parser.add_argument(
        "--retry-failed-partial-shards",
        action="store_true",
        help=(
            "When used with --resume-completed-shards, archive a failed or partial "
            "shard cache/FSV/run-log under fsv_root/failed_partial_shards before "
            "retrying that shard. Passed shard manifests are still the only skip condition."
        ),
    )
    parser.add_argument(
        "--isolate-embedder-processes",
        action="store_true",
        help=(
            "Run each active embedder in a separate Rust runner process, then run a final "
            "all-embedder readback pass. This avoids cumulative CUDA model residency inside "
            "one shard process while still requiring the final shard manifest/FSV to pass."
        ),
    )
    parser.add_argument(
        "--checkpoint-every-shards",
        type=int,
        default=1,
        help=(
            "Write a source-of-truth progress checkpoint after this many "
            "completed or safely skipped shards."
        ),
    )
    parser.add_argument(
        "--cargo-runner",
        default=DEFAULT_CARGO_RUNNER,
        help="Shell command for the release-built Rust shard runner.",
    )
    return parser.parse_args()


def utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def detected_cpu_worker_count() -> int:
    if hasattr(os, "sched_getaffinity"):
        affinity = os.sched_getaffinity(0)
        require("MEJEPA_SCHEDULER_CPU_AFFINITY_EMPTY: live CPU affinity is non-empty", bool(affinity))
        return len(affinity)
    count = os.cpu_count()
    require("MEJEPA_SCHEDULER_CPU_COUNT_MISSING: os.cpu_count returned a usable value", count is not None and count > 0)
    return int(count)


def format_cpu_affinity() -> str | None:
    if not hasattr(os, "sched_getaffinity"):
        return None
    cpus = sorted(os.sched_getaffinity(0))
    if not cpus:
        return None
    ranges: list[str] = []
    start = prev = cpus[0]
    for cpu in cpus[1:]:
        if cpu == prev + 1:
            prev = cpu
            continue
        ranges.append(f"{start}-{prev}" if start != prev else str(start))
        start = prev = cpu
    ranges.append(f"{start}-{prev}" if start != prev else str(start))
    return ",".join(ranges)


def require(label: str, condition: bool) -> None:
    if not condition:
        raise SchedulerError(f"python forward shard scheduler assertion failed: {label}")


def parse_embedder_int_map(raw: str, flag_name: str) -> dict[str, int]:
    if not raw.strip():
        return {}
    parsed: dict[str, int] = {}
    for part in raw.split(","):
        item = part.strip()
        require(f"{flag_name} contains no empty entries", bool(item))
        require(f"{flag_name} entries use slug=value", "=" in item)
        slug, value_text = item.split("=", 1)
        slug = slug.strip().lower()
        value_text = value_text.strip()
        require(f"{flag_name} embedder slug is supported true-batch slot", slug in SUPPORTED_TRUE_BATCH_SLUGS)
        require(f"{flag_name} embedder slug appears once", slug not in parsed)
        try:
            value = int(value_text)
        except ValueError as exc:
            raise SchedulerError(f"{flag_name} value for {slug} is not an integer: {value_text}") from exc
        require(f"{flag_name} value for {slug} is positive", value > 0)
        parsed[slug] = value
    return parsed


def validate_embedder_batch_maps(
    row_caps: dict[str, int],
    padded_source_budgets: dict[str, int],
) -> None:
    for sparse_slug in ("e6", "e13"):
        if sparse_slug in row_caps:
            require(f"{sparse_slug.upper()} row cap respects sparse hard max 32", row_caps[sparse_slug] <= 32)
    if "e7" in row_caps:
        require("E7 row cap respects CUDA OOM hard max 32", row_caps["e7"] <= 32)
    if "e12" in row_caps:
        require("E12 row cap respects hard max 32", row_caps["e12"] <= 32)
    if "e14" in row_caps:
        require("E14 row cap respects BGE activation hard max 8", row_caps["e14"] <= 8)
    for slug, value in padded_source_budgets.items():
        require(f"{slug} padded source-byte budget is positive", value > 0)


def validate_release_runner(args: argparse.Namespace) -> dict[str, Any]:
    parts = shlex.split(args.cargo_runner)
    require("MEJEPA_SCHEDULER_RUNNER_EMPTY: cargo runner command is non-empty", bool(parts))
    first = parts[0]
    if first == "cargo":
        raise SchedulerError(
            "MEJEPA_SCHEDULER_RELEASE_RUNNER_REQUIRED: corpus runs must use the "
            "release-built runner; run `cargo build --release -p "
            "context-graph-mejepa-embedders --example python_forward_cache_fsv` "
            "and pass --cargo-runner target/release/examples/python_forward_cache_fsv"
        )
    runner_path = Path(first)
    if runner_path.is_absolute() or "/" in first:
        resolved = runner_path if runner_path.is_absolute() else (Path.cwd() / runner_path)
        require(
            "MEJEPA_SCHEDULER_RELEASE_RUNNER_MISSING: release runner exists before writes",
            resolved.is_file(),
        )
        normalized = resolved.as_posix()
        require(
            "MEJEPA_SCHEDULER_RELEASE_RUNNER_PATH: runner path targets release example binary",
            normalized.endswith("/target/release/examples/python_forward_cache_fsv"),
        )
        return {
            "command": args.cargo_runner,
            "binary_path": str(resolved),
            "binary_size_bytes": resolved.stat().st_size,
            "release_runner": True,
        }
    return {
        "command": args.cargo_runner,
        "binary_path": first,
        "release_runner": True,
        "note": "runner resolved through PATH; command is not cargo run",
    }


def runner_batch_controls(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "true_batch_size": args.true_batch_size,
        "true_batch_max_padded_source_bytes": args.true_batch_max_padded_source_bytes,
        "embedder_batch_sizes": args.embedder_batch_size_map,
        "embedder_max_padded_source_bytes": args.embedder_max_padded_source_byte_map,
        "gpu_vram_guard_bytes": args.gpu_vram_guard_bytes,
        "worker_tiers": {
            "d_drive_io_workers": args.io_workers,
            "cpu_static_tokenization_workers": args.cpu_workers,
            "gpu_batch_workers": args.gpu_workers,
        },
        "cache_store_layout": args.cache_store_layout,
        "require_packed_store": args.require_packed_store,
        "isolate_embedder_processes": args.isolate_embedder_processes,
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return f"sha256:{digest.hexdigest()}"


def json_dump_checked(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = json.dumps(value, indent=2, sort_keys=True).encode("utf-8")
    tmp = path.with_suffix(path.suffix + f".tmp-{os.getpid()}")
    tmp.write_bytes(data)
    require(f"{path} JSON byte readback equality", tmp.read_bytes() == data)
    tmp.replace(path)
    require(f"{path} final JSON byte readback equality", path.read_bytes() == data)


def jsonl_append_checked(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    line = json.dumps(value, sort_keys=True) + "\n"
    with path.open("a", encoding="utf-8") as handle:
        handle.write(line)
        handle.flush()
        os.fsync(handle.fileno())
    last = path.read_text(encoding="utf-8").splitlines()[-1]
    require("event JSONL last-line readback parses", json.loads(last) == value)


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001 - fail-closed with path context.
        raise SchedulerError(f"failed to read JSON {path}: {exc}") from exc


def count_jsonl_rows(path: Path) -> int:
    if not path.exists():
        return 0
    count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                json.loads(line)
            except Exception as exc:  # noqa: BLE001 - include line number.
                raise SchedulerError(f"invalid JSONL row in {path}:{line_no}: {exc}") from exc
            count += 1
    return count


def tree_state(path: Path) -> dict[str, Any]:
    file_count = 0
    byte_count = 0
    sample: list[str] = []
    if path.exists():
        for item in sorted(path.rglob("*")):
            if not item.is_file():
                continue
            file_count += 1
            byte_count += item.stat().st_size
            if len(sample) < 20:
                sample.append(f"{item.relative_to(path)}:{sha256_file(item)}")
    return {
        "path": str(path),
        "exists": path.exists(),
        "file_count": file_count,
        "byte_count": byte_count,
        "hash_sample": sample,
        "hash_sample_truncated": file_count > len(sample),
    }


def file_state(path: Path) -> dict[str, Any]:
    state: dict[str, Any] = {
        "path": str(path),
        "exists": path.exists(),
        "is_file": path.is_file(),
    }
    if path.is_file():
        state["byte_count"] = path.stat().st_size
        state["sha256"] = sha256_file(path)
    return state


def run_probe(cmd: list[str]) -> dict[str, Any]:
    try:
        completed = subprocess.run(cmd, text=True, capture_output=True, check=False)
        return {
            "command": " ".join(shlex.quote(part) for part in cmd),
            "exit_code": completed.returncode,
            "stdout": completed.stdout,
            "stderr": completed.stderr,
        }
    except FileNotFoundError as exc:
        return {
            "command": " ".join(shlex.quote(part) for part in cmd),
            "exit_code": 127,
            "stdout": "",
            "stderr": str(exc),
        }


def require_probe_success(label: str, result: dict[str, Any]) -> None:
    if result["exit_code"] != 0:
        raise SchedulerError(
            f"{label}: command failed "
            f"command={result['command']!r} "
            f"exit_code={result['exit_code']} "
            f"stdout={result['stdout']!r} "
            f"stderr={result['stderr']!r}"
        )


def disk_snapshot() -> dict[str, Any]:
    return run_probe(
        ["df", "-B1", "--output=source,size,used,avail,pcent,target", "/", "/mnt/c", "/mnt/d"]
    )


def parse_disk_snapshot(snapshot: dict[str, Any]) -> dict[str, Any]:
    require("MEJEPA_SCHEDULER_DF_FAILED: df readback succeeded", snapshot["exit_code"] == 0)
    rows: dict[str, Any] = {}
    for line in snapshot["stdout"].splitlines()[1:]:
        parts = line.split()
        require("MEJEPA_SCHEDULER_DF_PARSE: df row has expected columns", len(parts) == 6)
        source, size, used, avail, percent, target = parts
        rows[target] = {
            "source": source,
            "size_bytes": int(size),
            "used_bytes": int(used),
            "available_bytes": int(avail),
            "percent_used": percent,
            "target": target,
        }
    for target in ("/", "/mnt/c", "/mnt/d"):
        require(f"MEJEPA_SCHEDULER_DF_MISSING: df includes {target}", target in rows)
    return rows


def first_cpuinfo_value(key: str) -> str | None:
    cpuinfo = Path("/proc/cpuinfo")
    if not cpuinfo.is_file():
        return None
    prefix = f"{key}\t:"
    for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith(prefix):
            return line.split(":", 1)[1].strip()
    return None


def cpu_snapshot() -> dict[str, Any]:
    nproc = run_probe(["nproc"])
    require_probe_success("MEJEPA_SCHEDULER_NPROC_FAILED", nproc)
    affinity_count = detected_cpu_worker_count()
    return {
        "nproc": nproc,
        "logical_threads": int(nproc["stdout"].strip()),
        "affinity_logical_threads": affinity_count,
        "affinity_cpus_allowed_list": format_cpu_affinity(),
        "os_cpu_count": os.cpu_count(),
        "model_name": first_cpuinfo_value("model name"),
        "cpu_cores_per_socket": first_cpuinfo_value("cpu cores"),
        "siblings": first_cpuinfo_value("siblings"),
    }


def parse_proc_meminfo() -> dict[str, Any]:
    path = Path("/proc/meminfo")
    require("MEJEPA_SCHEDULER_MEMINFO_EXISTS: /proc/meminfo exists", path.is_file())
    wanted = {
        "MemTotal",
        "MemAvailable",
        "MemFree",
        "Buffers",
        "Cached",
        "SwapTotal",
        "SwapFree",
    }
    rows: dict[str, int] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if ":" not in line:
            continue
        key, rest = line.split(":", 1)
        if key not in wanted:
            continue
        parts = rest.strip().split()
        require(f"MEJEPA_SCHEDULER_MEMINFO_PARSE: {key} has kB value", len(parts) >= 1)
        rows[f"{key}_bytes"] = int(parts[0]) * 1024
    for key in wanted:
        require(f"MEJEPA_SCHEDULER_MEMINFO_MISSING: {key}", f"{key}_bytes" in rows)
    require(
        "MEJEPA_SCHEDULER_MEMINFO_AVAILABLE_BOUNDS: MemAvailable <= MemTotal",
        rows["MemAvailable_bytes"] <= rows["MemTotal_bytes"],
    )
    return rows


def process_snapshot(pid: int | None = None) -> dict[str, Any]:
    pid = os.getpid() if pid is None else pid
    status_path = Path(f"/proc/{pid}/status")
    require(
        f"MEJEPA_SCHEDULER_PROC_STATUS_EXISTS: /proc/{pid}/status exists",
        status_path.is_file(),
    )
    fields: dict[str, str] = {}
    for line in status_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        fields[key.strip()] = value.strip()

    def kb_field(name: str) -> int | None:
        value = fields.get(name)
        if value is None:
            return None
        parts = value.split()
        require(f"MEJEPA_SCHEDULER_PROC_PARSE: {name} has kB value", len(parts) >= 1)
        return int(parts[0]) * 1024

    threads = int(fields.get("Threads", "0"))
    require("MEJEPA_SCHEDULER_PROC_THREADS: scheduler process has threads", threads > 0)
    return {
        "pid": pid,
        "name": fields.get("Name"),
        "state": fields.get("State"),
        "threads": threads,
        "vmrss_bytes": kb_field("VmRSS"),
        "vmhwm_bytes": kb_field("VmHWM"),
        "vmsize_bytes": kb_field("VmSize"),
        "vmswap_bytes": kb_field("VmSwap"),
        "cpus_allowed_list": fields.get("Cpus_allowed_list"),
    }


def gpu_snapshot(args: argparse.Namespace) -> dict[str, Any]:
    command = shlex.split(args.nvidia_smi_command) + [
        "--query-gpu=name,driver_version,compute_cap,memory.total,memory.free",
        "--format=csv,noheader,nounits",
    ]
    result = run_probe(command)
    require_probe_success("MEJEPA_SCHEDULER_CUDA_REQUIRED", result)
    gpus = []
    for line in result["stdout"].splitlines():
        if not line.strip():
            continue
        parts = [part.strip() for part in line.split(",")]
        require("MEJEPA_SCHEDULER_GPU_PARSE: nvidia-smi row has expected columns", len(parts) == 5)
        name, driver_version, compute_capability, memory_total_mib, memory_free_mib = parts
        gpus.append(
            {
                "name": name,
                "driver_version": driver_version,
                "compute_capability": compute_capability,
                "memory_total_mib": float(memory_total_mib),
                "memory_free_mib": float(memory_free_mib),
                "memory_total_bytes": int(float(memory_total_mib) * 1024 * 1024),
                "memory_free_bytes": int(float(memory_free_mib) * 1024 * 1024),
            }
        )
    require("MEJEPA_SCHEDULER_NO_GPU: at least one CUDA GPU detected", bool(gpus))
    nvcc = run_probe(shlex.split(args.nvcc_command) + ["--version"])
    return {
        "nvidia_smi": result,
        "gpu_count": len(gpus),
        "gpus": gpus,
        "nvcc": {
            **nvcc,
            "present": nvcc["exit_code"] == 0,
        },
    }


def gpu_resource_snapshot(args: argparse.Namespace) -> dict[str, Any]:
    command = shlex.split(args.nvidia_smi_command) + [
        "--query-gpu=name,memory.total,memory.used,memory.free,utilization.gpu,utilization.memory,pstate,temperature.gpu,power.draw",
        "--format=csv,noheader,nounits",
    ]
    result = run_probe(command)
    require_probe_success("MEJEPA_SCHEDULER_GPU_RESOURCE_SNAPSHOT_FAILED", result)
    gpus = []
    for line in result["stdout"].splitlines():
        if not line.strip():
            continue
        parts = [part.strip() for part in line.split(",")]
        require("MEJEPA_SCHEDULER_GPU_RESOURCE_PARSE: nvidia-smi row has expected columns", len(parts) == 9)
        name, total_mib, used_mib, free_mib, util_gpu, util_mem, pstate, temperature_gpu, power_draw = parts
        total_bytes = int(float(total_mib) * 1024 * 1024)
        used_bytes = int(float(used_mib) * 1024 * 1024)
        free_bytes = int(float(free_mib) * 1024 * 1024)
        require(
            "MEJEPA_SCHEDULER_GPU_RESOURCE_MEMORY_ACCOUNTING: used+free does not exceed total by more than one MiB",
            used_bytes + free_bytes <= total_bytes + 1024 * 1024,
        )
        gpus.append({
            "name": name,
            "memory_total_mib": float(total_mib),
            "memory_used_mib": float(used_mib),
            "memory_free_mib": float(free_mib),
            "memory_total_bytes": total_bytes,
            "memory_used_bytes": used_bytes,
            "memory_free_bytes": free_bytes,
            "utilization_gpu_percent": float(util_gpu),
            "utilization_memory_percent": float(util_mem),
            "pstate": pstate,
            "temperature_gpu_c": float(temperature_gpu),
            "power_draw_watts": power_draw,
        })
    require("MEJEPA_SCHEDULER_GPU_RESOURCE_NO_GPU: at least one CUDA GPU in resource snapshot", bool(gpus))
    return {
        "nvidia_smi": result,
        "gpu_count": len(gpus),
        "gpus": gpus,
    }


def resource_snapshot(args: argparse.Namespace, phase: str) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "phase": phase,
        "memory": parse_proc_meminfo(),
        "scheduler_process": process_snapshot(),
        "gpu": gpu_resource_snapshot(args),
        "disk": disk_snapshot(),
    }


def path_is_under(path: Path, root: Path) -> bool:
    try:
        path.expanduser().resolve(strict=False).relative_to(root.resolve(strict=False))
        return True
    except ValueError:
        return False


def projected_capacity(args: argparse.Namespace) -> dict[str, Any]:
    require("MEJEPA_SCHEDULER_ACTIVE_EMBEDDERS_INVALID: active embedder count is positive", args.active_embedder_count > 0)
    require("MEJEPA_SCHEDULER_RECORD_ESTIMATE_INVALID: estimated record bytes is positive", args.estimated_record_bytes > 0)
    require("MEJEPA_SCHEDULER_HOT_BUDGET_INVALID: hot-stage budget bytes is positive", args.hot_stage_budget_bytes > 0)
    require("MEJEPA_SCHEDULER_FSV_BUDGET_INVALID: FSV edge-copy budget is non-negative", args.fsv_edge_copy_budget_bytes >= 0)
    if args.projected_output_bytes is not None:
        require("MEJEPA_SCHEDULER_PROJECTED_BYTES_INVALID: projected output bytes is positive", args.projected_output_bytes > 0)
    planned_chunks = args.shard_size * args.shard_count
    projected_pairs = planned_chunks * args.active_embedder_count
    projected_output_bytes = args.projected_output_bytes
    if projected_output_bytes is None:
        projected_output_bytes = projected_pairs * args.estimated_record_bytes
    return {
        "planned_chunks": planned_chunks,
        "active_embedder_count": args.active_embedder_count,
        "projected_pairs": projected_pairs,
        "estimated_record_bytes": args.estimated_record_bytes,
        "projected_output_bytes": projected_output_bytes,
        "hot_stage_budget_bytes": args.hot_stage_budget_bytes,
        "fsv_edge_copy_budget_bytes": args.fsv_edge_copy_budget_bytes,
        "durable_slack_bytes": DEFAULT_DURABLE_SLACK_BYTES,
        "required_durable_available_bytes": projected_output_bytes
        + args.fsv_edge_copy_budget_bytes
        + DEFAULT_DURABLE_SLACK_BYTES,
    }


def host_preflight(args: argparse.Namespace) -> dict[str, Any]:
    hot_root = args.hot_root.expanduser().resolve(strict=False)
    cache_root = args.cache_root.expanduser().resolve(strict=False)
    fsv_root = args.fsv_root.expanduser().resolve(strict=False)
    runner = validate_release_runner(args)
    require(
        "MEJEPA_SCHEDULER_HOT_ROOT_WINDOWS_MOUNT: hot root must not be under /mnt/c",
        not path_is_under(hot_root, Path("/mnt/c")),
    )
    require(
        "MEJEPA_SCHEDULER_HOT_ROOT_DURABLE_MOUNT: hot root must use WSL ext4, not /mnt/d",
        not path_is_under(hot_root, Path("/mnt/d")),
    )
    require(
        "MEJEPA_SCHEDULER_CACHE_ROOT_NOT_D_ROOT: cache root must be durable under /mnt/d/contextgraph",
        path_is_under(cache_root, Path("/mnt/d/contextgraph")),
    )
    require(
        "MEJEPA_SCHEDULER_FSV_ROOT_NOT_D_ROOT: FSV root must be durable under /mnt/d/contextgraph/fsv",
        path_is_under(fsv_root, Path("/mnt/d/contextgraph/fsv")),
    )

    disks_raw = disk_snapshot()
    disks = parse_disk_snapshot(disks_raw)
    cpu = cpu_snapshot()
    gpu = gpu_snapshot(args)
    capacity = projected_capacity(args)
    worker_ceiling = min(int(cpu["logical_threads"]), int(cpu["affinity_logical_threads"]))
    require("MEJEPA_SCHEDULER_IO_WORKERS_INVALID: D-drive I/O workers is positive", args.io_workers > 0)
    require("MEJEPA_SCHEDULER_CPU_WORKERS_INVALID: CPU workers is positive", args.cpu_workers > 0)
    require("MEJEPA_SCHEDULER_GPU_WORKERS_INVALID: GPU workers is positive", args.gpu_workers > 0)
    require(
        "MEJEPA_SCHEDULER_IO_WORKERS_EXCEED_POLICY: D-drive/Docker I/O workers stay bounded",
        args.io_workers <= MAX_D_DRIVE_IO_WORKERS,
    )
    require(
        "MEJEPA_SCHEDULER_CPU_WORKERS_EXCEED_POLICY: CPU workers fit live WSL thread policy",
        args.cpu_workers <= worker_ceiling,
    )
    require(
        "MEJEPA_SCHEDULER_GPU_WORKERS_EXCEED_DEVICES: GPU workers fit detected CUDA devices",
        args.gpu_workers <= gpu["gpu_count"] * MAX_GPU_WORKERS_PER_DEVICE,
    )
    require(
        "MEJEPA_SCHEDULER_HOT_CAPACITY: WSL ext4 hot root has planned free bytes",
        capacity["hot_stage_budget_bytes"] <= disks["/"]["available_bytes"],
    )
    require(
        "MEJEPA_SCHEDULER_D_CAPACITY: D-drive durable root has planned free bytes",
        capacity["required_durable_available_bytes"] <= disks["/mnt/d"]["available_bytes"],
    )
    min_gpu_total = min(gpu_row["memory_total_bytes"] for gpu_row in gpu["gpus"])
    require(
        "MEJEPA_SCHEDULER_VRAM_GUARD_EXCEEDS_DEVICE: VRAM guard fits detected GPU memory",
        args.gpu_vram_guard_bytes <= min_gpu_total,
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "host": {
            "cpu": cpu,
            "gpu": gpu,
            "disks_raw": disks_raw,
            "filesystems": disks,
        },
        "paths": {
            "hot_root": str(hot_root),
            "cache_root": str(cache_root),
            "fsv_root": str(fsv_root),
            "hot_root_on_wsl_ext4": True,
            "cache_root_on_durable_d": True,
            "fsv_root_on_durable_d": True,
        },
        "runner": runner,
        "capacity_policy": capacity,
        "worker_policy": {
            "selected": runner_batch_controls(args)["worker_tiers"],
            "max_d_drive_io_workers": MAX_D_DRIVE_IO_WORKERS,
            "cpu_worker_ceiling": worker_ceiling,
            "detected_logical_threads": cpu["logical_threads"],
            "detected_affinity_logical_threads": cpu["affinity_logical_threads"],
            "detected_affinity_cpus_allowed_list": cpu["affinity_cpus_allowed_list"],
            "detected_gpu_count": gpu["gpu_count"],
            "max_gpu_workers_per_device": MAX_GPU_WORKERS_PER_DEVICE,
        },
    }


def build_plan(args: argparse.Namespace) -> list[ShardSpec]:
    require("start offset is non-negative", args.start_offset >= 0)
    require("shard size is positive", args.shard_size > 0)
    require("shard count is positive", args.shard_count > 0)
    require("max source bytes is positive", args.max_source_bytes > 0)
    require("true batch size is positive", args.true_batch_size > 0)
    if args.true_batch_max_padded_source_bytes is not None:
        require(
            "global padded source-byte budget is positive",
            args.true_batch_max_padded_source_bytes > 0,
        )
    validate_embedder_batch_maps(
        args.embedder_batch_size_map,
        args.embedder_max_padded_source_byte_map,
    )
    require("GPU VRAM guard bytes is positive", args.gpu_vram_guard_bytes > 0)
    require("D-drive I/O worker count is positive", args.io_workers > 0)
    require("CPU worker count is positive", args.cpu_workers > 0)
    require("GPU worker count is positive", args.gpu_workers > 0)
    require("JSONL sync interval is positive", args.jsonl_sync_every_rows > 0)
    require("runner timeout is positive", args.runner_timeout_seconds > 0)
    require("checkpoint interval is positive", args.checkpoint_every_shards > 0)
    require("chunks JSONL exists", args.chunks_jsonl.is_file())
    shards: list[ShardSpec] = []
    for idx in range(args.shard_count):
        offset = args.start_offset + idx * args.shard_size
        shard_id = f"shard-{idx:06d}-offset-{offset:09d}"
        shards.append(
            ShardSpec(
                shard_id=shard_id,
                chunk_offset=offset,
                max_chunks=args.shard_size,
                cache_root=args.cache_root / "shards" / shard_id,
                fsv_root=args.fsv_root / "shards" / shard_id,
            )
        )
    return shards


def event(path: Path, phase: str, detail: dict[str, Any]) -> None:
    jsonl_append_checked(
        path,
        {
            "schema_version": SCHEMA_VERSION,
            "task_id": TASK_ID,
            "timestamp_utc": utc_now(),
            "phase": phase,
            "detail": detail,
        },
    )


def run_shard(
    args: argparse.Namespace,
    shard: ShardSpec,
    event_path: Path,
    preflight: dict[str, Any],
    extra_env: dict[str, str] | None = None,
    run_log_name: str | None = None,
    event_phase: str = "shard_run_complete",
    require_packed_store: bool | None = None,
) -> dict[str, Any]:
    env = os.environ.copy()
    env.update(
        {
            "CG_MEJEPA_PY_FORWARD_CACHE_ROOT": str(shard.cache_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_FSV_ROOT": str(shard.fsv_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_HOT_ROOT": str(args.hot_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET": str(shard.chunk_offset),
            "CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS": str(shard.max_chunks),
            "CG_MEJEPA_PY_FORWARD_MAX_SOURCE_BYTES": str(args.max_source_bytes),
            "CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE": str(args.true_batch_size),
            "CG_MEJEPA_PY_FORWARD_GPU_VRAM_GUARD_BYTES": str(args.gpu_vram_guard_bytes),
            "CG_MEJEPA_PY_FORWARD_IO_WORKERS": str(args.io_workers),
            "CG_MEJEPA_PY_FORWARD_CPU_WORKERS": str(args.cpu_workers),
        "CG_MEJEPA_PY_FORWARD_GPU_WORKERS": str(args.gpu_workers),
        "CG_MEJEPA_PY_FORWARD_JSONL_SYNC_EVERY_ROWS": str(args.jsonl_sync_every_rows),
        "CG_MEJEPA_PY_FORWARD_CACHE_STORE_LAYOUT": args.cache_store_layout,
    }
    )
    if args.true_batch_max_padded_source_bytes is not None:
        env["CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES"] = str(
            args.true_batch_max_padded_source_bytes
        )
    for slug, value in args.embedder_batch_size_map.items():
        env[f"CG_MEJEPA_PY_FORWARD_TRUE_BATCH_SIZE_{slug.upper()}"] = str(value)
    for slug, value in args.embedder_max_padded_source_byte_map.items():
        env[f"CG_MEJEPA_PY_FORWARD_TRUE_BATCH_MAX_PADDED_SOURCE_BYTES_{slug.upper()}"] = str(value)
    if extra_env:
        env.update(extra_env)
    resource_before = resource_snapshot(args, f"{shard.shard_id}_runner_before")
    event(
        event_path,
        "shard_run_start",
        {
            "shard_id": shard.shard_id,
            "chunk_offset": shard.chunk_offset,
            "max_chunks": shard.max_chunks,
            "cache_root": str(shard.cache_root),
            "fsv_root": str(shard.fsv_root),
            "resource": resource_before,
        },
    )
    started = time.monotonic()
    completed = subprocess.run(
        shlex.split(args.cargo_runner),
        text=True,
        capture_output=True,
        env=env,
        timeout=args.runner_timeout_seconds,
        check=False,
    )
    elapsed = time.monotonic() - started
    resource_after = resource_snapshot(args, f"{shard.shard_id}_runner_after")
    run_log = {
        "shard_id": shard.shard_id,
        "exit_code": completed.returncode,
        "elapsed_seconds": round(elapsed, 3),
        "runner_batch_controls": runner_batch_controls(args),
        "runner_extra_env": extra_env or {},
        "host_preflight": preflight,
        "resource_before": resource_before,
        "resource_after": resource_after,
        "stdout_tail": completed.stdout[-8000:],
        "stderr_tail": completed.stderr[-8000:],
    }
    run_log_path = args.fsv_root / "run_logs" / (run_log_name or f"{shard.shard_id}.json")
    json_dump_checked(run_log_path, run_log)
    require(f"{shard.shard_id} runner exited zero", completed.returncode == 0)
    run_log.update(shard_completion_readback(args, shard, require_packed_store))
    run_log["skipped_completed"] = False
    json_dump_checked(run_log_path, run_log)
    event(event_path, event_phase, run_log)
    return run_log


def run_shard_embedder_isolated(
    args: argparse.Namespace,
    shard: ShardSpec,
    event_path: Path,
    preflight: dict[str, Any],
) -> dict[str, Any]:
    event(
        event_path,
        "shard_embedder_isolation_start",
        {
            "shard_id": shard.shard_id,
            "embedder_order": list(ISOLATED_FORWARD_SLUGS),
        },
    )
    subruns = []
    for slug in ISOLATED_FORWARD_SLUGS:
        subrun = run_shard(
            args,
            shard,
            event_path,
            preflight,
            extra_env={"CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER": slug},
            run_log_name=f"{shard.shard_id}.{slug}.json",
            event_phase="shard_embedder_subrun_complete",
            require_packed_store=False,
        )
        subruns.append(
            {
                "embedder_id": slug,
                "run_log": str(args.fsv_root / "run_logs" / f"{shard.shard_id}.{slug}.json"),
                "manifest_sha256": subrun.get("manifest_sha256"),
                "fsv_sha256": subrun.get("fsv_sha256"),
                "summary": subrun.get("summary"),
            }
        )
    final_run = run_shard(
        args,
        shard,
        event_path,
        preflight,
        event_phase="shard_run_complete",
    )
    final_run["isolated_embedder_subruns"] = subruns
    json_dump_checked(shard_run_log_path(args, shard), final_run)
    event(event_path, "shard_run_embedder_isolated_complete", final_run)
    return final_run


def load_existing_run_logs(fsv_root: Path) -> list[dict[str, Any]]:
    run_log_root = fsv_root / "run_logs"
    if not run_log_root.exists():
        return []
    logs = []
    for path in sorted(run_log_root.glob("*.json")):
        if not SHARD_RUN_LOG_NAME_PATTERN.fullmatch(path.name):
            continue
        log = read_json(path)
        require(f"{path} run log exit code zero", log.get("exit_code") == 0)
        logs.append(log)
    return logs


def validate_shard_fsv(shard: ShardSpec) -> dict[str, Any]:
    fsv = read_json(shard.fsv_path())
    require(f"{shard.shard_id} shard FSV passes", fsv.get("passes") is True)
    return {
        "fsv_path": str(shard.fsv_path()),
        "fsv_sha256": sha256_file(shard.fsv_path()),
    }


def shard_completion_readback(
    args: argparse.Namespace,
    shard: ShardSpec,
    require_packed_store: bool | None = None,
) -> dict[str, Any]:
    manifest = validate_manifest(
        shard,
        args.require_packed_store if require_packed_store is None else require_packed_store,
        validate_packed_readback=require_packed_store is not False,
    )
    fsv_readback = validate_shard_fsv(shard)
    return {
        "manifest_path": str(shard.manifest_path()),
        "manifest_sha256": sha256_file(shard.manifest_path()),
        **fsv_readback,
        "summary": manifest.get("summary"),
        "packed_store_readback": manifest.get("packed_store_readback"),
    }


def require_full_scheduler_shard(
    args: argparse.Namespace,
    shard: ShardSpec,
    summary: dict[str, Any],
) -> None:
    active_embedder_count = int(summary.get("active_embedder_count", -1))
    chunk_count = int(summary.get("chunk_count", -1))
    cache_record_count = int(summary.get("cache_record_count", -1))
    quarantine_row_count = int(summary.get("quarantine_row_count", -1))
    expected_pairs = chunk_count * args.active_embedder_count
    accounted_pairs = cache_record_count + quarantine_row_count
    require(
        f"{shard.shard_id} completed shard active embedder count matches scheduler",
        active_embedder_count == args.active_embedder_count,
    )
    require(
        f"{shard.shard_id} completed shard accounts all scheduler embedder pairs",
        accounted_pairs == expected_pairs,
    )


def completed_shard_readback(args: argparse.Namespace, shard: ShardSpec) -> dict[str, Any]:
    readback = shard_completion_readback(args, shard)
    summary = readback.get("summary")
    require(f"{shard.shard_id} completed shard summary exists", isinstance(summary, dict))
    require_full_scheduler_shard(args, shard, summary)
    return {
        "shard_id": shard.shard_id,
        "exit_code": 0,
        "elapsed_seconds": 0.0,
        "skipped_completed": True,
        "runner_batch_controls": runner_batch_controls(args),
        **readback,
    }


def shard_run_log_path(args: argparse.Namespace, shard: ShardSpec) -> Path:
    return args.fsv_root / "run_logs" / f"{shard.shard_id}.json"


def unique_archive_root(parent: Path, stem: str) -> Path:
    parent.mkdir(parents=True, exist_ok=True)
    candidate = parent / stem
    suffix = 1
    while candidate.exists():
        candidate = parent / f"{stem}-{suffix:02d}"
        suffix += 1
    return candidate


def unique_json_path(parent: Path, stem: str) -> Path:
    parent.mkdir(parents=True, exist_ok=True)
    candidate = parent / f"{stem}.json"
    suffix = 1
    while candidate.exists():
        candidate = parent / f"{stem}-{suffix:02d}.json"
        suffix += 1
    return candidate


def archive_failed_partial_shard(
    args: argparse.Namespace,
    shard: ShardSpec,
    reason: str,
) -> dict[str, Any]:
    run_log_path = shard_run_log_path(args, shard)
    before = {
        "cache_root": tree_state(shard.cache_root),
        "fsv_root": tree_state(shard.fsv_root),
        "run_log": file_state(run_log_path),
    }
    archive_root = unique_archive_root(
        args.fsv_root / "failed_partial_shards",
        f"{utc_now().replace('-', '').replace(':', '')}-{shard.shard_id}",
    )
    archive_root.mkdir(parents=True, exist_ok=False)
    moved: dict[str, str] = {}
    for label, source in (
        ("cache_root", shard.cache_root),
        ("fsv_root", shard.fsv_root),
    ):
        if source.exists():
            destination = archive_root / label
            shutil.move(str(source), str(destination))
            require(f"{label} moved into failed partial archive", destination.exists())
            require(f"{label} source removed after archive move", not source.exists())
            moved[label] = str(destination)
    if run_log_path.exists():
        destination = archive_root / "run_log.json"
        shutil.move(str(run_log_path), str(destination))
        require("run log moved into failed partial archive", destination.is_file())
        require("run log source removed after archive move", not run_log_path.exists())
        moved["run_log"] = str(destination)
    require("failed partial archive captured at least one artifact", bool(moved))
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_failed_partial_shard_archive",
        "created_at_utc": utc_now(),
        "shard_id": shard.shard_id,
        "chunk_offset": shard.chunk_offset,
        "max_chunks": shard.max_chunks,
        "reason": reason,
        "archive_root": str(archive_root),
        "before_state": before,
        "moved_artifacts": moved,
        "source_after_state": {
            "cache_root": tree_state(shard.cache_root),
            "fsv_root": tree_state(shard.fsv_root),
            "run_log": file_state(run_log_path),
        },
        "passes": True,
    }
    manifest_path = archive_root / "archive_manifest.json"
    json_dump_checked(manifest_path, manifest)
    readback = read_json(manifest_path)
    require("failed partial archive manifest readback passes", readback.get("passes") is True)
    return {
        **manifest,
        "archive_manifest_path": str(manifest_path),
        "archive_manifest_sha256": sha256_file(manifest_path),
    }


def write_checkpoint(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    run_logs: list[dict[str, Any]],
    started_at: str,
    preflight: dict[str, Any],
    phase: str,
) -> dict[str, Any]:
    completed_by_id: dict[str, dict[str, Any]] = {}
    for log in run_logs:
        shard_id = log.get("shard_id")
        require("checkpoint run log has shard id", isinstance(shard_id, str) and bool(shard_id))
        require(f"checkpoint run log {shard_id} exited zero", log.get("exit_code") == 0)
        require(f"checkpoint run log {shard_id} appears once", shard_id not in completed_by_id)
        summary = log.get("summary")
        require(f"checkpoint run log {shard_id} has validated summary", isinstance(summary, dict))
        completed_by_id[shard_id] = log

    completed_shards = [shard for shard in shards if shard.shard_id in completed_by_id]
    next_shard = next((shard for shard in shards if shard.shard_id not in completed_by_id), None)
    totals = {
        "chunk_count": 0,
        "cache_record_count": 0,
        "first_pass_hits": 0,
        "first_pass_writes": 0,
        "second_pass_hits": 0,
        "quarantine_row_count": 0,
        "batch_telemetry_row_count": 0,
    }
    active_embedder_count = args.active_embedder_count
    active_embedder_locked = False
    for shard in completed_shards:
        summary = completed_by_id[shard.shard_id]["summary"]
        if not active_embedder_locked:
            active_embedder_count = int(summary["active_embedder_count"])
            active_embedder_locked = True
        require(
            f"{shard.shard_id} checkpoint active embedder count stable",
            int(summary["active_embedder_count"]) == active_embedder_count,
        )
        for key in totals:
            totals[key] += int(summary[key])

    expected_pairs = totals["chunk_count"] * active_embedder_count
    accounted_pairs = totals["cache_record_count"] + totals["quarantine_row_count"]
    require("checkpoint accounted pairs match completed expected pairs", accounted_pairs == expected_pairs)
    require("checkpoint second pass covers cache records", totals["second_pass_hits"] == totals["cache_record_count"])
    checkpoint = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_shard_scheduler_checkpoint",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "phase": phase,
        "status": "running" if next_shard is not None else "ready_for_final_aggregate",
        "source_of_truth": {
            "checkpoint_cache_path": str(args.cache_root / "python_forward_cache_checkpoint.json"),
            "checkpoint_fsv_path": str(args.fsv_root / "python_forward_cache_checkpoint.json"),
            "cache_root": str(args.cache_root),
            "fsv_root": str(args.fsv_root),
            "run_log_root": str(args.fsv_root / "run_logs"),
        },
        "runner_controls": runner_batch_controls(args),
        "host_preflight": preflight,
        "progress": {
            "completed_shards": len(completed_shards),
            "total_shards": len(shards),
            "completed_chunk_capacity": len(completed_shards) * args.shard_size,
            "planned_chunk_capacity": len(shards) * args.shard_size,
            "next_shard": None
            if next_shard is None
            else {
                "shard_id": next_shard.shard_id,
                "chunk_offset": next_shard.chunk_offset,
                "max_chunks": next_shard.max_chunks,
                "cache_root": str(next_shard.cache_root),
                "fsv_root": str(next_shard.fsv_root),
            },
            "last_completed_shard": None
            if not completed_shards
            else {
                "shard_id": completed_shards[-1].shard_id,
                "chunk_offset": completed_shards[-1].chunk_offset,
                "max_chunks": completed_shards[-1].max_chunks,
            },
        },
        "summary": {
            **totals,
            "active_embedder_count": active_embedder_count,
            "expected_pairs": expected_pairs,
            "accounted_pairs": accounted_pairs,
            "shard_count": len(completed_shards),
        },
        "recent_run_logs": run_logs[-5:],
        "disk": disk_snapshot(),
        "resource": resource_snapshot(args, f"checkpoint_{phase}"),
        "passes": True,
    }
    cache_checkpoint_path = args.cache_root / "python_forward_cache_checkpoint.json"
    fsv_checkpoint_path = args.fsv_root / "python_forward_cache_checkpoint.json"
    json_dump_checked(cache_checkpoint_path, checkpoint)
    json_dump_checked(fsv_checkpoint_path, checkpoint)
    cache_readback = read_json(cache_checkpoint_path)
    fsv_readback = read_json(fsv_checkpoint_path)
    require("checkpoint cache and FSV summaries match", cache_readback["summary"] == fsv_readback["summary"])
    require("checkpoint readback passes", cache_readback["passes"] is True and fsv_readback["passes"] is True)
    return checkpoint


def checkpoint_file_paths(args: argparse.Namespace) -> tuple[Path, Path]:
    return (
        args.cache_root / "python_forward_cache_checkpoint.json",
        args.fsv_root / "python_forward_cache_checkpoint.json",
    )


def read_existing_checkpoint(args: argparse.Namespace) -> dict[str, Any]:
    cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
    require("resume checkpoint cache file exists", cache_checkpoint_path.is_file())
    require("resume checkpoint FSV file exists", fsv_checkpoint_path.is_file())
    cache_bytes = cache_checkpoint_path.read_bytes()
    fsv_bytes = fsv_checkpoint_path.read_bytes()
    require("resume checkpoint cache and FSV bytes match", cache_bytes == fsv_bytes)
    checkpoint = json.loads(cache_bytes.decode("utf-8"))
    require("resume checkpoint passes", checkpoint.get("passes") is True)
    require(
        "resume checkpoint artifact kind matches",
        checkpoint.get("artifact_kind") == "python_forward_shard_scheduler_checkpoint",
    )
    return checkpoint


def checkpoint_next_shard(checkpoint: dict[str, Any], shards: list[ShardSpec]) -> ShardSpec:
    progress = checkpoint.get("progress", {})
    next_info = progress.get("next_shard")
    require("resume checkpoint has a next shard", isinstance(next_info, dict))
    shard_id = next_info.get("shard_id")
    require("resume checkpoint next shard id is present", isinstance(shard_id, str) and bool(shard_id))
    matching = [shard for shard in shards if shard.shard_id == shard_id]
    require("resume checkpoint next shard exists in current shard plan", len(matching) == 1)
    shard = matching[0]
    require(
        "resume checkpoint next shard offset matches plan",
        int(next_info.get("chunk_offset", -1)) == shard.chunk_offset,
    )
    require(
        "resume checkpoint next shard size matches plan",
        int(next_info.get("max_chunks", -1)) == shard.max_chunks,
    )
    return shard


def write_resume_decision(
    args: argparse.Namespace,
    shard: ShardSpec,
    decision: dict[str, Any],
) -> dict[str, Any]:
    decision_root = args.fsv_root / "resume_decisions"
    path = unique_json_path(
        decision_root,
        f"{utc_now().replace('-', '').replace(':', '')}-{shard.shard_id}",
    )
    value = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_resume_decision",
        "created_at_utc": utc_now(),
        "shard_id": shard.shard_id,
        "chunk_offset": shard.chunk_offset,
        "max_chunks": shard.max_chunks,
        **decision,
    }
    json_dump_checked(path, value)
    readback = read_json(path)
    require("resume decision readback has pass flag", readback.get("passes") in (True, False))
    return {
        **value,
        "decision_path": str(path),
        "decision_sha256": sha256_file(path),
    }


def scan_checkpoint_next_shard_state(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    checkpoint: dict[str, Any],
) -> dict[str, Any]:
    shard = checkpoint_next_shard(checkpoint, shards)
    cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
    before = {
        "checkpoint_cache": file_state(cache_checkpoint_path),
        "checkpoint_fsv": file_state(fsv_checkpoint_path),
        "cache_root": tree_state(shard.cache_root),
        "fsv_root": tree_state(shard.fsv_root),
        "run_log": file_state(shard_run_log_path(args, shard)),
    }
    has_existing_state = (
        shard.cache_root.exists()
        or shard.fsv_root.exists()
        or shard_run_log_path(args, shard).exists()
    )
    if not has_existing_state:
        return write_resume_decision(
            args,
            shard,
            {
                "decision": "no_partial_state",
                "reason": "checkpoint next shard has no cache, FSV, or run-log state",
                "before_state": before,
                "after_state": {
                    "checkpoint_cache": file_state(cache_checkpoint_path),
                    "checkpoint_fsv": file_state(fsv_checkpoint_path),
                    "cache_root": tree_state(shard.cache_root),
                    "fsv_root": tree_state(shard.fsv_root),
                    "run_log": file_state(shard_run_log_path(args, shard)),
                },
                "checkpoint_sha256_unchanged": before["checkpoint_cache"].get("sha256")
                == file_state(cache_checkpoint_path).get("sha256"),
                "passes": True,
            },
        )
    try:
        completed = completed_shard_readback(args, shard)
    except SchedulerError as exc:
        if not args.retry_failed_partial_shards:
            decision = write_resume_decision(
                args,
                shard,
                {
                    "decision": "blocked_partial_state",
                    "reason": str(exc),
                    "before_state": before,
                    "after_state": {
                        "checkpoint_cache": file_state(cache_checkpoint_path),
                        "checkpoint_fsv": file_state(fsv_checkpoint_path),
                        "cache_root": tree_state(shard.cache_root),
                        "fsv_root": tree_state(shard.fsv_root),
                        "run_log": file_state(shard_run_log_path(args, shard)),
                    },
                    "checkpoint_sha256_unchanged": before["checkpoint_cache"].get("sha256")
                    == file_state(cache_checkpoint_path).get("sha256"),
                    "passes": False,
                },
            )
            raise SchedulerError(
                f"checkpoint next shard has unsafe partial state; decision={decision['decision_path']}"
            ) from exc
        archive = archive_failed_partial_shard(
            args,
            shard,
            f"pre-resume scanner rejected checkpoint next shard: {exc}",
        )
        return write_resume_decision(
            args,
            shard,
            {
                "decision": "archived_for_retry",
                "reason": str(exc),
                "before_state": before,
                "archive": archive,
                "after_state": {
                    "checkpoint_cache": file_state(cache_checkpoint_path),
                    "checkpoint_fsv": file_state(fsv_checkpoint_path),
                    "cache_root": tree_state(shard.cache_root),
                    "fsv_root": tree_state(shard.fsv_root),
                    "run_log": file_state(shard_run_log_path(args, shard)),
                },
                "checkpoint_sha256_unchanged": before["checkpoint_cache"].get("sha256")
                == file_state(cache_checkpoint_path).get("sha256"),
                "passes": True,
            },
        )
    return write_resume_decision(
        args,
        shard,
        {
            "decision": "exact_resume_proof_accepted",
            "reason": "completed shard readback passed against D-drive SoT",
            "before_state": before,
            "completed_readback": completed,
            "after_state": {
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_fsv": file_state(fsv_checkpoint_path),
                "cache_root": tree_state(shard.cache_root),
                "fsv_root": tree_state(shard.fsv_root),
                "run_log": file_state(shard_run_log_path(args, shard)),
            },
            "checkpoint_sha256_unchanged": before["checkpoint_cache"].get("sha256")
            == file_state(cache_checkpoint_path).get("sha256"),
            "passes": True,
        },
    )


def checkpoint_for_shard(shard: ShardSpec) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_shard_scheduler_checkpoint",
        "passes": True,
        "progress": {
            "next_shard": {
                "shard_id": shard.shard_id,
                "chunk_offset": shard.chunk_offset,
                "max_chunks": shard.max_chunks,
                "cache_root": str(shard.cache_root),
                "fsv_root": str(shard.fsv_root),
            }
        },
    }


def write_checkpoint_pair(args: argparse.Namespace, checkpoint: dict[str, Any]) -> dict[str, Any]:
    cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
    json_dump_checked(cache_checkpoint_path, checkpoint)
    json_dump_checked(fsv_checkpoint_path, checkpoint)
    cache_readback = read_json(cache_checkpoint_path)
    fsv_readback = read_json(fsv_checkpoint_path)
    require("checkpoint pair cache readback passes", cache_readback.get("passes") is True)
    require("checkpoint pair FSV readback passes", fsv_readback.get("passes") is True)
    require("checkpoint pair bytes match", cache_checkpoint_path.read_bytes() == fsv_checkpoint_path.read_bytes())
    return {
        "cache_checkpoint": file_state(cache_checkpoint_path),
        "fsv_checkpoint": file_state(fsv_checkpoint_path),
    }


def clone_resume_edge_args(
    args: argparse.Namespace,
    edge_root: Path,
    case_name: str,
    *,
    active_embedder_count: int | None = None,
    retry_failed_partial_shards: bool = True,
) -> argparse.Namespace:
    case_root = edge_root / case_name
    if case_root.exists():
        shutil.rmtree(case_root)
    edge_args = argparse.Namespace(**vars(args))
    edge_args.cache_root = case_root / "cache"
    edge_args.fsv_root = case_root / "fsv"
    edge_args.hot_root = case_root / "hot"
    edge_args.retry_failed_partial_shards = retry_failed_partial_shards
    edge_args.require_packed_store = True
    if active_embedder_count is not None:
        edge_args.active_embedder_count = active_embedder_count
    edge_args.cache_root.mkdir(parents=True)
    edge_args.fsv_root.mkdir(parents=True)
    edge_args.hot_root.mkdir(parents=True)
    os.chmod(edge_args.hot_root, 0o700)
    return edge_args


def resume_preflight_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    edge_root = args.fsv_root / "edge_inputs" / "resume_preflight"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = []

    def append_case(case_name: str, edge_args: argparse.Namespace, shard: ShardSpec, decision: dict[str, Any]) -> None:
        cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(edge_args)
        cases.append(
            {
                "case": case_name,
                "passes": True,
                "decision": decision["decision"],
                "decision_path": decision["decision_path"],
                "decision_sha256": decision["decision_sha256"],
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_fsv": file_state(fsv_checkpoint_path),
                "source_after_state": {
                    "cache_root": tree_state(shard.cache_root),
                    "fsv_root": tree_state(shard.fsv_root),
                    "run_log": file_state(shard_run_log_path(edge_args, shard)),
                },
            }
        )

    no_state_args = clone_resume_edge_args(args, edge_root, "no-partial-state", active_embedder_count=1)
    no_state_shard = ShardSpec(
        "edge-no-partial-state",
        500_000_000,
        1,
        no_state_args.cache_root / "shards" / "edge-no-partial-state",
        no_state_args.fsv_root / "shards" / "edge-no-partial-state",
    )
    write_checkpoint_pair(no_state_args, checkpoint_for_shard(no_state_shard))
    no_state_decision = scan_checkpoint_next_shard_state(no_state_args, [no_state_shard], read_existing_checkpoint(no_state_args))
    require("resume no-partial edge accepts absent state", no_state_decision["decision"] == "no_partial_state")
    require("resume no-partial edge keeps checkpoint unchanged", no_state_decision["checkpoint_sha256_unchanged"] is True)
    append_case("resume_no_partial_state_accepted", no_state_args, no_state_shard, no_state_decision)

    partial_args = clone_resume_edge_args(args, edge_root, "partial-without-manifest", active_embedder_count=1)
    partial_shard = ShardSpec(
        "edge-partial-without-manifest",
        500_000_001,
        1,
        partial_args.cache_root / "shards" / "edge-partial-without-manifest",
        partial_args.fsv_root / "shards" / "edge-partial-without-manifest",
    )
    write_checkpoint_pair(partial_args, checkpoint_for_shard(partial_shard))
    partial_shard.cache_root.mkdir(parents=True)
    partial_shard.fsv_root.mkdir(parents=True)
    (partial_shard.cache_root / "python_forward_cache_index.jsonl").write_text(
        json.dumps({"cache_key_sha256": f"sha256:{'6' * 64}"}) + "\n",
        encoding="utf-8",
    )
    json_dump_checked(partial_shard.fsv_root / "partial_marker.json", {"passes": False})
    json_dump_checked(
        shard_run_log_path(partial_args, partial_shard),
        {"shard_id": partial_shard.shard_id, "exit_code": 1, "stderr_tail": "synthetic partial"},
    )
    partial_decision = scan_checkpoint_next_shard_state(partial_args, [partial_shard], read_existing_checkpoint(partial_args))
    require("resume partial edge archives for retry", partial_decision["decision"] == "archived_for_retry")
    require("resume partial edge keeps checkpoint unchanged", partial_decision["checkpoint_sha256_unchanged"] is True)
    require("resume partial edge removes source cache root", not partial_shard.cache_root.exists())
    require("resume partial edge removes source FSV root", not partial_shard.fsv_root.exists())
    require("resume partial edge removes source run log", not shard_run_log_path(partial_args, partial_shard).exists())
    append_case("resume_partial_without_manifest_archived", partial_args, partial_shard, partial_decision)

    mismatch_args = clone_resume_edge_args(args, edge_root, "mismatched-completed-proof", active_embedder_count=2)
    mismatch_shard = write_minimal_packed_shard(
        edge_root / "mismatched-completed-proof" / "payload",
        "edge-mismatched-completed-proof",
        500_000_002,
        1,
    )
    write_checkpoint_pair(mismatch_args, checkpoint_for_shard(mismatch_shard))
    mismatch_decision = scan_checkpoint_next_shard_state(
        mismatch_args,
        [mismatch_shard],
        read_existing_checkpoint(mismatch_args),
    )
    require("resume mismatch edge archives for retry", mismatch_decision["decision"] == "archived_for_retry")
    require("resume mismatch edge keeps checkpoint unchanged", mismatch_decision["checkpoint_sha256_unchanged"] is True)
    require("resume mismatch edge removes source cache root", not mismatch_shard.cache_root.exists())
    require("resume mismatch edge removes source FSV root", not mismatch_shard.fsv_root.exists())
    append_case("resume_mismatched_completed_proof_archived", mismatch_args, mismatch_shard, mismatch_decision)

    exact_args = clone_resume_edge_args(
        args,
        edge_root,
        "exact-completed-proof",
        active_embedder_count=1,
        retry_failed_partial_shards=False,
    )
    exact_shard = write_minimal_packed_shard(
        edge_root / "exact-completed-proof" / "payload",
        "edge-exact-completed-proof",
        500_000_003,
        1,
    )
    write_checkpoint_pair(exact_args, checkpoint_for_shard(exact_shard))
    exact_decision = scan_checkpoint_next_shard_state(exact_args, [exact_shard], read_existing_checkpoint(exact_args))
    require("resume exact edge accepts completed proof", exact_decision["decision"] == "exact_resume_proof_accepted")
    require("resume exact edge keeps checkpoint unchanged", exact_decision["checkpoint_sha256_unchanged"] is True)
    require("resume exact edge preserves source cache root", exact_shard.cache_root.exists())
    require("resume exact edge preserves source FSV root", exact_shard.fsv_root.exists())
    append_case("resume_exact_completed_proof_accepted", exact_args, exact_shard, exact_decision)

    blocked_args = clone_resume_edge_args(
        args,
        edge_root,
        "blocked-without-retry",
        active_embedder_count=1,
        retry_failed_partial_shards=False,
    )
    blocked_shard = ShardSpec(
        "edge-blocked-without-retry",
        500_000_004,
        1,
        blocked_args.cache_root / "shards" / "edge-blocked-without-retry",
        blocked_args.fsv_root / "shards" / "edge-blocked-without-retry",
    )
    write_checkpoint_pair(blocked_args, checkpoint_for_shard(blocked_shard))
    blocked_shard.cache_root.mkdir(parents=True)
    blocked_shard.fsv_root.mkdir(parents=True)
    (blocked_shard.cache_root / "python_forward_cache_index.jsonl").write_text("", encoding="utf-8")
    try:
        scan_checkpoint_next_shard_state(blocked_args, [blocked_shard], read_existing_checkpoint(blocked_args))
    except SchedulerError as exc:
        decision_files = sorted((blocked_args.fsv_root / "resume_decisions").glob("*.json"))
        require("resume blocked edge wrote decision file", bool(decision_files))
        blocked_decision = read_json(decision_files[-1])
        require("resume blocked edge records blocked decision", blocked_decision.get("decision") == "blocked_partial_state")
        require("resume blocked edge records failed pass flag", blocked_decision.get("passes") is False)
        cases.append(
            {
                "case": "resume_partial_without_retry_blocks",
                "passes": True,
                "error": str(exc),
                "decision_path": str(decision_files[-1]),
                "decision_sha256": sha256_file(decision_files[-1]),
                "decision": blocked_decision.get("decision"),
                "source_after_state": {
                    "cache_root": tree_state(blocked_shard.cache_root),
                    "fsv_root": tree_state(blocked_shard.fsv_root),
                    "run_log": file_state(shard_run_log_path(blocked_args, blocked_shard)),
                },
            }
        )
    else:
        raise SchedulerError("resume blocked edge unexpectedly passed")

    return cases


def write_resume_preflight_fsv(
    args: argparse.Namespace,
    started_at: str,
    before: dict[str, Any],
    checkpoint: dict[str, Any],
    decision: dict[str, Any],
    edge_cases: list[dict[str, Any]],
) -> dict[str, Any]:
    require("resume preflight decision passes", decision.get("passes") is True)
    require("resume preflight edge cases pass", all(case.get("passes") is True for case in edge_cases))
    cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_resume_preflight_fsv",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "checkpoint_cache": str(cache_checkpoint_path),
            "checkpoint_fsv": str(fsv_checkpoint_path),
            "decision_path": decision["decision_path"],
            "cache_root": str(args.cache_root),
            "fsv_root": str(args.fsv_root),
            "hot_root": str(args.hot_root),
        },
        "before_state": before,
        "checkpoint_progress": checkpoint.get("progress"),
        "checkpoint_summary": checkpoint.get("summary"),
        "decision": decision,
        "edge_cases": edge_cases,
        "after_state": {
            "cache_root": tree_state(args.cache_root),
            "fsv_root": tree_state(args.fsv_root),
            "hot_root": tree_state(args.hot_root),
            "checkpoint_cache": file_state(cache_checkpoint_path),
            "checkpoint_fsv": file_state(fsv_checkpoint_path),
            "disk": disk_snapshot(),
            "resource": resource_snapshot(args, "resume_preflight_after"),
        },
        "passes": True,
    }
    fsv_path = args.fsv_root / "python_forward_resume_preflight_fsv.json"
    json_dump_checked(fsv_path, fsv)
    readback = read_json(fsv_path)
    require("resume preflight FSV readback passes", readback.get("passes") is True)
    return {
        **fsv,
        "fsv_path": str(fsv_path),
        "fsv_sha256": sha256_file(fsv_path),
    }


def count_packed_index_rows(packed_root: Path) -> int:
    if not packed_root.exists():
        return 0
    total = 0
    for path in sorted(packed_root.glob("*.index.jsonl")):
        total += count_jsonl_rows(path)
    return total


def validate_packed_store_manifest(shard: ShardSpec, manifest: dict[str, Any]) -> dict[str, Any]:
    source_of_truth = manifest.get("source_of_truth", {})
    packed_manifest_path = Path(source_of_truth.get("packed_manifest", ""))
    packed_root = Path(source_of_truth.get("packed_record_root", ""))
    require(f"{shard.shard_id} packed manifest path exists", packed_manifest_path.is_file())
    require(f"{shard.shard_id} packed record root exists", packed_root.is_dir())
    packed_manifest = read_json(packed_manifest_path)
    summary = manifest.get("summary", {})
    inputs = manifest.get("inputs", {})
    only_embedder = inputs.get("only_embedder")
    current_run_rows = summary.get("cache_record_count")
    cumulative_rows = summary.get("global_cache_record_count_at_manifest_write", current_run_rows)
    require(
        f"{shard.shard_id} packed current run row count is present",
        isinstance(current_run_rows, int) and current_run_rows >= 0,
    )
    require(
        f"{shard.shard_id} packed cumulative row count is present",
        isinstance(cumulative_rows, int) and cumulative_rows >= current_run_rows,
    )
    require(
        f"{shard.shard_id} packed manifest passes",
        packed_manifest.get("passes") is True,
    )
    require(
        f"{shard.shard_id} packed manifest layout",
        packed_manifest.get("layout") == "packed-jsonl",
    )
    require(
        f"{shard.shard_id} packed row count matches manifest",
        packed_manifest.get("row_count") == cumulative_rows,
    )
    packed_index_rows = count_packed_index_rows(packed_root)
    require(
        f"{shard.shard_id} packed index rows match manifest",
        packed_index_rows == cumulative_rows,
    )
    per_embedder = packed_manifest.get("per_embedder", {})
    if only_embedder:
        require(
            f"{shard.shard_id} only_embedder packed entry exists",
            only_embedder in per_embedder,
        )
        require(
            f"{shard.shard_id} only_embedder packed row count matches current run",
            per_embedder[only_embedder].get("row_count") == current_run_rows,
        )
    for embedder, row in packed_manifest.get("per_embedder", {}).items():
        record_path = Path(row.get("record_path", ""))
        index_path = Path(row.get("index_path", ""))
        require(f"{shard.shard_id} {embedder} packed record path exists", record_path.is_file())
        require(f"{shard.shard_id} {embedder} packed index path exists", index_path.is_file())
        require(
            f"{shard.shard_id} {embedder} packed index row count matches",
            count_jsonl_rows(index_path) == row.get("row_count"),
        )
    return {
        "packed_manifest_path": str(packed_manifest_path),
        "packed_manifest_sha256": sha256_file(packed_manifest_path),
        "packed_record_root": str(packed_root),
        "packed_index_rows": packed_index_rows,
        "current_run_rows": current_run_rows,
        "cumulative_rows": cumulative_rows,
        "packed_manifest": packed_manifest,
    }


def validate_manifest(
    shard: ShardSpec,
    require_packed_store: bool,
    validate_packed_readback: bool = True,
) -> dict[str, Any]:
    manifest = read_json(shard.manifest_path())
    require(f"{shard.shard_id} manifest status passed", manifest.get("status") == "passed")
    require(f"{shard.shard_id} manifest passes true", manifest.get("passes") is True)
    inputs = manifest.get("inputs", {})
    require(
        f"{shard.shard_id} chunk_offset readback",
        inputs.get("chunk_offset") == shard.chunk_offset,
    )
    require(f"{shard.shard_id} max_chunks readback", inputs.get("max_chunks") == shard.max_chunks)
    summary = manifest.get("summary", {})
    require(f"{shard.shard_id} chunk_count positive", summary.get("chunk_count", 0) > 0)
    require(
        f"{shard.shard_id} second pass covers first pass",
        summary.get("second_pass_hits") == summary.get("cache_record_count"),
    )
    index_rows = count_jsonl_rows(shard.index_path())
    quarantine_rows = count_jsonl_rows(shard.quarantine_path())
    batch_telemetry_path = Path(
        manifest.get("source_of_truth", {}).get("batch_telemetry_jsonl", "")
    )
    require(
        f"{shard.shard_id} batch telemetry path exists",
        batch_telemetry_path.is_file(),
    )
    batch_telemetry_rows = count_jsonl_rows(batch_telemetry_path)
    require(
        f"{shard.shard_id} batch telemetry exists when first-pass writes occurred",
        batch_telemetry_rows > 0 or summary.get("first_pass_writes") == 0,
    )
    require(
        f"{shard.shard_id} batch telemetry row count matches manifest",
        batch_telemetry_rows == summary.get("batch_telemetry_row_count"),
    )
    require(
        f"{shard.shard_id} index row count matches manifest",
        index_rows == summary.get("cache_record_count"),
    )
    require(
        f"{shard.shard_id} quarantine row count matches manifest",
        quarantine_rows == summary.get("quarantine_row_count"),
    )
    validate_shard_fsv(shard)
    layout = manifest.get("inputs", {}).get("cache_store_layout", "legacy-files")
    if require_packed_store:
        require(
            f"{shard.shard_id} packed store is required",
            layout == "packed-jsonl",
        )
    if layout == "packed-jsonl" and validate_packed_readback:
        manifest["packed_store_readback"] = validate_packed_store_manifest(shard, manifest)
    return manifest


def write_minimal_packed_shard(root: Path, shard_id: str, chunk_offset: int, max_chunks: int) -> ShardSpec:
    cache_root = root / shard_id
    fsv_root = root / f"{shard_id}-fsv"
    if cache_root.exists():
        shutil.rmtree(cache_root)
    if fsv_root.exists():
        shutil.rmtree(fsv_root)
    packed_root = cache_root / "packed_records"
    packed_root.mkdir(parents=True, exist_ok=True)
    fsv_root.mkdir(parents=True, exist_ok=True)
    record_path = packed_root / "e1.jsonl"
    index_path = packed_root / "e1.index.jsonl"
    record = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": f"sha256:{'1' * 64}",
        "vector_sha256": f"sha256:{'2' * 64}",
        "vector_len": 3,
    }
    jsonl_append_checked(record_path, record)
    index_row = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": record["cache_key_sha256"],
        "embedder_id": "e1",
        "packed_record_path": str(record_path),
        "byte_offset": 0,
        "byte_len": len(record_path.read_bytes().rstrip(b"\n")),
        "record_sha256": sha256_file(record_path),
        "vector_sha256": record["vector_sha256"],
        "vector_len": record["vector_len"],
    }
    jsonl_append_checked(index_path, index_row)
    index_jsonl = cache_root / "python_forward_cache_index.jsonl"
    quarantine_jsonl = cache_root / "python_forward_cache_quarantine.jsonl"
    telemetry_jsonl = cache_root / "python_forward_cache_batch_telemetry.jsonl"
    jsonl_append_checked(index_jsonl, {"cache_key_sha256": record["cache_key_sha256"]})
    quarantine_jsonl.write_text("", encoding="utf-8")
    jsonl_append_checked(telemetry_jsonl, {"batch_id": "edge", "batch_size": 1})
    packed_manifest_path = cache_root / "python_forward_cache_packed_manifest.json"
    json_dump_checked(
        packed_manifest_path,
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_forward_cache_packed_manifest",
            "layout": "packed-jsonl",
            "packed_record_root": str(packed_root),
            "row_count": 1,
            "per_embedder": {
                "e1": {
                    "record_path": str(record_path),
                    "record_sha256": sha256_file(record_path),
                    "byte_count": record_path.stat().st_size,
                    "index_path": str(index_path),
                    "index_sha256": sha256_file(index_path),
                    "row_count": 1,
                }
            },
            "passes": True,
        },
    )
    json_dump_checked(
        cache_root / "python_forward_cache_manifest.json",
        {
            "schema_version": SCHEMA_VERSION,
            "status": "passed",
            "passes": True,
            "inputs": {
                "chunk_offset": chunk_offset,
                "max_chunks": max_chunks,
                "cache_store_layout": "packed-jsonl",
            },
            "source_of_truth": {
                "index_jsonl": str(index_jsonl),
                "quarantine_jsonl": str(quarantine_jsonl),
                "batch_telemetry_jsonl": str(telemetry_jsonl),
                "packed_manifest": str(packed_manifest_path),
                "packed_record_root": str(packed_root),
            },
            "summary": {
                "chunk_count": 1,
                "active_embedder_count": 1,
                "cache_record_count": 1,
                "first_pass_hits": 0,
                "first_pass_writes": 1,
                "second_pass_hits": 1,
                "quarantine_row_count": 0,
                "batch_telemetry_row_count": 1,
            },
        },
    )
    json_dump_checked(fsv_root / "python_forward_cache_fsv.json", {"passes": True})
    return ShardSpec(shard_id, chunk_offset, max_chunks, cache_root, fsv_root)


def write_cumulative_isolated_packed_shard(
    root: Path,
    shard_id: str,
    chunk_offset: int,
    max_chunks: int,
) -> ShardSpec:
    shard = write_minimal_packed_shard(root, shard_id, chunk_offset, max_chunks)
    packed_root = shard.cache_root / "packed_records"
    record_path = packed_root / "e10.jsonl"
    index_path = packed_root / "e10.index.jsonl"
    record = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": f"sha256:{'4' * 64}",
        "vector_sha256": f"sha256:{'5' * 64}",
        "vector_len": 3,
    }
    jsonl_append_checked(record_path, record)
    index_row = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": record["cache_key_sha256"],
        "embedder_id": "e10",
        "packed_record_path": str(record_path),
        "byte_offset": 0,
        "byte_len": len(record_path.read_bytes().rstrip(b"\n")),
        "record_sha256": sha256_file(record_path),
        "vector_sha256": record["vector_sha256"],
        "vector_len": record["vector_len"],
    }
    jsonl_append_checked(index_path, index_row)
    index_jsonl = shard.cache_root / "python_forward_cache_index.jsonl"
    telemetry_jsonl = shard.cache_root / "python_forward_cache_batch_telemetry.jsonl"
    index_jsonl.write_text(json.dumps({"cache_key_sha256": record["cache_key_sha256"]}) + "\n", encoding="utf-8")
    telemetry_jsonl.write_text(json.dumps({"batch_id": "edge-e10", "batch_size": 1}) + "\n", encoding="utf-8")
    packed_manifest = read_json(shard.cache_root / "python_forward_cache_packed_manifest.json")
    packed_manifest["row_count"] = 2
    packed_manifest["per_embedder"]["e10"] = {
        "record_path": str(record_path),
        "record_sha256": sha256_file(record_path),
        "byte_count": record_path.stat().st_size,
        "index_path": str(index_path),
        "index_sha256": sha256_file(index_path),
        "row_count": 1,
    }
    json_dump_checked(shard.cache_root / "python_forward_cache_packed_manifest.json", packed_manifest)
    manifest_path = shard.cache_root / "python_forward_cache_manifest.json"
    manifest = read_json(manifest_path)
    manifest["inputs"]["only_embedder"] = "e10"
    manifest["summary"]["global_cache_record_count_at_manifest_write"] = 2
    json_dump_checked(manifest_path, manifest)
    return shard


def aggregate(shards: list[ShardSpec], output_path: Path, require_packed_store: bool = False) -> dict[str, Any]:
    require("at least one shard in aggregate", bool(shards))
    intervals: list[tuple[int, int, str]] = []
    manifests: list[dict[str, Any]] = []
    for shard in shards:
        end = shard.chunk_offset + shard.max_chunks
        for left, right, other_id in intervals:
            overlaps = shard.chunk_offset < right and left < end
            require(
                f"{shard.shard_id} does not overlap {other_id}",
                not overlaps,
            )
        intervals.append((shard.chunk_offset, end, shard.shard_id))
        manifests.append(validate_manifest(shard, require_packed_store))

    active_embedder_count = manifests[0]["summary"]["active_embedder_count"]
    totals = {
        "chunk_count": 0,
        "cache_record_count": 0,
        "first_pass_hits": 0,
        "first_pass_writes": 0,
        "second_pass_hits": 0,
        "quarantine_row_count": 0,
        "batch_telemetry_row_count": 0,
    }
    shard_rows = []
    for shard, manifest in zip(shards, manifests):
        summary = manifest["summary"]
        require(
            f"{shard.shard_id} active embedder count stable",
            summary["active_embedder_count"] == active_embedder_count,
        )
        for key in totals:
            totals[key] += int(summary[key])
        shard_rows.append(
            {
                "shard_id": shard.shard_id,
                "chunk_offset": shard.chunk_offset,
                "max_chunks": shard.max_chunks,
                "manifest_path": str(shard.manifest_path()),
                "manifest_sha256": sha256_file(shard.manifest_path()),
                "fsv_path": str(shard.fsv_path()),
                "fsv_sha256": sha256_file(shard.fsv_path()),
                "index_rows": count_jsonl_rows(shard.index_path()),
                "quarantine_rows": count_jsonl_rows(shard.quarantine_path()),
                "batch_telemetry_rows": int(summary["batch_telemetry_row_count"]),
                "packed_store_readback": manifest.get("packed_store_readback"),
                "summary": summary,
            }
        )

    expected_pairs = totals["chunk_count"] * active_embedder_count
    accounted_pairs = totals["cache_record_count"] + totals["quarantine_row_count"]
    require("aggregate accounted pairs match expected pairs", accounted_pairs == expected_pairs)
    require("aggregate second pass covers cache records", totals["second_pass_hits"] == totals["cache_record_count"])
    corpus_manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_cache_corpus_manifest",
        "created_at_utc": utc_now(),
        "status": "passed",
        "passes": True,
        "summary": {
            **totals,
            "active_embedder_count": active_embedder_count,
            "expected_pairs": expected_pairs,
            "accounted_pairs": accounted_pairs,
            "shard_count": len(shards),
        },
        "shards": shard_rows,
    }
    json_dump_checked(output_path, corpus_manifest)
    readback = read_json(output_path)
    require("aggregate manifest readback equals written summary", readback["summary"] == corpus_manifest["summary"])
    return corpus_manifest


def preflight_edge_checks(args: argparse.Namespace, preflight: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []

    def expect_preflight_failure(case: str, **overrides: Any) -> None:
        edge_args = argparse.Namespace(**vars(args))
        for key, value in overrides.items():
            setattr(edge_args, key, value)
        try:
            host_preflight(edge_args)
        except SchedulerError as exc:
            cases.append({"case": case, "passes": True, "error": str(exc)})
        else:
            raise SchedulerError(f"{case} edge unexpectedly passed")

    expect_preflight_failure(
        "hot_root_under_mnt_c_rejected_before_writes",
        hot_root=Path("/mnt/c/contextgraph-hot-edge"),
    )
    expect_preflight_failure(
        "d_root_projected_capacity_overflow_rejected_before_writes",
        projected_output_bytes=preflight["host"]["filesystems"]["/mnt/d"]["available_bytes"] + 1,
    )
    expect_preflight_failure(
        "missing_cuda_runtime_rejected_before_writes",
        nvidia_smi_command="/definitely/missing-nvidia-smi-contextgraph",
    )
    expect_preflight_failure(
        "too_many_gpu_workers_rejected_before_writes",
        gpu_workers=preflight["host"]["gpu"]["gpu_count"] + 1,
    )
    expect_preflight_failure(
        "too_many_cpu_workers_rejected_before_writes",
        cpu_workers=preflight["worker_policy"]["cpu_worker_ceiling"] + 1,
    )
    expect_preflight_failure(
        "zero_cpu_workers_rejected_before_writes",
        cpu_workers=0,
    )
    expect_preflight_failure(
        "debug_cargo_run_rejected_before_writes",
        cargo_runner="cargo run -p context-graph-mejepa-embedders --example python_forward_cache_fsv",
    )
    return cases


def edge_checks(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    preflight: dict[str, Any],
) -> list[dict[str, Any]]:
    edge_root = args.fsv_root / "edge_inputs"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = preflight_edge_checks(args, preflight)
    resources = resource_snapshot(args, "edge_resource_snapshot")
    require(
        "resource snapshot records RAM total",
        resources["memory"]["MemTotal_bytes"] > 0,
    )
    require(
        "resource snapshot records available RAM",
        resources["memory"]["MemAvailable_bytes"] > 0,
    )
    require(
        "resource snapshot records scheduler process",
        resources["scheduler_process"]["pid"] == os.getpid(),
    )
    require(
        "resource snapshot records GPU VRAM",
        resources["gpu"]["gpus"][0]["memory_total_bytes"] > 0
        and resources["gpu"]["gpus"][0]["memory_free_bytes"] > 0,
    )
    require(
        "resource snapshot records disk readback",
        resources["disk"]["exit_code"] == 0,
    )
    cases.append({
        "case": "resource_snapshot_records_ram_vram_cpu_disk",
        "passes": True,
        "memory": resources["memory"],
        "scheduler_process": resources["scheduler_process"],
        "gpu": resources["gpu"]["gpus"][0],
        "disk_command": resources["disk"]["command"],
    })

    invalid_configs = [
        ("invalid_e7_batch_config_rejected", {"e7": 64}),
        ("invalid_e12_batch_config_rejected", {"e12": 33}),
        ("invalid_e14_batch_config_rejected", {"e14": 16}),
    ]
    for case_name, row_caps in invalid_configs:
        try:
            validate_embedder_batch_maps(row_caps, {})
        except SchedulerError as exc:
            cases.append({"case": case_name, "passes": True, "error": str(exc)})
        else:
            raise SchedulerError(f"{case_name} edge unexpectedly passed")

    require(
        "isolated process list excludes serial temporal embedders",
        not {"e2", "e3", "e4"}.intersection(ISOLATED_FORWARD_SLUGS),
    )
    require(
        "isolated process list includes CUDA HDC embedder",
        "e9" in ISOLATED_FORWARD_SLUGS,
    )
    cases.append({
        "case": "isolated_process_list_matches_true_batch_capability",
        "passes": True,
        "isolated_forward_slugs": list(ISOLATED_FORWARD_SLUGS),
    })

    missing = ShardSpec(
        shard_id="edge-missing-manifest",
        chunk_offset=999_000_000,
        max_chunks=1,
        cache_root=edge_root / "missing",
        fsv_root=edge_root / "missing-fsv",
    )
    try:
        aggregate([missing], edge_root / "missing-output.json", args.require_packed_store)
    except SchedulerError as exc:
        cases.append({"case": "missing_manifest_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("missing manifest edge unexpectedly passed")

    overlap = ShardSpec(
        shard_id="edge-overlap",
        chunk_offset=shards[0].chunk_offset,
        max_chunks=shards[0].max_chunks,
        cache_root=shards[0].cache_root,
        fsv_root=shards[0].fsv_root,
    )
    try:
        aggregate([shards[0], overlap], edge_root / "overlap-output.json", args.require_packed_store)
    except SchedulerError as exc:
        cases.append({"case": "overlap_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("overlap edge unexpectedly passed")

    if args.require_packed_store:
        corrupt = write_minimal_packed_shard(edge_root, "edge-corrupt-manifest", 100_000_000, 1)
        (corrupt.cache_root / "python_forward_cache_manifest.json").write_text("{not-json", encoding="utf-8")
    else:
        corrupt_root = edge_root / "corrupt-shard"
        if corrupt_root.exists():
            shutil.rmtree(corrupt_root)
        shutil.copytree(shards[0].cache_root, corrupt_root)
        (corrupt_root / "python_forward_cache_manifest.json").write_text("{not-json", encoding="utf-8")
        corrupt = ShardSpec(
            shard_id="edge-corrupt-manifest",
            chunk_offset=shards[0].chunk_offset + 100_000_000,
            max_chunks=shards[0].max_chunks,
            cache_root=corrupt_root,
            fsv_root=shards[0].fsv_root,
        )
    try:
        aggregate([corrupt], edge_root / "corrupt-output.json", args.require_packed_store)
    except SchedulerError as exc:
        cases.append({"case": "corrupt_manifest_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("corrupt manifest edge unexpectedly passed")

    incomplete_offset = shards[0].chunk_offset + 200_000_000
    if args.require_packed_store:
        incomplete = write_minimal_packed_shard(edge_root, "edge-incomplete-shard", incomplete_offset, 1)
        manifest_path = incomplete.cache_root / "python_forward_cache_manifest.json"
        incomplete_manifest = read_json(manifest_path)
        incomplete_manifest["summary"]["second_pass_hits"] = 0
        json_dump_checked(manifest_path, incomplete_manifest)
    else:
        incomplete_root = edge_root / "incomplete-shard"
        if incomplete_root.exists():
            shutil.rmtree(incomplete_root)
        shutil.copytree(shards[0].cache_root, incomplete_root)
        manifest_path = incomplete_root / "python_forward_cache_manifest.json"
        incomplete_manifest = read_json(manifest_path)
        incomplete_manifest["inputs"]["chunk_offset"] = incomplete_offset
        incomplete_manifest["summary"]["second_pass_hits"] -= 1
        json_dump_checked(manifest_path, incomplete_manifest)
        incomplete = ShardSpec(
            shard_id="edge-incomplete-shard",
            chunk_offset=incomplete_offset,
            max_chunks=shards[0].max_chunks,
            cache_root=incomplete_root,
            fsv_root=shards[0].fsv_root,
        )
    try:
        aggregate([incomplete], edge_root / "incomplete-output.json", args.require_packed_store)
    except SchedulerError as exc:
        cases.append({"case": "incomplete_shard_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("incomplete shard edge unexpectedly passed")

    if args.require_packed_store:
        old_layout = write_minimal_packed_shard(edge_root, "edge-old-layout", 300_000_000, 1)
        manifest_path = old_layout.cache_root / "python_forward_cache_manifest.json"
        old_manifest = read_json(manifest_path)
        old_manifest["inputs"]["cache_store_layout"] = "legacy-files"
        del old_manifest["source_of_truth"]["packed_manifest"]
        del old_manifest["source_of_truth"]["packed_record_root"]
        json_dump_checked(manifest_path, old_manifest)
        try:
            aggregate([old_layout], edge_root / "old-layout-output.json", args.require_packed_store)
        except SchedulerError as exc:
            cases.append({"case": "old_per_record_layout_rejected_when_packed_required", "passes": True, "error": str(exc)})
        else:
            raise SchedulerError("old layout edge unexpectedly passed")

        isolated_cumulative = write_cumulative_isolated_packed_shard(
            edge_root,
            "edge-isolated-cumulative-packed",
            350_000_000,
            1,
        )
        isolated_manifest = validate_manifest(isolated_cumulative, args.require_packed_store)
        packed_readback = isolated_manifest["packed_store_readback"]
        require(
            "isolated cumulative packed edge preserves current run rows",
            packed_readback["current_run_rows"] == 1,
        )
        require(
            "isolated cumulative packed edge preserves cumulative rows",
            packed_readback["cumulative_rows"] == 2,
        )
        cases.append({
            "case": "isolated_embedder_cumulative_packed_store_validated",
            "passes": True,
            "current_run_rows": packed_readback["current_run_rows"],
            "cumulative_rows": packed_readback["cumulative_rows"],
        })
        partial_readback_args = argparse.Namespace(**vars(args))
        partial_readback_args.active_embedder_count = max(args.active_embedder_count, 2)
        try:
            completed_shard_readback(partial_readback_args, isolated_cumulative)
        except SchedulerError as exc:
            cases.append({
                "case": "isolated_partial_manifest_rejected_as_completed_shard",
                "passes": True,
                "expected_active_embedder_count": partial_readback_args.active_embedder_count,
                "partial_manifest_active_embedder_count": 1,
                "error": str(exc),
            })
        else:
            raise SchedulerError("isolated partial manifest unexpectedly passed completed shard readback")

        log_filter_root = edge_root / "run-log-filter-fsv"
        log_root = log_filter_root / "run_logs"
        if log_filter_root.exists():
            shutil.rmtree(log_filter_root)
        log_root.mkdir(parents=True)
        canonical_log = {
            "shard_id": "shard-123456-offset-987654321",
            "exit_code": 0,
            "summary": {"active_embedder_count": args.active_embedder_count},
        }
        isolated_log = {
            "shard_id": "shard-123456-offset-987654321",
            "exit_code": 0,
            "summary": {"active_embedder_count": 1},
        }
        json_dump_checked(log_root / "shard-123456-offset-987654321.json", canonical_log)
        json_dump_checked(log_root / "shard-123456-offset-987654321.e1.json", isolated_log)
        loaded_logs = load_existing_run_logs(log_filter_root)
        require("aggregate-only log loader ignores isolated subrun logs", len(loaded_logs) == 1)
        require(
            "aggregate-only log loader preserves canonical shard log",
            loaded_logs[0]["summary"]["active_embedder_count"] == args.active_embedder_count,
        )
        cases.append({
            "case": "aggregate_only_ignores_isolated_subrun_logs",
            "passes": True,
            "loaded_log_count": len(loaded_logs),
        })

    partial = ShardSpec(
        shard_id="edge-partial-archive",
        chunk_offset=400_000_000,
        max_chunks=1,
        cache_root=edge_root / "partial-cache",
        fsv_root=edge_root / "partial-fsv",
    )
    if partial.cache_root.exists():
        shutil.rmtree(partial.cache_root)
    if partial.fsv_root.exists():
        shutil.rmtree(partial.fsv_root)
    partial.cache_root.mkdir(parents=True)
    partial.fsv_root.mkdir(parents=True)
    (partial.cache_root / "python_forward_cache_index.jsonl").write_text(
        json.dumps({"cache_key_sha256": f"sha256:{'3' * 64}"}) + "\n",
        encoding="utf-8",
    )
    json_dump_checked(partial.fsv_root / "partial_marker.json", {"passes": False})
    json_dump_checked(
        shard_run_log_path(args, partial),
        {
            "shard_id": partial.shard_id,
            "exit_code": 1,
            "stderr_tail": "synthetic edge failed before manifest",
        },
    )
    archive = archive_failed_partial_shard(
        args,
        partial,
        "edge synthetic partial shard failed before passed manifest",
    )
    require(
        "partial archive edge removed source cache root",
        not partial.cache_root.exists(),
    )
    require(
        "partial archive edge removed source FSV root",
        not partial.fsv_root.exists(),
    )
    require(
        "partial archive edge removed source run log",
        not shard_run_log_path(args, partial).exists(),
    )
    cases.append({
        "case": "failed_partial_shard_archived_before_retry",
        "passes": True,
        "archive": archive,
    })

    return cases


def main() -> int:
    global TASK_ID
    args = parse_args()
    TASK_ID = args.task_id
    args.embedder_batch_size_map = parse_embedder_int_map(
        args.embedder_batch_sizes,
        "--embedder-batch-sizes",
    )
    args.embedder_max_padded_source_byte_map = parse_embedder_int_map(
        args.embedder_max_padded_source_bytes,
        "--embedder-max-padded-source-bytes",
    )
    shards = build_plan(args)
    preflight = host_preflight(args)
    event_path = args.fsv_root / "scheduler_events.jsonl"
    if args.mode == "run" and event_path.exists():
        event_path.unlink()
    args.cache_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    os.chmod(args.hot_root, 0o700)

    before = {
        "cache_root": tree_state(args.cache_root),
        "fsv_root": tree_state(args.fsv_root),
        "hot_root": tree_state(args.hot_root),
        "disk": disk_snapshot(),
        "resource": resource_snapshot(args, "scheduler_before"),
    }
    started_at = utc_now()
    plan = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_shard_scheduler_plan",
        "created_at_utc": started_at,
        "mode": args.mode,
        "resume_completed_shards": args.resume_completed_shards,
        "retry_failed_partial_shards": args.retry_failed_partial_shards,
        "isolate_embedder_processes": args.isolate_embedder_processes,
        "checkpoint_every_shards": args.checkpoint_every_shards,
        "chunks_jsonl": str(args.chunks_jsonl),
        "cache_root": str(args.cache_root),
        "fsv_root": str(args.fsv_root),
        "hot_root": str(args.hot_root),
        "runner_batch_controls": runner_batch_controls(args),
        "host_preflight": preflight,
        "shards": [
            {
                "shard_id": shard.shard_id,
                "chunk_offset": shard.chunk_offset,
                "max_chunks": shard.max_chunks,
                "cache_root": str(shard.cache_root),
                "fsv_root": str(shard.fsv_root),
            }
            for shard in shards
        ],
    }
    plan_path = args.cache_root / "python_forward_cache_shard_plan.json"
    json_dump_checked(plan_path, plan)
    event(event_path, "plan_written", {"plan_path": str(plan_path), "plan_sha256": sha256_file(plan_path)})

    if args.mode == "preflight-resume":
        checkpoint = read_existing_checkpoint(args)
        decision = scan_checkpoint_next_shard_state(args, shards, checkpoint)
        event(
            event_path,
            "resume_preflight_decision",
            {
                "decision": decision["decision"],
                "decision_path": decision["decision_path"],
                "decision_sha256": decision["decision_sha256"],
                "passes": decision["passes"],
            },
        )
        edge_cases = resume_preflight_edge_checks(args)
        fsv = write_resume_preflight_fsv(args, started_at, before, checkpoint, decision, edge_cases)
        print(json.dumps(fsv, indent=2, sort_keys=True))
        return 0

    run_logs = []
    cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
    if (
        args.mode == "run"
        and args.resume_completed_shards
        and cache_checkpoint_path.exists()
        and fsv_checkpoint_path.exists()
    ):
        checkpoint = read_existing_checkpoint(args)
        event(
            event_path,
            "resume_checkpoint_observed",
            {
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_fsv": file_state(fsv_checkpoint_path),
                "progress": checkpoint.get("progress"),
                "summary": checkpoint.get("summary"),
            },
        )
    else:
        checkpoint = write_checkpoint(args, shards, run_logs, started_at, preflight, "initial")
        event(
            event_path,
            "checkpoint_written",
            {
                "phase": checkpoint["phase"],
                "progress": checkpoint["progress"],
                "summary": checkpoint["summary"],
                "resource": checkpoint["resource"],
            },
        )
    if args.mode == "run":
        for shard in shards:
            if args.resume_completed_shards and (
                shard.cache_root.exists()
                or shard.fsv_root.exists()
                or shard_run_log_path(args, shard).exists()
            ):
                if shard.manifest_path().is_file():
                    try:
                        run_log = completed_shard_readback(args, shard)
                    except SchedulerError as exc:
                        if not args.retry_failed_partial_shards:
                            raise
                        archive = archive_failed_partial_shard(args, shard, str(exc))
                        event(event_path, "failed_partial_shard_archived_for_retry", archive)
                    else:
                        json_dump_checked(shard_run_log_path(args, shard), run_log)
                        event(event_path, "shard_run_skipped_completed", run_log)
                        run_logs.append(run_log)
                        if len(run_logs) % args.checkpoint_every_shards == 0:
                            checkpoint = write_checkpoint(
                                args,
                                shards,
                                run_logs,
                                started_at,
                                preflight,
                                "running",
                            )
                            event(
                                event_path,
                                "checkpoint_written",
                                {
                                    "phase": checkpoint["phase"],
                                    "progress": checkpoint["progress"],
                                    "summary": checkpoint["summary"],
                                    "resource": checkpoint["resource"],
                                },
                            )
                        continue
                else:
                    if not args.retry_failed_partial_shards:
                        raise SchedulerError(
                            f"{shard.shard_id} has partial cache state without a passed manifest: "
                            f"{shard.cache_root}"
                        )
                    archive = archive_failed_partial_shard(
                        args,
                        shard,
                        "partial shard state exists without a manifest that can pass readback",
                    )
                    event(event_path, "failed_partial_shard_archived_for_retry", archive)
            if args.isolate_embedder_processes:
                run_logs.append(run_shard_embedder_isolated(args, shard, event_path, preflight))
            else:
                run_logs.append(run_shard(args, shard, event_path, preflight))
            if len(run_logs) % args.checkpoint_every_shards == 0:
                checkpoint = write_checkpoint(
                    args,
                    shards,
                    run_logs,
                    started_at,
                    preflight,
                    "running",
                )
                event(
                    event_path,
                    "checkpoint_written",
                    {
                        "phase": checkpoint["phase"],
                        "progress": checkpoint["progress"],
                        "summary": checkpoint["summary"],
                        "resource": checkpoint["resource"],
                    },
                )
    else:
        run_logs = load_existing_run_logs(args.fsv_root)
        checkpoint = write_checkpoint(args, shards, run_logs, started_at, preflight, "aggregate_only")
        event(
            event_path,
            "checkpoint_written",
            {
                "phase": checkpoint["phase"],
                "progress": checkpoint["progress"],
                "summary": checkpoint["summary"],
                "resource": checkpoint["resource"],
            },
        )

    aggregate_path = args.cache_root / "python_forward_cache_corpus_manifest.json"
    corpus_manifest = aggregate(shards, aggregate_path, args.require_packed_store)
    checkpoint = write_checkpoint(args, shards, run_logs, started_at, preflight, "final")
    event(
        event_path,
        "checkpoint_written",
        {
            "phase": checkpoint["phase"],
            "progress": checkpoint["progress"],
            "summary": checkpoint["summary"],
            "resource": checkpoint["resource"],
        },
    )
    edge_cases = edge_checks(args, shards, preflight)
    after = {
        "cache_root": tree_state(args.cache_root),
        "fsv_root": tree_state(args.fsv_root),
        "hot_root": tree_state(args.hot_root),
        "disk": disk_snapshot(),
        "resource": resource_snapshot(args, "scheduler_after"),
    }
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_shard_scheduler_fsv",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "plan": str(plan_path),
            "events": str(event_path),
            "aggregate_manifest": str(aggregate_path),
            "checkpoint_cache": str(args.cache_root / "python_forward_cache_checkpoint.json"),
            "checkpoint_fsv": str(args.fsv_root / "python_forward_cache_checkpoint.json"),
            "cache_root": str(args.cache_root),
            "fsv_root": str(args.fsv_root),
            "hot_root": str(args.hot_root),
        },
        "runner_controls": {
            **runner_batch_controls(args),
        },
        "host_preflight": preflight,
        "before_state": before,
        "run_logs": run_logs,
        "checkpoint": checkpoint,
        "aggregate_manifest_sha256": sha256_file(aggregate_path),
        "aggregate_summary": corpus_manifest["summary"],
        "edge_cases": edge_cases,
        "after_state": after,
        "passes": True,
    }
    fsv_path = args.fsv_root / "python_forward_shard_scheduler_fsv.json"
    json_dump_checked(fsv_path, fsv)
    readback = read_json(fsv_path)
    require("scheduler FSV readback passes", readback["passes"] is True)
    print(json.dumps(fsv, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # noqa: BLE001 - CLI must fail loudly with context.
        print(f"PY_FORWARD_SHARD_SCHEDULER_FAILED: {exc}", file=sys.stderr)
        raise
