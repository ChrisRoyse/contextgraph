from __future__ import annotations

import ast
import hashlib
import json
import re
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file
from .oracle_events import canonical_json, sha256_file, sha256_prefixed_text


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-013"
PRODUCER = "contextgraph.swebench_lite_ops.python_test_intent"
DATACONTENTTYPE = "application/json"
RECORD_KINDS = (
    "test_case",
    "assertion",
    "expected_exception",
    "fixture",
    "parametrization",
    "skip_xfail",
    "target_label",
    "uncertainty",
)


class TestIntentError(SwebenchOpsError):
    pass


def test_intent_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    nullable_int = {"type": ["integer", "null"]}
    sha = {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"}
    sha_items = {"type": "array", "items": sha}
    props = {
        "schema_version": {"const": SCHEMA_VERSION},
        "intent_id": sha,
        "record_kind": {"enum": list(RECORD_KINDS)},
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
        "relative_path": nullable_string,
        "absolute_path": nullable_string,
        "file_sha256": nullable_string,
        "patch_path": nullable_string,
        "patch_sha256": nullable_string,
        "hunk_mapping_ids": sha_items,
        "chunk_ids": sha_items,
        "production_chunk_ids": sha_items,
        "test_name": nullable_string,
        "pytest_nodeid": nullable_string,
        "target_transition": {"enum": ["FAIL_TO_PASS", "PASS_TO_PASS", "FAIL_TO_FAIL", "PASS_TO_FAIL", None]},
        "line_start": nullable_int,
        "line_end": nullable_int,
        "source_text": {"type": "string"},
        "intent_kind": {"type": "string", "minLength": 1},
        "assertion_operator": nullable_string,
        "expected_behavior": nullable_string,
        "expected_exception": nullable_string,
        "marker_name": nullable_string,
        "parametrize_names": {"type": "array", "items": {"type": "string"}},
        "fixture_names": {"type": "array", "items": {"type": "string"}},
        "target_symbols": {"type": "array", "items": {"type": "string"}},
        "confidence": {"type": "number", "minimum": 0, "maximum": 1},
        "evidence": {"type": "object"},
        "source_readback_verified": {"const": True},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python test intent v1",
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
        raise TestIntentError(code, f"invalid JSON at {path}", "Regenerate the upstream artifact.", {"path": str(path), "error": str(exc)}) from exc


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    ensure_file(path, code, "required JSONL artifact")
    rows: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError as exc:
                raise TestIntentError(code, f"invalid JSONL line {line_number} at {path}", "Regenerate the upstream artifact.", {"path": str(path), "line_number": line_number, "error": str(exc)}) from exc
    return rows


def cleaned_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    rows = manifest.get("rows")
    if not isinstance(rows, list):
        raise TestIntentError("PY_TEST_INTENT_CLEANED_MANIFEST_INVALID", "cleaned manifest is missing rows", "Regenerate TASK-PY-DATA-004.", {"keys": sorted(manifest.keys())})
    return {row["row_id"]: row for row in rows}


def expected_targets(row: dict[str, Any]) -> list[dict[str, str]]:
    labels = row.get("expected_labels", {})
    targets: list[dict[str, str]] = []
    for key, transition in (("fail_to_pass", "FAIL_TO_PASS"), ("pass_to_pass", "PASS_TO_PASS")):
        for nodeid in labels.get(key, []):
            targets.append({"pytest_nodeid": nodeid, "target_transition": transition})
    seen: set[tuple[str, str]] = set()
    deduped = []
    for target in targets:
        identity = (target["pytest_nodeid"], target["target_transition"])
        if identity not in seen:
            deduped.append(target)
            seen.add(identity)
    overlap = {t["pytest_nodeid"] for t in targets if t["target_transition"] == "FAIL_TO_PASS"} & {t["pytest_nodeid"] for t in targets if t["target_transition"] == "PASS_TO_PASS"}
    if overlap:
        raise TestIntentError("PY_TEST_INTENT_CONTRADICTORY_LABELS", "same pytest node has contradictory target transitions", "Fix cleaned manifest expected_labels before extracting intent.", {"row_id": row["row_id"], "overlap": sorted(overlap)})
    return deduped


def base_test_name(pytest_nodeid: str) -> str:
    tail = pytest_nodeid.rsplit("::", 1)[-1]
    return tail.split("[", 1)[0]


def nodeid_path(pytest_nodeid: str) -> str:
    return pytest_nodeid.split("::", 1)[0]


def dotted_name(node: ast.AST) -> str:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return f"{dotted_name(node.value)}.{node.attr}"
    if isinstance(node, ast.Call):
        return dotted_name(node.func)
    return ast.unparse(node)


def source_segment(text: str, node: ast.AST) -> str:
    segment = ast.get_source_segment(text, node)
    return segment if segment is not None else ast.unparse(node)


def node_span(node: ast.AST) -> tuple[int | None, int | None]:
    start = getattr(node, "lineno", None)
    end = getattr(node, "end_lineno", start)
    return start, end


def overlaps(start: int | None, end: int | None, ranges: list[tuple[int, int]]) -> bool:
    if start is None or end is None:
        return False
    return any(start <= hi and end >= lo for lo, hi in ranges)


def names_in(node: ast.AST) -> list[str]:
    names: set[str] = set()
    for item in ast.walk(node):
        if isinstance(item, ast.Name):
            names.add(item.id)
        elif isinstance(item, ast.Attribute):
            try:
                names.add(dotted_name(item))
            except RecursionError:
                continue
        elif isinstance(item, ast.Call):
            try:
                names.add(dotted_name(item.func))
            except RecursionError:
                continue
    return sorted(names)


def parametrize_names(dec: ast.AST) -> list[str]:
    if not isinstance(dec, ast.Call) or "parametrize" not in dotted_name(dec.func):
        return []
    if not dec.args:
        return []
    first = dec.args[0]
    if isinstance(first, ast.Constant) and isinstance(first.value, str):
        return [part.strip() for part in re.split(r"[, ]+", first.value) if part.strip()]
    if isinstance(first, (ast.Tuple, ast.List)):
        return [ast.unparse(item) for item in first.elts]
    return [ast.unparse(first)]


def parametrize_case_count(dec: ast.AST) -> int | None:
    if not isinstance(dec, ast.Call) or len(dec.args) < 2:
        return None
    values = dec.args[1]
    if isinstance(values, (ast.List, ast.Tuple)):
        return len(values.elts)
    return None


def marker_name(dec: ast.AST) -> str | None:
    text = ast.unparse(dec)
    for marker in ("skipif", "skip", "xfail"):
        if f"pytest.mark.{marker}" in text or text.endswith(marker):
            return marker
    if "unittest.skip" in text or text.endswith(".skip"):
        return "skip"
    return None


def raises_exception(node: ast.AST) -> str | None:
    if isinstance(node, ast.With):
        for item in node.items:
            ctx = item.context_expr
            if isinstance(ctx, ast.Call) and "raises" in dotted_name(ctx.func) and ctx.args:
                return dotted_name(ctx.args[0])
    if isinstance(node, ast.Call) and "assertRaises" in dotted_name(node.func) and node.args:
        return dotted_name(node.args[0])
    return None


def assertion_operator(node: ast.Assert) -> str | None:
    test = node.test
    if isinstance(test, ast.Compare) and test.ops:
        return type(test.ops[0]).__name__
    if isinstance(test, ast.BoolOp):
        return type(test.op).__name__
    if isinstance(test, ast.UnaryOp):
        return type(test.op).__name__
    return type(test).__name__


def file_sha(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def intent_id(seed: dict[str, Any]) -> str:
    return sha256_prefixed_text(canonical_json(seed))


def make_record(base: dict[str, Any], schema_path: Path, now: str, record_kind: str, **values: Any) -> dict[str, Any]:
    line_start = values.get("line_start")
    seed = {
        "row_id": base["row_id"],
        "record_kind": record_kind,
        "relative_path": base.get("relative_path"),
        "pytest_nodeid": values.get("pytest_nodeid"),
        "test_name": values.get("test_name"),
        "line_start": line_start,
        "source_text": values.get("source_text", ""),
        "target_transition": values.get("target_transition"),
        "intent_kind": values.get("intent_kind", record_kind),
    }
    row = {
        "schema_version": SCHEMA_VERSION,
        "intent_id": intent_id(seed),
        "record_kind": record_kind,
        "source": f"contextgraph://python-test-intent/{base['task_instance_id']}/{base['workspace_state']}",
        "subject": f"{base['row_id']}#{base.get('relative_path')}#{record_kind}#{line_start}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "task_instance_id": base["task_instance_id"],
        "row_id": base["row_id"],
        "repo": base["repo"],
        "mutation_category": base["mutation_category"],
        "workspace_state": base["workspace_state"],
        "workspace_tree_hash": base["workspace_tree_hash"],
        "relative_path": base.get("relative_path"),
        "absolute_path": base.get("absolute_path"),
        "file_sha256": base.get("file_sha256"),
        "patch_path": base.get("patch_path"),
        "patch_sha256": base.get("patch_sha256"),
        "hunk_mapping_ids": sorted(set(base.get("hunk_mapping_ids", []))),
        "chunk_ids": sorted(set(values.get("chunk_ids", base.get("chunk_ids", [])))),
        "production_chunk_ids": sorted(set(values.get("production_chunk_ids", base.get("production_chunk_ids", [])))),
        "test_name": values.get("test_name"),
        "pytest_nodeid": values.get("pytest_nodeid"),
        "target_transition": values.get("target_transition"),
        "line_start": line_start,
        "line_end": values.get("line_end"),
        "source_text": values.get("source_text", ""),
        "intent_kind": values.get("intent_kind", record_kind),
        "assertion_operator": values.get("assertion_operator"),
        "expected_behavior": values.get("expected_behavior"),
        "expected_exception": values.get("expected_exception"),
        "marker_name": values.get("marker_name"),
        "parametrize_names": values.get("parametrize_names", []),
        "fixture_names": values.get("fixture_names", []),
        "target_symbols": sorted(set(values.get("target_symbols", []))),
        "confidence": values.get("confidence", 0.7),
        "evidence": values.get("evidence", {}),
        "source_readback_verified": True,
    }
    return row


class IntentVisitor(ast.NodeVisitor):
    def __init__(self, text: str, base: dict[str, Any], schema_path: Path, now: str, changed_ranges: list[tuple[int, int]], targets: list[dict[str, str]]) -> None:
        self.text = text
        self.base = base
        self.schema_path = schema_path
        self.now = now
        self.changed_ranges = changed_ranges
        self.targets = targets
        self.records: list[dict[str, Any]] = []
        self.stack: list[ast.AST] = []

    def matching_target(self, name: str | None) -> dict[str, str] | None:
        if not name:
            return None
        rel = self.base.get("relative_path") or ""
        matches = [t for t in self.targets if base_test_name(t["pytest_nodeid"]) == name and nodeid_path(t["pytest_nodeid"]) == rel]
        return matches[0] if matches else None

    def in_changed_test(self, node: ast.AST) -> bool:
        start, end = node_span(node)
        if overlaps(start, end, self.changed_ranges):
            return True
        for parent in reversed(self.stack):
            if isinstance(parent, (ast.FunctionDef, ast.AsyncFunctionDef)):
                p_start, p_end = node_span(parent)
                return overlaps(p_start, p_end, self.changed_ranges)
        return False

    def emit(self, record_kind: str, node: ast.AST, **values: Any) -> None:
        start, end = node_span(node)
        self.records.append(
            make_record(
                self.base,
                self.schema_path,
                self.now,
                record_kind,
                line_start=start,
                line_end=end,
                source_text=source_segment(self.text, node),
                **values,
            )
        )

    def visit(self, node: ast.AST) -> Any:
        self.stack.append(node)
        try:
            return super().visit(node)
        finally:
            self.stack.pop()

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        self._function(node)
        self.generic_visit(node)

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        self._function(node)
        self.generic_visit(node)

    def _function(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
        dec_text = [ast.unparse(dec) for dec in node.decorator_list]
        target = self.matching_target(node.name)
        is_fixture = any("fixture" in dec for dec in dec_text)
        is_test = node.name.startswith("test") or target is not None
        start, end = node_span(node)
        touched = overlaps(start, end, self.changed_ranges)
        if is_test and (touched or target is not None):
            self.emit(
                "test_case",
                node,
                test_name=node.name,
                pytest_nodeid=target["pytest_nodeid"] if target else None,
                target_transition=target["target_transition"] if target else None,
                fixture_names=list(node.args.args[i].arg for i in range(len(node.args.args))),
                target_symbols=names_in(node),
                confidence=0.9 if target or touched else 0.7,
                evidence={"decorators": dec_text, "touched_by_hunk": touched},
            )
        if is_fixture:
            self.emit("fixture", node, test_name=node.name, fixture_names=[node.name], confidence=0.95, evidence={"decorators": dec_text})
        for dec in node.decorator_list:
            names = parametrize_names(dec)
            if names:
                self.emit("parametrization", dec, test_name=node.name, parametrize_names=names, confidence=0.95, evidence={"case_count": parametrize_case_count(dec), "decorator": ast.unparse(dec)})
            marker = marker_name(dec)
            if marker:
                self.emit("skip_xfail", dec, test_name=node.name, marker_name=marker, confidence=0.95, evidence={"decorator": ast.unparse(dec)})

    def visit_Assert(self, node: ast.Assert) -> None:
        if self.in_changed_test(node):
            self.emit("assertion", node, assertion_operator=assertion_operator(node), expected_behavior=ast.unparse(node.test), target_symbols=names_in(node.test), confidence=0.95)
        self.generic_visit(node)

    def visit_With(self, node: ast.With) -> None:
        expected = raises_exception(node)
        if expected and self.in_changed_test(node):
            self.emit("expected_exception", node, expected_exception=expected, target_symbols=names_in(node), confidence=0.95)
        self.generic_visit(node)

    def visit_Call(self, node: ast.Call) -> None:
        expected = raises_exception(node)
        if expected and self.in_changed_test(node):
            self.emit("expected_exception", node, expected_exception=expected, target_symbols=names_in(node), confidence=0.9)
        name = dotted_name(node.func)
        if name in {"pytest.skip", "pytest.xfail"} and self.in_changed_test(node):
            self.emit("skip_xfail", node, marker_name=name.rsplit(".", 1)[-1], target_symbols=names_in(node), confidence=0.9)
        self.generic_visit(node)


def build_file_records(
    *,
    state: dict[str, Any],
    row: dict[str, Any],
    file_path: Path,
    relative_path: str,
    maps: list[dict[str, Any]],
    chunks: list[dict[str, Any]],
    schema_path: Path,
    now: str,
) -> list[dict[str, Any]]:
    ensure_file(file_path, "PY_TEST_INTENT_TEST_FILE_MISSING", "test file source")
    patch = next((p for p in state.get("applied_patches", []) if p["label"] == "test"), None)
    if patch is None:
        raise TestIntentError("PY_TEST_INTENT_TEST_PATCH_MISSING", "test workspace has no applied test patch", "Regenerate TASK-PY-DATA-006 workspace manifest.", {"row_id": state["row_id"]})
    patch_path = Path(patch["patch"]["path"])
    ensure_file(patch_path, "PY_TEST_INTENT_TEST_PATCH_FILE_MISSING", "test patch source")
    patch_sha = sha256_file(patch_path)
    if patch_sha != patch["patch"]["sha256"]:
        raise TestIntentError("PY_TEST_INTENT_PATCH_HASH_MISMATCH", "test patch hash changed before intent extraction", "Regenerate workspace materialization.", {"expected": patch["patch"]["sha256"], "actual": patch_sha, "path": str(patch_path)})
    actual_file_sha = file_sha(file_path)
    chunk_shas = {chunk.get("file_sha256") for chunk in chunks if chunk.get("file_sha256")}
    if chunk_shas and chunk_shas != {actual_file_sha}:
        raise TestIntentError("PY_TEST_INTENT_CHUNK_FILE_HASH_MISMATCH", "selected AST chunks do not match current test file bytes", "Regenerate AST chunks from the current workspace bytes.", {"relative_path": relative_path, "chunk_shas": sorted(chunk_shas), "actual": actual_file_sha})
    text = file_path.read_text(encoding="utf-8")
    try:
        tree = ast.parse(text, filename=str(file_path))
    except SyntaxError as exc:
        raise TestIntentError("PY_TEST_INTENT_TEST_FILE_PARSE_FAILED", "test patch file does not parse", "Reject malformed test patch input before reward extraction.", {"path": str(file_path), "error": str(exc)}) from exc
    production_ids = sorted({cid for item in maps for cid in item["related_production_chunk_ids"]})
    chunk_ids = sorted({cid for item in maps for cid in item["primary_chunk_ids"]})
    ranges = [(item["new_line_span"]["start"], item["new_line_span"]["end"]) for item in maps if item["new_line_span"]["start"] is not None and item["new_line_span"]["end"] is not None]
    base = {
        "task_instance_id": state["task_id"],
        "row_id": state["row_id"],
        "repo": state["repo"],
        "mutation_category": state["mutation_category"],
        "workspace_state": state["state"],
        "workspace_tree_hash": state["result_tree_hash"],
        "relative_path": relative_path,
        "absolute_path": str(file_path),
        "file_sha256": actual_file_sha,
        "patch_path": str(patch_path),
        "patch_sha256": patch_sha,
        "hunk_mapping_ids": [item["mapping_id"] for item in maps],
        "chunk_ids": chunk_ids,
        "production_chunk_ids": production_ids,
    }
    targets = [target for target in expected_targets(row) if nodeid_path(target["pytest_nodeid"]) == relative_path]
    visitor = IntentVisitor(text, base, schema_path, now, ranges, targets)
    visitor.visit(tree)
    records = list(visitor.records)
    for target in targets:
        records.append(
            make_record(
                base,
                schema_path,
                now,
                "target_label",
                test_name=base_test_name(target["pytest_nodeid"]),
                pytest_nodeid=target["pytest_nodeid"],
                target_transition=target["target_transition"],
                intent_kind="oracle_target_transition",
                confidence=1.0,
                evidence={"source": "cleaned_manifest.expected_labels", "oracle_status": row.get("oracle", {}).get("tests_status", {})},
            )
        )
    if not records:
        records.append(make_record(base, schema_path, now, "uncertainty", intent_kind="no_intent_records_for_touched_file", confidence=0.2, evidence={"changed_ranges": ranges}))
    return records
