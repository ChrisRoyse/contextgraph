"""Analyze Python reward-field quality and readiness gaps."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

from .smoke_intelligence_matrices import artifact_state, dump_json, iter_jsonl, read_json, sha256_file, tree_state, utc_now, write_jsonl


SCHEMA_VERSION = 1
TASK_ID = "TASK-PY-DATA-022"
ARTIFACT_KIND = "python_reward_field_quality_analysis"
DEFAULT_REWARD_ROOT = Path("/mnt/d/contextgraph/corpus/python-reward-tensors-v1")
DEFAULT_DDA_ROOT = Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/per-embedder")
DEFAULT_PAIRWISE_ROOT = Path("/mnt/d/contextgraph/corpus/python-dda-signals-v1/pairwise")
DEFAULT_TCT_ROOT = Path("/mnt/d/contextgraph/corpus/python-tct-cells-v1")
DEFAULT_MATRIX_ROOT = Path("/mnt/d/contextgraph/corpus/python-embedder-intelligence-smoke-1000-v2/intelligence-matrices")
DEFAULT_OUTPUT_ROOT = Path("/mnt/d/contextgraph/corpus/python-reward-field-analysis-v1")
DEFAULT_FSV_ROOT = Path("/mnt/d/contextgraph/fsv/task-py-data-022-reward-field-analysis-fsv")
DEFAULT_REWARD_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-021-reward-tensors-fsv/python_reward_tensors_fsv.json")
DEFAULT_DDA_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-017-dda-signals-fsv/python_dda_code_signals_fsv.json")
DEFAULT_TCT_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-020-tct-cells-fsv/python_tct_cells_fsv.json")
DEFAULT_MATRIX_FSV = Path("/mnt/d/contextgraph/fsv/task-py-data-062-post249-smoke-intelligence-fsv/smoke_intelligence_matrices_fsv.json")


class RewardFieldAnalysisError(RuntimeError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise RewardFieldAnalysisError(json.dumps({"error_code": code, "message": message, "details": details or {}}, sort_keys=True))


def sha256_text(payload: Any) -> str:
    data = json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return "sha256:" + hashlib.sha256(data).hexdigest()


def load_passing_json(path: Path, code: str) -> tuple[dict[str, Any], str]:
    doc = read_json(path)
    require(doc.get("passes") is True, code, f"required FSV does not pass: {path}", {"path": str(path)})
    return doc, sha256_file(path)


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    return [row for _, row in iter_jsonl(path)]


def ratio(numerator: int, denominator: int) -> float:
    return float(numerator / denominator) if denominator else 0.0


def density_rows(counter: Counter[str], total: int, axis: str) -> list[dict[str, Any]]:
    return [
        {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "python_reward_field_density",
            "task_id": TASK_ID,
            "axis": axis,
            "value": value,
            "count": int(count),
            "density": ratio(int(count), total),
        }
        for value, count in sorted(counter.items())
    ]


def q_head_density(chunk_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for head in ("q1", "q2", "q3", "q4", "q5"):
        populated = sum(1 for row in chunk_rows if row.get("heads", {}).get(head) is not None)
        code_labels = sum(1 for row in chunk_rows if (row.get("heads", {}).get(head) or {}).get("label_state") == "code_label")
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_reward_q_head_density",
                "task_id": TASK_ID,
                "head": head,
                "populated": populated,
                "code_label_count": code_labels,
                "row_count": len(chunk_rows),
                "population_ratio": ratio(populated, len(chunk_rows)),
                "code_label_ratio": ratio(code_labels, len(chunk_rows)),
            }
        )
    return rows


def label_balance(chunk_rows: list[dict[str, Any]]) -> dict[str, Any]:
    q2 = Counter(str((row.get("heads", {}).get("q2") or {}).get("label")) for row in chunk_rows)
    q3 = Counter("labeled" if (row.get("heads", {}).get("q3") or {}).get("failure_reason_targets") else "not_applicable" for row in chunk_rows)
    q1 = Counter(str((row.get("heads", {}).get("q1") or {}).get("label")) for row in chunk_rows)
    imbalance_flags = []
    for head, counts in (("q1", q1), ("q2", q2), ("q3", q3)):
        total = sum(counts.values())
        if total and max(counts.values()) / total >= 0.95:
            imbalance_flags.append({"head": head, "counts": dict(counts), "max_ratio": max(counts.values()) / total})
    return {"q1": dict(q1), "q2": dict(q2), "q3": dict(q3), "severe_imbalance_flags": imbalance_flags}


def embedder_density(signal_rows: list[dict[str, Any]], missing_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    present = Counter(str(row.get("embedder_id") or "unknown") for row in signal_rows)
    missing = Counter(str(row.get("embedder_id") or "unknown") for row in missing_rows)
    embedders = sorted(set(present) | set(missing))
    rows = []
    for embedder in embedders:
        p = int(present.get(embedder, 0))
        m = int(missing.get(embedder, 0))
        rows.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_reward_embedder_signal_density",
                "task_id": TASK_ID,
                "embedder_id": embedder,
                "present_count": p,
                "missing_count": m,
                "expected_count": p + m,
                "present_ratio": ratio(p, p + m),
                "missing_ratio": ratio(m, p + m),
            }
        )
    return rows


def tct_cell_quality(cell_rows: list[dict[str, Any]], warnings: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    warning_by_cell = Counter(str(row.get("cell_id") or "") for row in warnings)
    out = []
    policies = Counter()
    for row in cell_rows:
        policy = str(row.get("support_policy") or "unknown")
        policies[policy] += 1
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": "python_reward_tct_cell_quality",
                "task_id": TASK_ID,
                "cell_id": row.get("cell_id"),
                "mutation_category": (row.get("axes") or {}).get("mutation_category"),
                "chunk_kind": (row.get("axes") or {}).get("chunk_kind"),
                "chunk_membership_count": row.get("chunk_membership_count"),
                "density": row.get("density"),
                "support_policy": policy,
                "warning_count": int(warning_by_cell.get(str(row.get("cell_id") or ""), 0)),
            }
        )
    return out, {"policy_counts": dict(sorted(policies.items())), "warning_count": len(warnings)}


def leakage_and_oracle_risks(chunk_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    total = len(chunk_rows)
    test_surface = sum(1 for row in chunk_rows if "test_surface_change" in (row.get("heads", {}).get("q4", {}).get("concern_targets") or []))
    if ratio(test_surface, total) > 0.25:
        rows.append(
            {
                "finding_id": sha256_text({"finding": "test_surface_density", "count": test_surface}),
                "severity": "medium",
                "finding": "test_surface_density",
                "message": "A large share of smoke chunks are test-surface rows; train/eval splits must avoid overfitting to test-path markers.",
                "evidence": {"test_surface_rows": test_surface, "row_count": total, "ratio": ratio(test_surface, total)},
                "recommended_action": "Keep leakage detector and holdout quarantine gates before any ship-gate claim.",
            }
        )
    q2_by_category: dict[str, Counter[str]] = defaultdict(Counter)
    for row in chunk_rows:
        q2_by_category[str(row.get("mutation_category"))][str(row["heads"]["q2"]["label"])] += 1
    for category, counts in sorted(q2_by_category.items()):
        if len(counts) > 1:
            rows.append(
                {
                    "finding_id": sha256_text({"finding": "mixed_oracle_category", "category": category, "counts": dict(counts)}),
                    "severity": "info",
                    "finding": "mixed_oracle_category",
                    "message": "Category contains both pass and fail oracle labels; predictor should learn category+context, not category alone.",
                    "evidence": {"mutation_category": category, "q2_counts": dict(counts)},
                    "recommended_action": "Use TCT cells and DDA features for stratified training/evaluation.",
                }
            )
    return rows


def edge_cases() -> list[dict[str, Any]]:
    empty = density_rows(Counter(), 0, "empty_signal_family")
    severe_rows = [{"heads": {"q2": {"label": True}, "q3": {"failure_reason_targets": []}, "q1": {"label": True}}} for _ in range(99)]
    severe_rows.append({"heads": {"q2": {"label": False}, "q3": {"failure_reason_targets": ["x"]}, "q1": {"label": True}}})
    no_oracle = [{"heads": {"q2": {"label": None, "label_state": "excluded_infra"}, "q3": {"failure_reason_targets": []}, "q1": {"label": True}}} for _ in range(5)]
    return [
        {
            "case": "empty_signal_family",
            "before": {"rows": 0},
            "action": "compute density rows",
            "after": {"rows": len(empty)},
            "expected": "zero density rows without fake data",
            "pass": len(empty) == 0,
        },
        {
            "case": "severe_label_imbalance",
            "before": {"q2_true": 99, "q2_false": 1},
            "action": "compute label balance",
            "after": label_balance(severe_rows),
            "expected": "severe imbalance flag for q1/q2",
            "pass": any(flag["head"] == "q2" for flag in label_balance(severe_rows)["severe_imbalance_flags"]),
        },
        {
            "case": "corpus_subset_with_no_oracle_outcomes",
            "before": {"rows": len(no_oracle), "q2_code_labels": 0},
            "action": "compute Q-head density",
            "after": q_head_density(no_oracle),
            "expected": "Q2 code_label_count remains zero",
            "pass": next(row for row in q_head_density(no_oracle) if row["head"] == "q2")["code_label_count"] == 0,
        },
    ]


def command_python_reward_field_analysis(args: argparse.Namespace) -> int:
    started = utc_now()
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.fsv_root.mkdir(parents=True, exist_ok=True)
    before = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    _, reward_fsv_sha = load_passing_json(args.reward_fsv, "REWARD_ANALYSIS_REWARD_FSV_NOT_PASSING")
    _, dda_fsv_sha = load_passing_json(args.dda_fsv, "REWARD_ANALYSIS_DDA_FSV_NOT_PASSING")
    _, tct_fsv_sha = load_passing_json(args.tct_fsv, "REWARD_ANALYSIS_TCT_FSV_NOT_PASSING")
    _, matrix_fsv_sha = load_passing_json(args.matrix_fsv, "REWARD_ANALYSIS_MATRIX_FSV_NOT_PASSING")

    chunk_rows = load_jsonl(args.reward_root / "chunk_reward_tensors.jsonl")
    patch_rows = load_jsonl(args.reward_root / "patch_reward_tensors.jsonl")
    signal_rows = load_jsonl(args.dda_root / "per_embedder_signals.jsonl")
    missing_rows = load_jsonl(args.dda_root / "missing_signal_states.jsonl")
    pairwise_rows = load_jsonl(args.pairwise_root / "pairwise_interactions.jsonl")
    cell_rows = load_jsonl(args.tct_root / "tct_cell_support.jsonl")
    sparse_warnings = load_jsonl(args.tct_root / "tct_sparse_cell_warnings.jsonl")
    matrix_summary = read_json(args.matrix_root / "summary.json")
    require(len(chunk_rows) > 0, "REWARD_ANALYSIS_EMPTY_TENSOR_STORE", "chunk tensor store is empty")
    require(len(signal_rows) > 0, "REWARD_ANALYSIS_EMPTY_DDA_STORE", "DDA signal store is empty")

    density_by_task = density_rows(Counter(row["task_instance_id"] for row in chunk_rows), len(chunk_rows), "task_instance_id")
    density_by_category = density_rows(Counter(row["mutation_category"] for row in chunk_rows), len(chunk_rows), "mutation_category")
    density_by_chunk_kind = density_rows(Counter((row.get("axes") or {}).get("chunk_kind") for row in chunk_rows), len(chunk_rows), "chunk_kind")
    density_by_q_head = q_head_density(chunk_rows)
    density_by_embedder = embedder_density(signal_rows, missing_rows)
    tct_quality, tct_summary = tct_cell_quality(cell_rows, sparse_warnings)
    findings = leakage_and_oracle_risks(chunk_rows)

    collapsed_pair_count = sum(1 for row in pairwise_rows if row.get("collapsed_duplicate_candidate") is True)
    if collapsed_pair_count:
        findings.append(
            {
                "finding_id": sha256_text({"finding": "collapsed_embedder_pairs", "count": collapsed_pair_count}),
                "severity": "medium",
                "finding": "collapsed_embedder_pairs",
                "message": "Some embedder pairs are effectively redundant in the smoke window.",
                "evidence": {"collapsed_pair_count": collapsed_pair_count, "pairwise_rows": len(pairwise_rows)},
                "recommended_action": "Keep contribution and adaptive-weighting issues #246/#247 open before production training.",
            }
        )
    missing_total = len(missing_rows)
    if missing_total:
        findings.append(
            {
                "finding_id": sha256_text({"finding": "missing_embedder_outputs", "count": missing_total}),
                "severity": "medium",
                "finding": "missing_embedder_outputs",
                "message": "Missing/quarantined embedder outputs are explicit reward-field states and must remain visible during training.",
                "evidence": {"missing_rows": missing_total, "present_rows": len(signal_rows), "missing_ratio": ratio(missing_total, missing_total + len(signal_rows))},
                "recommended_action": "Train with quarantine masks; do not impute missing vectors as zero.",
            }
        )
    if tct_summary["policy_counts"].get("eligible", 0) < len(cell_rows):
        findings.append(
            {
                "finding_id": sha256_text({"finding": "low_support_tct_cells", "policies": tct_summary["policy_counts"]}),
                "severity": "high",
                "finding": "low_support_tct_cells",
                "message": "TCT cell support is not yet broad enough for a final ship-gate training claim.",
                "evidence": tct_summary,
                "recommended_action": "Use this smoke field for design; wait for full #199 data and #206 data-readiness gate for training.",
            }
        )

    labels = label_balance(chunk_rows)
    training_readiness = {
        "smoke_analysis_usable_now": True,
        "full_training_may_proceed": False,
        "blocking_reasons": [
            "#199 full corpus embedder explosion is still running",
            "#206 data-readiness gate is not closed",
            "low-support TCT cells remain explicit sparse warnings in the 1000-chunk smoke field",
        ],
        "recommended_next_issue": "#206",
    }
    report = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "source_of_truth": {
            "output_root": str(args.output_root),
            "reward_root": str(args.reward_root),
            "dda_root": str(args.dda_root),
            "tct_root": str(args.tct_root),
            "matrix_root": str(args.matrix_root),
        },
        "source_hashes": {
            "reward_fsv_sha256": reward_fsv_sha,
            "dda_fsv_sha256": dda_fsv_sha,
            "tct_fsv_sha256": tct_fsv_sha,
            "matrix_fsv_sha256": matrix_fsv_sha,
            "reward_report_sha256": sha256_file(args.reward_root / "reward_tensor_report.json"),
            "tct_report_sha256": sha256_file(args.tct_root / "tct_cell_report.json"),
            "matrix_summary_sha256": sha256_file(args.matrix_root / "summary.json"),
        },
        "summary": {
            "chunk_tensor_rows": len(chunk_rows),
            "patch_tensor_rows": len(patch_rows),
            "dda_present_rows": len(signal_rows),
            "dda_missing_rows": len(missing_rows),
            "tct_cell_rows": len(cell_rows),
            "tct_sparse_warning_rows": len(sparse_warnings),
            "pairwise_rows": len(pairwise_rows),
            "collapsed_pair_count": collapsed_pair_count,
            "finding_count": len(findings),
        },
        "label_balance": labels,
        "matrix_summary": matrix_summary.get("matrix_summary") or {},
        "training_readiness": training_readiness,
    }

    write_jsonl(args.output_root / "density_by_task.jsonl", density_by_task)
    write_jsonl(args.output_root / "density_by_mutation_category.jsonl", density_by_category)
    write_jsonl(args.output_root / "density_by_chunk_kind.jsonl", density_by_chunk_kind)
    write_jsonl(args.output_root / "density_by_q_head.jsonl", density_by_q_head)
    write_jsonl(args.output_root / "density_by_embedder.jsonl", density_by_embedder)
    write_jsonl(args.output_root / "tct_cell_quality.jsonl", tct_quality)
    write_jsonl(args.output_root / "gap_findings.jsonl", findings)
    edges = edge_cases()
    write_jsonl(args.output_root / "edge_case_states.jsonl", edges)
    dump_json(args.output_root / "reward_field_analysis_report.json", report)

    after = {"output_root": tree_state(args.output_root), "fsv_root": tree_state(args.fsv_root)}
    readback_counts = {
        "density_by_task": sum(1 for _ in iter_jsonl(args.output_root / "density_by_task.jsonl")),
        "density_by_mutation_category": sum(1 for _ in iter_jsonl(args.output_root / "density_by_mutation_category.jsonl")),
        "density_by_chunk_kind": sum(1 for _ in iter_jsonl(args.output_root / "density_by_chunk_kind.jsonl")),
        "density_by_q_head": sum(1 for _ in iter_jsonl(args.output_root / "density_by_q_head.jsonl")),
        "density_by_embedder": sum(1 for _ in iter_jsonl(args.output_root / "density_by_embedder.jsonl")),
        "tct_cell_quality": sum(1 for _ in iter_jsonl(args.output_root / "tct_cell_quality.jsonl")),
        "gap_findings": sum(1 for _ in iter_jsonl(args.output_root / "gap_findings.jsonl")),
        "edge_case_states": sum(1 for _ in iter_jsonl(args.output_root / "edge_case_states.jsonl")),
    }
    passes = (
        report["summary"]["chunk_tensor_rows"] == args.expected_chunk_count
        and readback_counts["density_by_mutation_category"] == 8
        and readback_counts["density_by_q_head"] == 5
        and readback_counts["density_by_embedder"] == 12
        and readback_counts["tct_cell_quality"] == len(cell_rows)
        and readback_counts["gap_findings"] >= 1
        and report["training_readiness"]["full_training_may_proceed"] is False
        and all(case["pass"] for case in edges)
    )
    fsv = {
        "schema_version": SCHEMA_VERSION,
        "task_id": TASK_ID,
        "artifact_kind": f"{ARTIFACT_KIND}_fsv",
        "started_at_utc": started,
        "created_at_utc": utc_now(),
        "passes": passes,
        "before_state": before,
        "after_state": after,
        "source_of_truth": {
            **report["source_of_truth"],
            "report": str(args.output_root / "reward_field_analysis_report.json"),
            "fsv": str(args.fsv_root / "python_reward_field_analysis_fsv.json"),
        },
        "trigger": {"command": "python3 scripts/swebench_lite_ops.py python-reward-field-analysis"},
        "readback": {
            **readback_counts,
            "report_sha256": sha256_file(args.output_root / "reward_field_analysis_report.json"),
            "gap_findings_sha256": sha256_file(args.output_root / "gap_findings.jsonl"),
            "tct_cell_quality_sha256": sha256_file(args.output_root / "tct_cell_quality.jsonl"),
            "artifact_state": artifact_state(args.output_root),
            "training_readiness": training_readiness,
            "label_balance": labels,
        },
        "edge_cases": edges,
    }
    dump_json(args.fsv_root / "python_reward_field_analysis_fsv.json", fsv)
    final = read_json(args.fsv_root / "python_reward_field_analysis_fsv.json")
    require(final.get("passes") is True, "REWARD_FIELD_ANALYSIS_FSV_FAILED", "reward-field analysis FSV failed", {"path": str(args.fsv_root / "python_reward_field_analysis_fsv.json")})
    print(json.dumps(final, sort_keys=True, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reward-root", type=Path, default=DEFAULT_REWARD_ROOT)
    parser.add_argument("--dda-root", type=Path, default=DEFAULT_DDA_ROOT)
    parser.add_argument("--pairwise-root", type=Path, default=DEFAULT_PAIRWISE_ROOT)
    parser.add_argument("--tct-root", type=Path, default=DEFAULT_TCT_ROOT)
    parser.add_argument("--matrix-root", type=Path, default=DEFAULT_MATRIX_ROOT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--fsv-root", type=Path, default=DEFAULT_FSV_ROOT)
    parser.add_argument("--reward-fsv", type=Path, default=DEFAULT_REWARD_FSV)
    parser.add_argument("--dda-fsv", type=Path, default=DEFAULT_DDA_FSV)
    parser.add_argument("--tct-fsv", type=Path, default=DEFAULT_TCT_FSV)
    parser.add_argument("--matrix-fsv", type=Path, default=DEFAULT_MATRIX_FSV)
    parser.add_argument("--expected-chunk-count", type=int, default=1000)
    parser.add_argument("--clean-output", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return command_python_reward_field_analysis(args)
    except RewardFieldAnalysisError as exc:
        print(f"PYTHON_REWARD_FIELD_ANALYSIS_FAILED: {exc}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
