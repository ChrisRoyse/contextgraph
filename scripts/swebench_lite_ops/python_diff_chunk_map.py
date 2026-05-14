from __future__ import annotations

from pathlib import Path
from typing import Any

from .core import utc_now, write_json_checked
from .oracle_events import (
    capacity_snapshot,
    file_state,
    load_jsonschema_validator,
    path_bytes,
    sha256_file,
    write_jsonl_checked,
)
from .python_diff_chunk_map_core import (
    SCHEMA_VERSION,
    TASK_ID,
    DiffChunkMapError,
    build_record,
    count_jsonl,
    edge_records,
    link_test_records,
    load_chunks,
    mapping_schema,
    parse_unified_diff,
    read_json,
    validate_records,
)


def command_diff_chunk_map(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.map_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.map_root / "diff_chunk_map_schema.json"
    records_path = args.map_root / "diff_chunk_map_records.jsonl"
    edge_path = args.map_root / "edge_case_diff_chunk_map_records.jsonl"
    manifest_path = args.map_root / "diff_chunk_map_manifest.json"
    fsv_path = args.fsv_root / "diff_chunk_map_fsv.json"
    before_state = {
        "checked_at": checked_at,
        "mapping_count": count_jsonl(records_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "hot_bytes": path_bytes(args.hot_root),
        "map_root_bytes": path_bytes(args.map_root),
    }
    workspace_manifest = read_json(args.workspace_manifest, "PY_DIFF_CHUNK_WORKSPACE_MANIFEST_INVALID")
    schema = mapping_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)
    chunks = load_chunks(args.chunks_jsonl)
    now = utc_now()
    records: list[dict[str, Any]] = []
    for state in workspace_manifest["states"]:
        for patch in state.get("applied_patches", []):
            text = Path(patch["patch"]["path"]).read_text(encoding="utf-8")
            for hunk in parse_unified_diff(text):
                records.append(build_record(state, patch, hunk, chunks, schema_path, now))
    link_test_records(records)
    validate_records(records, validator)
    if not records:
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_NO_RECORDS",
            "no diff-to-chunk mapping records were emitted",
            "Check workspace manifest applied_patches before continuing.",
            {"workspace_manifest": str(args.workspace_manifest)},
        )
    record_store = write_jsonl_checked(records_path, records)
    edges, edge_cases = edge_records(schema_path, args.hot_root, now)
    validate_records(edges, validator)
    edge_store = write_jsonl_checked(edge_path, edges)
    if not all(case["pass"] for case in edge_cases):
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_EDGE_FSV_FAILED",
            "diff-to-chunk edge-case mapping failed",
            "Fix edge behavior before closing TASK-PY-DATA-011.",
            {"edge_cases": edge_cases},
        )
    summary: dict[str, Any] = {"total": len(records), "by_patch_label": {}, "by_change_kind": {}}
    for record in records:
        summary["by_patch_label"][record["patch_label"]] = summary["by_patch_label"].get(record["patch_label"], 0) + 1
        summary["by_change_kind"][record["change_kind"]] = summary["by_change_kind"].get(record["change_kind"], 0) + 1
    happy_prod = next((r for r in records if r["task_instance_id"] == "astropy__astropy-12907" and r["patch_label"] == "gold" and r["file_new_path"] == "astropy/modeling/separable.py"), None)
    happy_test = next((r for r in records if r["task_instance_id"] == "astropy__astropy-12907" and r["patch_label"] == "test" and r["related_production_chunk_ids"]), None)
    happy_path = {
        "production_maps_cstack": bool(happy_prod and any("01a9396bc3468b036e161c5a192851210abbec90315cc4dca9c77b8981a5c413" in cid for cid in happy_prod["primary_chunk_ids"])),
        "test_maps_and_links_to_production": bool(happy_test),
    }
    if not all(happy_path.values()):
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_HAPPY_PATH_FAILED",
            "known production/test patch did not map to expected chunks",
            "Fix diff mapping before using records as reward data.",
            {"happy_path": happy_path},
        )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 194,
        "source_of_truth": {
            "schema": str(schema_path),
            "records_jsonl": str(records_path),
            "edge_records_jsonl": str(edge_path),
            "manifest": str(manifest_path),
            "fsv": str(fsv_path),
        },
        "inputs": {
            "workspace_manifest": str(args.workspace_manifest),
            "chunks_jsonl": str(args.chunks_jsonl),
        },
        "summary": summary,
        "records_sha256": record_store["sha256"],
        "edge_records_sha256": edge_store["sha256"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)
    after_state = {
        "checked_at": utc_now(),
        "mapping_count": count_jsonl(records_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/mnt/d")]),
        "hot_bytes": path_bytes(args.hot_root),
        "map_root_bytes": path_bytes(args.map_root),
        "fsv_root_bytes": path_bytes(args.fsv_root),
    }
    fsv = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 194,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py diff-chunk-map",
            "workspace_state_count": len(workspace_manifest["states"]),
            "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false",
        },
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_cases,
        "summary": summary,
        "passes": True,
    }
    write_json_checked(fsv_path, fsv)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": fsv["trigger"],
        "after_state": after_state,
        "happy_path": happy_path,
        "summary": summary,
        "record_store": record_store,
        "edge_store": edge_store,
        "manifest_sha256": sha256_file(manifest_path),
        "fsv_sha256": sha256_file(fsv_path),
        "passes": True,
    }
