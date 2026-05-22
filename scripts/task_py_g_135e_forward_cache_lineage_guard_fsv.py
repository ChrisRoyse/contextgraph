#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

from swebench_lite_ops.forward_cache_lineage import ForwardCacheLineageError, guard_packed_record_lineage
from swebench_lite_ops.python_real_corpus_constellation_smoke import (
    DEFAULT_CHUNKS_JSONL,
    DEFAULT_FORWARD_ROOT,
    EXPECTED_EMBEDDERS,
    load_chunk_manifest_window,
    read_selected_jsonl,
    select_completed_shards,
    spread_indices,
)
from swebench_lite_ops.smoke_intelligence_matrices import dump_json, read_json, sha256_file, utc_now


TASK_ID = "TASK-PY-G-135E"
ARTIFACT_KIND = "forward_cache_lineage_guard_fsv"
DEFAULT_FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-135e-forward-cache-lineage-guard-fsv")


def edge_cases() -> list[dict[str, Any]]:
    legacy_record = {
        "key": {"chunk_id": "chunk-1", "source_sha256": "sha256:abc"},
        "chunk": {
            "task_instance_id": "task-1",
            "workspace_state": "after",
            "absolute_path": "/mnt/d/contextgraph/corpus/old.py",
            "dataschema": "/mnt/d/contextgraph/corpus/old.schema.json",
        },
    }
    canonical = {
        "task_instance_id": "task-1",
        "workspace_state": "after",
        "chunk_id": "chunk-1",
        "source_sha256": "sha256:abc",
        "absolute_path": "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z/task-1/after/old.py",
        "dataschema": "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.schema.json",
    }
    cases: list[dict[str, Any]] = []

    lineage = guard_packed_record_lineage(legacy_record, canonical, context="edge:legacy_payload")
    cases.append(
        {
            "name": "legacy_packed_payload_allowed_only_as_provenance",
            "passes": lineage["raw_packed_path_metadata_legacy"] is True and lineage["canonical_ast_lineage_used"] is True,
            "lineage": lineage,
        }
    )

    stale = dict(canonical)
    stale["absolute_path"] = "/mnt/d/contextgraph/corpus/old.py"
    try:
        guard_packed_record_lineage(legacy_record, stale, context="edge:stale_canonical")
        cases.append({"name": "legacy_canonical_ast_path_rejected", "passes": False, "error": None})
    except ForwardCacheLineageError as exc:
        cases.append(
            {
                "name": "legacy_canonical_ast_path_rejected",
                "passes": exc.code == "MEJEPA_FORWARD_CACHE_CANONICAL_LINEAGE_LEGACY_MOUNTED_PATH",
                "error_code": exc.code,
                "error": str(exc),
            }
        )

    wrong_key = dict(canonical)
    wrong_key["chunk_id"] = "different-chunk"
    try:
        guard_packed_record_lineage(legacy_record, wrong_key, context="edge:key_mismatch")
        cases.append({"name": "packed_to_canonical_key_mismatch_rejected", "passes": False, "error": None})
    except ForwardCacheLineageError as exc:
        cases.append(
            {
                "name": "packed_to_canonical_key_mismatch_rejected",
                "passes": exc.code == "MEJEPA_FORWARD_CACHE_LINEAGE_KEY_MISMATCH",
                "error_code": exc.code,
                "error": str(exc),
            }
        )

    wrong_root = dict(canonical)
    wrong_root["absolute_path"] = "/tmp/contextgraph/not-production.py"
    try:
        guard_packed_record_lineage(legacy_record, wrong_root, context="edge:wrong_root")
        cases.append({"name": "non_aiwonder_absolute_canonical_path_rejected", "passes": False, "error": None})
    except ForwardCacheLineageError as exc:
        cases.append(
            {
                "name": "non_aiwonder_absolute_canonical_path_rejected",
                "passes": exc.code == "MEJEPA_FORWARD_CACHE_CANONICAL_LINEAGE_NOT_AIWONDER_ROOT",
                "error_code": exc.code,
                "error": str(exc),
            }
        )
    return cases


def sample_forward_cache(args: argparse.Namespace) -> dict[str, Any]:
    shards, _source_summary = select_completed_shards(args.forward_root, args.sample_shards)
    manifest_by_key, chunk_summary = load_chunk_manifest_window(args.chunks_jsonl, shards)
    sampled_records = 0
    raw_legacy_records = 0
    canonical_lineage_records = 0
    errors: list[dict[str, Any]] = []
    per_embedder: dict[str, int] = {}

    for shard in shards:
        shard_id = str(shard.get("shard_id") or "")
        shard_root = args.forward_root / "shards" / shard_id
        packed_manifest = read_json(shard_root / "python_forward_cache_packed_manifest.json")
        per_embedder_manifest = packed_manifest.get("per_embedder") or {}
        for embedder_id in args.expected_embedders.split(","):
            embedder_id = embedder_id.strip()
            if not embedder_id:
                continue
            entry = per_embedder_manifest.get(embedder_id)
            if entry is None:
                errors.append({"code": "MISSING_EMBEDDER", "shard_id": shard_id, "embedder_id": embedder_id})
                continue
            row_count = int(entry.get("row_count") or 0)
            wanted = spread_indices(row_count, min(args.sample_rows_per_embedder_per_shard, row_count))
            record_rows, _line_count = read_selected_jsonl(Path(entry.get("record_path") or ""), wanted)
            for line_index in wanted:
                record = record_rows[line_index]
                try:
                    from swebench_lite_ops.forward_cache_lineage import packed_record_row_key

                    row_key = packed_record_row_key(record)
                    canonical_chunk = manifest_by_key.get(row_key)
                    if canonical_chunk is None:
                        raise ForwardCacheLineageError(
                            "MEJEPA_FORWARD_CACHE_CANONICAL_CHUNK_MISSING",
                            "packed record has no canonical AST row",
                            {"shard_id": shard_id, "embedder_id": embedder_id, "row_key": row_key},
                        )
                    lineage = guard_packed_record_lineage(
                        record,
                        canonical_chunk,
                        context=f"{shard_id}:{embedder_id}:{line_index + 1}",
                    )
                except ForwardCacheLineageError as exc:
                    if len(errors) < 32:
                        errors.append(
                            {
                                "code": exc.code,
                                "message": exc.message,
                                "details": exc.details,
                                "shard_id": shard_id,
                                "embedder_id": embedder_id,
                                "line_index": line_index,
                            }
                        )
                    continue
                sampled_records += 1
                per_embedder[embedder_id] = per_embedder.get(embedder_id, 0) + 1
                canonical_lineage_records += 1 if lineage["canonical_ast_lineage_used"] else 0
                raw_legacy_records += 1 if lineage["raw_packed_path_metadata_legacy"] else 0

    return {
        "sampled_records": sampled_records,
        "raw_forward_cache_legacy_path_metadata_records": raw_legacy_records,
        "canonical_ast_lineage_records": canonical_lineage_records,
        "per_embedder_sampled_records": dict(sorted(per_embedder.items())),
        "errors": errors,
        "chunk_summary": chunk_summary,
    }


def build_report(args: argparse.Namespace) -> dict[str, Any]:
    edges = edge_cases()
    sample = sample_forward_cache(args)
    passes = (
        all(case.get("passes") is True for case in edges)
        and sample["sampled_records"] > 0
        and sample["canonical_ast_lineage_records"] == sample["sampled_records"]
        and sample["raw_forward_cache_legacy_path_metadata_records"] >= args.min_raw_legacy_records
        and not sample["errors"]
    )
    return {
        "schema_version": 1,
        "task_id": args.task_id,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "forward_root": str(args.forward_root),
            "chunks_jsonl": str(args.chunks_jsonl),
            "fsv_root": str(args.fsv_root),
        },
        "policy": "raw packed chunk path fields are legacy provenance only; active source lineage must come from canonical AST rows",
        "edge_cases": edges,
        "sample": sample,
        "thresholds": {
            "sample_shards": args.sample_shards,
            "sample_rows_per_embedder_per_shard": args.sample_rows_per_embedder_per_shard,
            "min_raw_legacy_records": args.min_raw_legacy_records,
        },
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--forward-root", type=Path, default=DEFAULT_FORWARD_ROOT)
    parser.add_argument("--chunks-jsonl", type=Path, default=DEFAULT_CHUNKS_JSONL)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--expected-embedders", default=",".join(EXPECTED_EMBEDDERS))
    parser.add_argument("--sample-shards", type=int, default=1)
    parser.add_argument("--sample-rows-per-embedder-per-shard", type=int, default=3)
    parser.add_argument("--min-raw-legacy-records", type=int, default=1)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    report = build_report(args)
    report_path = args.fsv_root / f"{ARTIFACT_KIND}.json"
    readback_path = args.fsv_root / f"{ARTIFACT_KIND}_readback.json"
    dump_json(report_path, report)
    readback = read_json(report_path)
    readback_report = {
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "created_at_utc": utc_now(),
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "readback_equal": readback == report,
        "passes": bool(readback.get("passes")) and readback == report,
        "sampled_records": readback.get("sample", {}).get("sampled_records"),
        "raw_forward_cache_legacy_path_metadata_records": readback.get("sample", {}).get(
            "raw_forward_cache_legacy_path_metadata_records"
        ),
    }
    dump_json(readback_path, readback_report)
    print(
        json.dumps(
            {
                "report": str(report_path),
                "report_sha256": sha256_file(report_path),
                "readback": str(readback_path),
                "readback_sha256": sha256_file(readback_path),
                "passes": report["passes"] and readback_report["passes"],
                "sample": report["sample"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if report["passes"] and readback_report["passes"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
