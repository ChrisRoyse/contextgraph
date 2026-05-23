#!/usr/bin/env python3
"""Shard scheduler for Python ME-JEPA-Code forward-cache generation.

This script is intentionally small and fail-closed. It does not embed code
itself; it schedules the Rust forward-cache READBACK runner over explicit chunk
windows, then aggregates the physical shard manifests it reads back from disk.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable


# Serializes appends to the scheduler event log (and any other shared JSONL
# paths) so worker threads in parallel-isolated mode cannot race the
# write-then-readback-assert in jsonl_append_checked().
_JSONL_APPEND_LOCK = threading.Lock()

# Serializes promote_hot_stage_shard across parallel-isolated worker threads.
# Multiple subruns from the SAME shard all promote to the same durable
# cache_root/readback_root pair; without this lock they race on tree_state byte
# counts and trip promote_tree_verified assertions. Forward (GPU) work still
# happens in parallel; only the brief move/archive step is serialized. #302.
_PROMOTE_HOT_STAGE_LOCK = threading.Lock()


AIWONDER_DURABLE_ROOT = Path("/zfs/archive/contextgraph")
AIWONDER_HOT_ROOT = Path("/zfs/hot/contextgraph")
AIWONDER_SCRATCH_ROOT = Path("/home/croyse/.cache/contextgraph")

DEFAULT_CHUNKS_JSONL = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_CACHE_ROOT = Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-029-sharded-cuda-scheduler-readback")
DEFAULT_HOT_ROOT = Path("/zfs/hot/contextgraph/runtime/python-forward-shard-scheduler")
DEFAULT_SMOKE_GATE_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-057-smoke-gate-preflight-readback")
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
DEFAULT_FULL_RELAUNCH_GATE_READBACK = Path(
    "/zfs/archive/contextgraph/readback/task-py-data-064-post249-full-run-relaunch-gate-readback/full_run_relaunch_gate_readback.json"
)
DEFAULT_FULL_AST_CORPUS_READBACK = Path(
    "/zfs/archive/contextgraph/readback/task-py-data-058-full-ast-corpus-readback/full_ast_corpus_readback.json"
)
DEFAULT_RUNNER_TIMEOUT_SECONDS = 3600
DEFAULT_CARGO_RUNNER = "target/release/examples/python_forward_cache_readback"
SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-029"
SUPPORTED_TRUE_BATCH_SLUGS = {"e1", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14"}
STATIC_AST_RETIRED_TEMPORAL_FORWARD_SLUGS = ("e2", "e3", "e4")
ISOLATED_FORWARD_SLUGS = ("e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9")
DEFAULT_ACTIVE_EMBEDDER_COUNT = len(ISOLATED_FORWARD_SLUGS)
DEFAULT_CORPUS_PROFILE = "static_ast_python"
DEFAULT_ESTIMATED_RECORD_BYTES = 16 * 1024
DEFAULT_HOT_STAGE_BUDGET_BYTES = 48 * 1024 * 1024 * 1024
DEFAULT_DURABLE_SLACK_BYTES = 2 * 1024 * 1024 * 1024
DEFAULT_READBACK_EDGE_COPY_BUDGET_BYTES = 2 * 1024 * 1024 * 1024
MAX_DURABLE_IO_WORKERS = 8
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
    readback_root: Path

    def manifest_path(self) -> Path:
        return self.cache_root / "python_forward_cache_manifest.json"

    def index_path(self) -> Path:
        return self.cache_root / "python_forward_cache_index.jsonl"

    def quarantine_path(self) -> Path:
        return self.cache_root / "python_forward_cache_quarantine.jsonl"

    def readback_path(self) -> Path:
        return self.readback_root / "python_forward_cache_readback.json"


def parse_args() -> argparse.Namespace:
    default_cpu_workers = detected_cpu_worker_count()
    parser = argparse.ArgumentParser(
        description="Run and aggregate sharded Python forward-cache generation."
    )
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--cache-root", type=Path, default=DEFAULT_CACHE_ROOT)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
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
            "For packed layout, run shard cache/READBACK assembly in a bounded aiwonder hot stage, "
            "then promote verified artifacts to the durable aiwonder SoT."
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
        help="Projected durable bytes per chunk/embedder output for pre-run aiwonder capacity checks.",
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
        help="Required free bytes on aiwonder hot storage before any writes.",
    )
    parser.add_argument(
        "--readback-edge-copy-budget-bytes",
        type=int,
        default=DEFAULT_READBACK_EDGE_COPY_BUDGET_BYTES,
        help="Required temporary durable bytes reserved for READBACK edge/readback artifacts.",
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
        "--smoke-gate-readback-root",
        type=Path,
        default=DEFAULT_SMOKE_GATE_READBACK_ROOT,
        help="Where to write the fail-closed smoke-gate preflight READBACK artifact.",
    )
    parser.add_argument("--smoke-manifest-readback", type=Path, default=DEFAULT_SMOKE_MANIFEST_READBACK)
    parser.add_argument("--smoke-forward-readback", type=Path, default=DEFAULT_SMOKE_FORWARD_READBACK)
    parser.add_argument("--smoke-intelligence-readback", type=Path, default=DEFAULT_SMOKE_INTELLIGENCE_READBACK)
    parser.add_argument("--smoke-storage-design-readback", type=Path, default=DEFAULT_SMOKE_STORAGE_DESIGN_READBACK)
    parser.add_argument("--full-relaunch-gate-readback", type=Path, default=DEFAULT_FULL_RELAUNCH_GATE_READBACK)
    parser.add_argument("--full-ast-corpus-readback", type=Path, default=DEFAULT_FULL_AST_CORPUS_READBACK)
    parser.add_argument(
        "--resume-completed-shards",
        action="store_true",
        help=(
            "In run mode, skip only shards whose manifest, READBACK, telemetry, "
            "index/quarantine, and prior run-log readback already pass. "
            "Resume fast-forward uses bounded manifest readback; the final "
            "aggregate still performs packed-store validation."
        ),
    )
    parser.add_argument(
        "--retry-failed-partial-shards",
        action="store_true",
        help=(
            "When used with --resume-completed-shards, archive a failed or partial "
            "shard cache/READBACK/run-log under readback_root/failed_partial_shards before "
            "retrying that shard. Passed shard manifests are still the only skip condition."
        ),
    )
    parser.add_argument(
        "--isolate-embedder-processes",
        action="store_true",
        help=(
            "Run each active embedder in a separate Rust runner process, then consolidate "
            "the physical per-embedder row files into one full-shard manifest/READBACK. This avoids "
            "cumulative CUDA model residency without a final all-embedder rerun."
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


def normalize_guard_path(path: Path) -> Path:
    expanded = path.expanduser()
    if expanded.is_absolute():
        return expanded.resolve(strict=False)
    return (Path.cwd() / expanded).resolve(strict=False)


def path_is_under(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def production_root_error(label: str, path: Path, expected: str) -> SchedulerError:
    return SchedulerError(
        f"MEJEPA_PRODUCTION_ROOT_NOT_AIWONDER: {label}={path} is not an aiwonder production root; {expected}"
    )


def require_production_durable_path(path: Path, label: str) -> Path:
    normalized = normalize_guard_path(path)
    if path_is_under(normalized, AIWONDER_DURABLE_ROOT):
        return normalized
    raise production_root_error(label, normalized, f"use {AIWONDER_DURABLE_ROOT}")


def require_aiwonder_hot_or_scratch_path(path: Path, label: str) -> Path:
    normalized = normalize_guard_path(path)
    if path_is_under(normalized, AIWONDER_HOT_ROOT) or path_is_under(normalized, AIWONDER_SCRATCH_ROOT):
        return normalized
    raise production_root_error(
        label,
        normalized,
        f"use {AIWONDER_HOT_ROOT} or explicitly budgeted aiwonder scratch under {AIWONDER_SCRATCH_ROOT}",
    )


def require_aiwonder_input_path(path: Path, label: str) -> Path:
    normalized = normalize_guard_path(path)
    if path_is_under(normalized, AIWONDER_DURABLE_ROOT) or path_is_under(normalized, AIWONDER_HOT_ROOT):
        return normalized
    raise production_root_error(
        label,
        normalized,
        f"use {AIWONDER_HOT_ROOT} for hot inputs or {AIWONDER_DURABLE_ROOT} for durable inputs",
    )


def validate_production_roots(args: argparse.Namespace) -> dict[str, Any]:
    checked = {
        "chunks_jsonl": str(require_aiwonder_input_path(args.chunks_jsonl, "chunks_jsonl")),
        "cache_root": str(require_production_durable_path(args.cache_root, "cache_root")),
        "readback_root": str(require_production_durable_path(args.readback_root, "readback_root")),
        "hot_root": str(require_aiwonder_hot_or_scratch_path(args.hot_root, "hot_root")),
    }
    for attr in (
        "smoke_gate_readback_root",
        "smoke_manifest_readback",
        "smoke_forward_readback",
        "smoke_intelligence_readback",
        "smoke_storage_design_readback",
        "full_relaunch_gate_readback",
        "full_ast_corpus_readback",
    ):
        checked[attr] = str(
            require_production_durable_path(getattr(args, attr), attr)
        )
    return checked


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
            "context-graph-mejepa-embedders --example python_forward_cache_readback` "
            "and pass --cargo-runner target/release/examples/python_forward_cache_readback"
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
            normalized.endswith("/target/release/examples/python_forward_cache_readback"),
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
            "durable_io_workers": args.io_workers,
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
    # Serialize write + readback-assert across parallel-isolated worker threads
    # so the last-line existence check cannot race against another append.
    with _JSONL_APPEND_LOCK:
        with path.open("a", encoding="utf-8") as handle:
            handle.write(line)
            handle.flush()
            os.fsync(handle.fileno())
        last = path.read_text(encoding="utf-8").splitlines()[-1]
        require("event JSONL last-line readback parses", json.loads(last) == value)


def jsonl_write_checked(path: Path, rows: list[Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows).encode("utf-8")
    tmp = path.with_suffix(path.suffix + f".tmp-{os.getpid()}")
    tmp.write_bytes(data)
    require(f"{path} JSONL temp byte readback equality", tmp.read_bytes() == data)
    tmp.replace(path)
    require(f"{path} JSONL final byte readback equality", path.read_bytes() == data)
    require(f"{path} JSONL final row count matches write", count_jsonl_rows(path) == len(rows))


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


def directory_state(path: Path) -> dict[str, Any]:
    return {
        "path": str(path),
        "exists": path.exists(),
        "is_dir": path.is_dir(),
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


def top_level_entries_state(path: Path, limit: int = 40) -> dict[str, Any]:
    entries: list[dict[str, Any]] = []
    count = 0
    if path.is_dir():
        for child in sorted(path.iterdir(), key=lambda item: item.name):
            count += 1
            if len(entries) < limit:
                entries.append(
                    {
                        "name": child.name,
                        "is_file": child.is_file(),
                        "is_dir": child.is_dir(),
                    }
                )
    return {
        "path": str(path),
        "exists": path.exists(),
        "is_dir": path.is_dir(),
        "entry_count": count,
        "sample": entries,
        "sample_truncated": count > len(entries),
    }


def scheduler_cache_root_state(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "path": str(args.cache_root),
        "scan_mode": "bounded_artifact_readback",
        "root": directory_state(args.cache_root),
        "top_level_entries": top_level_entries_state(args.cache_root),
        "artifacts": {
            "checkpoint": file_state(args.cache_root / "python_forward_cache_checkpoint.json"),
            "shard_plan": file_state(args.cache_root / "python_forward_cache_shard_plan.json"),
            "aggregate_manifest": file_state(args.cache_root / "python_forward_cache_corpus_manifest.json"),
            "shards_dir": directory_state(args.cache_root / "shards"),
            "records_dir": directory_state(args.cache_root / "records"),
        },
    }


def scheduler_readback_root_state(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "path": str(args.readback_root),
        "scan_mode": "bounded_artifact_readback",
        "root": directory_state(args.readback_root),
        "top_level_entries": top_level_entries_state(args.readback_root),
        "artifacts": {
            "checkpoint": file_state(args.readback_root / "python_forward_cache_checkpoint.json"),
            "scheduler_readback": file_state(args.readback_root / "python_forward_shard_scheduler_readback.json"),
            "resume_preflight_readback": file_state(args.readback_root / "python_forward_resume_preflight_readback.json"),
            "resume_gate_readback": file_state(args.readback_root / "python_forward_resume_gate_readback.json"),
            "events": file_state(args.readback_root / "scheduler_events.jsonl"),
            "run_logs_dir": directory_state(args.readback_root / "run_logs"),
            "shards_dir": directory_state(args.readback_root / "shards"),
            "failed_partial_shards_dir": directory_state(args.readback_root / "failed_partial_shards"),
            "failed_hot_stages_dir": directory_state(args.readback_root / "failed_hot_stages"),
        },
    }


def scheduler_hot_root_state(args: argparse.Namespace) -> dict[str, Any]:
    packed_stage = args.hot_root / "packed-shard-stage"
    packed_stage_entries = top_level_entries_state(packed_stage)
    return {
        "path": str(args.hot_root),
        "scan_mode": "bounded_artifact_readback",
        "root": directory_state(args.hot_root),
        "top_level_entries": top_level_entries_state(args.hot_root),
        "artifacts": {
            "packed_shard_stage_dir": directory_state(packed_stage),
            "packed_shard_stage_entries": packed_stage_entries,
            "model_artifact_cache_dir": directory_state(args.hot_root / "model-artifact-cache"),
        },
        "hot_stage_blocker_count": packed_stage_entries["entry_count"],
    }


def scheduler_root_state(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "cache_root": scheduler_cache_root_state(args),
        "readback_root": scheduler_readback_root_state(args),
        "hot_root": scheduler_hot_root_state(args),
    }


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


def _disk_snapshot_targets() -> list[str]:
    """Targets df should report for aiwonder durable + hot filesystems."""
    raw = os.environ.get("CG_MEJEPA_DISK_TARGETS", "/,/zfs/hot,/zfs/archive")
    targets = [t.strip() for t in raw.split(",") if t.strip()]
    require("MEJEPA_SCHEDULER_DISK_TARGETS_EMPTY: at least one df target required", bool(targets))
    return targets


def disk_snapshot() -> dict[str, Any]:
    targets = _disk_snapshot_targets()
    return run_probe(
        ["df", "-B1", "--output=source,size,used,avail,pcent,target", *targets]
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
    for target in _disk_snapshot_targets():
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
    require("MEJEPA_SCHEDULER_READBACK_BUDGET_INVALID: READBACK edge-copy budget is non-negative", args.readback_edge_copy_budget_bytes >= 0)
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
        "readback_edge_copy_budget_bytes": args.readback_edge_copy_budget_bytes,
        "durable_slack_bytes": DEFAULT_DURABLE_SLACK_BYTES,
        "required_durable_available_bytes": projected_output_bytes
        + args.readback_edge_copy_budget_bytes
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
            "path": args.smoke_manifest_readback,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "smoke_forward_cache",
            "issue": 252,
            "task_id": "TASK-PY-DATA-061",
            "path": args.smoke_forward_readback,
        },
        {
            "name": "smoke_intelligence_matrices",
            "issue": 253,
            "task_id": "TASK-PY-DATA-062",
            "path": args.smoke_intelligence_readback,
        },
        {
            "name": "smoke_storage_design",
            "issue": 254,
            "task_id": "TASK-PY-DATA-063",
            "path": args.smoke_storage_design_readback,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "full_relaunch_gate",
            "issue": 255,
            "task_id": "TASK-PY-DATA-064",
            "path": args.full_relaunch_gate_readback,
            "expected_chunks_jsonl": args.chunks_jsonl,
        },
        {
            "name": "full_ast_corpus",
            "issue": 249,
            "task_id": "TASK-PY-DATA-058",
            "path": args.full_ast_corpus_readback,
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


def archive_hot_stage_blockers_for_preflight(
    args: argparse.Namespace,
    shards: list[ShardSpec],
) -> list[dict[str, Any]]:
    blockers = hot_stage_blockers(args)
    if not blockers:
        return []
    require(
        "MEJEPA_SCHEDULER_PREFLIGHT_ARCHIVE_MODE: hot-stage blockers are archived only by preflight-resume",
        args.mode == "preflight-resume",
    )
    require(
        "MEJEPA_SCHEDULER_PREFLIGHT_ARCHIVE_RETRY: hot-stage blocker archive requires retry-failed-partial-shards",
        args.retry_failed_partial_shards,
    )
    shard_by_id = {shard.shard_id: shard for shard in shards}
    archives: list[dict[str, Any]] = []
    for blocker in blockers:
        stage_root = Path(str(blocker.get("path", "")))
        require(
            f"MEJEPA_SCHEDULER_PREFLIGHT_HOT_STAGE_DIR: {stage_root} is a directory",
            stage_root.is_dir(),
        )
        shard = shard_by_id.get(stage_root.name)
        require(
            f"MEJEPA_SCHEDULER_PREFLIGHT_HOT_STAGE_PLANNED: {stage_root.name} matches a planned shard",
            shard is not None,
        )
        archive = archive_interrupted_hot_stage(
            args,
            shard,
            stage_root,
            "preflight-resume archives interrupted hot-stage before smoke gate",
        )
        require(
            f"MEJEPA_SCHEDULER_PREFLIGHT_HOT_STAGE_ARCHIVED: {stage_root} archive exists",
            isinstance(archive, dict) and Path(str(archive["archive_root"])).exists(),
        )
        archives.append(archive)
    require("MEJEPA_SCHEDULER_PREFLIGHT_HOT_STAGE_CLEAN: hot-stage blockers are empty after archive", not hot_stage_blockers(args))
    return archives


def write_smoke_gate_readback(
    args: argparse.Namespace,
    started_at: str,
    required: bool,
    artifact_results: list[dict[str, Any]],
    hot_stage_results: list[dict[str, Any]],
) -> dict[str, Any]:
    args.smoke_gate_readback_root.mkdir(parents=True, exist_ok=True)
    readback_path = args.smoke_gate_readback_root / "python_forward_smoke_gate_preflight_readback.json"
    failures = [
        result
        for result in artifact_results
        if result.get("passes") is not True
    ] + hot_stage_results
    value = {
        "schema_version": SCHEMA_VERSION,
        "task_id": "TASK-PY-DATA-057",
        "artifact_kind": "python_forward_smoke_gate_preflight_readback",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "required": required,
        "full_run_task_detected": args.task_id == "TASK-PY-DATA-016",
        "source_of_truth": {
            "smoke_gate_readback": str(readback_path),
            "cache_root": str(args.cache_root),
            "readback_root": str(args.readback_root),
            "hot_root": str(args.hot_root),
            "smoke_manifest_readback": str(args.smoke_manifest_readback),
            "smoke_forward_readback": str(args.smoke_forward_readback),
            "smoke_intelligence_readback": str(args.smoke_intelligence_readback),
            "smoke_storage_design_readback": str(args.smoke_storage_design_readback),
            "full_relaunch_gate_readback": str(args.full_relaunch_gate_readback),
            "full_ast_corpus_readback": str(args.full_ast_corpus_readback),
        },
        "artifact_results": artifact_results,
        "hot_stage_results": hot_stage_results,
        "failure_count": len(failures),
        "failures": failures,
        "passes": required is False or not failures,
    }
    json_dump_checked(readback_path, value)
    readback = read_json(readback_path)
    require("smoke gate preflight READBACK readback has pass flag", readback.get("passes") in (True, False))
    return {
        **value,
        "readback_path": str(readback_path),
        "readback_sha256": sha256_file(readback_path),
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
    gate = write_smoke_gate_readback(args, started_at, required, artifact_results, hot_stage_results)
    if required and gate["passes"] is not True:
        failure_names = [
            str(item.get("name") or item.get("path"))
            for item in gate["failures"]
        ]
        raise SchedulerError(
            "MEJEPA_SCHEDULER_SMOKE_GATE_BLOCKED: full corpus run is blocked "
            f"until smoke gate artifacts pass; failures={failure_names}; readback={gate['readback_path']}"
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
        "smoke_manifest_readback": str(args.smoke_manifest_readback),
        "requested_chunks_jsonl": str(args.chunks_jsonl),
    }
    if not required:
        return {**base, "passes": True}

    require("TASK061_SMOKE_MANIFEST_READBACK_EXISTS: #251 smoke manifest READBACK exists", args.smoke_manifest_readback.is_file())
    payload = read_json(args.smoke_manifest_readback)
    require("TASK061_SMOKE_MANIFEST_TASK_ID: #251 READBACK task_id matches", payload.get("task_id") == "TASK-PY-DATA-060")
    require("TASK061_SMOKE_MANIFEST_PASSES: #251 READBACK passes", payload.get("passes") is True)
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
    readback_state = file_state(args.smoke_manifest_readback)
    expected_chunk_sha = (after_state.get("chunks_out") or {}).get("sha256") or summary.get("smoke_chunks_sha256")
    expected_manifest_sha = (after_state.get("manifest_out") or {}).get("sha256") or summary.get("smoke_manifest_sha256")
    require("TASK061_SMOKE_CHUNK_SHA_PRESENT: #251 chunk sha exists", isinstance(expected_chunk_sha, str) and expected_chunk_sha.startswith("sha256:"))
    require("TASK061_SMOKE_MANIFEST_SHA_PRESENT: #251 manifest sha exists", isinstance(expected_manifest_sha, str) and expected_manifest_sha.startswith("sha256:"))
    require("TASK061_SMOKE_CHUNKS_FILE_EXISTS: #251 smoke chunks file exists", chunk_state.get("is_file") is True)
    require("TASK061_SMOKE_MANIFEST_FILE_EXISTS: #251 smoke manifest file exists", manifest_state.get("is_file") is True)
    require(
        "TASK061_SMOKE_CHUNKS_HASH_MATCH: live smoke chunks hash matches #251 READBACK",
        chunk_state.get("sha256") == expected_chunk_sha,
    )
    require(
        "TASK061_SMOKE_MANIFEST_HASH_MATCH: live smoke manifest hash matches #251 READBACK",
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
            "smoke_manifest_readback": str(args.smoke_manifest_readback),
            "smoke_chunks_jsonl": str(smoke_chunks),
            "smoke_manifest_jsonl": str(smoke_manifest),
        },
        "readback_state": readback_state,
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
    readback_root = args.readback_root.expanduser().resolve(strict=False)
    runner = validate_release_runner(args)
    # Durable-root policy. Default production root is aiwonder
    # /zfs/archive/contextgraph; override via CG_MEJEPA_DURABLE_ROOT only for
    # explicitly issue-scoped smoke/revalidation work.
    # The hot-root must NOT be under that durable root; cache_root and readback_root
    # MUST be under it. Windows /mnt/c path remains forbidden always.
    durable_root = Path(os.environ.get("CG_MEJEPA_DURABLE_ROOT", str(AIWONDER_DURABLE_ROOT))).expanduser().resolve(strict=False)
    require(
        "MEJEPA_SCHEDULER_HOT_ROOT_WINDOWS_MOUNT: hot root must not be under /mnt/c",
        not path_is_under(hot_root, Path("/mnt/c")),
    )
    require(
        f"MEJEPA_SCHEDULER_HOT_ROOT_DURABLE_MOUNT: hot root must not be under durable root {durable_root}",
        not path_is_under(hot_root, durable_root),
    )
    require(
        f"MEJEPA_SCHEDULER_CACHE_ROOT_NOT_DURABLE_ROOT: cache root must be under {durable_root}",
        path_is_under(cache_root, durable_root),
    )
    require(
        f"MEJEPA_SCHEDULER_READBACK_ROOT_NOT_DURABLE_ROOT: READBACK root must be under {durable_root}",
        path_is_under(readback_root, durable_root),
    )

    disks_raw = disk_snapshot()
    disks = parse_disk_snapshot(disks_raw)
    cpu = cpu_snapshot()
    gpu = gpu_snapshot(args)
    capacity = projected_capacity(args)
    worker_ceiling = min(int(cpu["logical_threads"]), int(cpu["affinity_logical_threads"]))
    require("MEJEPA_SCHEDULER_IO_WORKERS_INVALID: durable I/O workers is positive", args.io_workers > 0)
    require("MEJEPA_SCHEDULER_CPU_WORKERS_INVALID: CPU workers is positive", args.cpu_workers > 0)
    require("MEJEPA_SCHEDULER_GPU_WORKERS_INVALID: GPU workers is positive", args.gpu_workers > 0)
    require(
        "MEJEPA_SCHEDULER_IO_WORKERS_EXCEED_POLICY: durable/Docker I/O workers stay bounded",
        args.io_workers <= MAX_DURABLE_IO_WORKERS,
    )
    require(
        "MEJEPA_SCHEDULER_CPU_WORKERS_EXCEED_POLICY: CPU workers fit live host thread policy",
        args.cpu_workers <= worker_ceiling,
    )
    require(
        "MEJEPA_SCHEDULER_GPU_WORKERS_EXCEED_DEVICES: GPU workers fit detected CUDA devices",
        args.gpu_workers <= gpu["gpu_count"] * MAX_GPU_WORKERS_PER_DEVICE,
    )
    # Hot stage check uses root by default; override with CG_MEJEPA_HOT_DISK_TARGET
    # if the operator's hot root is on a non-root mount (e.g., /home on aiwonder).
    hot_disk = os.environ.get("CG_MEJEPA_HOT_DISK_TARGET", "/")
    require(
        f"MEJEPA_SCHEDULER_HOT_CAPACITY: hot root disk target {hot_disk} has planned free bytes",
        capacity["hot_stage_budget_bytes"] <= disks[hot_disk]["available_bytes"],
    )
    durable_disk = os.environ.get("CG_MEJEPA_DURABLE_DISK_TARGET", "/zfs/archive")
    require(
        f"MEJEPA_SCHEDULER_DURABLE_CAPACITY: durable root disk target {durable_disk} has planned free bytes",
        capacity["required_durable_available_bytes"] <= disks[durable_disk]["available_bytes"],
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
            "readback_root": str(readback_root),
            "durable_root": str(durable_root),
            "hot_root_on_aiwonder_hot_or_scratch": path_is_under(hot_root, AIWONDER_HOT_ROOT)
            or path_is_under(hot_root, AIWONDER_SCRATCH_ROOT),
            "cache_root_on_durable_root": path_is_under(cache_root, durable_root),
            "readback_root_on_durable_root": path_is_under(readback_root, durable_root),
        },
        "runner": runner,
        "capacity_policy": capacity,
        "worker_policy": {
            "selected": runner_batch_controls(args)["worker_tiers"],
            "max_durable_io_workers": MAX_DURABLE_IO_WORKERS,
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
    require("durable I/O worker count is positive", args.io_workers > 0)
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
                readback_root=args.readback_root / "shards" / shard_id,
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
    post_promotion_snapshot: Callable[[dict[str, Any]], dict[str, Any]] | None = None,
    seed_hot_stage_from_durable: bool = True,
) -> dict[str, Any]:
    stage_promotion: dict[str, Any] | None = None
    stage_setup: dict[str, Any] | None = None
    runner_shard = shard
    runner_hot_root = args.hot_root
    if use_hot_stage_for_shard(args):
        stage_root = hot_stage_root(args, shard, run_log_name)
        runner_shard = hot_stage_shard(args, shard, run_log_name)
        stage_setup = prepare_hot_stage(
            args,
            shard,
            runner_shard,
            stage_root,
            seed_from_durable=seed_hot_stage_from_durable,
        )
        runner_hot_root = stage_root / "runner-hot"

    env = os.environ.copy()
    env.update(
        {
            "CG_MEJEPA_PY_FORWARD_CACHE_ROOT": str(runner_shard.cache_root),
            "CG_MEJEPA_PY_FORWARD_CACHE_READBACK_ROOT": str(runner_shard.readback_root),
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
            "CG_MEJEPA_PY_FORWARD_CORPUS_PROFILE": DEFAULT_CORPUS_PROFILE,
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
            "readback_root": str(shard.readback_root),
            "runner_cache_root": str(runner_shard.cache_root),
            "runner_readback_root": str(runner_shard.readback_root),
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
    run_log_path = args.readback_root / "run_logs" / (run_log_name or f"{shard.shard_id}.json")
    json_dump_checked(run_log_path, run_log)
    require(f"{shard.shard_id} runner exited zero", completed.returncode == 0)
    timing_ms = {"runner_subprocess_ms": round(elapsed * 1000.0, 3)}
    if use_hot_stage_for_shard(args):
        stage_readback_started = time.monotonic()
        stage_readback = shard_completion_readback(args, runner_shard, require_packed_store)
        timing_ms["stage_readback_ms"] = elapsed_ms(stage_readback_started)
        promotion_started = time.monotonic()
        # Serialize promotion against concurrent isolated subruns (#302). The
        # GPU forward work above already completed in parallel; this lock only
        # gates the brief mv/archive into the shared durable destination.
        with _PROMOTE_HOT_STAGE_LOCK:
            stage_promotion = promote_hot_stage_shard(args, shard, runner_shard, stage_root, stage_readback)
            timing_ms["hot_stage_promotion_ms"] = elapsed_ms(promotion_started)
            run_log["hot_stage_promotion"] = stage_promotion
            durable_readback_started = time.monotonic()
            run_log.update(shard_completion_readback(args, shard, require_packed_store))
            timing_ms["durable_readback_ms"] = elapsed_ms(durable_readback_started)
            if post_promotion_snapshot is not None:
                run_log["post_promotion_snapshot"] = post_promotion_snapshot(run_log)
            run_log["timing_ms"] = timing_ms
            run_log["skipped_completed"] = False
            json_dump_checked(run_log_path, run_log)
        event(event_path, event_phase, run_log)
        return run_log
    durable_readback_started = time.monotonic()
    run_log.update(shard_completion_readback(args, shard, require_packed_store))
    timing_ms["durable_readback_ms"] = elapsed_ms(durable_readback_started)
    run_log["timing_ms"] = timing_ms
    run_log["skipped_completed"] = False
    json_dump_checked(run_log_path, run_log)
    event(event_path, event_phase, run_log)
    return run_log


def copy_file_with_readback(source: Path, destination: Path) -> dict[str, Any]:
    require(f"{source} exists before isolated snapshot copy", source.is_file())
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    source_bytes = source.read_bytes()
    destination_bytes = destination.read_bytes()
    require(
        f"{destination} isolated snapshot byte readback matches source",
        destination_bytes == source_bytes,
    )
    return {
        "source": file_state(source),
        "destination": file_state(destination),
    }


def isolated_subrun_snapshot_root(args: argparse.Namespace, shard: ShardSpec) -> Path:
    return args.readback_root / "isolated_subrun_rows" / shard.shard_id


def snapshot_isolated_subrun_rows(
    args: argparse.Namespace,
    shard: ShardSpec,
    slug: str,
    subrun: dict[str, Any],
) -> dict[str, Any]:
    manifest = read_json(shard.manifest_path())
    inputs = manifest.get("inputs", {})
    summary = manifest.get("summary", {})
    require(f"{shard.shard_id} {slug} manifest records only_embedder", inputs.get("only_embedder") == slug)
    require(f"{shard.shard_id} {slug} manifest active count is one", summary.get("active_embedder_count") == 1)
    require(f"{shard.shard_id} {slug} manifest offset matches shard", inputs.get("chunk_offset") == shard.chunk_offset)
    require(f"{shard.shard_id} {slug} manifest size matches shard", inputs.get("max_chunks") == shard.max_chunks)
    require(
        f"{shard.shard_id} {slug} manifest chunks source matches scheduler",
        str(inputs.get("chunks_jsonl") or "") == str(args.chunks_jsonl),
    )
    batch_telemetry_path = Path(str(manifest.get("source_of_truth", {}).get("batch_telemetry_jsonl", "")))
    snapshot_root = isolated_subrun_snapshot_root(args, shard)
    files = {
        "manifest": copy_file_with_readback(shard.manifest_path(), snapshot_root / f"{slug}.manifest.json"),
        "readback": copy_file_with_readback(shard.readback_path(), snapshot_root / f"{slug}.readback.json"),
        "index": copy_file_with_readback(shard.index_path(), snapshot_root / f"{slug}.index.jsonl"),
        "quarantine": copy_file_with_readback(shard.quarantine_path(), snapshot_root / f"{slug}.quarantine.jsonl"),
        "batch_telemetry": copy_file_with_readback(batch_telemetry_path, snapshot_root / f"{slug}.batch_telemetry.jsonl"),
    }
    index_rows = count_jsonl_rows(Path(files["index"]["destination"]["path"]))
    quarantine_rows = count_jsonl_rows(Path(files["quarantine"]["destination"]["path"]))
    batch_rows = count_jsonl_rows(Path(files["batch_telemetry"]["destination"]["path"]))
    require(f"{shard.shard_id} {slug} snapshot index rows match manifest", index_rows == summary.get("cache_record_count"))
    require(
        f"{shard.shard_id} {slug} snapshot quarantine rows match manifest",
        quarantine_rows == summary.get("quarantine_row_count"),
    )
    require(
        f"{shard.shard_id} {slug} snapshot batch telemetry rows match manifest",
        batch_rows == summary.get("batch_telemetry_row_count"),
    )
    packed_records = snapshot_isolated_subrun_packed_records(snapshot_root, slug, manifest, index_rows)
    return {
        "embedder_id": slug,
        "snapshot_root": str(snapshot_root),
        "files": files,
        "packed_records": packed_records,
        "run_log": str(args.readback_root / "run_logs" / f"{shard.shard_id}.{slug}.json"),
        "run_log_sha256": sha256_file(args.readback_root / "run_logs" / f"{shard.shard_id}.{slug}.json"),
        "manifest_summary": summary,
        "manifest_sha256": sha256_file(shard.manifest_path()),
        "readback_sha256": sha256_file(shard.readback_path()),
        "index_row_count": index_rows,
        "quarantine_row_count": quarantine_rows,
        "batch_telemetry_row_count": batch_rows,
        "subrun_elapsed_seconds": subrun.get("elapsed_seconds"),
        "passes": True,
    }


def snapshot_isolated_subrun_packed_records(
    snapshot_root: Path,
    slug: str,
    manifest: dict[str, Any],
    expected_rows: int,
) -> dict[str, Any]:
    source_of_truth = manifest.get("source_of_truth", {})
    source_root = Path(str(source_of_truth.get("packed_record_root", "")))
    require(f"{slug} packed record root exists before snapshot", source_root.is_dir())
    destination_root = snapshot_root / f"{slug}.packed_records"
    if destination_root.exists():
        shutil.rmtree(destination_root)
    destination_root.mkdir(parents=True, exist_ok=False)

    source_record = source_root / f"{slug}.jsonl"
    source_index = source_root / f"{slug}.index.jsonl"
    destination_record = destination_root / f"{slug}.jsonl"
    destination_index = destination_root / f"{slug}.index.jsonl"
    if expected_rows > 0:
        copy_file_with_readback(source_record, destination_record)
        copy_file_with_readback(source_index, destination_index)
        rows = read_jsonl_rows(destination_index)
        require(f"{slug} packed snapshot index rows match current subrun", len(rows) == expected_rows)
        for row in rows:
            row["packed_record_path"] = str(destination_record)
        jsonl_write_checked(destination_index, rows)
    else:
        destination_record.write_bytes(b"")
        require(f"{slug} zero-row packed record snapshot is empty", destination_record.read_bytes() == b"")
        jsonl_write_checked(destination_index, [])
    payload_hash_readback = validate_packed_payload_hashes(destination_root)
    require(
        f"{slug} packed snapshot payload row count matches current subrun",
        payload_hash_readback.get("row_count") == expected_rows,
    )
    return {
        "root": str(destination_root),
        "record": file_state(destination_record),
        "index": file_state(destination_index),
        "row_count": expected_rows,
        "payload_hash_readback": payload_hash_readback,
        "passes": True,
    }


def merge_isolated_packed_records(
    shard: ShardSpec,
    snapshots: list[dict[str, Any]],
    per_embedder_expected_rows: dict[str, int],
) -> dict[str, Any]:
    packed_root = shard.cache_root / "packed_records"
    if packed_root.exists():
        shutil.rmtree(packed_root)
    packed_root.mkdir(parents=True, exist_ok=False)
    per_embedder: dict[str, Any] = {}
    merge_inputs: dict[str, Any] = {}
    total_rows = 0
    for snapshot in snapshots:
        slug = snapshot["embedder_id"]
        expected_rows = per_embedder_expected_rows[slug]
        packed_snapshot = snapshot.get("packed_records")
        require(f"{shard.shard_id} {slug} packed snapshot exists", isinstance(packed_snapshot, dict))
        source_record = Path(str((packed_snapshot.get("record") or {}).get("path", "")))
        source_index = Path(str((packed_snapshot.get("index") or {}).get("path", "")))
        require(f"{shard.shard_id} {slug} packed snapshot record exists", source_record.is_file())
        require(f"{shard.shard_id} {slug} packed snapshot index exists", source_index.is_file())

        destination_record = packed_root / f"{slug}.jsonl"
        destination_index = packed_root / f"{slug}.index.jsonl"
        shutil.copy2(source_record, destination_record)
        require(
            f"{shard.shard_id} {slug} packed payload copy byte equality",
            destination_record.read_bytes() == source_record.read_bytes(),
        )
        rows = read_jsonl_rows(source_index)
        require(f"{shard.shard_id} {slug} packed snapshot rows match expected", len(rows) == expected_rows)
        for row in rows:
            row["packed_record_path"] = str(destination_record)
        jsonl_write_checked(destination_index, rows)
        require(
            f"{shard.shard_id} {slug} merged packed index rows match expected",
            count_jsonl_rows(destination_index) == expected_rows,
        )
        per_embedder[slug] = {
            "record_path": str(destination_record),
            "record_sha256": sha256_file(destination_record),
            "byte_count": destination_record.stat().st_size,
            "index_path": str(destination_index),
            "index_sha256": sha256_file(destination_index),
            "row_count": expected_rows,
        }
        merge_inputs[slug] = {
            "snapshot_record": str(source_record),
            "snapshot_index": str(source_index),
            "snapshot_row_count": expected_rows,
        }
        total_rows += expected_rows

    packed_manifest_path = shard.cache_root / "python_forward_cache_packed_manifest.json"
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "python_forward_cache_packed_manifest",
        "created_at_utc": utc_now(),
        "layout": "packed-jsonl",
        "packed_record_root": str(packed_root),
        "row_count": total_rows,
        "per_embedder": per_embedder,
        "merge_source": "isolated_subrun_snapshots",
        "merge_inputs": merge_inputs,
        "passes": True,
    }
    json_dump_checked(packed_manifest_path, manifest)
    readback = read_json(packed_manifest_path)
    require(f"{shard.shard_id} merged packed manifest readback passes", readback.get("passes") is True)
    require(f"{shard.shard_id} merged packed manifest row count matches", readback.get("row_count") == total_rows)
    payload_hash_readback = validate_packed_payload_hashes(packed_root)
    require(
        f"{shard.shard_id} merged packed payload row count matches manifest",
        payload_hash_readback.get("row_count") == total_rows,
    )
    return {
        "packed_manifest_path": str(packed_manifest_path),
        "packed_manifest_sha256": sha256_file(packed_manifest_path),
        "packed_record_root": str(packed_root),
        "manifest": manifest,
        "payload_hash_readback": payload_hash_readback,
        "row_count": total_rows,
        "per_embedder": per_embedder,
        "passes": True,
    }


def aggregate_hot_stage_promotion(subruns: list[dict[str, Any]]) -> dict[str, Any] | None:
    promotions = [subrun.get("hot_stage_promotion") for subrun in subruns if isinstance(subrun.get("hot_stage_promotion"), dict)]
    if not promotions:
        return None
    timing_keys = (
        "path_rewrite_ms",
        "cache_promotion_ms",
        "readback_promotion_ms",
        "durable_readback_ms",
        "stage_cleanup_ms",
        "total_ms",
    )
    timing_ms = {
        key: round(sum(float(promotion.get("timing_ms", {}).get(key, 0.0)) for promotion in promotions), 3)
        for key in timing_keys
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_isolated_hot_stage_promotion_summary",
        "created_at_utc": utc_now(),
        "subrun_count": len(promotions),
        "timing_ms": timing_ms,
        "promotion_artifacts": [
            {
                "artifact_kind": promotion.get("artifact_kind"),
                "stage_root": promotion.get("stage_root"),
                "passes": promotion.get("passes"),
            }
            for promotion in promotions
        ],
        "passes": True,
    }


def consolidate_isolated_subruns(
    args: argparse.Namespace,
    shard: ShardSpec,
    snapshots: list[dict[str, Any]],
    subruns: list[dict[str, Any]],
) -> dict[str, Any]:
    started = time.monotonic()
    observed_slugs = tuple(snapshot["embedder_id"] for snapshot in snapshots)
    require(
        f"{shard.shard_id} isolated subrun order matches required embedders",
        observed_slugs == ISOLATED_FORWARD_SLUGS,
    )
    require(
        f"{shard.shard_id} isolated subrun count matches scheduler active embedders",
        len(snapshots) == args.active_embedder_count,
    )
    index_rows: list[Any] = []
    quarantine_rows: list[Any] = []
    batch_telemetry_rows: list[Any] = []
    summary_totals = {
        "cache_record_count": 0,
        "first_pass_hits": 0,
        "first_pass_writes": 0,
        "second_pass_hits": 0,
        "quarantine_row_count": 0,
        "batch_telemetry_row_count": 0,
    }
    chunk_count: int | None = None
    per_embedder_expected_rows: dict[str, int] = {}
    per_embedder_coverage: dict[str, Any] = {}
    source_manifest = read_json(Path(str(snapshots[-1]["files"]["manifest"]["destination"]["path"])))
    for snapshot in snapshots:
        slug = snapshot["embedder_id"]
        manifest = read_json(Path(str(snapshot["files"]["manifest"]["destination"]["path"])))
        inputs = manifest.get("inputs", {})
        summary = manifest.get("summary", {})
        require(f"{shard.shard_id} {slug} snapshot manifest only_embedder matches", inputs.get("only_embedder") == slug)
        require(f"{shard.shard_id} {slug} snapshot active count is one", summary.get("active_embedder_count") == 1)
        current_chunk_count = int(summary.get("chunk_count", -1))
        require(f"{shard.shard_id} {slug} snapshot chunk count positive", current_chunk_count > 0)
        if chunk_count is None:
            chunk_count = current_chunk_count
        require(f"{shard.shard_id} {slug} chunk count matches peer subruns", current_chunk_count == chunk_count)
        current_index_rows = read_jsonl_rows(Path(str(snapshot["files"]["index"]["destination"]["path"])))
        current_quarantine_rows = read_jsonl_rows(Path(str(snapshot["files"]["quarantine"]["destination"]["path"])))
        current_batch_rows = read_jsonl_rows(Path(str(snapshot["files"]["batch_telemetry"]["destination"]["path"])))
        require(f"{shard.shard_id} {slug} current index rows match snapshot", len(current_index_rows) == snapshot["index_row_count"])
        require(
            f"{shard.shard_id} {slug} current quarantine rows match snapshot",
            len(current_quarantine_rows) == snapshot["quarantine_row_count"],
        )
        require(
            f"{shard.shard_id} {slug} current batch rows match snapshot",
            len(current_batch_rows) == snapshot["batch_telemetry_row_count"],
        )
        per_embedder_expected_rows[slug] = len(current_index_rows)
        index_rows.extend(current_index_rows)
        quarantine_rows.extend(current_quarantine_rows)
        batch_telemetry_rows.extend(current_batch_rows)
        for key in summary_totals:
            summary_totals[key] += int(summary.get(key, 0))
        coverage_value = manifest.get("per_embedder_coverage") or {}
        if isinstance(coverage_value, dict):
            coverage_items = coverage_value.items()
        elif isinstance(coverage_value, list):
            coverage_items = (
                (row.get("embedder_id"), row)
                for row in coverage_value
                if isinstance(row, dict)
            )
        else:
            raise SchedulerError(f"{shard.shard_id} {slug} per_embedder_coverage has unsupported type")
        for embedder_id, coverage in coverage_items:
            if embedder_id == slug:
                per_embedder_coverage[embedder_id] = coverage
    require(f"{shard.shard_id} isolated chunk count established", chunk_count is not None)
    require(
        f"{shard.shard_id} isolated second pass covers cache records",
        summary_totals["second_pass_hits"] == summary_totals["cache_record_count"],
    )
    require(
        f"{shard.shard_id} isolated index rows match summed summaries",
        len(index_rows) == summary_totals["cache_record_count"],
    )
    require(
        f"{shard.shard_id} isolated quarantine rows match summed summaries",
        len(quarantine_rows) == summary_totals["quarantine_row_count"],
    )
    require(
        f"{shard.shard_id} isolated batch rows match summed summaries",
        len(batch_telemetry_rows) == summary_totals["batch_telemetry_row_count"],
    )
    expected_pairs = int(chunk_count) * args.active_embedder_count
    accounted_pairs = summary_totals["cache_record_count"] + summary_totals["quarantine_row_count"]
    require(f"{shard.shard_id} isolated consolidation accounts all pairs", accounted_pairs == expected_pairs)

    jsonl_write_checked(shard.index_path(), index_rows)
    jsonl_write_checked(shard.quarantine_path(), quarantine_rows)
    batch_telemetry_path = shard.cache_root / "python_forward_cache_batch_telemetry.jsonl"
    jsonl_write_checked(batch_telemetry_path, batch_telemetry_rows)

    packed_manifest_path = shard.cache_root / "python_forward_cache_packed_manifest.json"
    packed_root = shard.cache_root / "packed_records"
    packed_merge = merge_isolated_packed_records(shard, snapshots, per_embedder_expected_rows)
    packed_manifest = packed_merge["manifest"]
    require(f"{shard.shard_id} isolated packed manifest passes", packed_manifest.get("passes") is True)
    require(f"{shard.shard_id} isolated packed layout is packed-jsonl", packed_manifest.get("layout") == "packed-jsonl")
    packed_index_rows = count_packed_index_rows(packed_root)
    require(f"{shard.shard_id} isolated packed rows match index rows", packed_index_rows == len(index_rows))
    require(f"{shard.shard_id} isolated packed manifest row count matches", packed_manifest.get("row_count") == packed_index_rows)
    packed_per_embedder = packed_manifest.get("per_embedder", {})
    require(f"{shard.shard_id} isolated packed per_embedder object exists", isinstance(packed_per_embedder, dict))
    for slug, expected_rows in per_embedder_expected_rows.items():
        if expected_rows > 0:
            require(f"{shard.shard_id} isolated packed manifest includes {slug}", slug in packed_per_embedder)
            require(
                f"{shard.shard_id} isolated packed {slug} rows match snapshot",
                packed_per_embedder[slug].get("row_count") == expected_rows,
            )
        elif slug in packed_per_embedder:
            require(
                f"{shard.shard_id} isolated packed {slug} zero-row entry matches snapshot",
                packed_per_embedder[slug].get("row_count") == 0,
            )
    payload_hash_readback = packed_merge["payload_hash_readback"]
    require(
        f"{shard.shard_id} isolated packed payload row count matches index",
        payload_hash_readback.get("row_count") == len(index_rows),
    )

    final_summary = {
        **summary_totals,
        "chunk_count": int(chunk_count),
        "active_embedder_count": args.active_embedder_count,
        "expected_pairs": expected_pairs,
        "accounted_pairs": accounted_pairs,
        "global_cache_record_count_at_manifest_write": packed_index_rows,
    }
    inputs = dict(source_manifest.get("inputs", {}))
    inputs.pop("only_embedder", None)
    inputs["isolated_embedder_processes"] = True
    inputs["isolated_embedder_order"] = list(ISOLATED_FORWARD_SLUGS)
    inputs["cache_store_layout"] = "packed-jsonl"
    source_of_truth = dict(source_manifest.get("source_of_truth", {}))
    source_of_truth["index_jsonl"] = str(shard.index_path())
    source_of_truth["quarantine_jsonl"] = str(shard.quarantine_path())
    source_of_truth["batch_telemetry_jsonl"] = str(batch_telemetry_path)
    source_of_truth["packed_manifest"] = str(packed_manifest_path)
    source_of_truth["packed_record_root"] = str(packed_root)
    final_manifest = {
        **source_manifest,
        "status": "passed",
        "passes": True,
        "inputs": inputs,
        "source_of_truth": source_of_truth,
        "summary": final_summary,
        "per_embedder_coverage": per_embedder_coverage,
        "isolated_consolidation": {
            "embedder_order": list(ISOLATED_FORWARD_SLUGS),
            "subrun_snapshot_root": str(isolated_subrun_snapshot_root(args, shard)),
            "subrun_count": len(snapshots),
            "snapshot_manifest_sha256": {
                snapshot["embedder_id"]: snapshot["manifest_sha256"] for snapshot in snapshots
            },
            "packed_merge": {
                "packed_manifest_sha256": packed_merge["packed_manifest_sha256"],
                "row_count": packed_merge["row_count"],
                "per_embedder": packed_merge["per_embedder"],
            },
        },
    }
    json_dump_checked(shard.manifest_path(), final_manifest)
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_isolated_shard_consolidation_readback",
        "created_at_utc": utc_now(),
        "shard_id": shard.shard_id,
        "source_of_truth": {
            "manifest": str(shard.manifest_path()),
            "index_jsonl": str(shard.index_path()),
            "quarantine_jsonl": str(shard.quarantine_path()),
            "batch_telemetry_jsonl": str(batch_telemetry_path),
            "packed_manifest": str(packed_manifest_path),
            "packed_record_root": str(packed_root),
            "subrun_snapshot_root": str(isolated_subrun_snapshot_root(args, shard)),
        },
        "summary": final_summary,
        "subrun_snapshots": snapshots,
        "packed_merge": packed_merge,
        "packed_payload_hash_readback": payload_hash_readback,
        "passes": True,
    }
    json_dump_checked(shard.readback_path(), readback)
    durable_readback_started = time.monotonic()
    readback = shard_completion_readback(args, shard)
    require_full_scheduler_shard(args, shard, readback["summary"])
    timing_ms = {
        "isolated_row_consolidation_ms": elapsed_ms(started),
        "durable_readback_ms": elapsed_ms(durable_readback_started),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_isolated_shard_consolidation",
        "created_at_utc": utc_now(),
        "shard_id": shard.shard_id,
        "summary": final_summary,
        "subrun_snapshots": snapshots,
        "packed_payload_hash_readback": payload_hash_readback,
        "readback": readback,
        "timing_ms": timing_ms,
        "passes": True,
    }


def run_shard_embedder_isolated(
    args: argparse.Namespace,
    shard: ShardSpec,
    event_path: Path,
    preflight: dict[str, Any],
) -> dict[str, Any]:
    require(
        "isolated embedder order covers scheduler active embedder count",
        len(ISOLATED_FORWARD_SLUGS) == args.active_embedder_count,
    )
    event(
        event_path,
        "shard_embedder_isolation_start",
        {
            "shard_id": shard.shard_id,
            "embedder_order": list(ISOLATED_FORWARD_SLUGS),
        },
    )
    subruns: list[dict[str, Any]] = []
    snapshots: list[dict[str, Any]] = []
    isolated_started = time.monotonic()
    first_resource_before: dict[str, Any] | None = None

    # Parallel-isolated mode: launch N concurrent isolated embedder subprocesses
    # on the same GPU. CUDA MPS (Multi-Process Service) should be enabled for
    # actual kernel overlap; otherwise CUDA time-slices but we still amortise
    # the subprocess spawn cost across N waves instead of one process per
    # active static-AST embedder.
    # Configurable via CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS env var; default
    # 1 preserves the legacy sequential path. Recommended: 4-6 for RTX 5090
    # (32 GB VRAM, peak ~3.5 GB per embedder process incl. CUDA context).
    try:
        parallel_workers = int(os.environ.get("CG_MEJEPA_PY_FORWARD_PARALLEL_EMBEDDERS", "1"))
    except ValueError:
        parallel_workers = 1
    parallel_workers = max(1, min(parallel_workers, len(ISOLATED_FORWARD_SLUGS)))
    if parallel_workers > 1:
        require(
            "parallel isolated embedder mode requires packed hot-stage isolation",
            use_hot_stage_for_shard(args),
        )

    def _run_one_isolated_subrun(slug: str) -> tuple[str, dict[str, Any], dict[str, Any]]:
        extra_env = {"CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER": slug}
        if parallel_workers > 1:
            extra_env["CG_MEJEPA_PY_FORWARD_SKIP_EXISTING_PACKED_READ"] = "1"
        snapshot_holder: dict[str, Any] = {}

        def _snapshot_after_promotion(subrun_log: dict[str, Any]) -> dict[str, Any]:
            snapshot = snapshot_isolated_subrun_rows(args, shard, slug, subrun_log)
            snapshot_holder["snapshot"] = snapshot
            return {
                "embedder_id": slug,
                "manifest_sha256": snapshot["manifest_sha256"],
                "packed_row_count": snapshot["packed_records"]["row_count"],
                "passes": True,
            }

        subrun_inner = run_shard(
            args,
            shard,
            event_path,
            preflight,
            extra_env=extra_env,
            run_log_name=f"{shard.shard_id}.{slug}.json",
            event_phase="shard_embedder_subrun_complete",
            require_packed_store=False,
            post_promotion_snapshot=_snapshot_after_promotion if use_hot_stage_for_shard(args) else None,
            seed_hot_stage_from_durable=parallel_workers == 1,
        )
        snapshot_inner = snapshot_holder.get("snapshot")
        if snapshot_inner is None:
            snapshot_inner = snapshot_isolated_subrun_rows(args, shard, slug, subrun_inner)
        else:
            snapshot_inner["run_log_sha256"] = sha256_file(args.readback_root / "run_logs" / f"{shard.shard_id}.{slug}.json")
        return slug, subrun_inner, snapshot_inner

    if parallel_workers == 1:
        # Legacy sequential path — preserves byte-for-byte equivalence with the
        # pre-2026-05-15 scheduler when env var is unset / set to 1.
        for slug in ISOLATED_FORWARD_SLUGS:
            slug_done, subrun, snapshot = _run_one_isolated_subrun(slug)
            if first_resource_before is None:
                first_resource_before = subrun.get("resource_before")
            snapshots.append(snapshot)
            subruns.append(
                {
                    "embedder_id": slug_done,
                    "run_log": str(args.readback_root / "run_logs" / f"{shard.shard_id}.{slug_done}.json"),
                    "run_log_sha256": snapshot["run_log_sha256"],
                    "manifest_sha256": snapshot["manifest_sha256"],
                    "readback_sha256": snapshot["readback_sha256"],
                    "summary": subrun.get("summary"),
                    "snapshot": snapshot,
                    "timing_ms": subrun.get("timing_ms"),
                    "hot_stage_promotion": subrun.get("hot_stage_promotion"),
                }
            )
    else:
        # Parallel path: launch up to parallel_workers concurrent embedder
        # subprocesses. Results are collected, then re-ordered to match
        # ISOLATED_FORWARD_SLUGS so downstream consolidation sees the same
        # deterministic order regardless of completion sequence.
        event(
            event_path,
            "shard_embedder_isolation_parallel_dispatch",
            {
                "shard_id": shard.shard_id,
                "parallel_workers": parallel_workers,
                "slug_count": len(ISOLATED_FORWARD_SLUGS),
            },
        )
        results_by_slug: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
        with concurrent.futures.ThreadPoolExecutor(
            max_workers=parallel_workers,
            thread_name_prefix="isolated_subrun",
        ) as executor:
            future_to_slug = {
                executor.submit(_run_one_isolated_subrun, slug): slug
                for slug in ISOLATED_FORWARD_SLUGS
            }
            for future in concurrent.futures.as_completed(future_to_slug):
                slug_done, subrun_done, snapshot_done = future.result()
                results_by_slug[slug_done] = (subrun_done, snapshot_done)
        # Re-order deterministically by ISOLATED_FORWARD_SLUGS so consolidation
        # is identical to the sequential path.
        for slug in ISOLATED_FORWARD_SLUGS:
            subrun, snapshot = results_by_slug[slug]
            if first_resource_before is None:
                first_resource_before = subrun.get("resource_before")
            snapshots.append(snapshot)
            subruns.append(
                {
                    "embedder_id": slug,
                    "run_log": str(args.readback_root / "run_logs" / f"{shard.shard_id}.{slug}.json"),
                    "run_log_sha256": snapshot["run_log_sha256"],
                    "manifest_sha256": snapshot["manifest_sha256"],
                    "readback_sha256": snapshot["readback_sha256"],
                    "summary": subrun.get("summary"),
                    "snapshot": snapshot,
                    "timing_ms": subrun.get("timing_ms"),
                    "hot_stage_promotion": subrun.get("hot_stage_promotion"),
                }
            )
    consolidation = consolidate_isolated_subruns(args, shard, snapshots, subruns)
    final_readback = consolidation["readback"]
    runner_subprocess_ms = round(
        sum(
            float((subrun.get("timing_ms") or {}).get("runner_subprocess_ms", 0.0))
            for subrun in subruns
        ),
        3,
    )
    consolidation_timing = consolidation.get("timing_ms", {})
    final_resource_after = resource_snapshot(args, f"{shard.shard_id}_isolated_after")
    final_run = {
        "shard_id": shard.shard_id,
        "exit_code": 0,
        "elapsed_seconds": round(time.monotonic() - isolated_started, 3),
        "runner_batch_controls": runner_batch_controls(args),
        "runner_extra_env": {"CG_MEJEPA_PY_FORWARD_ONLY_EMBEDDER": "isolated_per_embedder"},
        "host_preflight": preflight,
        "resource_before": first_resource_before or resource_snapshot(args, f"{shard.shard_id}_isolated_before_missing"),
        "resource_after": final_resource_after,
        "stdout_tail": "",
        "stderr_tail": "",
        "timing_ms": {
            "runner_subprocess_ms": runner_subprocess_ms,
            "isolated_row_consolidation_ms": consolidation_timing.get("isolated_row_consolidation_ms", 0.0),
            "durable_readback_ms": consolidation_timing.get("durable_readback_ms", 0.0),
        },
        "skipped_completed": False,
        "isolated_embedder_subruns": subruns,
        "isolated_consolidation": consolidation,
        **final_readback,
    }
    promotion_summary = aggregate_hot_stage_promotion(subruns)
    if promotion_summary is not None:
        final_run["hot_stage_promotion"] = promotion_summary
    json_dump_checked(shard_run_log_path(args, shard), final_run)
    event(event_path, "shard_run_complete", final_run)
    event(event_path, "shard_run_embedder_isolated_complete", final_run)
    return final_run


def load_existing_run_logs(readback_root: Path) -> list[dict[str, Any]]:
    run_log_root = readback_root / "run_logs"
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


def validate_shard_readback(shard: ShardSpec) -> dict[str, Any]:
    readback = read_json(shard.readback_path())
    require(f"{shard.shard_id} shard READBACK passes", readback.get("passes") is True)
    return {
        "readback_path": str(shard.readback_path()),
        "readback_sha256": sha256_file(shard.readback_path()),
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
    readback_readback = validate_shard_readback(shard)
    return {
        "manifest_path": str(shard.manifest_path()),
        "manifest_sha256": sha256_file(shard.manifest_path()),
        **readback_readback,
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


def resume_skip_log_path(args: argparse.Namespace, shard: ShardSpec) -> Path:
    return args.readback_root / "resume_skipped_run_logs" / f"{shard.shard_id}.json"


def completed_shard_bounded_readback(args: argparse.Namespace, shard: ShardSpec) -> dict[str, Any]:
    readback = shard_completion_readback(args, shard, require_packed_store=False)
    summary = readback.get("summary")
    require(f"{shard.shard_id} bounded completed shard summary exists", isinstance(summary, dict))
    require_full_scheduler_shard(args, shard, summary)

    existing_run_log_path = shard_run_log_path(args, shard)
    existing_run_log_state = file_state(existing_run_log_path)
    require(f"{shard.shard_id} bounded completed shard run log exists", existing_run_log_state.get("exists") is True)
    existing_run_log = read_json(existing_run_log_path)
    require(f"{shard.shard_id} bounded completed shard run log exited zero", existing_run_log.get("exit_code") == 0)
    require(f"{shard.shard_id} bounded completed shard run log id matches", existing_run_log.get("shard_id") == shard.shard_id)
    require(
        f"{shard.shard_id} bounded completed shard run log summary matches manifest",
        existing_run_log.get("summary") == summary,
    )

    manifest = read_json(shard.manifest_path())
    source_of_truth = manifest.get("source_of_truth", {})
    layout = manifest.get("inputs", {}).get("cache_store_layout", "legacy-files")
    packed_artifacts = None
    if layout == "packed-jsonl":
        packed_manifest = Path(str(source_of_truth.get("packed_manifest", "")))
        packed_record_root = Path(str(source_of_truth.get("packed_record_root", "")))
        packed_artifacts = {
            "packed_manifest": file_state(packed_manifest),
            "packed_record_root": directory_state(packed_record_root),
        }
        require(
            f"{shard.shard_id} bounded completed shard packed manifest exists",
            packed_artifacts["packed_manifest"].get("exists") is True,
        )
        require(
            f"{shard.shard_id} bounded completed shard packed record root exists",
            packed_artifacts["packed_record_root"].get("is_dir") is True,
        )

    return {
        "shard_id": shard.shard_id,
        "exit_code": 0,
        "elapsed_seconds": 0.0,
        "skipped_completed": True,
        "resume_readback_mode": "bounded_completed_shard_manifest_readback",
        "runner_batch_controls": runner_batch_controls(args),
        "existing_run_log": existing_run_log_state,
        "bounded_artifacts": {
            "manifest": file_state(shard.manifest_path()),
            "readback": file_state(shard.readback_path()),
            "index": file_state(shard.index_path()),
            "quarantine": file_state(shard.quarantine_path()),
            "packed": packed_artifacts,
        },
        **readback,
    }


def shard_run_log_path(args: argparse.Namespace, shard: ShardSpec) -> Path:
    return args.readback_root / "run_logs" / f"{shard.shard_id}.json"


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
        "readback_root": tree_state(shard.readback_root),
        "run_log": file_state(run_log_path),
    }
    archive_root = unique_archive_root(
        args.readback_root / "failed_partial_shards",
        f"{utc_now().replace('-', '').replace(':', '')}-{shard.shard_id}",
    )
    archive_root.mkdir(parents=True, exist_ok=False)
    moved: dict[str, str] = {}
    for label, source in (
        ("cache_root", shard.cache_root),
        ("readback_root", shard.readback_root),
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
            "readback_root": tree_state(shard.readback_root),
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
        args.readback_root / "failed_hot_stages",
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
        readback_root=root / "readback",
    )


def use_hot_stage_for_shard(args: argparse.Namespace) -> bool:
    return args.stage_packed_shards_on_hot and args.cache_store_layout == "packed"


def prepare_hot_stage(
    args: argparse.Namespace,
    durable_shard: ShardSpec,
    stage_shard: ShardSpec,
    stage_root: Path,
    seed_from_durable: bool = True,
) -> dict[str, Any]:
    require("packed hot-stage root is not /mnt/c", not path_is_under(stage_root.resolve(strict=False), Path("/mnt/c")))
    archive = archive_interrupted_hot_stage(
        args,
        durable_shard,
        stage_root,
        "new hot-stage run refuses to reuse stale interrupted stage state",
    )
    stage_root.mkdir(parents=True, exist_ok=False)
    cache_seeded = seed_from_durable and durable_shard.cache_root.exists()
    readback_seeded = seed_from_durable and durable_shard.readback_root.exists()
    if cache_seeded:
        shutil.copytree(durable_shard.cache_root, stage_shard.cache_root)
    else:
        stage_shard.cache_root.mkdir(parents=True)
    if readback_seeded:
        shutil.copytree(durable_shard.readback_root, stage_shard.readback_root)
    else:
        stage_shard.readback_root.mkdir(parents=True)
    stage_path_rewrites = (
        rewrite_json_paths_in_tree(
            stage_shard.cache_root,
            {
                str(durable_shard.cache_root): str(stage_shard.cache_root),
                str(durable_shard.readback_root): str(stage_shard.readback_root),
            },
        )
        + rewrite_json_paths_in_tree(
            stage_shard.readback_root,
            {
                str(durable_shard.cache_root): str(stage_shard.cache_root),
                str(durable_shard.readback_root): str(stage_shard.readback_root),
            },
        )
    )
    stage_hot_root = stage_root / "runner-hot"
    stage_hot_root.mkdir(parents=True, exist_ok=True)
    os.chmod(stage_hot_root, 0o700)
    return {
        "stage_root": str(stage_root),
        "stage_cache_root": str(stage_shard.cache_root),
        "stage_readback_root": str(stage_shard.readback_root),
        "stage_runner_hot_root": str(stage_hot_root),
        "seed_from_durable": seed_from_durable,
        "durable_seed": {
            "cache_root": cache_seeded,
            "readback_root": readback_seeded,
        },
        "interrupted_stage_archive": archive,
        "stage_path_rewrites": stage_path_rewrites,
        "before_state": {
            "durable_cache_root": tree_state(durable_shard.cache_root),
            "durable_readback_root": tree_state(durable_shard.readback_root),
        },
        "stage_before_runner": {
            "stage_cache_root": tree_state(stage_shard.cache_root),
            "stage_readback_root": tree_state(stage_shard.readback_root),
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


def _move_with_readback(src: Path, dst: Path, label: str) -> None:
    """Move src → dst, then verify both dst.exists() and not src.exists().

    Defensive against NTFS-on-WSL2 and other filesystems where shutil.move /
    os.rename can return success but the post-rename stat takes a moment to
    reflect. Retries the existence check briefly with sync(); if still wrong,
    raises with full path + stat diagnostics so the next agent can root-cause.
    """
    shutil.move(str(src), str(dst))
    # On NTFS-via-WSL2 a directory rename can leave inconsistent stat for a
    # short window. Retry the existence check up to 5 times with a brief
    # sync() between attempts. Most callers see success on the first check.
    for attempt in range(5):
        if dst.exists() and not src.exists():
            return
        try:
            os.sync()
        except Exception:
            pass
        time.sleep(0.05 * (attempt + 1))
    # Fail loud with diagnostics so the next agent has data, per doctrine §7.2.
    diag = {
        "label": label,
        "src": str(src),
        "dst": str(dst),
        "src_exists_post_move": src.exists(),
        "dst_exists_post_move": dst.exists(),
        "src_parent_exists": src.parent.exists(),
        "dst_parent_exists": dst.parent.exists(),
        "src_parent_listing": sorted([p.name for p in src.parent.iterdir()][:20]) if src.parent.exists() else None,
        "dst_parent_listing": sorted([p.name for p in dst.parent.iterdir()][:20]) if dst.parent.exists() else None,
    }
    raise SchedulerError(
        f"python forward shard scheduler move readback failed for {label}: {json.dumps(diag, sort_keys=True)}"
    )


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
        _move_with_readback(destination, archived_existing, f"{label}-archive")
        require(f"{label} previous destination archived", archived_existing.exists())
        require(f"{label} previous destination removed", not destination.exists())
    _move_with_readback(temp, destination, f"{label}-promote")
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
        str(stage_shard.readback_root): str(durable_shard.readback_root),
    }
    rewrite_started = time.monotonic()
    rewrites = (
        rewrite_json_paths_in_tree(stage_shard.cache_root, replacements)
        + rewrite_json_paths_in_tree(stage_shard.readback_root, replacements)
    )
    path_rewrite_ms = elapsed_ms(rewrite_started)
    after_rewrite = {
        "stage_cache_root": tree_state(stage_shard.cache_root),
        "stage_readback_root": tree_state(stage_shard.readback_root),
    }
    archive_parent = args.readback_root / "hot_stage_promoted_overwrites" / durable_shard.shard_id
    cache_promotion_started = time.monotonic()
    cache_promotion = promote_tree_verified(
        stage_shard.cache_root,
        durable_shard.cache_root,
        archive_parent / "cache",
        "cache_root",
    )
    cache_promotion_ms = elapsed_ms(cache_promotion_started)
    readback_promotion_started = time.monotonic()
    readback_promotion = promote_tree_verified(
        stage_shard.readback_root,
        durable_shard.readback_root,
        archive_parent / "readback",
        "readback_root",
    )
    readback_promotion_ms = elapsed_ms(readback_promotion_started)
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
        "readback_promotion": readback_promotion,
        "durable_readback": durable_readback,
        "stage_after_state": tree_state(stage_root),
        "timing_ms": {
            "path_rewrite_ms": path_rewrite_ms,
            "cache_promotion_ms": cache_promotion_ms,
            "readback_promotion_ms": readback_promotion_ms,
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
    resume_baseline: dict[str, Any] | None = None,
) -> dict[str, Any]:
    checkpoint_started = time.monotonic()
    resume_prefix = None
    resume_prefix_count = 0
    if resume_baseline is not None:
        resume_prefix = validate_resume_checkpoint_prefix(args, shards, resume_baseline)
        resume_prefix_count = int(resume_prefix["completed_shards"])
    completed_by_id: dict[str, dict[str, Any]] = {}
    for index, log in enumerate(run_logs):
        shard_id = log.get("shard_id")
        require("checkpoint run log has shard id", isinstance(shard_id, str) and bool(shard_id))
        require(f"checkpoint run log {shard_id} exited zero", log.get("exit_code") == 0)
        require(f"checkpoint run log {shard_id} appears once", shard_id not in completed_by_id)
        expected_index = resume_prefix_count + index
        require(f"checkpoint run log {shard_id} does not exceed plan", expected_index < len(shards))
        require(
            f"checkpoint run log {shard_id} follows resume prefix order",
            shards[expected_index].shard_id == shard_id,
        )
        summary = log.get("summary")
        require(f"checkpoint run log {shard_id} has validated summary", isinstance(summary, dict))
        completed_by_id[shard_id] = log

    completed_count = resume_prefix_count + len(run_logs)
    completed_shards = shards[:completed_count]
    next_shard = shards[completed_count] if completed_count < len(shards) else None
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
    if resume_prefix is not None:
        prefix_summary = resume_prefix["summary"]
        active_embedder_count = int(prefix_summary["active_embedder_count"])
        active_embedder_locked = True
        for key in totals:
            totals[key] += int(prefix_summary[key])
    for log in run_logs:
        summary = log["summary"]
        if not active_embedder_locked:
            active_embedder_count = int(summary["active_embedder_count"])
            active_embedder_locked = True
        require(
            f"{log['shard_id']} checkpoint active embedder count stable",
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
            "checkpoint_readback_path": str(args.readback_root / "python_forward_cache_checkpoint.json"),
            "cache_root": str(args.cache_root),
            "readback_root": str(args.readback_root),
            "run_log_root": str(args.readback_root / "run_logs"),
        },
        "runner_controls": runner_batch_controls(args),
        "host_preflight": preflight,
        "resume_prefix": resume_prefix,
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
                "readback_root": str(next_shard.readback_root),
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
    readback_checkpoint_path = args.readback_root / "python_forward_cache_checkpoint.json"
    checkpoint["timing_ms"]["checkpoint_update_ms"] = elapsed_ms(checkpoint_started)
    json_dump_checked(cache_checkpoint_path, checkpoint)
    json_dump_checked(readback_checkpoint_path, checkpoint)
    cache_readback = read_json(cache_checkpoint_path)
    readback_readback = read_json(readback_checkpoint_path)
    require("checkpoint cache and READBACK summaries match", cache_readback["summary"] == readback_readback["summary"])
    require("checkpoint readback passes", cache_readback["passes"] is True and readback_readback["passes"] is True)
    return checkpoint


def checkpoint_file_paths(args: argparse.Namespace) -> tuple[Path, Path]:
    return (
        args.cache_root / "python_forward_cache_checkpoint.json",
        args.readback_root / "python_forward_cache_checkpoint.json",
    )


def read_existing_checkpoint(args: argparse.Namespace) -> dict[str, Any]:
    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    require("resume checkpoint cache file exists", cache_checkpoint_path.is_file())
    require("resume checkpoint READBACK file exists", readback_checkpoint_path.is_file())
    cache_bytes = cache_checkpoint_path.read_bytes()
    readback_bytes = readback_checkpoint_path.read_bytes()
    require("resume checkpoint cache and READBACK bytes match", cache_bytes == readback_bytes)
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


def validate_resume_checkpoint_prefix(
    args: argparse.Namespace,
    shards: list[ShardSpec],
    checkpoint: dict[str, Any],
) -> dict[str, Any]:
    progress = checkpoint.get("progress", {})
    summary = checkpoint.get("summary", {})
    require("resume checkpoint progress exists", isinstance(progress, dict))
    require("resume checkpoint summary exists", isinstance(summary, dict))
    completed_count = int(progress.get("completed_shards", -1))
    require("resume checkpoint completed shard count is in range", 0 <= completed_count <= len(shards))
    require("resume checkpoint summary shard count matches progress", int(summary.get("shard_count", -1)) == completed_count)
    expected_pairs = int(summary.get("chunk_count", 0)) * int(summary.get("active_embedder_count", 0))
    accounted_pairs = int(summary.get("cache_record_count", 0)) + int(summary.get("quarantine_row_count", 0))
    require("resume checkpoint summary expected pairs match", int(summary.get("expected_pairs", -1)) == expected_pairs)
    require("resume checkpoint summary accounted pairs match", int(summary.get("accounted_pairs", -1)) == accounted_pairs)
    require("resume checkpoint summary accounted equals expected", accounted_pairs == expected_pairs)
    require(
        "resume checkpoint summary second pass covers cache records",
        int(summary.get("second_pass_hits", -1)) == int(summary.get("cache_record_count", -2)),
    )
    next_info = progress.get("next_shard")
    if completed_count < len(shards):
        require("resume checkpoint next shard exists", isinstance(next_info, dict))
        planned_next = shards[completed_count]
        require("resume checkpoint next shard id matches prefix", next_info.get("shard_id") == planned_next.shard_id)
        require(
            "resume checkpoint next shard offset matches prefix",
            int(next_info.get("chunk_offset", -1)) == planned_next.chunk_offset,
        )
        require(
            "resume checkpoint next shard size matches prefix",
            int(next_info.get("max_chunks", -1)) == planned_next.max_chunks,
        )
    else:
        require("resume complete checkpoint has no next shard", next_info is None)

    last_info = progress.get("last_completed_shard")
    last_readback = None
    if completed_count > 0:
        require("resume checkpoint last completed shard exists", isinstance(last_info, dict))
        planned_last = shards[completed_count - 1]
        require("resume checkpoint last shard id matches prefix", last_info.get("shard_id") == planned_last.shard_id)
        last_manifest = file_state(planned_last.manifest_path())
        last_readback = file_state(planned_last.readback_path())
        last_run_log = file_state(shard_run_log_path(args, planned_last))
        require("resume checkpoint last shard manifest exists", last_manifest.get("exists") is True)
        require("resume checkpoint last shard READBACK exists", last_readback.get("exists") is True)
        require("resume checkpoint last shard run log exists", last_run_log.get("exists") is True)
        manifest = read_json(planned_last.manifest_path())
        require("resume checkpoint last shard manifest passes", manifest.get("passes") is True)
        last_readback = {
            "shard_id": planned_last.shard_id,
            "manifest": last_manifest,
            "readback": last_readback,
            "run_log": last_run_log,
            "manifest_summary": manifest.get("summary"),
        }
    else:
        require("resume empty checkpoint has no last completed shard", last_info is None)

    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    return {
        "completed_shards": completed_count,
        "summary": summary,
        "progress": progress,
        "checkpoint_cache": file_state(cache_checkpoint_path),
        "checkpoint_readback": file_state(readback_checkpoint_path),
        "last_completed_shard": last_readback,
        "scan_mode": "checkpoint_prefix_bounded_readback",
        "passes": True,
    }


def write_resume_decision(
    args: argparse.Namespace,
    shard: ShardSpec,
    decision: dict[str, Any],
) -> dict[str, Any]:
    decision_root = args.readback_root / "resume_decisions"
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
    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    before = {
        "checkpoint_cache": file_state(cache_checkpoint_path),
        "checkpoint_readback": file_state(readback_checkpoint_path),
        "cache_root": tree_state(shard.cache_root),
        "readback_root": tree_state(shard.readback_root),
        "run_log": file_state(shard_run_log_path(args, shard)),
    }
    has_existing_state = (
        shard.cache_root.exists()
        or shard.readback_root.exists()
        or shard_run_log_path(args, shard).exists()
    )
    if not has_existing_state:
        return write_resume_decision(
            args,
            shard,
            {
                "decision": "no_partial_state",
                "reason": "checkpoint next shard has no cache, READBACK, or run-log state",
                "before_state": before,
                "after_state": {
                    "checkpoint_cache": file_state(cache_checkpoint_path),
                    "checkpoint_readback": file_state(readback_checkpoint_path),
                    "cache_root": tree_state(shard.cache_root),
                    "readback_root": tree_state(shard.readback_root),
                    "run_log": file_state(shard_run_log_path(args, shard)),
                },
                "checkpoint_sha256_unchanged": before["checkpoint_cache"].get("sha256")
                == file_state(cache_checkpoint_path).get("sha256"),
                "passes": True,
            },
        )
    try:
        completed = completed_shard_bounded_readback(args, shard)
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
                        "checkpoint_readback": file_state(readback_checkpoint_path),
                        "cache_root": tree_state(shard.cache_root),
                        "readback_root": tree_state(shard.readback_root),
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
                    "checkpoint_readback": file_state(readback_checkpoint_path),
                    "cache_root": tree_state(shard.cache_root),
                    "readback_root": tree_state(shard.readback_root),
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
            "reason": "completed shard bounded readback passed against durable aiwonder SoT",
            "before_state": before,
            "completed_readback": completed,
            "after_state": {
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_readback": file_state(readback_checkpoint_path),
                "cache_root": tree_state(shard.cache_root),
                "readback_root": tree_state(shard.readback_root),
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
                "readback_root": str(shard.readback_root),
            }
        },
    }


def write_checkpoint_pair(args: argparse.Namespace, checkpoint: dict[str, Any]) -> dict[str, Any]:
    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    json_dump_checked(cache_checkpoint_path, checkpoint)
    json_dump_checked(readback_checkpoint_path, checkpoint)
    cache_readback = read_json(cache_checkpoint_path)
    readback_readback = read_json(readback_checkpoint_path)
    require("checkpoint pair cache readback passes", cache_readback.get("passes") is True)
    require("checkpoint pair READBACK readback passes", readback_readback.get("passes") is True)
    require("checkpoint pair bytes match", cache_checkpoint_path.read_bytes() == readback_checkpoint_path.read_bytes())
    return {
        "cache_checkpoint": file_state(cache_checkpoint_path),
        "readback_checkpoint": file_state(readback_checkpoint_path),
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
    edge_args.readback_root = case_root / "readback"
    edge_args.hot_root = case_root / "hot"
    edge_args.retry_failed_partial_shards = retry_failed_partial_shards
    edge_args.require_packed_store = True
    if active_embedder_count is not None:
        edge_args.active_embedder_count = active_embedder_count
    edge_args.cache_root.mkdir(parents=True)
    edge_args.readback_root.mkdir(parents=True)
    edge_args.hot_root.mkdir(parents=True)
    os.chmod(edge_args.hot_root, 0o700)
    return edge_args


def resume_preflight_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    edge_root = args.readback_root / "edge_inputs" / "resume_preflight"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, Any]] = []

    def append_case(case_name: str, edge_args: argparse.Namespace, shard: ShardSpec, decision: dict[str, Any]) -> None:
        cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(edge_args)
        cases.append(
            {
                "case": case_name,
                "passes": True,
                "decision": decision["decision"],
                "decision_path": decision["decision_path"],
                "decision_sha256": decision["decision_sha256"],
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_readback": file_state(readback_checkpoint_path),
                "source_after_state": {
                    "cache_root": tree_state(shard.cache_root),
                    "readback_root": tree_state(shard.readback_root),
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
        no_state_args.readback_root / "shards" / "edge-no-partial-state",
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
        partial_args.readback_root / "shards" / "edge-partial-without-manifest",
    )
    write_checkpoint_pair(partial_args, checkpoint_for_shard(partial_shard))
    partial_shard.cache_root.mkdir(parents=True)
    partial_shard.readback_root.mkdir(parents=True)
    (partial_shard.cache_root / "python_forward_cache_index.jsonl").write_text(
        json.dumps({"cache_key_sha256": f"sha256:{'6' * 64}"}) + "\n",
        encoding="utf-8",
    )
    json_dump_checked(partial_shard.readback_root / "partial_marker.json", {"passes": False})
    json_dump_checked(
        shard_run_log_path(partial_args, partial_shard),
        {"shard_id": partial_shard.shard_id, "exit_code": 1, "stderr_tail": "synthetic partial"},
    )
    partial_decision = scan_checkpoint_next_shard_state(partial_args, [partial_shard], read_existing_checkpoint(partial_args))
    require("resume partial edge archives for retry", partial_decision["decision"] == "archived_for_retry")
    require("resume partial edge keeps checkpoint unchanged", partial_decision["checkpoint_sha256_unchanged"] is True)
    require("resume partial edge removes source cache root", not partial_shard.cache_root.exists())
    require("resume partial edge removes source READBACK root", not partial_shard.readback_root.exists())
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
    require("resume mismatch edge removes source READBACK root", not mismatch_shard.readback_root.exists())
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
    exact_manifest_summary = read_json(exact_shard.manifest_path())["summary"]
    json_dump_checked(
        shard_run_log_path(exact_args, exact_shard),
        {
            "shard_id": exact_shard.shard_id,
            "exit_code": 0,
            "elapsed_seconds": 1.0,
            "skipped_completed": False,
            "summary": exact_manifest_summary,
        },
    )
    write_checkpoint_pair(exact_args, checkpoint_for_shard(exact_shard))
    exact_decision = scan_checkpoint_next_shard_state(exact_args, [exact_shard], read_existing_checkpoint(exact_args))
    require("resume exact edge accepts completed proof", exact_decision["decision"] == "exact_resume_proof_accepted")
    require("resume exact edge keeps checkpoint unchanged", exact_decision["checkpoint_sha256_unchanged"] is True)
    require("resume exact edge preserves source cache root", exact_shard.cache_root.exists())
    require("resume exact edge preserves source READBACK root", exact_shard.readback_root.exists())
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
        blocked_args.readback_root / "shards" / "edge-blocked-without-retry",
    )
    write_checkpoint_pair(blocked_args, checkpoint_for_shard(blocked_shard))
    blocked_shard.cache_root.mkdir(parents=True)
    blocked_shard.readback_root.mkdir(parents=True)
    (blocked_shard.cache_root / "python_forward_cache_index.jsonl").write_text("", encoding="utf-8")
    try:
        scan_checkpoint_next_shard_state(blocked_args, [blocked_shard], read_existing_checkpoint(blocked_args))
    except SchedulerError as exc:
        decision_files = sorted((blocked_args.readback_root / "resume_decisions").glob("*.json"))
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
                    "readback_root": tree_state(blocked_shard.readback_root),
                    "run_log": file_state(shard_run_log_path(blocked_args, blocked_shard)),
                },
            }
        )
    else:
        raise SchedulerError("resume blocked edge unexpectedly passed")

    return cases


def write_resume_preflight_readback(
    args: argparse.Namespace,
    started_at: str,
    before: dict[str, Any],
    checkpoint: dict[str, Any],
    decision: dict[str, Any],
    edge_cases: list[dict[str, Any]],
) -> dict[str, Any]:
    require("resume preflight decision passes", decision.get("passes") is True)
    require("resume preflight edge cases pass", all(case.get("passes") is True for case in edge_cases))
    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_resume_preflight_readback",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "checkpoint_cache": str(cache_checkpoint_path),
            "checkpoint_readback": str(readback_checkpoint_path),
            "decision_path": decision["decision_path"],
            "cache_root": str(args.cache_root),
            "readback_root": str(args.readback_root),
            "hot_root": str(args.hot_root),
        },
        "before_state": before,
        "checkpoint_progress": checkpoint.get("progress"),
        "checkpoint_summary": checkpoint.get("summary"),
        "decision": decision,
        "edge_cases": edge_cases,
        "after_state": {
            **scheduler_root_state(args),
            "checkpoint_cache": file_state(cache_checkpoint_path),
            "checkpoint_readback": file_state(readback_checkpoint_path),
            "disk": disk_snapshot(),
            "resource": resource_snapshot(args, "resume_preflight_after"),
        },
        "passes": True,
    }
    readback_path = args.readback_root / "python_forward_resume_preflight_readback.json"
    json_dump_checked(readback_path, readback)
    readback = read_json(readback_path)
    require("resume preflight READBACK readback passes", readback.get("passes") is True)
    return {
        **readback,
        "readback_path": str(readback_path),
        "readback_sha256": sha256_file(readback_path),
    }


def hot_stage_promotion_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    edge_root = args.readback_root / "edge_inputs" / "hot_stage_promotion"
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
        edge_root / "durable-readback",
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
                "stage_readback_root": tree_state(stage_edge_shard.readback_root),
            },
        }
    )
    shutil.rmtree(stage_root)

    parallel_stage_durable = write_minimal_packed_shard(
        edge_root / "parallel-empty-hot-stage",
        "edge-parallel-empty-hot-stage",
        600_000_006,
        1,
        args.chunks_jsonl,
    )
    parallel_stage_name = f"{parallel_stage_durable.shard_id}.e4.json"
    parallel_stage_root = hot_stage_root(args, parallel_stage_durable, parallel_stage_name)
    if parallel_stage_root.exists():
        shutil.rmtree(parallel_stage_root)
    parallel_stage_shard = hot_stage_shard(args, parallel_stage_durable, parallel_stage_name)
    parallel_stage_setup = prepare_hot_stage(
        args,
        parallel_stage_durable,
        parallel_stage_shard,
        parallel_stage_root,
        seed_from_durable=False,
    )
    require(
        "parallel isolated hot-stage edge keeps durable packed records out of worker stage",
        not (parallel_stage_shard.cache_root / "packed_records").exists(),
    )
    require(
        "parallel isolated hot-stage edge leaves durable packed records intact",
        (parallel_stage_durable.cache_root / "packed_records" / "e1.jsonl").is_file(),
    )
    require(
        "parallel isolated hot-stage edge records disabled durable seed",
        parallel_stage_setup.get("seed_from_durable") is False
        and parallel_stage_setup.get("durable_seed") == {"cache_root": False, "readback_root": False},
    )
    cases.append(
        {
            "case": "parallel_isolated_hot_stage_starts_empty_without_durable_copy",
            "passes": True,
            "durable_cache_root": tree_state(parallel_stage_durable.cache_root),
            "stage_setup": parallel_stage_setup,
            "stage_after_prepare": {
                "cache_root": tree_state(parallel_stage_shard.cache_root),
                "readback_root": tree_state(parallel_stage_shard.readback_root),
            },
        }
    )
    shutil.rmtree(parallel_stage_root)

    preflight_args = argparse.Namespace(**vars(args))
    preflight_args.mode = "preflight-resume"
    preflight_args.retry_failed_partial_shards = True
    preflight_args.hot_root = args.hot_root / "edge-hot-stage-preflight-root"
    preflight_args.readback_root = edge_root / "preflight-archive-readback"
    preflight_shard = ShardSpec(
        "edge-hot-stage-preflight-archive",
        600_000_005,
        1,
        edge_root / "preflight-durable-cache",
        edge_root / "preflight-durable-readback",
    )
    preflight_stage_root = preflight_args.hot_root / "packed-shard-stage" / preflight_shard.shard_id
    if preflight_args.hot_root.exists():
        shutil.rmtree(preflight_args.hot_root)
    if preflight_args.readback_root.exists():
        shutil.rmtree(preflight_args.readback_root)
    preflight_stage_root.mkdir(parents=True)
    (preflight_stage_root / "stale-marker.txt").write_text("interrupted", encoding="utf-8")
    preflight_archives = archive_hot_stage_blockers_for_preflight(preflight_args, [preflight_shard])
    require("preflight hot-stage archive edge archived one blocker", len(preflight_archives) == 1)
    require("preflight hot-stage archive edge removed source", not preflight_stage_root.exists())
    require("preflight hot-stage archive edge leaves no blockers", not hot_stage_blockers(preflight_args))
    cases.append(
        {
            "case": "preflight_resume_archives_hot_stage_before_smoke_gate",
            "passes": True,
            "archive": preflight_archives[0],
        }
    )

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
            "readback_promotion_ms",
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


def write_resume_gate_readback(
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
    readback_path = args.readback_root / "python_forward_resume_gate_readback.json"
    value = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_resume_gate_readback",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "resume_gate_readback": str(readback_path),
            "events": str(event_path),
            "checkpoint_cache": str(args.cache_root / "python_forward_cache_checkpoint.json"),
            "checkpoint_readback": str(args.readback_root / "python_forward_cache_checkpoint.json"),
            "aggregate_manifest": str(args.cache_root / "python_forward_cache_corpus_manifest.json"),
            "cache_root": str(args.cache_root),
            "readback_root": str(args.readback_root),
            "run_log_root": str(args.readback_root / "run_logs"),
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
    json_dump_checked(readback_path, value)
    readback = read_json(readback_path)
    require("resume gate READBACK readback passes", readback.get("passes") is True)
    require("resume gate edge cases pass", all(case.get("passes") is True for case in edge_cases))
    value["resume_gate_sha256"] = sha256_file(readback_path)
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
    validate_shard_readback(shard)
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
    readback_root = root / f"{shard_id}-readback"
    if cache_root.exists():
        shutil.rmtree(cache_root)
    if readback_root.exists():
        shutil.rmtree(readback_root)
    packed_root = cache_root / "packed_records"
    packed_root.mkdir(parents=True, exist_ok=True)
    readback_root.mkdir(parents=True, exist_ok=True)
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
    json_dump_checked(readback_root / "python_forward_cache_readback.json", {"passes": True})
    return ShardSpec(shard_id, chunk_offset, max_chunks, cache_root, readback_root)


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
                "readback_path": str(shard.readback_path()),
                "readback_sha256": sha256_file(shard.readback_path()),
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
        hot_stage_budget_bytes=preflight["host"]["filesystems"]["/"]["size_bytes"] + 1,
    )
    durable_disk_for_edge = os.environ.get("CG_MEJEPA_DURABLE_DISK_TARGET", "/zfs/archive")
    expect_preflight_failure(
        "durable_root_projected_capacity_overflow_rejected_before_writes",
        projected_output_bytes=preflight["host"]["filesystems"][durable_disk_for_edge]["size_bytes"] + 1,
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
        cargo_runner="cargo run -p context-graph-mejepa-embedders --example python_forward_cache_readback",
    )
    expect_preflight_failure(
        "missing_release_runner_rejected_before_writes",
        cargo_runner="/definitely/missing/python_forward_cache_readback",
    )
    return cases


def task061_smoke_manifest_gate_edge_checks(args: argparse.Namespace) -> list[dict[str, Any]]:
    if not task061_smoke_manifest_gate_required(args):
        return []
    edge_root = args.readback_root / "edge_inputs" / "task061_smoke_manifest_gate"
    edge_root.mkdir(parents=True, exist_ok=True)
    real_payload = read_json(args.smoke_manifest_readback)
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
    synthetic_readback = edge_root / "synthetic_stale_smoke_manifest_readback.json"
    json_dump_checked(synthetic_readback, synthetic_payload)

    edge_args = argparse.Namespace(**vars(args))
    edge_args.chunks_jsonl = stale_chunks
    edge_args.smoke_manifest_readback = synthetic_readback
    try:
        task061_smoke_manifest_gate(edge_args)
    except SchedulerError as exc:
        error = str(exc)
        require("task061 stale hash edge failed on smoke chunks hash", "TASK061_SMOKE_CHUNKS_HASH_MATCH" in error)
        return [
            {
                "case": "stale_task061_smoke_chunks_hash_rejected",
                "passes": True,
                "synthetic_readback": str(synthetic_readback),
                "synthetic_readback_sha256": sha256_file(synthetic_readback),
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
    edge_root = args.readback_root / "edge_inputs"
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
        "static AST isolated process list retires temporal embedders",
        not set(STATIC_AST_RETIRED_TEMPORAL_FORWARD_SLUGS).intersection(ISOLATED_FORWARD_SLUGS),
    )
    require(
        "isolated process list includes CUDA HDC embedder",
        "e9" in ISOLATED_FORWARD_SLUGS,
    )
    require(
        "isolated process list covers all active scheduler embedders",
        len(ISOLATED_FORWARD_SLUGS) == args.active_embedder_count,
    )
    cases.append({
        "case": "isolated_process_list_covers_all_required_embedders",
        "passes": True,
        "isolated_forward_slugs": list(ISOLATED_FORWARD_SLUGS),
        "retired_temporal_forward_slugs": list(STATIC_AST_RETIRED_TEMPORAL_FORWARD_SLUGS),
    })

    bounded_args = argparse.Namespace(**vars(args))
    bounded_args.cache_root = edge_root / "bounded-root-cache"
    bounded_args.readback_root = edge_root / "bounded-root-readback"
    bounded_args.hot_root = edge_root / "bounded-root-hot"
    for root in (bounded_args.cache_root, bounded_args.readback_root, bounded_args.hot_root):
        root.mkdir(parents=True, exist_ok=True)
    json_dump_checked(
        bounded_args.cache_root / "python_forward_cache_checkpoint.json",
        {"passes": True, "root": "cache"},
    )
    json_dump_checked(
        bounded_args.readback_root / "python_forward_cache_checkpoint.json",
        {"passes": True, "root": "readback"},
    )
    json_dump_checked(
        bounded_args.cache_root / "python_forward_cache_shard_plan.json",
        {"passes": True, "artifact": "plan"},
    )
    unrelated = bounded_args.readback_root / "unrelated-large-subtree"
    unrelated.mkdir(parents=True, exist_ok=True)
    sentinel = unrelated / "sentinel.txt"
    sentinel.write_text("must not be recursively hashed by scheduler root readback\n", encoding="utf-8")
    sentinel_sha = sha256_file(sentinel)
    root_state = scheduler_root_state(bounded_args)
    root_state_json = json.dumps(root_state, sort_keys=True)
    require(
        "bounded scheduler root state records checkpoint artifacts",
        root_state["cache_root"]["artifacts"]["checkpoint"].get("exists") is True
        and root_state["readback_root"]["artifacts"]["checkpoint"].get("exists") is True,
    )
    require(
        "bounded scheduler root state does not hash unrelated READBACK subtree",
        sentinel_sha not in root_state_json,
    )
    require(
        "bounded scheduler root state declares bounded scan mode",
        root_state["readback_root"]["scan_mode"] == "bounded_artifact_readback",
    )
    cases.append(
        {
            "case": "bounded_scheduler_root_state_ignores_unrelated_subtree",
            "passes": True,
            "cache_checkpoint": root_state["cache_root"]["artifacts"]["checkpoint"],
            "readback_checkpoint": root_state["readback_root"]["artifacts"]["checkpoint"],
            "unrelated_subtree": directory_state(unrelated),
            "sentinel_sha256_not_present": sentinel_sha,
            "scan_mode": root_state["readback_root"]["scan_mode"],
        }
    )

    prefix_args = argparse.Namespace(**vars(args))
    prefix_args.cache_root = edge_root / "resume-prefix-cache"
    prefix_args.readback_root = edge_root / "resume-prefix-readback"
    prefix_args.hot_root = edge_root / "resume-prefix-hot"
    prefix_shards = [
        ShardSpec(
            shard_id="edge-prefix-000",
            chunk_offset=0,
            max_chunks=1,
            cache_root=prefix_args.cache_root / "shards" / "edge-prefix-000",
            readback_root=prefix_args.readback_root / "shards" / "edge-prefix-000",
        ),
        ShardSpec(
            shard_id="edge-prefix-001",
            chunk_offset=1,
            max_chunks=1,
            cache_root=prefix_args.cache_root / "shards" / "edge-prefix-001",
            readback_root=prefix_args.readback_root / "shards" / "edge-prefix-001",
        ),
    ]
    prefix_summary = {
        "chunk_count": 1,
        "cache_record_count": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "first_pass_hits": 0,
        "first_pass_writes": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "second_pass_hits": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "quarantine_row_count": 0,
        "batch_telemetry_row_count": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "active_embedder_count": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "expected_pairs": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "accounted_pairs": DEFAULT_ACTIVE_EMBEDDER_COUNT,
        "shard_count": 1,
    }
    json_dump_checked(
        prefix_shards[0].manifest_path(),
        {"passes": True, "summary": prefix_summary},
    )
    json_dump_checked(prefix_shards[0].readback_path(), {"passes": True})
    json_dump_checked(
        shard_run_log_path(prefix_args, prefix_shards[0]),
        {"shard_id": prefix_shards[0].shard_id, "exit_code": 0, "summary": prefix_summary},
    )
    prefix_checkpoint = {
        "passes": True,
        "artifact_kind": "python_forward_shard_scheduler_checkpoint",
        "progress": {
            "completed_shards": 1,
            "total_shards": 2,
            "completed_chunk_capacity": 1,
            "planned_chunk_capacity": 2,
            "last_completed_shard": {
                "shard_id": prefix_shards[0].shard_id,
                "chunk_offset": 0,
                "max_chunks": 1,
            },
            "next_shard": {
                "shard_id": prefix_shards[1].shard_id,
                "chunk_offset": 1,
                "max_chunks": 1,
            },
        },
        "summary": prefix_summary,
    }
    json_dump_checked(prefix_args.cache_root / "python_forward_cache_checkpoint.json", prefix_checkpoint)
    json_dump_checked(prefix_args.readback_root / "python_forward_cache_checkpoint.json", prefix_checkpoint)
    prefix_readback = validate_resume_checkpoint_prefix(prefix_args, prefix_shards, prefix_checkpoint)
    require(
        "resume checkpoint prefix uses bounded readback",
        prefix_readback["scan_mode"] == "checkpoint_prefix_bounded_readback",
    )
    cases.append(
        {
            "case": "resume_checkpoint_prefix_bounded_readback",
            "passes": True,
            "completed_shards": prefix_readback["completed_shards"],
            "checkpoint_cache": prefix_readback["checkpoint_cache"],
            "last_completed_shard": prefix_readback["last_completed_shard"],
            "scan_mode": prefix_readback["scan_mode"],
        }
    )

    bounded_completed_args = argparse.Namespace(**vars(args))
    bounded_completed_args.active_embedder_count = 1
    bounded_completed_args.cache_root = edge_root / "bounded-completed-cache"
    bounded_completed_args.readback_root = edge_root / "bounded-completed-readback"
    bounded_completed = write_minimal_packed_shard(
        bounded_completed_args.cache_root / "shards",
        "edge-bounded-completed",
        10_000,
        1,
        args.chunks_jsonl,
    )
    bounded_summary = read_json(bounded_completed.manifest_path())["summary"]
    bounded_existing_log = {
        "shard_id": bounded_completed.shard_id,
        "exit_code": 0,
        "elapsed_seconds": 1.0,
        "skipped_completed": False,
        "summary": bounded_summary,
    }
    json_dump_checked(shard_run_log_path(bounded_completed_args, bounded_completed), bounded_existing_log)
    existing_run_log_sha = sha256_file(shard_run_log_path(bounded_completed_args, bounded_completed))
    bounded_completed_readback = completed_shard_bounded_readback(bounded_completed_args, bounded_completed)
    require(
        "bounded completed readback does not rewrite canonical run log",
        sha256_file(shard_run_log_path(bounded_completed_args, bounded_completed)) == existing_run_log_sha,
    )
    require(
        "bounded completed readback skips packed payload scan",
        bounded_completed_readback.get("packed_store_readback") is None
        and bounded_completed_readback.get("resume_readback_mode") == "bounded_completed_shard_manifest_readback",
    )
    cases.append(
        {
            "case": "completed_shard_bounded_readback_preserves_existing_run_log",
            "passes": True,
            "scan_mode": bounded_completed_readback["resume_readback_mode"],
            "existing_run_log": bounded_completed_readback["existing_run_log"],
            "bounded_artifacts": bounded_completed_readback["bounded_artifacts"],
        }
    )

    missing_log_shard = write_minimal_packed_shard(
        bounded_completed_args.cache_root / "shards",
        "edge-bounded-completed-missing-log",
        10_010,
        1,
        args.chunks_jsonl,
    )
    try:
        completed_shard_bounded_readback(bounded_completed_args, missing_log_shard)
    except SchedulerError as exc:
        cases.append({"case": "completed_shard_bounded_readback_rejects_missing_run_log", "passes": True, "error": str(exc)})
    else:
        raise SchedulerError("bounded completed readback missing run log edge unexpectedly passed")

    missing = ShardSpec(
        shard_id="edge-missing-manifest",
        chunk_offset=999_000_000,
        max_chunks=1,
        cache_root=edge_root / "missing",
        readback_root=edge_root / "missing-readback",
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
        readback_root=shards[0].readback_root,
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
            readback_root=shards[0].readback_root,
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
            readback_root=shards[0].readback_root,
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

        log_filter_root = edge_root / "run-log-filter-readback"
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
        readback_root=edge_root / "partial-readback",
    )
    if partial.cache_root.exists():
        shutil.rmtree(partial.cache_root)
    if partial.readback_root.exists():
        shutil.rmtree(partial.readback_root)
    partial.cache_root.mkdir(parents=True)
    partial.readback_root.mkdir(parents=True)
    (partial.cache_root / "python_forward_cache_index.jsonl").write_text(
        json.dumps({"cache_key_sha256": f"sha256:{'3' * 64}"}) + "\n",
        encoding="utf-8",
    )
    json_dump_checked(partial.readback_root / "partial_marker.json", {"passes": False})
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
        "partial archive edge removed source READBACK root",
        not partial.readback_root.exists(),
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
    production_root_guard = validate_production_roots(args)
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
    pre_smoke_hot_stage_archives = archive_hot_stage_blockers_for_preflight(args, shards) if args.mode == "preflight-resume" else []
    smoke_gate = smoke_gate_preflight(args)
    preflight = host_preflight(args)
    preflight["production_root_guard"] = production_root_guard
    preflight["task061_smoke_manifest_gate"] = task061_gate
    preflight["smoke_gate"] = smoke_gate
    preflight["pre_smoke_hot_stage_archives"] = pre_smoke_hot_stage_archives
    event_path = args.readback_root / "scheduler_events.jsonl"
    if args.mode == "run" and event_path.exists():
        event_path.unlink()
    args.cache_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    os.chmod(args.hot_root, 0o700)

    before = {
        **scheduler_root_state(args),
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
        "readback_root": str(args.readback_root),
        "hot_root": str(args.hot_root),
        "runner_batch_controls": runner_batch_controls(args),
        "host_preflight": preflight,
        "shards": [
            {
                "shard_id": shard.shard_id,
                "chunk_offset": shard.chunk_offset,
                "max_chunks": shard.max_chunks,
                "cache_root": str(shard.cache_root),
                "readback_root": str(shard.readback_root),
            }
            for shard in shards
        ],
    }
    plan_path = args.cache_root / "python_forward_cache_shard_plan.json"
    json_dump_checked(plan_path, plan)
    event(event_path, "plan_written", {"plan_path": str(plan_path), "plan_sha256": sha256_file(plan_path)})

    if args.mode == "preflight-resume":
        cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
        if cache_checkpoint_path.exists() or readback_checkpoint_path.exists():
            checkpoint = read_existing_checkpoint(args)
        else:
            checkpoint = write_checkpoint(args, shards, [], started_at, preflight, "fresh_preflight")
            event(
                event_path,
                "fresh_preflight_checkpoint_written",
                {
                    "checkpoint_cache": file_state(cache_checkpoint_path),
                    "checkpoint_readback": file_state(readback_checkpoint_path),
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
        readback = write_resume_preflight_readback(args, started_at, before, checkpoint, decision, edge_cases)
        print(json.dumps(readback, indent=2, sort_keys=True))
        return 0

    run_logs = []
    resume_baseline = None
    resume_start_index = 0
    cache_checkpoint_path, readback_checkpoint_path = checkpoint_file_paths(args)
    if (
        args.mode == "run"
        and args.resume_completed_shards
        and cache_checkpoint_path.exists()
        and readback_checkpoint_path.exists()
    ):
        checkpoint = read_existing_checkpoint(args)
        resume_baseline = checkpoint
        resume_prefix = validate_resume_checkpoint_prefix(args, shards, checkpoint)
        resume_start_index = int(resume_prefix["completed_shards"])
        event(
            event_path,
            "resume_checkpoint_observed",
            {
                "checkpoint_cache": file_state(cache_checkpoint_path),
                "checkpoint_readback": file_state(readback_checkpoint_path),
                "progress": checkpoint.get("progress"),
                "summary": checkpoint.get("summary"),
                "resume_prefix": resume_prefix,
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
        for shard in shards[resume_start_index:]:
            if args.resume_completed_shards and (
                shard.cache_root.exists()
                or shard.readback_root.exists()
                or shard_run_log_path(args, shard).exists()
            ):
                if shard.manifest_path().is_file():
                    try:
                        run_log = completed_shard_bounded_readback(args, shard)
                    except SchedulerError as exc:
                        if not args.retry_failed_partial_shards:
                            raise
                        archive = archive_failed_partial_shard(args, shard, str(exc))
                        event(event_path, "failed_partial_shard_archived_for_retry", archive)
                    else:
                        skip_log_path = resume_skip_log_path(args, shard)
                        run_log["resume_skip_log_path"] = str(skip_log_path)
                        json_dump_checked(skip_log_path, run_log)
                        run_log["resume_skip_log"] = file_state(skip_log_path)
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
                                resume_baseline=resume_baseline,
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
                    resume_baseline=resume_baseline,
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
        run_logs = load_existing_run_logs(args.readback_root)
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
    checkpoint = write_checkpoint(
        args,
        shards,
        run_logs,
        started_at,
        preflight,
        "final",
        resume_baseline=resume_baseline,
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
    edge_cases = edge_checks(args, shards, preflight)
    resume_gate = None
    if args.write_resume_gate:
        resume_gate = write_resume_gate_readback(
            args,
            started_at,
            event_path,
            run_logs,
            checkpoint,
            corpus_manifest,
            preflight,
        )
    after = {
        **scheduler_root_state(args),
        "disk": disk_snapshot(),
        "resource": resource_snapshot(args, "scheduler_after"),
    }
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_forward_shard_scheduler_readback",
        "started_at_utc": started_at,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "plan": str(plan_path),
            "events": str(event_path),
            "aggregate_manifest": str(aggregate_path),
            "checkpoint_cache": str(args.cache_root / "python_forward_cache_checkpoint.json"),
            "checkpoint_readback": str(args.readback_root / "python_forward_cache_checkpoint.json"),
            "cache_root": str(args.cache_root),
            "readback_root": str(args.readback_root),
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
    readback_path = args.readback_root / "python_forward_shard_scheduler_readback.json"
    json_dump_checked(readback_path, readback)
    readback = read_json(readback_path)
    require("scheduler READBACK readback passes", readback["passes"] is True)
    print(json.dumps(readback, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # noqa: BLE001 - CLI must fail loudly with context.
        print(f"PY_FORWARD_SHARD_SCHEDULER_FAILED: {exc}", file=sys.stderr)
        raise
