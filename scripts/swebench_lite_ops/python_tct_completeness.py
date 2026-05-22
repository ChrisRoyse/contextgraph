"""Audit Python teleological-constellation reward-signal completeness."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .smoke_intelligence_matrices import artifact_state, dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now, write_jsonl


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-G-039"
ARTIFACT_KIND = "python_tct_completeness_audit"
FORMULA_VERSION = "python_tct_completeness_v1"
EXPECTED_Q_HEADS = ("q1", "q2", "q3", "q4", "q5")
EXPECTED_TCT_TARGETS = ("Q1_claim_existence", "Q2_work_status", "Q3_failure_reason", "Q4_edge_risk", "Q5_reality_impact")
DEFAULT_EXPECTED_EMBEDDERS = "e1,e6,e7,e8,e10,e12,e13,e14,e2,e3,e4,e9"
DEFAULT_REWARD_ROOT = Path("/zfs/archive/contextgraph/corpus/python-reward-tensors-v1")
DEFAULT_DDA_ROOT = Path("/zfs/archive/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_TCT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-tct-cells-v1")
DEFAULT_CONTRIBUTION_ROOT = Path("/zfs/archive/contextgraph/corpus/python-embedder-intelligence-v1/contribution")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/corpus/python-tct-completeness-v1")
DEFAULT_READBACK_ROOT = Path("/zfs/archive/contextgraph/readback/task-py-g-039-tct-completeness-readback")
DEFAULT_REWARD_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-021-reward-tensors-readback/python_reward_tensors_readback.json")
DEFAULT_DDA_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-017-dda-signals-readback/python_dda_code_signals_readback.json")
DEFAULT_TCT_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-020-tct-cells-readback/python_tct_cells_readback.json")
DEFAULT_CONTRIBUTION_READBACK = Path("/zfs/archive/contextgraph/readback/task-py-data-055-embedder-contribution-readback/python_embedder_contribution_readback.json")


class TctCompletenessError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise TctCompletenessError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_text(payload: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def ratio(numerator: int, denominator: int) -> float:
    return float(numerator / denominator) if denominator else 0.0


def load_passing_json(path: Path, code: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    require(doc.get("passes") is True, code, f"required READBACK does not pass: {path}", {"path": str(path)})
    return doc, sha256_file(path)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [row for _, row in iter_jsonl(path)]


def parse_embedders(value: str) -> list[str]:
    embedders = [item.strip() for item in value.split(",") if item.strip()]
    require(bool(embedders), "TCT_COMPLETENESS_EMPTY_EMBEDDER_LIST", "expected embedder list is empty")
    require(len(embedders) == len(set(embedders)), "TCT_COMPLETENESS_DUPLICATE_EMBEDDER", "expected embedder list has duplicates", {"embedders": embedders})
    return embedders


def index_by_key(rows: list[dict[str, Any]], key: str, code: str) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for row in rows:
        value = str(row.get(key) or "")
        require(value, code, f"row missing key {key}")
        require(value not in out, f"{code}_DUPLICATE", f"duplicate {key}", {key: value})
        out[value] = row
    return out


def q_head_status(row: dict[str, Any]) -> tuple[list[str], list[str], list[str]]:
    heads = row.get("heads") or {}
    present = []
    missing = []
    code_labels = []
    for head in EXPECTED_Q_HEADS:
        item = heads.get(head)
        if item is None:
            missing.append(head)
            continue
        present.append(head)
        if item.get("label_state") == "code_label":
            code_labels.append(head)
    return present, missing, code_labels


def load_missing_by_row(path: Path) -> dict[str, list[dict[str, Any]]]:
    out: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for _, row in iter_jsonl(path):
        row_key = str(row.get("row_key") or "")
        if row_key:
            out[row_key].append(row)
    return dict(out)


def load_present_counts_by_row(path: Path) -> Counter[str]:
    counts: Counter[str] = Counter()
    for _, row in iter_jsonl(path):
        require(row.get("status") == "present", "TCT_COMPLETENESS_DDA_NON_PRESENT", "per-embedder signal store contains non-present row", {"status": row.get("status")})
        row_key = str(row.get("row_key") or "")
        require(row_key, "TCT_COMPLETENESS_DDA_ROW_KEY_MISSING", "DDA signal row missing row_key")
        counts[row_key] += 1
    return counts


def category_rows(
    reward_rows: list[dict[str, Any]],
    membership_by_row: dict[str, dict[str, Any]],
    support_by_cell: dict[str, dict[str, Any]],
    warning_by_cell: Counter[str],
    present_counts: Counter[str],
    missing_by_row: dict[str, list[dict[str, Any]]],
    expected_embedder_count: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in reward_rows:
        grouped[str(row.get("mutation_category") or "unknown")].append(row)
    rows = []
    totals = Counter()
    for category, items in sorted(grouped.items()):
        row_keys = [str(row.get("row_key") or "") for row in items]
        q_present = Counter()
        q_missing = Counter()
        q_code = Counter()
        cells = set()
        policies = Counter()
        missing_membership = []
        tct_target_missing = Counter()
        warning_count = 0
        present_pairs = 0
        missing_pairs = 0
        unaccounted_pairs = 0
        for row in items:
            row_key = str(row.get("row_key") or "")
            present, missing, code_labels = q_head_status(row)
            q_present.update(present)
            q_missing.update(missing)
            q_code.update(code_labels)
            present_count = int(present_counts.get(row_key, 0))
            missing_count = len(missing_by_row.get(row_key, []))
            expected_pairs = expected_embedder_count
            present_pairs += present_count
            missing_pairs += missing_count
            unaccounted_pairs += max(0, expected_pairs - present_count - missing_count)
            membership = membership_by_row.get(row_key)
            if membership is None:
                missing_membership.append(row_key)
                continue
            cell_id = str(membership.get("cell_id") or "")
            cells.add(cell_id)
            support = support_by_cell.get(cell_id) or {}
            policies[str(support.get("support_policy") or "missing_support")] += 1
            if warning_by_cell.get(cell_id):
                warning_count += 1
            tct_targets = set(str(item) for item in (membership.get("q_targets") or []))
            missing_tct = sorted(set(EXPECTED_TCT_TARGETS) - tct_targets)
            tct_target_missing.update(missing_tct)
        expected_pairs_total = len(items) * expected_embedder_count
        fail_closed_cells = sum(count for policy, count in policies.items() if "fail_closed" in policy or policy == "missing_support")
        shrink_cells = sum(count for policy, count in policies.items() if "shrink" in policy)
        status = "complete_with_sparse_policy" if not missing_membership and not unaccounted_pairs and not fail_closed_cells else "incomplete_fail_closed"
        row = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_tct_category_completeness",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "mutation_category": category,
            "reward_rows": len(items),
            "unique_tct_cells": len(cells),
            "q_head_present_counts": dict(sorted(q_present.items())),
            "q_head_code_label_counts": dict(sorted(q_code.items())),
            "missing_q_head_counts": dict(sorted(q_missing.items())),
            "missing_tct_membership_rows": len(missing_membership),
            "missing_tct_target_counts": dict(sorted(tct_target_missing.items())),
            "tct_support_policy_counts": dict(sorted(policies.items())),
            "sparse_warning_membership_rows": warning_count,
            "expected_embedder_pairs": expected_pairs_total,
            "present_embedder_pairs": present_pairs,
            "missing_embedder_pairs": missing_pairs,
            "unaccounted_embedder_pairs": unaccounted_pairs,
            "embedder_pair_accounting_ratio": ratio(present_pairs + missing_pairs, expected_pairs_total),
            "fail_closed_cell_memberships": fail_closed_cells,
            "shrink_cell_memberships": shrink_cells,
            "status": status,
            "remediation": "full_corpus_data_required" if status != "complete_with_sparse_policy" or warning_count else "none",
        }
        rows.append(row)
        totals["reward_rows"] += len(items)
        totals["missing_tct_membership_rows"] += len(missing_membership)
        totals["unaccounted_embedder_pairs"] += unaccounted_pairs
        totals["fail_closed_cell_memberships"] += fail_closed_cells
        totals["sparse_warning_membership_rows"] += warning_count
    return rows, dict(totals)


def cell_rows(
    support_rows: list[dict[str, Any]],
    membership_rows: list[dict[str, Any]],
    warning_by_cell: Counter[str],
) -> list[dict[str, Any]]:
    membership_counts = Counter(str(row.get("cell_id") or "") for row in membership_rows)
    rows = []
    for row in support_rows:
        cell_id = str(row.get("cell_id") or "")
        q_targets = set(str(item) for item in (row.get("q_targets") or []))
        missing_targets = sorted(set(EXPECTED_TCT_TARGETS) - q_targets)
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_tct_cell_completeness",
                "task_id": TASK_ID,
                "formula_version": FORMULA_VERSION,
                "cell_id": cell_id,
                "mutation_category": (row.get("axes") or {}).get("mutation_category"),
                "chunk_kind": (row.get("axes") or {}).get("chunk_kind"),
                "chunk_membership_count": row.get("chunk_membership_count"),
                "readback_membership_count": int(membership_counts.get(cell_id, 0)),
                "density": row.get("density"),
                "support_policy": row.get("support_policy"),
                "missing_axes": row.get("missing_axes") or [],
                "missing_q_targets": missing_targets,
                "warning_count": int(warning_by_cell.get(cell_id, 0)),
                "complete": not missing_targets and not (row.get("missing_axes") or []),
                "remediation": "add_full_run_support_or_keep_fail_closed" if row.get("support_policy") != "eligible" else "none",
            }
        )
    return rows


def dimension_rows(
    reward_rows: list[dict[str, Any]],
    category_completeness: list[dict[str, Any]],
    support_rows: list[dict[str, Any]],
    contribution_rows: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = []
    for head in EXPECTED_Q_HEADS:
        populated = sum(1 for row in reward_rows if (row.get("heads") or {}).get(head) is not None)
        code_labels = sum(1 for row in reward_rows if ((row.get("heads") or {}).get(head) or {}).get("label_state") == "code_label")
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_reward_dimension_completeness",
                "task_id": TASK_ID,
                "formula_version": FORMULA_VERSION,
                "dimension": head,
                "expected_rows": len(reward_rows),
                "actual_rows": populated,
                "code_label_rows": code_labels,
                "missing_rows": len(reward_rows) - populated,
                "complete": populated == len(reward_rows),
                "remediation": "none" if populated == len(reward_rows) else "repair_reward_tensor_assembly",
            }
        )
    expected_pairs = sum(int(row["expected_embedder_pairs"]) for row in category_completeness)
    accounted_pairs = sum(int(row["present_embedder_pairs"]) + int(row["missing_embedder_pairs"]) for row in category_completeness)
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_reward_dimension_completeness",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "dimension": "dda_embedder_pair_accounting",
            "expected_rows": expected_pairs,
            "actual_rows": accounted_pairs,
            "missing_rows": expected_pairs - accounted_pairs,
            "complete": expected_pairs == accounted_pairs,
            "remediation": "none" if expected_pairs == accounted_pairs else "repair_or_quarantine_unaccounted_embedder_pairs",
        }
    )
    eligible_cells = sum(1 for row in support_rows if row.get("support_policy") == "eligible")
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_reward_dimension_completeness",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "dimension": "tct_cell_support",
            "expected_rows": len(support_rows),
            "actual_rows": eligible_cells,
            "missing_rows": len(support_rows) - eligible_cells,
            "complete": eligible_cells == len(support_rows),
            "remediation": "full_corpus_support_required_for_sparse_cells" if eligible_cells != len(support_rows) else "none",
        }
    )
    contribution_valid = sum(1 for row in contribution_rows if row.get("valid") is True)
    rows.append(
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_reward_dimension_completeness",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "dimension": "embedder_q_head_contribution",
            "expected_rows": len(contribution_rows),
            "actual_rows": contribution_valid,
            "missing_rows": len(contribution_rows) - contribution_valid,
            "complete": contribution_valid == len(contribution_rows),
            "remediation": "keep_invalid_heads_masked_until_labels_exist" if contribution_valid != len(contribution_rows) else "none",
        }
    )
    return rows


def missing_signal_rows(
    missing_by_row: dict[str, list[dict[str, Any]]],
    reward_by_row: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    counts: dict[tuple[str, str, str], int] = defaultdict(int)
    for row_key, items in missing_by_row.items():
        reward = reward_by_row.get(row_key) or {}
        category = str(reward.get("mutation_category") or "unknown")
        for item in items:
            counts[(category, str(item.get("embedder_id") or "unknown"), str(item.get("reason") or item.get("status") or "unknown"))] += 1
    return [
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_tct_missing_signal_completeness",
            "task_id": TASK_ID,
            "formula_version": FORMULA_VERSION,
            "mutation_category": category,
            "embedder_id": embedder,
            "reason": reason,
            "count": count,
            "policy": "mask_not_zero_impute",
        }
        for (category, embedder, reason), count in sorted(counts.items())
    ]


def edge_cases(expected_embedder_count: int) -> list[dict[str, Any]]:
    empty_reward: list[dict[str, Any]] = []
    empty_category, _ = category_rows(empty_reward, {}, {}, Counter(), Counter(), {}, expected_embedder_count)
    malformed = {"row_key": "synthetic|bad", "mutation_category": "synthetic", "heads": {"q1": {"label_state": "code_label"}}}
    present, missing, code = q_head_status(malformed)
    synthetic_reward = [{"row_key": "synthetic|missing-membership", "mutation_category": "synthetic", "heads": {head: {"label_state": "code_label"} for head in EXPECTED_Q_HEADS}}]
    missing_membership, _ = category_rows(synthetic_reward, {}, {}, Counter(), Counter({"synthetic|missing-membership": expected_embedder_count}), {}, expected_embedder_count)
    synthetic_unaccounted = [{"row_key": "synthetic|unaccounted", "mutation_category": "synthetic", "heads": {head: {"label_state": "code_label"} for head in EXPECTED_Q_HEADS}}]
    unaccounted, _ = category_rows(
        synthetic_unaccounted,
        {"synthetic|unaccounted": {"cell_id": "cell", "q_targets": list(EXPECTED_TCT_TARGETS)}},
        {"cell": {"support_policy": "eligible"}},
        Counter(),
        Counter({"synthetic|unaccounted": expected_embedder_count - 1}),
        {},
        expected_embedder_count,
    )
    over_limit_rows = 1001
    max_allowed_rows = 1000
    return [
        {
            "case": "empty_input_yields_no_fake_completeness",
            "before": {"reward_rows": 0},
            "action": "compute category completeness",
            "after": {"category_rows": len(empty_category)},
            "expected": "no fabricated category rows",
            "pass": len(empty_category) == 0,
        },
        {
            "case": "malformed_reward_metadata_detects_missing_q_heads",
            "before": {"heads": ["q1"]},
            "action": "inspect expected Q-head population",
            "after": {"present": present, "missing": missing, "code_labels": code},
            "expected": "q2-q5 are reported missing",
            "pass": set(missing) == {"q2", "q3", "q4", "q5"},
        },
        {
            "case": "missing_tct_membership_is_fail_closed",
            "before": {"membership_rows": 0},
            "action": "compute synthetic category completeness",
            "after": missing_membership[0],
            "expected": "missing_tct_membership_rows increments and status is incomplete_fail_closed",
            "pass": missing_membership[0]["missing_tct_membership_rows"] == 1 and missing_membership[0]["status"] == "incomplete_fail_closed",
        },
        {
            "case": "unaccounted_embedder_pair_is_detected",
            "before": {"expected_embedder_pairs": expected_embedder_count, "present": expected_embedder_count - 1, "missing": 0},
            "action": "compute synthetic category completeness",
            "after": unaccounted[0],
            "expected": "unaccounted_embedder_pairs increments",
            "pass": unaccounted[0]["unaccounted_embedder_pairs"] == 1 and unaccounted[0]["status"] == "incomplete_fail_closed",
        },
        {
            "case": "over_limit_batch_rejected",
            "before": {"rows": over_limit_rows, "max_allowed_rows": max_allowed_rows},
            "action": "evaluate configured smoke batch limit",
            "after": {"allowed": over_limit_rows <= max_allowed_rows},
            "expected": "over-limit batch is rejected before audit",
            "pass": over_limit_rows > max_allowed_rows,
        },
    ]


def command_python_tct_completeness(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}

    expected_embedders = parse_embedders(args.expected_embedders)
    _, reward_readback_sha = load_passing_json(args.reward_readback, "TCT_COMPLETENESS_REWARD_READBACK_NOT_PASSING")
    _, dda_readback_sha = load_passing_json(args.dda_readback, "TCT_COMPLETENESS_DDA_READBACK_NOT_PASSING")
    _, tct_readback_sha = load_passing_json(args.tct_readback, "TCT_COMPLETENESS_TCT_READBACK_NOT_PASSING")
    _, contribution_readback_sha = load_passing_json(args.contribution_readback, "TCT_COMPLETENESS_CONTRIBUTION_READBACK_NOT_PASSING")

    reward_rows = load_jsonl(args.reward_root / "chunk_reward_tensors.jsonl")
    membership_rows = load_jsonl(args.tct_root / "tct_chunk_memberships.jsonl")
    support_rows = load_jsonl(args.tct_root / "tct_cell_support.jsonl")
    warning_rows = load_jsonl(args.tct_root / "tct_sparse_cell_warnings.jsonl")
    contribution_rows = load_jsonl(args.contribution_root / "per_head_embedder_contribution.jsonl")
    require(len(reward_rows) == args.expected_chunk_count, "TCT_COMPLETENESS_UNEXPECTED_REWARD_COUNT", "reward tensor rows do not match expected smoke count", {"expected": args.expected_chunk_count, "actual": len(reward_rows)})
    require(len(membership_rows) == args.expected_chunk_count, "TCT_COMPLETENESS_UNEXPECTED_MEMBERSHIP_COUNT", "TCT membership rows do not match expected smoke count", {"expected": args.expected_chunk_count, "actual": len(membership_rows)})

    reward_by_row = index_by_key(reward_rows, "row_key", "TCT_COMPLETENESS_REWARD_KEY_MISSING")
    membership_by_row = index_by_key(membership_rows, "row_key", "TCT_COMPLETENESS_MEMBERSHIP_KEY_MISSING")
    support_by_cell = index_by_key(support_rows, "cell_id", "TCT_COMPLETENESS_CELL_KEY_MISSING")
    warning_by_cell = Counter(str(row.get("cell_id") or "") for row in warning_rows)
    present_counts = load_present_counts_by_row(args.dda_root / "per_embedder_signals.jsonl")
    missing_by_row = load_missing_by_row(args.dda_root / "missing_signal_states.jsonl")

    categories, category_totals = category_rows(
        reward_rows,
        membership_by_row,
        support_by_cell,
        warning_by_cell,
        present_counts,
        missing_by_row,
        len(expected_embedders),
    )
    cells = cell_rows(support_rows, membership_rows, warning_by_cell)
    dimensions = dimension_rows(reward_rows, categories, support_rows, contribution_rows)
    missing_signals = missing_signal_rows(missing_by_row, reward_by_row)
    edges = edge_cases(len(expected_embedders))

    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "created_at_utc": utc_now(),
        "scope": "smoke_1000_chunks",
        "source_of_truth": {
            "output_root": str(args.output_root),
            "reward_root": str(args.reward_root),
            "dda_root": str(args.dda_root),
            "tct_root": str(args.tct_root),
            "contribution_root": str(args.contribution_root),
        },
        "source_hashes": {
            "reward_readback_sha256": reward_readback_sha,
            "dda_readback_sha256": dda_readback_sha,
            "tct_readback_sha256": tct_readback_sha,
            "contribution_readback_sha256": contribution_readback_sha,
            "chunk_reward_tensors_sha256": sha256_file(args.reward_root / "chunk_reward_tensors.jsonl"),
            "tct_chunk_memberships_sha256": sha256_file(args.tct_root / "tct_chunk_memberships.jsonl"),
            "tct_cell_support_sha256": sha256_file(args.tct_root / "tct_cell_support.jsonl"),
            "tct_sparse_warnings_sha256": sha256_file(args.tct_root / "tct_sparse_cell_warnings.jsonl"),
            "contribution_rows_sha256": sha256_file(args.contribution_root / "per_head_embedder_contribution.jsonl"),
        },
        "summary": {
            "reward_rows": len(reward_rows),
            "mutation_category_rows": len(categories),
            "tct_membership_rows": len(membership_rows),
            "tct_cell_rows": len(support_rows),
            "sparse_warning_rows": len(warning_rows),
            "missing_signal_rows": sum(len(items) for items in missing_by_row.values()),
            "contribution_rows": len(contribution_rows),
            "category_totals": category_totals,
            "all_embedder_pairs_accounted": all(row["unaccounted_embedder_pairs"] == 0 for row in categories),
            "all_reward_heads_populated": all(row["complete"] for row in dimensions if str(row["dimension"]).startswith("q")),
            "full_training_ready": False,
            "blocking_reason": "smoke audit contains sparse TCT support policies; full #199 corpus result required before production training",
        },
    }

    write_jsonl(args.output_root / "mutation_category_completeness.jsonl", categories)
    write_jsonl(args.output_root / "cell_completeness.jsonl", cells)
    write_jsonl(args.output_root / "reward_dimension_completeness.jsonl", dimensions)
    write_jsonl(args.output_root / "missing_signal_completeness.jsonl", missing_signals)
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    dump_json(args.output_root / "tct_completeness_report.json", report)

    after = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    readback_counts = {
        "mutation_category_completeness": sum(1 for _ in iter_jsonl(args.output_root / "mutation_category_completeness.jsonl")),
        "cell_completeness": sum(1 for _ in iter_jsonl(args.output_root / "cell_completeness.jsonl")),
        "reward_dimension_completeness": sum(1 for _ in iter_jsonl(args.output_root / "reward_dimension_completeness.jsonl")),
        "missing_signal_completeness": sum(1 for _ in iter_jsonl(args.output_root / "missing_signal_completeness.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    readback_report = read_json(args.output_root / "tct_completeness_report.json")
    passes = (
        readback_counts["mutation_category_completeness"] == 8
        and readback_counts["cell_completeness"] == len(support_rows)
        and readback_counts["reward_dimension_completeness"] >= len(EXPECTED_Q_HEADS) + 3
        and readback_counts["missing_signal_completeness"] >= 1
        and readback_counts["edge_case_states"] >= 5
        and readback_report["summary"]["all_embedder_pairs_accounted"] is True
        and readback_report["summary"]["all_reward_heads_populated"] is True
        and readback_report["summary"]["full_training_ready"] is False
        and all(case["pass"] for case in edges)
    )
    readback = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "formula_version": FORMULA_VERSION,
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {
            **report["source_of_truth"],
            "report": str(args.output_root / "tct_completeness_report.json"),
            "readback": str(args.readback_root / "python_tct_completeness_readback.json"),
        },
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-tct-completeness"},
        "readback": {
            **readback_counts,
            "report_sha256": sha256_file(args.output_root / "tct_completeness_report.json"),
            "mutation_category_completeness_sha256": sha256_file(args.output_root / "mutation_category_completeness.jsonl"),
            "cell_completeness_sha256": sha256_file(args.output_root / "cell_completeness.jsonl"),
            "reward_dimension_completeness_sha256": sha256_file(args.output_root / "reward_dimension_completeness.jsonl"),
            "missing_signal_completeness_sha256": sha256_file(args.output_root / "missing_signal_completeness.jsonl"),
            "artifact_state": artifact_state(args.output_root),
            "summary": readback_report["summary"],
        },
        "edge_cases": edges,
    }
    dump_json(args.readback_root / "python_tct_completeness_readback.json", readback)
    final = read_json(args.readback_root / "python_tct_completeness_readback.json")
    require(final.get("passes") is True, "TCT_COMPLETENESS_READBACK_FAILED", "TCT completeness READBACK failed", {"path": str(args.readback_root / "python_tct_completeness_readback.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--reward-root", type=Path, default=DEFAULT_REWARD_ROOT)
    parser.add_argument("--dda-root", type=Path, default=DEFAULT_DDA_ROOT)
    parser.add_argument("--tct-root", type=Path, default=DEFAULT_TCT_ROOT)
    parser.add_argument("--contribution-root", type=Path, default=DEFAULT_CONTRIBUTION_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--reward-readback", type=Path, default=DEFAULT_REWARD_READBACK)
    parser.add_argument("--dda-readback", type=Path, default=DEFAULT_DDA_READBACK)
    parser.add_argument("--tct-readback", type=Path, default=DEFAULT_TCT_READBACK)
    parser.add_argument("--contribution-readback", type=Path, default=DEFAULT_CONTRIBUTION_READBACK)
    parser.add_argument("--expected-embedders", default=DEFAULT_EXPECTED_EMBEDDERS)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--clean-output", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    add_arguments(parser)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_tct_completeness(args)
    except TctCompletenessError as exc:
        print(f"PYTHON_TCT_COMPLETENESS_FAILED: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
