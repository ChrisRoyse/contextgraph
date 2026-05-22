"""Fail-closed Python ME-JEPA reward-data readiness gate."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any

from .core import SwebenchOpsError
from .smoke_intelligence_matrices import artifact_state, dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now, write_jsonl


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-023"
ARTIFACT_KIND = "python_mejepa_data_readiness_gate"
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-data-readiness-gate-v1")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-data-023-data-readiness-gate-readback")
EXPECTED_MUTATION_CATEGORIES = {
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
}


class DataReadinessGateError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Read the readiness READBACK and fix the named prerequisite before launching ME-JEPA training.",
            details,
        )


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise DataReadinessGateError(code, message, details)


def count_jsonl(path: Path) -> int:
    return sum(1 for _ in iter_jsonl(path))


def readback_passes(doc: dict[str, Any]) -> bool:
    return doc.get("passes") is True or doc.get("all_passed") is True or doc.get("pass") is True


def load_readback(path: Path, stage: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    return doc, sha256_file(path)


def make_stage(name: str, passes: bool, evidence: dict[str, Any], blockers: list[str]) -> dict[str, Any]:
    return {
        "name": name,
        "passes": bool(passes),
        "evidence": evidence,
        "blocking_reasons": [] if passes else blockers,
    }


def category_set(mapping: dict[str, Any]) -> set[str]:
    return {str(k) for k in mapping.keys()}


def compare_hash(label: str, expected: str | None, actual: str | None) -> dict[str, Any]:
    return {
        "label": label,
        "expected": expected,
        "actual": actual,
        "passes": bool(expected and actual and expected == actual),
    }


def decision_from_stages(
    *,
    stages: list[dict[str, Any]],
    production_full_run_complete: bool,
    production_blockers: list[str],
    training_scope: str,
) -> dict[str, Any]:
    stage_blockers = [
        f"{stage['name']}: {reason}"
        for stage in stages
        if stage.get("passes") is not True
        for reason in stage.get("blocking_reasons", [])
    ]
    smoke_ready = not stage_blockers
    production_ready = smoke_ready and production_full_run_complete and not production_blockers
    if training_scope == "smoke-dry-run":
        permits = smoke_ready
        blockers = stage_blockers
    elif training_scope == "full-production":
        permits = production_ready
        blockers = stage_blockers + production_blockers
    else:
        raise DataReadinessGateError("PYTHON_DATA_READINESS_GATE_UNKNOWN_SCOPE", f"unknown training scope: {training_scope}")
    return {
        "training_scope": training_scope,
        "smoke_dry_run_may_proceed": smoke_ready,
        "production_training_may_proceed": production_ready,
        "permits_requested_training": permits,
        "blocking_reasons": blockers,
        "stage_blocking_reasons": stage_blockers,
        "production_blocking_reasons": production_blockers,
        "stage_failure_count": len(stage_blockers),
        "production_full_run_complete": production_full_run_complete,
    }


def edge_case_rows() -> list[dict[str, Any]]:
    base_stage = make_stage("base_complete_stage", True, {"synthetic": True}, [])
    missing_dda = [
        base_stage,
        make_stage("dda_signals", False, {"table": "per_embedder_signals.jsonl", "exists": False}, ["missing DDA signal table"]),
    ]
    stale_manifest = [
        base_stage,
        make_stage(
            "reward_tensors",
            False,
            {"hash_check": compare_hash("smoke_manifest_sha256", "sha256:expected", "sha256:stale")},
            ["reward tensors were built against a stale smoke manifest hash"],
        ),
    ]
    incomplete_oracle = [
        base_stage,
        make_stage(
            "oracle_outcomes",
            False,
            {"chunk_tensor_rows": 1000, "q2_label_rows": 990},
            ["not every reward tensor has a Q2 oracle outcome label"],
        ),
    ]
    cases = [
        ("missing_dda_table", missing_dda, "missing DDA signal table"),
        ("stale_manifest_hash", stale_manifest, "stale smoke manifest hash"),
        ("incomplete_oracle_outcomes", incomplete_oracle, "Q2 oracle outcome label"),
    ]
    rows = []
    for case, stages, expected_substring in cases:
        decision = decision_from_stages(
            stages=stages,
            production_full_run_complete=True,
            production_blockers=[],
            training_scope="smoke-dry-run",
        )
        joined = " | ".join(decision["blocking_reasons"])
        rows.append(
            {
                "case": case,
                "before": {"stage_count": len(stages), "request": "smoke-dry-run"},
                "action": "evaluate data-readiness decision",
                "after": decision,
                "expected": f"requested training denied with blocker containing `{expected_substring}`",
                "pass": decision["permits_requested_training"] is False and expected_substring in joined,
            }
        )
    return rows


def build_current_decision(args: argparse.Namespace) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any]]:
    cleaned_readback, cleaned_readback_sha = load_readback(args.cleaned_manifest_readback, "cleaned_manifest")
    workspace_readback, workspace_readback_sha = load_readback(args.workspace_readback, "workspaces")
    oracle_events_readback, oracle_events_readback_sha = load_readback(args.oracle_events_readback, "oracle_events")
    full_ast_readback, full_ast_readback_sha = load_readback(args.full_ast_readback, "full_ast_corpus")
    smoke_manifest_readback, smoke_manifest_readback_sha = load_readback(args.smoke_manifest_readback, "smoke_manifest")
    forward_readback, forward_readback_sha = load_readback(args.forward_readback, "embeddings")
    dda_readback, dda_readback_sha = load_readback(args.dda_readback, "dda_signals")
    pairwise_readback, pairwise_readback_sha = load_readback(args.pairwise_readback, "pairwise_dda")
    tct_readback, tct_readback_sha = load_readback(args.tct_readback, "tct_cells")
    reward_readback, reward_readback_sha = load_readback(args.reward_readback, "reward_tensors")
    analysis_readback, analysis_readback_sha = load_readback(args.analysis_readback, "quality_report")

    cleaned_manifest = read_json(args.cleaned_manifest)
    workspace_manifest = read_json(args.workspace_manifest)
    reward_report = read_json(args.reward_root / "reward_tensor_report.json")
    analysis_report = read_json(args.analysis_root / "reward_field_analysis_report.json")
    tct_report = read_json(args.tct_root / "tct_cell_report.json")
    forward_summary = forward_readback.get("aggregate_summary") or (forward_readback.get("checkpoint") or {}).get("summary") or {}
    full_ast_summary = full_ast_readback.get("summaries") or {}
    full_checkpoint = read_json(args.full_run_checkpoint) if args.full_run_checkpoint.exists() else {}

    cleaned_categories = category_set(cleaned_manifest.get("category_counts") or {})
    cleaned_stage_passes = (
        readback_passes(cleaned_readback)
        and int(cleaned_manifest.get("row_count") or 0) == args.expected_manifest_rows
        and cleaned_categories == EXPECTED_MUTATION_CATEGORIES
        and int(cleaned_manifest.get("quarantine_count") or 0) == 0
    )
    workspace_stage_passes = readback_passes(workspace_readback) and int(workspace_manifest.get("workspace_count") or 0) > 0
    chunk_total = int(((full_ast_summary.get("chunks") or {}).get("total")) or 0)
    coverage = full_ast_summary.get("coverage") or {}
    ast_stage_passes = (
        readback_passes(full_ast_readback)
        and chunk_total >= args.expected_min_ast_chunks
        and int(coverage.get("task_count") or 0) == args.expected_task_count
        and category_set({c: True for c in (coverage.get("categories") or [])}) == EXPECTED_MUTATION_CATEGORIES
    )
    smoke_rows = count_jsonl(args.smoke_manifest)
    smoke_manifest_sha = sha256_file(args.smoke_manifest)
    smoke_stage_passes = readback_passes(smoke_manifest_readback) and smoke_rows == args.expected_smoke_chunks
    forward_expected_pairs = int(forward_summary.get("expected_pairs") or 0)
    forward_accounted_pairs = int(forward_summary.get("accounted_pairs") or 0)
    forward_stage_passes = (
        readback_passes(forward_readback)
        and int(forward_summary.get("chunk_count") or 0) == args.expected_smoke_chunks
        and int(forward_summary.get("active_embedder_count") or 0) == args.expected_embedder_count
        and forward_accounted_pairs == args.expected_smoke_chunks * args.expected_embedder_count
        and forward_accounted_pairs == forward_expected_pairs
    )
    dda_present_rows = count_jsonl(args.dda_root / "per_embedder_signals.jsonl")
    dda_missing_rows = count_jsonl(args.dda_root / "missing_signal_states.jsonl")
    dda_stage_passes = (
        readback_passes(dda_readback)
        and (dda_present_rows + dda_missing_rows) == args.expected_smoke_chunks * args.expected_embedder_count
        and int((analysis_report.get("summary") or {}).get("dda_present_rows") or 0) == dda_present_rows
        and int((analysis_report.get("summary") or {}).get("dda_missing_rows") or 0) == dda_missing_rows
    )
    pairwise_stage_passes = readback_passes(pairwise_readback) and count_jsonl(args.pairwise_root / "pairwise_interactions.jsonl") >= 1
    tct_summary = tct_report.get("summary") or {}
    tct_stage_passes = (
        readback_passes(tct_readback)
        and int(tct_summary.get("chunk_membership_count") or 0) == args.expected_smoke_chunks
        and int(tct_summary.get("mutation_category_support_rows") or 0) == len(EXPECTED_MUTATION_CATEGORIES)
        and count_jsonl(args.tct_root / "tct_cell_support.jsonl") > 0
    )
    reward_summary = reward_report.get("summary") or {}
    head_population = reward_summary.get("head_population") or {}
    reward_hash_checks = [
        compare_hash("smoke_manifest_sha256", (reward_report.get("source_hashes") or {}).get("smoke_manifest_sha256"), smoke_manifest_sha),
        compare_hash("dda_readback_sha256", (reward_report.get("source_hashes") or {}).get("dda_readback_sha256"), dda_readback_sha),
        compare_hash("tct_readback_sha256", (reward_report.get("source_hashes") or {}).get("tct_readback_sha256"), tct_readback_sha),
    ]
    reward_stage_passes = (
        readback_passes(reward_readback)
        and int(reward_summary.get("chunk_tensor_count") or 0) == args.expected_smoke_chunks
        and int(reward_summary.get("patch_tensor_count") or 0) > 0
        and all(int(head_population.get(head) or 0) == args.expected_smoke_chunks for head in ("q1", "q2", "q3", "q4", "q5"))
        and category_set(reward_summary.get("category_counts") or {}) == EXPECTED_MUTATION_CATEGORIES
        and all(check["passes"] for check in reward_hash_checks)
    )
    q2_label_rows = int(reward_summary.get("q2_pass_count") or 0) + int(reward_summary.get("q2_fail_count") or 0)
    oracle_stage_passes = readback_passes(oracle_events_readback) and q2_label_rows == int(reward_summary.get("chunk_tensor_count") or 0)
    quality_hash_checks = [
        compare_hash("reward_readback_sha256", (analysis_report.get("source_hashes") or {}).get("reward_readback_sha256"), reward_readback_sha),
        compare_hash("dda_readback_sha256", (analysis_report.get("source_hashes") or {}).get("dda_readback_sha256"), dda_readback_sha),
        compare_hash("tct_readback_sha256", (analysis_report.get("source_hashes") or {}).get("tct_readback_sha256"), tct_readback_sha),
    ]
    analysis_training = analysis_report.get("training_readiness") or {}
    quality_stage_passes = (
        readback_passes(analysis_readback)
        and bool(analysis_training.get("smoke_analysis_usable_now")) is True
        and int((analysis_report.get("summary") or {}).get("chunk_tensor_rows") or 0) == args.expected_smoke_chunks
        and all(check["passes"] for check in quality_hash_checks)
    )

    full_progress = full_checkpoint.get("progress") or {}
    full_summary = full_checkpoint.get("summary") or {}
    completed_shards = int(full_progress.get("completed_shards") or 0)
    total_shards = int(full_progress.get("total_shards") or 0)
    production_full_run_complete = bool(
        full_checkpoint.get("status") == "complete"
        and total_shards > 0
        and completed_shards == total_shards
        and int(full_summary.get("active_embedder_count") or 0) == args.expected_embedder_count
    )
    production_blockers = []
    if not production_full_run_complete:
        production_blockers.append(
            f"#199 full corpus embedder explosion incomplete: completed_shards={completed_shards}, total_shards={total_shards}, status={full_checkpoint.get('status')}"
        )
    if int(tct_summary.get("sparse_warning_count") or 0) > 0:
        production_blockers.append(
            f"TCT sparse warning rows remain explicit smoke-design blockers: {int(tct_summary.get('sparse_warning_count') or 0)}"
        )

    stages = [
        make_stage("manifest", cleaned_stage_passes, {"readback_sha256": cleaned_readback_sha, "row_count": cleaned_manifest.get("row_count"), "category_count": len(cleaned_categories)}, ["cleaned manifest is not a verified 300x8 Python manifest"]),
        make_stage("workspaces", workspace_stage_passes, {"readback_sha256": workspace_readback_sha, "workspace_count": workspace_manifest.get("workspace_count")}, ["workspace source-of-truth is missing or unverified"]),
        make_stage("ast_chunks", ast_stage_passes, {"readback_sha256": full_ast_readback_sha, "chunk_total": chunk_total, "task_count": coverage.get("task_count"), "categories": coverage.get("categories")}, ["full AST chunk corpus is partial, stale, or under the minimum chunk floor"]),
        make_stage("smoke_manifest", smoke_stage_passes, {"readback_sha256": smoke_manifest_readback_sha, "row_count": smoke_rows, "sha256": smoke_manifest_sha}, ["smoke manifest is missing, stale, or not exactly 1000 chunks"]),
        make_stage("embeddings", forward_stage_passes, {"readback_sha256": forward_readback_sha, "summary": forward_summary}, ["embedding forward cache does not account for all smoke chunk x embedder pairs"]),
        make_stage("dda_signals", dda_stage_passes, {"readback_sha256": dda_readback_sha, "present_rows": dda_present_rows, "missing_rows": dda_missing_rows}, ["DDA signal table is missing, stale, or not aligned to forward-cache expected pairs"]),
        make_stage("pairwise_dda", pairwise_stage_passes, {"readback_sha256": pairwise_readback_sha, "pairwise_rows": count_jsonl(args.pairwise_root / "pairwise_interactions.jsonl")}, ["pairwise DDA interaction rows are missing or unverified"]),
        make_stage("tct_cells", tct_stage_passes, {"readback_sha256": tct_readback_sha, "summary": tct_summary}, ["TCT cell memberships/support are missing or do not cover the smoke reward field"]),
        make_stage("oracle_outcomes", oracle_stage_passes, {"readback_sha256": oracle_events_readback_sha, "q2_label_rows": q2_label_rows, "chunk_tensor_rows": reward_summary.get("chunk_tensor_count")}, ["not every reward tensor has a Q2 oracle outcome label"]),
        make_stage("reward_tensors", reward_stage_passes, {"readback_sha256": reward_readback_sha, "summary": reward_summary, "hash_checks": reward_hash_checks}, ["reward tensors are incomplete or stale against manifest/DDA/TCT sources"]),
        make_stage("quality_report", quality_stage_passes, {"readback_sha256": analysis_readback_sha, "summary": analysis_report.get("summary"), "hash_checks": quality_hash_checks}, ["reward-field quality report is missing or stale against tensor/source READBACKs"]),
    ]
    decision = decision_from_stages(
        stages=stages,
        production_full_run_complete=production_full_run_complete,
        production_blockers=production_blockers,
        training_scope=args.training_scope,
    )
    dependencies = {
        "readback_hashes": {
            "cleaned_manifest_readback": cleaned_readback_sha,
            "workspace_readback": workspace_readback_sha,
            "oracle_events_readback": oracle_events_readback_sha,
            "full_ast_readback": full_ast_readback_sha,
            "smoke_manifest_readback": smoke_manifest_readback_sha,
            "forward_readback": forward_readback_sha,
            "dda_readback": dda_readback_sha,
            "pairwise_readback": pairwise_readback_sha,
            "tct_readback": tct_readback_sha,
            "reward_readback": reward_readback_sha,
            "analysis_readback": analysis_readback_sha,
        },
        "artifact_hashes": {
            "cleaned_manifest": sha256_file(args.cleaned_manifest),
            "workspace_manifest": sha256_file(args.workspace_manifest),
            "smoke_manifest": smoke_manifest_sha,
            "reward_report": sha256_file(args.reward_root / "reward_tensor_report.json"),
            "analysis_report": sha256_file(args.analysis_root / "reward_field_analysis_report.json"),
            "tct_report": sha256_file(args.tct_root / "tct_cell_report.json"),
        },
        "full_run_checkpoint": {
            "path": str(args.full_run_checkpoint),
            "exists": args.full_run_checkpoint.exists(),
            "sha256": sha256_file(args.full_run_checkpoint) if args.full_run_checkpoint.exists() else None,
            "progress": full_progress,
            "summary": full_summary,
            "status": full_checkpoint.get("status"),
        },
    }
    return decision, stages, dependencies


def command_python_data_readiness_gate(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output:
        for root in (args.output_root, args.readback_root):
            if root.exists():
                shutil.rmtree(root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}

    decision, stages, dependencies = build_current_decision(args)
    preflight = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": "python_mejepa_training_preflight_log",
        "created_at_utc": utc_now(),
        "training_scope": args.training_scope,
        "would_launch_training": decision["permits_requested_training"],
        "launch_mode": "dry_run_only",
        "exit_code_if_enforced": 0 if decision["permits_requested_training"] else 2,
        "blocking_reasons": decision["blocking_reasons"],
        "source_of_truth": {
            "readiness_decision": str(args.output_root / "data_readiness_decision.json"),
            "preflight_log": str(args.output_root / "training_preflight_log.json"),
        },
    }
    decision_artifact = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "readback_root": str(args.readback_root),
            "data_readiness_decision": str(args.output_root / "data_readiness_decision.json"),
            "training_preflight_log": str(args.output_root / "training_preflight_log.json"),
        },
        "decision": decision,
        "stages": stages,
        "dependencies": dependencies,
        "downstream_gate_consumers": ["#211", "#246", "#247", "#165", "#172"],
    }
    edge_rows = edge_case_rows()
    dump_json(args.output_root / "data_readiness_decision.json", decision_artifact)
    dump_json(args.output_root / "training_preflight_log.json", preflight)
    write_jsonl(args.output_root / "edge_case_decisions.jsonl", edge_rows)

    after = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    readback_decision = read_json(args.output_root / "data_readiness_decision.json")
    readback_preflight = read_json(args.output_root / "training_preflight_log.json")
    readback_edge_count = count_jsonl(args.output_root / "edge_case_decisions.jsonl")
    readback_decision_summary = readback_decision["decision"]
    expected_exit_code = 0 if readback_decision_summary["permits_requested_training"] else 2
    if args.training_scope == "smoke-dry-run":
        happy_case = "complete_smoke_reward_field_permits_dry_run_preflight"
        happy_expected = "smoke dry-run permitted while production training remains blocked until full corpus readiness"
        happy_pass = (
            readback_decision_summary["smoke_dry_run_may_proceed"] is True
            and readback_preflight["would_launch_training"] is True
            and readback_decision_summary["permits_requested_training"] is True
            and readback_decision_summary["production_training_may_proceed"] is False
            and readback_preflight["exit_code_if_enforced"] == expected_exit_code
        )
    else:
        happy_case = "full_production_request_fails_closed_until_corpus_ready"
        happy_expected = "full-production request follows the production readiness verdict and fails closed while blockers remain"
        happy_pass = (
            readback_preflight["would_launch_training"] == readback_decision_summary["production_training_may_proceed"]
            and readback_decision_summary["permits_requested_training"] == readback_decision_summary["production_training_may_proceed"]
            and readback_preflight["exit_code_if_enforced"] == expected_exit_code
            and (
                readback_decision_summary["production_training_may_proceed"] is True
                or len(readback_decision_summary["blocking_reasons"]) > 0
            )
        )
    happy_path = {
        "case": happy_case,
        "before": {"training_scope": args.training_scope, "stage_count": len(stages)},
        "action": "read durable readiness decision and preflight log",
        "after": {
            "smoke_dry_run_may_proceed": readback_decision_summary["smoke_dry_run_may_proceed"],
            "permits_requested_training": readback_preflight["would_launch_training"],
            "production_training_may_proceed": readback_decision_summary["production_training_may_proceed"],
            "production_blockers": readback_decision_summary["production_blocking_reasons"],
            "exit_code_if_enforced": readback_preflight["exit_code_if_enforced"],
        },
        "expected": happy_expected,
        "pass": happy_pass,
    }
    passes = (
        happy_path["pass"]
        and all(stage["passes"] for stage in readback_decision["stages"])
        and readback_edge_count == 3
        and all(row["pass"] for _, row in iter_jsonl(args.output_root / "edge_case_decisions.jsonl"))
        and readback_preflight["exit_code_if_enforced"] == expected_exit_code
    )
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {
            **decision_artifact["source_of_truth"],
            "readback": str(args.readback_root / "python_data_readiness_gate_readback.json"),
        },
        "trigger": {
            "command": "python3 scripts/swebench_lite_ops.py python-data-readiness-gate",
            "training_scope": args.training_scope,
            "enforce": args.enforce,
        },
        "happy_path": happy_path,
        "readback": {
            "decision_sha256": sha256_file(args.output_root / "data_readiness_decision.json"),
            "preflight_sha256": sha256_file(args.output_root / "training_preflight_log.json"),
            "edge_case_decisions_sha256": sha256_file(args.output_root / "edge_case_decisions.jsonl"),
            "edge_case_count": readback_edge_count,
            "decision": readback_decision["decision"],
            "stage_count": len(readback_decision["stages"]),
            "artifact_state": artifact_state(args.output_root),
        },
        "edge_cases": edge_rows,
    }
    dump_json(args.readback_root / "python_data_readiness_gate_readback.json", readback)
    final = read_json(args.readback_root / "python_data_readiness_gate_readback.json")
    require(final.get("passes") is True, "PYTHON_DATA_READINESS_GATE_READBACK_FAILED", "data readiness gate READBACK failed", {"path": str(args.readback_root / "python_data_readiness_gate_readback.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    if args.enforce and not final["readback"]["decision"]["permits_requested_training"]:
        return 2
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--cleaned-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-cleaned-manifest-v1/cleaned_manifest.json"))
    parser.add_argument("--cleaned-manifest-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-004-cleaned-manifest-readback/cleaned_manifest_readback.json"))
    parser.add_argument("--workspace-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-workspaces-v1/workspace_manifest.json"))
    parser.add_argument("--workspace-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-006-workspaces-readback/workspace_readback.json"))
    parser.add_argument("--oracle-events-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-008-oracle-events-readback/oracle_events_readback.json"))
    parser.add_argument("--full-ast-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-058-full-ast-corpus-readback/full_ast_corpus_readback.json"))
    parser.add_argument("--smoke-manifest", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/manifest/smoke_manifest.jsonl"))
    parser.add_argument("--smoke-manifest-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-060-post249-smoke-manifest-readback/smoke_manifest_readback.json"))
    parser.add_argument("--forward-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-061-post249-smoke-forward-readback/python_forward_shard_scheduler_readback.json"))
    parser.add_argument("--dda-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder"))
    parser.add_argument("--dda-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-017-dda-signals-readback/python_dda_code_signals_readback.json"))
    parser.add_argument("--pairwise-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/pairwise"))
    parser.add_argument("--pairwise-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-018-pairwise-dda-mi-readback/python_pairwise_dda_mi_readback.json"))
    parser.add_argument("--tct-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1"))
    parser.add_argument("--tct-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-020-tct-cells-readback/python_tct_cells_readback.json"))
    parser.add_argument("--reward-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1"))
    parser.add_argument("--reward-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-021-reward-tensors-readback/python_reward_tensors_readback.json"))
    parser.add_argument("--analysis-root", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-reward-field-analysis-v1"))
    parser.add_argument("--analysis-readback", type=Path, default=Path("/zfs/archive/contextgraph/readback/task-py-data-022-reward-field-analysis-readback/python_reward_field_analysis_readback.json"))
    parser.add_argument("--full-run-checkpoint", type=Path, default=Path("/zfs/archive/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-c8117f15-20260515/python_forward_cache_checkpoint.json"))
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--training-scope", choices=["smoke-dry-run", "full-production"], default="smoke-dry-run")
    parser.add_argument("--expected-manifest-rows", type=int, default=2400)
    parser.add_argument("--expected-task-count", type=int, default=300)
    parser.add_argument("--expected-smoke-chunks", type=int, default=1000)
    parser.add_argument("--expected-embedder-count", type=int, default=12)
    parser.add_argument("--expected-min-ast-chunks", type=int, default=375000)
    parser.add_argument("--clean-output", action="store_true")
    parser.add_argument("--enforce", action="store_true")


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_data_readiness_gate(args)
    except DataReadinessGateError as exc:
        print(json.dumps({"ok": False, **exc.to_json()}, indent=2, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
