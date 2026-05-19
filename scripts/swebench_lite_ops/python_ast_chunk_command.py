from __future__ import annotations

import hashlib
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
from .python_ast_chunks import (
    SCHEMA_VERSION,
    TASK_ID,
    AstChunkError,
    chunk_schema,
    chunks_for_record,
    load_jsonl,
)


def validate_chunks(chunks: list[dict[str, Any]], validator: Any) -> None:
    for chunk in chunks:
        errors = sorted(validator.iter_errors(chunk), key=lambda err: list(err.path))
        if errors:
            raise AstChunkError(
                "PY_AST_CHUNK_SCHEMA_VALIDATION_FAILED",
                "generated AST chunk failed strict JSON Schema validation",
                "Fix chunk builder before writing reward data.",
                {"chunk_id": chunk.get("chunk_id"), "errors": [err.message for err in errors[:10]]},
            )


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def edge_records(schema_path: Path, hot_root: Path, now: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    edge_root = hot_root / "edge-files"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases = {
        "nested.py": "class Outer:\n    def method(self):\n        def inner():\n            return 1\n        return inner()\n",
        "decorator_fixture.py": "import pytest\n\n@pytest.fixture(name='thing')\ndef decorated():\n    assert True\n    return 1\n",
        "imports_only.py": "# comment\nimport os\nfrom sys import path\n",
        "comments_only.py": "# only comments\n# no code\n",
    }
    records = []
    for name, text in cases.items():
        path = edge_root / name
        path.write_text(text, encoding="utf-8")
        raw = path.read_bytes()
        records.append(
            {
                "parse_status": "empty" if name == "comments_only.py" else "parsed",
                "absolute_path": str(path),
                "relative_path": name,
                "file_sha256": "sha256:" + hashlib.sha256(raw).hexdigest(),
                "task_instance_id": "synthetic_py_ast_chunks",
                "workspace_state": "edge_cases",
                "mutation_category": "chunk_edge_case",
                "repo": "synthetic/ast-chunks",
                "workspace_tree_hash": "0" * 40,
            }
        )
    chunks: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    for record in records:
        record_chunks, record_diagnostics = chunks_for_record(record, schema_path, now)
        chunks.extend(record_chunks)
        diagnostics.extend(record_diagnostics)
    observed = {
        "nested_parented": any(chunk["relative_path"] == "nested.py" and chunk["parent_chunk_id"] for chunk in chunks),
        "fixture_chunk": any(chunk["relative_path"] == "decorator_fixture.py" and chunk["chunk_kind"] == "fixture" for chunk in chunks),
        "assertion_chunk": any(chunk["relative_path"] == "decorator_fixture.py" and chunk["chunk_kind"] == "assertion" for chunk in chunks),
        "imports_only": sum(1 for chunk in chunks if chunk["relative_path"] == "imports_only.py" and chunk["chunk_kind"] == "import"),
        "comments_only": any(chunk["relative_path"] == "comments_only.py" and chunk["chunk_kind"] == "module_comment_only" for chunk in chunks),
    }
    return chunks, {
        "before_state": {"edge_chunk_count": count_jsonl(hot_root / "edge_case_chunks.jsonl")},
        "after_state": {"edge_chunk_count": len(chunks), "observed": observed},
        "expected": {
            "nested_parented": True,
            "fixture_chunk": True,
            "assertion_chunk": True,
            "imports_only": 2,
            "comments_only": True,
        },
        "diagnostics": diagnostics,
        "pass": observed == {
            "nested_parented": True,
            "fixture_chunk": True,
            "assertion_chunk": True,
            "imports_only": 2,
            "comments_only": True,
        },
    }


def summarize(chunks: list[dict[str, Any]]) -> dict[str, Any]:
    by_kind: dict[str, int] = {}
    by_workspace: dict[str, int] = {}
    for chunk in chunks:
        by_kind[chunk["chunk_kind"]] = by_kind.get(chunk["chunk_kind"], 0) + 1
        key = f"{chunk['task_instance_id']}::{chunk['workspace_state']}"
        by_workspace[key] = by_workspace.get(key, 0) + 1
    return {"total": len(chunks), "by_kind": by_kind, "by_workspace": by_workspace}


def command_ast_chunks(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.chunk_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.chunk_root / "python_ast_chunk_schema.json"
    chunks_path = args.chunk_root / "python_ast_chunks.jsonl"
    edge_path = args.chunk_root / "edge_case_chunks.jsonl"
    manifest_path = args.chunk_root / "python_ast_chunk_manifest.json"
    fsv_path = args.fsv_root / "python_ast_chunk_fsv.json"
    before_state = {
        "checked_at": checked_at,
        "chunk_count": count_jsonl(chunks_path),
        "edge_chunk_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "chunks": file_state(chunks_path),
        "edge_chunks": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "chunk_root_bytes": path_bytes(args.chunk_root),
    }
    parse_records = load_jsonl(args.parse_records, "PY_AST_CHUNK_PARSE_RECORDS_INVALID")
    schema = chunk_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)
    now = utc_now()
    chunks: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    parsed_records = [record for record in parse_records if record.get("parse_status") in {"parsed", "empty", "syntax_error"}]
    for record in parsed_records:
        record_chunks, record_diagnostics = chunks_for_record(record, schema_path, now)
        chunks.extend(record_chunks)
        diagnostics.extend(record_diagnostics)
    validate_chunks(chunks, validator)
    chunk_store = write_jsonl_checked(chunks_path, chunks)
    edges, edge_audit = edge_records(schema_path, args.hot_root, now)
    validate_chunks(edges, validator)
    edge_store = write_jsonl_checked(edge_path, edges)
    summary = summarize(chunks)
    happy = next(
        (
            chunk
            for chunk in chunks
            if chunk["relative_path"] == "astropy/modeling/separable.py"
            and chunk["name"] == "_cstack"
            and chunk["workspace_state"] == "gold"
        ),
        None,
    )
    if happy is None:
        raise AstChunkError(
            "PY_AST_CHUNK_HAPPY_PATH_MISSING",
            "known patched function chunk was not emitted",
            "Inspect AST chunk semantic rules for function chunk coverage.",
            {"target": "astropy/modeling/separable.py::_cstack::gold"},
        )
    happy_path = {
        "case": "known_patched_function_chunk",
        "expected": "gold astropy/modeling/separable.py::_cstack function chunk with verified source slice",
        "observed": {
            "chunk_id": happy["chunk_id"],
            "chunk_kind": happy["chunk_kind"],
            "node_kind": happy["node_kind"],
            "line_start": happy["line_start"],
            "line_end": happy["line_end"],
            "source_slice_verified": happy["source_slice_verified"],
            "source_contains_patch": "right" in happy["source_text"],
        },
        "pass": happy["chunk_kind"] == "function" and happy["source_slice_verified"] and "right" in happy["source_text"],
    }
    if not happy_path["pass"] or not edge_audit["pass"]:
        raise AstChunkError(
            "PY_AST_CHUNK_FSV_FAILED",
            "AST chunk FSV checks failed",
            "Fix chunk emission before using chunks for reward data.",
            {"happy_path": happy_path, "edge_audit": edge_audit},
        )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 193,
        "source_of_truth": {
            "schema": str(schema_path),
            "chunks_jsonl": str(chunks_path),
            "edge_chunks_jsonl": str(edge_path),
            "manifest": str(manifest_path),
            "fsv": str(fsv_path),
        },
        "inputs": {"parse_records": str(args.parse_records)},
        "storage_policy": {
            "decision_issue": 212,
            "durable_root": "/zfs/archive/contextgraph",
            "hot_root": str(args.hot_root),
            "hot_budget_bytes": args.hot_budget_bytes,
        },
        "summary": summary,
        "diagnostics": {
            "chunk_reparse_warning_count": len(diagnostics),
            "edge_reparse_warning_count": len(edge_audit["diagnostics"]),
            "sample": diagnostics[:20],
        },
        "chunks_sha256": chunk_store["sha256"],
        "edge_chunks_sha256": edge_store["sha256"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)
    hot_bytes = path_bytes(args.hot_root)
    if hot_bytes > args.hot_budget_bytes:
        raise AstChunkError(
            "PY_AST_CHUNK_HOT_BUDGET_EXCEEDED",
            f"AST chunk hot staging used {hot_bytes} bytes, over budget {args.hot_budget_bytes}",
            "Reduce hot staging volume or increase explicit task budget with capacity evidence.",
            {"hot_root": str(args.hot_root), "actual_bytes": hot_bytes, "budget_bytes": args.hot_budget_bytes},
        )
    after_state = {
        "checked_at": utc_now(),
        "chunk_count": count_jsonl(chunks_path),
        "edge_chunk_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "chunks": file_state(chunks_path),
        "edge_chunks": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/zfs/archive")]),
        "hot_bytes": hot_bytes,
        "chunk_root_bytes": path_bytes(args.chunk_root),
        "fsv_root_bytes": path_bytes(args.fsv_root),
    }
    fsv = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 193,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py ast-chunks",
            "parse_record_count": len(parse_records),
            "chunkable_parse_record_count": len(parsed_records),
            "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false",
        },
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": [
            {"case": "nested_function_class", "pass": edge_audit["after_state"]["observed"]["nested_parented"], "observed": edge_audit["after_state"]["observed"]},
            {"case": "decorator_heavy_function", "pass": edge_audit["after_state"]["observed"]["fixture_chunk"], "observed": edge_audit["after_state"]["observed"]},
            {"case": "imports_or_comments_only", "pass": edge_audit["after_state"]["observed"]["imports_only"] == 2 and edge_audit["after_state"]["observed"]["comments_only"], "observed": edge_audit["after_state"]["observed"]},
        ],
        "summary": summary,
        "diagnostics": manifest["diagnostics"],
        "passes": True,
    }
    if not all(case["pass"] for case in fsv["edge_cases"]):
        raise AstChunkError(
            "PY_AST_CHUNK_EDGE_FSV_FAILED",
            "AST chunk edge-case readback failed",
            "Fix edge chunk behavior before closing TASK-PY-DATA-010.",
            {"edge_cases": fsv["edge_cases"]},
        )
    write_json_checked(fsv_path, fsv)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": fsv["trigger"],
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": fsv["edge_cases"],
        "summary": summary,
        "diagnostics": manifest["diagnostics"],
        "chunk_store": chunk_store,
        "edge_store": edge_store,
        "manifest_sha256": sha256_file(manifest_path),
        "fsv_sha256": sha256_file(fsv_path),
        "passes": True,
    }
