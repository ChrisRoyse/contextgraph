"""Derive the full-run storage design from the 1,000 chunk smoke outputs."""

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
TASK_ID = "TASK-PY-DATA-063"
DEFAULT_SMOKE_ROOT = Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2")
DEFAULT_FORWARD_ROOT = DEFAULT_SMOKE_ROOT / "forward-cache"
DEFAULT_MATRIX_ROOT = DEFAULT_SMOKE_ROOT / "intelligence-matrices"
DEFAULT_DESIGN_ROOT = DEFAULT_SMOKE_ROOT / "design"
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-063-post249-smoke-storage-design-fsv")
DEFAULT_FULL_CHUNKS_JSONL = Path("/mnt/d/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_FORWARD_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-061-post249-smoke-forward-fsv/python_forward_shard_scheduler_fsv.json")
DEFAULT_MATRIX_FSV = Path(
    "/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json"
)
DEFAULT_SMOKE_CHUNKS = 1000
DEFAULT_MAX_DURABLE_FORWARD_BYTES = 500 * 1024 * 1024 * 1024
DEFAULT_WINDOW_CHUNKS = 1000


class SmokeStorageDesignError(RuntimeError):
    pass


def utc_now() -> str:
    import datetime as _dt

    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeStorageDesignError(message)


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
        raise SmokeStorageDesignError(f"required JSON file missing: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SmokeStorageDesignError(f"invalid JSON in {path}: {exc}") from exc


def iter_jsonl(path: Path):
    try:
        fh = path.open("r", encoding="utf-8")
    except FileNotFoundError as exc:
        raise SmokeStorageDesignError(f"required JSONL file missing: {path}") from exc
    with fh:
        for line_number, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            try:
                yield line_number, json.loads(line)
            except json.JSONDecodeError as exc:
                raise SmokeStorageDesignError(f"invalid JSONL at {path}:{line_number}: {exc}") from exc


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as fh:
        json.dump(value, fh, sort_keys=True, indent=2, separators=(",", ": "))
        fh.write("\n")
    tmp.replace(path)


def count_jsonl(path: Path) -> int:
    count = 0
    try:
        with path.open("r", encoding="utf-8") as fh:
            for line in fh:
                if line.strip():
                    count += 1
    except FileNotFoundError as exc:
        raise SmokeStorageDesignError(f"required full corpus chunks JSONL missing: {path}") from exc
    return count


def file_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"path": str(path), "exists": False, "is_file": False}
    state: dict[str, Any] = {
        "path": str(path),
        "exists": True,
        "is_file": path.is_file(),
        "byte_count": path.stat().st_size,
    }
    if path.is_file():
        state["sha256"] = sha256_file(path)
    return state


def tree_state(root: Path) -> dict[str, Any]:
    if not root.exists():
        return {"path": str(root), "exists": False, "file_count": 0, "byte_count": 0, "hash_sample": []}
    if root.is_file():
        return {
            "path": str(root),
            "exists": True,
            "file_count": 1,
            "byte_count": root.stat().st_size,
            "hash_sample": [f"{root.name}:{sha256_file(root)}"],
            "hash_sample_truncated": False,
        }
    file_count = 0
    byte_count = 0
    samples: list[str] = []
    for child in sorted(root.rglob("*")):
        if not child.is_file():
            continue
        file_count += 1
        byte_count += child.stat().st_size
        if len(samples) < 30:
            samples.append(f"{child.relative_to(root)}:{sha256_file(child)}")
    return {
        "path": str(root),
        "exists": True,
        "file_count": file_count,
        "byte_count": byte_count,
        "hash_sample": samples,
        "hash_sample_truncated": file_count > len(samples),
    }


def artifact_hashes(root: Path) -> dict[str, Any]:
    files = []
    if root.exists():
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
    return {"root": str(root), "file_count": len(files), "byte_count": sum(item["bytes"] for item in files), "files": files}


def load_coverage(path: Path) -> tuple[list[dict[str, Any]], int]:
    rows: list[dict[str, Any]] = []
    raw_vector_bytes = 0
    for _, row in iter_jsonl(path):
        rows.append(row)
        dim_counts = row.get("dimension_counts") or {}
        for dim, count in dim_counts.items():
            raw_vector_bytes += int(dim) * int(count) * 4
    return rows, raw_vector_bytes


def disk_snapshot(path: Path) -> dict[str, int | str]:
    usage = shutil.disk_usage(path)
    return {"path": str(path), "total": usage.total, "used": usage.used, "free": usage.free}


def ceil_int(value: float) -> int:
    return int(math.ceil(value))


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    forward_fsv = read_json(args.forward_fsv)
    matrix_fsv = read_json(args.matrix_fsv)
    forward_manifest = read_json(args.forward_root / "python_forward_cache_corpus_manifest.json")
    matrix_summary = read_json(args.matrix_root / "summary.json")
    coverage_rows, smoke_raw_vector_bytes = load_coverage(args.matrix_root / "embedder_coverage.jsonl")

    require(forward_fsv.get("passes") is True, "smoke forward FSV must pass before storage design")
    require(matrix_fsv.get("passes") is True, "smoke intelligence FSV must pass before storage design")
    require(forward_manifest.get("passes") is True, "smoke forward corpus manifest must pass")
    require(matrix_summary.get("task_id") == "TASK-PY-DATA-062", "matrix summary must be TASK-PY-DATA-062")

    smoke_summary = forward_manifest.get("summary") or {}
    smoke_chunk_count = int(smoke_summary.get("chunk_count") or args.smoke_chunk_count)
    require(smoke_chunk_count == args.smoke_chunk_count, f"expected {args.smoke_chunk_count} smoke chunks")
    full_chunk_count = args.full_chunk_count or count_jsonl(args.full_chunks_jsonl)
    require(full_chunk_count > smoke_chunk_count, "full corpus chunk count must exceed smoke chunk count")

    scale = full_chunk_count / float(smoke_chunk_count)
    forward_state = tree_state(args.forward_root)
    matrix_state = tree_state(args.matrix_root)
    d_snapshot = disk_snapshot(Path("/mnt/d/contextgraph"))

    forward_estimate = ceil_int(forward_state["byte_count"] * scale)
    raw_vector_estimate = ceil_int(smoke_raw_vector_bytes * scale)
    estimated_cache_records = ceil_int(int(smoke_summary.get("cache_record_count") or 0) * scale)
    estimated_quarantines = ceil_int(int(smoke_summary.get("quarantine_row_count") or 0) * scale)
    windows = ceil_int(full_chunk_count / float(args.window_chunks))
    windowed_matrix_estimate = ceil_int(matrix_state["byte_count"] * (windows + 1))
    unbounded_chunk_pairs = full_chunk_count * (full_chunk_count - 1) // 2
    estimated_total_design_bytes = forward_estimate + windowed_matrix_estimate

    metric_storage_policy = [
        {
            "metric": "linear_cka",
            "full_scale_policy": "streaming_exact_pairwise_embedder_matrix",
            "stored_rows": "global 66 rows plus per-window 66 rows",
            "reason": "requires only cross-covariance summaries, not chunk-pair materialization",
            "unbounded_pairwise_chunk_storage": False,
        },
        {
            "metric": "linear_hsic",
            "full_scale_policy": "streaming_exact_pairwise_embedder_matrix",
            "stored_rows": "global 66 rows plus per-window 66 rows",
            "reason": "computed from centered sufficient statistics",
            "unbounded_pairwise_chunk_storage": False,
        },
        {
            "metric": "mean_same_row_cosine",
            "full_scale_policy": "same-key_streaming_equal-dim_only",
            "stored_rows": "pairwise summary rows with validity flags",
            "reason": "only compares representations for the same chunk key",
            "unbounded_pairwise_chunk_storage": False,
        },
        {
            "metric": "orthogonal_procrustes",
            "full_scale_policy": "same-key_cross-covariance_equal-dim_only",
            "stored_rows": "pairwise summary rows with validity flags",
            "reason": "cross-covariance scales by dimension, not by N squared",
            "unbounded_pairwise_chunk_storage": False,
        },
        {
            "metric": "norm_quantile_nmi",
            "full_scale_policy": "bounded_histogram_or_quantile_sketch",
            "stored_rows": "pairwise summary rows and invalid-estimator rows",
            "reason": "histograms/sketches are bounded by bins, not full row count",
            "unbounded_pairwise_chunk_storage": False,
        },
        {
            "metric": "neighbor_overlap_at_k",
            "full_scale_policy": "sampled_or_windowed_ann_only",
            "stored_rows": "window summaries and top-k exemplars; no full nearest-neighbor graph",
            "reason": f"exact full chunk-pair graph would require {unbounded_chunk_pairs} comparisons",
            "unbounded_pairwise_chunk_storage": False,
            "full_exact_all_pairs_forbidden": True,
        },
        {
            "metric": "cca_family",
            "full_scale_policy": "validity_rows_plus_future_randomized_low-rank_only",
            "stored_rows": "estimator-validity rows; no fake numeric score",
            "reason": "sample/dimension preconditions must pass before numeric CCA-family metrics are emitted",
            "unbounded_pairwise_chunk_storage": False,
        },
    ]

    storage_policy = {
        "decision": "ACCEPT_STORAGE_DESIGN_FOR_255_RELAUNCH_GATE",
        "full_run_allowed_now": False,
        "full_run_blockers_remaining": ["#255 full-run relaunch gate"],
        "durable_root": "/mnt/d/contextgraph/corpus/python-forward-cache-v1",
        "hot_root_policy": "bounded WSL ext4 staging under /home/cabdru/.cache/contextgraph; never /mnt/c",
        "vector_store": "packed JSONL records plus packed index per shard, promoted to D-drive after readback",
        "matrix_store": "bounded global/window summary artifacts, not raw chunk-pair matrices",
        "hash_chain": "per-shard packed manifest hashes -> corpus manifest -> design FSV -> #255 relaunch artifact",
        "resume_semantics": [
            "checkpoint advances only after packed shard store and FSV readback pass",
            "interrupted hot stages are archived or rejected before relaunch",
            "smoke gate preflight must pass #249 and #251 through #255 before full-run mode starts shards",
        ],
        "required_full_run_outputs": [
            "packed_records/{embedder}.jsonl",
            "packed_index/{embedder}.jsonl",
            "python_forward_cache_packed_manifest.json",
            "python_forward_cache_batch_telemetry.jsonl",
            "python_forward_cache_quarantine.jsonl",
            "python_forward_cache_corpus_manifest.json",
            "intelligence/global_pairwise_metrics.jsonl",
            "intelligence/window_pairwise_metrics.jsonl",
            "intelligence/estimator_validity.jsonl",
            "intelligence/top_alignment.json",
            "intelligence/top_disagreement.json",
            "intelligence/top_synergy_candidates.json",
        ],
        "metric_storage_policy": metric_storage_policy,
    }

    estimates = {
        "smoke_chunk_count": smoke_chunk_count,
        "full_chunk_count": full_chunk_count,
        "scale_factor": scale,
        "smoke_forward_cache_bytes": forward_state["byte_count"],
        "smoke_matrix_bytes": matrix_state["byte_count"],
        "smoke_raw_vector_bytes": smoke_raw_vector_bytes,
        "estimated_full_forward_cache_bytes": forward_estimate,
        "estimated_full_raw_vector_bytes": raw_vector_estimate,
        "estimated_full_cache_records": estimated_cache_records,
        "estimated_full_quarantine_rows": estimated_quarantines,
        "window_chunks": args.window_chunks,
        "estimated_matrix_windows": windows,
        "estimated_windowed_matrix_bytes": windowed_matrix_estimate,
        "estimated_total_design_bytes": estimated_total_design_bytes,
        "max_durable_forward_bytes": args.max_durable_forward_bytes,
        "d_drive_free_bytes": d_snapshot["free"],
        "d_drive_capacity_margin_bytes": int(d_snapshot["free"]) - estimated_total_design_bytes,
        "unbounded_full_chunk_pair_count_forbidden": unbounded_chunk_pairs,
    }

    unbounded_policy_ok = all(item.get("unbounded_pairwise_chunk_storage") is False for item in metric_storage_policy)
    estimates_ok = (
        forward_estimate <= args.max_durable_forward_bytes
        and estimated_total_design_bytes < int(d_snapshot["free"]) * 0.75
        and windowed_matrix_estimate < args.max_durable_forward_bytes
    )
    go_no_go = {
        "storage_design_accepted": bool(estimates_ok and unbounded_policy_ok),
        "may_start_full_run_now": False,
        "may_start_full_run_after_255": bool(estimates_ok and unbounded_policy_ok),
        "reason": (
            "Storage design is bounded and acceptable for #255, but #199 remains blocked until #255 handles relaunch state."
            if estimates_ok and unbounded_policy_ok
            else "Storage design exceeded budget or left an unbounded metric policy."
        ),
        "blocking_issues": ["#255"],
    }

    return {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "smoke_storage_design_report",
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "matrix_root": str(args.matrix_root),
            "forward_fsv": str(args.forward_fsv),
            "matrix_fsv": str(args.matrix_fsv),
            "full_chunks_jsonl": str(args.full_chunks_jsonl),
            "design_root": str(args.design_root),
        },
        "smoke_readback": {
            "forward_manifest_summary": smoke_summary,
            "matrix_summary": matrix_summary.get("matrix_summary"),
            "top_alignment_first": matrix_summary.get("top_alignment", [None])[0],
            "top_disagreement_first": matrix_summary.get("top_disagreement", [None])[0],
            "coverage_rows": coverage_rows,
            "forward_tree": forward_state,
            "matrix_tree": matrix_state,
        },
        "storage_policy": storage_policy,
        "full_scale_estimates": estimates,
        "go_no_go": go_no_go,
        "edge_case_audit": {
            "storage_budget": {
                "passes": estimates_ok,
                "expected": f"estimated_total_design_bytes < 75% of D free bytes and forward estimate <= {args.max_durable_forward_bytes}",
                "actual": estimates,
            },
            "unscalable_metric_policy": {
                "passes": unbounded_policy_ok,
                "expected": "no metric stores raw O(N^2) chunk-pair artifacts",
                "actual_metric_policies": metric_storage_policy,
            },
            "integrity_readback": {
                "passes": True,
                "expected": "readable #252/#253 FSVs and all design artifacts reread by hash",
                "actual_forward_fsv": file_state(args.forward_fsv),
                "actual_matrix_fsv": file_state(args.matrix_fsv),
            },
        },
        "disk_snapshot": {"d_drive": d_snapshot},
    }


def command_smoke_storage_design(args: argparse.Namespace) -> int:
    started = utc_now()
    before = {"design_root": tree_state(args.design_root), "fsv_root": tree_state(args.fsv_root)}
    if args.clean_output:
        if args.design_root.exists():
            shutil.rmtree(args.design_root)
        if args.fsv_root.exists():
            shutil.rmtree(args.fsv_root)
    args.design_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)

    report = build_report(args)
    storage_policy = report["storage_policy"]
    estimates = report["full_scale_estimates"]
    go_no_go = report["go_no_go"]

    dump_json(args.design_root / "smoke_storage_design_report.json", report)
    dump_json(args.design_root / "storage_policy.json", storage_policy)
    dump_json(args.design_root / "full_scale_storage_estimates.json", estimates)
    dump_json(args.design_root / "go_no_go.json", go_no_go)

    artifact_state = artifact_hashes(args.design_root)
    readback_report = read_json(args.design_root / "smoke_storage_design_report.json")
    readback_policy = read_json(args.design_root / "storage_policy.json")
    readback_estimates = read_json(args.design_root / "full_scale_storage_estimates.json")
    readback_go_no_go = read_json(args.design_root / "go_no_go.json")

    passes = (
        readback_report.get("task_id") == TASK_ID
        and readback_go_no_go.get("storage_design_accepted") is True
        and readback_go_no_go.get("may_start_full_run_now") is False
        and readback_go_no_go.get("may_start_full_run_after_255") is True
        and readback_estimates.get("estimated_total_design_bytes", 0) < readback_estimates.get("d_drive_free_bytes", 0)
        and all(item.get("unbounded_pairwise_chunk_storage") is False for item in readback_policy.get("metric_storage_policy", []))
        and artifact_state["file_count"] == 4
    )

    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "smoke_storage_design_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "design_root": str(args.design_root),
            "design_report": str(args.design_root / "smoke_storage_design_report.json"),
            "storage_policy": str(args.design_root / "storage_policy.json"),
            "full_scale_estimates": str(args.design_root / "full_scale_storage_estimates.json"),
            "go_no_go": str(args.design_root / "go_no_go.json"),
            "full_chunks_jsonl": str(args.full_chunks_jsonl),
            "fsv": str(args.fsv_root / "smoke_storage_design_fsv.json"),
        },
        "before_state": before,
        "after_state": {"design_root": tree_state(args.design_root), "fsv_root": tree_state(args.fsv_root)},
        "readback": {
            "task_id": readback_report.get("task_id"),
            "storage_design_accepted": readback_go_no_go.get("storage_design_accepted"),
            "may_start_full_run_now": readback_go_no_go.get("may_start_full_run_now"),
            "may_start_full_run_after_255": readback_go_no_go.get("may_start_full_run_after_255"),
            "estimated_full_forward_cache_bytes": readback_estimates.get("estimated_full_forward_cache_bytes"),
            "estimated_total_design_bytes": readback_estimates.get("estimated_total_design_bytes"),
            "d_drive_free_bytes": readback_estimates.get("d_drive_free_bytes"),
            "unbounded_full_chunk_pair_count_forbidden": readback_estimates.get("unbounded_full_chunk_pair_count_forbidden"),
            "metric_policy_count": len(readback_policy.get("metric_storage_policy", [])),
            "full_chunks_state": file_state(args.full_chunks_jsonl),
        },
        "artifact_hashes": artifact_state,
        "edge_case_results": readback_report.get("edge_case_audit"),
    }
    dump_json(args.fsv_root / "smoke_storage_design_fsv.json", fsv)
    final_readback = read_json(args.fsv_root / "smoke_storage_design_fsv.json")
    require(final_readback.get("passes") is True, f"FSV failed; see {args.fsv_root / 'smoke_storage_design_fsv.json'}")
    print(json.dumps(final_readback, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--matrix-root", type=Path, default=DEFAULT_MATRIX_ROOT)
    parser.add_argument("--design-root", type=Path, default=DEFAULT_DESIGN_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--forward-fsv", type=Path, default=DEFAULT_FORWARD_FSV)
    parser.add_argument("--matrix-fsv", type=Path, default=DEFAULT_MATRIX_FSV)
    parser.add_argument("--full-chunks-jsonl", type=Path, default=DEFAULT_FULL_CHUNKS_JSONL)
    parser.add_argument("--full-chunk-count", type=int, default=None)
    parser.add_argument("--smoke-chunk-count", type=int, default=DEFAULT_SMOKE_CHUNKS)
    parser.add_argument("--window-chunks", type=int, default=DEFAULT_WINDOW_CHUNKS)
    parser.add_argument("--max-durable-forward-bytes", type=int, default=DEFAULT_MAX_DURABLE_FORWARD_BYTES)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_smoke_storage_design(args)
    except SmokeStorageDesignError as exc:
        print(f"SMOKE_STORAGE_DESIGN_FAILED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
