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
from typing import Any, Iterable

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
DEFAULT_CORPUS_INDEX = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json")
DEFAULT_SOURCE_EXTENSION_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135i-delete-test-call-source-extension-v1"
)
DEFAULT_SOURCE_EXTENSION_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135i-delete-test-call-source-extension-fsv"
)
DEFAULT_PRIOR_SOURCE_EXTENSION_PLAN = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135h-delete-test-call-source-extension-v1/"
    "delete_test_call_regeneration_candidates.jsonl"
)
SOURCE_EXTENSION_TASK_ID = "TASK-PY-G-135I"
SOURCE_EXTENSION_POLICY_VERSION = "delete_test_call_source_pass_hypothesis_extension_v2"
DELETE_TEST_CALL_DEGENERATE_CELL_HALT_CODE = "DELETE_TEST_CALL_DEGENERATE_CELL_REGENERATION_HALTED"
DELETE_TEST_CALL_DEGENERATE_CELL_DECISION_ISSUE = "https://github.com/ChrisRoyse/contextgraph/issues/810"
DELETE_TEST_CALL_DEGENERATE_CELL_EVIDENCE_ISSUE = "https://github.com/ChrisRoyse/contextgraph/issues/808"
DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOMES_PATH = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135i-delete-test-call-oracle-outcomes-v1/"
    "delete_test_call_oracle_outcomes.jsonl"
)
DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOMES_SHA256 = (
    "sha256:4b74dbc45d5df274659af4d3169a92c6bd812f5aa43d4051d4febc3811f86d6d"
)
DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOME_COUNTS = {
    "resolved": 32,
    "unresolved": 9079,
    "command_failure": 0,
    "rows": 9111,
}


def halt_delete_test_call_regeneration(command: str) -> None:
    raise SwebenchOpsError(
        DELETE_TEST_CALL_DEGENERATE_CELL_HALT_CODE,
        f"{command} is halted by #810: delete_test_call::python is a degenerate all-fail cell, not a Pearson-regeneration target",
        "Use the #532 degenerate-cell contract for delete_test_call::python and redirect new oracle budget to mixed cells.",
        {
            "command": command,
            "cell": "delete_test_call::python",
            "decision_issue": DELETE_TEST_CALL_DEGENERATE_CELL_DECISION_ISSUE,
            "evidence_issue": DELETE_TEST_CALL_DEGENERATE_CELL_EVIDENCE_ISSUE,
            "outcomes_jsonl": str(DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOMES_PATH),
            "outcomes_sha256": DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOMES_SHA256,
            "outcome_counts": DELETE_TEST_CALL_DEGENERATE_CELL_OUTCOME_COUNTS,
            "oracle_pass_rate": 32 / 9111,
            "gate_contract": "degenerate_all_fail_fail_recall_wilson_brier_ece",
        },
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


def add_source_extension_arguments(parser) -> None:
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE_ROOT)
    parser.add_argument("--corpus-index", type=Path, default=DEFAULT_CORPUS_INDEX)
    parser.add_argument("--existing-candidate-plan", type=Path, default=DEFAULT_CANDIDATE_PLAN)
    parser.add_argument(
        "--additional-existing-candidate-plan",
        type=Path,
        action="append",
        default=[DEFAULT_PRIOR_SOURCE_EXTENSION_PLAN],
        help="Additional candidate JSONL plans whose source spans must not be regenerated.",
    )
    parser.add_argument("--output-root", type=Path, default=DEFAULT_SOURCE_EXTENSION_OUTPUT_ROOT)
    parser.add_argument(
        "--readback-root",
        "--fsv-root",
        dest="readback_root",
        type=Path,
        default=DEFAULT_SOURCE_EXTENSION_READBACK_ROOT,
    )
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_SOURCE_EXTENSION_READBACK_ROOT)
    parser.add_argument("--max-files", type=int, default=None)
    parser.add_argument("--max-candidates", type=int, default=None)
    parser.add_argument("--clean-output", action="store_true")


def command_python_delete_test_call_regeneration(args) -> dict[str, Any]:
    halt_delete_test_call_regeneration("python-delete-test-call-regeneration")
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
    input_task_ids = sorted({str(row.get("task_id") or "TASK-PY-G-135C") for row in rows})
    manifest = {
        "schema_version": 1,
        "task_id": input_task_ids[0] if len(input_task_ids) == 1 else "TASK-PY-G-DELETE-TEST-CALL-MIXED",
        "artifact_kind": "delete_test_call_regeneration_patch_manifest",
        "created_at": utc_now(),
        "candidate_plan": str(args.candidate_plan),
        "candidate_plan_sha256": sha256_file(args.candidate_plan),
        "input_task_ids": input_task_ids,
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


def command_python_delete_test_call_source_extension(args) -> dict[str, Any]:
    halt_delete_test_call_regeneration("python-delete-test-call-source-extension")
    validate_source_extension_args(args)
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    task_meta = load_task_partition_meta(args.corpus_index)
    existing_plans = source_extension_existing_candidate_plans(args)
    existing_spans = load_existing_candidate_spans(existing_plans)
    rows, scan = generate_source_extension_candidates(
        source_root=args.source_root,
        task_meta=task_meta,
        existing_spans=existing_spans,
        max_files=args.max_files,
        max_candidates=args.max_candidates,
    )
    if not rows:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_EXTENSION_EMPTY",
            "source extension produced no pre-oracle pass-hypothesis rows",
            "Inspect the canonical source root and structural source rules before rerunning.",
            {"source_root": str(args.source_root), "corpus_index": str(args.corpus_index)},
        )

    plan_path = args.output_root / "delete_test_call_regeneration_candidates.jsonl"
    manifest_path = args.output_root / "delete_test_call_source_extension_manifest.json"
    readback_path = args.readback_root / "delete_test_call_source_extension_readback.json"

    write_jsonl_checked(plan_path, rows)
    counts_by_split_sublabel = Counter((row["stable_partition"], row["assertion_duty_sublabel"]) for row in rows)
    counts_by_hypothesis = Counter(
        (
            row["stable_partition"],
            row["assertion_duty_sublabel"],
            hypothesis["kind"],
        )
        for row in rows
        for hypothesis in row.get("pre_oracle_pass_hypothesis", {}).get("hypotheses", [])
    )
    manifest = {
        "schema_version": 1,
        "task_id": SOURCE_EXTENSION_TASK_ID,
        "artifact_kind": "delete_test_call_source_extension_manifest",
        "created_at_utc": utc_now(),
        "policy_version": SOURCE_EXTENSION_POLICY_VERSION,
        "source_root": str(args.source_root),
        "corpus_index": str(args.corpus_index),
        "corpus_index_sha256": sha256_file(args.corpus_index),
        "existing_candidate_plans": [str(path) for path in existing_plans],
        "existing_candidate_plan_sha256": {str(path): sha256_file(path) for path in existing_plans},
        "plan_path": str(plan_path),
        "plan_sha256": sha256_file(plan_path),
        "candidate_count": len(rows),
        "counts_by_split_sublabel": counter_to_dict(counts_by_split_sublabel),
        "counts_by_split_sublabel_hypothesis": counter_to_dict(counts_by_hypothesis),
        "scan": scan,
        "selection_policy": "all_structural_pre_oracle_pass_hypothesis_rows_stable_source_order_v2",
        "oracle_required_before_label_use": True,
        "regeneration_ready_for_oracle_batch": True,
        "zero_vector_or_oracle_label_live_input_used": False,
        "target_oracle_labels_used_as_features": False,
        "holdout_tuning_used": False,
    }
    write_json_checked(manifest_path, manifest)

    readback = {
        "manifest_exists": manifest_path.is_file(),
        "plan_exists": plan_path.is_file(),
        "manifest_sha256": sha256_file(manifest_path),
        "plan_sha256": sha256_file(plan_path),
        "plan_line_count": count_lines(plan_path),
        "candidate_count": len(rows),
        "distinct_candidate_row_ids": len({row["candidate_row_id"] for row in rows}),
        "counts_by_split_sublabel": counter_to_dict(counts_by_split_sublabel),
        "target_oracle_labels_used_as_features": False,
        "holdout_tuning_used": False,
    }
    write_json_checked(readback_path, readback)

    return {
        "ok": readback["plan_line_count"] == len(rows) == readback["distinct_candidate_row_ids"],
        "all_passed": readback["plan_line_count"] == len(rows) == readback["distinct_candidate_row_ids"],
        "command": "python-delete-test-call-source-extension",
        "checked_at": utc_now(),
        "manifest": manifest,
        "readback": readback,
        "sample": rows[:5],
    }


def validate_source_extension_args(args) -> None:
    for name in ("source_root", "corpus_index", "existing_candidate_plan", "output_root", "readback_root", "evidence_root"):
        path = getattr(args, name)
        if has_mounted_drive_fragment(path):
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_SOURCE_EXTENSION_MOUNTED_PATH_REJECTED",
                f"{name} points at mounted-drive lineage: {path}",
                "Use aiwonder /zfs/archive/contextgraph or /zfs/hot/contextgraph paths only.",
                {"name": name, "path": str(path)},
            )
    if not args.source_root.is_dir():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_EXTENSION_SOURCE_ROOT_MISSING",
            f"source root missing: {args.source_root}",
            "Restore the canonical aiwonder #613 source snapshot before generating extension candidates.",
            {"source_root": str(args.source_root)},
        )
    if not args.corpus_index.is_file():
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_EXTENSION_INDEX_MISSING",
            f"corpus index missing: {args.corpus_index}",
            "Use the canonical SWE-bench Lite 300x8 index so split assignment is predeclared.",
            {"corpus_index": str(args.corpus_index)},
        )
    for path in source_extension_existing_candidate_plans(args):
        if has_mounted_drive_fragment(path):
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_SOURCE_EXTENSION_MOUNTED_PATH_REJECTED",
                f"existing candidate plan points at mounted-drive lineage: {path}",
                "Use aiwonder /zfs/archive/contextgraph or /zfs/hot/contextgraph paths only.",
                {"name": "existing_candidate_plan", "path": str(path)},
            )
        if not path.is_file():
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_SOURCE_EXTENSION_EXISTING_PLAN_MISSING",
                f"existing candidate plan missing: {path}",
                "Use the #135C plan and any prior source-extension plans to avoid regenerating duplicate source spans.",
                {"existing_candidate_plan": str(path)},
            )
    for name in ("max_files", "max_candidates"):
        value = getattr(args, name)
        if value is not None and value <= 0:
            raise SwebenchOpsError(
                "DELETE_TEST_CALL_SOURCE_EXTENSION_BAD_LIMIT",
                f"{name} must be positive when set: {value}",
                "Use a positive limit or omit the limit.",
                {"name": name, "value": value},
            )


def load_task_partition_meta(path: Path) -> dict[str, dict[str, str]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    entries = payload.get("entries")
    if not isinstance(entries, list):
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_EXTENSION_BAD_INDEX",
            f"corpus index has no entries array: {path}",
            "Use the canonical 300x8 corpus index.",
            {"corpus_index": str(path)},
        )
    task_meta: dict[str, dict[str, str]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        task_id = str(entry.get("task_id") or "")
        bucket = str(entry.get("bucket") or "")
        repo = str(entry.get("repo") or "")
        if not task_id or not bucket or not repo:
            continue
        task_meta.setdefault(task_id, {"stable_partition": bucket, "repo": repo})
    if not task_meta:
        raise SwebenchOpsError(
            "DELETE_TEST_CALL_SOURCE_EXTENSION_EMPTY_INDEX",
            f"corpus index yielded no task metadata: {path}",
            "Use the canonical 300x8 corpus index.",
            {"corpus_index": str(path)},
        )
    return task_meta


def source_extension_existing_candidate_plans(args) -> list[Path]:
    paths = [args.existing_candidate_plan]
    paths.extend(args.additional_existing_candidate_plan or [])
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path)
        if key in seen:
            continue
        unique.append(path)
        seen.add(key)
    return unique


def load_existing_candidate_spans(paths: list[Path]) -> set[tuple[str, str, str, int]]:
    spans: set[tuple[str, str, str, int]] = set()
    for path in paths:
        for row in load_jsonl(path):
            task_id = require_str(row, "source_task_id")
            source_path = require_str(row, "source_path")
            source_sha = require_str(row, "source_file_sha256")
            offset = require_int(row, "source_site_offset")
            spans.add((task_id, source_path, source_sha, offset))
    return spans


def generate_source_extension_candidates(
    *,
    source_root: Path,
    task_meta: dict[str, dict[str, str]],
    existing_spans: set[tuple[str, str, str, int]],
    max_files: int | None,
    max_candidates: int | None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    seen_candidate_ids: set[str] = set()
    scanned_files = 0
    parsed_files = 0
    skipped_files_without_assertion_markers = 0
    parse_failures: list[dict[str, str]] = []
    for task_id in sorted(task_meta):
        base_root = source_root / task_id / "base"
        if not base_root.is_dir():
            continue
        for source_file in sorted(base_root.rglob("*.py")):
            if max_files is not None and scanned_files >= max_files:
                return rows, source_extension_scan_stats(
                    scanned_files, parsed_files, skipped_files_without_assertion_markers, parse_failures, max_files, True
                )
            scanned_files += 1
            source_bytes = source_file.read_bytes()
            if not has_assertion_marker(source_bytes):
                skipped_files_without_assertion_markers += 1
                continue
            try:
                source_text = source_bytes.decode("utf-8")
                tree = ast.parse(source_text)
            except (UnicodeDecodeError, SyntaxError, ValueError) as exc:
                parse_failures.append({"source_file": str(source_file), "error": f"{type(exc).__name__}: {exc}"})
                continue
            parsed_files += 1
            source_path = str(source_file.relative_to(base_root))
            source_sha = "sha256:" + hashlib.sha256(source_bytes).hexdigest()
            source_lines = source_text.splitlines(keepends=True)
            line_starts = byte_line_starts(source_text)
            parent_map = build_parent_map(tree)
            skip_intervals, dead_intervals = precompute_source_extension_intervals(tree)
            candidate_index = 0
            for statement in iter_source_extension_candidate_statements(tree):
                candidate_index += 1
                classification = classify_source_extension_candidate(Path(source_path), statement)
                if classification is None:
                    continue
                start_byte = line_starts[statement.lineno - 1]
                span_key = (task_id, source_path, source_sha, start_byte)
                if span_key in existing_spans:
                    continue
                hypotheses = source_extension_hypotheses(
                    tree=tree,
                    statement=statement,
                    parent_map=parent_map,
                    source_path=Path(source_path),
                    classification=classification,
                    skip_intervals=skip_intervals,
                    dead_intervals=dead_intervals,
                )
                if not hypotheses:
                    continue
                row = build_source_extension_candidate_row(
                    task_id=task_id,
                    task_meta=task_meta[task_id],
                    source_path=source_path,
                    source_sha=source_sha,
                    source_lines=source_lines,
                    line_starts=line_starts,
                    statement=statement,
                    classification=classification,
                    candidate_index=candidate_index,
                    hypotheses=hypotheses,
                )
                if row["candidate_row_id"] in seen_candidate_ids:
                    continue
                seen_candidate_ids.add(row["candidate_row_id"])
                rows.append(row)
                if max_candidates is not None and len(rows) >= max_candidates:
                    return rows, source_extension_scan_stats(
                        scanned_files,
                        parsed_files,
                        skipped_files_without_assertion_markers,
                        parse_failures,
                        max_files,
                        True,
                    )
    return rows, source_extension_scan_stats(
        scanned_files, parsed_files, skipped_files_without_assertion_markers, parse_failures, max_files, False
    )


def source_extension_scan_stats(
    scanned_files: int,
    parsed_files: int,
    skipped_files_without_assertion_markers: int,
    parse_failures: list[dict[str, str]],
    max_files: int | None,
    stopped_by_limit: bool,
) -> dict[str, Any]:
    return {
        "scanned_files": scanned_files,
        "parsed_files": parsed_files,
        "skipped_files_without_assertion_markers": skipped_files_without_assertion_markers,
        "parse_failure_count": len(parse_failures),
        "parse_failure_sample": parse_failures[:20],
        "max_files": max_files,
        "stopped_by_limit": stopped_by_limit,
    }


def has_assertion_marker(source_bytes: bytes) -> bool:
    return any(marker in source_bytes for marker in (b"assert", b".assert", b"pytest.raises", b"with raises"))


def iter_source_extension_candidate_statements(tree: ast.AST) -> Iterable[ast.stmt]:
    for node in ast.walk(tree):
        if isinstance(node, (ast.Assert, ast.Expr, ast.With)):
            yield node


def classify_source_extension_candidate(source_path: Path, statement: ast.stmt) -> tuple[str, str, str] | None:
    testish = is_testish_source_path(source_path)
    if isinstance(statement, ast.Assert):
        if testish:
            return ("other_assertion_like_deleted", "plain_assert", "test")
        return ("production_plain_assert_deleted", "plain_assert", "source")
    if pytest_raises_call(statement) is not None:
        return ("pytest_raises_deleted", "pytest_raises", "test" if testish else "source")
    if assertion_call(statement) is not None:
        return ("test_self_assert_deleted", "self_assert", "test" if testish else "source")
    return None


def is_testish_source_path(source_path: Path) -> bool:
    return "tests" in source_path.parts or source_path.name.startswith("test_") or source_path.name.endswith("_test.py")


def build_source_extension_candidate_row(
    *,
    task_id: str,
    task_meta: dict[str, str],
    source_path: str,
    source_sha: str,
    source_lines: list[str],
    line_starts: list[int],
    statement: ast.stmt,
    classification: tuple[str, str, str],
    candidate_index: int,
    hypotheses: list[dict[str, Any]],
) -> dict[str, Any]:
    sublabel, assertion_kind, file_kind = classification
    start_line = int(statement.lineno)
    end_line = int(getattr(statement, "end_lineno", statement.lineno) or statement.lineno)
    start_byte = line_starts[start_line - 1]
    end_byte = line_starts[end_line] if end_line < len(line_starts) else sum(
        len(line.encode("utf-8")) for line in source_lines
    )
    statement_text = "".join(source_lines[start_line - 1 : end_line])
    preview = first_nonempty_line(statement_text)
    identity = {
        "policy_version": SOURCE_EXTENSION_POLICY_VERSION,
        "source_task_id": task_id,
        "source_path": source_path,
        "source_file_sha256": source_sha,
        "source_site_offset": start_byte,
        "source_site_len": end_byte - start_byte,
        "source_site_preview": preview,
    }
    candidate_id = "sha256:" + hashlib.sha256(json.dumps(identity, sort_keys=True).encode("utf-8")).hexdigest()
    return {
        "schema_version": 1,
        "task_id": SOURCE_EXTENSION_TASK_ID,
        "artifact_kind": "delete_test_call_regeneration_candidate",
        "candidate_row_id": candidate_id,
        "candidate_index_in_source_file": candidate_index,
        "source_task_id": task_id,
        "repo": task_meta["repo"],
        "stable_partition": task_meta["stable_partition"],
        "source_path": source_path,
        "source_file_sha256": source_sha,
        "source_site_offset": start_byte,
        "source_site_len": end_byte - start_byte,
        "source_site_preview": preview,
        "assertion_kind": assertion_kind,
        "assertion_duty_sublabel": sublabel,
        "file_kind": file_kind,
        "expected_patch_builder": "git_blob_sparse",
        "active_path_policy": "aiwonder_source_root_only",
        "selected_from_existing_failure_source_file": False,
        "selected_by_source_extension_rule": True,
        "source_extension_policy_version": SOURCE_EXTENSION_POLICY_VERSION,
        "oracle_required": True,
        "oracle_status": "not_run",
        "zero_vector_or_oracle_label_live_input_used": False,
        "target_oracle_labels_used_as_features": False,
        "holdout_tuning_used": False,
        "pre_oracle_pass_hypothesis": finalize_source_extension_hypotheses(hypotheses),
    }


def first_nonempty_line(text: str) -> str:
    for line in text.splitlines():
        stripped = line.strip()
        if stripped:
            return stripped[:240]
    return text.strip()[:240]


def finalize_source_extension_hypotheses(hypotheses: list[dict[str, Any]]) -> dict[str, Any]:
    hypotheses.sort(key=lambda item: (-float(item.get("confidence") or 0.0), str(item.get("kind") or "")))
    return {
        "policy_version": SOURCE_EXTENSION_POLICY_VERSION,
        "analysis_status": "evaluated",
        "target_oracle_labels_used_as_features": False,
        "hypotheses": hypotheses,
        "hypothesis_count": len(hypotheses),
        "highest_confidence": float(hypotheses[0].get("confidence") or 0.0),
        "primary_kind": str(hypotheses[0].get("kind") or ""),
    }


def source_extension_hypotheses(
    *,
    tree: ast.AST,
    statement: ast.stmt,
    parent_map: dict[ast.AST, ast.AST],
    source_path: Path,
    classification: tuple[str, str, str],
    skip_intervals: list[tuple[int, int]],
    dead_intervals: list[tuple[int, int]],
) -> list[dict[str, Any]]:
    hypotheses: list[dict[str, Any]] = []
    tautology_kind = source_extension_tautology_kind(statement)
    if tautology_kind is not None:
        hypotheses.append(
            {
                "kind": "tautological_assertion",
                "confidence": 0.92,
                "evidence": {"tautology_kind": tautology_kind, "lineno": int(statement.lineno)},
            }
        )
    trivial_raises = trivial_pytest_raises_hypothesis(statement)
    if trivial_raises is not None:
        hypotheses.append(trivial_raises)
    pytest_raises_body = pytest_raises_call_body_hypothesis(statement)
    if pytest_raises_body is not None:
        hypotheses.append(pytest_raises_body)
    redundant = source_extension_redundant_assertion_hypothesis(tree, statement)
    if redundant is not None:
        hypotheses.append(redundant)
    guarded = guarded_production_assert_hypothesis(statement, parent_map, source_path, classification)
    if guarded is not None:
        hypotheses.append(guarded)
    if line_in_intervals(int(statement.lineno), skip_intervals):
        hypotheses.append(
            {
                "kind": "unconditional_skip_scope",
                "confidence": 0.95,
                "evidence": {"lineno": int(statement.lineno)},
            }
        )
    if line_in_intervals(int(statement.lineno), dead_intervals):
        hypotheses.append(
            {
                "kind": "literal_dead_branch",
                "confidence": 0.95,
                "evidence": {"lineno": int(statement.lineno)},
            }
        )
    return hypotheses


def source_extension_tautology_kind(statement: ast.stmt) -> str | None:
    if isinstance(statement, ast.Assert):
        return tautology_kind_for_expr(statement.test)
    call = assertion_call(statement)
    if call is None:
        return None
    method = assertion_call_method_name(call)
    args = list(call.args)
    if method in {"asserttrue", "assert_true"} and args and tautology_kind_for_expr(args[0]) is not None:
        return tautology_kind_for_expr(args[0])
    if method in {"assertfalse", "assert_false"} and args and falsehood_kind_for_expr(args[0]) is not None:
        return falsehood_kind_for_expr(args[0])
    if method in {"assertequal", "assertequals", "assertis"} and len(args) >= 2 and expr_key(args[0]) == expr_key(args[1]):
        return "same_expression_arguments"
    if method == "assertgreaterequal" and len(args) >= 2 and is_len_call(args[0]) and is_zero_constant(args[1]):
        return "len_nonnegative"
    if method == "assertlessequal" and len(args) >= 2 and is_zero_constant(args[0]) and is_len_call(args[1]):
        return "len_nonnegative"
    return None


def trivial_pytest_raises_hypothesis(statement: ast.stmt) -> dict[str, Any] | None:
    if not isinstance(statement, ast.With):
        return None
    raises_call = pytest_raises_call(statement)
    if raises_call is None or len(statement.body) != 1 or not isinstance(statement.body[0], ast.Raise):
        return None
    expected = expr_key(raises_call.args[0]) if raises_call.args else ""
    actual = raised_exception_key(statement.body[0])
    if expected and actual and expected == actual:
        return {
            "kind": "trivial_pytest_raises_block",
            "confidence": 0.8,
            "evidence": {"exception_key": expected, "lineno": int(statement.lineno)},
        }
    return None


def pytest_raises_call_body_hypothesis(statement: ast.stmt) -> dict[str, Any] | None:
    if not isinstance(statement, ast.With):
        return None
    raises_call = pytest_raises_call(statement)
    if raises_call is None:
        return None
    executable_types = [
        type(child).__name__
        for child in statement.body
        if not isinstance(child, ast.Pass) and not is_docstring_statement(child)
    ]
    if not executable_types:
        return None
    expected = expr_key(raises_call.args[0]) if raises_call.args else ""
    return {
        "kind": "pytest_raises_negative_path_deleted",
        "confidence": 0.68,
        "evidence": {
            "exception_key": expected,
            "lineno": int(statement.lineno),
            "body_statement_types": executable_types[:10],
        },
    }


def is_docstring_statement(statement: ast.stmt) -> bool:
    return (
        isinstance(statement, ast.Expr)
        and isinstance(statement.value, ast.Constant)
        and isinstance(statement.value.value, str)
    )


def source_extension_redundant_assertion_hypothesis(tree: ast.AST, statement: ast.stmt) -> dict[str, Any] | None:
    keys = assertion_semantic_keys(statement)
    if not keys or not hasattr(statement, "lineno"):
        return None
    scope = enclosing_scope(tree, int(statement.lineno))
    matches: list[int] = []
    for other in iter_scope_assertion_statements(scope):
        if assertion_semantic_keys(other) & keys:
            matches.append(int(other.lineno))
    if len(set(matches)) < 2:
        return None
    return {
        "kind": "redundant_assertion_same_scope",
        "confidence": 0.66,
        "evidence": {
            "scope_name": scope_name(scope),
            "scope_node_type": type(scope).__name__,
            "matching_assertion_count": len(set(matches)),
            "matching_lines": sorted(set(matches))[:20],
            "semantic_keys": sorted(keys)[:10],
            "lineno": int(statement.lineno),
        },
    }


def guarded_production_assert_hypothesis(
    statement: ast.stmt,
    parent_map: dict[ast.AST, ast.AST],
    source_path: Path,
    classification: tuple[str, str, str],
) -> dict[str, Any] | None:
    sublabel, assertion_kind, _file_kind = classification
    if sublabel != "production_plain_assert_deleted" or assertion_kind != "plain_assert" or not isinstance(statement, ast.Assert):
        return None
    if is_testish_source_path(source_path):
        return None
    body, index = containing_statement_body(parent_map, statement)
    if body is None or index is None or index <= 0:
        return None
    guard_lines: list[int] = []
    for previous in body[:index]:
        if production_assert_guard_matches(previous, statement.test):
            guard_lines.append(int(previous.lineno))
    if not guard_lines:
        return None
    return {
        "kind": "production_assert_redundant_guarded_invariant",
        "confidence": 0.7,
        "evidence": {
            "assert_lineno": int(statement.lineno),
            "guard_lines": guard_lines[-5:],
            "source_path": str(source_path),
        },
    }


def production_assert_guard_matches(statement: ast.stmt, assert_expr: ast.expr) -> bool:
    if not isinstance(statement, ast.If):
        return False
    if expression_negates(statement.test, assert_expr) and branch_exits(statement.body):
        return True
    if expr_key(statement.test) == expr_key(assert_expr) and branch_exits(statement.orelse):
        return True
    return False


def branch_exits(statements: list[ast.stmt]) -> bool:
    return bool(statements) and all(statement_always_exits(statement) for statement in statements)


def statement_always_exits(statement: ast.stmt) -> bool:
    if isinstance(statement, (ast.Raise, ast.Return, ast.Break, ast.Continue)):
        return True
    if isinstance(statement, ast.If):
        return branch_exits(statement.body) and branch_exits(statement.orelse)
    return False


def precompute_source_extension_intervals(tree: ast.AST) -> tuple[list[tuple[int, int]], list[tuple[int, int]]]:
    skip_intervals: list[tuple[int, int]] = []
    dead_intervals: list[tuple[int, int]] = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and any(
            is_unconditional_skip_decorator(decorator) for decorator in node.decorator_list
        ):
            skip_intervals.append((int(node.lineno), int(getattr(node, "end_lineno", node.lineno) or node.lineno)))
        if isinstance(node, ast.If):
            value = literal_bool(node.test)
            if value is False:
                dead_intervals.extend(statement_intervals(node.body))
            elif value is True:
                dead_intervals.extend(statement_intervals(node.orelse))
    return skip_intervals, dead_intervals


def statement_intervals(statements: list[ast.stmt]) -> list[tuple[int, int]]:
    return [
        (int(statement.lineno), int(getattr(statement, "end_lineno", statement.lineno) or statement.lineno))
        for statement in statements
        if hasattr(statement, "lineno")
    ]


def line_in_intervals(line: int, intervals: list[tuple[int, int]]) -> bool:
    return any(start <= line <= end for start, end in intervals)


def is_unconditional_skip_decorator(decorator: ast.expr) -> bool:
    name = call_name(decorator.func if isinstance(decorator, ast.Call) else decorator).lower()
    args = list(decorator.args) if isinstance(decorator, ast.Call) else []
    if name.endswith("pytest.mark.skip") or name.endswith("mark.skip") or name.endswith("unittest.skip"):
        return True
    if name.endswith(".skip") or name == "skip":
        return True
    if (name.endswith("skipif") or name.endswith("skip_if")) and args and literal_bool(args[0]) is True:
        return True
    if (name.endswith("skipunless") or name.endswith("skip_unless")) and args and literal_bool(args[0]) is False:
        return True
    return False


def literal_bool(expr: ast.expr) -> bool | None:
    if isinstance(expr, ast.Constant):
        if isinstance(expr.value, bool):
            return expr.value
        if expr.value in (0, 1):
            return bool(expr.value)
        if expr.value in ("", None):
            return False
    if isinstance(expr, (ast.List, ast.Tuple, ast.Set)) and not expr.elts:
        return False
    if isinstance(expr, ast.Dict) and not expr.keys:
        return False
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.Not):
        value = literal_bool(expr.operand)
        return None if value is None else not value
    return None


def build_parent_map(tree: ast.AST) -> dict[ast.AST, ast.AST]:
    parent_map: dict[ast.AST, ast.AST] = {}
    for parent in ast.walk(tree):
        for child in ast.iter_child_nodes(parent):
            parent_map[child] = parent
    return parent_map


def assertion_call(statement: ast.stmt) -> ast.Call | None:
    if not isinstance(statement, ast.Expr) or not isinstance(statement.value, ast.Call):
        return None
    name = call_name(statement.value.func)
    if ".assert" in name or name.startswith("assert"):
        return statement.value
    return None


def assertion_call_method_name(call: ast.Call) -> str:
    return call_name(call.func).rsplit(".", 1)[-1].lower()


def call_name(expr: ast.expr) -> str:
    if isinstance(expr, ast.Name):
        return expr.id
    if isinstance(expr, ast.Attribute):
        prefix = call_name(expr.value)
        return f"{prefix}.{expr.attr}" if prefix else expr.attr
    return ""


def containing_statement_body(
    parent_map: dict[ast.AST, ast.AST],
    statement: ast.stmt,
) -> tuple[list[ast.stmt] | None, int | None]:
    current: ast.AST = statement
    parent = parent_map.get(current)
    while parent is not None:
        for _field_name, value in ast.iter_fields(parent):
            if isinstance(value, list) and current in value:
                if all(isinstance(item, ast.stmt) for item in value):
                    return value, value.index(current)
        current = parent
        parent = parent_map.get(current)
    return None, None


def enclosing_scope(tree: ast.AST, line: int) -> ast.AST:
    scopes: list[ast.AST] = [tree]
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            continue
        end_line = int(getattr(node, "end_lineno", None) or node.lineno)
        if int(node.lineno) <= line <= end_line:
            scopes.append(node)
    scopes.sort(key=scope_span)
    return scopes[0]


def scope_span(node: ast.AST) -> int:
    if isinstance(node, ast.Module):
        return 10**9
    return int(getattr(node, "end_lineno", 0) or 0) - int(getattr(node, "lineno", 0) or 0)


def scope_name(node: ast.AST) -> str:
    if isinstance(node, ast.Module):
        return "<module>"
    return str(getattr(node, "name", type(node).__name__))


def iter_scope_assertion_statements(scope: ast.AST) -> Iterable[ast.stmt]:
    def walk(node: ast.AST, *, root: bool = False) -> Iterable[ast.stmt]:
        if not root and isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            return
        if isinstance(node, (ast.Assert, ast.Expr, ast.With)) and assertion_semantic_keys(node):
            yield node
        for child in ast.iter_child_nodes(node):
            yield from walk(child)

    yield from walk(scope, root=True)


def assertion_semantic_keys(statement: ast.stmt) -> set[str]:
    if isinstance(statement, ast.Assert):
        return expression_assertion_keys(statement.test)
    call = assertion_call(statement)
    if call is None:
        if isinstance(statement, ast.With):
            raises = pytest_raises_call(statement)
            if raises is not None and raises.args:
                return {f"pytest_raises:{expr_key(raises.args[0])}"}
        return set()
    method = assertion_call_method_name(call)
    args = list(call.args)
    if method in {"assertequal", "assertequals"} and len(args) >= 2:
        return {commutative_key("equals", args[0], args[1])}
    if method == "assertis" and len(args) >= 2:
        return {commutative_key("is", args[0], args[1])}
    if method == "assertisinstance" and len(args) >= 2:
        return {f"isinstance:{expr_key(args[0])}:{expr_key(args[1])}"}
    if method in {"asserttrue", "assert_true"} and args:
        return {f"truthy:{expr_key(args[0])}"}
    if method in {"assertfalse", "assert_false"} and args:
        return {f"falsey:{expr_key(args[0])}"}
    return set()


def expression_assertion_keys(expr: ast.expr) -> set[str]:
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        op = expr.ops[0]
        right = expr.comparators[0]
        if isinstance(op, ast.Eq):
            return {commutative_key("equals", expr.left, right)}
        if isinstance(op, ast.Is):
            return {commutative_key("is", expr.left, right)}
    if isinstance(expr, ast.Call) and call_name(expr.func) == "isinstance" and len(expr.args) >= 2:
        return {f"isinstance:{expr_key(expr.args[0])}:{expr_key(expr.args[1])}"}
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.Not):
        return {f"falsey:{expr_key(expr.operand)}"}
    return {f"truthy:{expr_key(expr)}"}


def commutative_key(prefix: str, left: ast.expr, right: ast.expr) -> str:
    first, second = sorted((expr_key(left), expr_key(right)))
    return f"{prefix}:{first}:{second}"


def expression_negates(left: ast.expr, right: ast.expr) -> bool:
    if isinstance(left, ast.UnaryOp) and isinstance(left.op, ast.Not):
        return expr_key(left.operand) == expr_key(right)
    if isinstance(right, ast.UnaryOp) and isinstance(right.op, ast.Not):
        return expr_key(left) == expr_key(right.operand)
    return falsehood_kind_for_expr(
        ast.Compare(left=left, ops=[ast.NotEq()], comparators=[right])
    ) is not None


def expr_key(expr: ast.AST) -> str:
    return ast.dump(expr, annotate_fields=True, include_attributes=False)


def tautology_kind_for_expr(expr: ast.expr) -> str | None:
    if isinstance(expr, ast.Constant) and bool(expr.value) is True:
        return "truthy_constant"
    if isinstance(expr, ast.UnaryOp) and isinstance(expr.op, ast.Not):
        return falsehood_kind_for_expr(expr.operand)
    if isinstance(expr, ast.BoolOp) and isinstance(expr.op, ast.Or):
        if any(tautology_kind_for_expr(value) is not None for value in expr.values):
            return "or_contains_tautology"
        if boolop_has_complementary_pair(expr.values):
            return "or_complementary_pair"
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        left = expr.left
        right = expr.comparators[0]
        op = expr.ops[0]
        if isinstance(op, (ast.Eq, ast.Is)) and expr_key(left) == expr_key(right):
            return "same_expression_comparison"
        if isinstance(op, ast.GtE) and is_len_call(left) and is_zero_constant(right):
            return "len_nonnegative"
        if isinstance(op, ast.LtE) and is_zero_constant(left) and is_len_call(right):
            return "len_nonnegative"
        if isinstance(op, ast.In) and expression_tuple_contains(right, left):
            return "self_membership_tuple"
    return None


def falsehood_kind_for_expr(expr: ast.expr) -> str | None:
    if isinstance(expr, ast.Constant) and bool(expr.value) is False:
        return "falsey_constant"
    if isinstance(expr, ast.BoolOp) and isinstance(expr.op, ast.And):
        if any(falsehood_kind_for_expr(value) is not None for value in expr.values):
            return "and_contains_falsehood"
        if boolop_has_complementary_pair(expr.values):
            return "and_complementary_pair"
    if isinstance(expr, ast.Compare) and len(expr.ops) == 1 and len(expr.comparators) == 1:
        if isinstance(expr.ops[0], (ast.NotEq, ast.IsNot)) and expr_key(expr.left) == expr_key(expr.comparators[0]):
            return "same_expression_negative_comparison"
    return None


def boolop_has_complementary_pair(values: list[ast.expr]) -> bool:
    plain = {expr_key(value) for value in values if not (isinstance(value, ast.UnaryOp) and isinstance(value.op, ast.Not))}
    negated = {expr_key(value.operand) for value in values if isinstance(value, ast.UnaryOp) and isinstance(value.op, ast.Not)}
    return bool(plain & negated)


def expression_tuple_contains(container: ast.expr, needle: ast.expr) -> bool:
    if not isinstance(container, (ast.Tuple, ast.List, ast.Set)):
        return False
    needle_key = expr_key(needle)
    return any(expr_key(item) == needle_key for item in container.elts)


def is_zero_constant(expr: ast.expr) -> bool:
    return isinstance(expr, ast.Constant) and expr.value == 0


def is_len_call(expr: ast.expr) -> bool:
    return isinstance(expr, ast.Call) and call_name(expr.func) == "len" and len(expr.args) == 1


def pytest_raises_call(statement: ast.With) -> ast.Call | None:
    if not isinstance(statement, ast.With):
        return None
    for item in statement.items:
        call = item.context_expr
        if isinstance(call, ast.Call) and call_name(call.func).endswith("pytest.raises"):
            return call
        if isinstance(call, ast.Call) and call_name(call.func) == "raises":
            return call
    return None


def raised_exception_key(node: ast.Raise) -> str:
    exc = node.exc
    if isinstance(exc, ast.Call):
        return expr_key(exc.func)
    if isinstance(exc, ast.expr):
        return expr_key(exc)
    return ""


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
