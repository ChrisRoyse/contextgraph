from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file
from .oracle_events import (
    canonical_json,
    sha256_prefixed_text,
)


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-011"
PRODUCER = "contextgraph.swebench_lite_ops.python_diff_chunk_map"
DATACONTENTTYPE = "application/json"
HUNK_RE = re.compile(r"@@ -(?P<old_start>\d+)(?:,(?P<old_count>\d+))? \+(?P<new_start>\d+)(?:,(?P<new_count>\d+))? @@")


class DiffChunkMapError(SwebenchOpsError):
    pass


@dataclass(frozen=True)
class Hunk:
    old_path: str | None
    new_path: str | None
    index: int
    header: str
    old_start: int
    old_count: int
    new_start: int
    new_count: int
    lines: tuple[str, ...]
    old_changed: tuple[int, ...]
    new_changed: tuple[int, ...]
    old_expected: tuple[tuple[int, str], ...]
    new_expected: tuple[tuple[int, str], ...]


def mapping_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    nullable_int = {"type": ["integer", "null"]}
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    span = {
        "type": "object",
        "properties": {"start": nullable_int, "end": nullable_int},
        "required": ["start", "end"],
        "additionalProperties": False,
    }
    chunk_ref = {
        "type": "object",
        "properties": {
            "chunk_id": sha,
            "chunk_kind": {"type": "string", "minLength": 1},
            "name": nullable_string,
            "qualname": nullable_string,
            "line_start": {"type": "integer", "minimum": 1},
            "line_end": {"type": "integer", "minimum": 1},
        },
        "required": ["chunk_id", "chunk_kind", "name", "qualname", "line_start", "line_end"],
        "additionalProperties": False,
    }
    props = {
        "schema_version": {"const": SCHEMA_VERSION},
        "mapping_id": sha,
        "source": {"type": "string", "minLength": 1},
        "subject": {"type": "string", "minLength": 1},
        "dataschema": {"type": "string", "minLength": 1},
        "datacontenttype": {"const": DATACONTENTTYPE},
        "time": {"type": "string", "minLength": 1},
        "producer": {"const": PRODUCER},
        "task_id": {"const": TASK_ID},
        "task_instance_id": {"type": "string", "minLength": 1},
        "row_id": {"type": "string", "minLength": 1},
        "repo": {"type": "string", "minLength": 1},
        "mutation_category": {"type": "string", "minLength": 1},
        "workspace_state": {"type": "string", "minLength": 1},
        "workspace_tree_hash": {"type": "string", "pattern": "^[0-9a-f]{40}$"},
        "patch_label": {"enum": ["gold", "mutation", "test", "edge"]},
        "patch_path": {"type": "string", "minLength": 1},
        "patch_sha256": sha,
        "patch_text_sha256": sha,
        "file_old_path": nullable_string,
        "file_new_path": nullable_string,
        "hunk_index": {"type": "integer", "minimum": 0},
        "hunk_header": {"type": "string", "minLength": 1},
        "hunk_sha256": sha,
        "old_line_span": span,
        "new_line_span": span,
        "old_changed_lines": {"type": "array", "items": {"type": "integer", "minimum": 1}},
        "new_changed_lines": {"type": "array", "items": {"type": "integer", "minimum": 1}},
        "change_kind": {"enum": ["add", "delete", "modify"]},
        "target_side": {"enum": ["old", "new"]},
        "old_chunks": {"type": "array", "items": chunk_ref},
        "new_chunks": {"type": "array", "items": chunk_ref},
        "primary_chunk_ids": {"type": "array", "items": sha, "minItems": 1},
        "nearest_chunk_id": sha,
        "nearest_symbol": nullable_string,
        "related_production_chunk_ids": {"type": "array", "items": sha},
        "source_readback_verified": {"const": True},
        "mapping_status": {"const": "mapped"},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python diff-to-chunk mapping v1",
        "type": "object",
        "properties": props,
        "required": sorted(props),
        "additionalProperties": False,
    }


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise DiffChunkMapError(
            code,
            f"invalid JSON at {path}",
            "Regenerate upstream artifacts before mapping diffs.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def load_chunks(path: Path) -> dict[tuple[str, str, str, str], list[dict[str, Any]]]:
    ensure_file(path, "PY_DIFF_CHUNK_SOURCE_CHUNKS_MISSING", "AST chunk JSONL")
    index: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            row = json.loads(line)
            key = (row["task_instance_id"], row["workspace_state"], row["mutation_category"], row["relative_path"])
            index.setdefault(key, []).append(
                {
                    "chunk_id": row["chunk_id"],
                    "chunk_kind": row["chunk_kind"],
                    "name": row["name"],
                    "qualname": row["qualname"],
                    "line_start": row["line_start"],
                    "line_end": row["line_end"],
                    "depth": row["depth"],
                }
            )
    for chunks in index.values():
        chunks.sort(key=lambda item: (item["line_start"], item["line_end"], item["chunk_kind"]))
    return index


def norm_path(path: str) -> str | None:
    path = path.strip().split("\t", 1)[0]
    if path == "/dev/null":
        return None
    if path.startswith(("a/", "b/")):
        return path[2:]
    return path


def parse_unified_diff(text: str) -> list[Hunk]:
    hunks: list[Hunk] = []
    old_path: str | None = None
    new_path: str | None = None
    current: dict[str, Any] | None = None
    old_line = 0
    new_line = 0

    def flush() -> None:
        nonlocal current
        if current is None:
            return
        hunks.append(Hunk(**current))
        current = None

    for line in text.splitlines():
        if line.startswith("diff --git "):
            flush()
            old_path = None
            new_path = None
            continue
        if line.startswith("--- "):
            flush()
            old_path = norm_path(line[4:])
            continue
        if line.startswith("+++ "):
            new_path = norm_path(line[4:])
            continue
        match = HUNK_RE.match(line)
        if match:
            flush()
            old_line = int(match.group("old_start"))
            new_line = int(match.group("new_start"))
            current = {
                "old_path": old_path,
                "new_path": new_path,
                "index": len(hunks),
                "header": line,
                "old_start": old_line,
                "old_count": int(match.group("old_count") or "1"),
                "new_start": new_line,
                "new_count": int(match.group("new_count") or "1"),
                "lines": [],
                "old_changed": [],
                "new_changed": [],
                "old_expected": [],
                "new_expected": [],
            }
            continue
        if current is None or line.startswith("\\"):
            continue
        marker = line[:1]
        content = line[1:] if marker in {" ", "+", "-"} else line
        current["lines"].append(line)
        if marker == " ":
            current["old_expected"].append((old_line, content))
            current["new_expected"].append((new_line, content))
            old_line += 1
            new_line += 1
        elif marker == "-":
            current["old_changed"].append(old_line)
            current["old_expected"].append((old_line, content))
            old_line += 1
        elif marker == "+":
            current["new_changed"].append(new_line)
            current["new_expected"].append((new_line, content))
            new_line += 1
    flush()
    return hunks


def verify_lines(root: Path, relative_path: str | None, expected: tuple[tuple[int, str], ...]) -> bool:
    if relative_path is None or not expected:
        return True
    path = root / relative_path
    ensure_file(path, "PY_DIFF_CHUNK_TARGET_FILE_MISSING", "diff target source file")
    lines = path.read_bytes().splitlines()
    for line_number, expected_text in expected:
        if line_number < 1 or line_number > len(lines):
            return False
        if lines[line_number - 1] != expected_text.encode("utf-8"):
            return False
    return True


def span(lines: tuple[int, ...]) -> dict[str, int | None]:
    return {"start": min(lines), "end": max(lines)} if lines else {"start": None, "end": None}


def overlap(chunks: list[dict[str, Any]], lines: tuple[int, ...]) -> list[dict[str, Any]]:
    if not lines:
        return []
    start = min(lines)
    end = max(lines)
    found = [chunk for chunk in chunks if chunk["line_start"] <= end and chunk["line_end"] >= start]
    found.sort(key=lambda chunk: (chunk["line_end"] - chunk["line_start"], -chunk["depth"], chunk["chunk_kind"]))
    return [{key: chunk[key] for key in ("chunk_id", "chunk_kind", "name", "qualname", "line_start", "line_end")} for chunk in found]


def mapping_id(row: dict[str, Any]) -> str:
    return sha256_prefixed_text(
        canonical_json(
            {
                "row_id": row["row_id"],
                "patch_label": row["patch_label"],
                "file_old_path": row["file_old_path"],
                "file_new_path": row["file_new_path"],
                "hunk_index": row["hunk_index"],
                "hunk_sha256": row["hunk_sha256"],
            }
        )
    )


def build_record(
    state: dict[str, Any],
    patch: dict[str, Any],
    hunk: Hunk,
    chunks: dict[tuple[str, str, str, str], list[dict[str, Any]]],
    schema_path: Path,
    now: str,
) -> dict[str, Any]:
    patch_path = Path(patch["patch"]["path"])
    patch_bytes = patch_path.read_bytes()
    patch_sha = "sha256:" + hashlib.sha256(patch_bytes).hexdigest()
    if patch_sha != patch["patch"]["sha256"]:
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_PATCH_HASH_MISMATCH",
            f"patch file hash mismatch: {patch_path}",
            "Regenerate workspace materialization before mapping diffs.",
            {"expected": patch["patch"]["sha256"], "actual": patch_sha, "path": str(patch_path)},
        )
    target_root = Path(state["hot_workspace_path"])
    base_root = target_root.parent / "base"
    if not verify_lines(base_root, hunk.old_path, hunk.old_expected) or not verify_lines(target_root, hunk.new_path, hunk.new_expected):
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_HUNK_BYTES_MISMATCH",
            "diff hunk bytes do not match base or target workspace files",
            "Regenerate workspaces from the patch source of truth before mapping.",
            {"row_id": state["row_id"], "patch_label": patch["label"], "hunk_header": hunk.header},
        )
    old_key = (state["task_id"], "base", "known_good", hunk.old_path or "")
    new_key = (state["task_id"], state["state"], state["mutation_category"], hunk.new_path or "")
    old_reference_lines = hunk.old_changed or tuple(line_number for line_number, _ in hunk.old_expected)
    old_chunks = overlap(chunks.get(old_key, []), old_reference_lines)
    new_chunks = overlap(chunks.get(new_key, []), hunk.new_changed)
    change_kind = "add" if not hunk.old_changed else "delete" if not hunk.new_changed else "modify"
    primary = new_chunks if new_chunks else old_chunks
    target_side = "new" if new_chunks else "old"
    if not primary:
        raise DiffChunkMapError(
            "PY_DIFF_CHUNK_HUNK_UNMAPPED",
            "diff hunk did not overlap any AST chunk",
            "Fix chunk coverage or mapping logic before using this reward field.",
            {"row_id": state["row_id"], "patch_label": patch["label"], "hunk_header": hunk.header},
        )
    hunk_sha = sha256_prefixed_text("\n".join((hunk.header, *hunk.lines)))
    row = {
        "schema_version": SCHEMA_VERSION,
        "mapping_id": "",
        "source": f"contextgraph://python-diff-chunk-map/{state['task_id']}/{state['state']}",
        "subject": f"{state['row_id']}#{patch['label']}#{hunk.index}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "task_instance_id": state["task_id"],
        "row_id": state["row_id"],
        "repo": state["repo"],
        "mutation_category": state["mutation_category"],
        "workspace_state": state["state"],
        "workspace_tree_hash": state["result_tree_hash"],
        "patch_label": patch["label"],
        "patch_path": str(patch_path),
        "patch_sha256": patch_sha,
        "patch_text_sha256": sha256_prefixed_text(patch_bytes.decode("utf-8")),
        "file_old_path": hunk.old_path,
        "file_new_path": hunk.new_path,
        "hunk_index": hunk.index,
        "hunk_header": hunk.header,
        "hunk_sha256": hunk_sha,
        "old_line_span": span(hunk.old_changed),
        "new_line_span": span(hunk.new_changed),
        "old_changed_lines": list(hunk.old_changed),
        "new_changed_lines": list(hunk.new_changed),
        "change_kind": change_kind,
        "target_side": target_side,
        "old_chunks": old_chunks,
        "new_chunks": new_chunks,
        "primary_chunk_ids": [chunk["chunk_id"] for chunk in primary],
        "nearest_chunk_id": primary[0]["chunk_id"],
        "nearest_symbol": primary[0]["qualname"] or primary[0]["name"],
        "related_production_chunk_ids": [],
        "source_readback_verified": True,
        "mapping_status": "mapped",
    }
    row["mapping_id"] = mapping_id(row)
    return row


def link_test_records(records: list[dict[str, Any]]) -> None:
    production_by_row: dict[tuple[str, str], list[str]] = {}
    for record in records:
        if record["patch_label"] in {"gold", "mutation"}:
            production_by_row.setdefault((record["task_instance_id"], record["row_id"]), []).extend(record["primary_chunk_ids"])
    for record in records:
        if record["patch_label"] == "test":
            key = (record["task_instance_id"], record["row_id"])
            record["related_production_chunk_ids"] = sorted(set(production_by_row.get(key, [])))


def edge_chunk(task_id: str, state: str, mutation_category: str, relative_path: str, kind: str, line_start: int, line_end: int, name: str | None) -> tuple[tuple[str, str, str, str], dict[str, Any]]:
    payload = {
        "task_id": task_id,
        "state": state,
        "mutation_category": mutation_category,
        "relative_path": relative_path,
        "kind": kind,
        "line_start": line_start,
        "line_end": line_end,
        "name": name,
    }
    chunk = {
        "chunk_id": sha256_prefixed_text(canonical_json(payload)),
        "chunk_kind": kind,
        "name": name,
        "qualname": name,
        "line_start": line_start,
        "line_end": line_end,
        "depth": 0,
    }
    return (task_id, state, mutation_category, relative_path), chunk


def write_edge_patch(root: Path, name: str, text: str) -> dict[str, Any]:
    path = root / f"{name}.patch"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    sha = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    return {"label": "edge", "patch": {"path": str(path), "sha256": sha}}


def edge_records(schema_path: Path, hot_root: Path, now: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    edge_root = hot_root / "edge-workspaces"
    edge_root.mkdir(parents=True, exist_ok=True)
    chunk_index: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
    records: list[dict[str, Any]] = []

    def add_chunk(*args: Any) -> None:
        key, chunk = edge_chunk(*args)
        chunk_index.setdefault(key, []).append(chunk)

    # Edge 1: hunk at file start maps to the first-line module statement chunk.
    task = "synthetic_file_start"
    base = edge_root / task / "base"
    target = edge_root / task / "edge_file_start"
    base.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    (base / "first.py").write_text("VALUE = 1\n", encoding="utf-8")
    (target / "first.py").write_text("VALUE = 2\n", encoding="utf-8")
    add_chunk(task, "base", "known_good", "first.py", "module_statement", 1, 1, None)
    add_chunk(task, "edge_file_start", "edge_case", "first.py", "module_statement", 1, 1, None)
    patch = write_edge_patch(edge_root / task, "file_start", "diff --git a/first.py b/first.py\n--- a/first.py\n+++ b/first.py\n@@ -1,1 +1,1 @@\n-VALUE = 1\n+VALUE = 2\n")
    state = {"task_id": task, "row_id": f"{task}::edge", "repo": "synthetic/edge", "mutation_category": "edge_case", "state": "edge_file_start", "result_tree_hash": "1" * 40, "hot_workspace_path": str(target)}
    records.extend(build_record(state, patch, hunk, chunk_index, schema_path, now) for hunk in parse_unified_diff(Path(patch["patch"]["path"]).read_text(encoding="utf-8")))

    # Edge 2: deleted symbol maps to the old/base chunk when no new chunk exists.
    task = "synthetic_deleted_symbol"
    base = edge_root / task / "base"
    target = edge_root / task / "edge_deleted_symbol"
    base.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    (base / "delete_me.py").write_text("def gone():\n    return 1\n\nkeep = 1\n", encoding="utf-8")
    (target / "delete_me.py").write_text("keep = 1\n", encoding="utf-8")
    add_chunk(task, "base", "known_good", "delete_me.py", "function", 1, 2, "gone")
    add_chunk(task, "edge_deleted_symbol", "edge_case", "delete_me.py", "module_statement", 1, 1, None)
    patch = write_edge_patch(edge_root / task, "deleted_symbol", "diff --git a/delete_me.py b/delete_me.py\n--- a/delete_me.py\n+++ b/delete_me.py\n@@ -1,4 +1,1 @@\n-def gone():\n-    return 1\n-\n keep = 1\n")
    state = {"task_id": task, "row_id": f"{task}::edge", "repo": "synthetic/edge", "mutation_category": "edge_case", "state": "edge_deleted_symbol", "result_tree_hash": "2" * 40, "hot_workspace_path": str(target)}
    records.extend(build_record(state, patch, hunk, chunk_index, schema_path, now) for hunk in parse_unified_diff(Path(patch["patch"]["path"]).read_text(encoding="utf-8")))

    # Edge 3: test-only patch maps to a test chunk and has no production link.
    task = "synthetic_test_only"
    base = edge_root / task / "base"
    target = edge_root / task / "test_patch"
    base.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    (base / "tests/test_only.py").parent.mkdir(parents=True, exist_ok=True)
    (target / "tests/test_only.py").parent.mkdir(parents=True, exist_ok=True)
    (base / "tests/test_only.py").write_text("def test_only():\n    assert 1\n", encoding="utf-8")
    (target / "tests/test_only.py").write_text("def test_only():\n    assert 2\n", encoding="utf-8")
    add_chunk(task, "base", "known_good", "tests/test_only.py", "test_function", 1, 2, "test_only")
    add_chunk(task, "test_patch", "edge_case", "tests/test_only.py", "test_function", 1, 2, "test_only")
    patch = write_edge_patch(edge_root / task, "test_only", "diff --git a/tests/test_only.py b/tests/test_only.py\n--- a/tests/test_only.py\n+++ b/tests/test_only.py\n@@ -1,2 +1,2 @@\n def test_only():\n-    assert 1\n+    assert 2\n")
    patch = {"label": "test", "patch": patch["patch"]}
    state = {"task_id": task, "row_id": f"{task}::edge", "repo": "synthetic/edge", "mutation_category": "edge_case", "state": "test_patch", "result_tree_hash": "3" * 40, "hot_workspace_path": str(target)}
    records.extend(build_record(state, patch, hunk, chunk_index, schema_path, now) for hunk in parse_unified_diff(Path(patch["patch"]["path"]).read_text(encoding="utf-8")))
    link_test_records(records)
    cases = [
        {"case": "hunk_at_file_start", "pass": any(r["task_instance_id"] == "synthetic_file_start" and r["new_line_span"] == {"start": 1, "end": 1} and r["target_side"] == "new" for r in records)},
        {"case": "deleted_symbol", "pass": any(r["task_instance_id"] == "synthetic_deleted_symbol" and r["change_kind"] == "delete" and r["target_side"] == "old" and r["nearest_symbol"] == "gone" for r in records)},
        {"case": "test_only_patch_no_production_hunk", "pass": any(r["task_instance_id"] == "synthetic_test_only" and r["patch_label"] == "test" and not r["related_production_chunk_ids"] for r in records)},
    ]
    return records, cases


def validate_records(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise DiffChunkMapError(
                "PY_DIFF_CHUNK_SCHEMA_VALIDATION_FAILED",
                "generated diff-to-chunk record failed strict JSON Schema validation",
                "Fix record builder before writing reward data.",
                {"mapping_id": record.get("mapping_id"), "errors": [err.message for err in errors[:10]]},
            )


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())
