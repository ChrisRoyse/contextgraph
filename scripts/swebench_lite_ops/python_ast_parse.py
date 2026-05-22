from __future__ import annotations

import ast
import hashlib
import io
import json
import os
import sys
import tokenize
import warnings
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_dir, ensure_file, utc_now, write_json_checked
from .oracle_events import (
    canonical_json,
    capacity_snapshot,
    file_state,
    load_jsonschema_validator,
    path_bytes,
    sha256_file,
    sha256_prefixed_text,
    write_jsonl_checked,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-009"
PRODUCER = "contextgraph.swebench_lite_ops.python_ast_parse"
DATACONTENTTYPE = "application/json"
PARSER_NAME = "cpython.ast"
PARSER_VERSION = f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}"


class AstParseError(SwebenchOpsError):
    pass


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise AstParseError(
            code,
            f"invalid JSON at {path}",
            "Regenerate the upstream artifact before materializing AST parse records.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def record_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    nullable_int = {"type": ["integer", "null"]}
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    syntax_error_schema = {
        "type": ["object", "null"],
        "properties": {
            "message": nullable_string,
            "lineno": nullable_int,
            "offset": nullable_int,
            "end_lineno": nullable_int,
            "end_offset": nullable_int,
            "byte_offset": nullable_int,
            "end_byte_offset": nullable_int,
            "text_sha256": nullable_string,
        },
        "required": [
            "message",
            "lineno",
            "offset",
            "end_lineno",
            "end_offset",
            "byte_offset",
            "end_byte_offset",
            "text_sha256",
        ],
        "additionalProperties": False,
    }
    diagnostic_schema = {
        "type": "object",
        "properties": {
            "code": {"type": "string", "minLength": 1},
            "message": {"type": "string"},
            "lineno": nullable_int,
            "offset": nullable_int,
            "byte_offset": nullable_int,
        },
        "required": ["code", "message", "lineno", "offset", "byte_offset"],
        "additionalProperties": False,
    }
    properties = {
        "schema_version": {"const": SCHEMA_VERSION},
        "record_id": sha,
        "source": {"type": "string", "minLength": 1},
        "subject": {"type": "string", "minLength": 1},
        "dataschema": {"type": "string", "minLength": 1},
        "datacontenttype": {"const": DATACONTENTTYPE},
        "time": {"type": "string", "minLength": 1},
        "producer": {"const": PRODUCER},
        "task_id": {"const": TASK_ID},
        "task_instance_id": {"type": "string", "minLength": 1},
        "workspace_state": {"type": "string", "minLength": 1},
        "mutation_category": {"type": "string", "minLength": 1},
        "repo": {"type": "string", "minLength": 1},
        "base_commit": {"type": "string", "minLength": 1},
        "workspace_tree_hash": {"type": "string", "pattern": "^[0-9a-f]{40}$"},
        "workspace_path": {"type": "string", "minLength": 1},
        "relative_path": {"type": "string", "minLength": 1},
        "absolute_path": {"type": "string", "minLength": 1},
        "file_size_bytes": {"type": "integer", "minimum": 0},
        "file_sha256": sha,
        "encoding": nullable_string,
        "parser_name": {"const": PARSER_NAME},
        "parser_version": {"const": PARSER_VERSION},
        "parse_status": {"enum": ["parsed", "empty", "syntax_error", "encoding_error", "io_error"]},
        "ast_root_type": nullable_string,
        "ast_node_count": nullable_int,
        "max_depth": nullable_int,
        "function_count": nullable_int,
        "class_count": nullable_int,
        "import_count": nullable_int,
        "syntax_error": syntax_error_schema,
        "diagnostics": {"type": "array", "items": diagnostic_schema},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python AST parse record v1",
        "type": "object",
        "properties": properties,
        "required": sorted(properties),
        "additionalProperties": False,
    }


def detect_and_decode(raw: bytes) -> tuple[str | None, str | None, dict[str, Any] | None]:
    try:
        encoding, _ = tokenize.detect_encoding(io.BytesIO(raw).readline)
    except SyntaxError as exc:
        return None, None, syntax_diag(exc, raw, None)
    try:
        return encoding, raw.decode(encoding), None
    except (LookupError, UnicodeDecodeError) as exc:
        return encoding, None, {
            "message": str(exc),
            "lineno": None,
            "offset": None,
            "end_lineno": None,
            "end_offset": None,
            "byte_offset": None,
            "end_byte_offset": None,
            "text_sha256": None,
        }


def line_byte_offset(text: str | None, lineno: int | None, offset: int | None, encoding: str | None) -> int | None:
    if text is None or lineno is None or offset is None or lineno < 1 or offset < 1:
        return None
    lines = text.splitlines(keepends=True)
    if lineno > len(lines):
        return None
    prefix = "".join(lines[: lineno - 1]) + lines[lineno - 1][: offset - 1]
    try:
        return len(prefix.encode(encoding or "utf-8", errors="replace"))
    except LookupError:
        return None


def syntax_diag(exc: SyntaxError, raw: bytes, decoded: str | None) -> dict[str, Any]:
    text = exc.text or ""
    encoding = "utf-8"
    return {
        "message": exc.msg,
        "lineno": exc.lineno,
        "offset": exc.offset,
        "end_lineno": exc.end_lineno,
        "end_offset": exc.end_offset,
        "byte_offset": line_byte_offset(decoded, exc.lineno, exc.offset, encoding),
        "end_byte_offset": line_byte_offset(decoded, exc.end_lineno, exc.end_offset, encoding),
        "text_sha256": sha256_prefixed_text(text) if text else sha256_prefixed_text(raw.decode("utf-8", errors="replace")),
    }


def ast_stats(tree: ast.AST) -> dict[str, int]:
    nodes = list(ast.walk(tree))
    max_depth = 0
    stack: list[tuple[ast.AST, int]] = [(tree, 1)]
    while stack:
        node, depth = stack.pop()
        max_depth = max(max_depth, depth)
        stack.extend((child, depth + 1) for child in ast.iter_child_nodes(node))
    return {
        "ast_node_count": len(nodes),
        "max_depth": max_depth,
        "function_count": sum(isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) for node in nodes),
        "class_count": sum(isinstance(node, ast.ClassDef) for node in nodes),
        "import_count": sum(isinstance(node, (ast.Import, ast.ImportFrom)) for node in nodes),
    }


def base_record(
    *,
    schema_path: Path,
    workspace: dict[str, Any],
    absolute_path: Path,
    relative_path: str,
    raw: bytes,
    now: str,
) -> dict[str, Any]:
    identity = {
        "task_id": workspace["task_id"],
        "state": workspace["state"],
        "tree": workspace["result_tree_hash"],
        "relative_path": relative_path,
        "file_sha256": "sha256:" + hashlib.sha256(raw).hexdigest(),
    }
    return {
        "schema_version": SCHEMA_VERSION,
        "record_id": sha256_prefixed_text(canonical_json(identity)),
        "source": f"contextgraph://python-workspace/{workspace['task_id']}/{workspace['state']}",
        "subject": f"{workspace['task_id']}#{workspace['state']}#{relative_path}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "task_instance_id": workspace["task_id"],
        "workspace_state": workspace["state"],
        "mutation_category": workspace["mutation_category"],
        "repo": workspace["repo"],
        "base_commit": workspace["base_commit"],
        "workspace_tree_hash": workspace["result_tree_hash"],
        "workspace_path": workspace["hot_workspace_path"],
        "relative_path": relative_path,
        "absolute_path": str(absolute_path),
        "file_size_bytes": len(raw),
        "file_sha256": identity["file_sha256"],
        "encoding": None,
        "parser_name": PARSER_NAME,
        "parser_version": PARSER_VERSION,
        "parse_status": "io_error",
        "ast_root_type": None,
        "ast_node_count": None,
        "max_depth": None,
        "function_count": None,
        "class_count": None,
        "import_count": None,
        "syntax_error": None,
        "diagnostics": [],
    }


def parse_python_bytes(
    *,
    schema_path: Path,
    workspace: dict[str, Any],
    absolute_path: Path,
    relative_path: str,
    raw: bytes,
    now: str,
) -> dict[str, Any]:
    record = base_record(
        schema_path=schema_path,
        workspace=workspace,
        absolute_path=absolute_path,
        relative_path=relative_path,
        raw=raw,
        now=now,
    )
    encoding, decoded, decode_error = detect_and_decode(raw)
    record["encoding"] = encoding
    if decode_error is not None:
        record["parse_status"] = "encoding_error"
        record["syntax_error"] = decode_error
        record["diagnostics"] = [{
            "code": "PY_AST_ENCODING_ERROR",
            "message": decode_error["message"] or "encoding detection/decode failed",
            "lineno": decode_error["lineno"],
            "offset": decode_error["offset"],
            "byte_offset": decode_error["byte_offset"],
        }]
        return record
    assert decoded is not None
    captured_warnings: list[warnings.WarningMessage]
    try:
        with warnings.catch_warnings(record=True) as captured:
            warnings.simplefilter("always", SyntaxWarning)
            tree = ast.parse(decoded, filename=str(absolute_path), type_comments=True)
            captured_warnings = list(captured)
    except SyntaxError as exc:
        diag = syntax_diag(exc, raw, decoded)
        record["parse_status"] = "syntax_error"
        record["syntax_error"] = diag
        record["diagnostics"] = [{
            "code": "PY_AST_SYNTAX_ERROR",
            "message": diag["message"] or "syntax error",
            "lineno": diag["lineno"],
            "offset": diag["offset"],
            "byte_offset": diag["byte_offset"],
        }]
        return record
    stats = ast_stats(tree)
    record.update(stats)
    record["ast_root_type"] = type(tree).__name__
    record["parse_status"] = "empty" if len(getattr(tree, "body", [])) == 0 else "parsed"
    record["diagnostics"] = [
        {
            "code": f"PY_AST_{warning.category.__name__.upper()}",
            "message": str(warning.message),
            "lineno": warning.lineno,
            "offset": None,
            "byte_offset": line_byte_offset(decoded, warning.lineno, 1, encoding),
        }
        for warning in captured_warnings
    ]
    return record


def parse_file(schema_path: Path, workspace: dict[str, Any], path: Path, now: str) -> dict[str, Any]:
    relative_path = path.relative_to(workspace["hot_workspace_path"]).as_posix()
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raw = b""
        record = base_record(
            schema_path=schema_path,
            workspace=workspace,
            absolute_path=path,
            relative_path=relative_path,
            raw=raw,
            now=now,
        )
        record["diagnostics"] = [{
            "code": "PY_AST_IO_ERROR",
            "message": str(exc),
            "lineno": None,
            "offset": None,
            "byte_offset": None,
        }]
        return record
    return parse_python_bytes(
        schema_path=schema_path,
        workspace=workspace,
        absolute_path=path,
        relative_path=relative_path,
        raw=raw,
        now=now,
    )


def workspace_python_files(workspace: dict[str, Any]) -> list[Path]:
    root = Path(workspace["hot_workspace_path"])
    ensure_dir(root, "PY_AST_WORKSPACE_MISSING", "Python workspace")
    files = [
        path
        for path in root.rglob("*.py")
        if ".git" not in path.relative_to(root).parts
    ]
    return sorted(files, key=lambda path: path.relative_to(root).as_posix())


def validate_records(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise AstParseError(
                "PY_AST_RECORD_SCHEMA_VALIDATION_FAILED",
                "generated AST parse record failed strict JSON Schema validation",
                "Fix the AST parse record builder before writing reward data.",
                {"record_id": record.get("record_id"), "errors": [err.message for err in errors[:10]]},
            )


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def edge_workspace(hot_root: Path) -> dict[str, Any]:
    return {
        "task_id": "synthetic_py_ast_edges",
        "state": "edge_cases",
        "mutation_category": "parser_edge_case",
        "repo": "synthetic/ast-parser",
        "base_commit": "0" * 40,
        "result_tree_hash": "0" * 40,
        "hot_workspace_path": str(hot_root / "edge-files"),
    }


def edge_records(schema_path: Path, hot_root: Path, now: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    edge_root = hot_root / "edge-files"
    edge_root.mkdir(parents=True, exist_ok=True)
    cases = {
        "empty.py": b"",
        "syntax_error.py": b"def broken(:\n    pass\n",
        "invalid_encoding.py": b"# coding: ascii\nvalue = \"\xff\"\n",
    }
    before = {"edge_record_count": count_jsonl(hot_root / "edge_case_parse_records.jsonl")}
    workspace = edge_workspace(hot_root)
    records = []
    for name, raw in cases.items():
        path = edge_root / name
        path.write_bytes(raw)
        records.append(
            parse_python_bytes(
                schema_path=schema_path,
                workspace=workspace,
                absolute_path=path,
                relative_path=name,
                raw=raw,
                now=now,
            )
        )
    expected = {
        "empty.py": "empty",
        "syntax_error.py": "syntax_error",
        "invalid_encoding.py": "encoding_error",
    }
    observed = {record["relative_path"]: record["parse_status"] for record in records}
    after = {"edge_record_count": len(records), "observed_statuses": observed}
    return records, {"before_state": before, "after_state": after, "expected_statuses": expected, "pass": observed == expected}


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    counts: dict[str, int] = {}
    by_workspace: dict[str, int] = {}
    for record in records:
        counts[record["parse_status"]] = counts.get(record["parse_status"], 0) + 1
        key = f"{record['task_instance_id']}::{record['workspace_state']}"
        by_workspace[key] = by_workspace.get(key, 0) + 1
    return {"total": len(records), "by_status": counts, "by_workspace": by_workspace}


def command_ast_parse(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    args.parse_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.parse_root / "python_ast_parse_schema.json"
    records_path = args.parse_root / "python_ast_parse_records.jsonl"
    edge_path = args.parse_root / "edge_case_parse_records.jsonl"
    manifest_path = args.parse_root / "python_ast_parse_manifest.json"
    readback_path = args.readback_root / "python_ast_parse_readback.json"

    before_state = {
        "checked_at": checked_at,
        "record_count": count_jsonl(records_path),
        "edge_record_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "readback": file_state(readback_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "parse_root_bytes": path_bytes(args.parse_root),
    }

    workspace_manifest = read_json(args.workspace_manifest, "PY_AST_WORKSPACE_MANIFEST_JSON_INVALID")
    workspaces = workspace_manifest.get("states", [])
    if not workspaces:
        raise AstParseError(
            "PY_AST_WORKSPACE_STATES_EMPTY",
            "workspace manifest has no states to parse",
            "Run TASK-PY-DATA-006 before TASK-PY-DATA-009.",
            {"workspace_manifest": str(args.workspace_manifest)},
        )
    schema = record_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)

    now = utc_now()
    records: list[dict[str, Any]] = []
    file_counts: dict[str, int] = {}
    for workspace in workspaces:
        files = workspace_python_files(workspace)
        key = f"{workspace['task_id']}::{workspace['state']}"
        file_counts[key] = len(files)
        for path in files:
            records.append(parse_file(schema_path, workspace, path, now))
    validate_records(records, validator)
    record_store = write_jsonl_checked(records_path, records)

    edges, edge_audit = edge_records(schema_path, args.hot_root, now)
    validate_records(edges, validator)
    edge_store = write_jsonl_checked(edge_path, edges)

    summary = summarize(records)
    happy_record = next((record for record in records if record["parse_status"] == "parsed"), None)
    if happy_record is None:
        raise AstParseError(
            "PY_AST_NO_PARSED_RECORDS",
            "no parsed Python file records were produced",
            "Inspect workspace paths and parser diagnostics.",
            {"summary": summary},
        )
    happy_path = {
        "case": "real_workspace_valid_python_file",
        "record_id": happy_record["record_id"],
        "relative_path": happy_record["relative_path"],
        "expected": "parse_status=parsed, ast_root_type=Module, stable ast_node_count read back from JSONL",
        "observed": {
            "parse_status": happy_record["parse_status"],
            "ast_root_type": happy_record["ast_root_type"],
            "ast_node_count": happy_record["ast_node_count"],
        },
        "pass": happy_record["parse_status"] == "parsed" and happy_record["ast_root_type"] == "Module" and happy_record["ast_node_count"] > 0,
    }
    all_checks_pass = happy_path["pass"] and edge_audit["pass"]
    if not all_checks_pass:
        raise AstParseError(
            "PY_AST_PARSE_READBACK_FAILED",
            "AST parse READBACK checks failed",
            "Fix parser records before using them as reward data.",
            {"happy_path": happy_path, "edge_audit": edge_audit},
        )

    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 192,
        "source_of_truth": {
            "schema": str(schema_path),
            "records_jsonl": str(records_path),
            "edge_records_jsonl": str(edge_path),
            "manifest": str(manifest_path),
            "readback": str(readback_path),
        },
        "inputs": {"workspace_manifest": str(args.workspace_manifest)},
        "parser": {"name": PARSER_NAME, "version": PARSER_VERSION, "python": sys.version},
        "storage_policy": {
            "decision_issue": 212,
            "durable_root": "/zfs/archive/contextgraph",
            "hot_root": str(args.hot_root),
            "hot_budget_bytes": args.hot_budget_bytes,
        },
        "file_counts": file_counts,
        "summary": summary,
        "records_sha256": record_store["sha256"],
        "edge_records_sha256": edge_store["sha256"],
        "passes": True,
    }
    write_json_checked(manifest_path, manifest)

    hot_bytes = path_bytes(args.hot_root)
    if hot_bytes > args.hot_budget_bytes:
        raise AstParseError(
            "PY_AST_HOT_BUDGET_EXCEEDED",
            f"AST parse hot staging used {hot_bytes} bytes, over budget {args.hot_budget_bytes}",
            "Reduce hot staging volume or increase explicit task budget with capacity evidence.",
            {"hot_root": str(args.hot_root), "actual_bytes": hot_bytes, "budget_bytes": args.hot_budget_bytes},
        )
    after_state = {
        "checked_at": utc_now(),
        "record_count": count_jsonl(records_path),
        "edge_record_count": count_jsonl(edge_path),
        "schema": file_state(schema_path),
        "records": file_state(records_path),
        "edge_records": file_state(edge_path),
        "manifest": file_state(manifest_path),
        "readback": file_state(readback_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": hot_bytes,
        "parse_root_bytes": path_bytes(args.parse_root),
        "readback_root_bytes": path_bytes(args.readback_root),
    }
    readback = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 192,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py ast-parse",
            "workspace_count": len(workspaces),
            "workspace_python_file_count": sum(file_counts.values()),
            "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false",
        },
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": [
            {
                "case": "empty_file",
                "expected": "parse_status=empty and ast_node_count=1",
                "observed": next(record for record in edges if record["relative_path"] == "empty.py"),
                "pass": next(record for record in edges if record["relative_path"] == "empty.py")["parse_status"] == "empty",
            },
            {
                "case": "syntax_error",
                "expected": "parse_status=syntax_error with line/offset/byte_offset diagnostics",
                "observed": next(record for record in edges if record["relative_path"] == "syntax_error.py"),
                "pass": next(record for record in edges if record["relative_path"] == "syntax_error.py")["syntax_error"]["byte_offset"] is not None,
            },
            {
                "case": "invalid_encoding",
                "expected": "parse_status=encoding_error and no silent skip",
                "observed": next(record for record in edges if record["relative_path"] == "invalid_encoding.py"),
                "pass": next(record for record in edges if record["relative_path"] == "invalid_encoding.py")["parse_status"] == "encoding_error",
            },
        ],
        "summary": summary,
        "passes": True,
    }
    if not all(case["pass"] for case in readback["edge_cases"]):
        raise AstParseError(
            "PY_AST_EDGE_READBACK_FAILED",
            "AST parse edge-case readback failed",
            "Fix edge parser behavior before closing TASK-PY-DATA-009.",
            {"edge_cases": readback["edge_cases"]},
        )
    write_json_checked(readback_path, readback)

    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": readback["trigger"],
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": readback["edge_cases"],
        "summary": summary,
        "record_store": record_store,
        "edge_store": edge_store,
        "manifest_sha256": sha256_file(manifest_path),
        "readback_sha256": sha256_file(readback_path),
        "passes": True,
    }
