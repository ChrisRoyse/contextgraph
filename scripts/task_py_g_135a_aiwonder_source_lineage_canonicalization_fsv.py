#!/usr/bin/env python3
"""TASK-PY-G-135A aiwonder source-lineage canonicalization FSV.

This harness promotes the retained Python AST chunk JSONL from legacy mounted
drive source paths to aiwonder-native `/zfs/archive/contextgraph/...` paths.
It keeps the old paths only as explicit legacy provenance fields and verifies
source bytes by hash before replacing the active artifact.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import shutil
import sys
from collections import Counter
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

from swebench_lite_ops.core import SwebenchOpsError, utc_now  # noqa: E402
from swebench_lite_ops.python_ast_chunks import chunk_schema, decode_source  # noqa: E402
from swebench_lite_ops.python_static_graph_core import records_for_parse_record  # noqa: E402
from swebench_lite_ops.smoke_manifest import validate_candidate_source  # noqa: E402
from swebench_lite_ops.source_readback import (  # noqa: E402
    is_legacy_mounted_path,
    read_full_source_bytes,
    sha256_prefixed_bytes,
)


TASK_ID = "TASK-PY-G-135A"
AST_CHUNKS_PATH = Path(os.environ.get("CG_AST_CHUNKS", "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl"))
AST_ROOT = AST_CHUNKS_PATH.parent
SOURCE_ROOT = Path(
    os.environ.get(
        "CG_AST_SOURCE_ROOT",
        str(AST_ROOT / "source-files-300x8-20260515T013205Z"),
    )
)
SCHEMA_PATH = AST_ROOT / "python_ast_chunk_schema.json"
FSV_ROOT = Path(
    os.environ.get(
        "CG_FSV_TASK_PY_G_135A_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-135a-aiwonder-source-lineage-canonicalization-fsv",
    )
)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    with path.open("r", encoding="utf-8") as fh:
        return sum(1 for line in fh if line.strip())


def load_first_row(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as fh:
        for line in fh:
            if line.strip():
                return json.loads(line)
    raise RuntimeError(f"empty JSONL: {path}")


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def source_suffix(legacy_path: str, source_root: Path) -> str:
    marker = f"/{source_root.name}/"
    if marker not in legacy_path:
        raise RuntimeError(f"cannot map legacy source path to {source_root.name}: {legacy_path}")
    suffix = legacy_path.split(marker, 1)[1]
    if not suffix or suffix.startswith("../") or "/../" in suffix:
        raise RuntimeError(f"unsafe source suffix from legacy path: {legacy_path}")
    return suffix


def canonicalize_active_ast(run_dir: Path, run_id: str) -> dict[str, Any]:
    if not AST_CHUNKS_PATH.is_file():
        raise RuntimeError(f"AST chunks JSONL missing: {AST_CHUNKS_PATH}")
    if not SOURCE_ROOT.is_dir():
        raise RuntimeError(f"canonical source root missing: {SOURCE_ROOT}")

    tmp_jsonl = AST_CHUNKS_PATH.with_name(f"{AST_CHUNKS_PATH.name}.g135a-{run_id}.tmp")
    tmp_schema = SCHEMA_PATH.with_name(f"{SCHEMA_PATH.name}.g135a-{run_id}.tmp")
    backup_jsonl = AST_CHUNKS_PATH.with_name(f"{AST_CHUNKS_PATH.stem}.legacy-pre-g135a-{run_id}{AST_CHUNKS_PATH.suffix}")
    backup_schema = SCHEMA_PATH.with_name(f"{SCHEMA_PATH.stem}.legacy-pre-g135a-{run_id}{SCHEMA_PATH.suffix}")

    before_sha = sha256_file(AST_CHUNKS_PATH)
    before_schema_sha = sha256_file(SCHEMA_PATH) if SCHEMA_PATH.is_file() else None
    before_count = count_jsonl(AST_CHUNKS_PATH)
    counters: Counter[str] = Counter()
    examples: list[dict[str, Any]] = []
    file_bytes_cache: dict[Path, bytes] = {}

    tmp_schema.write_text(json.dumps(chunk_schema(SCHEMA_PATH), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    with AST_CHUNKS_PATH.open("r", encoding="utf-8") as src, tmp_jsonl.open("w", encoding="utf-8") as dst:
        for line_number, line in enumerate(src, start=1):
            if not line.strip():
                continue
            row = json.loads(line)
            old_abs = str(row.get("absolute_path") or "")
            old_schema = str(row.get("dataschema") or "")
            if is_legacy_mounted_path(old_abs):
                counters["legacy_absolute_path_rows_before"] += 1
            if is_legacy_mounted_path(old_schema):
                counters["legacy_dataschema_rows_before"] += 1

            suffix = source_suffix(old_abs, SOURCE_ROOT)
            canonical_path = SOURCE_ROOT / suffix
            if canonical_path not in file_bytes_cache:
                if not canonical_path.is_file():
                    raise RuntimeError(f"canonical source file missing for row {line_number}: {canonical_path}")
                file_bytes_cache[canonical_path] = canonical_path.read_bytes()
            actual_file_sha = sha256_prefixed_bytes(file_bytes_cache[canonical_path])
            if actual_file_sha != row["file_sha256"]:
                raise RuntimeError(
                    f"file sha mismatch for row {line_number}: expected {row['file_sha256']} actual {actual_file_sha} path {canonical_path}"
                )
            source_bytes = str(row["source_text"]).encode(str(row.get("encoding") or "utf-8"))
            if sha256_prefixed_bytes(source_bytes) != row["source_sha256"]:
                raise RuntimeError(f"source_text sha mismatch for row {line_number}: {row.get('chunk_id')}")
            byte_start = int(row["byte_start"])
            byte_end = int(row["byte_end"])
            if byte_start < 0 or byte_end < byte_start:
                raise RuntimeError(f"invalid byte range for row {line_number}: {byte_start}:{byte_end}")
            if file_bytes_cache[canonical_path][byte_start:byte_end] != source_bytes:
                raise RuntimeError(
                    f"canonical source slice mismatch for row {line_number}: {row.get('chunk_id')} path {canonical_path}"
                )

            old_chunk_id = row["chunk_id"]
            old_source_sha = row["source_sha256"]
            row["absolute_path"] = str(canonical_path)
            row["dataschema"] = str(SCHEMA_PATH)
            row["legacy_provenance_path"] = row.get("legacy_provenance_path") or old_abs
            row["legacy_dataschema_path"] = row.get("legacy_dataschema_path") or old_schema
            if row["chunk_id"] != old_chunk_id or row["source_sha256"] != old_source_sha:
                raise RuntimeError(f"stable key changed for row {line_number}: {old_chunk_id}")
            if is_legacy_mounted_path(row["absolute_path"]):
                counters["legacy_absolute_path_rows_after"] += 1
            if is_legacy_mounted_path(row["dataschema"]):
                counters["legacy_dataschema_rows_after"] += 1
            counters["rows_written"] += 1
            counters[f"category:{row.get('mutation_category')}"] += 1
            if len(examples) < 5:
                examples.append(
                    {
                        "line_number": line_number,
                        "chunk_id": row["chunk_id"],
                        "absolute_path": row["absolute_path"],
                        "legacy_provenance_path": row["legacy_provenance_path"],
                    }
                )
            dst.write(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n")

    if counters["rows_written"] != before_count:
        raise RuntimeError(f"row count changed: before {before_count}, after {counters['rows_written']}")

    shutil.copy2(AST_CHUNKS_PATH, backup_jsonl)
    if SCHEMA_PATH.is_file():
        shutil.copy2(SCHEMA_PATH, backup_schema)
    tmp_jsonl.replace(AST_CHUNKS_PATH)
    tmp_schema.replace(SCHEMA_PATH)

    after_count = count_jsonl(AST_CHUNKS_PATH)
    after_sha = sha256_file(AST_CHUNKS_PATH)
    after_schema_sha = sha256_file(SCHEMA_PATH)
    return {
        "before": {
            "ast_chunks_path": str(AST_CHUNKS_PATH),
            "row_count": before_count,
            "sha256": before_sha,
            "schema_sha256": before_schema_sha,
        },
        "after": {
            "ast_chunks_path": str(AST_CHUNKS_PATH),
            "row_count": after_count,
            "sha256": after_sha,
            "schema_sha256": after_schema_sha,
        },
        "backups": {
            "ast_chunks_backup": str(backup_jsonl),
            "schema_backup": str(backup_schema) if backup_schema.exists() else None,
        },
        "counters": dict(counters),
        "examples": examples,
        "source_root": str(SOURCE_ROOT),
        "unique_source_files_verified": len(file_bytes_cache),
    }


def readback_active_ast(path: Path) -> dict[str, Any]:
    counters: Counter[str] = Counter()
    examples: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as fh:
        for line_number, line in enumerate(fh, start=1):
            if not line.strip():
                continue
            row = json.loads(line)
            counters["rows"] += 1
            if is_legacy_mounted_path(row.get("absolute_path")):
                counters["active_absolute_path_legacy"] += 1
            if is_legacy_mounted_path(row.get("dataschema")):
                counters["active_dataschema_legacy"] += 1
            if is_legacy_mounted_path(row.get("legacy_provenance_path")):
                counters["legacy_provenance_rows"] += 1
            if is_legacy_mounted_path(row.get("legacy_dataschema_path")):
                counters["legacy_dataschema_rows"] += 1
            if len(examples) < 3:
                examples.append(
                    {
                        "line_number": line_number,
                        "chunk_id": row.get("chunk_id"),
                        "absolute_path": row.get("absolute_path"),
                        "dataschema": row.get("dataschema"),
                        "legacy_provenance_path": row.get("legacy_provenance_path"),
                    }
                )
    return {"counters": dict(counters), "examples": examples, "sha256": sha256_file(path)}


def consumer_edge_checks(row: dict[str, Any], run_dir: Path) -> dict[str, Any]:
    checks: dict[str, Any] = {}
    legacy_row = dict(row)
    legacy_row["absolute_path"] = str(legacy_row.get("legacy_provenance_path") or "/mnt/d/contextgraph/missing.py")
    try:
        read_full_source_bytes(
            legacy_row,
            legacy_code="EDGE_LEGACY_REJECTED",
            missing_code="EDGE_MISSING",
            hash_mismatch_code="EDGE_HASH",
            remediation="edge",
        )
        checks["legacy_path_rejected"] = {"pass": False, "error_code": None}
    except SwebenchOpsError as exc:
        checks["legacy_path_rejected"] = {"pass": exc.code == "EDGE_LEGACY_REJECTED", "error_code": exc.code}

    try:
        path, raw = read_full_source_bytes(
            row,
            legacy_code="EDGE_CANONICAL_LEGACY",
            missing_code="EDGE_CANONICAL_MISSING",
            hash_mismatch_code="EDGE_CANONICAL_HASH",
            remediation="edge",
        )
        checks["canonical_file_readback"] = {
            "pass": True,
            "path": str(path),
            "file_sha256": sha256_prefixed_bytes(raw),
        }
    except SwebenchOpsError as exc:
        checks["canonical_file_readback"] = {"pass": False, "error_code": exc.code, "details": exc.details}

    legacy_smoke = validate_candidate_source(legacy_row)
    checks["smoke_manifest_legacy_uses_embedded_source"] = {
        "pass": legacy_smoke.get("live_workspace_verdict") == "legacy_mounted_path_rejected"
        and legacy_smoke.get("source_text_matches_declared_hash") is True,
        "proof": legacy_smoke,
    }

    try:
        _, _, _ = decode_source(row)
        checks["python_ast_chunks_decode_source"] = {"pass": True}
    except SwebenchOpsError as exc:
        checks["python_ast_chunks_decode_source"] = {"pass": False, "error_code": exc.code, "details": exc.details}

    try:
        graph_schema_path = run_dir / "edge_python_static_graph_schema.json"
        parse_record = {
            **row,
            "parse_status": "parsed",
            "module_name": None,
        }
        records = records_for_parse_record(parse_record, graph_schema_path, utc_now(), {}, {})
        checks["python_static_graph_core_canonical_readback"] = {"pass": len(records) > 0, "record_count": len(records)}
    except Exception as exc:  # noqa: BLE001 - FSV edge evidence records typed failure text.
        checks["python_static_graph_core_canonical_readback"] = {"pass": False, "error": str(exc)[:500]}

    return checks


def main() -> int:
    run_id = dt.datetime.now(dt.timezone.utc).strftime("run-%Y%m%dT%H%M%SZ")
    run_dir = FSV_ROOT / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    report_path = run_dir / "task_py_g_135a_aiwonder_source_lineage_canonicalization_fsv.json"
    readback_path = run_dir / "task_py_g_135a_aiwonder_source_lineage_readback.json"

    result: dict[str, Any] = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "checked_at": utc_now(),
        "inputs": {
            "ast_chunks_path": str(AST_CHUNKS_PATH),
            "schema_path": str(SCHEMA_PATH),
            "source_root": str(SOURCE_ROOT),
            "fsv_root": str(FSV_ROOT),
        },
    }
    try:
        result["canonicalization"] = canonicalize_active_ast(run_dir, run_id)
        result["readback"] = readback_active_ast(AST_CHUNKS_PATH)
        first = load_first_row(AST_CHUNKS_PATH)
        result["consumer_edge_checks"] = consumer_edge_checks(first, run_dir)
        edge_pass = all(item.get("pass") is True for item in result["consumer_edge_checks"].values())
        readback = result["readback"]["counters"]
        result["all_passed"] = (
            result["canonicalization"]["after"]["row_count"] == 399777
            and readback.get("rows") == 399777
            and readback.get("active_absolute_path_legacy", 0) == 0
            and readback.get("active_dataschema_legacy", 0) == 0
            and readback.get("legacy_provenance_rows") == 399777
            and edge_pass
        )
        result["promoted"] = result["all_passed"]
        result["blockers"] = [] if result["all_passed"] else ["TASK_PY_G_135A_CANONICALIZATION_READBACK_FAILED"]
    except Exception as exc:  # noqa: BLE001 - retained FSV should capture root failure.
        result["all_passed"] = False
        result["promoted"] = False
        result["blockers"] = ["TASK_PY_G_135A_CANONICALIZATION_FAILED"]
        result["error"] = repr(exc)

    write_json(report_path, result)
    readback = {
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "report_reopened": json.loads(report_path.read_text(encoding="utf-8")),
    }
    write_json(readback_path, readback)
    print(json.dumps({"report": str(report_path), "readback": str(readback_path), "all_passed": result["all_passed"]}, indent=2, sort_keys=True))
    return 0 if result["all_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
