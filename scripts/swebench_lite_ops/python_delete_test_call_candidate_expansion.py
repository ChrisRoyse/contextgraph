"""Deterministic TASK-PY-G-135E candidate expansion plan."""

from __future__ import annotations

import argparse
import shutil
from collections import Counter
from pathlib import Path
from typing import Any

from .core import sha256_file, utc_now, write_json_checked
from .python_delete_test_call_candidate_expansion_core import (
    ARTIFACT_KIND,
    DEFAULT_BATCH_KEY_PREFIX,
    TASK_ID,
    annotate_selected_rows,
    build_batch_rows,
    count_lines,
    duplicate_task_batch_count,
    has_pre_oracle_pass_hypothesis,
    load_candidate_patches,
    load_existing_candidate_ids,
    mounted_drive_hit_count,
    pass_hypothesis_kind_counts,
    read_json,
    read_next_action,
    select_candidates,
    target_map,
    target_quotas,
    tree_state,
    validate_paths,
    validate_selection,
    write_jsonl_checked,
    write_text_checked,
)
from .python_delete_test_call_regeneration import (
    DEFAULT_OUTPUT_ROOT as DEFAULT_PATCH_ROOT,
    halt_delete_test_call_regeneration,
)


DEFAULT_PATCHES_JSONL = DEFAULT_PATCH_ROOT / "delete_test_call_candidate_patches.jsonl"
DEFAULT_LABEL_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-labels-v1")
DEFAULT_OUTCOME_SEARCH_ROOT = Path("/zfs/archive/contextgraph/state/regeneration-plans")
DEFAULT_NEXT_ACTION_REPORT = Path(
    "/zfs/archive/contextgraph/state/labels/"
    "task-py-g-135d-candidate-next-action-v1/candidate_keyed_next_action_report.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/regeneration-plans/"
    "task-py-g-135e-delete-test-call-candidate-expansion-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/"
    "task-py-g-135e-delete-test-call-candidate-expansion-fsv/manual-readback"
)

def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--batch-key-prefix", default=DEFAULT_BATCH_KEY_PREFIX)
    parser.add_argument("--candidate-patches", type=Path, default=DEFAULT_PATCHES_JSONL)
    parser.add_argument("--existing-label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--existing-outcomes-jsonl", type=Path, action="append", default=None)
    parser.add_argument("--existing-outcome-search-root", type=Path, default=DEFAULT_OUTCOME_SEARCH_ROOT)
    parser.add_argument("--next-action-report", type=Path, default=DEFAULT_NEXT_ACTION_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument(
        "--target",
        action="append",
        default=None,
        help="Override one fixed quota as split:sublabel=count, for example holdout:pytest_raises_deleted=4.",
    )
    parser.add_argument("--clean-output", action="store_true")
    parser.add_argument(
        "--prefer-pass-hypothesis",
        action="store_true",
        help="Prefer candidates with static pre-oracle pass hypotheses while preserving split/sublabel quotas.",
    )
    parser.add_argument(
        "--require-pass-hypothesis",
        action="store_true",
        help="Select only candidates carrying a static pre-oracle pass hypothesis.",
    )


def command_python_delete_test_call_candidate_expansion(args: argparse.Namespace) -> dict[str, Any]:
    halt_delete_test_call_regeneration("python-delete-test-call-candidate-expansion")
    started_at = utc_now()
    validate_paths(args)
    targets = target_quotas(args.target)
    next_action = read_next_action(args.next_action_report)
    candidates = load_candidate_patches(args.candidate_patches)
    existing_ids, existing_sources = load_existing_candidate_ids(args)

    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    selected, shortfalls, available_counts = select_candidates(
        candidates,
        existing_ids,
        targets,
        prefer_pass_hypothesis=args.prefer_pass_hypothesis or args.require_pass_hypothesis,
        require_pass_hypothesis=args.require_pass_hypothesis,
    )
    selected_rows = annotate_selected_rows(selected, task_id=args.task_id)
    batch_rows = build_batch_rows(
        selected_rows,
        batch_key_prefix=args.batch_key_prefix,
        selection_policy=f"{args.task_id} fixed split/sublabel expansion",
    )
    validation_failures = validate_selection(
        selected_rows,
        batch_rows,
        shortfalls,
        require_pass_hypothesis=args.require_pass_hypothesis,
    )

    selected_path = args.output_root / "delete_test_call_candidate_expansion_selected.jsonl"
    batch_plan_path = args.output_root / "delete_test_call_candidate_expansion_oracle_batch_plan.jsonl"
    ids_path = args.output_root / "delete_test_call_candidate_expansion_ids.txt"
    manifest_path = args.output_root / "delete_test_call_candidate_expansion_manifest.json"
    readback_path = args.readback_root / "delete_test_call_candidate_expansion_readback.json"

    write_jsonl_checked(selected_path, selected_rows)
    write_jsonl_checked(batch_plan_path, batch_rows)
    write_text_checked(ids_path, "".join(f"{row['candidate_row_id']}\n" for row in selected_rows))

    counts_by_group = Counter((row["stable_partition"], row["assertion_duty_sublabel"]) for row in selected_rows)
    pass_hypothesis_selected_count = sum(1 for row in selected_rows if has_pre_oracle_pass_hypothesis(row))
    pass_hypothesis_counts = pass_hypothesis_kind_counts(selected_rows)
    manifest = {
        "schema_version": 1,
        "task_id": args.task_id,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "candidate_patches": str(args.candidate_patches),
        "candidate_patches_sha256": sha256_file(args.candidate_patches),
        "existing_label_root": str(args.existing_label_root),
        "next_action_report": str(args.next_action_report),
        "next_action_report_sha256": sha256_file(args.next_action_report),
        "next_action_kind": next_action.get("recommended_action_kind"),
        "existing_candidate_count": len(existing_ids),
        "existing_sources": existing_sources,
        "target_quotas": target_map(targets),
        "available_unoracled_counts": target_map(available_counts),
        "selected_candidate_count": len(selected_rows),
        "oracle_batch_count": len(batch_rows),
        "counts_by_partition_sublabel": target_map(counts_by_group),
        "pre_oracle_pass_hypothesis": {
            "prefer_pass_hypothesis": args.prefer_pass_hypothesis or args.require_pass_hypothesis,
            "require_pass_hypothesis": args.require_pass_hypothesis,
            "selected_with_hypothesis_count": pass_hypothesis_selected_count,
            "hypothesis_kind_counts": pass_hypothesis_counts,
            "target_oracle_labels_used_as_features": False,
        },
        "shortfalls": shortfalls,
        "validation_failures": validation_failures,
        "source_of_truth": {
            "selected_candidates_jsonl": str(selected_path),
            "oracle_batch_plan_jsonl": str(batch_plan_path),
            "candidate_ids_txt": str(ids_path),
            "manifest": str(manifest_path),
            "readback": str(readback_path),
        },
        "sha256": {
            "selected_candidates_jsonl": sha256_file(selected_path),
            "oracle_batch_plan_jsonl": sha256_file(batch_plan_path),
            "candidate_ids_txt": sha256_file(ids_path),
        },
        "policy": {
            "selection_kind": "fixed_split_sublabel_quota_from_source_backed_candidate_pool",
            "selection_order": "stable_source_task_path_offset_candidate_id",
            "pass_hypothesis_ordering": "hypothesis_first_then_confidence_then_stable_order"
            if args.prefer_pass_hypothesis or args.require_pass_hypothesis
            else "disabled",
            "batch_key_prefix": args.batch_key_prefix,
            "holdout_tuning_used": False,
            "target_oracle_labels_used_as_features": False,
            "candidate_row_id_keyed": True,
            "oracle_batch_plan_compatible_with": "python-delete-test-call-oracle-executor --batch-plan",
        },
        "all_passed": bool(selected_rows) and not validation_failures,
    }
    write_json_checked(manifest_path, manifest)

    readback = {
        "schema_version": 1,
        "task_id": args.task_id,
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "passes": read_json(manifest_path, "DTC_EXPANSION_MANIFEST_READBACK_MISSING").get("all_passed") is True,
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)},
        "manifest_path": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "selected_line_count": count_lines(selected_path),
        "batch_plan_line_count": count_lines(batch_plan_path),
        "ids_line_count": count_lines(ids_path),
        "distinct_candidate_row_ids": len({row["candidate_row_id"] for row in selected_rows}),
        "duplicate_task_batch_count": duplicate_task_batch_count(batch_rows),
        "mounted_drive_hits_in_outputs": mounted_drive_hit_count([selected_path, batch_plan_path, ids_path, manifest_path]),
        "selected_sha256": sha256_file(selected_path),
        "batch_plan_sha256": sha256_file(batch_plan_path),
        "pre_oracle_pass_hypothesis": {
            "selected_with_hypothesis_count": pass_hypothesis_selected_count,
            "hypothesis_kind_counts": pass_hypothesis_counts,
            "target_oracle_labels_used_as_features": False,
        },
        "summary": {
            "selected_candidate_count": len(selected_rows),
            "oracle_batch_count": len(batch_rows),
            "counts_by_partition_sublabel": target_map(counts_by_group),
            "selected_with_hypothesis_count": pass_hypothesis_selected_count,
            "hypothesis_kind_counts": pass_hypothesis_counts,
            "shortfalls": shortfalls,
            "validation_failures": validation_failures,
        },
    }
    write_json_checked(readback_path, readback)

    return {
        "ok": manifest["all_passed"],
        "all_passed": manifest["all_passed"],
        "command": "python-delete-test-call-candidate-expansion",
        "checked_at": utc_now(),
        "manifest_path": str(manifest_path),
        "readback_path": str(readback_path),
        "manifest_sha256": sha256_file(manifest_path),
        "readback_sha256": sha256_file(readback_path),
        "selected_candidate_count": len(selected_rows),
        "oracle_batch_count": len(batch_rows),
        "counts_by_partition_sublabel": target_map(counts_by_group),
        "selected_with_hypothesis_count": pass_hypothesis_selected_count,
        "hypothesis_kind_counts": pass_hypothesis_counts,
        "shortfalls": shortfalls,
        "validation_failures": validation_failures,
    }
