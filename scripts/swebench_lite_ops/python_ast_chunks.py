from __future__ import annotations

import ast
import hashlib
import json
import warnings
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file
from .oracle_events import (
    canonical_json,
    sha256_prefixed_text,
)
from .python_ast_parse import detect_and_decode, line_byte_offset
from .python_secret_redaction import SecretRedactionError, redact_text
from .source_readback import read_full_source_bytes


SCHEMA_VERSION = 2
TASK_ID = "TASK-PY-DATA-010"
PRODUCER = "contextgraph.swebench_lite_ops.python_ast_chunks"
DATACONTENTTYPE = "application/json"
CHUNK_KINDS = [
    "module_empty",
    "module_comment_only",
    "import",
    "module_statement",
    "function",
    "class",
    "test_function",
    "test_class",
    "fixture",
    "assertion",
    "syntax_error_file",
]


class AstChunkError(SwebenchOpsError):
    pass


def chunk_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    nullable_int = {"type": ["integer", "null"]}
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    redaction_event = {
        "type": "object",
        "properties": {
            "schema_version": {"const": 1},
            "task_id": {"const": "TASK-PY-G-059"},
            "artifact_kind": {"const": "python_secret_redaction_event"},
            "chunk_id": sha,
            "source_sha256": sha,
            "redacted_source_sha256": sha,
            "event_index": {"type": "integer", "minimum": 0},
            "redaction_count": {"type": "integer", "minimum": 1},
            "pattern_id": {"type": "string", "minLength": 1},
            "description": {"type": "string", "minLength": 1},
            "byte_offset": {"type": "integer", "minimum": 0},
            "byte_length": {"type": "integer", "minimum": 1},
            "redacted_length": {"type": "integer", "minimum": 1},
        },
        "required": [
            "artifact_kind",
            "byte_length",
            "byte_offset",
            "chunk_id",
            "description",
            "event_index",
            "pattern_id",
            "redacted_length",
            "redacted_source_sha256",
            "redaction_count",
            "schema_version",
            "source_sha256",
            "task_id",
        ],
        "additionalProperties": False,
    }
    optional_props = {
        "legacy_provenance_path": nullable_string,
        "legacy_dataschema_path": nullable_string,
    }
    props = {
        "schema_version": {"const": SCHEMA_VERSION},
        "chunk_id": sha,
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
        "workspace_tree_hash": {"type": "string", "pattern": "^[0-9a-f]{40}$"},
        "relative_path": {"type": "string", "minLength": 1},
        "absolute_path": {"type": "string", "minLength": 1},
        "file_sha256": sha,
        "encoding": {"type": "string", "minLength": 1},
        "chunk_kind": {"enum": CHUNK_KINDS},
        "node_kind": {"type": "string", "minLength": 1},
        "name": nullable_string,
        "qualname": nullable_string,
        "parent_chunk_id": nullable_string,
        "depth": {"type": "integer", "minimum": 0},
        "line_start": {"type": "integer", "minimum": 1},
        "line_end": {"type": "integer", "minimum": 1},
        "col_start": {"type": "integer", "minimum": 0},
        "col_end": {"type": "integer", "minimum": 0},
        "byte_start": {"type": "integer", "minimum": 0},
        "byte_end": {"type": "integer", "minimum": 0},
        "source_sha256": sha,
        "source_text": {"type": "string"},
        "redacted_source_text": {"type": "string"},
        "redacted_source_sha256": sha,
        "redaction_count": {"type": "integer", "minimum": 0},
        "redaction_events": {"type": "array", "items": redaction_event},
        "decorators": {"type": "array", "items": {"type": "string"}},
        "source_slice_verified": {"const": True},
        **optional_props,
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python AST chunk v1",
        "type": "object",
        "properties": props,
        "required": sorted(k for k in props if k not in optional_props),
        "additionalProperties": False,
    }


def load_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    ensure_file(path, code, "required JSONL artifact")
    rows = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise AstChunkError(
                code,
                f"invalid JSONL line {line_number} at {path}",
                "Regenerate upstream parse records before chunking.",
                {"path": str(path), "line_number": line_number, "error": str(exc)},
            ) from exc
    return rows


def decode_source(record: dict[str, Any]) -> tuple[bytes, str, str]:
    path, raw = read_full_source_bytes(
        record,
        legacy_code="PY_AST_CHUNK_LEGACY_SOURCE_PATH",
        missing_code="PY_AST_CHUNK_SOURCE_FILE_MISSING",
        hash_mismatch_code="PY_AST_CHUNK_SOURCE_HASH_MISMATCH",
        remediation="Regenerate AST parse records from the current workspace bytes.",
    )
    encoding, decoded, error = detect_and_decode(raw)
    if error is not None or decoded is None or encoding is None:
        raise AstChunkError(
            "PY_AST_CHUNK_SOURCE_DECODE_FAILED",
            f"parsed record source no longer decodes: {path}",
            "Regenerate AST parse records from stable source bytes.",
            {"path": str(path), "error": error},
        )
    return raw, decoded, encoding


def char_index_from_utf8_col(line: str, col: int) -> int:
    prefix = line.encode("utf-8")[:col]
    return len(prefix.decode("utf-8", errors="ignore"))


def source_span(raw: bytes, decoded: str, encoding: str, node: ast.AST) -> dict[str, Any]:
    lineno = getattr(node, "lineno", None)
    col = getattr(node, "col_offset", None)
    end_lineno = getattr(node, "end_lineno", None)
    end_col = getattr(node, "end_col_offset", None)
    if None in (lineno, col, end_lineno, end_col):
        raise AstChunkError(
            "PY_AST_CHUNK_NODE_SPAN_MISSING",
            "AST node lacks complete source span",
            "Only chunk nodes with explicit source spans.",
            {"node_kind": type(node).__name__},
        )
    lines = decoded.splitlines(keepends=True)
    start_char = char_index_from_utf8_col(lines[lineno - 1], col)
    end_char = char_index_from_utf8_col(lines[end_lineno - 1], end_col)
    if lineno == end_lineno:
        text = lines[lineno - 1][start_char:end_char]
    else:
        text = lines[lineno - 1][start_char:] + "".join(lines[lineno:end_lineno - 1]) + lines[end_lineno - 1][:end_char]
    byte_start = line_byte_offset(decoded, lineno, start_char + 1, encoding)
    byte_end = line_byte_offset(decoded, end_lineno, end_char + 1, encoding)
    if byte_start is None or byte_end is None:
        raise AstChunkError(
            "PY_AST_CHUNK_BYTE_SPAN_FAILED",
            "could not map AST span to byte offsets",
            "Fix span conversion before emitting chunks.",
            {"lineno": lineno, "end_lineno": end_lineno},
        )
    source_bytes = raw[byte_start:byte_end]
    if source_bytes != text.encode(encoding):
        raise AstChunkError(
            "PY_AST_CHUNK_SOURCE_SLICE_MISMATCH",
            "chunk source slice does not re-read to original bytes",
            "Reject this chunk and fix span conversion before using chunks as reward data.",
            {"byte_start": byte_start, "byte_end": byte_end, "node_kind": type(node).__name__},
        )
    return {
        "line_start": lineno,
        "line_end": end_lineno,
        "col_start": col,
        "col_end": end_col,
        "byte_start": byte_start,
        "byte_end": byte_end,
        "source_text": text,
        "source_sha256": "sha256:" + hashlib.sha256(source_bytes).hexdigest(),
    }


def decorator_names(node: ast.AST) -> list[str]:
    names = []
    for dec in getattr(node, "decorator_list", []):
        names.append(ast.unparse(dec))
    return names


def semantic_kind(node: ast.AST, relative_path: str) -> str | None:
    if isinstance(node, ast.Assert):
        return "assertion"
    if isinstance(node, (ast.Import, ast.ImportFrom)):
        return "import"
    if isinstance(node, ast.ClassDef):
        return "test_class" if node.name.startswith("Test") or "/test" in relative_path else "class"
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
        decorators = decorator_names(node)
        if any("pytest.fixture" in dec or dec.endswith("fixture") or dec == "fixture" for dec in decorators):
            return "fixture"
        if node.name.startswith("test_") or "/test" in relative_path or relative_path.startswith("test"):
            return "test_function"
        return "function"
    if isinstance(node, ast.stmt):
        return "module_statement"
    return None


def chunk_identity(record: dict[str, Any], chunk_kind: str, node_kind: str, span: dict[str, Any], name: str | None) -> str:
    return sha256_prefixed_text(
        canonical_json(
            {
                "task_instance_id": record["task_instance_id"],
                "mutation_category": record["mutation_category"],
                "file_sha256": record["file_sha256"],
                "chunk_kind": chunk_kind,
                "node_kind": node_kind,
                "name": name,
                "byte_start": span["byte_start"],
                "byte_end": span["byte_end"],
            }
        )
    )


def build_chunk(
    record: dict[str, Any],
    schema_path: Path,
    now: str,
    node: ast.AST,
    chunk_kind: str,
    span: dict[str, Any],
    parent_chunk_id: str | None,
    depth: int,
    qualname: str | None,
    encoding: str,
    node_kind_override: str | None = None,
) -> dict[str, Any]:
    name = getattr(node, "name", None)
    node_kind = node_kind_override or type(node).__name__
    chunk_id = chunk_identity(record, chunk_kind, node_kind, span, name)
    redaction = redaction_payload(chunk_id, span["source_sha256"], span["source_text"])
    return {
        "schema_version": SCHEMA_VERSION,
        "chunk_id": chunk_id,
        "source": f"contextgraph://python-ast-chunks/{record['task_instance_id']}/{record['workspace_state']}",
        "subject": f"{record['task_instance_id']}#{record['workspace_state']}#{record['relative_path']}#{chunk_id}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "task_instance_id": record["task_instance_id"],
        "workspace_state": record["workspace_state"],
        "mutation_category": record["mutation_category"],
        "repo": record["repo"],
        "workspace_tree_hash": record["workspace_tree_hash"],
        "relative_path": record["relative_path"],
        "absolute_path": record["absolute_path"],
        "file_sha256": record["file_sha256"],
        "encoding": encoding,
        "chunk_kind": chunk_kind,
        "node_kind": node_kind,
        "name": name,
        "qualname": qualname,
        "parent_chunk_id": parent_chunk_id,
        "depth": depth,
        "line_start": span["line_start"],
        "line_end": span["line_end"],
        "col_start": span["col_start"],
        "col_end": span["col_end"],
        "byte_start": span["byte_start"],
        "byte_end": span["byte_end"],
        "source_sha256": span["source_sha256"],
        "source_text": span["source_text"],
        "redacted_source_text": redaction["redacted_source_text"],
        "redacted_source_sha256": redaction["redacted_source_sha256"],
        "redaction_count": redaction["redaction_count"],
        "redaction_events": redaction["redaction_events"],
        "decorators": decorator_names(node),
        "source_slice_verified": True,
    }


def redaction_payload(chunk_id: str, source_sha256: str, source_text: str) -> dict[str, Any]:
    try:
        redacted = redact_text(source_text)
    except SecretRedactionError as exc:
        raise AstChunkError(
            exc.code,
            f"secret redaction failed for chunk {chunk_id}: {exc.message}",
            "Reject the chunk from embedding and fix the redaction pipeline before emitting AST chunks.",
            {"chunk_id": chunk_id, "source_sha256": source_sha256, **exc.details},
        ) from exc

    redacted_sha = "sha256:" + hashlib.sha256(redacted.content.encode("utf-8")).hexdigest()
    events = []
    redaction_count = redacted.redaction_count
    for index, event in enumerate(redacted.events):
        row = event.to_json()
        row.update(
            {
                "schema_version": 1,
                "task_id": "TASK-PY-G-059",
                "artifact_kind": "python_secret_redaction_event",
                "chunk_id": chunk_id,
                "source_sha256": source_sha256,
                "redacted_source_sha256": redacted_sha,
                "event_index": index,
                "redaction_count": redaction_count,
            }
        )
        events.append(row)
    return {
        "redacted_source_text": redacted.content,
        "redacted_source_sha256": redacted_sha,
        "redaction_count": redaction_count,
        "redaction_events": events,
    }


def module_empty_chunk(record: dict[str, Any], schema_path: Path, raw: bytes, decoded: str, encoding: str, now: str) -> dict[str, Any]:
    kind = "module_empty" if len(raw) == 0 else "module_comment_only"
    lines = decoded.splitlines() or [""]
    span = {
        "line_start": 1,
        "line_end": max(1, len(lines)),
        "col_start": 0,
        "col_end": len(lines[-1]),
        "byte_start": 0,
        "byte_end": len(raw),
        "source_text": decoded,
        "source_sha256": "sha256:" + hashlib.sha256(raw).hexdigest(),
    }
    fake = ast.Pass(lineno=1, col_offset=0, end_lineno=span["line_end"], end_col_offset=span["col_end"])
    return build_chunk(record, schema_path, now, fake, kind, span, None, 0, None, encoding, node_kind_override="Module")


def syntax_error_chunk(record: dict[str, Any], schema_path: Path, raw: bytes, decoded: str, encoding: str, now: str) -> dict[str, Any]:
    lines = decoded.splitlines(keepends=True) or [""]
    diagnostic = record.get("syntax_error") or {}
    lineno = diagnostic.get("lineno") if isinstance(diagnostic, dict) else None
    center = lineno if isinstance(lineno, int) and lineno >= 1 else 1
    start_line = max(1, center - 20)
    end_line = min(len(lines), center + 20)
    prefix = "".join(lines[: start_line - 1])
    text = "".join(lines[start_line - 1 : end_line])
    byte_start = len(prefix.encode(encoding))
    source_bytes = text.encode(encoding)
    byte_end = byte_start + len(source_bytes)
    if raw[byte_start:byte_end] != source_bytes:
        raise AstChunkError(
            "PY_AST_CHUNK_SYNTAX_ERROR_SLICE_MISMATCH",
            "syntax-error chunk source slice does not re-read to original bytes",
            "Fix syntax-error span conversion before using compile-error chunks as reward data.",
            {"relative_path": record["relative_path"], "line_start": start_line, "line_end": end_line},
        )
    last_line = lines[end_line - 1] if end_line >= 1 else ""
    span = {
        "line_start": start_line,
        "line_end": max(start_line, end_line),
        "col_start": 0,
        "col_end": len(last_line.rstrip("\r\n")),
        "byte_start": byte_start,
        "byte_end": byte_end,
        "source_text": text,
        "source_sha256": "sha256:" + hashlib.sha256(source_bytes).hexdigest(),
    }
    fake = ast.Pass(lineno=start_line, col_offset=0, end_lineno=span["line_end"], end_col_offset=span["col_end"])
    return build_chunk(
        record,
        schema_path,
        now,
        fake,
        "syntax_error_file",
        span,
        None,
        0,
        None,
        encoding,
        node_kind_override="SyntaxError",
    )


def parse_decoded(record: dict[str, Any], decoded: str) -> tuple[ast.Module, list[dict[str, Any]]]:
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always", SyntaxWarning)
        tree = ast.parse(decoded, filename=record["absolute_path"], type_comments=True)
    diagnostics = [
        {
            "code": "PY_AST_CHUNK_REPARSE_SYNTAXWARNING",
            "message": str(warning.message),
            "category": warning.category.__name__,
            "relative_path": record["relative_path"],
            "absolute_path": record["absolute_path"],
            "lineno": warning.lineno,
        }
        for warning in caught
        if issubclass(warning.category, SyntaxWarning)
    ]
    return tree, diagnostics


def chunks_for_record(record: dict[str, Any], schema_path: Path, now: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if record["parse_status"] not in {"parsed", "empty", "syntax_error"}:
        return [], []
    raw, decoded, encoding = decode_source(record)
    if record["parse_status"] == "syntax_error":
        return [syntax_error_chunk(record, schema_path, raw, decoded, encoding, now)], []
    tree, diagnostics = parse_decoded(record, decoded)
    if record["parse_status"] == "empty":
        return [module_empty_chunk(record, schema_path, raw, decoded, encoding, now)], diagnostics

    parent: dict[ast.AST, ast.AST | None] = {tree: None}
    for node in ast.walk(tree):
        for child in ast.iter_child_nodes(node):
            parent[child] = node

    chunks: list[dict[str, Any]] = []
    node_to_chunk: dict[ast.AST, str] = {}
    qualname_by_node: dict[ast.AST, str] = {}
    ordered = sorted(
        [node for node in ast.walk(tree) if semantic_kind(node, record["relative_path"]) is not None],
        key=lambda node: (getattr(node, "lineno", 0), getattr(node, "col_offset", 0), type(node).__name__),
    )
    for node in ordered:
        kind = semantic_kind(node, record["relative_path"])
        if kind is None:
            continue
        if kind == "module_statement" and not isinstance(parent.get(node), ast.Module):
            continue
        parent_node = parent.get(node)
        parent_chunk_id = None
        depth = 0
        qual_parts: list[str] = []
        while parent_node is not None:
            if parent_node in node_to_chunk:
                parent_chunk_id = node_to_chunk[parent_node]
                depth += 1
                if getattr(parent_node, "name", None):
                    qual_parts.append(parent_node.name)
            parent_node = parent.get(parent_node)
        name = getattr(node, "name", None)
        qualname = ".".join(reversed(qual_parts + ([name] if name else []))) or name
        span = source_span(raw, decoded, encoding, node)
        chunk = build_chunk(record, schema_path, now, node, kind, span, parent_chunk_id, depth, qualname, encoding)
        chunks.append(chunk)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            node_to_chunk[node] = chunk["chunk_id"]
            qualname_by_node[node] = qualname or ""
    return chunks, diagnostics
