from __future__ import annotations

import ast
import hashlib
import json
from pathlib import Path
from typing import Any

from .core import utc_now, write_json_checked
from .oracle_events import (
    assert_hot_root_allowed,
    capacity_snapshot,
    file_state,
    load_jsonschema_validator,
    path_bytes,
    sha256_file,
    write_jsonl_checked,
)
from .python_test_intent_core import (
    SCHEMA_VERSION,
    TASK_ID,
    TestIntentError,
    build_file_records,
    cleaned_index,
    read_json,
    read_jsonl,
    test_intent_schema,
)


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    return sum(1 for line in path.read_text(encoding="utf-8").splitlines() if line.strip())


def summarize(records: list[dict[str, Any]]) -> dict[str, Any]:
    by_kind: dict[str, int] = {}
    by_transition: dict[str, int] = {}
    by_task: dict[str, int] = {}
    for record in records:
        by_kind[record["record_kind"]] = by_kind.get(record["record_kind"], 0) + 1
        by_task[record["task_instance_id"]] = by_task.get(record["task_instance_id"], 0) + 1
        if record["target_transition"]:
            by_transition[record["target_transition"]] = by_transition.get(record["target_transition"], 0) + 1
    return {"total": len(records), "by_record_kind": by_kind, "by_target_transition": by_transition, "by_task_instance_id": by_task}


def validate_records(records: list[dict[str, Any]], validator: Any) -> None:
    for record in records:
        errors = sorted(validator.iter_errors(record), key=lambda err: list(err.path))
        if errors:
            raise TestIntentError(
                "PY_TEST_INTENT_SCHEMA_VALIDATION_FAILED",
                "generated test-intent record failed strict JSON Schema validation",
                "Fix record construction before writing reward data.",
                {"intent_id": record.get("intent_id"), "errors": [err.message for err in errors[:10]]},
            )


def test_patch_states(workspace_manifest: dict[str, Any]) -> list[dict[str, Any]]:
    states = []
    for state in workspace_manifest.get("states", []):
        if any(patch.get("label") == "test" for patch in state.get("applied_patches", [])):
            states.append(state)
    if not states:
        raise TestIntentError("PY_TEST_INTENT_NO_TEST_PATCH_STATES", "workspace manifest has no test patch states", "Run TASK-PY-DATA-006 before extracting test intent.", {})
    return states


def test_diff_maps(path: Path) -> list[dict[str, Any]]:
    maps = [row for row in read_jsonl(path, "PY_TEST_INTENT_DIFF_MAP_INVALID") if row.get("patch_label") == "test"]
    if not maps:
        raise TestIntentError("PY_TEST_INTENT_NO_TEST_DIFF_MAPS", "diff map artifact has no test-patch records", "Run TASK-PY-DATA-011 before extracting test intent.", {"path": str(path)})
    return maps


def group_maps(maps: list[dict[str, Any]]) -> dict[tuple[str, str, str, str], list[dict[str, Any]]]:
    grouped: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {}
    for row in maps:
        rel = row.get("file_new_path")
        if not rel:
            raise TestIntentError("PY_TEST_INTENT_TEST_MAP_PATH_MISSING", "test diff map has no file_new_path", "Fix TASK-PY-DATA-011 output before continuing.", {"mapping_id": row.get("mapping_id")})
        key = (row["task_instance_id"], row["workspace_state"], row["mutation_category"], rel)
        grouped.setdefault(key, []).append(row)
    return grouped


def selected_chunks(chunks_path: Path, keys: set[tuple[str, str, str, str]]) -> dict[tuple[str, str, str, str], list[dict[str, Any]]]:
    found: dict[tuple[str, str, str, str], list[dict[str, Any]]] = {key: [] for key in keys}
    with chunks_path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise TestIntentError("PY_TEST_INTENT_CHUNKS_JSONL_INVALID", "AST chunk JSONL contains invalid JSON", "Regenerate TASK-PY-DATA-010.", {"path": str(chunks_path), "line_number": line_number, "error": str(exc)}) from exc
            try:
                key = (row["task_instance_id"], row["workspace_state"], row["mutation_category"], row["relative_path"])
            except KeyError as exc:
                raise TestIntentError("PY_TEST_INTENT_CHUNK_FIELD_MISSING", "AST chunk JSONL row is missing a required key", "Regenerate TASK-PY-DATA-010 before extracting test intent.", {"path": str(chunks_path), "line_number": line_number, "missing": str(exc)}) from exc
            if key in found:
                found[key].append(row)
    missing = [key for key, rows in found.items() if not rows]
    if missing:
        raise TestIntentError("PY_TEST_INTENT_SELECTED_CHUNKS_MISSING", "test diff map references files without AST chunks", "Regenerate AST chunks and diff maps from the same workspace manifest.", {"missing": [list(key) for key in missing[:10]], "missing_count": len(missing)})
    return found


def build_real_records(args: Any, schema_path: Path, now: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    cleaned = read_json(args.cleaned_manifest, "PY_TEST_INTENT_CLEANED_MANIFEST_INVALID")
    rows = cleaned_index(cleaned)
    workspace = read_json(args.workspace_manifest, "PY_TEST_INTENT_WORKSPACE_MANIFEST_INVALID")
    states = test_patch_states(workspace)
    state_by_key = {(s["task_id"], s["state"], s["mutation_category"]): s for s in states}
    maps = test_diff_maps(args.diff_map_records)
    by_file = group_maps(maps)
    chunks = selected_chunks(args.chunks_jsonl, set(by_file))
    records: list[dict[str, Any]] = []
    for key, file_maps in sorted(by_file.items()):
        task_id, state_name, mutation_category, rel = key
        state = state_by_key.get((task_id, state_name, mutation_category))
        if state is None:
            raise TestIntentError("PY_TEST_INTENT_STATE_FOR_MAP_MISSING", "test diff map does not link to a workspace test state", "Regenerate diff maps from the active workspace manifest.", {"key": list(key)})
        row = rows.get(state["row_id"])
        if row is None:
            raise TestIntentError("PY_TEST_INTENT_ROW_FOR_STATE_MISSING", "workspace state does not link to cleaned manifest row", "Regenerate cleaned manifest/workspace manifest pair.", {"row_id": state["row_id"]})
        records.extend(build_file_records(state=state, row=row, file_path=Path(state["hot_workspace_path"]) / rel, relative_path=rel, maps=file_maps, chunks=chunks[key], schema_path=schema_path, now=now))
    source_audit = {
        "cleaned_manifest_rows": len(rows),
        "workspace_state_count": len(workspace.get("states", [])),
        "test_patch_state_count": len(states),
        "test_diff_map_count": len(maps),
        "selected_chunk_file_count": len(chunks),
    }
    return records, source_audit


def write_edge_file(root: Path, name: str, text: str) -> tuple[Path, str]:
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path, "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def synthetic_edge_records(schema_path: Path, hot_root: Path, now: str) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    root = hot_root / "edge-test-intent"
    text = (
        "import pytest\n\n"
        "@pytest.fixture\n"
        "def thing():\n"
        "    return 1\n\n"
        "@pytest.mark.parametrize(\"value,expected\", [(1, 2), (2, 3)])\n"
        "def test_param(value, expected):\n"
        "    assert value + 1 == expected\n\n"
        "def test_raises():\n"
        "    with pytest.raises(ValueError):\n"
        "        int(\"x\")\n\n"
        "@pytest.mark.xfail(reason=\"synthetic known failure\")\n"
        "def test_xfail():\n"
        "    assert thing() == 2\n\n"
        "@pytest.mark.skip(reason=\"synthetic skip\")\n"
        "def test_skip():\n"
        "    assert thing() == 1\n"
    )
    file_path, file_hash = write_edge_file(root, "tests/test_synthetic_intent.py", text)
    patch_path, patch_hash = write_edge_file(root, "synthetic.patch", "diff --git a/tests/test_synthetic_intent.py b/tests/test_synthetic_intent.py\n--- /dev/null\n+++ b/tests/test_synthetic_intent.py\n@@ -0,0 +1,21 @@\n" + "\n".join("+" + line for line in text.splitlines()) + "\n")
    tree = ast.parse(text)
    chunks = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            chunks.append({"file_sha256": file_hash})
    state = {
        "task_id": "synthetic_test_intent",
        "row_id": "synthetic_test_intent::edge",
        "repo": "synthetic/test-intent",
        "mutation_category": "edge_case",
        "state": "test_patch",
        "result_tree_hash": "4" * 40,
        "hot_workspace_path": str(root),
        "applied_patches": [{"label": "test", "patch": {"path": str(patch_path), "sha256": patch_hash}}],
    }
    row = {
        "row_id": state["row_id"],
        "expected_labels": {
            "fail_to_pass": ["tests/test_synthetic_intent.py::test_param[1-2]"],
            "pass_to_pass": ["tests/test_synthetic_intent.py::test_skip"],
        },
        "oracle": {"tests_status": {"FAIL_TO_PASS": {"success": ["tests/test_synthetic_intent.py::test_param[1-2]"]}}},
    }
    maps = [
        {
            "mapping_id": "sha256:" + "5" * 64,
            "new_line_span": {"start": 1, "end": len(text.splitlines())},
            "primary_chunk_ids": ["sha256:" + "6" * 64],
            "related_production_chunk_ids": ["sha256:" + "7" * 64],
        }
    ]
    records = build_file_records(state=state, row=row, file_path=file_path, relative_path="tests/test_synthetic_intent.py", maps=maps, chunks=chunks, schema_path=schema_path, now=now)
    cases = [
        {"case": "parametrized_test", "pass": any(r["record_kind"] == "parametrization" and r["parametrize_names"] == ["value", "expected"] for r in records)},
        {"case": "expected_exception", "pass": any(r["record_kind"] == "expected_exception" and r["expected_exception"] == "ValueError" for r in records)},
        {"case": "xfail_or_skip", "pass": any(r["record_kind"] == "skip_xfail" and r["marker_name"] == "xfail" for r in records) and any(r["record_kind"] == "skip_xfail" and r["marker_name"] == "skip" for r in records)},
        {"case": "fixture_record", "pass": any(r["record_kind"] == "fixture" and r["test_name"] == "thing" for r in records)},
    ]
    return records, cases


def command_test_intent(args: Any) -> dict[str, Any]:
    checked_at = utc_now()
    assert_hot_root_allowed(args.hot_root)
    args.intent_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    schema_path = args.intent_root / "python_test_intent_schema.json"
    records_path = args.intent_root / "python_test_intent_records.jsonl"
    edge_path = args.intent_root / "edge_case_test_intent_records.jsonl"
    manifest_path = args.intent_root / "python_test_intent_manifest.json"
    readback_path = args.readback_root / "python_test_intent_readback.json"
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
        "hot_budget_bytes": args.hot_budget_bytes,
        "intent_root_bytes": path_bytes(args.intent_root),
    }
    schema = test_intent_schema(schema_path)
    validator = load_jsonschema_validator(schema)
    write_json_checked(schema_path, schema)
    now = utc_now()
    records, source_audit = build_real_records(args, schema_path, now)
    validate_records(records, validator)
    record_store = write_jsonl_checked(records_path, records)
    edge_records, edge_cases = synthetic_edge_records(schema_path, args.hot_root, now)
    validate_records(edge_records, validator)
    edge_store = write_jsonl_checked(edge_path, edge_records)
    if path_bytes(args.hot_root) > args.hot_budget_bytes:
        raise TestIntentError("PY_TEST_INTENT_HOT_BUDGET_EXCEEDED", "test-intent hot staging exceeded its explicit byte budget", "Reduce hot staging volume or increase budget with capacity evidence.", {"hot_root": str(args.hot_root), "actual_bytes": path_bytes(args.hot_root), "budget_bytes": args.hot_budget_bytes})
    summary = summarize(records)
    happy_path = {
        "requests_hook_assertion": any(r["record_kind"] == "assertion" and r["task_instance_id"] == "psf__requests-863" and "assert callable(h) is True" in r["source_text"] and r["production_chunk_ids"] for r in records),
        "astropy_fail_to_pass_labels": any(r["record_kind"] == "target_label" and r["task_instance_id"] == "astropy__astropy-12907" and r["target_transition"] == "FAIL_TO_PASS" for r in records),
        "real_records_link_chunks": any(r["chunk_ids"] and r["production_chunk_ids"] for r in records),
    }
    if not all(happy_path.values()) or not all(case["pass"] for case in edge_cases):
        raise TestIntentError("PY_TEST_INTENT_READBACK_FAILED", "test-intent READBACK checks failed", "Fix extractor before using test intent as reward data.", {"happy_path": happy_path, "edge_cases": edge_cases})
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "created_at_utc": checked_at,
        "task_id": TASK_ID,
        "issue": 196,
        "source_of_truth": {"schema": str(schema_path), "records_jsonl": str(records_path), "edge_records_jsonl": str(edge_path), "manifest": str(manifest_path), "readback": str(readback_path)},
        "inputs": {"cleaned_manifest": str(args.cleaned_manifest), "workspace_manifest": str(args.workspace_manifest), "chunks_jsonl": str(args.chunks_jsonl), "diff_map_records": str(args.diff_map_records)},
        "storage_policy_decision": 212,
        "source_audit": source_audit,
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
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "hot_budget_bytes": args.hot_budget_bytes,
        "intent_root_bytes": path_bytes(args.intent_root),
    }
    readback = {
        "schema_version": 1,
        "checked_at_utc": utc_now(),
        "task_id": TASK_ID,
        "issue": 196,
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py test-intent", "schema_validation": "jsonschema Draft 2020-12, additionalProperties=false"},
        "after_state": after_state,
        "happy_path": happy_path,
        "edge_cases": edge_cases,
        "source_audit": source_audit,
        "summary": summary,
        "passes": True,
    }
    write_json_checked(readback_path, readback)
    return {
        "ok": True,
        "checked_at": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": readback["trigger"],
        "after_state": after_state,
        "post_write_readback": {"readback": file_state(readback_path), "records": file_state(records_path), "manifest": file_state(manifest_path), "readback_root_bytes": path_bytes(args.readback_root)},
        "happy_path": happy_path,
        "edge_cases": edge_cases,
        "summary": summary,
        "record_store": record_store,
        "edge_store": edge_store,
        "manifest_sha256": sha256_file(manifest_path),
        "readback_sha256": sha256_file(readback_path),
        "passes": True,
    }
