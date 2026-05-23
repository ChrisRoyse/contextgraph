from __future__ import annotations

import hashlib
import json
import os
import shutil
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

from .core import SwebenchOpsError, sha256_file, utc_now, write_json_checked
from .oracle_events import canonical_json, load_jsonschema_validator, sha256_prefixed_text, write_jsonl_checked
from .python_ast_chunk_command import validate_chunks
from .python_ast_chunks import chunk_schema, chunks_for_record
from .python_ast_parse import parse_python_bytes, record_schema, validate_records
from .python_delete_test_call_regeneration import (
    DEFAULT_OUTPUT_ROOT as DEFAULT_PATCH_ROOT,
    build_unified_diff,
    has_mounted_drive_fragment,
    leading_whitespace,
    line_ending,
)


TASK_ID = "TASK-PY-G-135C"
DEFAULT_PATCHES_JSONL = DEFAULT_PATCH_ROOT / "delete_test_call_candidate_patches.jsonl"
DEFAULT_OUTCOME_SEARCH_ROOT = Path("/zfs/archive/contextgraph/state/regeneration-plans")
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135c-candidate-keyed-corpus-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135c-candidate-keyed-corpus-materializer-fsv"
)


class CandidateCorpusError(SwebenchOpsError):
    pass


def add_arguments(parser) -> None:
    parser.add_argument("--candidate-patches", type=Path, default=DEFAULT_PATCHES_JSONL)
    parser.add_argument("--outcomes-jsonl", type=Path, action="append", default=None)
    parser.add_argument("--outcome-root", type=Path, action="append", default=None)
    parser.add_argument("--outcome-search-root", type=Path, default=DEFAULT_OUTCOME_SEARCH_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--candidate-row-id", action="append", default=None)
    parser.add_argument("--max-outcomes", type=int, default=None)
    parser.add_argument("--clean-output", action="store_true")


def command_python_delete_test_call_candidate_corpus(args) -> dict[str, Any]:
    validate_args(args)
    outcome_paths = resolve_outcome_paths(args)
    candidates = load_candidate_patches(args.candidate_patches)
    outcomes = load_outcomes(outcome_paths)
    outcomes = filter_outcomes(outcomes, normalize_repeated_ids(args.candidate_row_id), args.max_outcomes)
    if not outcomes:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_NO_OUTCOMES",
            "no candidate oracle outcomes selected",
            "Select at least one candidate-keyed #615 oracle outcome row.",
            {"outcome_paths": [str(path) for path in outcome_paths], "candidate_row_id": args.candidate_row_id},
        )

    output_root = args.output_root
    if args.clean_output and output_root.exists():
        shutil.rmtree(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    schema_root = output_root / "schemas"
    source_root = output_root / "source-files"
    parse_schema_path = schema_root / "candidate_python_ast_parse_schema.json"
    chunk_schema_path = schema_root / "candidate_python_ast_chunk_schema.json"
    write_json_checked(parse_schema_path, record_schema(parse_schema_path))
    write_json_checked(chunk_schema_path, chunk_schema(chunk_schema_path))
    parse_validator = load_jsonschema_validator(record_schema(parse_schema_path))
    chunk_validator = load_jsonschema_validator(chunk_schema(chunk_schema_path))

    now = utc_now()
    corpus_rows: list[dict[str, Any]] = []
    source_rows: list[dict[str, Any]] = []
    parse_records: list[dict[str, Any]] = []
    chunks: list[dict[str, Any]] = []
    chunk_links: list[dict[str, Any]] = []
    rejections: list[dict[str, Any]] = []
    seen_candidate_ids: set[str] = set()

    for index, outcome in enumerate(outcomes):
        candidate_id = str(outcome.get("candidate_row_id") or "")
        try:
            if candidate_id in seen_candidate_ids:
                raise CandidateCorpusError(
                    "DTC_CANDIDATE_CORPUS_DUPLICATE_OUTCOME_ID",
                    f"duplicate outcome candidate_row_id: {candidate_id}",
                    "Provide each candidate outcome once, or split conflicting attempts into separate corpus roots.",
                    {"candidate_row_id": candidate_id},
                )
            seen_candidate_ids.add(candidate_id)
            candidate = candidates.get(candidate_id)
            if candidate is None:
                raise CandidateCorpusError(
                    "DTC_CANDIDATE_CORPUS_PATCH_ROW_MISSING",
                    f"outcome has no matching candidate patch row: {candidate_id}",
                    "Regenerate or pass the candidate patches JSONL used by the oracle executor.",
                    {"candidate_row_id": candidate_id},
                )
            materialized = materialize_candidate(
                outcome=outcome,
                candidate=candidate,
                output_source_root=source_root,
                parse_schema_path=parse_schema_path,
                chunk_schema_path=chunk_schema_path,
                parse_validator=parse_validator,
                chunk_validator=chunk_validator,
                now=now,
            )
            corpus_rows.append(materialized["corpus_row"])
            source_rows.extend(materialized["source_rows"])
            parse_records.append(materialized["parse_record"])
            chunks.extend(materialized["chunks"])
            chunk_links.extend(materialized["chunk_links"])
        except SwebenchOpsError as exc:
            rejections.append(
                {
                    "row_index": index,
                    "candidate_row_id": candidate_id or outcome.get("candidate_row_id"),
                    "source_task_id": outcome.get("source_task_id"),
                    **exc.to_json(),
                }
            )

    if parse_records:
        validate_records(parse_records, parse_validator)
    if chunks:
        validate_chunks(chunks, chunk_validator)

    corpus_path = output_root / "candidate_keyed_corpus_rows.jsonl"
    source_path = output_root / "candidate_keyed_source_files.jsonl"
    parse_path = output_root / "candidate_keyed_parse_records.jsonl"
    chunks_path = output_root / "candidate_keyed_ast_chunks.jsonl"
    links_path = output_root / "candidate_keyed_ast_chunk_links.jsonl"
    rejections_path = output_root / "candidate_keyed_materializer_rejections.jsonl"
    manifest_path = output_root / "candidate_keyed_corpus_manifest.json"

    corpus_store = write_jsonl_checked(corpus_path, corpus_rows)
    source_store = write_jsonl_checked(source_path, source_rows)
    parse_store = write_jsonl_checked(parse_path, parse_records)
    chunk_store = write_jsonl_checked(chunks_path, chunks)
    link_store = write_jsonl_checked(links_path, chunk_links)
    rejection_store = write_jsonl_checked(rejections_path, rejections)

    by_partition_sublabel_status = Counter(
        (
            row.get("stable_partition", "unknown"),
            row.get("assertion_duty_sublabel", "unknown"),
            row.get("oracle_status", "unknown"),
        )
        for row in corpus_rows
    )
    manifest = {
        "schema_version": 1,
        "task_id": TASK_ID,
        "artifact_kind": "delete_test_call_candidate_keyed_corpus_manifest",
        "created_at": now,
        "candidate_patches": str(args.candidate_patches),
        "candidate_patches_sha256": sha256_file(args.candidate_patches),
        "outcomes_jsonl": [str(path) for path in outcome_paths],
        "outcomes_jsonl_sha256": {str(path): sha256_file(path) for path in outcome_paths},
        "output_root": str(output_root),
        "source_root": str(source_root),
        "selected_outcome_rows": len(outcomes),
        "duplicate_outcome_source_rows": sum(
            max(0, len(row.get("_duplicate_outcome_sources") or []) - 1)
            for row in outcomes
        ),
        "accepted_candidate_rows": len(corpus_rows),
        "rejected_candidate_rows": len(rejections),
        "parse_record_rows": len(parse_records),
        "ast_chunk_rows": len(chunks),
        "chunk_link_rows": len(chunk_links),
        "source_file_rows": len(source_rows),
        "distinct_candidate_row_ids": len({row["candidate_row_id"] for row in corpus_rows}),
        "distinct_source_task_ids": len({row["source_task_id"] for row in corpus_rows}),
        "counts_by_partition_sublabel_status": counter_to_dict(by_partition_sublabel_status),
        "source_of_truth": {
            "corpus_rows_jsonl": str(corpus_path),
            "source_files_jsonl": str(source_path),
            "parse_records_jsonl": str(parse_path),
            "ast_chunks_jsonl": str(chunks_path),
            "ast_chunk_links_jsonl": str(links_path),
            "rejections_jsonl": str(rejections_path),
            "manifest": str(manifest_path),
            "parse_schema": str(parse_schema_path),
            "chunk_schema": str(chunk_schema_path),
        },
        "sha256": {
            "corpus_rows": corpus_store["sha256"],
            "source_files": source_store["sha256"],
            "parse_records": parse_store["sha256"],
            "ast_chunks": chunk_store["sha256"],
            "ast_chunk_links": link_store["sha256"],
            "rejections": rejection_store["sha256"],
        },
        "candidate_row_id_keyed": True,
        "category_collapse_prevented": len(corpus_rows) == len({row["candidate_row_id"] for row in corpus_rows}),
        "target_oracle_labels_used_as_features": False,
        "oracle_outcome_target_only": True,
        "mounted_drive_lineage_rejected": True,
        "all_passed": bool(corpus_rows) and not rejections,
    }
    write_json_checked(manifest_path, manifest)

    readback = {
        "manifest_exists": manifest_path.is_file(),
        "corpus_line_count": count_jsonl(corpus_path),
        "source_line_count": count_jsonl(source_path),
        "parse_record_line_count": count_jsonl(parse_path),
        "chunk_line_count": count_jsonl(chunks_path),
        "chunk_link_line_count": count_jsonl(links_path),
        "rejection_line_count": count_jsonl(rejections_path),
        "manifest_sha256": sha256_file(manifest_path),
        "distinct_candidate_row_ids_in_corpus": count_distinct_jsonl(corpus_path, "candidate_row_id"),
        "distinct_source_task_ids_in_corpus": count_distinct_jsonl(corpus_path, "source_task_id"),
        "materialized_mutated_files": sum(1 for row in source_rows if row["workspace_state"] == "candidate_mutated"),
    }
    return {
        "ok": manifest["all_passed"],
        "all_passed": manifest["all_passed"],
        "command": "python-delete-test-call-candidate-corpus",
        "checked_at": utc_now(),
        "manifest": manifest,
        "readback": readback,
        "rejection_sample": rejections[:10],
        "corpus_sample": corpus_rows[:3],
    }


def validate_args(args) -> None:
    if args.max_outcomes is not None and args.max_outcomes <= 0:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_BAD_MAX_OUTCOMES",
            f"--max-outcomes must be positive when set: {args.max_outcomes}",
            "Use a positive outcome limit or omit --max-outcomes.",
            {"max_outcomes": args.max_outcomes},
        )
    paths = [args.candidate_patches, args.output_root, args.readback_root, args.evidence_root, args.outcome_search_root]
    paths.extend(args.outcomes_jsonl or [])
    paths.extend(args.outcome_root or [])
    for path in paths:
        if has_mounted_drive_fragment(path):
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_MOUNTED_PATH_REJECTED",
                f"mounted-drive path is not active lineage: {path}",
                "Use aiwonder /zfs/archive/contextgraph or /zfs/hot/contextgraph paths only.",
                {"path": str(path)},
            )
    if not args.candidate_patches.is_file():
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_PATCHES_MISSING",
            f"candidate patches JSONL missing: {args.candidate_patches}",
            "Run python-delete-test-call-regeneration before candidate corpus materialization.",
            {"candidate_patches": str(args.candidate_patches)},
        )


def resolve_outcome_paths(args) -> list[Path]:
    paths: list[Path] = []
    for root in args.outcome_root or []:
        path = root / "delete_test_call_oracle_outcomes.jsonl"
        if not path.is_file():
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_OUTCOME_ROOT_MISSING_JSONL",
                f"outcome root does not contain delete_test_call_oracle_outcomes.jsonl: {root}",
                "Pass a #615 oracle outcome root or explicit --outcomes-jsonl file.",
                {"outcome_root": str(root), "expected_jsonl": str(path)},
            )
        paths.append(path)
    paths.extend(args.outcomes_jsonl or [])
    if not paths:
        paths = sorted(
            args.outcome_search_root.glob(
                "task-py-g-135c-delete-test-call-regeneration-v3-oracle-outcomes*/"
                "delete_test_call_oracle_outcomes.jsonl"
            )
        )
    if not paths:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_NO_OUTCOME_PATHS",
            "no #615 oracle outcome JSONL files were found",
            "Pass --outcomes-jsonl or --outcome-root for the candidate-keyed oracle evidence.",
            {"outcome_search_root": str(args.outcome_search_root)},
        )
    missing = [str(path) for path in paths if not path.is_file()]
    if missing:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_OUTCOME_JSONL_MISSING",
            f"outcome JSONL files are missing: {missing[:3]}",
            "Pass only existing #615 oracle outcome JSONLs.",
            {"missing": missing},
        )
    return unique_paths(paths)


def load_candidate_patches(path: Path) -> dict[str, dict[str, Any]]:
    rows: dict[str, dict[str, Any]] = {}
    for line_number, row in enumerate(read_jsonl(path, "DTC_CANDIDATE_CORPUS_PATCHES_BAD_JSONL"), start=1):
        candidate_id = require_string(row, "candidate_row_id")
        if candidate_id in rows:
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_DUPLICATE_PATCH_ID",
                f"duplicate candidate patch id: {candidate_id}",
                "Regenerate patch rows with unique candidate ids.",
                {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
            )
        rows[candidate_id] = row
    if not rows:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_PATCHES_EMPTY",
            f"candidate patch JSONL is empty: {path}",
            "Run python-delete-test-call-regeneration before candidate corpus materialization.",
            {"path": str(path)},
        )
    return rows


def load_outcomes(paths: list[Path]) -> list[dict[str, Any]]:
    outcomes_by_id: dict[str, dict[str, Any]] = {}
    for path in paths:
        for line_number, row in enumerate(read_jsonl(path, "DTC_CANDIDATE_CORPUS_OUTCOMES_BAD_JSONL"), start=1):
            candidate_id = require_string(row, "candidate_row_id")
            row = dict(row)
            row["_outcome_jsonl"] = str(path)
            row["_outcome_line_number"] = line_number
            row["_duplicate_outcome_sources"] = [{"path": str(path), "line_number": line_number}]
            previous = outcomes_by_id.get(candidate_id)
            if previous is not None:
                if duplicate_outcome_key(previous) != duplicate_outcome_key(row):
                    raise CandidateCorpusError(
                        "DTC_CANDIDATE_CORPUS_CONFLICTING_DUPLICATE_OUTCOME_ID",
                        f"duplicate candidate outcome id has conflicting target/report evidence: {candidate_id}",
                        "Resolve conflicting oracle attempts before materializing candidate training data.",
                        {
                            "candidate_row_id": candidate_id,
                            "previous": duplicate_outcome_key(previous),
                            "current": duplicate_outcome_key(row),
                            "current_path": str(path),
                            "current_line_number": line_number,
                        },
                    )
                previous["_duplicate_outcome_sources"].append({"path": str(path), "line_number": line_number})
                continue
            outcomes_by_id[candidate_id] = row
    return list(outcomes_by_id.values())


def filter_outcomes(outcomes: list[dict[str, Any]], candidate_ids: list[str] | None, max_outcomes: int | None) -> list[dict[str, Any]]:
    selected = outcomes
    if candidate_ids:
        wanted = set(candidate_ids)
        selected = [row for row in selected if row.get("candidate_row_id") in wanted]
        missing = sorted(wanted - {str(row.get("candidate_row_id")) for row in selected})
        if missing:
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_SELECTED_ID_MISSING",
                f"selected candidate ids are absent from outcome rows: {missing[:5]}",
                "Select candidate ids from the provided #615 oracle outcomes.",
                {"missing_candidate_row_ids": missing},
            )
    if max_outcomes is not None:
        selected = selected[:max_outcomes]
    return selected


def materialize_candidate(
    *,
    outcome: dict[str, Any],
    candidate: dict[str, Any],
    output_source_root: Path,
    parse_schema_path: Path,
    chunk_schema_path: Path,
    parse_validator: Any,
    chunk_validator: Any,
    now: str,
) -> dict[str, Any]:
    validate_joined_rows(outcome, candidate)
    candidate_id = require_string(outcome, "candidate_row_id")
    source_task_id = require_string(outcome, "source_task_id")
    source_path = require_relative_python_path(require_string(outcome, "source_path"), "source_path")
    candidate_key = safe_component(candidate_id.removeprefix("sha256:"))
    base_source_file = Path(require_string(candidate, "source_file"))
    if has_mounted_drive_fragment(base_source_file):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_MOUNTED_SOURCE_FILE_REJECTED",
            f"candidate source_file points at mounted-drive lineage: {base_source_file}",
            "Regenerate candidate patch rows against aiwonder source snapshots.",
            {"candidate_row_id": candidate_id, "source_file": str(base_source_file)},
        )
    if not base_source_file.is_file():
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_SOURCE_FILE_MISSING",
            f"source file missing for candidate {candidate_id}: {base_source_file}",
            "Restore the canonical #613 source snapshot or reject this candidate.",
            {"candidate_row_id": candidate_id, "source_file": str(base_source_file)},
        )

    base_bytes = base_source_file.read_bytes()
    expected_base_sha = require_string(candidate, "source_file_sha256")
    actual_base_sha = sha256_prefixed_bytes(base_bytes)
    if actual_base_sha != expected_base_sha:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_SOURCE_SHA_MISMATCH",
            f"source SHA mismatch for candidate {candidate_id}",
            "Do not materialize candidate data from source bytes that differ from the patch plan.",
            {"candidate_row_id": candidate_id, "expected": expected_base_sha, "actual": actual_base_sha},
        )

    base_text = base_bytes.decode("utf-8")
    mutated_text = mutate_source_text(base_text, candidate)
    expected_patch_sha = require_string(outcome, "patch_sha256")
    patch_text = build_unified_diff(source_path, base_text, mutated_text)
    actual_patch_sha = sha256_prefixed_text(patch_text)
    if actual_patch_sha != expected_patch_sha:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_PATCH_SHA_RECONSTRUCTION_MISMATCH",
            f"reconstructed patch SHA mismatch for candidate {candidate_id}",
            "Regenerate candidate patch rows from the same source bytes used by oracle execution.",
            {"candidate_row_id": candidate_id, "expected": expected_patch_sha, "actual": actual_patch_sha},
        )
    patch_path = Path(require_string(outcome, "patch_path"))
    if has_mounted_drive_fragment(patch_path):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_MOUNTED_PATCH_PATH_REJECTED",
            f"candidate patch_path points at mounted-drive lineage: {patch_path}",
            "Use aiwonder patch artifacts only.",
            {"candidate_row_id": candidate_id, "patch_path": str(patch_path)},
        )
    if not patch_path.is_file():
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_PATCH_FILE_MISSING",
            f"patch file missing for candidate {candidate_id}: {patch_path}",
            "Restore the candidate patch file before corpus materialization.",
            {"candidate_row_id": candidate_id, "patch_path": str(patch_path)},
        )
    if sha256_file(patch_path) != expected_patch_sha:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_PATCH_FILE_SHA_MISMATCH",
            f"patch file SHA mismatch for candidate {candidate_id}",
            "Do not trust an outcome row whose patch file bytes differ from the recorded oracle patch.",
            {"candidate_row_id": candidate_id, "patch_path": str(patch_path), "expected": expected_patch_sha, "actual": sha256_file(patch_path)},
        )

    validate_report_artifacts(outcome)
    candidate_root = output_source_root / candidate_key
    base_out = candidate_root / "base" / source_path
    mutated_out = candidate_root / "candidate_mutated" / source_path
    write_bytes_checked(base_out, base_bytes)
    mutated_bytes = mutated_text.encode("utf-8")
    write_bytes_checked(mutated_out, mutated_bytes)
    mutated_sha = sha256_prefixed_bytes(mutated_bytes)

    workspace_tree_hash = hashlib.sha1(mutated_bytes).hexdigest()
    workspace = {
        "task_id": source_task_id,
        "state": f"candidate_mutated_{candidate_key[:24]}",
        "mutation_category": "delete_test_call",
        "repo": require_string(outcome, "repo"),
        "base_commit": str(candidate.get("base_commit") or outcome.get("base_commit") or "source-snapshot"),
        "result_tree_hash": workspace_tree_hash,
        "hot_workspace_path": str(candidate_root / "candidate_mutated"),
    }
    parse_record = parse_python_bytes(
        schema_path=parse_schema_path,
        workspace=workspace,
        absolute_path=mutated_out,
        relative_path=source_path,
        raw=mutated_bytes,
        now=now,
    )
    validate_records([parse_record], parse_validator)
    record_chunks, diagnostics = chunks_for_record(parse_record, chunk_schema_path, now)
    validate_chunks(record_chunks, chunk_validator)
    if not record_chunks:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_AST_CHUNKS_EMPTY",
            f"candidate produced no AST chunks: {candidate_id}",
            "Inspect the materialized source and parser diagnostics before using this row.",
            {"candidate_row_id": candidate_id, "parse_status": parse_record.get("parse_status"), "diagnostics": diagnostics},
        )

    candidate_corpus_row_id = sha256_prefixed_text(
        canonical_json(
            {
                "candidate_row_id": candidate_id,
                "source_task_id": source_task_id,
                "source_path": source_path,
                "patch_sha256": expected_patch_sha,
                "oracle_status": outcome.get("oracle_status"),
            }
        )
    )
    corpus_row = {
        "schema_version": 1,
        "artifact_kind": "delete_test_call_candidate_keyed_corpus_row",
        "candidate_corpus_row_id": candidate_corpus_row_id,
        "candidate_row_id": candidate_id,
        "source_task_id": source_task_id,
        "repo": require_string(outcome, "repo"),
        "source_path": source_path,
        "mutation_category": "delete_test_call",
        "assertion_duty_sublabel": require_string(outcome, "assertion_duty_sublabel"),
        "stable_partition": require_string(outcome, "stable_partition"),
        "oracle_status": require_string(outcome, "oracle_status"),
        "oracle_all_passed": bool(outcome.get("oracle_all_passed")),
        "resolved": bool(outcome.get("resolved")),
        "target": {
            "oracle_all_passed": bool(outcome.get("oracle_all_passed")),
            "resolved": bool(outcome.get("resolved")),
            "oracle_status": require_string(outcome, "oracle_status"),
        },
        "target_oracle_label_used_as_feature": False,
        "oracle_outcome_target_only": True,
        "live_feature_fields_exclude_target": True,
        "patch_path": str(patch_path),
        "patch_sha256": expected_patch_sha,
        "source_file": str(base_source_file),
        "source_file_sha256": actual_base_sha,
        "materialized_base_path": str(base_out),
        "materialized_mutated_path": str(mutated_out),
        "materialized_mutated_sha256": mutated_sha,
        "parse_record_id": parse_record["record_id"],
        "parse_status": parse_record["parse_status"],
        "ast_chunk_count": len(record_chunks),
        "workspace_state": workspace["state"],
        "workspace_tree_hash": workspace_tree_hash,
        "workspace_tree_hash_kind": "sha1_of_materialized_candidate_source",
        "selected_statement": candidate.get("selected_statement"),
        "source_site_effective_offset": candidate.get("source_site_effective_offset"),
        "outcome_jsonl": outcome.get("_outcome_jsonl"),
        "outcome_line_number": outcome.get("_outcome_line_number"),
        "report_path": outcome.get("report_path"),
        "report_sha256": outcome.get("report_sha256"),
        "test_output_path": outcome.get("test_output_path"),
        "test_output_sha256": outcome.get("test_output_sha256"),
    }
    source_rows = [
        source_file_row(
            candidate_corpus_row_id=candidate_corpus_row_id,
            candidate_id=candidate_id,
            source_task_id=source_task_id,
            workspace_state="base",
            source_path=source_path,
            path=base_out,
            sha256=actual_base_sha,
        ),
        source_file_row(
            candidate_corpus_row_id=candidate_corpus_row_id,
            candidate_id=candidate_id,
            source_task_id=source_task_id,
            workspace_state="candidate_mutated",
            source_path=source_path,
            path=mutated_out,
            sha256=mutated_sha,
        ),
    ]
    chunk_links = [
        {
            "schema_version": 1,
            "artifact_kind": "delete_test_call_candidate_ast_chunk_link",
            "candidate_corpus_row_id": candidate_corpus_row_id,
            "candidate_row_id": candidate_id,
            "source_task_id": source_task_id,
            "source_path": source_path,
            "patch_sha256": expected_patch_sha,
            "oracle_status": corpus_row["oracle_status"],
            "oracle_all_passed": corpus_row["oracle_all_passed"],
            "target_oracle_label_used_as_feature": False,
            "chunk_id": chunk["chunk_id"],
            "chunk_kind": chunk["chunk_kind"],
            "chunk_source_sha256": chunk["source_sha256"],
            "materialized_mutated_path": str(mutated_out),
            "materialized_mutated_sha256": mutated_sha,
        }
        for chunk in record_chunks
    ]
    return {
        "corpus_row": corpus_row,
        "source_rows": source_rows,
        "parse_record": parse_record,
        "chunks": record_chunks,
        "chunk_links": chunk_links,
    }


def validate_joined_rows(outcome: dict[str, Any], candidate: dict[str, Any]) -> None:
    candidate_id = require_string(outcome, "candidate_row_id")
    require_string(candidate, "candidate_row_id")
    if candidate["candidate_row_id"] != candidate_id:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_JOIN_ID_MISMATCH",
            "joined candidate and outcome rows have different candidate ids",
            "Regenerate candidate/outcome inputs from a consistent #615 run.",
            {"outcome_candidate_row_id": candidate_id, "candidate_candidate_row_id": candidate.get("candidate_row_id")},
        )
    for field in ("source_task_id", "source_path", "patch_sha256"):
        if str(outcome.get(field) or "") != str(candidate.get(field) or ""):
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_JOIN_FIELD_MISMATCH",
                f"candidate/outcome mismatch for {field}",
                "Do not join oracle outcomes to different patch/source rows.",
                {
                    "candidate_row_id": candidate_id,
                    "field": field,
                    "outcome": outcome.get(field),
                    "candidate": candidate.get(field),
                },
            )
    for field in ("source_path", "patch_path", "report_path", "test_output_path"):
        value = outcome.get(field) or candidate.get(field)
        if value and has_mounted_drive_fragment(Path(str(value))):
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_MOUNTED_ROW_FIELD_REJECTED",
                f"{field} contains mounted-drive lineage",
                "Regenerate candidate/outcome rows against aiwonder roots.",
                {"candidate_row_id": candidate_id, "field": field, "value": str(value)},
            )


def mutate_source_text(base_text: str, candidate: dict[str, Any]) -> str:
    selected = candidate.get("selected_statement")
    if not isinstance(selected, dict):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_SELECTED_STATEMENT_MISSING",
            "candidate row has no selected_statement object",
            "Regenerate candidate patch rows with AST statement span evidence.",
            {"candidate_row_id": candidate.get("candidate_row_id")},
        )
    start_line = require_int(selected, "start_line")
    end_line = require_int(selected, "end_line")
    if start_line <= 0 or end_line < start_line:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_BAD_SELECTED_STATEMENT_SPAN",
            "selected_statement line span is invalid",
            "Regenerate candidate patch rows with valid AST statement span evidence.",
            {"candidate_row_id": candidate.get("candidate_row_id"), "selected_statement": selected},
        )
    lines = base_text.splitlines(keepends=True)
    if end_line > len(lines):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_SELECTED_STATEMENT_OUT_OF_BOUNDS",
            "selected_statement line span exceeds source length",
            "Regenerate candidate patch rows from the canonical source bytes.",
            {"candidate_row_id": candidate.get("candidate_row_id"), "end_line": end_line, "line_count": len(lines)},
        )
    strategy = require_string(candidate, "mutation_strategy")
    if strategy == "delete_statement":
        return "".join(lines[: start_line - 1] + lines[end_line:])
    if strategy == "replace_statement_with_pass":
        replacement = leading_whitespace(lines[start_line - 1]) + "pass" + line_ending(lines[start_line - 1])
        return "".join(lines[: start_line - 1] + [replacement] + lines[end_line:])
    raise CandidateCorpusError(
        "DTC_CANDIDATE_CORPUS_UNKNOWN_MUTATION_STRATEGY",
        f"unknown mutation strategy: {strategy}",
        "Add explicit materialization support for new candidate mutation strategies.",
        {"candidate_row_id": candidate.get("candidate_row_id"), "mutation_strategy": strategy},
    )


def validate_report_artifacts(outcome: dict[str, Any]) -> None:
    candidate_id = require_string(outcome, "candidate_row_id")
    report_path = Path(require_string(outcome, "report_path"))
    if has_mounted_drive_fragment(report_path):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_MOUNTED_REPORT_PATH_REJECTED",
            f"report_path points at mounted-drive lineage: {report_path}",
            "Use aiwonder SWE-bench run report artifacts only.",
            {"candidate_row_id": candidate_id, "report_path": str(report_path)},
        )
    if not report_path.is_file():
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_REPORT_FILE_MISSING",
            f"oracle report missing for candidate {candidate_id}: {report_path}",
            "Do not emit canonical candidate rows without physical oracle report evidence.",
            {"candidate_row_id": candidate_id, "report_path": str(report_path)},
        )
    expected_report_sha = require_string(outcome, "report_sha256")
    actual_report_sha = sha256_file(report_path)
    if actual_report_sha != expected_report_sha:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_REPORT_SHA_MISMATCH",
            f"oracle report SHA mismatch for candidate {candidate_id}",
            "Do not emit canonical candidate rows without report byte readback.",
            {"candidate_row_id": candidate_id, "expected": expected_report_sha, "actual": actual_report_sha},
        )
    test_output_path = outcome.get("test_output_path")
    test_output_sha = outcome.get("test_output_sha256")
    if test_output_path:
        path = Path(str(test_output_path))
        if has_mounted_drive_fragment(path):
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_MOUNTED_TEST_OUTPUT_PATH_REJECTED",
                f"test_output_path points at mounted-drive lineage: {path}",
                "Use aiwonder SWE-bench run artifacts only.",
                {"candidate_row_id": candidate_id, "test_output_path": str(path)},
            )
        if not path.is_file():
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_TEST_OUTPUT_FILE_MISSING",
                f"oracle test output missing for candidate {candidate_id}: {path}",
                "Do not emit canonical candidate rows without physical test-output evidence.",
                {"candidate_row_id": candidate_id, "test_output_path": str(path)},
            )
        if isinstance(test_output_sha, str) and test_output_sha and sha256_file(path) != test_output_sha:
            raise CandidateCorpusError(
                "DTC_CANDIDATE_CORPUS_TEST_OUTPUT_SHA_MISMATCH",
                f"oracle test output SHA mismatch for candidate {candidate_id}",
                "Do not emit canonical candidate rows without test-output byte readback.",
                {"candidate_row_id": candidate_id, "expected": test_output_sha, "actual": sha256_file(path)},
            )


def source_file_row(
    *,
    candidate_corpus_row_id: str,
    candidate_id: str,
    source_task_id: str,
    workspace_state: str,
    source_path: str,
    path: Path,
    sha256: str,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "artifact_kind": "delete_test_call_candidate_source_file",
        "candidate_corpus_row_id": candidate_corpus_row_id,
        "candidate_row_id": candidate_id,
        "source_task_id": source_task_id,
        "workspace_state": workspace_state,
        "source_path": source_path,
        "path": str(path),
        "sha256": sha256,
        "size_bytes": path.stat().st_size,
        "mounted_drive_lineage": False,
    }


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateCorpusError(
                    code,
                    f"invalid JSONL at {path}:{line_number}",
                    "Regenerate the malformed artifact before candidate corpus materialization.",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            if not isinstance(row, dict):
                raise CandidateCorpusError(
                    code,
                    f"JSONL row is not an object at {path}:{line_number}",
                    "Regenerate the malformed artifact before candidate corpus materialization.",
                    {"path": str(path), "line_number": line_number},
                )
            rows.append(row)
    return rows


def require_string(row: dict[str, Any], key: str) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_REQUIRED_STRING_MISSING",
            f"required string field missing: {key}",
            "Regenerate upstream #615 artifacts with complete candidate lineage.",
            {"key": key, "row_keys": sorted(row.keys())},
        )
    return value


def require_int(row: dict[str, Any], key: str) -> int:
    value = row.get(key)
    if not isinstance(value, int):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_REQUIRED_INT_MISSING",
            f"required integer field missing: {key}",
            "Regenerate upstream #615 artifacts with complete candidate source spans.",
            {"key": key, "row_keys": sorted(row.keys())},
        )
    return value


def require_relative_python_path(value: str, field: str) -> str:
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or has_mounted_drive_fragment(Path(value)) or not value.endswith(".py"):
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_BAD_RELATIVE_SOURCE_PATH",
            f"{field} must be a safe relative Python path: {value}",
            "Regenerate candidate rows with canonical relative source paths.",
            {"field": field, "value": value},
        )
    return path.as_posix()


def normalize_repeated_ids(values: list[str] | None) -> list[str] | None:
    if not values:
        return None
    ids: list[str] = []
    for value in values:
        ids.extend(part.strip() for part in value.split(",") if part.strip())
    return ids or None


def unique_paths(paths: list[Path]) -> list[Path]:
    seen: set[str] = set()
    out: list[Path] = []
    for path in paths:
        resolved = str(path)
        if resolved not in seen:
            out.append(path)
            seen.add(resolved)
    return out


def duplicate_outcome_key(row: dict[str, Any]) -> tuple[Any, ...]:
    return (
        row.get("source_task_id"),
        row.get("source_path"),
        row.get("patch_sha256"),
        row.get("oracle_status"),
        row.get("oracle_all_passed"),
        row.get("resolved"),
        row.get("report_sha256"),
    )


def safe_component(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in {"-", "_", "."} else "_" for ch in value)


def sha256_prefixed_bytes(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def write_bytes_checked(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_bytes() != payload:
        raise CandidateCorpusError(
            "DTC_CANDIDATE_CORPUS_FILE_READBACK_MISMATCH",
            f"file readback mismatch at {path}",
            "Inspect filesystem integrity or concurrent writers before using candidate corpus output.",
            {"path": str(path)},
        )


def count_jsonl(path: Path) -> int:
    if not path.is_file():
        return 0
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for line in handle if line.strip())


def count_distinct_jsonl(path: Path, field: str) -> int:
    if not path.is_file():
        return 0
    values: set[str] = set()
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            if not line.strip():
                continue
            value = json.loads(line).get(field)
            if isinstance(value, str):
                values.add(value)
    return len(values)


def counter_to_dict(counter: Counter) -> dict[str, int]:
    return {"|".join(map(str, key if isinstance(key, tuple) else (key,))): value for key, value in sorted(counter.items())}
