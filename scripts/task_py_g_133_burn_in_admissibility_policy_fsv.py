#!/usr/bin/env python3
"""TASK-PY-G-133 burn-in admissibility policy FSV.

Reads current aiwonder gate evidence and emits the decision #167 needs before
any four-window burn-in rows can count.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


G392_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
G392_SHA256 = "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c"

PANEL_B_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-111-cross-panel-triangulation-fsv/"
    "run-20260522T012329Z-parity-active392/cross_panel_triangulation_fsv.json"
)
PANEL_B_SHA256 = "d0c1bc060835692e02ab81a2eec325a07116e1db87dc44139ab468150edf0c31"

POWER_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092p-ship-gate-power-triage-fsv/"
    "run-20260521T183128Z/task_py_g_092p_ship_gate_power_triage_fsv.json"
)
POWER_SHA256 = "43a85441ce92b535b9cdce1d488d7c1745389155db58217643766c5c57d3121a"

NEGATIVE_CONTROL_REPORTS = {
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
            "/zfs/archive/contextgraph/fsv/task-py-g-092d2-delete-test-call-assertion-duty-fsv/"
            "run-20260521T191546Z/task_py_g_092d2_delete_test_call_assertion_duty_fsv.json"
        ),
        "02777f6007554cff53e685fba01f26214f199bc443cba2b4b217b3a5e4f5c157",
    ),
}

FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-133-burn-in-admissibility-policy-fsv"
)
REPORT_NAME = "task_py_g_133_burn_in_admissibility_policy_fsv.json"
READBACK_NAME = "task_py_g_133_burn_in_admissibility_policy_readback.json"
TARGET = 0.95


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_verified_json(path: Path, expected_sha: str, name: str) -> dict[str, Any]:
    actual_sha = sha256_file(path)
    if actual_sha != expected_sha:
        raise ValueError(f"{name} sha drift: expected {expected_sha}, got {actual_sha}")
    return json.loads(path.read_text(encoding="utf-8"))


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def findmnt(path: str) -> dict[str, str]:
    proc = subprocess.run(
        ["findmnt", "-T", path, "-o", "TARGET,SOURCE,FSTYPE,OPTIONS", "-n"],
        check=True,
        capture_output=True,
        text=True,
    )
    parts = proc.stdout.strip().split(maxsplit=3)
    if len(parts) != 4:
        raise ValueError(f"unexpected findmnt output for {path!r}: {proc.stdout!r}")
    return {
        "target": parts[0],
        "source": parts[1],
        "fstype": parts[2],
        "options": parts[3],
    }


def root_ready() -> dict[str, Any]:
    archive = findmnt("/zfs/archive/contextgraph")
    hot = findmnt("/zfs/hot/contextgraph")
    return {
        "archive": archive,
        "hot": hot,
        "ready": (
            archive["source"] == "archive/contextgraph"
            and archive["fstype"] == "zfs"
            and hot["source"] == "hotpool/contextgraph"
            and hot["fstype"] == "zfs"
        ),
    }


def panel_b_summary(report: dict[str, Any]) -> dict[str, Any]:
    metric = report["modelBackedWindow"]["metric"]
    checks = report["checks"]
    return {
        "all_passed": report["allPassed"],
        "ship_gate_eligible": report["shipGateEligible"],
        "row_aligned": checks["rowAligned"],
        "row_count": checks["rowCount"],
        "panel_b_model_backed": report["modelBackedPanelB"],
        "panel_b_resident_during_normal_inference": checks[
            "panelBResidentDuringNormalInference"
        ],
        "encoder_non_overlap": checks["encoderNonOverlap"],
        "panel_ab_artifact_overlap_count": checks["panelABArtifactOverlapCount"],
        "panel_a_oracle_correlation": metric["panelAOracleCorrelation"],
        "panel_b_oracle_correlation": metric["panelBOracleCorrelation"],
        "abs_correlation_delta": metric["absCorrelationDelta"],
        "flag": metric["flag"],
        "ready": (
            report["allPassed"] is True
            and report["shipGateEligible"] is True
            and report["modelBackedPanelB"] is True
            and checks["rowAligned"] is True
            and checks["encoderNonOverlap"] is True
            and checks["panelBResidentDuringNormalInference"] is False
            and checks["panelABArtifactOverlapCount"] == 0
            and metric["flag"] == "healthy"
            and metric["absCorrelationDelta"] <= 0.05
        ),
    }


def power_summary(report: dict[str, Any]) -> dict[str, Any]:
    active = report["active_abstain_surface"]
    return {
        "burn_in_admissible_now": report["burn_in_admissible_now"],
        "active_metric_blockers": active["active_metric_blockers"],
        "active_statistical_power_blockers": active["active_statistical_power_blockers"],
        "rolling_window_blockers": active["rolling_window_blockers"],
        "rolling_window_statistical_power_blockers": active[
            "rolling_window_statistical_power_blockers"
        ],
        "active_metric_blocker_count": len(active["active_metric_blockers"]),
        "active_statistical_power_blocker_count": len(
            active["active_statistical_power_blockers"]
        ),
        "rolling_window_blocker_count": len(active["rolling_window_blockers"]),
        "rolling_window_statistical_power_blocker_count": len(
            active["rolling_window_statistical_power_blockers"]
        ),
    }


def active_abstain_summary(report: dict[str, Any]) -> dict[str, Any]:
    active_band = report["legal_score_sharpness_abstain_band"]
    metrics = active_band["holdout"]["metrics"]
    application = active_band["holdout_application"]
    return {
        "source_key": "legal_score_sharpness_abstain_band",
        "selection_forbidden": active_band["selection_forbidden"],
        "blockers": active_band["blockers"],
        "active_rows": application["active_row_count"],
        "abstained_rows": application["abstained_row_count"],
        "total_rows": application["total_row_count"],
        "active_coverage": application["active_coverage"],
        "active_correlation": metrics["prediction_oracle_correlation"],
        "active_brier": metrics["brier"],
        "active_ece": metrics["ece"],
        "active_accuracy": metrics["accuracy"],
        "per_category": {
            name: {
                "active_row_count": value["active_row_count"],
                "abstained_row_count": value["abstained_row_count"],
                "active_gate_passed": value["active_gate_passed"],
                "active_gate_blockers": value["active_gate_blockers"],
                "active_oracle_variance_class": value["active_oracle_variance_class"],
            }
            for name, value in application["per_category"].items()
        },
    }


def negative_control_summary() -> dict[str, Any]:
    result: dict[str, Any] = {}
    for name, (path, expected_sha) in NEGATIVE_CONTROL_REPORTS.items():
        report = load_verified_json(path, expected_sha, f"{name}_negative_control")
        result[name] = {
            "path": str(path),
            "sha256": sha256_file(path),
            "all_passed": report.get("all_passed", report.get("allPassed")),
            "repair_promoted": report.get("repair_promoted"),
            "residual_blocker": report.get("residual_blocker"),
        }
    return result


def classify_policy(roots: dict[str, Any], panel_b: dict[str, Any], power: dict[str, Any]) -> dict[str, Any]:
    blockers: list[str] = []
    if not roots["ready"]:
        blockers.append("AIWONDER_ROOTS_NOT_ON_EXPECTED_ZFS_DATASETS")
    if not panel_b["ready"]:
        blockers.append("PANEL_B_PARITY_NOT_READY")
    if power["active_metric_blocker_count"] > 0:
        blockers.append("ACTIVE_CELL_METRIC_BLOCKERS_REMAIN")
    if power["active_statistical_power_blocker_count"] > 0:
        blockers.append("ACTIVE_STATISTICAL_POWER_BLOCKERS_REMAIN")
    if power["rolling_window_blocker_count"] > 0:
        blockers.append("ROLLING_WINDOW_BLOCKERS_REMAIN")
    if power["rolling_window_statistical_power_blocker_count"] > 0:
        blockers.append("ROLLING_WINDOW_POWER_BLOCKERS_REMAIN")
    return {
        "burn_in_start_allowed": not blockers,
        "blockers": blockers,
        "decision_code": "TASK_PY_G_133_BURN_IN_READY"
        if not blockers
        else "TASK_PY_G_133_BURN_IN_POWER_NOT_ADMISSIBLE",
        "next_action": "start #167 burn-in with the named roots and panel evidence"
        if not blockers
        else (
            "do not start #167; create split-stable additional row support or a "
            "window-length policy that resolves the listed active and rolling "
            "power blockers without bucket consolidation or holdout tuning"
        ),
    }


def edge_case_audit() -> dict[str, dict[str, Any]]:
    ready_roots = {
        "ready": True,
        "archive": {"source": "archive/contextgraph", "fstype": "zfs"},
        "hot": {"source": "hotpool/contextgraph", "fstype": "zfs"},
    }
    ready_panel = {
        "ready": True,
        "abs_correlation_delta": 0.01,
        "panel_a_oracle_correlation": 0.96,
        "panel_b_oracle_correlation": 0.97,
    }
    ready_power = {
        "active_metric_blocker_count": 0,
        "active_statistical_power_blocker_count": 0,
        "rolling_window_blocker_count": 0,
        "rolling_window_statistical_power_blocker_count": 0,
    }
    root_fail = dict(ready_roots)
    root_fail["ready"] = False
    panel_fail = dict(ready_panel)
    panel_fail["ready"] = False
    power_fail = dict(ready_power)
    power_fail["active_metric_blocker_count"] = 1
    cases = {
        "happy_path_all_predicates_green": {
            "expected": True,
            "actual": classify_policy(ready_roots, ready_panel, ready_power)[
                "burn_in_start_allowed"
            ],
        },
        "root_not_ready_blocks_burn_in": {
            "expected": False,
            "actual": classify_policy(root_fail, ready_panel, ready_power)[
                "burn_in_start_allowed"
            ],
        },
        "panel_b_not_ready_blocks_burn_in": {
            "expected": False,
            "actual": classify_policy(ready_roots, panel_fail, ready_power)[
                "burn_in_start_allowed"
            ],
        },
        "active_power_blocker_blocks_burn_in": {
            "expected": False,
            "actual": classify_policy(ready_roots, ready_panel, power_fail)[
                "burn_in_start_allowed"
            ],
        },
    }
    for case in cases.values():
        case["passed"] = case["actual"] == case["expected"]
    return cases


def main() -> None:
    started = datetime.now(timezone.utc)
    run_id = f"run-{started.strftime('%Y%m%dT%H%M%SZ')}"
    run_dir = FSV_ROOT / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    g392 = load_verified_json(G392_REPORT, G392_SHA256, "g392_report")
    panel_b_report = load_verified_json(PANEL_B_REPORT, PANEL_B_SHA256, "panel_b_report")
    power_report = load_verified_json(POWER_REPORT, POWER_SHA256, "power_report")
    roots = root_ready()
    panel_b = panel_b_summary(panel_b_report)
    power = power_summary(power_report)
    active_abstain = active_abstain_summary(g392)
    negative_controls = negative_control_summary()
    edge_cases = edge_case_audit()
    policy = classify_policy(roots, panel_b, power)

    source_truth = {
        "g392_report": {"path": str(G392_REPORT), "sha256": sha256_file(G392_REPORT)},
        "panel_b_report": {"path": str(PANEL_B_REPORT), "sha256": sha256_file(PANEL_B_REPORT)},
        "power_report": {"path": str(POWER_REPORT), "sha256": sha256_file(POWER_REPORT)},
        "root_readback": roots,
        "negative_control_reports": negative_controls,
    }
    all_passed = (
        source_truth["g392_report"]["sha256"] == G392_SHA256
        and source_truth["panel_b_report"]["sha256"] == PANEL_B_SHA256
        and source_truth["power_report"]["sha256"] == POWER_SHA256
        and roots["ready"] is True
        and panel_b["ready"] is True
        and active_abstain["selection_forbidden"] is False
        and active_abstain["active_correlation"] >= TARGET
        and all(item["passed"] for item in edge_cases.values())
    )
    report = {
        "schema_version": 1,
        "issue": 610,
        "task_id": "TASK-PY-G-133",
        "run_id": run_id,
        "started_at": started.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "burn_in_start_allowed": policy["burn_in_start_allowed"],
        "decision_code": policy["decision_code"],
        "next_action": policy["next_action"],
        "policy": {
            "reject_bucket_consolidation": True,
            "reject_holdout_tuning": True,
            "allow_more_rows_or_longer_windows_only_with_split_stable_lineage": True,
            "target": TARGET,
        },
        "source_of_truth": source_truth,
        "active_abstain_summary": active_abstain,
        "panel_b_summary": panel_b,
        "power_summary": power,
        "blockers": policy["blockers"],
        "edge_cases": edge_cases,
    }

    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, report)
    report_sha = sha256_file(report_path)
    readback = {
        "schema_version": 1,
        "report_path": str(report_path),
        "report_sha256": report_sha,
        "report_reopened_sha256": sha256_file(report_path),
        "all_passed": report["all_passed"],
        "burn_in_start_allowed": report["burn_in_start_allowed"],
        "decision_code": report["decision_code"],
        "root_ready": roots["ready"],
        "panel_b_ready": panel_b["ready"],
        "blocker_count": len(policy["blockers"]),
    }
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback)
    print(json.dumps({"report": str(report_path), "sha256": report_sha}, indent=2))


if __name__ == "__main__":
    main()
