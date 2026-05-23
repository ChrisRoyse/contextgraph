from __future__ import annotations

import ast
import difflib
import hashlib
import json
import os
import shutil
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked


DEFAULT_CANDIDATE_PLAN = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-delete-test-call-regeneration-v1/"
    "delete_test_call_regeneration_candidates.jsonl"
)
DEFAULT_SOURCE_ROOT = Path(
    "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/"
    "source-files-300x8-20260515T013205Z"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-delete-test-call-regeneration-v2-patches"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135c-delete-test-call-regeneration-patches-fsv"
)


@dataclass(frozen=True)
class StatementSpan:
    node_type: str
    start_line: int
    end_line: int
    col_offset: int
    end_col_offset: int
    start_byte: int
    end_byte: int
    text: str


def add_arguments(parser) -> None:
    parser.add_argument("--candidate-plan", type=Path, default=DEFAULT_CANDIDATE_PLAN)
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--max-candidates", type=int, default=None)
    parser.add_argument("--clean-output", action="store_true")


def command_python_delete_test_call_regeneration(args) -> dict[str, Any]:
    if args.max_candidates is not None and args.max_candidates <= 0:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_BAD_MAX_CANDIDATES",
            f"--max-candidates must be positive when set: {args.max_candidates}",
            "Use a positive limit or omit --max-candidates.",
            {"max_candidates": args.max_candidates},
        )
    if not args.candidate_plan.is_file():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_CANDIDATE_PLAN_MISSING",
            f"candidate plan missing: {args.candidate_plan}",
            "Materialize the #615 candidate plan before building patch artifacts.",
            {"candidate_plan": str(args.candidate_plan)},
        )
    if not args.source_root.is_dir():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_ROOT_MISSING",
            f"source root missing: {args.source_root}",
            "Restore the canonical aiwonder #613 source snapshot before building patch artifacts.",
            {"source_root": str(args.source_root)},
        )
    if has_mounted_drive_fragment(args.candidate_plan) or has_mounted_drive_fragment(args.source_root):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_MOUNTED_PATH_INPUT_REJECTED",
            "candidate plan and source root must be aiwonder paths, not mounted-drive paths",
            "Use /zfs/archive/contextgraph or /zfs/hot/contextgraph sources only.",
            {"candidate_plan": str(args.candidate_plan), "source_root": str(args.source_root)},
        )

    rows = load_jsonl(args.candidate_plan)
    if args.max_candidates is not None:
        rows = rows[: args.max_candidates]
    if not rows:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_CANDIDATE_PLAN_EMPTY",
            f"candidate plan contains no rows after filtering: {args.candidate_plan}",
            "Regenerate the #615 candidate plan with at least one row.",
            {"candidate_plan": str(args.candidate_plan), "max_candidates": args.max_candidates},
        )

    output_root = args.output_root
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    patches_dir = output_root / "patches"
    output_root.mkdir(parents=True, exist_ok=True)
    patches_dir.mkdir(parents=True, exist_ok=True)

    accepted: list[dict[str, Any]] = []
    rejections: list[dict[str, Any]] = []
    for index, row in enumerate(rows):
        try:
            accepted.append(materialize_candidate(row, index, args.source_root, patches_dir))
        except SwebenchOpsError as exc:
            rejections.append(
                {
                    "row_index": index,
                    "candidate_row_id": row.get("candidate_row_id"),
                    "source_task_id": row.get("source_task_id"),
                    "source_path": row.get("source_path"),
                    **exc.to_json(),
                }
            )

    color_batches(accepted)

    accepted_path = output_root / "delete_test_call_candidate_patches.jsonl"
    rejection_path = output_root / "delete_test_call_candidate_rejections.jsonl"
    batch_plan_path = output_root / "delete_test_call_oracle_batch_plan.jsonl"
    manifest_path = output_root / "delete_test_call_patch_manifest.json"

    write_jsonl_checked(accepted_path, accepted)
    write_jsonl_checked(rejection_path, rejections)
    batch_rows = build_batch_rows(accepted)
    write_jsonl_checked(batch_plan_path, batch_rows)

    support_counts = Counter(
        (
            row["stable_partition"],
            row["assertion_duty_sublabel"],
            row["mutation_strategy"],
        )
        for row in accepted
    )
    by_status = Counter(row["patch_materialization_status"] for row in accepted)
    by_partition_sublabel = Counter(
        (row.get("stable_partition", "unknown"), row.get("assertion_duty_sublabel", "unknown"))
        for row in rows
    )
    rejections_by_code = Counter(row["error_code"] for row in rejections)
    manifest = {
        "schema_version": 1,
        "task_id": "TASK-PY-G-135C",
        "artifact_kind": "delete_test_call_regeneration_patch_manifest",
        "created_at": utc_now(),
        "candidate_plan": str(args.candidate_plan),
        "candidate_plan_sha256": sha256_file(args.candidate_plan),
        "source_root": str(args.source_root),
        "output_root": str(output_root),
        "patch_builder": "python_ast_statement_delete_or_pass_v1",
        "input_rows": len(rows),
        "accepted_rows": len(accepted),
        "rejected_rows": len(rejections),
        "oracle_batch_count": len(batch_rows),
        "max_candidates": args.max_candidates,
        "counts_by_input_partition_sublabel": counter_to_dict(by_partition_sublabel),
        "counts_by_patch_status": dict(sorted(by_status.items())),
        "counts_by_partition_sublabel_strategy": counter_to_dict(support_counts),
        "rejections_by_code": dict(sorted(rejections_by_code.items())),
        "accepted_jsonl": str(accepted_path),
        "accepted_jsonl_sha256": sha256_file(accepted_path),
        "rejections_jsonl": str(rejection_path),
        "rejections_jsonl_sha256": sha256_file(rejection_path),
        "oracle_batch_plan_jsonl": str(batch_plan_path),
        "oracle_batch_plan_jsonl_sha256": sha256_file(batch_plan_path),
        "patches_dir": str(patches_dir),
        "all_passed": bool(accepted) and not rejections,
        "zero_vector_or_oracle_label_live_input_used": False,
        "target_oracle_labels_used_as_features": False,
    }
    write_json_checked(manifest_path, manifest)

    readback = {
        "manifest_exists": manifest_path.is_file(),
        "accepted_exists": accepted_path.is_file(),
        "rejections_exists": rejection_path.is_file(),
        "batch_plan_exists": batch_plan_path.is_file(),
        "manifest_sha256": sha256_file(manifest_path),
        "accepted_line_count": count_lines(accepted_path),
        "rejection_line_count": count_lines(rejection_path),
        "batch_plan_line_count": count_lines(batch_plan_path),
        "patch_file_count": len(list(patches_dir.glob("*.patch"))),
    }

    return {
        "ok": manifest["all_passed"],
        "all_passed": manifest["all_passed"],
        "command": "python-delete-test-call-regeneration",
        "checked_at": utc_now(),
        "manifest": manifest,
        "readback": readback,
        "rejection_sample": rejections[:10],
        "accepted_sample": accepted[:3],
    }


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise SwebenchOpsError(
                    "DELETE_TEST_CALL_CANDIDATE_PLAN_BAD_JSONL",
                    f"invalid JSONL at {path}:{line_number}",
                    "Regenerate the candidate plan with valid JSONL.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            candidate_id = require_str(row, "candidate_row_id")
            if candidate_id in seen_ids:
                raise SwebenchOpsError(
                    "DELETE_TEST_CALL_DUPLICATE_CANDIDATE_ROW_ID",
                    f"duplicate candidate_row_id: {candidate_id}",
                    "Regenerate the candidate plan with stable unique row ids.",
                    {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
                )
            seen_ids.add(candidate_id)
            rows.append(row)
    return rows


def materialize_candidate(row: dict[str, Any], row_index: int, source_root: Path, patches_dir: Path) -> dict[str, Any]:
    candidate_id = require_str(row, "candidate_row_id")
    task_id = require_str(row, "source_task_id")
    source_path = require_str(row, "source_path")
    if has_mounted_drive_fragment(Path(source_path)) or source_path.startswith("/") or ".." in Path(source_path).parts:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_BAD_SOURCE_PATH",
            f"bad source_path for {candidate_id}: {source_path}",
            "Use canonical relative source paths only.",
            {"candidate_row_id": candidate_id, "source_path": source_path},
        )
    source_file = source_root / task_id / "base" / source_path
    if not source_file.is_file():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_FILE_MISSING",
            f"source file missing for {candidate_id}: {source_file}",
            "Restore the canonical source snapshot or drop the invalid candidate.",
            {"candidate_row_id": candidate_id, "source_file": str(source_file)},
        )

    source_bytes = source_file.read_bytes()
    actual_source_sha = "sha256:" + hashlib.sha256(source_bytes).hexdigest()
    expected_source_sha = require_str(row, "source_file_sha256")
    if actual_source_sha != expected_source_sha:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_SHA_MISMATCH",
            f"source hash mismatch for {candidate_id}",
            "Do not run oracle on a candidate whose source bytes differ from the plan.",
            {
                "candidate_row_id": candidate_id,
                "source_file": str(source_file),
                "expected": expected_source_sha,
                "actual": actual_source_sha,
            },
        )

    offset = require_int(row, "source_site_offset")
    length = require_int(row, "source_site_len")
    if offset < 0 or length <= 0 or offset + length > len(source_bytes):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_SPAN_OUT_OF_BOUNDS",
            f"source span out of bounds for {candidate_id}",
            "Regenerate candidate offsets from the canonical source snapshot.",
            {"candidate_row_id": candidate_id, "offset": offset, "length": length, "source_bytes": len(source_bytes)},
        )
    source_text = source_bytes.decode("utf-8")
    exact_source_site_text = source_bytes[offset : offset + length].decode("utf-8", "replace")
    effective_offset = resolve_effective_offset(row, source_bytes, offset, exact_source_site_text)
    source_site_preview = str(row.get("source_site_preview") or "").strip()
    source_site_text = source_bytes[
        effective_offset : effective_offset + len(source_site_preview.encode("utf-8"))
    ].decode("utf-8", "replace")
    source_lines = source_text.splitlines(keepends=True)
    line_starts = byte_line_starts(source_text)
    candidate_line = byte_offset_to_line(line_starts, effective_offset)

    try:
        tree = ast.parse(source_text)
    except SyntaxError as exc:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_ORIGINAL_SOURCE_PARSE_ERROR",
            f"original source does not parse for {candidate_id}: {source_file}",
            "Do not create assertion-duty candidates from parse-invalid base source.",
            {"candidate_row_id": candidate_id, "source_file": str(source_file), "error": str(exc)},
        ) from exc

    span = find_statement_span(tree, source_lines, line_starts, candidate_line)
    if span is None:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_STATEMENT_SPAN_NOT_FOUND",
            f"could not find AST statement for {candidate_id}",
            "Regenerate candidate offsets from AST statement locations.",
            {"candidate_row_id": candidate_id, "candidate_line": candidate_line},
        )
    if source_site_preview and source_site_preview not in span.text:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_PREVIEW_NOT_IN_SELECTED_STATEMENT",
            f"selected AST statement does not contain source_site_preview for {candidate_id}",
            "Regenerate candidate offsets/previews from canonical source bytes before running oracle.",
            {
                "candidate_row_id": candidate_id,
                "source_site_preview": source_site_preview,
                "selected_statement_preview": span.text[:240],
                "source_site_offset": offset,
                "effective_source_site_offset": effective_offset,
            },
        )
    line_bytes = source_lines[span.start_line - 1].encode("utf-8")
    prefix = line_bytes[: span.col_offset].decode("utf-8", "replace")
    if prefix.strip():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_INLINE_STATEMENT_UNSUPPORTED",
            f"candidate statement is inline and cannot be safely line-deleted: {candidate_id}",
            "Add an inline-statement mutation strategy before using this candidate.",
            {"candidate_row_id": candidate_id, "line": span.start_line, "prefix": prefix},
        )

    delete_lines = source_lines[: span.start_line - 1] + source_lines[span.end_line :]
    delete_text = "".join(delete_lines)
    delete_parse_error = parse_error(delete_text)
    if delete_parse_error is None:
        mutation_strategy = "delete_statement"
        mutated_text = delete_text
        syntax_repair_applied = False
    else:
        indent = leading_whitespace(source_lines[span.start_line - 1])
        replacement = indent + "pass" + line_ending(source_lines[span.start_line - 1])
        pass_lines = source_lines[: span.start_line - 1] + [replacement] + source_lines[span.end_line :]
        pass_text = "".join(pass_lines)
        pass_parse_error = parse_error(pass_text)
        if pass_parse_error is not None:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_MUTATION_PARSE_ERROR",
                f"neither deletion nor pass replacement parses for {candidate_id}",
                "Add a narrower syntax-preserving mutation strategy before running oracle on this row.",
                {
                    "candidate_row_id": candidate_id,
                    "delete_parse_error": delete_parse_error,
                    "pass_parse_error": pass_parse_error,
                },
            )
        mutation_strategy = "replace_statement_with_pass"
        mutated_text = pass_text
        syntax_repair_applied = True

    patch_text = build_unified_diff(source_path, source_text, mutated_text)
    if not patch_text.strip():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_EMPTY_PATCH",
            f"empty patch for {candidate_id}",
            "Fix the candidate span before running oracle.",
            {"candidate_row_id": candidate_id},
        )
    patch_id = candidate_id.removeprefix("sha256:")
    patch_path = patches_dir / f"{patch_id}.patch"
    write_text_checked(patch_path, patch_text)
    patch_sha = "sha256:" + hashlib.sha256(patch_text.encode("utf-8")).hexdigest()

    out = dict(row)
    out.update(
        {
            "row_index": row_index,
            "patch_materialization_status": "ready_for_oracle",
            "patch_builder": "python_ast_statement_delete_or_pass_v1",
            "mutation_strategy": mutation_strategy,
            "syntax_repair_applied": syntax_repair_applied,
            "delete_statement_parse_error": delete_parse_error,
            "selected_statement": {
                "node_type": span.node_type,
                "start_line": span.start_line,
                "end_line": span.end_line,
                "start_byte": span.start_byte,
                "end_byte": span.end_byte,
                "sha256": "sha256:" + hashlib.sha256(span.text.encode("utf-8")).hexdigest(),
                "text_preview": span.text[:200],
            },
            "source_site_text": source_site_text,
            "source_site_exact_text": exact_source_site_text,
            "source_site_effective_offset": effective_offset,
            "source_site_offset_adjusted": effective_offset != offset,
            "source_site_offset_adjustment_bytes": effective_offset - offset,
            "source_site_preview_contained": source_site_preview in source_site_text.strip(),
            "selected_statement_contains_preview": (not source_site_preview) or source_site_preview in span.text,
            "source_file": str(source_file),
            "patch_path": str(patch_path),
            "patch_path_rel": str(patch_path.relative_to(patches_dir.parent)),
            "patch_sha256": patch_sha,
            "patch_size_bytes": patch_path.stat().st_size,
            "oracle_status": "ready",
            "target_oracle_label_used": False,
        }
    )
    return out


def resolve_effective_offset(
    row: dict[str, Any],
    source_bytes: bytes,
    original_offset: int,
    exact_source_site_text: str,
) -> int:
    preview = str(row.get("source_site_preview") or "").strip()
    if not preview:
        return original_offset
    preview_bytes = preview.encode("utf-8")
    matches: list[int] = []
    start = 0
    while True:
        found = source_bytes.find(preview_bytes, start)
        if found < 0:
            break
        matches.append(found)
        start = found + 1
    if not matches:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_PREVIEW_NOT_FOUND",
            f"source_site_preview not found in source bytes for {row.get('candidate_row_id')}",
            "Regenerate candidate previews from the canonical source snapshot.",
            {
                "candidate_row_id": row.get("candidate_row_id"),
                "source_site_preview": preview,
                "source_site_offset": original_offset,
                "exact_source_site_text": exact_source_site_text,
            },
        )
    nearest = min(matches, key=lambda value: abs(value - original_offset))
    distance = abs(nearest - original_offset)
    if distance > 4096:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_PREVIEW_TOO_FAR_FROM_OFFSET",
            f"nearest source_site_preview is too far from source_site_offset for {row.get('candidate_row_id')}",
            "Regenerate candidate offsets from canonical source bytes; the preview is ambiguous.",
            {
                "candidate_row_id": row.get("candidate_row_id"),
                "source_site_preview": preview,
                "source_site_offset": original_offset,
                "nearest_preview_offset": nearest,
                "distance_bytes": distance,
                "match_count": len(matches),
            },
        )
    return nearest


def find_statement_span(
    tree: ast.AST,
    source_lines: list[str],
    line_starts: list[int],
    candidate_line: int,
) -> StatementSpan | None:
    candidates: list[ast.stmt] = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.stmt):
            continue
        if not hasattr(node, "lineno") or not hasattr(node, "end_lineno"):
            continue
        if node.lineno <= candidate_line <= (node.end_lineno or node.lineno):
            candidates.append(node)
    if not candidates:
        return None
    candidates.sort(
        key=lambda node: (
            (node.end_lineno or node.lineno) - node.lineno,
            getattr(node, "col_offset", 0),
            -node.lineno,
        )
    )
    node = candidates[0]
    start_line = int(node.lineno)
    end_line = int(node.end_lineno or node.lineno)
    start_byte = line_starts[start_line - 1]
    end_byte = line_starts[end_line] if end_line < len(line_starts) else sum(len(line.encode("utf-8")) for line in source_lines)
    text = "".join(source_lines[start_line - 1 : end_line])
    return StatementSpan(
        node_type=type(node).__name__,
        start_line=start_line,
        end_line=end_line,
        col_offset=int(getattr(node, "col_offset", 0)),
        end_col_offset=int(getattr(node, "end_col_offset", 0) or 0),
        start_byte=start_byte,
        end_byte=end_byte,
        text=text,
    )


def byte_line_starts(text: str) -> list[int]:
    starts = [0]
    total = 0
    for line in text.splitlines(keepends=True):
        total += len(line.encode("utf-8"))
        starts.append(total)
    return starts


def byte_offset_to_line(line_starts: list[int], offset: int) -> int:
    line = 1
    for index, start in enumerate(line_starts):
        if start > offset:
            break
        line = index + 1
    return max(1, min(line, len(line_starts)))


def parse_error(text: str) -> str | None:
    try:
        ast.parse(text)
    except (SyntaxError, ValueError) as exc:
        return f"{type(exc).__name__}: {exc}"
    return None


def build_unified_diff(source_path: str, before: str, after: str) -> str:
    before_lines = before.splitlines(keepends=True)
    after_lines = after.splitlines(keepends=True)
    diff_lines = list(
        difflib.unified_diff(
            before_lines,
            after_lines,
            fromfile=f"a/{source_path}",
            tofile=f"b/{source_path}",
            lineterm="",
        )
    )
    if not diff_lines:
        return ""
    normalized = [f"diff --git a/{source_path} b/{source_path}\n"]
    for line in diff_lines:
        normalized.append(line if line.endswith("\n") else line + "\n")
    return "".join(normalized)


def color_batches(rows: list[dict[str, Any]]) -> None:
    batches: list[set[str]] = []
    for row in rows:
        task_id = row["source_task_id"]
        selected_index = None
        for batch_index, tasks in enumerate(batches):
            if task_id not in tasks:
                selected_index = batch_index
                break
        if selected_index is None:
            selected_index = len(batches)
            batches.append(set())
        batches[selected_index].add(task_id)
        row["oracle_batch_index"] = selected_index
        row["oracle_batch_key"] = f"delete-test-call-regeneration-batch-{selected_index:04d}"


def build_batch_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    grouped: dict[int, list[dict[str, Any]]] = {}
    for row in rows:
        grouped.setdefault(int(row["oracle_batch_index"]), []).append(row)
    batch_rows: list[dict[str, Any]] = []
    for batch_index, batch in sorted(grouped.items()):
        task_ids = [row["source_task_id"] for row in batch]
        if len(task_ids) != len(set(task_ids)):
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_BATCH_COLORING_DUPLICATE_TASK",
                f"oracle batch {batch_index} contains duplicate source_task_id",
                "Fix color_batches before running the SWE-bench batch oracle.",
                {"batch_index": batch_index, "source_task_ids": task_ids},
            )
        batch_rows.append(
            {
                "schema_version": 1,
                "artifact_kind": "delete_test_call_oracle_batch",
                "batch_index": batch_index,
                "batch_key": f"delete-test-call-regeneration-batch-{batch_index:04d}",
                "prediction_count": len(batch),
                "source_task_ids": task_ids,
                "candidate_row_ids": [row["candidate_row_id"] for row in batch],
                "patch_paths": [row["patch_path"] for row in batch],
            }
        )
    return batch_rows


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_TEXT_READBACK_MISMATCH",
            f"text readback mismatch at {path}",
            "Investigate the filesystem for partial writes or concurrent writers.",
            {"path": str(path)},
        )


def write_jsonl_checked(path: Path, rows: list[dict[str, Any]]) -> None:
    text = "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows)
    write_text_checked(path, text)


def count_lines(path: Path) -> int:
    if not path.exists():
        return 0
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for _ in handle)


def counter_to_dict(counter: Counter) -> dict[str, int]:
    return {"|".join(map(str, key if isinstance(key, tuple) else (key,))): value for key, value in sorted(counter.items())}


def require_str(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_REQUIRED_STRING_MISSING",
            f"required string field missing: {key}",
            "Regenerate the candidate plan with all required lineage fields.",
            {"key": key, "row": row},
        )
    return value


def require_int(row: dict[str, Any], key: str) -> int:
    value = row.get(key)
    if not isinstance(value, int):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_REQUIRED_INT_MISSING",
            f"required integer field missing: {key}",
            "Regenerate the candidate plan with all required source offset fields.",
            {"key": key, "row": row},
        )
    return value


def leading_whitespace(line: str) -> str:
    return line[: len(line) - len(line.lstrip(" \t"))]


def line_ending(line: str) -> str:
    return "\r\n" if line.endswith("\r\n") else "\n"


def has_mounted_drive_fragment(path: Path) -> bool:
    text = str(path)
    return "/mnt/d" in text or "/mnt/c" in text
