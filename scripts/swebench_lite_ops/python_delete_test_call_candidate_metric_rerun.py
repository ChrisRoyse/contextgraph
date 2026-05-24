"""Candidate-keyed #392/#611 metric rerun for TASK-PY-G-135C."""

from __future__ import annotations

import argparse
import json
import math
import shutil
from collections import Counter, defaultdict
from itertools import combinations
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
MIN_FEATURE_FIT_SUPPORT = 2
MIN_MIXED_CLASS_SUPPORT = 8
MIN_HOLDOUT_ROWS = 30
TOP_FEATURE_POLICY_LIMIT = 24
TOP_PAIR_POLICY_LIMIT = 32


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


def score_rows_with_scores(rows: list[dict[str, Any]], *, policy_name: str, scores: dict[str, float]) -> list[dict[str, Any]]:
    scored = []
    for row in rows:
        row_id = str(row.get("candidate_row_id"))
        score = float(scores[row_id])
        scored.append(
            {
                **row_identity(row),
                "actual": 1 if bool(row.get("oracle_all_passed")) else 0,
                "actual_oracle_pass": bool(row.get("oracle_all_passed")),
                "score": finite_float(score),
                "predicted_pass": score >= 0.5,
                "score_in_abstain_band": ACTIVE_SCORE_LOW < score < ACTIVE_SCORE_HIGH,
                "active_after_abstain": score <= ACTIVE_SCORE_LOW or score >= ACTIVE_SCORE_HIGH,
                "policy": policy_name,
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


def live_safe_features(row: dict[str, Any]) -> list[str]:
    features = {str(label) for label in row.get("live_predictor_labels") or []}
    sublabel = str(row.get("assertion_duty_sublabel") or "").strip()
    if sublabel:
        features.add(f"candidate_sublabel:{sublabel}")
    return sorted(features)


def validate_live_safe_feature_map(rows: list[dict[str, Any]], feature_map: dict[str, list[str]]) -> list[str]:
    synthetic = [{"live_predictor_labels": features} for features in feature_map.values()]
    failures = []
    if target_leakage_count(synthetic):
        failures.append("DTC_METRIC_FEATURE_AUDIT_TARGET_LABEL_IN_LIVE_INPUT")
    row_ids = {str(row.get("candidate_row_id")) for row in rows}
    if set(feature_map) != row_ids:
        failures.append("DTC_METRIC_FEATURE_AUDIT_ROW_ID_MISMATCH")
    return failures


def candidate_feature_policy_report(
    rows: list[dict[str, Any]],
    feature_map: dict[str, list[str]],
    *,
    feature_tuple: tuple[str, ...],
    fallback_score: float,
    alpha: float,
    policy_kind: str,
) -> dict[str, Any]:
    fit_rows = [row for row in rows if str(row.get("partition")) in FIT_SPLITS]
    present_fit = [
        row
        for row in fit_rows
        if all(feature in set(feature_map[str(row.get("candidate_row_id"))]) for feature in feature_tuple)
    ]
    score = smooth_pass_rate(present_fit, alpha)
    scores = {}
    for row in rows:
        row_id = str(row.get("candidate_row_id"))
        present = all(feature in set(feature_map[row_id]) for feature in feature_tuple)
        scores[row_id] = score if present else fallback_score
    policy_name = f"candidate_live_{policy_kind}_empirical_bayes::{'+'.join(feature_tuple)}"
    scored = score_rows_with_scores(rows, policy_name=policy_name, scores=scores)
    metrics = policy_metrics(scored)
    return {
        "policy": {
            "name": policy_name,
            "kind": policy_kind,
            "features": list(feature_tuple),
            "fit_splits": sorted(FIT_SPLITS),
            "fit_row_count": len(fit_rows),
            "support": len(present_fit),
            "fit_pass_count": sum(1 for row in present_fit if bool(row.get("oracle_all_passed"))),
            "fit_fail_count": sum(1 for row in present_fit if not bool(row.get("oracle_all_passed"))),
            "score_when_present": finite_float(score),
            "score_when_absent": finite_float(fallback_score),
            "holdout_tuning_used": False,
        },
        "metrics": metrics,
        "holdout_gate_verdict": gate_verdict(metrics["by_split"]["holdout"]["full"]),
    }


def calibration_brier(report: dict[str, Any]) -> float:
    brier = report["metrics"]["by_split"]["calibration"]["full"].get("brier")
    return float("inf") if brier is None else float(brier)


def feature_policy_audit(rows: list[dict[str, Any]], *, alpha: float) -> dict[str, Any]:
    fit_rows = [row for row in rows if str(row.get("partition")) in FIT_SPLITS]
    fallback = smooth_pass_rate(fit_rows, alpha)
    feature_map = {str(row.get("candidate_row_id")): live_safe_features(row) for row in rows}
    validation_failures = validate_live_safe_feature_map(rows, feature_map)
    feature_counts = Counter(
        feature
        for row in fit_rows
        for feature in feature_map[str(row.get("candidate_row_id"))]
    )
    feature_tuples = [
        (feature,)
        for feature, support in sorted(feature_counts.items())
        if support >= MIN_FEATURE_FIT_SUPPORT
    ]
    pair_counts: Counter[tuple[str, str]] = Counter()
    for row in fit_rows:
        row_features = feature_map[str(row.get("candidate_row_id"))]
        pair_counts.update(tuple(pair) for pair in combinations(row_features, 2))
    pair_tuples = [
        pair
        for pair, support in sorted(pair_counts.items())
        if support >= MIN_FEATURE_FIT_SUPPORT
    ]

    individual_reports = [
        candidate_feature_policy_report(
            rows,
            feature_map,
            feature_tuple=feature_tuple,
            fallback_score=fallback,
            alpha=alpha,
            policy_kind="feature",
        )
        for feature_tuple in feature_tuples
    ]
    pair_reports = [
        candidate_feature_policy_report(
            rows,
            feature_map,
            feature_tuple=feature_tuple,
            fallback_score=fallback,
            alpha=alpha,
            policy_kind="feature_pair",
        )
        for feature_tuple in pair_tuples
    ]
    individual_reports.sort(key=lambda report: (calibration_brier(report), report["policy"]["name"]))
    pair_reports.sort(key=lambda report: (calibration_brier(report), report["policy"]["name"]))
    selectable = individual_reports + pair_reports
    selectable.sort(key=lambda report: (calibration_brier(report), report["policy"]["name"]))
    selected = selectable[0] if selectable else None
    return {
        "selection_split": "calibration",
        "fit_splits": sorted(FIT_SPLITS),
        "holdout_tuning_used": False,
        "fallback_fit_pass_rate": finite_float(fallback),
        "min_feature_fit_support": MIN_FEATURE_FIT_SUPPORT,
        "feature_row_count": len(feature_map),
        "candidate_feature_count": len(feature_tuples),
        "candidate_pair_count": len(pair_tuples),
        "validation_failures": validation_failures,
        "selected_policy": selected,
        "selected_policy_promoted": bool(selected and selected["holdout_gate_verdict"]["passed"]),
        "top_individual_feature_policies": individual_reports[:TOP_FEATURE_POLICY_LIMIT],
        "top_pair_feature_policies": pair_reports[:TOP_PAIR_POLICY_LIMIT],
    }


def split_class_counts(rows: list[dict[str, Any]]) -> dict[str, dict[str, int]]:
    counts: dict[str, dict[str, int]] = {}
    for split in EVALUATION_SPLITS:
        split_rows = [row for row in rows if str(row.get("partition")) == split]
        pass_count = sum(1 for row in split_rows if bool(row.get("oracle_all_passed")))
        counts[split] = {"n": len(split_rows), "pass": pass_count, "fail": len(split_rows) - pass_count}
    return counts


def sublabel_support(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    sublabels = sorted({str(row.get("assertion_duty_sublabel")) for row in rows})
    out = []
    for sublabel in sublabels:
        split_counts = {}
        members = [row for row in rows if str(row.get("assertion_duty_sublabel")) == sublabel]
        for split in EVALUATION_SPLITS:
            split_rows = [row for row in members if str(row.get("partition")) == split]
            pass_count = sum(1 for row in split_rows if bool(row.get("oracle_all_passed")))
            split_counts[split] = {"n": len(split_rows), "pass": pass_count, "fail": len(split_rows) - pass_count}
        out.append(
            {
                "assertion_duty_sublabel": sublabel,
                "total": {
                    "n": len(members),
                    "pass": sum(1 for row in members if bool(row.get("oracle_all_passed"))),
                    "fail": sum(1 for row in members if not bool(row.get("oracle_all_passed"))),
                },
                "by_split": split_counts,
            }
        )
    return out


def next_action_report(rows: list[dict[str, Any]], policy_reports: list[dict[str, Any]], feature_audit: dict[str, Any]) -> dict[str, Any]:
    split_counts = split_class_counts(rows)
    sublabels = sublabel_support(rows)
    selected_base = next(report for report in policy_reports if report["policy"]["name"] == selected_policy_name(policy_reports))
    selected_feature = feature_audit.get("selected_policy")
    selected_base_holdout = selected_base["metrics"]["by_split"]["holdout"]["full"]
    selected_feature_holdout = (selected_feature or {}).get("metrics", {}).get("by_split", {}).get("holdout", {}).get("full", {})
    calibration = split_counts["calibration"]
    holdout = split_counts["holdout"]
    support_deficits = {
        "calibration_pass_deficit_to_min_mixed_class_support": max(0, MIN_MIXED_CLASS_SUPPORT - calibration["pass"]),
        "calibration_fail_deficit_to_min_mixed_class_support": max(0, MIN_MIXED_CLASS_SUPPORT - calibration["fail"]),
        "holdout_pass_deficit_to_min_mixed_class_support": max(0, MIN_MIXED_CLASS_SUPPORT - holdout["pass"]),
        "holdout_fail_deficit_to_min_mixed_class_support": max(0, MIN_MIXED_CLASS_SUPPORT - holdout["fail"]),
        "holdout_row_deficit_to_min_holdout_rows": max(0, MIN_HOLDOUT_ROWS - holdout["n"]),
    }
    sublabel_deficits = []
    for item in sublabels:
        reasons = []
        if item["by_split"]["calibration"]["pass"] == 0:
            reasons.append("NO_CALIBRATION_PASS_SUPPORT")
        if item["by_split"]["holdout"]["pass"] == 0:
            reasons.append("NO_HOLDOUT_PASS_SUPPORT")
        if item["total"]["pass"] == 0:
            reasons.append("NO_PASS_SUPPORT_ANY_SPLIT")
        if item["by_split"]["holdout"]["n"] < 3:
            reasons.append("LOW_HOLDOUT_N_PER_SUBLABEL")
        if reasons:
            sublabel_deficits.append({"assertion_duty_sublabel": item["assertion_duty_sublabel"], "reasons": reasons, "support": item})
    no_promotable_feature = not bool(feature_audit.get("selected_policy_promoted"))
    no_promotable_base = not bool(selected_base["holdout_gate_verdict"]["passed"])
    if no_promotable_base and no_promotable_feature:
        action_kind = "generate_more_split_stable_candidate_oracle_rows"
        action = (
            "Extend deterministic candidate generation before another metric-promotion attempt. "
            "Current legal priors and calibration-selected live-safe feature interactions do not produce a promotable holdout policy; "
            "use a predeclared generator across all splits, then rerun candidate labels and this metric readback without selecting on holdout."
        )
    elif no_promotable_base:
        action_kind = "evaluate_selected_live_safe_feature_policy"
        action = "A calibration-selected live-safe feature policy is the next candidate metric; rerun with this policy isolated and manual readback."
    else:
        action_kind = "promote_candidate_sublabel_policy"
        action = "Selected candidate-sublabel policy passes the holdout gate; promote through the downstream gate."
    return {
        "artifact_kind": f"{ARTIFACT_KIND}_next_action_report",
        "schema_version": 1,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "selection_split": "calibration",
        "holdout_tuning_used": False,
        "split_class_counts": split_counts,
        "sublabel_support": sublabels,
        "support_deficits": support_deficits,
        "sublabel_deficits": sublabel_deficits,
        "base_policy_selected": selected_base["policy"],
        "base_policy_holdout_full_metric": selected_base_holdout,
        "feature_policy_selected": (selected_feature or {}).get("policy"),
        "feature_policy_holdout_full_metric": selected_feature_holdout,
        "feature_policy_selected_holdout_gate_verdict": (selected_feature or {}).get("holdout_gate_verdict"),
        "recommended_action_kind": action_kind,
        "recommended_action": action,
        "minimum_support_targets": {
            "min_mixed_class_support_per_eval_split": MIN_MIXED_CLASS_SUPPORT,
            "min_holdout_rows": MIN_HOLDOUT_ROWS,
            "promotion_corr_target": SHIP_CORRELATION_TARGET,
            "promotion_brier_target": SHIP_BRIER_TARGET,
            "promotion_ece_target": SHIP_ECE_TARGET,
        },
        "validation_notes": [
            "live-safe feature audit uses live_predictor_labels plus assertion_duty_sublabel only",
            "target-side oracle labels remain supervision-only",
            "feature/policy selection uses calibration metrics; holdout is read only after selection",
        ],
    }

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

    feature_audit = feature_policy_audit(rows, alpha=float(args.smoothing_alpha))
    validation_failures.extend(feature_audit.get("validation_failures") or [])
    selected = selected_policy_name(policy_reports)
    selected_report = next(report for report in policy_reports if report["policy"]["name"] == selected)
    repair_promoted = bool(selected_report["holdout_gate_verdict"]["passed"])
    next_action = next_action_report(rows, policy_reports, feature_audit)
    decision = {
        "repair_promoted": repair_promoted,
        "selected_policy": selected,
        "selection_split": "calibration",
        "holdout_tuning_used": False,
        "typed_blocker": None if repair_promoted else "TASK_PY_G_135C_CANDIDATE_KEYED_METRIC_NOT_PROMOTABLE",
        "next_smallest_evidence_action": None
        if repair_promoted
        else next_action["recommended_action"],
        "next_smallest_evidence_action_kind": None if repair_promoted else next_action["recommended_action_kind"],
        "holdout_gate_verdict": selected_report["holdout_gate_verdict"],
    }

    rows_path = args.output_root / "candidate_metric_rerun_rows.jsonl"
    policy_path = args.output_root / "candidate_metric_policy_reports.json"
    feature_path = args.output_root / "candidate_live_feature_policy_audit.json"
    next_action_path = args.output_root / "candidate_keyed_next_action_report.json"
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
    write_json_checked(
        feature_path,
        {
            "artifact_kind": f"{ARTIFACT_KIND}_live_feature_policy_audit",
            "schema_version": 1,
            "task_id": TASK_ID,
            "created_at_utc": utc_now(),
            **feature_audit,
        },
    )
    write_json_checked(next_action_path, next_action)
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
        "feature_policy_audit": feature_audit,
        "next_action_report": next_action,
        "output_artifacts": {
            "candidate_metric_rerun_rows": str(rows_path),
            "candidate_metric_policy_reports": str(policy_path),
            "candidate_live_feature_policy_audit": str(feature_path),
            "candidate_keyed_next_action_report": str(next_action_path),
        },
        "line_counts": {"candidate_metric_rerun_rows": row_count},
        "stable_hashes": {
            "candidate_metric_rerun_rows": sha256_file(rows_path),
            "candidate_metric_policy_reports": sha256_file(policy_path),
            "candidate_live_feature_policy_audit": sha256_file(feature_path),
            "candidate_keyed_next_action_report": sha256_file(next_action_path),
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
            "candidate_live_feature_policy_audit": sha256_file(feature_path),
            "candidate_keyed_next_action_report": sha256_file(next_action_path),
        },
        "summary": summary,
        "next_action_report_path": str(next_action_path),
        "next_action_report_sha256": sha256_file(next_action_path),
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
