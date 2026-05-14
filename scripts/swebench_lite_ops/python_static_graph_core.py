from __future__ import annotations

import ast
import hashlib
import json
import warnings
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, ensure_file
from .oracle_events import canonical_json, sha256_prefixed_text
from .python_ast_parse import detect_and_decode


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-012"
PRODUCER = "contextgraph.swebench_lite_ops.python_static_graph"
DATACONTENTTYPE = "application/json"


class StaticGraphError(SwebenchOpsError):
    pass


def graph_schema(schema_path: Path) -> dict[str, Any]:
    nullable_string = {"type": ["string", "null"]}
    nullable_int = {"type": ["integer", "null"]}
    sha_nullable = {"type": ["string", "null"], "pattern": "^sha256:[0-9a-f]{64}$"}
    props = {
        "schema_version": {"const": SCHEMA_VERSION},
        "graph_id": {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"},
        "source": {"type": "string", "minLength": 1},
        "subject": {"type": "string", "minLength": 1},
        "dataschema": {"type": "string", "minLength": 1},
        "datacontenttype": {"const": DATACONTENTTYPE},
        "time": {"type": "string", "minLength": 1},
        "producer": {"const": PRODUCER},
        "task_id": {"const": TASK_ID},
        "record_kind": {"enum": ["symbol", "import", "call", "type_hint", "reference", "fixture", "test_dependency", "uncertainty"]},
        "task_instance_id": {"type": "string", "minLength": 1},
        "workspace_state": {"type": "string", "minLength": 1},
        "mutation_category": {"type": "string", "minLength": 1},
        "repo": {"type": "string", "minLength": 1},
        "workspace_tree_hash": {"type": "string", "pattern": "^[0-9a-f]{40}$"},
        "relative_path": {"type": "string", "minLength": 1},
        "file_sha256": {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"},
        "module_name": nullable_string,
        "chunk_id": sha_nullable,
        "chunk_kind": nullable_string,
        "chunk_qualname": nullable_string,
        "line_start": nullable_int,
        "line_end": nullable_int,
        "edge_kind": nullable_string,
        "symbol_kind": nullable_string,
        "name": nullable_string,
        "qualname": nullable_string,
        "target_name": nullable_string,
        "target_module": nullable_string,
        "target_chunk_id": sha_nullable,
        "target_relative_path": nullable_string,
        "resolution_status": {"enum": ["local", "internal_module", "external_or_missing", "dynamic_constant", "dynamic_unknown", "unresolved", "not_applicable", "cycle"]},
        "occurrence_count": {"type": "integer", "minimum": 1},
        "uncertainty_reason": nullable_string,
        "details": {"type": "object"},
        "source_readback_verified": {"const": True},
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": str(schema_path),
        "title": "ContextGraph Python static reality graph v1",
        "type": "object",
        "properties": props,
        "required": sorted(props),
        "additionalProperties": False,
    }


def load_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    return json.loads(path.read_text(encoding="utf-8"))


def load_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    ensure_file(path, code, "required JSONL artifact")
    rows = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as exc:
            raise StaticGraphError(code, f"invalid JSONL line {line_number} at {path}", "Regenerate upstream artifacts.", {"error": str(exc)}) from exc
    return rows


def module_name_for_path(relative_path: str) -> str:
    if not relative_path.endswith(".py"):
        return relative_path.replace("/", ".")
    parts = relative_path[:-3].split("/")
    if parts[-1] == "__init__":
        parts = parts[:-1] or ["__init__"]
    return ".".join(parts)


def package_for_module(module: str, relative_path: str) -> str:
    if relative_path.endswith("__init__.py"):
        return module
    return module.rpartition(".")[0]


def build_module_index(parse_records: list[dict[str, Any]]) -> dict[tuple[str, str, str], dict[str, str]]:
    index: dict[tuple[str, str, str], dict[str, str]] = {}
    for record in parse_records:
        if not record["relative_path"].endswith(".py"):
            continue
        key = (record["task_instance_id"], record["workspace_state"], record["mutation_category"])
        index.setdefault(key, {})[module_name_for_path(record["relative_path"])] = record["relative_path"]
    return index


def build_chunk_index(chunks: list[dict[str, Any]]) -> dict[tuple[str, str, str, str], list[dict[str, Any]]]:
    index: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
    for chunk in chunks:
        key = (chunk["task_instance_id"], chunk["workspace_state"], chunk["mutation_category"], chunk["relative_path"])
        index.setdefault(key, []).append(chunk)
    for rows in index.values():
        rows.sort(key=lambda item: (item["line_start"], item["line_end"], item["chunk_kind"]))
    return index


def chunk_for_line(chunks: list[dict[str, Any]], line: int | None) -> dict[str, Any] | None:
    if line is None:
        return None
    hits = [chunk for chunk in chunks if chunk["line_start"] <= line <= chunk["line_end"]]
    hits.sort(key=lambda chunk: (chunk["line_end"] - chunk["line_start"], -chunk["depth"], chunk["chunk_kind"]))
    return hits[0] if hits else None


def resolve_relative(module: str | None, level: int, current_module: str, relative_path: str) -> str | None:
    if level == 0:
        return module
    package = package_for_module(current_module, relative_path)
    parts = package.split(".") if package else []
    keep = len(parts) - level + 1
    if keep < 0:
        return None
    prefix = parts[:keep]
    if module:
        prefix.extend(module.split("."))
    return ".".join(part for part in prefix if part)


def resolve_module(target: str | None, modules: dict[str, str]) -> tuple[str, str | None]:
    if not target:
        return "unresolved", None
    if target in modules:
        return "internal_module", modules[target]
    if any(name.startswith(target + ".") for name in modules):
        return "internal_module", modules.get(target)
    return "external_or_missing", None


def record_id(seed: dict[str, Any]) -> str:
    return sha256_prefixed_text(canonical_json(seed))


def make_record(
    *,
    schema_path: Path,
    now: str,
    base: dict[str, Any],
    record_kind: str,
    chunk: dict[str, Any] | None,
    node: ast.AST | None,
    **values: Any,
) -> dict[str, Any]:
    graph_id = record_id(
        {
            "record_kind": record_kind,
            "task_instance_id": base["task_instance_id"],
            "workspace_state": base["workspace_state"],
            "relative_path": base["relative_path"],
            "chunk_id": chunk["chunk_id"] if chunk else None,
            "line": getattr(node, "lineno", None),
            "edge_kind": values.get("edge_kind"),
            "name": values.get("name"),
            "target_name": values.get("target_name"),
            "target_module": values.get("target_module"),
        }
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "graph_id": graph_id,
        "source": f"contextgraph://python-static-graph/{base['task_instance_id']}/{base['workspace_state']}",
        "subject": f"{base['task_instance_id']}#{base['workspace_state']}#{base['relative_path']}#{graph_id}",
        "dataschema": str(schema_path),
        "datacontenttype": DATACONTENTTYPE,
        "time": now,
        "producer": PRODUCER,
        "task_id": TASK_ID,
        "record_kind": record_kind,
        "task_instance_id": base["task_instance_id"],
        "workspace_state": base["workspace_state"],
        "mutation_category": base["mutation_category"],
        "repo": base["repo"],
        "workspace_tree_hash": base["workspace_tree_hash"],
        "relative_path": base["relative_path"],
        "file_sha256": base["file_sha256"],
        "module_name": base["module_name"],
        "chunk_id": chunk["chunk_id"] if chunk else None,
        "chunk_kind": chunk["chunk_kind"] if chunk else None,
        "chunk_qualname": chunk["qualname"] if chunk else None,
        "line_start": getattr(node, "lineno", None),
        "line_end": getattr(node, "end_lineno", getattr(node, "lineno", None)),
        "edge_kind": values.get("edge_kind"),
        "symbol_kind": values.get("symbol_kind"),
        "name": values.get("name"),
        "qualname": values.get("qualname"),
        "target_name": values.get("target_name"),
        "target_module": values.get("target_module"),
        "target_chunk_id": values.get("target_chunk_id"),
        "target_relative_path": values.get("target_relative_path"),
        "resolution_status": values.get("resolution_status", "not_applicable"),
        "occurrence_count": values.get("occurrence_count", 1),
        "uncertainty_reason": values.get("uncertainty_reason"),
        "details": values.get("details", {}),
        "source_readback_verified": True,
    }


def dotted_name(node: ast.AST) -> str:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return f"{dotted_name(node.value)}.{node.attr}"
    return ast.unparse(node)


def decorators(node: ast.AST) -> list[str]:
    return [ast.unparse(dec) for dec in getattr(node, "decorator_list", [])]


class StaticVisitor(ast.NodeVisitor):
    def __init__(self, *, schema_path: Path, now: str, base: dict[str, Any], chunks: list[dict[str, Any]], modules: dict[str, str]) -> None:
        self.schema_path = schema_path
        self.now = now
        self.base = base
        self.chunks = chunks
        self.modules = modules
        self.records: list[dict[str, Any]] = []
        self.calls: dict[tuple[str | None, str], dict[str, Any]] = {}
        self.references: dict[str | None, dict[str, Any]] = {}
        self.optional_depth = 0

    def emit(self, record_kind: str, node: ast.AST | None, **values: Any) -> None:
        self.records.append(make_record(schema_path=self.schema_path, now=self.now, base=self.base, record_kind=record_kind, chunk=chunk_for_line(self.chunks, getattr(node, "lineno", None)), node=node, **values))

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        decs = decorators(node)
        if any("fixture" in dec for dec in decs):
            kind = "fixture"
            record_kind = "fixture"
        elif node.name.startswith("test_") or "/test" in self.base["relative_path"]:
            kind = "test_function"
            record_kind = "symbol"
        else:
            kind = "function"
            record_kind = "symbol"
        params = [arg.arg for arg in (*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs)]
        self.emit(record_kind, node, symbol_kind=kind, name=node.name, qualname=node.name, details={"decorators": decs, "parameters": params})
        for arg in (*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs):
            if arg.annotation:
                self.emit("type_hint", arg.annotation, edge_kind="annotation", name=arg.arg, target_name=ast.unparse(arg.annotation))
        if node.returns:
            self.emit("type_hint", node.returns, edge_kind="return_annotation", name=node.name, target_name=ast.unparse(node.returns))
        self.generic_visit(node)

    visit_AsyncFunctionDef = visit_FunctionDef

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        self.emit("symbol", node, symbol_kind="class", name=node.name, qualname=node.name, details={"decorators": decorators(node)})
        self.generic_visit(node)

    def visit_Import(self, node: ast.Import) -> None:
        for alias in node.names:
            status, rel = resolve_module(alias.name, self.modules)
            reason = "missing_optional_dependency" if self.optional_depth and status == "external_or_missing" else None
            self.emit("import", node, edge_kind="import", name=alias.asname or alias.name, target_name=alias.name, target_module=alias.name, target_relative_path=rel, resolution_status=status, uncertainty_reason=reason, details={"optional": bool(self.optional_depth)})
        self.generic_visit(node)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        base_module = resolve_relative(node.module, node.level, self.base["module_name"], self.base["relative_path"])
        for alias in node.names:
            candidate = f"{base_module}.{alias.name}" if base_module and alias.name != "*" else base_module
            status, rel = resolve_module(candidate, self.modules)
            if status != "internal_module":
                status, rel = resolve_module(base_module, self.modules)
            reason = "missing_optional_dependency" if self.optional_depth and status == "external_or_missing" else None
            self.emit("import", node, edge_kind="from_import", name=alias.asname or alias.name, target_name=alias.name, target_module=candidate or base_module, target_relative_path=rel, resolution_status=status, uncertainty_reason=reason, details={"level": node.level, "optional": bool(self.optional_depth)})
        self.generic_visit(node)

    def visit_Try(self, node: ast.Try) -> None:
        optional = any(isinstance(item.type, ast.Name) and item.type.id == "ImportError" for item in node.handlers if item.type)
        if optional:
            self.optional_depth += 1
        for child in node.body:
            self.visit(child)
        if optional:
            self.optional_depth -= 1
        for child in (*node.handlers, *node.orelse, *node.finalbody):
            self.visit(child)

    def visit_Call(self, node: ast.Call) -> None:
        target = dotted_name(node.func)
        if target == "__import__" or target.endswith("import_module"):
            arg = node.args[0] if node.args else None
            if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
                status, rel = resolve_module(arg.value, self.modules)
                self.emit("uncertainty", node, edge_kind="dynamic_import", target_name=target, target_module=arg.value, target_relative_path=rel, resolution_status="dynamic_constant", uncertainty_reason=None if rel else status)
            else:
                self.emit("uncertainty", node, edge_kind="dynamic_import", target_name=target, resolution_status="dynamic_unknown", uncertainty_reason="non_constant_import_target")
        else:
            chunk = chunk_for_line(self.chunks, getattr(node, "lineno", None))
            key = (chunk["chunk_id"] if chunk else None, target)
            item = self.calls.setdefault(key, {"node": node, "count": 0})
            item["count"] += 1
        self.generic_visit(node)

    def visit_AnnAssign(self, node: ast.AnnAssign) -> None:
        self.emit("type_hint", node.annotation, edge_kind="annotation", name=ast.unparse(node.target), target_name=ast.unparse(node.annotation))
        self.generic_visit(node)

    def visit_Name(self, node: ast.Name) -> None:
        if isinstance(node.ctx, ast.Load):
            chunk = chunk_for_line(self.chunks, node.lineno)
            key = chunk["chunk_id"] if chunk else None
            item = self.references.setdefault(key, {"node": node, "count": 0, "names": {}})
            item["count"] += 1
            item["names"][node.id] = item["names"].get(node.id, 0) + 1
        self.generic_visit(node)

    def finish(self) -> list[dict[str, Any]]:
        for (chunk_id, target), item in self.calls.items():
            chunk = next((candidate for candidate in self.chunks if candidate["chunk_id"] == chunk_id), None)
            self.records.append(make_record(schema_path=self.schema_path, now=self.now, base=self.base, record_kind="call", chunk=chunk, node=item["node"], edge_kind="call", target_name=target, resolution_status="unresolved", occurrence_count=item["count"]))
        for chunk_id, item in self.references.items():
            chunk = next((candidate for candidate in self.chunks if candidate["chunk_id"] == chunk_id), None)
            top_names = dict(sorted(item["names"].items(), key=lambda pair: (-pair[1], pair[0]))[:128])
            self.records.append(make_record(schema_path=self.schema_path, now=self.now, base=self.base, record_kind="reference", chunk=chunk, node=item["node"], edge_kind="name_load_summary", name="symbol_reference_summary", resolution_status="unresolved", occurrence_count=item["count"], details={"unique_name_count": len(item["names"]), "top_names": top_names}))
        return self.records


def records_for_parse_record(record: dict[str, Any], schema_path: Path, now: str, chunk_index: dict[tuple[str, str, str, str], list[dict[str, Any]]], module_index: dict[tuple[str, str, str], dict[str, str]]) -> list[dict[str, Any]]:
    base = {**record, "module_name": module_name_for_path(record["relative_path"])}
    if record["parse_status"] != "parsed":
        return [make_record(schema_path=schema_path, now=now, base=base, record_kind="uncertainty", chunk=None, node=None, edge_kind="parse_unavailable", resolution_status="unresolved", uncertainty_reason=record["parse_status"])]
    path = Path(record["absolute_path"])
    raw = path.read_bytes()
    if "sha256:" + hashlib.sha256(raw).hexdigest() != record["file_sha256"]:
        raise StaticGraphError("PY_STATIC_GRAPH_SOURCE_HASH_MISMATCH", f"source file hash changed: {path}", "Regenerate AST parse records before graph extraction.", {"path": str(path)})
    encoding, decoded, error = detect_and_decode(raw)
    if error is not None or decoded is None:
        raise StaticGraphError("PY_STATIC_GRAPH_DECODE_FAILED", f"source file no longer decodes: {path}", "Regenerate AST parse records before graph extraction.", {"path": str(path), "error": error})
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always", SyntaxWarning)
        tree = ast.parse(decoded, filename=str(path))
    key = (record["task_instance_id"], record["workspace_state"], record["mutation_category"], record["relative_path"])
    mod_key = (record["task_instance_id"], record["workspace_state"], record["mutation_category"])
    visitor = StaticVisitor(schema_path=schema_path, now=now, base=base, chunks=chunk_index.get(key, []), modules=module_index.get(mod_key, {}))
    visitor.visit(tree)
    records = visitor.finish()
    for warning in caught:
        records.append(
            make_record(
                schema_path=schema_path,
                now=now,
                base=base,
                record_kind="uncertainty",
                chunk=None,
                node=None,
                edge_kind="parse_syntax_warning",
                resolution_status="unresolved",
                uncertainty_reason="syntax_warning",
                details={"message": str(warning.message), "category": warning.category.__name__, "lineno": warning.lineno},
            )
        )
    return records


def add_test_dependencies(records: list[dict[str, Any]], schema_path: Path, now: str) -> None:
    fixtures: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
    tests = []
    for rec in records:
        key = (rec["task_instance_id"], rec["workspace_state"], rec["mutation_category"], rec["repo"])
        if rec["record_kind"] == "fixture":
            fixtures.setdefault(key, []).append(rec)
        if rec["record_kind"] == "symbol" and rec["symbol_kind"] == "test_function":
            tests.append(rec)
    for test in tests:
        params = test["details"].get("parameters", [])
        key = (test["task_instance_id"], test["workspace_state"], test["mutation_category"], test["repo"])
        for fixture in fixtures.get(key, []):
            if fixture["name"] in params:
                base = {**test, "module_name": test["module_name"]}
                records.append(make_record(schema_path=schema_path, now=now, base=base, record_kind="test_dependency", chunk={"chunk_id": test["chunk_id"], "chunk_kind": test["chunk_kind"], "qualname": test["chunk_qualname"]}, node=None, edge_kind="uses_fixture", name=test["name"], target_name=fixture["name"], target_chunk_id=fixture["chunk_id"], target_relative_path=fixture["relative_path"], resolution_status="local"))


def add_circular_imports(records: list[dict[str, Any]], schema_path: Path, now: str) -> None:
    imports = [r for r in records if r["record_kind"] == "import" and r["resolution_status"] == "internal_module" and r["module_name"] and r["target_module"]]
    edges = {(r["task_instance_id"], r["workspace_state"], r["mutation_category"], r["module_name"], r["target_module"].split(".")[0] if "." in r["target_module"] else r["target_module"]): r for r in imports}
    for (task, state, cat, src, dst), rec in list(edges.items()):
        if (task, state, cat, dst, src) in edges and src < dst:
            base = {**rec, "module_name": src}
            records.append(make_record(schema_path=schema_path, now=now, base=base, record_kind="uncertainty", chunk=None, node=None, edge_kind="circular_import", target_module=dst, resolution_status="cycle", uncertainty_reason="mutual_static_import"))


def build_static_records(parse_records: list[dict[str, Any]], chunks: list[dict[str, Any]], schema_path: Path, now: str) -> list[dict[str, Any]]:
    chunk_index = build_chunk_index(chunks)
    module_index = build_module_index(parse_records)
    records: list[dict[str, Any]] = []
    for record in parse_records:
        if record["parse_status"] in {"parsed", "empty", "syntax_error", "encoding_error"}:
            records.extend(records_for_parse_record(record, schema_path, now, chunk_index, module_index))
    add_test_dependencies(records, schema_path, now)
    add_circular_imports(records, schema_path, now)
    return records
