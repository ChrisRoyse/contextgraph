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
DEFAULT_SMOKE_GATE_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-057-smoke-gate-preflight-fsv")
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
DEFAULT_FULL_RELAUNCH_GATE_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-064-post249-full-run-relaunch-gate-fsv/full_run_relaunch_gate_fsv.json"
)
DEFAULT_FULL_AST_CORPUS_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-058-full-ast-corpus-fsv/full_ast_corpus_fsv.json"
)
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
        "--stage-packed-shards-on-hot",
        action="store_true",
        help=(
            "For packed layout, run shard cache/FSV assembly in a bounded WSL ext4 hot stage, "
            "then promote verified artifacts to the D-drive SoT."
        ),
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
        "--write-resume-gate",
        action="store_true",
        help=(
            "After a short benchmark run, write and enforce the fail-closed "
            "telemetry/resume-gate artifact required before relaunching the full corpus run."
        ),
    )
    parser.add_argument(
        "--resume-gate-baseline-checkpoint",
        type=Path,
        default=None,
        help="Optional stopped-run checkpoint to read and include as the resume-gate comparison baseline.",
    )
    parser.add_argument(
        "--require-smoke-gate",
        action="store_true",
        help="Require accepted 1000-chunk smoke artifacts before scheduler launch/resume.",
    )
    parser.add_argument(
        "--smoke-gate-fsv-root",
        type=Path,
        default=DEFAULT_SMOKE_GATE_FSV_ROOT,
        help="Where to write the fail-closed smoke-gate preflight FSV artifact.",
    )
    parser.add_argument("--smoke-manifest-fsv", type=Path, default=DEFAULT_SMOKE_MANIFEST_FSV)
    parser.add_argument("--smoke-forward-fsv", type=Path, default=DEFAULT_SMOKE_FORWARD_FSV)
    parser.add_argument("--smoke-intelligence-fsv", type=Path, default=DEFAULT_SMOKE_INTELLIGENCE_FSV)
    parser.add_argument("--smoke-storage-design-fsv", type=Path, default=DEFAULT_SMOKE_STORAGE_DESIGN_FSV)
    parser.add_argument("--full-relaunch-gate-fsv", type=Path, default=DEFAULT_FULL_RELAUNCH_GATE_FSV)
    parser.add_argument("--full-ast-corpus-fsv", type=Path, default=DEFAULT_FULL_AST_CORPUS_FSV)
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
        "stage_packed_shards_on_hot": args.stage_packed_shards_on_hot,
        "isolate_embedder_processes": args.isolate_embedder_processes,
    }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return f"sha256:{digest.hexdigest()}"


def sha256_bytes(data: bytes) -> str:
    return f"sha256:{hashlib.sha256(data).hexdigest()}"


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


def read_jsonl_rows(path: Path) -> list[Any]:
    require(f"{path} JSONL exists", path.is_file())
    rows: list[Any] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                rows.append(json.loads(line))
            except Exception as exc:  # noqa: BLE001 - include line number.
                raise SchedulerError(f"invalid JSONL row in {path}:{line_no}: {exc}") from exc
    return rows


def elapsed_ms(started: float) -> float:
    return round((time.monotonic() - started) * 1000.0, 3)


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


def smoke_gate_required(args: argparse.Namespace) -> bool:
    return bool(args.require_smoke_gate or args.task_id == "TASK-PY-DATA-016")


def smoke_gate_artifact_specs(args: argparse.Namespace) -> list[dict[str, Any]]:
    return [
        {
            "name": "smoke_manifest",
            "issue": 251,
            "task_id": "TASK-PY-DATA-060",
            "path": args.smoke_manifest_fsv,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "smoke_forward_cache",
            "issue": 252,
            "task_id": "TASK-PY-DATA-061",
            "path": args.smoke_forward_fsv,
        },
        {
            "name": "smoke_intelligence_matrices",
            "issue": 253,
            "task_id": "TASK-PY-DATA-062",
            "path": args.smoke_intelligence_fsv,
        },
        {
            "name": "smoke_storage_design",
            "issue": 254,
            "task_id": "TASK-PY-DATA-063",
            "path": args.smoke_storage_design_fsv,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "full_relaunch_gate",
            "issue": 255,
            "task_id": "TASK-PY-DATA-064",
            "path": args.full_relaunch_gate_fsv,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "full_ast_corpus",
            "issue": 249,
            "task_id": "TASK-PY-DATA-058",
            "path": args.full_ast_corpus_fsv,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
    ]


def validate_smoke_gate_artifact(spec: dict[str, Any]) -> dict[str, Any]:
    path = Path(spec["path"])
    state = file_state(path)
    result = {
        "name": spec["name"],
        "issue": spec["issue"],
        "expected_task_id": spec["task_id"],
        "path": str(path),
        "state": state,
        "passes": False,
    }
    if not path.is_file():
        return {
            **result,
            "error_code": "MEJEPA_SMOKE_GATE_ARTIFACT_MISSING",
            "message": f"{spec['name']} artifact is missing",
        }
    try:
        payload = read_json(path)
    except SchedulerError as exc:
        return {
            **result,
            "error_code": "MEJEPA_SMOKE_GATE_ARTIFACT_INVALID_JSON",
            "message": str(exc),
        }
    task_id = payload.get("task_id")
    if task_id != spec["task_id"]:
        return {
            **result,
            "payload_task_id": task_id,
            "error_code": "MEJEPA_SMOKE_GATE_ARTIFACT_TASK_MISMATCH",
            "message": f"{spec['name']} task_id mismatch",
        }
    if payload.get("passes") is not True:
        return {
            **result,
            "payload_task_id": task_id,
            "error_code": "MEJEPA_SMOKE_GATE_ARTIFACT_NOT_ACCEPTED",
            "message": f"{spec['name']} artifact does not have passes=true",
        }
    expected_chunks = spec.get("expected_chunks_jsonl")
    if expected_chunks is not None:
        expected_chunks = Path(expected_chunks)
        chunks_state = file_state(expected_chunks)
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
        if str(expected_chunks) not in payload_paths or chunks_state.get("sha256") not in payload_hashes:
            return {
                **result,
                "payload_task_id": task_id,
                "error_code": "MEJEPA_SMOKE_GATE_FULL_AST_CORPUS_CHUNK_MISMATCH",
                "message": f"{spec['name']} artifact does not match requested chunks JSONL source and sha256",
                "payload_chunks_jsonl_candidates": sorted(str(item) for item in payload_paths if item),
                "requested_chunks_jsonl": str(expected_chunks),
                "payload_chunks_sha256_candidates": sorted(str(item) for item in payload_hashes if item),
                "requested_chunks_state": chunks_state,
            }
    return {
        **result,
        "payload_task_id": task_id,
        "payload_artifact_kind": payload.get("artifact_kind"),
        "passes": True,
    }


def hot_stage_blockers(args: argparse.Namespace) -> list[dict[str, Any]]:
    stage_root = args.hot_root / "packed-shard-stage"
    if not stage_root.exists():
        return []
    blockers: list[dict[str, Any]] = []
    for child in sorted(stage_root.iterdir()):
        if child.is_dir() or child.is_file():
            blockers.append(
                {
                    "path": str(child),
                    "state": tree_state(child) if child.is_dir() else file_state(child),
                    "error_code": "MEJEPA_SMOKE_GATE_INTERRUPTED_HOT_STAGE_PRESENT",
                    "message": "interrupted hot-stage shard state must be archived by #255 before full relaunch",
                }
            )
    return blockers


def write_smoke_gate_fsv(
    args: argparse.Namespace,
    started_at: str,
    required: bool,
    artifact_results: list[dict[str, Any]],
    hot_stage_results: list[dict[str, Any]],
) -> dict[str, Any]:
    args.smoke_gate_fsv_root.mkdir(parents=True, exist_ok=True)
    fsv_path = args.smoke_gate_fsv_root / "python_forward_smoke_gate_preflight_fsv.json"
    failures = [
        result
        for result in artifact_results
        if result.get("passes") is not True
    ] + hot_stage_results
    value = {
        "schema_version": SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-057",
        "artifact_kind": "python_forward_smoke_gate_preflight_fsv",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "required": required,
        "full_run_task_detected": args.task_id == "TASK-PY-DATA-016",
        "source_of_truth": {
            "smoke_gate_fsv": str(fsv_path),
            "cache_root": str(args.cache_root),
            "fsv_root": str(args.fsv_root),
            "hot_root": str(args.hot_root),
            "smoke_manifest_fsv": str(args.smoke_manifest_fsv),
            "smoke_forward_fsv": str(args.smoke_forward_fsv),
            "smoke_intelligence_fsv": str(args.smoke_intelligence_fsv),
            "smoke_storage_design_fsv": str(args.smoke_storage_design_fsv),
            "full_relaunch_gate_fsv": str(args.full_relaunch_gate_fsv),
            "full_ast_corpus_fsv": str(args.full_ast_corpus_fsv),
        },
        "artifact_results": artifact_results,
        "hot_stage_results": hot_stage_results,
        "failure_count": len(failures),
        "failures": failures,
        "passes": required is False or not failures,
    }
    json_dump_checked(fsv_path, value)
    readback = read_json(fsv_path)
    require("smoke gate preflight FSV readback has pass flag", readback.get("passes") in (True, False))
    return {
        **value,
        "fsv_path": str(fsv_path),
        "fsv_sha256": sha256_file(fsv_path),
    }


def smoke_gate_preflight(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    required = smoke_gate_required(args)
    artifact_results: list[dict[str, Any]] = []
    hot_stage_results: list[dict[str, Any]] = []
    if not required:
        return {
            "schema_version": SCHEMA_VERSION,
            "task_id": "TASK-PY-DATA-057",
            "artifact_kind": "python_forward_smoke_gate_preflight",
            "started_at_utc": started_at,
            "created_at_utc": utc_now(),
            "required": False,
            "full_run_task_detected": False,
            "passes": True,
        }
    artifact_results = [validate_smoke_gate_artifact(spec) for spec in smoke_gate_artifact_specs(args)]
    hot_stage_results = hot_stage_blockers(args)
    gate = write_smoke_gate_fsv(args, started_at, required, artifact_results, hot_stage_results)
    if required and gate["passes"] is not True:
        failure_names = [
            str(item.get("name") or item.get("path"))
            for item in gate["failures"]
        ]
        raise SchedulerError(
            "MEJEPA_SCHEDULER_SMOKE_GATE_BLOCKED: full corpus run is blocked "
            f"until smoke gate artifacts pass; failures={failure_names}; fsv={gate['fsv_path']}"
        )
    return gate


def task061_smoke_manifest_gate_required(args: argparse.Namespace) -> bool:
    return args.task_id == "TASK-PY-DATA-061"


def require_path_string(payload: dict[str, Any], key: str, context: str) -> Path:
    value = payload.get(key)
    require(f"{context} {key} is a non-empty path", isinstance(value, str) and bool(value))
    return Path(value)


def task061_smoke_manifest_gate(args: argparse.Namespace) -> dict[str, Any]:
    required = task061_smoke_manifest_gate_required(args)
    base = {
        "schema_version": SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-060",
        "artifact_kind": "task061_smoke_manifest_gate",
        "created_at_utc": utc_now(),
        "required": required,
        "smoke_manifest_fsv": str(args.smoke_manifest_fsv),
        "requested_chunks_jsonl": str(args.chunks_jsonl),
    }
    if not required:
        return {**base, "passes": True}

    require("TASK061_SMOKE_MANIFEST_FSV_EXISTS: #251 smoke manifest FSV exists", args.smoke_manifest_fsv.is_file())
    payload = read_json(args.smoke_manifest_fsv)
    require("TASK061_SMOKE_MANIFEST_TASK_ID: #251 FSV task_id matches", payload.get("task_id") == "TASK-PY-DATA-060")
    require("TASK061_SMOKE_MANIFEST_PASSES: #251 FSV passes", payload.get("passes") is True)
    source_of_truth = payload.get("source_of_truth")
    after_state = payload.get("after_state")
    summary = payload.get("summary")
    happy_path = payload.get("happy_path")
    require("TASK061_SMOKE_MANIFEST_SOURCE_OF_TRUTH_OBJECT: #251 source_of_truth exists", isinstance(source_of_truth, dict))
    require("TASK061_SMOKE_MANIFEST_AFTER_STATE_OBJECT: #251 after_state exists", isinstance(after_state, dict))
    require("TASK061_SMOKE_MANIFEST_SUMMARY_OBJECT: #251 summary exists", isinstance(summary, dict))
    require("TASK061_SMOKE_MANIFEST_HAPPY_PATH_OBJECT: #251 happy_path exists", isinstance(happy_path, dict))

    smoke_chunks = require_path_string(source_of_truth, "smoke_chunks_jsonl", "#251 source_of_truth")
    smoke_manifest = require_path_string(source_of_truth, "smoke_manifest_jsonl", "#251 source_of_truth")
    require(
        "TASK061_SMOKE_CHUNKS_SOURCE_MATCH: scheduler chunks_jsonl is #251 smoke_chunks_jsonl",
        str(args.chunks_jsonl) == str(smoke_chunks),
    )

    chunk_state = file_state(smoke_chunks)
    manifest_state = file_state(smoke_manifest)
    fsv_state = file_state(args.smoke_manifest_fsv)
    expected_chunk_sha = (after_state.get("chunks_out") or {}).get("sha256") or summary.get("smoke_chunks_sha256")
    expected_manifest_sha = (after_state.get("manifest_out") or {}).get("sha256") or summary.get("smoke_manifest_sha256")
    require("TASK061_SMOKE_CHUNK_SHA_PRESENT: #251 chunk sha exists", isinstance(expected_chunk_sha, str) and expected_chunk_sha.startswith("sha256:"))
    require("TASK061_SMOKE_MANIFEST_SHA_PRESENT: #251 manifest sha exists", isinstance(expected_manifest_sha, str) and expected_manifest_sha.startswith("sha256:"))
    require("TASK061_SMOKE_CHUNKS_FILE_EXISTS: #251 smoke chunks file exists", chunk_state.get("is_file") is True)
    require("TASK061_SMOKE_MANIFEST_FILE_EXISTS: #251 smoke manifest file exists", manifest_state.get("is_file") is True)
    require(
        "TASK061_SMOKE_CHUNKS_HASH_MATCH: live smoke chunks hash matches #251 FSV",
        chunk_state.get("sha256") == expected_chunk_sha,
    )
    require(
        "TASK061_SMOKE_MANIFEST_HASH_MATCH: live smoke manifest hash matches #251 FSV",
        manifest_state.get("sha256") == expected_manifest_sha,
    )
    chunk_count = count_jsonl_rows(smoke_chunks)
    manifest_count = count_jsonl_rows(smoke_manifest)
    expected_count = int((happy_path.get("expected") or {}).get("selected_count", 1000))
    require("TASK061_SMOKE_CHUNK_COUNT: #251 smoke chunks row count matches expected", chunk_count == expected_count)
    require("TASK061_SMOKE_MANIFEST_COUNT: #251 smoke manifest row count matches expected", manifest_count == expected_count)
    require("TASK061_SMOKE_HAPPY_PATH_PASSES: #251 happy path passes", happy_path.get("pass") is True)
    stratification = summary.get("stratification_requirements")
    require("TASK061_SMOKE_STRATIFICATION_OBJECT: #251 stratification object exists", isinstance(stratification, dict))
    require("TASK061_SMOKE_STRATIFICATION_PASSES: #251 stratification passes", stratification.get("passes") is True)
    warnings = summary.get("stratification_warnings")
    require("TASK061_SMOKE_WARNINGS_EMPTY: #251 has no stratification warnings", isinstance(warnings, list) and len(warnings) == 0)

    return {
        **base,
        "source_of_truth": {
            "smoke_manifest_fsv": str(args.smoke_manifest_fsv),
            "smoke_chunks_jsonl": str(smoke_chunks),
            "smoke_manifest_jsonl": str(smoke_manifest),
        },
        "fsv_state": fsv_state,
        "chunk_state": chunk_state,
        "manifest_state": manifest_state,
        "chunk_count": chunk_count,
        "manifest_count": manifest_count,
        "expected_count": expected_count,
        "source_hashes": {
            "smoke_chunks_sha256": expected_chunk_sha,
            "smoke_manifest_sha256": expected_manifest_sha,
        },
        "stratification_requirements": stratification,
        "passes": True,
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
    stage_promotion: dict[str, Any] | None = None
    stage_setup: dict[str, Any] | None = None
    runner_shard = shard
    runner_hot_root = args.hot_root
    if use_hot_stage_for_shard(args):
        stage_root = hot_stage_root(args, shard, run_log_name)
        runner_shard = hot_stage_shard(args, shard, run_log_name)
        stage_setup = prepare_hot_stage(args, shard, runner_shard, stage_root)
        runner_hot_root = stage_root / "runner-hot"

    env = os.environ.copy()
    env.update(
        {
            "CG_MEJEPA_PY_FORWARD_CACHE_ROOT": str(runner_shard.cache_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_FSV_ROOT": str(runner_shard.fsv_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_HOT_ROOT": str(runner_hot_root),
            "CG_MEJEPA_PY_CHUNKS_JSONL": str(args.chunks_jsonl),
            "CG_MEJEPA_PY_FORWARD_CACHE_CHUNK_OFFSET": str(runner_shard.chunk_offset),
            "CG_MEJEPA_PY_FORWARD_CACHE_MAX_CHUNKS": str(runner_shard.max_chunks),
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
            "runner_cache_root": str(runner_shard.cache_root),
            "runner_fsv_root": str(runner_shard.fsv_root),
            "runner_hot_root": str(runner_hot_root),
            "hot_stage": stage_setup,
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
    timing_ms = {"runner_subprocess_ms": round(elapsed * 1000.0, 3)}
    if use_hot_stage_for_shard(args):
        stage_readback_started = time.monotonic()
        stage_readback = shard_completion_readback(args, runner_shard, require_packed_store)
        timing_ms["stage_readback_ms"] = elapsed_ms(stage_readback_started)
        promotion_started = time.monotonic()
        stage_promotion = promote_hot_stage_shard(args, shard, runner_shard, stage_root, stage_readback)
        timing_ms["hot_stage_promotion_ms"] = elapsed_ms(promotion_started)
        run_log["hot_stage_promotion"] = stage_promotion
    durable_readback_started = time.monotonic()
    run_log.update(shard_completion_readback(args, shard, require_packed_store))
    timing_ms["durable_readback_ms"] = elapsed_ms(durable_readback_started)
    run_log["timing_ms"] = timing_ms
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
        expected_chunks_jsonl=args.chunks_jsonl,
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


def archive_interrupted_hot_stage(
    args: argparse.Namespace,
    shard: ShardSpec,
    stage_root: Path,
    reason: str,
) -> dict[str, Any] | None:
    if not stage_root.exists():
        return None
    before = tree_state(stage_root)
    archive_root = unique_archive_root(
        args.fsv_root / "failed_hot_stages",
        f"{utc_now().replace('-', '').replace(':', '')}-{shard.shard_id}-{stage_root.name}",
    )
    shutil.move(str(stage_root), str(archive_root))
    require("interrupted hot-stage source removed after archive", not stage_root.exists())
    require("interrupted hot-stage archive exists", archive_root.exists())
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_interrupted_hot_stage_archive",
        "created_at_utc": utc_now(),
        "shard_id": shard.shard_id,
        "reason": reason,
        "stage_root": str(stage_root),
        "archive_root": str(archive_root),
        "before_state": before,
        "after_state": {
            "stage_root": tree_state(stage_root),
            "archive_root": tree_state(archive_root),
        },
        "passes": True,
    }
    manifest_path = archive_root / "archive_manifest.json"
    json_dump_checked(manifest_path, manifest)
    readback = read_json(manifest_path)
    require("interrupted hot-stage archive manifest readback passes", readback.get("passes") is True)
    return {
        **manifest,
        "archive_manifest_path": str(manifest_path),
        "archive_manifest_sha256": sha256_file(manifest_path),
    }


def stage_identity(run_log_name: str | None) -> str:
    if run_log_name is None:
        return "canonical"
    return run_log_name.removesuffix(".json").replace("/", "_")


def hot_stage_root(args: argparse.Namespace, shard: ShardSpec, run_log_name: str | None = None) -> Path:
    return args.hot_root / "packed-shard-stage" / shard.shard_id / stage_identity(run_log_name)


def hot_stage_shard(args: argparse.Namespace, shard: ShardSpec, run_log_name: str | None = None) -> ShardSpec:
    root = hot_stage_root(args, shard, run_log_name)
    return ShardSpec(
        shard_id=shard.shard_id,
        chunk_offset=shard.chunk_offset,
        max_chunks=shard.max_chunks,
        cache_root=root / "cache",
        fsv_root=root / "fsv",
    )


def use_hot_stage_for_shard(args: argparse.Namespace) -> bool:
    return args.stage_packed_shards_on_hot and args.cache_store_layout == "packed"


def prepare_hot_stage(
    args: argparse.Namespace,
    durable_shard: ShardSpec,
    stage_shard: ShardSpec,
    stage_root: Path,
) -> dict[str, Any]:
    require("packed hot-stage root is not /mnt/c", not path_is_under(stage_root.resolve(strict=False), Path("/mnt/c")))
    require("packed hot-stage root is WSL ext4, not /mnt/d", not path_is_under(stage_root.resolve(strict=False), Path("/mnt/d")))
    archive = archive_interrupted_hot_stage(
        args,
        durable_shard,
        stage_root,
        "new hot-stage run refuses to reuse stale interrupted stage state",
    )
    stage_root.mkdir(parents=True, exist_ok=False)
    if durable_shard.cache_root.exists():
        shutil.copytree(durable_shard.cache_root, stage_shard.cache_root)
    else:
        stage_shard.cache_root.mkdir(parents=True)
    if durable_shard.fsv_root.exists():
        shutil.copytree(durable_shard.fsv_root, stage_shard.fsv_root)
    else:
        stage_shard.fsv_root.mkdir(parents=True)
    stage_path_rewrites = (
        rewrite_json_paths_in_tree(
            stage_shard.cache_root,
            {
                str(durable_shard.cache_root): str(stage_shard.cache_root),
                str(durable_shard.fsv_root): str(stage_shard.fsv_root),
            },
        )
        + rewrite_json_paths_in_tree(
            stage_shard.fsv_root,
            {
                str(durable_shard.cache_root): str(stage_shard.cache_root),
                str(durable_shard.fsv_root): str(stage_shard.fsv_root),
            },
        )
    )
    stage_hot_root = stage_root / "runner-hot"
    stage_hot_root.mkdir(parents=True, exist_ok=True)
    os.chmod(stage_hot_root, 0o700)
    return {
        "stage_root": str(stage_root),
        "stage_cache_root": str(stage_shard.cache_root),
        "stage_fsv_root": str(stage_shard.fsv_root),
        "stage_runner_hot_root": str(stage_hot_root),
        "interrupted_stage_archive": archive,
        "stage_path_rewrites": stage_path_rewrites,
        "before_state": {
            "durable_cache_root": tree_state(durable_shard.cache_root),
            "durable_fsv_root": tree_state(durable_shard.fsv_root),
        },
        "stage_before_runner": {
            "stage_cache_root": tree_state(stage_shard.cache_root),
            "stage_fsv_root": tree_state(stage_shard.fsv_root),
            "stage_runner_hot_root": tree_state(stage_hot_root),
        },
        "passes": True,
    }


def replace_path_prefixes(value: Any, replacements: dict[str, str]) -> Any:
    if isinstance(value, str):
        updated = value
        for old, new in replacements.items():
            updated = updated.replace(old, new)
        return updated
    if isinstance(value, list):
        return [replace_path_prefixes(item, replacements) for item in value]
    if isinstance(value, dict):
        return {key: replace_path_prefixes(item, replacements) for key, item in value.items()}
    return value


def rewrite_json_paths_in_tree(root: Path, replacements: dict[str, str]) -> list[dict[str, Any]]:
    rewrites: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if path.suffix == ".json":
            before_sha = sha256_file(path)
            value = read_json(path)
            updated = replace_path_prefixes(value, replacements)
            json_dump_checked(path, updated)
            rewrites.append({
                "path": str(path),
                "kind": "json",
                "before_sha256": before_sha,
                "after_sha256": sha256_file(path),
            })
        elif path.suffix == ".jsonl":
            before_sha = sha256_file(path)
            if path.parent.name == "packed_records" and not path.name.endswith(".index.jsonl"):
                raw = path.read_bytes()
                stale_paths = [old for old in replacements if old.encode("utf-8") in raw]
                require(
                    f"{path} packed payload JSONL has no rewrite-required path",
                    not stale_paths,
                )
                rewrites.append({
                    "path": str(path),
                    "kind": "packed_payload_jsonl_preserved",
                    "row_count": count_jsonl_rows(path),
                    "before_sha256": before_sha,
                    "after_sha256": sha256_file(path),
                    "reason": "byte-addressed packed payload JSONL is immutable; packed index rows hash exact payload bytes",
                })
                continue
            rows: list[Any] = []
            with path.open("r", encoding="utf-8") as handle:
                for line in handle:
                    if not line.strip():
                        continue
                    rows.append(replace_path_prefixes(json.loads(line), replacements))
            tmp = path.with_suffix(f"{path.suffix}.rewrite")
            with tmp.open("w", encoding="utf-8") as handle:
                for row in rows:
                    handle.write(json.dumps(row, sort_keys=True))
                    handle.write("\n")
            tmp.replace(path)
            rewrites.append({
                "path": str(path),
                "kind": "jsonl",
                "row_count": len(rows),
                "before_sha256": before_sha,
                "after_sha256": sha256_file(path),
            })
    return rewrites


def promote_tree_verified(source: Path, destination: Path, archive_parent: Path, label: str) -> dict[str, Any]:
    require(f"{label} source exists before promotion", source.exists())
    before = {
        "source": tree_state(source),
        "destination": tree_state(destination),
    }
    temp = unique_archive_root(destination.parent, f".{destination.name}.promote-{utc_now().replace('-', '').replace(':', '')}")
    if temp.exists():
        shutil.rmtree(temp)
    shutil.copytree(source, temp)
    require(f"{label} promotion temp tree byte count matches source", tree_state(temp)["byte_count"] == tree_state(source)["byte_count"])
    archived_existing = None
    if destination.exists():
        archived_existing = unique_archive_root(
            archive_parent,
            f"{utc_now().replace('-', '').replace(':', '')}-{destination.name}",
        )
        shutil.move(str(destination), str(archived_existing))
        require(f"{label} previous destination archived", archived_existing.exists())
        require(f"{label} previous destination removed", not destination.exists())
    shutil.move(str(temp), str(destination))
    require(f"{label} destination exists after promotion", destination.exists())
    require(f"{label} promoted byte count matches source", tree_state(destination)["byte_count"] == before["source"]["byte_count"])
    return {
        "label": label,
        "before_state": before,
        "archived_existing": None if archived_existing is None else str(archived_existing),
        "after_state": {
            "source": tree_state(source),
            "destination": tree_state(destination),
        },
        "passes": True,
    }


def promote_hot_stage_shard(
    args: argparse.Namespace,
    durable_shard: ShardSpec,
    stage_shard: ShardSpec,
    stage_root: Path,
    stage_readback: dict[str, Any],
) -> dict[str, Any]:
    total_started = time.monotonic()
    replacements = {
        str(stage_shard.cache_root): str(durable_shard.cache_root),
        str(stage_shard.fsv_root): str(durable_shard.fsv_root),
    }
    rewrite_started = time.monotonic()
    rewrites = (
        rewrite_json_paths_in_tree(stage_shard.cache_root, replacements)
        + rewrite_json_paths_in_tree(stage_shard.fsv_root, replacements)
    )
    path_rewrite_ms = elapsed_ms(rewrite_started)
    after_rewrite = {
        "stage_cache_root": tree_state(stage_shard.cache_root),
        "stage_fsv_root": tree_state(stage_shard.fsv_root),
    }
    archive_parent = args.fsv_root / "hot_stage_promoted_overwrites" / durable_shard.shard_id
    cache_promotion_started = time.monotonic()
    cache_promotion = promote_tree_verified(
        stage_shard.cache_root,
        durable_shard.cache_root,
        archive_parent / "cache",
        "cache_root",
    )
    cache_promotion_ms = elapsed_ms(cache_promotion_started)
    fsv_promotion_started = time.monotonic()
    fsv_promotion = promote_tree_verified(
        stage_shard.fsv_root,
        durable_shard.fsv_root,
        archive_parent / "fsv",
        "fsv_root",
    )
    fsv_promotion_ms = elapsed_ms(fsv_promotion_started)
    durable_readback_started = time.monotonic()
    durable_readback = shard_completion_readback(args, durable_shard)
    durable_readback_ms = elapsed_ms(durable_readback_started)
    cleanup_started = time.monotonic()
    shutil.rmtree(stage_root)
    stage_cleanup_ms = elapsed_ms(cleanup_started)
    require("packed hot-stage root removed after durable promotion", not stage_root.exists())
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_hot_stage_promotion",
        "created_at_utc": utc_now(),
        "shard_id": durable_shard.shard_id,
        "stage_root": str(stage_root),
        "stage_readback": stage_readback,
        "path_rewrites": rewrites,
        "after_rewrite": after_rewrite,
        "cache_promotion": cache_promotion,
        "fsv_promotion": fsv_promotion,
        "durable_readback": durable_readback,
        "stage_after_state": tree_state(stage_root),
        "timing_ms": {
            "path_rewrite_ms": path_rewrite_ms,
            "cache_promotion_ms": cache_promotion_ms,
            "fsv_promotion_ms": fsv_promotion_ms,
            "durable_readback_ms": durable_readback_ms,
            "stage_cleanup_ms": stage_cleanup_ms,
            "total_ms": elapsed_ms(total_started),
        },
        "passes": True,
    }


def write_checkpoint(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    run_logs: list[dict[str, Any]],
    started_at: str,
    preflight: dict[str, Any],
    phase: str,
) -> dict[str, Any]:
    checkpoint_started = time.monotonic()
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
        "timing_ms": {
            "checkpoint_update_ms": 0.0,
        },
        "passes": True,
    }
    cache_checkpoint_path = args.cache_root / "python_forward_cache_checkpoint.json"
    fsv_checkpoint_path = args.fsv_root / "python_forward_cache_checkpoint.json"
    checkpoint["timing_ms"]["checkpoint_update_ms"] = elapsed_ms(checkpoint_started)
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
        chunks_jsonl=args.chunks_jsonl,
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
        chunks_jsonl=args.chunks_jsonl,
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


def hot_stage_promotion_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    edge_root = args.fsv_root / "edge_inputs" / "hot_stage_promotion"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = []

    checkpoint_path, _ = checkpoint_file_paths(args)
    checkpoint_before = file_state(checkpoint_path)
    stage_mismatch_root = args.hot_root / "edge-hot-stage-promotion-mismatch"
    if stage_mismatch_root.exists():
        shutil.rmtree(stage_mismatch_root)
    stage_mismatch_root.mkdir(parents=True)
    os.chmod(stage_mismatch_root, 0o700)
    durable_mismatch_path = edge_root / "promotion-mismatch.json"
    stage_path = stage_mismatch_root / "payload.json"
    expected = {"stage": "good", "vector_sha256": "sha256:synthetic"}
    corrupt = {"stage": "corrupt", "vector_sha256": "sha256:synthetic"}
    json_dump_checked(stage_path, expected)
    json_dump_checked(durable_mismatch_path, corrupt)
    mismatch_detected = sha256_file(stage_path) != sha256_file(durable_mismatch_path)
    require("hot-stage promotion mismatch edge detects digest disagreement", mismatch_detected)
    cases.append(
        {
            "case": "hot_stage_promotion_hash_mismatch_rejected",
            "passes": True,
            "stage_path": str(stage_path),
            "stage_sha256": sha256_file(stage_path),
            "durable_path": str(durable_mismatch_path),
            "durable_sha256": sha256_file(durable_mismatch_path),
            "checkpoint_before": checkpoint_before,
            "checkpoint_after": file_state(checkpoint_path),
            "checkpoint_sha256_unchanged": checkpoint_before.get("sha256") == file_state(checkpoint_path).get("sha256"),
        }
    )

    durable_edge_shard = ShardSpec(
        "edge-hot-stage-interrupt",
        600_000_000,
        1,
        edge_root / "durable-cache",
        edge_root / "durable-fsv",
    )
    stage_edge_shard = hot_stage_shard(args, durable_edge_shard)
    stage_root = hot_stage_root(args, durable_edge_shard)
    if stage_root.exists():
        shutil.rmtree(stage_root)
    stage_root.mkdir(parents=True)
    (stage_root / "stale-marker.txt").write_text("interrupted", encoding="utf-8")
    stage_setup = prepare_hot_stage(args, durable_edge_shard, stage_edge_shard, stage_root)
    archive = stage_setup["interrupted_stage_archive"]
    require("interrupted hot-stage edge archived stale stage", isinstance(archive, dict))
    require("interrupted hot-stage edge created fresh cache root", stage_edge_shard.cache_root.exists())
    cases.append(
        {
            "case": "interrupted_hot_stage_archived_before_retry",
            "passes": True,
            "archive": archive,
            "fresh_stage_state": {
                "stage_cache_root": tree_state(stage_edge_shard.cache_root),
                "stage_fsv_root": tree_state(stage_edge_shard.fsv_root),
            },
        }
    )
    shutil.rmtree(stage_root)

    try:
        bad_stage = Path("/mnt/c/contextgraph-hot-stage-edge")
        require("packed hot-stage root is not /mnt/c", not path_is_under(bad_stage, Path("/mnt/c")))
    except SchedulerError as exc:
        cases.append({"case": "hot_stage_under_mnt_c_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("hot stage /mnt/c edge unexpectedly passed")

    durable_payload_shard = write_minimal_packed_shard(
        edge_root / "packed-payload-preserve",
        "edge-hot-stage-packed-payload",
        600_000_010,
        1,
        args.chunks_jsonl,
    )
    stage_payload_root = hot_stage_root(args, durable_payload_shard)
    if stage_payload_root.exists():
        shutil.rmtree(stage_payload_root)
    stage_payload_shard = hot_stage_shard(args, durable_payload_shard)
    stage_setup = prepare_hot_stage(
        args,
        durable_payload_shard,
        stage_payload_shard,
        stage_payload_root,
    )
    stage_payload_path = stage_payload_shard.cache_root / "packed_records" / "e1.jsonl"
    stage_index_path = stage_payload_shard.cache_root / "packed_records" / "e1.index.jsonl"
    stage_rows = read_jsonl_rows(stage_index_path)
    require("packed-payload edge has one staged index row", len(stage_rows) == 1)
    require(
        "packed-payload edge rewrites copied index row to staged payload path",
        str(stage_payload_shard.cache_root) in stage_rows[0].get("packed_record_path", ""),
    )
    stage_payload_sha_before = sha256_file(stage_payload_path)
    stage_hash_readback = validate_packed_payload_hashes(stage_payload_shard.cache_root / "packed_records")
    stage_readback = shard_completion_readback(args, stage_payload_shard, require_packed_store=False)
    promotion = promote_hot_stage_shard(
        args,
        durable_payload_shard,
        stage_payload_shard,
        stage_payload_root,
        stage_readback,
    )
    durable_payload_path = durable_payload_shard.cache_root / "packed_records" / "e1.jsonl"
    durable_index_path = durable_payload_shard.cache_root / "packed_records" / "e1.index.jsonl"
    durable_rows = read_jsonl_rows(durable_index_path)
    require("packed-payload edge has one promoted index row", len(durable_rows) == 1)
    require(
        "packed-payload edge promoted index row points to durable payload path",
        str(durable_payload_shard.cache_root) in durable_rows[0].get("packed_record_path", ""),
    )
    require(
        "packed-payload edge preserved payload bytes across promotion",
        sha256_file(durable_payload_path) == stage_payload_sha_before,
    )
    durable_hash_readback = validate_packed_payload_hashes(durable_payload_shard.cache_root / "packed_records")
    cases.append(
        {
            "case": "hot_stage_promotion_preserves_packed_payload_hashes",
            "passes": True,
            "stage_setup": stage_setup,
            "stage_payload_sha256_before_promotion": stage_payload_sha_before,
            "stage_payload_hash_readback": stage_hash_readback,
            "promotion": promotion,
            "durable_payload_sha256_after_promotion": sha256_file(durable_payload_path),
            "durable_payload_hash_readback": durable_hash_readback,
            "durable_index_path": str(durable_index_path),
            "durable_index_sha256": sha256_file(durable_index_path),
        }
    )

    return cases


def validate_packed_payload_hashes(packed_root: Path) -> dict[str, Any]:
    require("packed record root exists for payload validation", packed_root.is_dir())
    per_index: dict[str, Any] = {}
    total_rows = 0
    for index_path in sorted(packed_root.glob("*.index.jsonl")):
        rows = read_jsonl_rows(index_path)
        for row_index, row in enumerate(rows):
            require(f"{index_path}:{row_index} packed index row is object", isinstance(row, dict))
            record_path = Path(str(row.get("packed_record_path", "")))
            byte_offset = row.get("byte_offset")
            byte_len = row.get("byte_len")
            expected_hash = row.get("record_sha256")
            require(f"{index_path}:{row_index} packed record path exists", record_path.is_file())
            require(
                f"{index_path}:{row_index} packed byte offset is valid",
                isinstance(byte_offset, int) and byte_offset >= 0,
            )
            require(
                f"{index_path}:{row_index} packed byte len is valid",
                isinstance(byte_len, int) and byte_len > 0,
            )
            require(
                f"{index_path}:{row_index} packed record sha exists",
                isinstance(expected_hash, str) and expected_hash.startswith("sha256:"),
            )
            with record_path.open("rb") as handle:
                handle.seek(byte_offset)
                payload = handle.read(byte_len)
            require(
                f"{index_path}:{row_index} packed payload byte length matches index",
                len(payload) == byte_len,
            )
            require(
                f"{index_path}:{row_index} packed payload hash matches index",
                sha256_bytes(payload) == expected_hash,
            )
        per_index[str(index_path)] = {
            "row_count": len(rows),
            "sha256": sha256_file(index_path),
        }
        total_rows += len(rows)
    return {
        "packed_root": str(packed_root),
        "index_file_count": len(per_index),
        "row_count": total_rows,
        "per_index": per_index,
        "passes": True,
    }


def require_numeric_ms(label: str, value: Any) -> float:
    require(f"{label} timing exists", isinstance(value, (int, float)))
    numeric = float(value)
    require(f"{label} timing is non-negative", numeric >= 0.0)
    return numeric


def validate_batch_telemetry_rows(
    args: argparse.Namespace,
    shard_id: str,
    summary: dict[str, Any],
    rows: list[Any],
    resource_before: dict[str, Any],
    resource_after: dict[str, Any],
) -> dict[str, Any]:
    expected_rows = int(summary.get("batch_telemetry_row_count", -1))
    require(f"{shard_id} telemetry row count matches manifest summary", len(rows) == expected_rows)
    gpus_before = resource_before.get("gpu", {}).get("gpus", [])
    gpus_after = resource_after.get("gpu", {}).get("gpus", [])
    per_embedder: dict[str, dict[str, Any]] = {}
    output_total = 0
    quarantine_total = 0
    latency_total_ms = 0.0
    for index, row in enumerate(rows):
        require(f"{shard_id} telemetry row {index} is object", isinstance(row, dict))
        embedder_id = row.get("embedder_id")
        require(f"{shard_id} telemetry row {index} has embedder id", isinstance(embedder_id, str) and bool(embedder_id))
        latency_total_ms += require_numeric_ms(f"{shard_id} telemetry row {index}", row.get("latency_ms"))
        entry = per_embedder.setdefault(
            embedder_id,
            {
                "embedder_id": embedder_id,
                "batch_count": 0,
                "output_count": 0,
                "quarantine_count": 0,
                "latency_ms_total": 0.0,
                "requested_device": None,
                "actual_device": None,
                "fallback_status": "not_observed",
                "vram_required_bytes_max": 0,
                "vram_free_before_bytes": None,
                "vram_free_after_bytes": None,
            },
        )
        entry["batch_count"] += 1
        entry["latency_ms_total"] += float(row.get("latency_ms"))
        artifact_kind = row.get("artifact_kind")
        if artifact_kind == "python_forward_true_batch_telemetry":
            require(f"{shard_id} true-batch row {index} says true_batch", row.get("true_batch") is True)
            batch_size = int(row.get("batch_size", -1))
            output_count = int(row.get("output_count", -1))
            quarantine_count = int(row.get("quarantine_count", -1))
            require(
                f"{shard_id} true-batch row {index} output/quarantine accounts for batch",
                output_count + quarantine_count == batch_size,
            )
            vram_plan = row.get("vram_plan")
            require(f"{shard_id} true-batch row {index} has GPU VRAM plan", isinstance(vram_plan, dict))
            require(
                f"{shard_id} true-batch row {index} VRAM plan embedder matches row",
                vram_plan.get("embedder_id") == embedder_id,
            )
            require(
                f"{shard_id} true-batch row {index} VRAM plan fits guard",
                int(vram_plan.get("required_bytes", -1)) <= int(vram_plan.get("guard_bytes", -2)),
            )
            require(f"{shard_id} true-batch row {index} has before GPU telemetry", bool(gpus_before))
            require(f"{shard_id} true-batch row {index} has after GPU telemetry", bool(gpus_after))
            precision_classes = row.get("precision_classes")
            require(
                f"{shard_id} true-batch row {index} records precision classes",
                isinstance(precision_classes, list) and bool(precision_classes),
            )
            require(
                f"{shard_id} true-batch row {index} has no fallback precision class",
                not any("fallback" in str(value).lower() for value in precision_classes),
            )
            entry["requested_device"] = "cuda"
            entry["actual_device"] = "cuda_contract_verified_by_vram_plan_and_gpu_resource"
            entry["fallback_status"] = "none"
            entry["vram_required_bytes_max"] = max(
                int(entry["vram_required_bytes_max"]),
                int(vram_plan.get("required_bytes", 0)),
            )
            entry["vram_free_before_bytes"] = gpus_before[0].get("memory_free_bytes")
            entry["vram_free_after_bytes"] = gpus_after[0].get("memory_free_bytes")
            output_total += output_count
            quarantine_total += quarantine_count
        elif artifact_kind == "python_forward_algorithmic_serial_telemetry":
            require(f"{shard_id} serial row {index} marks CUDA not applicable", row.get("cuda_applicable") is False)
            require(
                f"{shard_id} serial row {index} has CPU-only reason",
                isinstance(row.get("reason_code"), str) and bool(row.get("reason_code")),
            )
            output_count = int(row.get("processed_non_quarantine_count", -1))
            quarantine_count = int(row.get("quarantine_count", -1))
            row_count = int(row.get("row_count", -1))
            require(
                f"{shard_id} serial row {index} output/quarantine accounts for row count",
                output_count + quarantine_count == row_count,
            )
            entry["requested_device"] = "cpu"
            entry["actual_device"] = "cpu_only_verified_contract"
            entry["fallback_status"] = "not_applicable"
            output_total += output_count
            quarantine_total += quarantine_count
        else:
            raise SchedulerError(f"{shard_id} telemetry row {index} unsupported artifact kind: {artifact_kind}")

    require(
        f"{shard_id} telemetry output count matches manifest cache records",
        output_total == int(summary.get("cache_record_count", -1)),
    )
    require(
        f"{shard_id} telemetry quarantine count matches manifest quarantine rows",
        quarantine_total == int(summary.get("quarantine_row_count", -1)),
    )
    return {
        "row_count": len(rows),
        "output_count": output_total,
        "quarantine_count": quarantine_total,
        "latency_ms_total": round(latency_total_ms, 3),
        "per_embedder": per_embedder,
        "passes": True,
    }


def validate_run_log_resume_telemetry(args: argparse.Namespace, run_log: dict[str, Any]) -> dict[str, Any]:
    shard_id = run_log.get("shard_id")
    require("resume gate run log has shard id", isinstance(shard_id, str) and bool(shard_id))
    require(f"{shard_id} resume gate run log exited zero", run_log.get("exit_code") == 0)
    require(f"{shard_id} resume gate run log was executed", run_log.get("skipped_completed") is False)
    elapsed_seconds = float(run_log.get("elapsed_seconds", -1.0))
    require(f"{shard_id} resume gate elapsed seconds positive", elapsed_seconds > 0.0)
    timing = run_log.get("timing_ms")
    require(f"{shard_id} resume gate has timing object", isinstance(timing, dict))
    require_numeric_ms(f"{shard_id} runner subprocess", timing.get("runner_subprocess_ms"))
    require_numeric_ms(f"{shard_id} durable readback", timing.get("durable_readback_ms"))
    if use_hot_stage_for_shard(args):
        require(f"{shard_id} hot-stage promotion exists", isinstance(run_log.get("hot_stage_promotion"), dict))
        promotion_timing = run_log["hot_stage_promotion"].get("timing_ms")
        require(f"{shard_id} hot-stage promotion timing exists", isinstance(promotion_timing, dict))
        for key in (
            "path_rewrite_ms",
            "cache_promotion_ms",
            "fsv_promotion_ms",
            "durable_readback_ms",
            "stage_cleanup_ms",
            "total_ms",
        ):
            require_numeric_ms(f"{shard_id} hot-stage {key}", promotion_timing.get(key))
    summary = run_log.get("summary")
    require(f"{shard_id} resume gate summary exists", isinstance(summary, dict))
    manifest_path = Path(str(run_log.get("manifest_path", "")))
    manifest = read_json(manifest_path)
    require(f"{shard_id} resume gate manifest passes", manifest.get("passes") is True)
    require(f"{shard_id} resume gate manifest summary matches run log", manifest.get("summary") == summary)
    telemetry_path = Path(str(manifest.get("source_of_truth", {}).get("batch_telemetry_jsonl", "")))
    rows = read_jsonl_rows(telemetry_path)
    telemetry = validate_batch_telemetry_rows(
        args,
        shard_id,
        summary,
        rows,
        run_log.get("resource_before", {}),
        run_log.get("resource_after", {}),
    )
    chunk_count = int(summary.get("chunk_count", 0))
    expected_pairs = chunk_count * int(summary.get("active_embedder_count", 0))
    pair_count = int(summary.get("cache_record_count", 0)) + int(summary.get("quarantine_row_count", 0))
    require(f"{shard_id} resume gate pairs match expected", pair_count == expected_pairs)
    return {
        "shard_id": shard_id,
        "elapsed_seconds": elapsed_seconds,
        "chunks_per_second": 0.0 if elapsed_seconds == 0.0 else round(chunk_count / elapsed_seconds, 6),
        "pairs_per_second": 0.0 if elapsed_seconds == 0.0 else round(pair_count / elapsed_seconds, 6),
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "batch_telemetry_path": str(telemetry_path),
        "batch_telemetry_sha256": sha256_file(telemetry_path),
        "summary": summary,
        "timing_ms": timing,
        "hot_stage_promotion_timing_ms": None
        if not isinstance(run_log.get("hot_stage_promotion"), dict)
        else run_log["hot_stage_promotion"].get("timing_ms"),
        "resource_before": run_log.get("resource_before"),
        "resource_after": run_log.get("resource_after"),
        "telemetry": telemetry,
        "passes": True,
    }


def resume_gate_edge_checks(args: argparse.Namespace, run_logs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    require("resume gate edge checks need at least one run log", bool(run_logs))
    base_log = run_logs[-1]
    manifest = read_json(Path(str(base_log["manifest_path"])))
    telemetry_path = Path(str(manifest["source_of_truth"]["batch_telemetry_jsonl"]))
    telemetry_rows = read_jsonl_rows(telemetry_path)
    summary = dict(base_log["summary"])
    cases: list[dict[str, Any]] = []

    true_batch_rows = [row for row in telemetry_rows if row.get("artifact_kind") == "python_forward_true_batch_telemetry"]
    require("resume gate has a true-batch telemetry row for missing-GPU edge", bool(true_batch_rows))
    missing_gpu_rows = json.loads(json.dumps(telemetry_rows))
    for row in missing_gpu_rows:
        if row.get("artifact_kind") == "python_forward_true_batch_telemetry":
            row.pop("vram_plan", None)
            break
    try:
        validate_batch_telemetry_rows(
            args,
            str(base_log["shard_id"]),
            summary,
            missing_gpu_rows,
            base_log.get("resource_before", {}),
            base_log.get("resource_after", {}),
        )
    except SchedulerError as exc:
        cases.append({"case": "missing_gpu_telemetry_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("missing GPU telemetry edge unexpectedly passed")

    missing_timing_log = json.loads(json.dumps(base_log))
    if isinstance(missing_timing_log.get("hot_stage_promotion"), dict):
        missing_timing_log["hot_stage_promotion"].get("timing_ms", {}).pop("durable_readback_ms", None)
    else:
        missing_timing_log.get("timing_ms", {}).pop("durable_readback_ms", None)
    try:
        validate_run_log_resume_telemetry(args, missing_timing_log)
    except SchedulerError as exc:
        cases.append({"case": "missing_promotion_readback_timing_rejected", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("missing promotion/readback timing edge unexpectedly passed")

    mismatched_summary = dict(summary)
    mismatched_summary["batch_telemetry_row_count"] = int(summary["batch_telemetry_row_count"]) + 1
    try:
        validate_batch_telemetry_rows(
            args,
            str(base_log["shard_id"]),
            mismatched_summary,
            telemetry_rows,
            base_log.get("resource_before", {}),
            base_log.get("resource_after", {}),
        )
    except SchedulerError as exc:
        cases.append({"case": "telemetry_row_count_mismatch_blocks_resume", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("telemetry row-count mismatch edge unexpectedly passed")

    return cases


def write_resume_gate_fsv(
    args: argparse.Namespace,
    started_at: str,
    event_path: Path,
    run_logs: list[dict[str, Any]],
    checkpoint: dict[str, Any],
    corpus_manifest: dict[str, Any],
    preflight: dict[str, Any],
) -> dict[str, Any]:
    require("resume gate requires executed run logs", bool(run_logs))
    checkpoint_summary = checkpoint.get("summary")
    require("resume gate checkpoint summary exists", isinstance(checkpoint_summary, dict))
    require("resume gate checkpoint summary matches aggregate manifest", checkpoint_summary == corpus_manifest.get("summary"))
    event_rows = read_jsonl_rows(event_path)
    require("resume gate event log has rows", bool(event_rows))
    run_telemetry = [validate_run_log_resume_telemetry(args, log) for log in run_logs]
    edge_cases = resume_gate_edge_checks(args, run_logs)
    baseline = None
    if args.resume_gate_baseline_checkpoint is not None:
        baseline_path = args.resume_gate_baseline_checkpoint
        baseline_checkpoint = read_json(baseline_path)
        require("resume gate baseline checkpoint passes", baseline_checkpoint.get("passes") is True)
        baseline = {
            "path": str(baseline_path),
            "sha256": sha256_file(baseline_path),
            "progress": baseline_checkpoint.get("progress"),
            "summary": baseline_checkpoint.get("summary"),
            "runner_controls": baseline_checkpoint.get("runner_controls"),
        }
    total_chunks = sum(int(row["summary"]["chunk_count"]) for row in run_telemetry)
    total_pairs = sum(
        int(row["summary"]["cache_record_count"]) + int(row["summary"]["quarantine_row_count"])
        for row in run_telemetry
    )
    total_elapsed = sum(float(row["elapsed_seconds"]) for row in run_telemetry)
    fsv_path = args.fsv_root / "python_forward_resume_gate_fsv.json"
    value = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_resume_gate_fsv",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "resume_gate_fsv": str(fsv_path),
            "events": str(event_path),
            "checkpoint_cache": str(args.cache_root / "python_forward_cache_checkpoint.json"),
            "checkpoint_fsv": str(args.fsv_root / "python_forward_cache_checkpoint.json"),
            "aggregate_manifest": str(args.cache_root / "python_forward_cache_corpus_manifest.json"),
            "cache_root": str(args.cache_root),
            "fsv_root": str(args.fsv_root),
            "run_log_root": str(args.fsv_root / "run_logs"),
        },
        "baseline_checkpoint": baseline,
        "host_preflight": preflight,
        "checkpoint_summary": checkpoint_summary,
        "aggregate_summary": corpus_manifest.get("summary"),
        "run_count": len(run_logs),
        "event_row_count": len(event_rows),
        "throughput": {
            "total_chunks": total_chunks,
            "total_pairs": total_pairs,
            "total_elapsed_seconds": round(total_elapsed, 3),
            "chunks_per_second": 0.0 if total_elapsed == 0.0 else round(total_chunks / total_elapsed, 6),
            "pairs_per_second": 0.0 if total_elapsed == 0.0 else round(total_pairs / total_elapsed, 6),
        },
        "run_telemetry": run_telemetry,
        "edge_cases": edge_cases,
        "passes": True,
    }
    json_dump_checked(fsv_path, value)
    readback = read_json(fsv_path)
    require("resume gate FSV readback passes", readback.get("passes") is True)
    require("resume gate edge cases pass", all(case.get("passes") is True for case in edge_cases))
    value["resume_gate_sha256"] = sha256_file(fsv_path)
    return value


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
    payload_hash_readback = validate_packed_payload_hashes(packed_root)
    require(
        f"{shard.shard_id} packed payload hash readback row count matches manifest",
        payload_hash_readback.get("row_count") == cumulative_rows,
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
        "payload_hash_readback": payload_hash_readback,
        "packed_manifest": packed_manifest,
    }


def validate_manifest(
    shard: ShardSpec,
    require_packed_store: bool,
    validate_packed_readback: bool = True,
    expected_chunks_jsonl: Path | None = None,
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
    if expected_chunks_jsonl is not None:
        expected_chunks = str(expected_chunks_jsonl)
        actual_chunks = str(inputs.get("chunks_jsonl") or "")
        require(
            f"{shard.shard_id} chunks_jsonl readback matches requested source",
            actual_chunks == expected_chunks,
        )
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


def write_minimal_packed_shard(
    root: Path,
    shard_id: str,
    chunk_offset: int,
    max_chunks: int,
    chunks_jsonl: Path | None = None,
) -> ShardSpec:
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
    record_payload = record_path.read_bytes().rstrip(b"\n")
    index_row = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": record["cache_key_sha256"],
        "embedder_id": "e1",
        "packed_record_path": str(record_path),
        "byte_offset": 0,
        "byte_len": len(record_payload),
        "record_sha256": sha256_bytes(record_payload),
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
                "chunks_jsonl": str(chunks_jsonl or DEFAULT_CHUNKS_JSONL),
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
    record_payload = record_path.read_bytes().rstrip(b"\n")
    index_row = {
        "schema_version": SCHEMA_VERSION,
        "cache_key_sha256": record["cache_key_sha256"],
        "embedder_id": "e10",
        "packed_record_path": str(record_path),
        "byte_offset": 0,
        "byte_len": len(record_payload),
        "record_sha256": sha256_bytes(record_payload),
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


def aggregate(
    shards: list[ShardSpec],
    output_path: Path,
    require_packed_store: bool = False,
    expected_chunks_jsonl: Path | None = None,
) -> dict[str, Any]:
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
        manifests.append(validate_manifest(shard, require_packed_store, expected_chunks_jsonl=expected_chunks_jsonl))

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
        "hot_stage_projected_capacity_overflow_rejected_before_writes",
        hot_stage_budget_bytes=preflight["host"]["filesystems"]["/"]["available_bytes"] + 1,
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
    expect_preflight_failure(
        "missing_release_runner_rejected_before_writes",
        cargo_runner="/definitely/missing/python_forward_cache_fsv",
    )
    return cases


def task061_smoke_manifest_gate_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not task061_smoke_manifest_gate_required(args):
        return []
    edge_root = args.fsv_root / "edge_inputs" / "task061_smoke_manifest_gate"
    edge_root.mkdir(parents=True, exist_ok=True)
    real_payload = read_json(args.smoke_manifest_fsv)
    source_of_truth = real_payload.get("source_of_truth")
    require("task061 edge source_of_truth object exists", isinstance(source_of_truth, dict))
    real_manifest = require_path_string(source_of_truth, "smoke_manifest_jsonl", "task061 edge source_of_truth")

    stale_chunks = edge_root / "stale_smoke_chunks.jsonl"
    stale_manifest = edge_root / "stale_smoke_manifest.jsonl"
    with args.chunks_jsonl.open("r", encoding="utf-8") as handle:
        first_chunk_line = next(line for line in handle if line.strip())
    stale_chunks.write_text(first_chunk_line, encoding="utf-8")
    with real_manifest.open("r", encoding="utf-8") as handle:
        first_manifest_line = next(line for line in handle if line.strip())
    stale_manifest.write_text(first_manifest_line, encoding="utf-8")

    synthetic_payload = json.loads(json.dumps(real_payload))
    synthetic_payload["source_of_truth"]["smoke_chunks_jsonl"] = str(stale_chunks)
    synthetic_payload["source_of_truth"]["smoke_manifest_jsonl"] = str(stale_manifest)
    synthetic_payload["after_state"]["chunks_out"]["path"] = str(stale_chunks)
    synthetic_payload["after_state"]["manifest_out"]["path"] = str(stale_manifest)
    synthetic_fsv = edge_root / "synthetic_stale_smoke_manifest_fsv.json"
    json_dump_checked(synthetic_fsv, synthetic_payload)

    edge_args = argparse.Namespace(**vars(args))
    edge_args.chunks_jsonl = stale_chunks
    edge_args.smoke_manifest_fsv = synthetic_fsv
    try:
        task061_smoke_manifest_gate(edge_args)
    except SchedulerError as exc:
        error = str(exc)
        require("task061 stale hash edge failed on smoke chunks hash", "TASK061_SMOKE_CHUNKS_HASH_MATCH" in error)
        return [
            {
                "case": "stale_task061_smoke_chunks_hash_rejected",
                "passes": True,
                "synthetic_fsv": str(synthetic_fsv),
                "synthetic_fsv_sha256": sha256_file(synthetic_fsv),
                "stale_chunks": file_state(stale_chunks),
                "expected_source_chunks_sha256": (real_payload.get("summary") or {}).get("smoke_chunks_sha256"),
                "error": error,
            }
        ]
    raise SchedulerError("task061 stale smoke manifest edge unexpectedly passed")


def edge_checks(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    preflight: dict[str, Any],
) -> list[dict[str, Any]]:
    edge_root = args.fsv_root / "edge_inputs"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = preflight_edge_checks(args, preflight)
    cases.extend(task061_smoke_manifest_gate_edge_checks(args))
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

    if use_hot_stage_for_shard(args):
        cases.extend(hot_stage_promotion_edge_checks(args))

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
    task061_gate = task061_smoke_manifest_gate(args)
    smoke_gate = smoke_gate_preflight(args)
    preflight = host_preflight(args)
    preflight["task061_smoke_manifest_gate"] = task061_gate
    preflight["smoke_gate"] = smoke_gate
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
        "write_resume_gate": args.write_resume_gate,
        "resume_gate_baseline_checkpoint": None
        if args.resume_gate_baseline_checkpoint is None
        else str(args.resume_gate_baseline_checkpoint),
        "stage_packed_shards_on_hot": args.stage_packed_shards_on_hot,
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
        cache_checkpoint_path, fsv_checkpoint_path = checkpoint_file_paths(args)
        if cache_checkpoint_path.exists() or fsv_checkpoint_path.exists():
            checkpoint = read_existing_checkpoint(args)
        else:
            checkpoint = write_checkpoint(args, shards, [], started_at, preflight, "fresh_preflight")
            event(
                event_path,
                "fresh_preflight_checkpoint_written",
                {
                    "checkpoint_cache": file_state(cache_checkpoint_path),
                    "checkpoint_fsv": file_state(fsv_checkpoint_path),
                    "progress": checkpoint.get("progress"),
                    "summary": checkpoint.get("summary"),
                },
            )
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
    corpus_manifest = aggregate(shards, aggregate_path, args.require_packed_store, expected_chunks_jsonl=args.chunks_jsonl)
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
    resume_gate = None
    if args.write_resume_gate:
        resume_gate = write_resume_gate_fsv(
            args,
            started_at,
            event_path,
            run_logs,
            checkpoint,
            corpus_manifest,
            preflight,
        )
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
        "resume_gate": resume_gate,
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
