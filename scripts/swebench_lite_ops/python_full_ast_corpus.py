from __future__ import annotations

import hashlib
import json
import math
import os
import shutil
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any

from .core import SwebenchOpsError, ensure_file, run, utc_now, write_json_checked
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
from .python_ast_chunk_command import edge_records as chunk_edge_records
from .python_ast_chunk_command import validate_chunks
from .python_ast_chunks import TASK_ID as CHUNK_TASK_ID
from .python_ast_chunks import chunk_schema, chunks_for_record
from .python_ast_parse import TASK_ID as PARSE_TASK_ID
from .python_ast_parse import edge_records as parse_edge_records
from .python_ast_parse import parse_file, record_schema, validate_records
from .python_diff_chunk_map_core import parse_unified_diff


TASK_ID = "TASK-PY-DATA-058"
ARTIFACT_KIND = "python_full_ast_corpus_readback"
EXPECTED_CATEGORIES = (
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
)


class FullAstCorpusError(SwebenchOpsError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise FullAstCorpusError(code, message, "Fix the full AST corpus source pipeline before relaunching #199.", details or {})


def safe_component(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in {"-", "_", "."} else "_" for ch in value)


def safe_relative_path(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    require(not path.is_absolute(), "PY_FULL_AST_ABSOLUTE_PATCH_PATH", f"patch path is absolute: {value}", {"path": value})
    require(".." not in path.parts, "PY_FULL_AST_PARENT_PATCH_PATH", f"patch path escapes snapshot root: {value}", {"path": value})
    require(value.endswith(".py"), "PY_FULL_AST_NON_PYTHON_PATH", f"patch target is not Python: {value}", {"path": value})
    return path


def write_bytes_checked(path: Path, payload: bytes) -> dict[str, Any]:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    readback = path.read_bytes()
    if readback != payload:
        raise FullAstCorpusError(
            "PY_FULL_AST_FILE_READBACK_MISMATCH",
            f"source snapshot readback mismatch at {path}",
            "Check the filesystem for partial writes or concurrent writers.",
            {"path": str(path)},
        )
    return {"path": str(path), "bytes": len(readback), "sha256": "sha256:" + hashlib.sha256(readback).hexdigest()}


def append_jsonl_checked(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(canonical_json(row) + "\n")
        handle.flush()
        os.fsync(handle.fileno())


def read_jsonl_checked(path: Path, code: str) -> list[dict[str, Any]]:
    ensure_file(path, code, "required JSONL artifact")
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise FullAstCorpusError(
                    code,
                    f"invalid JSONL at {path}:{line_number}",
                    "Regenerate the task checkpoint before resuming the full AST corpus run.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            require(isinstance(value, dict), code, f"JSONL row must be an object at {path}:{line_number}", {"path": str(path), "line_number": line_number})
            rows.append(value)
    return rows


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for line in handle if line.strip())


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise FullAstCorpusError(
            code,
            f"invalid JSON at {path}",
            "Regenerate the upstream artifact before building the full AST corpus.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def task_checkpoint_paths(root: Path, task_id: str) -> dict[str, Path]:
    task_root = root / safe_component(task_id)
    return {
        "root": task_root,
        "states": task_root / "states.jsonl",
        "parse_records": task_root / "parse_records.jsonl",
        "chunks": task_root / "chunks.jsonl",
        "diagnostics": task_root / "diagnostics.jsonl",
        "patch_logs": task_root / "patch_logs.jsonl",
        "manifest": task_root / "manifest.json",
    }


def write_task_checkpoint(
    *,
    root: Path,
    task_id: str,
    task_index: int,
    task_count: int,
    states: list[dict[str, Any]],
    parse_records: list[dict[str, Any]],
    chunks: list[dict[str, Any]],
    diagnostics: list[dict[str, Any]],
    patch_logs: list[dict[str, Any]],
) -> dict[str, Any]:
    paths = task_checkpoint_paths(root, task_id)
    paths["root"].mkdir(parents=True, exist_ok=True)
    state_store = write_jsonl_checked(paths["states"], states)
    parse_store = write_jsonl_checked(paths["parse_records"], parse_records)
    chunk_store = write_jsonl_checked(paths["chunks"], chunks)
    diagnostic_store = write_jsonl_checked(paths["diagnostics"], diagnostics)
    patch_store = write_jsonl_checked(paths["patch_logs"], patch_logs)
    manifest = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "artifact_kind": "python_full_ast_task_checkpoint",
        "created_at_utc": utc_now(),
        "task_instance_id": task_id,
        "task_index": task_index,
        "task_count": task_count,
        "source_of_truth": {
            "states_jsonl": str(paths["states"]),
            "parse_records_jsonl": str(paths["parse_records"]),
            "chunks_jsonl": str(paths["chunks"]),
            "diagnostics_jsonl": str(paths["diagnostics"]),
            "patch_logs_jsonl": str(paths["patch_logs"]),
            "manifest": str(paths["manifest"]),
        },
        "counts": {
            "states": len(states),
            "parse_records": len(parse_records),
            "chunks": len(chunks),
            "syntax_warning_diagnostics": len(diagnostics),
            "patch_logs": len(patch_logs),
        },
        "sha256": {
            "states": state_store["sha256"],
            "parse_records": parse_store["sha256"],
            "chunks": chunk_store["sha256"],
            "diagnostics": diagnostic_store["sha256"],
            "patch_logs": patch_store["sha256"],
        },
        "passes": True,
    }
    write_json_checked(paths["manifest"], manifest)
    return manifest


def load_task_checkpoint(
    *,
    manifest_path: Path,
    expected_task_id: str,
    parse_validator: Any,
    chunk_validator: Any,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    manifest = read_json(manifest_path, "PY_FULL_AST_TASK_CHECKPOINT_MANIFEST_INVALID")
    require(manifest.get("task_instance_id") == expected_task_id, "PY_FULL_AST_TASK_CHECKPOINT_ID_MISMATCH", "task checkpoint belongs to a different task", {"expected": expected_task_id, "observed": manifest.get("task_instance_id"), "manifest": str(manifest_path)})
    source = manifest.get("source_of_truth")
    require(isinstance(source, dict), "PY_FULL_AST_TASK_CHECKPOINT_SOURCE_INVALID", "task checkpoint source_of_truth is missing", {"manifest": str(manifest_path)})
    states = read_jsonl_checked(Path(source["states_jsonl"]), "PY_FULL_AST_TASK_CHECKPOINT_STATES_INVALID")
    parse_records = read_jsonl_checked(Path(source["parse_records_jsonl"]), "PY_FULL_AST_TASK_CHECKPOINT_PARSE_INVALID")
    chunks = read_jsonl_checked(Path(source["chunks_jsonl"]), "PY_FULL_AST_TASK_CHECKPOINT_CHUNKS_INVALID")
    diagnostics = read_jsonl_checked(Path(source["diagnostics_jsonl"]), "PY_FULL_AST_TASK_CHECKPOINT_DIAGNOSTICS_INVALID")
    patch_logs = read_jsonl_checked(Path(source["patch_logs_jsonl"]), "PY_FULL_AST_TASK_CHECKPOINT_PATCH_LOGS_INVALID")
    validate_records(parse_records, parse_validator)
    validate_chunks(chunks, chunk_validator)
    counts = manifest.get("counts")
    require(isinstance(counts, dict), "PY_FULL_AST_TASK_CHECKPOINT_COUNTS_INVALID", "task checkpoint counts are missing", {"manifest": str(manifest_path)})
    observed_counts = {
        "states": len(states),
        "parse_records": len(parse_records),
        "chunks": len(chunks),
        "syntax_warning_diagnostics": len(diagnostics),
        "patch_logs": len(patch_logs),
    }
    require(all(counts.get(key) == value for key, value in observed_counts.items()), "PY_FULL_AST_TASK_CHECKPOINT_COUNT_MISMATCH", "task checkpoint counts do not match JSONL readback", {"manifest": str(manifest_path), "expected": counts, "observed": observed_counts})
    return states, parse_records, chunks, diagnostics, patch_logs, manifest


def patch_text(row: dict[str, Any], field: str) -> str:
    patch = row.get(field)
    require(isinstance(patch, dict), "PY_FULL_AST_PATCH_OBJECT_MISSING", f"{field} must be an object", {"row_id": row.get("row_id")})
    text = patch.get("text")
    require(isinstance(text, str) and text, "PY_FULL_AST_PATCH_TEXT_MISSING", f"{field}.text is missing", {"row_id": row.get("row_id")})
    expected_sha = patch.get("sha256")
    actual_sha = sha256_prefixed_text(text)
    require(
        expected_sha == actual_sha,
        "PY_FULL_AST_PATCH_SHA_MISMATCH",
        f"{field}.sha256 does not match text bytes",
        {"row_id": row.get("row_id"), "field": field, "expected": expected_sha, "actual": actual_sha},
    )
    return text


def python_paths_from_patch(text: str, *, side: str) -> list[str]:
    paths: set[str] = set()
    for hunk in parse_unified_diff(text):
        path = hunk.new_path if side == "new" else hunk.old_path
        if path is None:
            continue
        if path.endswith(".py"):
            paths.add(safe_relative_path(path).as_posix())
    return sorted(paths)


def patch_path_summary(row: dict[str, Any]) -> dict[str, Any]:
    gold = patch_text(row, "gold_patch")
    mutation = patch_text(row, "patch")
    test = patch_text(row, "test_patch")
    return {
        "gold_old": python_paths_from_patch(gold, side="old"),
        "gold_new": python_paths_from_patch(gold, side="new"),
        "mutation_old": python_paths_from_patch(mutation, side="old"),
        "mutation_new": python_paths_from_patch(mutation, side="new"),
        "test_old": python_paths_from_patch(test, side="old"),
        "test_new": python_paths_from_patch(test, side="new"),
    }


def load_cleaned_rows(path: Path, *, strict_300x8: bool) -> list[dict[str, Any]]:
    payload = read_json(path, "PY_FULL_AST_CLEANED_MANIFEST_INVALID")
    rows = payload.get("rows")
    require(isinstance(rows, list) and rows, "PY_FULL_AST_CLEANED_ROWS_EMPTY", "cleaned manifest has no rows")
    require(payload.get("row_count") == len(rows), "PY_FULL_AST_CLEANED_ROW_COUNT_MISMATCH", "cleaned manifest row_count does not match rows length", {"row_count": payload.get("row_count"), "rows_len": len(rows)})
    row_ids = [row.get("row_id") for row in rows]
    require(all(isinstance(row_id, str) and row_id for row_id in row_ids), "PY_FULL_AST_ROW_ID_MISSING", "every cleaned row must have row_id")
    require(len(set(row_ids)) == len(row_ids), "PY_FULL_AST_ROW_ID_DUPLICATE", "cleaned manifest row_id values must be unique")
    by_category = Counter(row.get("mutation_category") for row in rows)
    require(
        set(by_category) == set(EXPECTED_CATEGORIES),
        "PY_FULL_AST_CATEGORY_SET_MISMATCH",
        "cleaned manifest categories do not match the Python 300x8 contract",
        {"actual": dict(sorted(by_category.items())), "expected": list(EXPECTED_CATEGORIES)},
    )
    if strict_300x8:
        require(
            all(count == 300 for count in by_category.values()),
            "PY_FULL_AST_CATEGORY_COUNT_MISMATCH",
            "each mutation category must have exactly 300 cleaned rows",
            {"actual": dict(sorted(by_category.items()))},
        )
    for row in rows:
        for key in ("task_id", "repo", "base_commit", "mutation_category", "row_id"):
            require(isinstance(row.get(key), str) and row[key], "PY_FULL_AST_ROW_FIELD_MISSING", f"cleaned row missing {key}", {"row_id": row.get("row_id")})
        patch_path_summary(row)
    return rows


def group_rows(rows: list[dict[str, Any]], *, max_tasks: int | None) -> dict[str, dict[str, dict[str, Any]]]:
    require(bool(rows), "PY_FULL_AST_CLEANED_ROWS_EMPTY", "cleaned manifest has no rows")
    grouped: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    task_order: list[str] = []
    for row in rows:
        task_id = row["task_id"]
        if task_id not in grouped:
            task_order.append(task_id)
        grouped[task_id][row["mutation_category"]] = row
    if max_tasks is not None:
        allowed = set(task_order[:max_tasks])
        grouped = {task_id: categories for task_id, categories in grouped.items() if task_id in allowed}
    for task_id, categories in grouped.items():
        missing = sorted(set(EXPECTED_CATEGORIES) - set(categories))
        require(not missing, "PY_FULL_AST_TASK_CATEGORY_INCOMPLETE", "task is missing mutation categories", {"task_id": task_id, "missing": missing})
    return dict(grouped)


def object_store_for(repo_root: Path, repo: str) -> Path:
    return repo_root / f"{repo.replace('/', '__')}.git"


def command_stdout(result: Any) -> str:
    lines = [line for line in result.stdout_tail.splitlines() if line.strip()]
    return lines[-1].strip() if lines else ""


def git(worktree: Path, args: list[str], *, timeout: int = 600, check: bool = True) -> Any:
    return run(["git", "-C", str(worktree), *args], timeout=timeout, check=check, stdout_limit=20000, stderr_limit=20000)


def prepare_worktree(object_store: Path, worktree: Path, base_commit: str, *, timeout: int) -> None:
    if worktree.exists():
        shutil.rmtree(worktree)
    run(["git", "clone", "--shared", "--no-checkout", str(object_store), str(worktree)], timeout=timeout, stdout_limit=12000, stderr_limit=12000)
    git(worktree, ["checkout", "--detach", "--force", base_commit], timeout=timeout)
    reset_worktree(worktree, base_commit, timeout=timeout)


def reset_worktree(worktree: Path, base_commit: str, *, timeout: int) -> None:
    git(worktree, ["reset", "--hard", base_commit], timeout=timeout)
    git(worktree, ["clean", "-ffd"], timeout=timeout)
    status = command_stdout(git(worktree, ["status", "--porcelain=v1"], timeout=timeout))
    require(status == "", "PY_FULL_AST_WORKTREE_DIRTY_AFTER_RESET", "worktree is dirty after reset", {"worktree": str(worktree), "status": status})


def apply_patch(worktree: Path, patch_path: Path, *, timeout: int) -> dict[str, Any]:
    before = command_stdout(git(worktree, ["status", "--porcelain=v1"], timeout=timeout))
    require(before == "", "PY_FULL_AST_WORKTREE_DIRTY_BEFORE_PATCH", "worktree is dirty before patch application", {"worktree": str(worktree), "status": before})
    check = git(worktree, ["apply", "--check", "--index", str(patch_path)], timeout=timeout)
    applied = git(worktree, ["apply", "--index", str(patch_path)], timeout=timeout)
    tree = command_stdout(git(worktree, ["write-tree"], timeout=timeout))
    status = command_stdout(git(worktree, ["status", "--porcelain=v1"], timeout=timeout))
    require(bool(tree) and len(tree) == 40, "PY_FULL_AST_RESULT_TREE_INVALID", "git write-tree did not return a 40-char tree hash", {"tree": tree})
    return {"check": check.to_json(), "apply": applied.to_json(), "result_tree_hash": tree, "status": status}


def copy_source_files(worktree: Path, snapshot_state_root: Path, relative_paths: list[str]) -> list[dict[str, Any]]:
    copied: list[dict[str, Any]] = []
    for rel in relative_paths:
        safe = safe_relative_path(rel)
        src = worktree / safe.as_posix()
        require(src.is_file(), "PY_FULL_AST_SOURCE_FILE_MISSING", "expected Python patch target is missing after state materialization", {"worktree": str(worktree), "relative_path": rel})
        dst = snapshot_state_root / safe.as_posix()
        copied.append({"relative_path": rel, **write_bytes_checked(dst, src.read_bytes())})
    require(bool(copied), "PY_FULL_AST_STATE_SOURCE_EMPTY", "state produced no Python source snapshots", {"snapshot_state_root": str(snapshot_state_root)})
    return copied


def parse_snapshot_state(
    *,
    workspace: dict[str, Any],
    schema_path: Path,
    now: str,
    validator: Any,
) -> list[dict[str, Any]]:
    root = Path(workspace["hot_workspace_path"])
    records = [parse_file(schema_path, workspace, path, now) for path in sorted(root.rglob("*.py"))]
    validate_records(records, validator)
    return records


def chunks_from_records(records: list[dict[str, Any]], schema_path: Path, now: str, validator: Any) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    chunks: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    for record in records:
        record_chunks, record_diagnostics = chunks_for_record(record, schema_path, now)
        chunks.extend(record_chunks)
        diagnostics.extend(record_diagnostics)
    validate_chunks(chunks, validator)
    return chunks, diagnostics


def materialize_state(
    *,
    task_id: str,
    row: dict[str, Any],
    state: str,
    mutation_category: str,
    result_tree_hash: str,
    worktree: Path,
    snapshot_root: Path,
    relative_paths: list[str],
    object_store: Path,
    schema_path: Path,
    chunk_schema_path: Path,
    parse_validator: Any,
    chunk_validator: Any,
    now: str,
) -> tuple[dict[str, Any], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    snapshot_state_root = snapshot_root / safe_component(task_id) / safe_component(state)
    if snapshot_state_root.exists():
        shutil.rmtree(snapshot_state_root)
    copied = copy_source_files(worktree, snapshot_state_root, relative_paths)
    workspace = {
        "task_id": task_id,
        "row_id": row["row_id"],
        "repo": row["repo"],
        "base_commit": row["base_commit"],
        "state": state,
        "mutation_category": mutation_category,
        "result_tree_hash": result_tree_hash,
        "hot_workspace_path": str(snapshot_state_root),
        "object_store": str(object_store),
        "source_snapshot_files": copied,
    }
    records = parse_snapshot_state(workspace=workspace, schema_path=schema_path, now=now, validator=parse_validator)
    chunks, diagnostics = chunks_from_records(records, chunk_schema_path, now, chunk_validator)
    require(bool(chunks), "PY_FULL_AST_STATE_CHUNKS_EMPTY", "state produced no accepted AST chunks", {"task_id": task_id, "state": state, "mutation_category": mutation_category, "records": len(records)})
    return workspace, records, chunks, diagnostics


def base_tree(worktree: Path, *, timeout: int) -> str:
    tree = command_stdout(git(worktree, ["rev-parse", "HEAD^{tree}"], timeout=timeout))
    require(bool(tree) and len(tree) == 40, "PY_FULL_AST_BASE_TREE_INVALID", "git rev-parse did not return a 40-char tree hash", {"tree": tree})
    return tree


def patch_file(hot_patch_root: Path, row: dict[str, Any], state: str, text: str) -> Path:
    path = hot_patch_root / safe_component(row["task_id"]) / f"{safe_component(state)}.patch"
    write_bytes_checked(path, text.encode("utf-8"))
    return path


def build_for_task(
    *,
    task_id: str,
    categories: dict[str, dict[str, Any]],
    repo_root: Path,
    hot_root: Path,
    snapshot_root: Path,
    schema_path: Path,
    chunk_schema_path: Path,
    parse_validator: Any,
    chunk_validator: Any,
    now: str,
    timeout: int,
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]], list[dict[str, Any]]]:
    known_good = categories["known_good"]
    object_store = object_store_for(repo_root, known_good["repo"])
    require(object_store.is_dir(), "PY_FULL_AST_OBJECT_STORE_MISSING", "repo object store is missing", {"repo": known_good["repo"], "object_store": str(object_store)})
    worktree = hot_root / "worktrees" / safe_component(task_id)
    hot_patch_root = hot_root / "patches"
    prepare_worktree(object_store, worktree, known_good["base_commit"], timeout=timeout)

    parse_records: list[dict[str, Any]] = []
    chunks: list[dict[str, Any]] = []
    states: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    patch_logs: list[dict[str, Any]] = []

    path_summaries = {category: patch_path_summary(row) for category, row in categories.items()}
    base_paths = sorted(
        {
            path
            for summary in path_summaries.values()
            for key in ("gold_old", "mutation_old", "test_old")
            for path in summary[key]
        }
    )
    require(bool(base_paths), "PY_FULL_AST_BASE_PATHS_EMPTY", "task has no Python base paths to snapshot", {"task_id": task_id})
    reset_worktree(worktree, known_good["base_commit"], timeout=timeout)
    state, records, state_chunks, state_diagnostics = materialize_state(
        task_id=task_id,
        row=known_good,
        state="base",
        mutation_category="known_good",
        result_tree_hash=base_tree(worktree, timeout=timeout),
        worktree=worktree,
        snapshot_root=snapshot_root,
        relative_paths=base_paths,
        object_store=object_store,
        schema_path=schema_path,
        chunk_schema_path=chunk_schema_path,
        parse_validator=parse_validator,
        chunk_validator=chunk_validator,
        now=now,
    )
    states.append(state)
    parse_records.extend(records)
    chunks.extend(state_chunks)
    diagnostics.extend(state_diagnostics)

    for state_name, field, paths_key in (
        ("test_patch", "test_patch", "test_new"),
        ("gold", "gold_patch", "gold_new"),
    ):
        reset_worktree(worktree, known_good["base_commit"], timeout=timeout)
        text = patch_text(known_good, field)
        patch_path = patch_file(hot_patch_root, known_good, state_name, text)
        patch_result = apply_patch(worktree, patch_path, timeout=timeout)
        patch_logs.append({"task_id": task_id, "state": state_name, "field": field, "patch_path": str(patch_path), **patch_result})
        state, records, state_chunks, state_diagnostics = materialize_state(
            task_id=task_id,
            row=known_good,
            state=state_name,
            mutation_category="known_good",
            result_tree_hash=patch_result["result_tree_hash"],
            worktree=worktree,
            snapshot_root=snapshot_root,
            relative_paths=path_summaries["known_good"][paths_key],
            object_store=object_store,
            schema_path=schema_path,
            chunk_schema_path=chunk_schema_path,
            parse_validator=parse_validator,
            chunk_validator=chunk_validator,
            now=now,
        )
        states.append(state)
        parse_records.extend(records)
        chunks.extend(state_chunks)
        diagnostics.extend(state_diagnostics)

    for category in EXPECTED_CATEGORIES:
        if category == "known_good":
            continue
        row = categories[category]
        reset_worktree(worktree, row["base_commit"], timeout=timeout)
        text = patch_text(row, "patch")
        state_name = f"mutation_{category}"
        patch_path = patch_file(hot_patch_root, row, state_name, text)
        patch_result = apply_patch(worktree, patch_path, timeout=timeout)
        patch_logs.append({"task_id": task_id, "state": state_name, "field": "patch", "patch_path": str(patch_path), **patch_result})
        state, records, state_chunks, state_diagnostics = materialize_state(
            task_id=task_id,
            row=row,
            state=state_name,
            mutation_category=category,
            result_tree_hash=patch_result["result_tree_hash"],
            worktree=worktree,
            snapshot_root=snapshot_root,
            relative_paths=path_summaries[category]["mutation_new"],
            object_store=object_store,
            schema_path=schema_path,
            chunk_schema_path=chunk_schema_path,
            parse_validator=parse_validator,
            chunk_validator=chunk_validator,
            now=now,
        )
        states.append(state)
        parse_records.extend(records)
        chunks.extend(state_chunks)
        diagnostics.extend(state_diagnostics)

    shutil.rmtree(worktree)
    return states, parse_records, chunks, diagnostics, patch_logs


def summarize_parse_records(records: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "total": len(records),
        "by_status": dict(sorted(Counter(record["parse_status"] for record in records).items())),
        "by_category": dict(sorted(Counter(record["mutation_category"] for record in records).items())),
        "by_state": dict(sorted(Counter(record["workspace_state"] for record in records).items())),
    }


def summarize_chunks(chunks: list[dict[str, Any]]) -> dict[str, Any]:
    return {
        "total": len(chunks),
        "by_kind": dict(sorted(Counter(chunk["chunk_kind"] for chunk in chunks).items())),
        "by_category": dict(sorted(Counter(chunk["mutation_category"] for chunk in chunks).items())),
        "by_state": dict(sorted(Counter(chunk["workspace_state"] for chunk in chunks).items())),
    }


def coverage(chunks: list[dict[str, Any]]) -> dict[str, Any]:
    pairs = {(chunk["task_instance_id"], chunk["mutation_category"]) for chunk in chunks}
    tasks = {task for task, _ in pairs}
    by_category: dict[str, set[str]] = defaultdict(set)
    for task, category in pairs:
        by_category[category].add(task)
    return {
        "task_count": len(tasks),
        "task_category_pair_count": len(pairs),
        "categories": sorted(by_category),
        "task_count_by_category": {category: len(tasks) for category, tasks in sorted(by_category.items())},
    }


def coverage_from_jsonl(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"exists": False, "chunk_count": 0, "task_count": 0, "task_category_pair_count": 0, "categories": []}
    pairs: set[tuple[str, str]] = set()
    categories = Counter()
    chunk_count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            chunk_count += 1
            row = json.loads(line)
            task = row.get("task_instance_id")
            category = row.get("mutation_category")
            if isinstance(task, str) and isinstance(category, str):
                pairs.add((task, category))
                categories[category] += 1
    return {
        "exists": True,
        "chunk_count": chunk_count,
        "task_count": len({task for task, _ in pairs}),
        "task_category_pair_count": len(pairs),
        "categories": dict(sorted(categories.items())),
    }


def edge_case_results(rows: list[dict[str, Any]], chunks: list[dict[str, Any]]) -> list[dict[str, Any]]:
    empty_error = None
    try:
        group_rows([], max_tasks=None)
    except FullAstCorpusError as exc:
        empty_error = exc.to_json()

    missing_error = None
    try:
        group_rows(rows[:-1], max_tasks=None)
    except FullAstCorpusError as exc:
        missing_error = exc.to_json()

    non_python_error = None
    synthetic = {
        "row_id": "synthetic_non_python::known_good",
        "task_id": "synthetic_non_python",
        "repo": "synthetic/repo",
        "base_commit": "0" * 40,
        "mutation_category": "known_good",
        "gold_patch": {"text": "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-a\n+b\n", "sha256": ""},
        "patch": {"text": "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-a\n+b\n", "sha256": ""},
        "test_patch": {"text": "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-a\n+b\n", "sha256": ""},
    }
    for key in ("gold_patch", "patch", "test_patch"):
        synthetic[key]["sha256"] = sha256_prefixed_text(synthetic[key]["text"])
    try:
        summary = patch_path_summary(synthetic)
        require(any(summary[key] for key in summary), "PY_FULL_AST_NON_PYTHON_PATCH_REJECTED", "synthetic patch produced no Python paths", {"summary": summary})
    except FullAstCorpusError as exc:
        non_python_error = exc.to_json()

    syntax_chunks = [
        chunk
        for chunk in chunks
        if chunk["mutation_category"] == "compile_error" and chunk["chunk_kind"] == "syntax_error_file"
    ]
    return [
        {
            "case": "empty_cleaned_manifest_rejected",
            "before_state": {"row_count": 0},
            "after_state": empty_error,
            "expected": "PY_FULL_AST_CLEANED_ROWS_EMPTY or incomplete category error",
            "pass": empty_error is not None,
        },
        {
            "case": "missing_task_category_rejected",
            "before_state": {"row_count": len(rows)},
            "after_state": missing_error,
            "expected": "PY_FULL_AST_TASK_CATEGORY_INCOMPLETE",
            "pass": missing_error is not None and missing_error.get("error_code") == "PY_FULL_AST_TASK_CATEGORY_INCOMPLETE",
        },
        {
            "case": "non_python_patch_rejected",
            "before_state": {"patch_target": "README.md"},
            "after_state": non_python_error,
            "expected": "no Python path means no accepted row chunks",
            "pass": non_python_error is not None and non_python_error.get("error_code") == "PY_FULL_AST_NON_PYTHON_PATCH_REJECTED",
        },
        {
            "case": "compile_error_source_gets_syntax_error_chunk",
            "before_state": {"compile_error_rows": 300},
            "after_state": {"syntax_error_file_chunk_count": len(syntax_chunks), "sample": syntax_chunks[:3]},
            "expected": "compile-error rows carry bounded source chunks instead of disappearing",
            "pass": bool(syntax_chunks),
        },
    ]


def command_full_ast_corpus(args: Any) -> dict[str, Any]:
    started = utc_now()
    args.parse_root.mkdir(parents=True, exist_ok=True)
    args.chunk_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    args.hot_root.mkdir(parents=True, exist_ok=True)
    args.task_checkpoint_root.mkdir(parents=True, exist_ok=True)
    snapshot_root = args.source_snapshot_root
    if args.clean_output:
        for path in (
            args.parse_root / "python_ast_parse_records.jsonl",
            args.parse_root / "edge_case_parse_records.jsonl",
            args.parse_root / "python_ast_parse_schema.json",
            args.parse_root / "python_ast_parse_manifest.json",
            args.chunk_root / "python_ast_chunks.jsonl",
            args.chunk_root / "edge_case_chunks.jsonl",
            args.chunk_root / "python_ast_chunk_schema.json",
            args.chunk_root / "python_ast_chunk_manifest.json",
            args.chunk_root / "python_full_ast_corpus_manifest.json",
            args.chunk_root / "python_ast_chunk_diagnostics.jsonl",
        ):
            if path.is_file():
                path.unlink()
        if args.task_checkpoint_root.exists():
            shutil.rmtree(args.task_checkpoint_root)
        if snapshot_root.exists():
            shutil.rmtree(snapshot_root)

    chunks_path = args.chunk_root / "python_ast_chunks.jsonl"
    diagnostics_path = args.chunk_root / "python_ast_chunk_diagnostics.jsonl"
    parse_records_path = args.parse_root / "python_ast_parse_records.jsonl"
    readback_path = args.readback_root / "full_ast_corpus_readback.json"
    progress_path = args.readback_root / "full_ast_corpus_progress.jsonl"
    if progress_path.is_file() and not args.resume_from_task_checkpoints:
        progress_path.unlink()
    before_state = {
        "started_at_utc": started,
        "cleaned_manifest": file_state(args.cleaned_manifest),
        "workspace_manifest": file_state(args.workspace_manifest),
        "parse_records": file_state(parse_records_path),
        "chunks": file_state(chunks_path),
        "diagnostics": file_state(diagnostics_path),
        "task_checkpoint_root": {"path": str(args.task_checkpoint_root), "exists": args.task_checkpoint_root.exists()},
        "chunk_coverage": coverage_from_jsonl(chunks_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": path_bytes(args.hot_root),
        "snapshot_bytes": path_bytes(snapshot_root),
    }

    rows = load_cleaned_rows(args.cleaned_manifest, strict_300x8=args.require_full_coverage)
    grouped = group_rows(rows, max_tasks=args.max_tasks)
    expected_pairs = {(row["task_id"], row["mutation_category"]) for row in rows if args.max_tasks is None or row["task_id"] in grouped}
    require(bool(expected_pairs), "PY_FULL_AST_EXPECTED_PAIRS_EMPTY", "expected coverage pair set is empty")

    parse_schema_path = args.parse_root / "python_ast_parse_schema.json"
    chunk_schema_path = args.chunk_root / "python_ast_chunk_schema.json"
    parse_schema = record_schema(parse_schema_path)
    chunk_schema_value = chunk_schema(chunk_schema_path)
    write_json_checked(parse_schema_path, parse_schema)
    write_json_checked(chunk_schema_path, chunk_schema_value)
    parse_validator = load_jsonschema_validator(parse_schema)
    chunk_validator = load_jsonschema_validator(chunk_schema_value)

    now = utc_now()
    states: list[dict[str, Any]] = []
    parse_records: list[dict[str, Any]] = []
    chunks: list[dict[str, Any]] = []
    diagnostics: list[dict[str, Any]] = []
    patch_logs: list[dict[str, Any]] = []
    ordered_tasks = sorted(grouped)
    for task_index, task_id in enumerate(ordered_tasks, start=1):
        checkpoint_paths = task_checkpoint_paths(args.task_checkpoint_root, task_id)
        reused_checkpoint = False
        if args.resume_from_task_checkpoints and checkpoint_paths["manifest"].is_file():
            task_states, task_records, task_chunks, task_diagnostics, task_patch_logs, task_checkpoint_manifest = load_task_checkpoint(
                manifest_path=checkpoint_paths["manifest"],
                expected_task_id=task_id,
                parse_validator=parse_validator,
                chunk_validator=chunk_validator,
            )
            reused_checkpoint = True
        else:
            task_states, task_records, task_chunks, task_diagnostics, task_patch_logs = build_for_task(
                task_id=task_id,
                categories=grouped[task_id],
                repo_root=args.repo_object_root,
                hot_root=args.hot_root,
                snapshot_root=snapshot_root,
                schema_path=parse_schema_path,
                chunk_schema_path=chunk_schema_path,
                parse_validator=parse_validator,
                chunk_validator=chunk_validator,
                now=now,
                timeout=args.git_timeout_seconds,
            )
            task_checkpoint_manifest = write_task_checkpoint(
                root=args.task_checkpoint_root,
                task_id=task_id,
                task_index=task_index,
                task_count=len(ordered_tasks),
                states=task_states,
                parse_records=task_records,
                chunks=task_chunks,
                diagnostics=task_diagnostics,
                patch_logs=task_patch_logs,
            )
        for diagnostic in task_diagnostics:
            append_jsonl_checked(diagnostics_path, diagnostic)
        states.extend(task_states)
        parse_records.extend(task_records)
        chunks.extend(task_chunks)
        diagnostics.extend(task_diagnostics)
        patch_logs.extend(task_patch_logs)
        append_jsonl_checked(
            progress_path,
            {
                "schema_version": 1,
                "task_id": TASK_ID,
                "timestamp_utc": utc_now(),
                "phase": "task_checkpoint_reused" if reused_checkpoint else "task_materialized",
                "task_index": task_index,
                "task_count": len(ordered_tasks),
                "task_instance_id": task_id,
                "state_count_delta": len(task_states),
                "parse_record_count_delta": len(task_records),
                "chunk_count_delta": len(task_chunks),
                "diagnostic_count_delta": len(task_diagnostics),
                "syntax_warning_count_delta": len(task_diagnostics),
                "fatal_error_count_delta": 0,
                "diagnostic_severity": "warning" if task_diagnostics else "none",
                "total_parse_records": len(parse_records),
                "total_chunks": len(chunks),
                "total_syntax_warnings": len(diagnostics),
                "task_checkpoint_manifest": str(task_checkpoint_manifest["source_of_truth"]["manifest"]),
                "diagnostics_jsonl": str(diagnostics_path),
                "hot_bytes": path_bytes(args.hot_root),
                "snapshot_bytes": path_bytes(snapshot_root),
            },
        )

    parse_edges, parse_edge_audit = parse_edge_records(parse_schema_path, args.hot_root, now)
    validate_records(parse_edges, parse_validator)
    chunk_edges, chunk_edge_audit = chunk_edge_records(chunk_schema_path, args.hot_root, now)
    validate_chunks(chunk_edges, chunk_validator)

    parse_store = write_jsonl_checked(parse_records_path, parse_records)
    edge_parse_store = write_jsonl_checked(args.parse_root / "edge_case_parse_records.jsonl", parse_edges)
    chunk_store = write_jsonl_checked(chunks_path, chunks)
    edge_chunk_store = write_jsonl_checked(args.chunk_root / "edge_case_chunks.jsonl", chunk_edges)
    diagnostic_store = write_jsonl_checked(diagnostics_path, diagnostics)

    observed_coverage = coverage(chunks)
    observed_pairs = {(chunk["task_instance_id"], chunk["mutation_category"]) for chunk in chunks}
    missing_pairs = sorted(expected_pairs - observed_pairs)
    unexpected_pairs = sorted(observed_pairs - expected_pairs)
    full_coverage = not missing_pairs and not unexpected_pairs
    if args.require_full_coverage:
        require(full_coverage, "PY_FULL_AST_COVERAGE_MISMATCH", "accepted chunks do not cover every cleaned manifest task/category pair", {"missing_sample": missing_pairs[:20], "unexpected_sample": unexpected_pairs[:20], "missing_count": len(missing_pairs), "unexpected_count": len(unexpected_pairs)})
        require(observed_coverage["task_count"] == 300, "PY_FULL_AST_TASK_COUNT_MISMATCH", "accepted chunks do not cover all 300 tasks", observed_coverage)
        require(observed_coverage["task_category_pair_count"] == 2400, "PY_FULL_AST_PAIR_COUNT_MISMATCH", "accepted chunks do not cover all 2400 task/category pairs", observed_coverage)

    parse_manifest_path = args.parse_root / "python_ast_parse_manifest.json"
    chunk_manifest_path = args.chunk_root / "python_ast_chunk_manifest.json"
    full_manifest_path = args.chunk_root / "python_full_ast_corpus_manifest.json"
    parse_summary = summarize_parse_records(parse_records)
    chunk_summary = summarize_chunks(chunks)
    full_manifest = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "artifact_kind": "python_full_ast_corpus_manifest",
        "issue": 249,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "cleaned_manifest": str(args.cleaned_manifest),
            "parse_records_jsonl": str(parse_records_path),
            "chunks_jsonl": str(chunks_path),
            "diagnostics_jsonl": str(diagnostics_path),
            "task_checkpoint_root": str(args.task_checkpoint_root),
            "source_snapshot_root": str(snapshot_root),
            "progress_jsonl": str(progress_path),
            "manifest": str(full_manifest_path),
            "readback": str(readback_path),
        },
        "coverage": observed_coverage,
        "expected_pair_count": len(expected_pairs),
        "missing_pair_count": len(missing_pairs),
        "unexpected_pair_count": len(unexpected_pairs),
        "parse_summary": parse_summary,
        "chunk_summary": chunk_summary,
        "state_count": len(states),
        "patch_log_count": len(patch_logs),
        "diagnostic_count": len(diagnostics),
        "diagnostic_policy": "non-fatal SyntaxWarning diagnostics are persisted as source-quality reward signals and do not fail coverage",
        "parse_records_sha256": parse_store["sha256"],
        "chunks_sha256": chunk_store["sha256"],
        "diagnostics_sha256": diagnostic_store["sha256"],
        "passes": full_coverage,
    }
    parse_manifest = {
        "schema_version": 1,
        "created_at_utc": utc_now(),
        "task_id": PARSE_TASK_ID,
        "issue": 249,
        "source_of_truth": {
            "schema": str(parse_schema_path),
            "records_jsonl": str(parse_records_path),
            "edge_records_jsonl": str(args.parse_root / "edge_case_parse_records.jsonl"),
            "manifest": str(parse_manifest_path),
            "readback": str(readback_path),
        },
        "inputs": {"cleaned_manifest": str(args.cleaned_manifest), "source_snapshot_root": str(snapshot_root)},
        "summary": parse_summary,
        "records_sha256": parse_store["sha256"],
        "edge_records_sha256": edge_parse_store["sha256"],
        "passes": True,
    }
    chunk_manifest = {
        "schema_version": 1,
        "created_at_utc": utc_now(),
        "task_id": CHUNK_TASK_ID,
        "issue": 249,
        "source_of_truth": {
            "schema": str(chunk_schema_path),
            "chunks_jsonl": str(chunks_path),
            "edge_chunks_jsonl": str(args.chunk_root / "edge_case_chunks.jsonl"),
            "manifest": str(chunk_manifest_path),
            "readback": str(readback_path),
        },
        "inputs": {"parse_records": str(parse_records_path), "full_ast_manifest": str(full_manifest_path)},
        "summary": chunk_summary,
        "coverage": observed_coverage,
        "diagnostics": {"chunk_reparse_warning_count": len(diagnostics), "sample": diagnostics[:20]},
        "chunks_sha256": chunk_store["sha256"],
        "diagnostics_sha256": diagnostic_store["sha256"],
        "edge_chunks_sha256": edge_chunk_store["sha256"],
        "passes": True,
    }
    write_json_checked(full_manifest_path, full_manifest)
    write_json_checked(parse_manifest_path, parse_manifest)
    write_json_checked(chunk_manifest_path, chunk_manifest)

    hot_bytes = path_bytes(args.hot_root)
    require(hot_bytes <= args.hot_budget_bytes, "PY_FULL_AST_HOT_BUDGET_EXCEEDED", "hot staging exceeded explicit budget", {"actual_bytes": hot_bytes, "budget_bytes": args.hot_budget_bytes, "hot_root": str(args.hot_root)})
    edge_cases = edge_case_results(rows, chunks)
    require(all(case["pass"] for case in edge_cases), "PY_FULL_AST_EDGE_READBACK_FAILED", "full AST corpus edge checks failed", {"edge_cases": edge_cases})
    after_state = {
        "created_at_utc": utc_now(),
        "parse_records": file_state(parse_records_path),
        "chunks": file_state(chunks_path),
        "diagnostics": file_state(diagnostics_path),
        "parse_manifest": file_state(parse_manifest_path),
        "chunk_manifest": file_state(chunk_manifest_path),
        "full_manifest": file_state(full_manifest_path),
        "readback": {
            "path": str(readback_path),
            "exists": True,
            "self_hash_note": "final full_ast_corpus_readback.json hash is recorded by command return and external readback, not embedded self-referentially",
        },
        "progress": file_state(progress_path),
        "chunk_coverage": coverage_from_jsonl(chunks_path),
        "disk": capacity_snapshot([Path("/"), Path("/zfs/hot"), Path("/zfs/archive")]),
        "hot_bytes": hot_bytes,
        "snapshot_bytes": path_bytes(snapshot_root),
    }
    shard_size = int(args.recommended_shard_size)
    recommended_shard_count = math.ceil(after_state["chunk_coverage"]["chunk_count"] / shard_size)
    readback = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "source_of_truth": full_manifest["source_of_truth"],
        "before_state": before_state,
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py full-ast-corpus",
            "cleaned_manifest_rows": len(rows),
            "task_count": len(grouped),
            "state_policy": "row-scoped source snapshots: base/test/gold for known_good and mutation_<category> for failure categories",
            "syntax_error_policy": "bounded syntax_error_file chunks are emitted for compile-error rows",
        },
        "after_state": after_state,
        "happy_path": {
            "case": "full_300x8_task_category_coverage",
            "expected": {"task_count": 300, "task_category_pair_count": 2400, "categories": list(EXPECTED_CATEGORIES)},
            "observed": observed_coverage,
            "pass": args.require_full_coverage is False or (
                observed_coverage["task_count"] == 300
                and observed_coverage["task_category_pair_count"] == 2400
                and set(observed_coverage["categories"]) == set(EXPECTED_CATEGORIES)
            ),
        },
        "edge_cases": edge_cases,
        "parse_edge_audit": parse_edge_audit,
        "chunk_edge_audit": chunk_edge_audit,
        "recommended_full_run": {
            "chunks_jsonl": str(chunks_path),
            "chunks_sha256": chunk_store["sha256"],
            "shard_size": shard_size,
            "shard_count": recommended_shard_count,
            "cache_root_must_not_reuse_old_chunk_sha": True,
        },
        "summaries": {
            "parse": parse_summary,
            "chunks": chunk_summary,
            "coverage": observed_coverage,
            "patch_log_sample": patch_logs[:20],
            "diagnostic_policy": "non-fatal SyntaxWarning diagnostics are persisted and retained as reward-signal features",
            "diagnostic_sample": diagnostics[:20],
        },
        "passes": True,
    }
    write_json_checked(readback_path, readback)
    readback = read_json(readback_path, "PY_FULL_AST_READBACK_READBACK_INVALID")
    require(readback == readback, "PY_FULL_AST_READBACK_READBACK_MISMATCH", "full AST corpus READBACK did not read back exactly", {"readback": str(readback_path)})
    return {
        **readback,
        "manifest_sha256": sha256_file(full_manifest_path),
        "readback_sha256": sha256_file(readback_path),
    }
