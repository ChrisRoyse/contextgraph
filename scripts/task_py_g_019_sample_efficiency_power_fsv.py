#!/usr/bin/env python3
"""TASK-PY-G-019 sample-efficiency / burn-in power FSV.

Computes whether the current #610 residual can be resolved by row support or
window length alone, without touching predictor features or tuning holdout.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TARGET = 0.95
Z_95 = 1.959963984540054
FISHER_TARGET_Z = math.atanh(TARGET)

G610_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-133-burn-in-admissibility-policy-fsv/"
    "run-20260522T123721Z/task_py_g_133_burn_in_admissibility_policy_fsv.json"
)
G610_SHA256 = "d766c06d8da35f5a1e55cfbc7981b464ce65a595d910ca16bfbc15c7e465658c"

FSV_ROOT = Path("/zfs/archive/contextgraph/fsv/task-py-g-019-sample-efficiency-power-fsv")
REPORT_NAME = "task_py_g_019_sample_efficiency_power_fsv.json"
READBACK_NAME = "task_py_g_019_sample_efficiency_power_readback.json"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json_verified(path: Path, expected_sha: str, name: str) -> dict[str, Any]:
    actual_sha = sha256_file(path)
    if actual_sha != expected_sha:
        raise ValueError(f"{name} sha drift: expected {expected_sha}, got {actual_sha}")
    return json.loads(path.read_text(encoding="utf-8"))


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def fisher_lower_95(r_value: float, n_value: int) -> float | None:
    if n_value <= 3 or abs(r_value) >= 1.0:
        return None
    return math.tanh(math.atanh(r_value) - Z_95 / math.sqrt(n_value - 3))


def required_n_for_fisher_lower(r_value: float, target: float = TARGET) -> int | None:
    if r_value <= target or r_value >= 1.0:
        return None
    margin = math.atanh(r_value) - math.atanh(target)
    if margin <= 0:
        return None
    return math.ceil(3.0 + (Z_95 / margin) ** 2)


def wilson_lower_for_all_successes(n_value: int) -> float:
    if n_value <= 0:
        return 0.0
    return n_value / (n_value + Z_95**2)


def required_all_successes_for_wilson(target: float = TARGET) -> int:
    n_value = 1
    while wilson_lower_for_all_successes(n_value) < target:
        n_value += 1
    return n_value


def parse_corr_blocker(blocker: dict[str, Any]) -> dict[str, Any]:
    reason = blocker.get("reason", blocker.get("raw", ""))
    match = re.search(r"prediction_oracle_correlation ([0-9.\\-]+) < ([0-9.\\-]+)", reason)
    if not match:
        return {
            "category": blocker.get("category"),
            "language": blocker.get("language"),
            "raw": blocker.get("raw"),
            "parse_status": "malformed",
        }
    observed = float(match.group(1))
    target = float(match.group(2))
    return {
        "category": blocker.get("category"),
        "language": blocker.get("language"),
        "observed_correlation": observed,
        "target": target,
        "point_metric_below_target": observed < target,
        "window_length_only_can_fix": observed >= target,
        "raw": blocker.get("raw"),
        "parse_status": "ok",
    }


def source_readback(g610: dict[str, Any]) -> dict[str, Any]:
    sources = g610["source_of_truth"]
    result = {
        "g610_report": {"path": str(G610_REPORT), "sha256": sha256_file(G610_REPORT)},
    }
    for key in ["g392_report", "panel_b_report", "power_report"]:
        item = sources[key]
        path = Path(item["path"])
        result[key] = {"path": str(path), "sha256": sha256_file(path)}
    return result


def classify_policy(g610: dict[str, Any]) -> dict[str, Any]:
    power = g610["power_summary"]
    active = g610["active_abstain_summary"]
    active_metric_blockers = [
        parse_corr_blocker(item) for item in power["active_metric_blockers"]
    ]
    point_metric_blockers = [
        item for item in active_metric_blockers if item.get("point_metric_below_target")
    ]
    degenerate_required_n = required_all_successes_for_wilson(TARGET)
    per_category = active["per_category"]
    degenerate_support = {}
    for category, value in per_category.items():
        variance_class = value["active_oracle_variance_class"]
        if variance_class in {"all_fail", "all_pass"}:
            current_n = int(value["active_row_count"])
            degenerate_support[category] = {
                "variance_class": variance_class,
                "current_active_rows": current_n,
                "required_all_correct_rows": degenerate_required_n,
                "additional_all_correct_rows_needed": max(
                    0, degenerate_required_n - current_n
                ),
                "current_wilson_lower_if_all_correct": wilson_lower_for_all_successes(
                    current_n
                ),
            }
    overall_required_n = required_n_for_fisher_lower(active["active_correlation"], TARGET)
    overall_additional = (
        None
        if overall_required_n is None
        else max(0, overall_required_n - int(active["active_rows"]))
    )
    support_policy = "blocked" if point_metric_blockers else "extend_windows"
    if support_policy == "blocked":
        next_action = (
            "repair active point metrics for delete_test_call/off_by_one/subtle_flip "
            "or generate split-stable new rows that demonstrably change those point "
            "estimates before #167 burn-in can be admissible"
        )
    else:
        next_action = (
            "extend active support to the computed Fisher/Wilson minima, then rerun "
            "#610 before starting #167"
        )
    return {
        "support_policy": support_policy,
        "next_action": next_action,
        "active_metric_blockers": active_metric_blockers,
        "point_metric_blocker_count": len(point_metric_blockers),
        "window_length_only_blocked_by_point_metrics": len(point_metric_blockers) > 0,
        "overall_active_fisher": {
            "current_rows": active["active_rows"],
            "current_correlation": active["active_correlation"],
            "current_lower_95": fisher_lower_95(
                active["active_correlation"], int(active["active_rows"])
            ),
            "required_rows_if_point_estimate_unchanged": overall_required_n,
            "additional_rows_if_point_estimate_unchanged": overall_additional,
        },
        "degenerate_cell_wilson": {
            "target": TARGET,
            "required_all_correct_rows": degenerate_required_n,
            "per_category": degenerate_support,
        },
        "rolling_window_blockers": power["rolling_window_blockers"],
        "rolling_window_statistical_power_blockers": power[
            "rolling_window_statistical_power_blockers"
        ],
        "policy_constraints": {
            "bucket_consolidation_allowed": False,
            "holdout_tuning_allowed": False,
            "new_embedder_or_model_allowed": False,
            "mounted_drive_active_evidence_allowed": False,
        },
    }


def edge_case_audit() -> dict[str, dict[str, Any]]:
    malformed = parse_corr_blocker({"raw": "not a metric blocker"})
    impossible = required_n_for_fisher_lower(0.90, TARGET)
    possible = required_n_for_fisher_lower(0.99, TARGET)
    wilson_n = required_all_successes_for_wilson(TARGET)
    cases = {
        "malformed_blocker_is_detected": {
            "expected": "malformed",
            "actual": malformed["parse_status"],
        },
        "below_target_fisher_point_estimate_cannot_be_power_fixed": {
            "expected": None,
            "actual": impossible,
        },
        "above_target_fisher_point_estimate_has_required_n": {
            "expected": "positive_integer",
            "actual": possible,
        },
        "zero_variance_wilson_requires_more_than_30_rows": {
            "expected": True,
            "actual": wilson_n > 30,
        },
    }
    for case in cases.values():
        expected = case["expected"]
        actual = case["actual"]
        case["passed"] = (
            (expected == "positive_integer" and isinstance(actual, int) and actual > 0)
            or actual == expected
        )
    return cases


def main() -> None:
    started = datetime.now(timezone.utc)
    run_id = f"run-{started.strftime('%Y%m%dT%H%M%SZ')}"
    run_dir = FSV_ROOT / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    g610 = load_json_verified(G610_REPORT, G610_SHA256, "g610_report")
    readback = source_readback(g610)
    policy = classify_policy(g610)
    edge_cases = edge_case_audit()
    all_passed = (
        readback["g610_report"]["sha256"] == G610_SHA256
        and policy["support_policy"] in {"add_rows", "extend_windows", "blocked"}
        and all(item["passed"] for item in edge_cases.values())
    )
    report = {
        "schema_version": 1,
        "issue": 150,
        "task_id": "TASK-PY-G-019",
        "run_id": run_id,
        "started_at": started.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "support_policy": policy["support_policy"],
        "next_action": policy["next_action"],
        "source_of_truth": readback,
        "power_policy": policy,
        "edge_cases": edge_cases,
    }

    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, report)
    report_sha = sha256_file(report_path)
    readback_report = {
        "schema_version": 1,
        "report_path": str(report_path),
        "report_sha256": report_sha,
        "report_reopened_sha256": sha256_file(report_path),
        "all_passed": all_passed,
        "support_policy": policy["support_policy"],
        "point_metric_blocker_count": policy["point_metric_blocker_count"],
    }
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback_report)
    print(json.dumps({"report": str(report_path), "sha256": report_sha}, indent=2))


if __name__ == "__main__":
    main()
