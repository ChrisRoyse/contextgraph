from __future__ import annotations

import ast
import hashlib
from pathlib import Path
from typing import Any

from .core import utc_now, write_json_checked
from .oracle_events import capacity_snapshot, file_state, load_jsonschema_validator, path_bytes, sha256_file, write_jsonl_checked
from .python_static_graph_core import (
    SCHEMA_VERSION,
    TASK_ID,
    StaticGraphError,
    build_static_records,
    graph_schema,
    load_jsonl,
    make_record,
)
from .source_readback import read_full_source_bytes


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def validate_records(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise StaticGraphError(
                "PY_STATIC_GRAPH_SCHEMA_VALIDATION_FAILED",
                "generated static graph record failed strict JSON Schema validation",
                "Fix graph record construction before writing reward data.",
                {"graph_id": record.get("graph_id"), "errors": [err.message for err in errors[:10]]},
            )


def synthetic_chunk(task: str, state: str, rel: str, kind: str, name: str | None, line_start: int, line_end: int) -> dict[str, Any]:
    seed = f"{task}:{state}:{rel}:{kind}:{name}:{line_start}:{line_end}".encode("utf-8")
    return {
        "task_instance_id": task,
        "workspace_state": state,
        "mutation_category": "static_graph_edge_case",
        "relative_path": rel,
        "chunk_id": "sha256:" + hashlib.sha256(seed).hexdigest(),
        "chunk_kind": kind,
        "qualname": name,
        "line_start": line_start,
        "line_end": line_end,
        "depth": 0,
    }


def parse_record(task: str, state: str, root: Path, rel: str, repo: str = "synthetic/static-graph") -> dict[str, Any]:
    path = root / rel
    raw = path.read_bytes()
    return {
        "parse_status": "parsed",
        "absolute_path": str(path),
        "relative_path": rel,
        "file_sha256": "sha256:" + hashlib.sha256(raw).hexdigest(),
        "task_instance_id": task,
        "workspace_state": state,
        "mutation_category": "static_graph_edge_case",
        "repo": repo,
        "workspace_tree_hash": hashlib.sha1(f"{task}:{state}".encode("utf-8")).hexdigest(),
    }


def synthetic_edge_records(schema_path: Path, hot_root: Path, now: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    edge_root = hot_root / "edge-static-graph"
    task = "synthetic_static_graph"
    state = "edge_cases"
    root = edge_root / task / state
    root.mkdir(parents=True, exist_ok=True)
    files = {
        "dynamic_import.py": "import importlib\nname = 'math'\nmodule = importlib.import_module(name)\n",
        "optional_dep.py": "try:\n    import definitely_missing_optional_abc123\nexcept ImportError:\n    definitely_missing_optional_abc123 = None\n",
        "a.py": "import b\nVALUE_A = 1\n",
        "b.py": "import a\nVALUE_B = 1\n",
        "test_fixture.py": "import pytest\n\n@pytest.fixture\ndef thing():\n    return 1\n\ndef test_uses_fixture(thing):\n    assert thing == 1\n",
    }
    for rel, text in files.items():
        (root / rel).parent.mkdir(parents=True, exist_ok=True)
        (root / rel).write_text(text, encoding="utf-8")
    parse_records = [parse_record(task, state, root, rel) for rel in files]
    chunks = []
    for record in parse_records:
        path, raw = read_full_source_bytes(
            record,
            legacy_code="PY_STATIC_GRAPH_SYNTHETIC_LEGACY_SOURCE_PATH",
            missing_code="PY_STATIC_GRAPH_SYNTHETIC_SOURCE_MISSING",
            hash_mismatch_code="PY_STATIC_GRAPH_SYNTHETIC_SOURCE_HASH_MISMATCH",
            remediation="Regenerate synthetic static-graph edge source files before building edge records.",
        )
        tree = ast.parse(raw.decode("utf-8"), filename=str(path))
        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                kind = "fixture" if any("fixture" in ast.unparse(dec) for dec in node.decorator_list) else "test_function" if node.name.startswith("test_") else "function"
                chunks.append(synthetic_chunk(task, state, record["relative_path"], kind, node.name, node.lineno, node.end_lineno or node.lineno))
            elif isinstance(node, ast.ClassDef):
                chunks.append(synthetic_chunk(task, state, record["relative_path"], "class", node.name, node.lineno, node.end_lineno or node.lineno))
            elif isinstance(node, (ast.Import, ast.ImportFrom)):
                chunks.append(synthetic_chunk(task, state, record["relative_path"], "import", None, node.lineno, node.end_lineno or node.lineno))
    records = build_static_records(parse_records, chunks, schema_path, now)
    observed = {
        "dynamic_import_uncertainty": any(r["edge_kind"] == "dynamic_import" and r["resolution_status"] == "dynamic_unknown" for r in records),
        "missing_optional_dependency": any(r["target_module"] == "definitely_missing_optional_abc123" and r["uncertainty_reason"] == "missing_optional_dependency" for r in records),
        "circular_import": any(r["edge_kind"] == "circular_import" and r["resolution_status"] == "cycle" for r in records),
        "fixture_dependency": any(r["record_kind"] == "test_dependency" and r["edge_kind"] == "uses_fixture" and r["target_name"] == "thing" for r in records),
    }
    return records, {"observed": observed, "pass": all(observed.values())}


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    by_kind: dict[str, int] = {}
    by_status: dict[str, int] = {}
    for record in records:
        by_kind[record["record_kind"]] = by_kind.get(record["record_kind"], 0) + 1
        by_status[record["resolution_status"]] = by_status.get(record["resolution_status"], 0) + 1
    return {"total": len(records), "by_kind": by_kind, "by_resolution_status": by_status}


def command_static_graph(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.graph_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.graph_root / "python_static_graph_schema.json"
    records_path = args.graph_root / "python_static_graph_records.jsonl"
    edge_path = args.graph_root / "edge_case_static_graph_records.jsonl"
    manifest_path = args.graph_root / "python_static_graph_manifest.json"
    fsv_path = args.fsv_root / "python_static_graph_fsv.json"
    before_state = {
        "checked_at": checked_at,
        "record_count": count_jsonl(records_path),
        "edge_record_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "fsv": file_state(fsv_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "graph_root_bytes": path_bytes(args.graph_root),
    }
    schema = graph_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)
    parse_records = load_jsonl(args.parse_records, "PY_STATIC_GRAPH_PARSE_RECORDS_INVALID")
    chunks = load_jsonl(args.chunks_jsonl, "PY_STATIC_GRAPH_CHUNKS_INVALID")
    now = utc_now()
    records = build_static_records(parse_records, chunks, schema_path, now)
    validate_records(records, validator)
    record_store = write_jsonl_checked(records_path, records)
    edge_records, edge_audit = synthetic_edge_records(schema_path, args.hot_root, now)
    validate_records(edge_records, validator)
    edge_store = write_jsonl_checked(edge_path, edge_records)
    summary = summarize(records)
    happy_path = {
        "known_import_present": any(r["record_kind"] == "import" and r["relative_path"] == "astropy/modeling/separable.py" and r["target_module"] == "numpy" for r in records),
        "known_call_present": any(r["record_kind"] == "call" and r["relative_path"] == "astropy/modeling/separable.py" and r["target_name"] == "_coord_matrix" for r in records),
        "real_records_link_chunks": any(r["record_kind"] in {"symbol", "call", "import"} and r["chunk_id"] for r in records),
    }
    if not all(happy_path.values()) or not edge_audit["pass"]:
        raise StaticGraphError("PY_STATIC_GRAPH_FSV_FAILED", "static graph FSV checks failed", "Fix extractor before using graph features.", {"happy_path": happy_path, "edge_audit": edge_audit})
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 195,
        "source_of_truth": {
            "schema": str(schema_path),
            "records_jsonl": str(records_path),
            "edge_records_jsonl": str(edge_path),
            "manifest": str(manifest_path),
            "fsv": str(fsv_path),
        },
        "inputs": {"parse_records": str(args.parse_records), "chunks_jsonl": str(args.chunks_jsonl)},
        "summary": summary,
        "records_sha256": record_store["sha256"],
        "edge_records_sha256": edge_store["sha256"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)
    after_state = {
        "checked_at": utc_now(),
        "record_count": count_jsonl(records_path),
        "edge_record_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "disk": capacity_snapshot([Path("/"), Path("/mnt/c"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "graph_root_bytes": path_bytes(args.graph_root),
    }
    fsv = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 195,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py static-graph", "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false"},
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_audit,
        "summary": summary,
        "passes": True,
    }
    write_json_checked(fsv_path, fsv)
    fsv_readback = {
        "fsv": file_state(fsv_path),
        "fsv_root_bytes": path_bytes(args.fsv_root),
    }
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": fsv["trigger"],
        "after_state": after_state,
        "post_write_readback": fsv_readback,
        "happy_path": happy_path,
        "edge_cases": edge_audit,
        "summary": summary,
        "record_store": record_store,
        "edge_store": edge_store,
        "manifest_sha256": sha256_file(manifest_path),
        "fsv_sha256": sha256_file(fsv_path),
        "passes": True,
    }
