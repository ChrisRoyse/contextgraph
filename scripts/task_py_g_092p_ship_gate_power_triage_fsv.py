#!/usr/bin/env python3
"""TASK-PY-G-092P ship-gate statistical-power triage FSV.

This is a diagnostic/reporting harness only. It reads the current aiwonder
ship-gate and #392 active/abstain artifacts, enumerates the active and rolling
power blockers, and writes the admissible gate-moving policy under /zfs.
"""

from __future__ import annotations

import hashlib
import json
import math
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SHIP_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
SHIP_REPORT_SHA256 = "8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499"
G392_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
G392_REPORT_SHA256 = "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c"
FOCUSED_REPORTS = {
    "off_by_one": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092o-off-by-one-cell-repair-fsv/"
            "run-20260521T173000Z/task_py_g_092o_off_by_one_cell_repair_fsv.json"
        ),
        "22325d21b2334884c74d79e78774a7dcfe23237c31ff01bce38831cc6c1773e0",
    ),
    "subtle_flip": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092s-subtle-flip-cell-repair-fsv/"
            "run-20260521T173054Z/task_py_g_092s_subtle_flip_cell_repair_fsv.json"
        ),
        "2d77ec0ca736f64fbb49eae9112638fb99343bd0261e15925cf39f2128083f91",
    ),
    "delete_test_call": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092d-delete-test-call-cell-repair-fsv/"
            "run-20260521T181604Z/task_py_g_092d_delete_test_call_cell_repair_fsv.json"
        ),
        "7ee938fcead66002e283dfe315a8d95aa14b7194e0b89b7bc78a1a440c9a0d5d",
    ),
}
FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092p-ship-gate-power-triage-fsv"
)
REPORT_NAME = "task_py_g_092p_ship_gate_power_triage_fsv.json"
READBACK_NAME = "task_py_g_092p_ship_gate_power_triage_readback.json"
TARGET = 0.95
AIWONDER_PREFIX = "/zfs/"
DURABLE_FSV_PREFIX = "/zfs/archive/contextgraph/fsv/"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_aiwonder_path(path: Path, name: str, *, must_exist: bool) -> None:
    try:
        resolved = path.resolve(strict=must_exist)
    except FileNotFoundError as exc:
        raise FileNotFoundError(f"{name} does not exist: {path}") from exc
    text = str(resolved)
    if "/mnt/d" in text:
        raise ValueError(f"{name} must not use legacy /mnt/d: {path}")
    if not text.startswith(AIWONDER_PREFIX):
        raise ValueError(f"{name} must be on aiwonder /zfs: {path}")
    if not (text == DURABLE_FSV_PREFIX.rstrip("/") or text.startswith(DURABLE_FSV_PREFIX)):
        raise ValueError(f"{name} must be under durable FSV root {DURABLE_FSV_PREFIX}: {path}")
    if must_exist and not path.exists():
        raise FileNotFoundError(f"{name} does not exist: {path}")


def load_verified_json(path: Path, expected_sha: str, name: str) -> dict[str, Any]:
    require_aiwonder_path(path, name, must_exist=True)
    actual_sha = sha256_file(path)
    if actual_sha != expected_sha:
        raise ValueError(f"{name} sha drift: expected {expected_sha}, got {actual_sha}")
    return json.loads(path.read_text(encoding="utf-8"))


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def fisher_z_ci(r: float | None, n: int) -> dict[str, Any] | None:
    if r is None or n <= 3:
        return None
    bounded = max(min(float(r), 0.999999), -0.999999)
    z = math.atanh(bounded)
    se = 1.0 / math.sqrt(n - 3)
    lower = math.tanh(z - 1.96 * se)
    upper = math.tanh(z + 1.96 * se)
    return {
        "lower": lower,
        "upper": upper,
        "standalone_readiness_claim_powered": lower >= TARGET,
    }


def wilson_lower(successes: int, total: int) -> float | None:
    if total == 0:
        return None
    z = 1.959963984540054
    phat = successes / total
    denom = 1.0 + (z * z) / total
    center = phat + (z * z) / (2.0 * total)
    spread = z * math.sqrt((phat * (1.0 - phat) + (z * z) / (4.0 * total)) / total)
    return (center - spread) / denom


def compact_metric(metric: dict[str, Any]) -> dict[str, Any]:
    variance_class = metric.get("oracle_variance_class")
    if variance_class == "all_pass":
        recall_successes = metric.get("predicted_pass_count", 0)
    elif variance_class == "all_fail":
        recall_successes = metric.get("predicted_fail_count", 0)
    else:
        recall_successes = 0
    return {
        "sample_count": metric.get("sample_count"),
        "oracle_variance_class": metric.get("oracle_variance_class"),
        "prediction_oracle_correlation": metric.get("prediction_oracle_correlation"),
        "accuracy": metric.get("accuracy"),
        "brier": metric.get("brier"),
        "ece": metric.get("ece"),
        "actual_pass_count": metric.get("actual_pass_count"),
        "actual_fail_count": metric.get("actual_fail_count"),
        "predicted_pass_count": metric.get("predicted_pass_count"),
        "predicted_fail_count": metric.get("predicted_fail_count"),
        "fail_recall": metric.get("fail_recall"),
        "pass_recall": metric.get("pass_recall"),
        "fisher_z_95_ci": (
            metric.get("statistical_power", {}).get("fisher_z_95_ci")
            if metric.get("statistical_power")
            else fisher_z_ci(
                metric.get("prediction_oracle_correlation"),
                int(metric.get("sample_count") or 0),
            )
        ),
        "wilson_lower_95": (
            metric.get("statistical_power", {}).get("actual_class_recall_wilson_lower_95")
            if metric.get("statistical_power")
            else wilson_lower(int(recall_successes), int(metric.get("sample_count") or 0))
            if variance_class in {"all_pass", "all_fail"}
            else None
        ),
        "gate_blockers": metric.get("gate_contract", {}).get("blockers", []),
        "power_blockers": metric.get("statistical_power", {}).get("blockers", []),
    }


def blocker_record(blocker: str) -> dict[str, Any]:
    record: dict[str, Any] = {"raw": blocker}
    text = blocker
    if text.startswith("window_"):
        window, rest = text.split(":", 1)
        record["window"] = window
        text = rest
    if "::" in text:
        cell, reason = text.split(" ", 1)
        category, language = cell.split("::", 1)
        record.update({"category": category, "language": language, "reason": reason})
    else:
        record["reason"] = text
    return record


def focused_summary(reports: dict[str, dict[str, Any]]) -> dict[str, Any]:
    summary = {}
    for name, report in reports.items():
        residual = report.get("residual_blocker") or report.get("typed_blocker") or {}
        summary[name] = {
            "all_passed": report.get("all_passed"),
            "repair_promoted": report.get("repair_promoted"),
            "cell_repair_passed": report.get("cell_repair_passed"),
            "residual_blocker": residual,
        }
    return summary


def classify_active_cell(
    category: str,
    metric: dict[str, Any],
    application: dict[str, Any],
    support: dict[str, Any],
    focused: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    sample_count = int(metric.get("sample_count") or application.get("active_row_count") or 0)
    actual_fail = int(metric.get("actual_fail_count") or application.get("active_actual_fail_count") or 0)
    actual_pass = int(metric.get("actual_pass_count") or application.get("active_actual_pass_count") or 0)
    min_class = min(actual_fail, actual_pass)
    corr = metric.get("prediction_oracle_correlation")
    ci = fisher_z_ci(corr, sample_count)
    variance = metric.get("oracle_variance_class")
    split = support.get(category, {})
    total_fail = int(split.get("train_fail", 0)) + int(split.get("calibration_fail", 0)) + int(
        split.get("holdout_fail", 0)
    )
    total_pass = int(split.get("train_pass", 0)) + int(split.get("calibration_pass", 0)) + int(
        split.get("holdout_pass", 0)
    )
    has_metric_blocker = bool(metric.get("gate_contract", {}).get("blockers"))
    has_power_blocker = bool(metric.get("statistical_power", {}).get("blockers"))
    if category == "delete_test_call":
        policy = (
            "not repair-eligible on this split; add or rebalance failure-bearing "
            "delete_test_call rows before promoting #569"
        )
        next_issue = "#570"
    elif category == "off_by_one":
        policy = (
            "feature/selector weak but split has both classes; keep blocked until a "
            "calibration-supported live-safe signal appears"
        )
        next_issue = "#565"
    elif category == "subtle_flip":
        policy = (
            "feature/selector weak but split has both classes; #568 may proceed only "
            "if it introduces calibration-supported constructive-refactor evidence"
        )
        next_issue = "#568"
    elif has_power_blocker and variance in {"all_pass", "all_fail"}:
        policy = (
            "degenerate-cell point metric is green but Wilson lower bound is under "
            "target; do not count as standalone readiness until more window evidence exists"
        )
        next_issue = "#167"
    elif has_metric_blocker or has_power_blocker:
        policy = "blocked; needs focused issue before burn-in can count this cell"
        next_issue = None
    else:
        policy = "not currently a gate-moving blocker"
        next_issue = None
    return {
        "category": category,
        "language": "python",
        "sample_count": sample_count,
        "effective_active_count": application.get("active_row_count", sample_count),
        "active_coverage": application.get("active_coverage"),
        "actual_pass_count": actual_pass,
        "actual_fail_count": actual_fail,
        "predicted_pass_count": metric.get("predicted_pass_count"),
        "predicted_fail_count": metric.get("predicted_fail_count"),
        "split_support": split,
        "total_split_pass": total_pass,
        "total_split_fail": total_fail,
        "metric": compact_metric(metric),
        "computed_fisher_z_95_ci": ci,
        "min_active_class_count": min_class,
        "metric_blocked": has_metric_blocker,
        "power_blocked": has_power_blocker,
        "focused_repair_summary": focused.get(category),
        "admissible_policy": policy,
        "next_issue": next_issue,
    }


def edge_case_audit(active_diag: dict[str, Any]) -> dict[str, Any]:
    legacy_path_rejected = False
    legacy_path_error = None
    try:
        require_aiwonder_path(Path("/" + "mnt/d/contextgraph/fsv/example.json"), "EDGE_LEGACY", must_exist=False)
    except ValueError as exc:
        legacy_path_rejected = True
        legacy_path_error = exc.__class__.__name__
    empty_blockers = {"metric_blockers": [], "power_blockers": []}
    empty_blockers_alone_do_not_make_burn_in = (
        not empty_blockers["metric_blockers"]
        and not empty_blockers["power_blockers"]
        and active_diag["active_four_window_gate_passed"] is False
    )
    single_minority_ci = fisher_z_ci(0.6948082447052002, 30)
    return {
        "empty_blocker_list_is_green_only_when_power_and_metric_pass": {
            "input": empty_blockers,
            "expected": "eligible only if Panel B, aiwonder readiness, and four windows also pass",
            "passed": empty_blockers_alone_do_not_make_burn_in,
        },
        "degenerate_all_pass_cell_uses_wilson_not_pearson": {
            "input": {"successes": 30, "total": 30},
            "wilson_lower_95": wilson_lower(30, 30),
            "passed": (wilson_lower(30, 30) or 0.0) < TARGET,
        },
        "mixed_cell_single_minority_class_is_underpowered": {
            "input": {"actual_fail_count": 1, "actual_pass_count": 29},
            "fisher_z_95_ci": single_minority_ci,
            "passed": single_minority_ci is not None and single_minority_ci["lower"] < TARGET,
            "reason": "delete_test_call has one active holdout failure and zero calibration failures",
        },
        "legacy_mnt_d_path_is_rejected": {
            "input_kind": "legacy mounted-drive path",
            "error": legacy_path_error,
            "passed": legacy_path_rejected,
            "reason": "require_aiwonder_path rejects this before report creation",
        },
        "rolling_windows_not_countable_while_blockers_exist": {
            "rolling_window_blocker_count": len(active_diag.get("rolling_window_blockers", [])),
            "rolling_window_power_blocker_count": len(
                active_diag.get("rolling_window_statistical_power_blockers", [])
            ),
            "passed": bool(active_diag.get("rolling_window_blockers"))
            and bool(active_diag.get("rolling_window_statistical_power_blockers")),
        },
    }


def main() -> int:
    started = datetime.now(timezone.utc)
    run_id = f"run-{started.strftime('%Y%m%dT%H%M%SZ')}"
    require_aiwonder_path(FSV_ROOT, "FSV_ROOT", must_exist=False)
    run_dir = FSV_ROOT / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    ship = load_verified_json(SHIP_REPORT, SHIP_REPORT_SHA256, "SHIP_REPORT")
    g392 = load_verified_json(G392_REPORT, G392_REPORT_SHA256, "G392_REPORT")
    focused = {
        key: load_verified_json(path, expected_sha, f"FOCUSED_REPORT_{key}")
        for key, (path, expected_sha) in FOCUSED_REPORTS.items()
    }
    execute = ship["execute_and_inspect"]
    active_diag = execute["task_py_g_392_active_abstain_ship_gate_diagnostic"]
    embedded_source_path = Path(active_diag["source_report_path"])
    require_aiwonder_path(embedded_source_path, "ACTIVE_DIAG_SOURCE_REPORT", must_exist=True)
    active_source = load_verified_json(
        embedded_source_path,
        active_diag["source_report_sha256"],
        "ACTIVE_DIAG_SOURCE_REPORT",
    )
    embedded_source_sha256 = sha256_file(embedded_source_path)
    active_metrics = active_diag["active_metrics"]
    active_cells = active_diag["active_per_category_metrics"]
    applications = active_diag["per_category_application"]
    g392_consistency = {
        "class_support_equal": active_source["feature_source"]["class_support_by_category"]
        == g392["feature_source"]["class_support_by_category"],
        "holdout_metrics_equal": active_source["legal_score_sharpness_abstain_band"]["holdout"][
            "metrics"
        ]
        == g392["legal_score_sharpness_abstain_band"]["holdout"]["metrics"],
        "holdout_per_category_metrics_equal": active_source["legal_score_sharpness_abstain_band"][
            "holdout"
        ]["per_category_metrics"]
        == g392["legal_score_sharpness_abstain_band"]["holdout"]["per_category_metrics"],
    }
    if not all(g392_consistency.values()):
        raise ValueError(
            "ACTIVE_DIAG_SOURCE_REPORT and G392_REPORT disagree on support or holdout metrics"
        )
    support = active_source["feature_source"]["class_support_by_category"]
    cell_summaries = [
        classify_active_cell(category, metric, applications.get(category, {}), support, focused_summary(focused))
        for category, metric in sorted(active_cells.items())
    ]

    active_corr = active_metrics["prediction_oracle_correlation"]
    active_power_ci = active_metrics["statistical_power"]["fisher_z_95_ci"]
    active_metric_blocker_count = len(active_diag["active_metric_blockers"])
    active_power_blocker_count = len(active_diag["active_statistical_power_blockers"])
    rolling_blocker_count = len(active_diag["rolling_window_blockers"])
    rolling_power_blocker_count = len(active_diag["rolling_window_statistical_power_blockers"])
    policy = {
        "burn_in_admissible_now": False,
        "reason": (
            f"The #392 active point estimate is {active_corr:.6f}, but the aggregate "
            f"Fisher lower bound is {active_power_ci['lower']:.6f}; "
            f"{active_metric_blocker_count} active metric blockers, "
            f"{active_power_blocker_count} active power blockers, "
            f"{rolling_blocker_count} rolling-window blockers, and "
            f"{rolling_power_blocker_count} rolling-window power blockers remain."
        ),
        "do_not_do": [
            "Do not spend P0 time on broad skill, constellation, or feature expansion without current aiwonder gate lift.",
            "Do not promote holdout-only weak-cell rescue rules.",
            "Do not consolidate mutation buckets to hide weak cells.",
            "Do not start #167 burn-in while active/rolling power blockers remain.",
        ],
        "p0_order": [
            {
                "issue": "#570",
                "action": (
                    "create/support a split-legal delete_test_call failure-support expansion "
                    "or rebalance task so #569 can be tested without holdout tuning"
                ),
                "why": "delete_test_call has train_fail=2, calibration_fail=0, holdout_fail=1 and #567 found no legal repair.",
            },
            {
                "issue": "#521/#405",
                "action": "produce the first real Panel B measurement",
                "why": "Panel A cannot satisfy the gate alone.",
            },
            {
                "issue": "#372",
                "action": "finish aiwonder evidence-root readiness before any burn-in run",
                "why": "four-window evidence must be durable on /zfs.",
            },
            {
                "issue": "#568",
                "action": "attempt only a calibration-supported subtle_flip constructive-refactor signal",
                "why": "#566 proved the current holdout rescue would be illegal.",
            },
            {
                "issue": "#565",
                "action": "keep blocked until new calibration-supported off_by_one evidence exists",
                "why": "#565 already found a useful but illegal holdout-only rule.",
            },
            {
                "issue": "#167/#165",
                "action": "start four-window burn-in only after active cells, Panel B, power, and aiwonder readiness are green",
                "why": "current active and rolling windows are not admissible.",
            },
        ],
        "window_policy": {
            "extend_window_length": "defer until active cells and Panel B are green; current windows are pre-fix diagnostics",
            "bucket_consolidation": "reject for gate close; categories encode different failure semantics",
            "add_more_instances": "required first for delete_test_call failure support",
            "eligible_for_burn_in": False,
        },
    }

    edge_cases = edge_case_audit(active_diag)
    source_truth = {
        "ship_report": {"path": str(SHIP_REPORT), "sha256": sha256_file(SHIP_REPORT)},
        "g392_report": {"path": str(G392_REPORT), "sha256": sha256_file(G392_REPORT)},
        "embedded_active_source_report": {
            "path": str(embedded_source_path),
            "sha256": embedded_source_sha256,
        },
        "active_source_matches_g392_report_for_support_and_metrics": g392_consistency,
        "focused_reports": {
            name: {"path": str(path), "sha256": sha256_file(path)}
            for name, (path, _expected) in FOCUSED_REPORTS.items()
        },
        "output_root": str(FSV_ROOT),
        "active_paths_do_not_include_mnt_d": all(
            "/mnt/d" not in str(value)
            for value in [
                SHIP_REPORT,
                G392_REPORT,
                FSV_ROOT,
                *[path for path, _expected in FOCUSED_REPORTS.values()],
            ]
        ),
    }
    focused_report_shas_ok = all(
        source_truth["focused_reports"][name]["sha256"] == expected
        for name, (_path, expected) in FOCUSED_REPORTS.items()
    )
    all_passed = (
        source_truth["ship_report"]["sha256"] == SHIP_REPORT_SHA256
        and source_truth["g392_report"]["sha256"] == G392_REPORT_SHA256
        and source_truth["embedded_active_source_report"]["sha256"]
        == active_diag["source_report_sha256"]
        and all(
            source_truth["active_source_matches_g392_report_for_support_and_metrics"].values()
        )
        and focused_report_shas_ok
        and source_truth["active_paths_do_not_include_mnt_d"] is True
        and all(item["passed"] for item in edge_cases.values())
        and active_diag["active_metric_gate_passed"] is False
        and active_diag["active_power_gate_passed"] is False
        and active_diag["active_four_window_gate_passed"] is False
    )
    report = {
        "schema_version": 1,
        "issue": 564,
        "task_id": "TASK-PY-G-092P",
        "run_id": run_id,
        "started_at": started.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "burn_in_admissible_now": False,
        "source_of_truth": source_truth,
        "formal_ship_gate": {
            "ship_gate_passed": execute["ship_gate_passed"],
            "final_metrics": compact_metric(execute["final_metrics"]),
            "ship_gate_blockers": [blocker_record(item) for item in execute["ship_gate_blockers"]],
            "statistical_power_blockers": [
                blocker_record(item) for item in execute["statistical_power_blockers"]
            ],
        },
        "active_abstain_surface": {
            "source_report_path": active_diag["source_report_path"],
            "source_report_sha256": active_diag["source_report_sha256"],
            "active_row_count": active_diag["active_row_count"],
            "total_row_count": active_diag["total_row_count"],
            "active_coverage": active_diag["active_coverage"],
            "active_metrics": compact_metric(active_metrics),
            "active_metric_gate_passed": active_diag["active_metric_gate_passed"],
            "active_power_gate_passed": active_diag["active_power_gate_passed"],
            "active_four_window_gate_passed": active_diag["active_four_window_gate_passed"],
            "active_metric_blockers": [
                blocker_record(item) for item in active_diag["active_metric_blockers"]
            ],
            "active_statistical_power_blockers": [
                blocker_record(item) for item in active_diag["active_statistical_power_blockers"]
            ],
            "rolling_window_blockers": [
                blocker_record(item) for item in active_diag["rolling_window_blockers"]
            ],
            "rolling_window_statistical_power_blockers": [
                blocker_record(item)
                for item in active_diag["rolling_window_statistical_power_blockers"]
            ],
            "cell_summaries": cell_summaries,
        },
        "focused_weak_cell_reports": focused_summary(focused),
        "admissible_gate_moving_policy": policy,
        "edge_case_audit": edge_cases,
    }
    report_path = run_dir / REPORT_NAME
    readback_path = run_dir / READBACK_NAME
    write_json_0600(report_path, report)
    readback = {
        "schema_version": 1,
        "run_id": run_id,
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "all_passed": report["all_passed"],
        "burn_in_admissible_now": report["burn_in_admissible_now"],
        "active_metric_blocker_count": len(
            report["active_abstain_surface"]["active_metric_blockers"]
        ),
        "active_power_blocker_count": len(
            report["active_abstain_surface"]["active_statistical_power_blockers"]
        ),
        "rolling_window_blocker_count": len(
            report["active_abstain_surface"]["rolling_window_blockers"]
        ),
        "rolling_window_power_blocker_count": len(
            report["active_abstain_surface"]["rolling_window_statistical_power_blockers"]
        ),
        "next_gate_issue": "#570",
    }
    write_json_0600(readback_path, readback)
    print(json.dumps(readback, indent=2, sort_keys=True))
    return 0 if all_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
