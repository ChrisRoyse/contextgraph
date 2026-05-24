"""Candidate-keyed #392/#611 metric rerun for TASK-PY-G-135C."""

from __future__ import annotations

import argparse
import json
import math
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .python_delete_test_call_candidate_downstream import (
    baseline_summary,
    iter_jsonl,
    read_json,
    split_support,
    target_leakage_count,
)
from .python_per_cell_smooth_ece import metric_block
from .smoke_intelligence_matrices import tree_state


TASK_ID = "TASK-PY-G-135C"
ARTIFACT_KIND = "delete_test_call_candidate_keyed_metric_rerun"

DEFAULT_LABEL_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-labels-v1")
DEFAULT_DOWNSTREAM_READBACK = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-135c-candidate-keyed-downstream-fsv/"
    "run-20260524T064516Z/candidate_keyed_downstream_readback.json"
)
DEFAULT_BASELINE_611_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-134-active-point-metric-repair-fsv/"
    "run-20260522T130250Z/task_py_g_134_active_point_metric_repair_fsv.json"
)
DEFAULT_BASELINE_392_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-metric-rerun-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-135c-candidate-keyed-metric-rerun-fsv/manual-readback"
)

FIT_SPLITS = {"train", "calibration"}
EVALUATION_SPLITS = ("train", "calibration", "holdout")
ACTIVE_SCORE_LOW = 0.20
ACTIVE_SCORE_HIGH = 0.80
SHIP_CORRELATION_TARGET = 0.95
SHIP_BRIER_TARGET = 0.05
SHIP_ECE_TARGET = 0.05


class CandidateMetricRerunError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the #615 candidate-keyed labels/downstream readback and rerun only after the source-of-truth mismatch is fixed.",
            details or {},
        )


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--downstream-readback", type=Path, default=DEFAULT_DOWNSTREAM_READBACK)
    parser.add_argument("--baseline-611-report", type=Path, default=DEFAULT_BASELINE_611_REPORT)
    parser.add_argument("--baseline-392-report", type=Path, default=DEFAULT_BASELINE_392_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--expected-candidate-count", type=int, default=52)
    parser.add_argument("--min-calibration-failures-per-sublabel", type=int, default=1)
    parser.add_argument("--smoothing-alpha", type=float, default=1.0)
    parser.add_argument("--clean-output", action="store_true")


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise CandidateMetricRerunError(code, message, details)


def reject_mounted_drive(name: str, path: Path) -> None:
    text = str(path)
    if text.startswith("/mnt/c") or text.startswith("/mnt/d"):
        raise CandidateMetricRerunError(
            "DTC_METRIC_MOUNTED_DRIVE_PATH",
            f"{name} points at a mounted-drive path",
            {"name": name, "path": text},
        )


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":"), allow_nan=False))
            handle.write("\n")
            count += 1
    tmp.replace(path)
    return count


def load_code_rows(label_root: Path) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]]]:
    manifest = read_json(label_root / "candidate_keyed_label_rebuild_manifest.json", "DTC_METRIC_LABEL_MANIFEST_MISSING")
    report = read_json(label_root / "candidate_keyed_label_rebuild_report.json", "DTC_METRIC_LABEL_REPORT_MISSING")
    require(manifest.get("passes") is True, "DTC_METRIC_LABEL_MANIFEST_NOT_PASSING", "candidate label manifest is not passing")
    require(report.get("passes") is True, "DTC_METRIC_LABEL_REPORT_NOT_PASSING", "candidate label report is not passing")
    rows = list(
        iter_jsonl(
            label_root / "candidate_code_state_constellations.jsonl",
            "DTC_METRIC_CODE_STATES_MISSING",
        )
    )
    return manifest, report, rows


def load_downstream_readback(path: Path) -> dict[str, Any]:
    payload = read_json(path, "DTC_METRIC_DOWNSTREAM_READBACK_MISSING")
    require(payload.get("passes") is True, "DTC_METRIC_DOWNSTREAM_READBACK_NOT_PASSING", "downstream readback is not passing")
    summary = payload.get("summary") or {}
    movement = (summary.get("movement") or {})
    require(
        movement.get("old_no_calibration_failure_blocker_falsified") is True,
        "DTC_METRIC_DOWNSTREAM_SUPPORT_NOT_FALSIFIED",
        "downstream readback did not prove the old no-calibration-failure blocker was falsified",
    )
    return payload


def validate_code_rows(rows: list[dict[str, Any]], expected_count: int, min_calibration_failures: int) -> list[str]:
    failures: list[str] = []
    candidate_ids = [str(row.get("candidate_row_id") or "") for row in rows]
    if expected_count > 0 and len(rows) != expected_count:
        failures.append("DTC_METRIC_CANDIDATE_COUNT_MISMATCH")
    if len(set(candidate_ids)) != len(candidate_ids):
        failures.append("DTC_METRIC_DUPLICATE_CANDIDATE_ROW_ID")
    required = ("candidate_row_id", "source_task_id", "mutation_category", "assertion_duty_sublabel", "partition")
    if sum(1 for row in rows for field in required if not row.get(field)):
        failures.append("DTC_METRIC_REQUIRED_FIELD_MISSING")
    if any(row.get("mutation_category") != "delete_test_call" for row in rows):
        failures.append("DTC_METRIC_UNEXPECTED_MUTATION_CATEGORY")
    if target_leakage_count(rows):
        failures.append("DTC_METRIC_TARGET_LABEL_IN_LIVE_INPUT")
    support = split_support(rows, min_calibration_failures)
    if not support["passes"]:
        failures.append("DTC_METRIC_SPLIT_SUPPORT_GATE_FAILED")
    if not any(str(row.get("partition")) == "holdout" for row in rows):
        failures.append("DTC_METRIC_HOLDOUT_ROWS_MISSING")
    return failures


def smooth_pass_rate(rows: list[dict[str, Any]], alpha: float) -> float:
    require(rows, "DTC_METRIC_EMPTY_RATE_INPUT", "cannot fit a pass-rate score from zero rows")
    passed = sum(1 for row in rows if bool(row.get("oracle_all_passed")))
    return finite_float((passed + alpha) / (len(rows) + 2.0 * alpha))


def grouped_rates(
    fit_rows: list[dict[str, Any]],
    *,
    key_fn: Any,
    fallback_score: float,
    alpha: float,
) -> tuple[dict[str, float], Counter[str]]:
    grouped: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for row in fit_rows:
        grouped[str(key_fn(row))].append(row)
    rates = {key: smooth_pass_rate(members, alpha) for key, members in sorted(grouped.items())}
    counts = Counter({key: len(members) for key, members in grouped.items()})
    rates["__fallback__"] = fallback_score
    return rates, counts


def row_identity(row: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_row_id": row.get("candidate_row_id"),
        "source_task_id": row.get("source_task_id"),
        "repo": row.get("repo"),
        "partition": row.get("partition"),
        "category": "delete_test_call::python",
        "category_key": f"delete_test_call::{row.get('assertion_duty_sublabel')}::python",
        "assertion_duty_sublabel": row.get("assertion_duty_sublabel"),
        "workspace_state": row.get("workspace_state"),
    }


def score_rows(rows: list[dict[str, Any]], policy: dict[str, Any]) -> list[dict[str, Any]]:
    scored = []
    for row in rows:
        score = score_for_row(row, policy)
        scored.append(
            {
                **row_identity(row),
                "actual": 1 if bool(row.get("oracle_all_passed")) else 0,
                "actual_oracle_pass": bool(row.get("oracle_all_passed")),
                "score": finite_float(score),
                "predicted_pass": score >= 0.5,
                "score_in_abstain_band": ACTIVE_SCORE_LOW < score < ACTIVE_SCORE_HIGH,
                "active_after_abstain": score <= ACTIVE_SCORE_LOW or score >= ACTIVE_SCORE_HIGH,
                "policy": policy["name"],
            }
        )
    return scored


def score_for_row(row: dict[str, Any], policy: dict[str, Any]) -> float:
    name = policy["name"]
    if name == "legacy_collapsed_global_empirical_bayes":
        return float(policy["score"])
    if name == "candidate_sublabel_empirical_bayes":
        key = str(row.get("assertion_duty_sublabel"))
        return float(policy["rates"].get(key, policy["rates"]["__fallback__"]))
    raise CandidateMetricRerunError("DTC_METRIC_UNKNOWN_POLICY", f"unknown score policy: {name}", {"policy": policy})


def metric_summary(rows: list[dict[str, Any]]) -> dict[str, Any]:
    full = metric_block([int(row["actual"]) for row in rows], [float(row["score"]) for row in rows])
    active = [row for row in rows if row.get("active_after_abstain") is True]
    return {
        "full": with_counts(rows, full),
        "active_after_abstain": with_counts(
            active,
            metric_block([int(row["actual"]) for row in active], [float(row["score"]) for row in active]) if active else {},
        ),
        "row_counts": {
            "full": len(rows),
            "active_after_abstain": len(active),
            "abstain_pool": len(rows) - len(active),
        },
    }


def with_counts(rows: list[dict[str, Any]], metrics: dict[str, Any]) -> dict[str, Any]:
    labels = [int(row["actual"]) for row in rows]
    scores = [float(row["score"]) for row in rows]
    return {
        **metrics,
        "pass_count": sum(labels),
        "fail_count": len(labels) - sum(labels),
        "predicted_pass_count": sum(1 for score in scores if score >= 0.5),
        "predicted_fail_count": sum(1 for score in scores if score < 0.5),
        "score_min": finite_float(min(scores)) if scores else None,
        "score_max": finite_float(max(scores)) if scores else None,
    }


def policy_metrics(scored_rows: list[dict[str, Any]]) -> dict[str, Any]:
    by_split = {
        split: metric_summary([row for row in scored_rows if row["partition"] == split])
        for split in EVALUATION_SPLITS
    }
    by_sublabel = {}
    for sublabel in sorted({str(row["assertion_duty_sublabel"]) for row in scored_rows}):
        by_sublabel[sublabel] = metric_summary([row for row in scored_rows if row["assertion_duty_sublabel"] == sublabel])
    return {
        "all_rows": metric_summary(scored_rows),
        "by_split": by_split,
        "by_sublabel": by_sublabel,
    }


def gate_verdict(metrics: dict[str, Any]) -> dict[str, Any]:
    full = metrics.get("full") or {}
    corr = full.get("prediction_oracle_correlation")
    brier = full.get("brier")
    ece = full.get("fixed_bin_ece")
    blockers = []
    if corr is None or float(corr) < SHIP_CORRELATION_TARGET:
        blockers.append("CORRELATION_BELOW_0_95")
    if brier is None or float(brier) > SHIP_BRIER_TARGET:
        blockers.append("BRIER_ABOVE_0_05")
    if ece is None or float(ece) > SHIP_ECE_TARGET:
        blockers.append("ECE_ABOVE_0_05")
    if full.get("n", 0) < 30:
        blockers.append("LOW_CELL_N_LT_30")
    if full.get("pass_count", 0) == 0 or full.get("fail_count", 0) == 0:
        blockers.append("DEGENERATE_ORACLE_VARIANCE")
    return {"passed": not blockers, "blockers": blockers}


def selected_policy_name(policy_reports: list[dict[str, Any]]) -> str:
    candidates = []
    for report in policy_reports:
        calibration = report["metrics"]["by_split"]["calibration"]["full"]
        brier = calibration.get("brier")
        if brier is None:
            continue
        candidates.append((float(brier), str(report["policy"]["name"])))
    require(bool(candidates), "DTC_METRIC_NO_SELECTABLE_POLICY", "no policy produced calibration Brier")
    return min(candidates)[1]


def build_policies(rows: list[dict[str, Any]], *, alpha: float) -> list[dict[str, Any]]:
    fit_rows = [row for row in rows if str(row.get("partition")) in FIT_SPLITS]
    require(bool(fit_rows), "DTC_METRIC_FIT_ROWS_MISSING", "train/calibration fit rows are missing")
    fallback = smooth_pass_rate(fit_rows, alpha)
    sublabel_rates, sublabel_counts = grouped_rates(
        fit_rows,
        key_fn=lambda row: row.get("assertion_duty_sublabel"),
        fallback_score=fallback,
        alpha=alpha,
    )
    return [
        {
            "name": "legacy_collapsed_global_empirical_bayes",
            "score": finite_float(fallback),
            "fit_splits": sorted(FIT_SPLITS),
            "fit_row_count": len(fit_rows),
            "holdout_tuning_used": False,
        },
        {
            "name": "candidate_sublabel_empirical_bayes",
            "rates": sublabel_rates,
            "fit_counts": dict(sorted(sublabel_counts.items())),
            "fit_splits": sorted(FIT_SPLITS),
            "fit_row_count": len(fit_rows),
            "holdout_tuning_used": False,
            "category_key_policy": "delete_test_call::<assertion_duty_sublabel>",
        },
    ]

def command_python_delete_test_call_candidate_metric_rerun(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    for name, path in (
        ("label_root", args.label_root),
        ("downstream_readback", args.downstream_readback),
        ("baseline_611_report", args.baseline_611_report),
        ("baseline_392_report", args.baseline_392_report),
        ("output_root", args.output_root),
        ("readback_root", args.readback_root),
    ):
        reject_mounted_drive(name, path)
    require(args.expected_candidate_count >= 0, "DTC_METRIC_EXPECTED_COUNT_INVALID", "--expected-candidate-count must be >= 0")
    require(args.min_calibration_failures_per_sublabel > 0, "DTC_METRIC_MIN_CALIBRATION_SUPPORT_INVALID", "minimum calibration support must be positive")
    require(math.isfinite(float(args.smoothing_alpha)) and args.smoothing_alpha > 0.0, "DTC_METRIC_ALPHA_INVALID", "--smoothing-alpha must be positive finite")

    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    manifest, label_report, rows = load_code_rows(args.label_root)
    downstream_readback = load_downstream_readback(args.downstream_readback)
    validation_failures = validate_code_rows(rows, int(args.expected_candidate_count), int(args.min_calibration_failures_per_sublabel))
    support = split_support(rows, int(args.min_calibration_failures_per_sublabel))
    policies = build_policies(rows, alpha=float(args.smoothing_alpha))

    all_scored_rows: list[dict[str, Any]] = []
    policy_reports = []
    for policy in policies:
        scored = score_rows(rows, policy)
        all_scored_rows.extend(scored)
        metrics = policy_metrics(scored)
        policy_reports.append(
            {
                "policy": policy,
                "metrics": metrics,
                "holdout_gate_verdict": gate_verdict(metrics["by_split"]["holdout"]["full"]),
            }
        )

    selected = selected_policy_name(policy_reports)
    selected_report = next(report for report in policy_reports if report["policy"]["name"] == selected)
    repair_promoted = bool(selected_report["holdout_gate_verdict"]["passed"])
    decision = {
        "repair_promoted": repair_promoted,
        "selected_policy": selected,
        "selection_split": "calibration",
        "holdout_tuning_used": False,
        "typed_blocker": None if repair_promoted else "TASK_PY_G_135C_CANDIDATE_KEYED_METRIC_NOT_PROMOTABLE",
        "next_smallest_evidence_action": None
        if repair_promoted
        else "Do not promote a delete_test_call repair from these candidate-keyed rows alone; use this readback to design the next split-legal feature/data action before rerunning #150/#610.",
        "holdout_gate_verdict": selected_report["holdout_gate_verdict"],
    }

    rows_path = args.output_root / "candidate_metric_rerun_rows.jsonl"
    policy_path = args.output_root / "candidate_metric_policy_reports.json"
    report_path = args.output_root / "candidate_keyed_metric_rerun_report.json"
    readback_path = args.readback_root / "candidate_keyed_metric_rerun_readback.json"
    row_count = write_jsonl(rows_path, all_scored_rows)

    baseline_611 = baseline_summary(args.baseline_611_report)
    baseline_392 = read_json(args.baseline_392_report, "DTC_METRIC_BASELINE_392_BAD_JSON")
    summary = {
        "candidate_count": len(rows),
        "distinct_candidate_ids": len({str(row.get("candidate_row_id")) for row in rows}),
        "target_leakage_live_label_count": target_leakage_count(rows),
        "split_support_gate": support,
        "validation_failures": validation_failures,
        "policies_evaluated": [report["policy"]["name"] for report in policy_reports],
        "selected_policy": selected,
        "repair_promoted": repair_promoted,
        "decision": decision,
        "old_delete_test_call_metric": baseline_611.get("old_delete_test_call_metric"),
        "old_no_calibration_failure_blocker_falsified": (
            (downstream_readback.get("summary") or {}).get("movement") or {}
        ).get("old_no_calibration_failure_blocker_falsified"),
    }
    policy_payload = {
        "artifact_kind": f"{ARTIFACT_KIND}_policy_reports",
        "schema_version": 1,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "policy_reports": policy_reports,
    }
    write_json_checked(policy_path, policy_payload)
    report = {
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "schema_version": 1,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "passes": not validation_failures,
        "repair_promoted": repair_promoted,
        "source_of_truth": {
            "label_root": str(args.label_root),
            "downstream_readback": str(args.downstream_readback),
            "baseline_611_report": str(args.baseline_611_report),
            "baseline_392_report": str(args.baseline_392_report),
            "output_root": str(args.output_root),
            "readback_root": str(args.readback_root),
        },
        "input_hashes": {
            "candidate_keyed_manifest": sha256_file(args.label_root / "candidate_keyed_label_rebuild_manifest.json"),
            "candidate_keyed_report": sha256_file(args.label_root / "candidate_keyed_label_rebuild_report.json"),
            "candidate_code_state_constellations": sha256_file(args.label_root / "candidate_code_state_constellations.jsonl"),
            "downstream_readback": sha256_file(args.downstream_readback),
            "baseline_611_report": sha256_file(args.baseline_611_report),
            "baseline_392_report": sha256_file(args.baseline_392_report),
        },
        "label_manifest_line_counts": manifest.get("line_counts"),
        "label_report_summary": label_report.get("summary") or {},
        "baseline_611": baseline_611,
        "baseline_392": {
            "path": str(args.baseline_392_report),
            "sha256": sha256_file(args.baseline_392_report),
            "calibration_rows": len(baseline_392.get("score_sharpness_abstain_calibration_row_diagnostics") or []),
            "holdout_rows": len(baseline_392.get("score_sharpness_abstain_holdout_row_diagnostics") or []),
            "typed_blocker": baseline_392.get("typed_blocker"),
        },
        "summary": summary,
        "policy_reports": policy_reports,
        "output_artifacts": {
            "candidate_metric_rerun_rows": str(rows_path),
            "candidate_metric_policy_reports": str(policy_path),
        },
        "line_counts": {"candidate_metric_rerun_rows": row_count},
        "stable_hashes": {
            "candidate_metric_rerun_rows": sha256_file(rows_path),
            "candidate_metric_policy_reports": sha256_file(policy_path),
        },
        "policy": {
            "candidate_row_id_keyed": True,
            "category_key_policy": "delete_test_call::<assertion_duty_sublabel>",
            "fit_splits": sorted(FIT_SPLITS),
            "selection_split": "calibration",
            "holdout_tuning_used": False,
            "target_labels_are_supervision_only": True,
            "slot_identity_preserved": True,
        },
    }
    write_json_checked(report_path, report)
    readback = {
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "schema_version": 1,
        "task_id": TASK_ID,
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "passes": read_json(report_path, "DTC_METRIC_REPORT_READBACK_MISSING").get("passes") is True,
        "repair_promoted": repair_promoted,
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)},
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "line_counts": {"candidate_metric_rerun_rows": row_count},
        "stable_hashes": {
            "candidate_metric_rerun_rows": sha256_file(rows_path),
            "candidate_metric_policy_reports": sha256_file(policy_path),
        },
        "summary": summary,
    }
    write_json_checked(readback_path, readback)
    return {
        "ok": not validation_failures,
        "all_passed": not validation_failures,
        "command": "python-delete-test-call-candidate-metric-rerun",
        "checked_at": utc_now(),
        "report_path": str(report_path),
        "readback_path": str(readback_path),
        "report_sha256": sha256_file(report_path),
        "readback_sha256": sha256_file(readback_path),
        "summary": summary,
    }
