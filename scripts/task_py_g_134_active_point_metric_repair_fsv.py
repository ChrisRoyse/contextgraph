#!/usr/bin/env python3
"""TASK-PY-G-134 active point-metric repair/falsification FSV.

This harness reads the current aiwonder evidence for the three burn-in weak
cells and decides whether any already-retained, split-legal repair is promotable
without holdout tuning, bucket consolidation, mounted-drive evidence, or a new
embedder.
"""

from __future__ import annotations

import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TARGET = 0.95
WEAK_CELLS = ("delete_test_call", "off_by_one", "subtle_flip")
WEAK_CELL_KEYS = tuple(f"{cell}::python" for cell in WEAK_CELLS)

G392_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
G392_SHA256 = "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c"
G610_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-133-burn-in-admissibility-policy-fsv/"
    "run-20260522T123721Z/task_py_g_133_burn_in_admissibility_policy_fsv.json"
)
G610_SHA256 = "d766c06d8da35f5a1e55cfbc7981b464ce65a595d910ca16bfbc15c7e465658c"
G150_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-019-sample-efficiency-power-fsv/"
    "run-20260522T124456Z/task_py_g_019_sample_efficiency_power_fsv.json"
)
G150_SHA256 = "585c54502084c6e96037d1c49df6b901a48822284a8c839c76c9542fe8513019"

FOCUSED_NEGATIVE_REPORTS = {
    "off_by_one_holdout_only_rule": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092o-off-by-one-cell-repair-fsv/"
            "run-20260521T173000Z/task_py_g_092o_off_by_one_cell_repair_fsv.json"
        ),
        "22325d21b2334884c74d79e78774a7dcfe23237c31ff01bce38831cc6c1773e0",
    ),
    "subtle_flip_holdout_only_rescue": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092s-subtle-flip-cell-repair-fsv/"
            "run-20260521T173054Z/task_py_g_092s_subtle_flip_cell_repair_fsv.json"
        ),
        "2d77ec0ca736f64fbb49eae9112638fb99343bd0261e15925cf39f2128083f91",
    ),
    "delete_test_call_simple_signal": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092d-delete-test-call-cell-repair-fsv/"
            "run-20260521T181604Z/task_py_g_092d_delete_test_call_cell_repair_fsv.json"
        ),
        "7ee938fcead66002e283dfe315a8d95aa14b7194e0b89b7bc78a1a440c9a0d5d",
    ),
    "delete_test_call_assertion_duty": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092d2-delete-test-call-assertion-duty-fsv/"
            "run-20260521T191546Z/task_py_g_092d2_delete_test_call_assertion_duty_fsv.json"
        ),
        "02777f6007554cff53e685fba01f26214f199bc443cba2b4b217b3a5e4f5c157",
    ),
    "delete_test_call_support_overlay": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-092d3-delete-test-call-support-fsv/"
            "run-20260521T184638Z/task_py_g_092d3_delete_test_call_support_fsv.json"
        ),
        "922dd433ab3b2f0df0fa617dc9be31782a1afe2db2ac3a254f04774e32257cfa",
    ),
}

PAIR_AND_CALIBRATION_REPORTS = {
    "pair_feature_gate_probe": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124-cross-embedder-pair-features-fsv/"
            "run-20260522T010955Z-topk-probe/pair_feature_gate_probe_report.json"
        ),
        "1cbbee4f3f65d640690660ddd2ed279cf9046f790328ee53d527c2f82d0296f3",
    ),
    "conditional_pair_admission": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124b-conditional-pair-admission-fsv/"
            "run-20260522T020107Z/pair_conditional_admission_probe_report.json"
        ),
        "782389633f5aa781d3b104bde3d172da94b1de73a250002371043df205e83e51",
    ),
    "pair_interaction_mining": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124b-pair-interaction-mining-fsv/"
            "run-20260522T021203Z/pair_interaction_mining_report.json"
        ),
        "80bfd88bb3e5548fad3b2b6612ac36b5efdcf8912d2e32f155785cdf44143a7f",
    ),
    "named_pair_selector_retest": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124c-named-pair-selector-fsv/"
            "run-20260522T040650Z/named_pair_selector_retest_report.json"
        ),
        "49bcd18b84f9098c78a750e086e25c66fe2d0379ac6b0a7f9ccf3de9195a934b",
    ),
    "mendeleev_pair_table": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124d-mendeleev-pair-table-fsv/"
            "run-20260522T041330Z/mendeleev_pair_table_report.json"
        ),
        "f10be125972132a870053b9511b2d9e9382c147f0ee803896d96dca1d41b7af9",
    ),
    "per_cell_calibration_substrate": (
        Path(
            "/zfs/archive/contextgraph/fsv/task-py-g-124e-per-cell-calibration-substrate-fsv/"
            "run-20260522T040221Z/per_cell_calibration_substrate_report.json"
        ),
        "2de2439474ae12496d670bc1977487723fba3239c246c8a0227737e52fdc79b4",
    ),
}

FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-134-active-point-metric-repair-fsv"
)
REPORT_NAME = "task_py_g_134_active_point_metric_repair_fsv.json"
READBACK_NAME = "task_py_g_134_active_point_metric_repair_readback.json"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def require_aiwonder_path(path: Path, name: str, *, must_exist: bool) -> None:
    resolved = path.resolve(strict=must_exist)
    text = str(resolved)
    if "/mnt/d" in text or "/mnt/c" in text:
        raise ValueError(f"{name} must not use mounted-drive evidence: {path}")
    if not text.startswith("/zfs/archive/contextgraph/"):
        raise ValueError(f"{name} must be retained aiwonder evidence: {path}")
    if must_exist and not path.exists():
        raise FileNotFoundError(f"{name} missing: {path}")


def load_verified_json(path: Path, expected_sha: str, name: str) -> dict[str, Any]:
    require_aiwonder_path(path, name, must_exist=True)
    actual = sha256_file(path)
    if actual != expected_sha:
        raise ValueError(f"{name} sha drift: expected {expected_sha}, got {actual}")
    return json.loads(path.read_text(encoding="utf-8"))


def load_report_set(spec: dict[str, tuple[Path, str]]) -> dict[str, dict[str, Any]]:
    out = {}
    for name, (path, expected_sha) in spec.items():
        payload = load_verified_json(path, expected_sha, name)
        out[name] = {
            "path": str(path),
            "sha256": sha256_file(path),
            "payload": payload,
        }
    return out


def compact_g392_cell(report: dict[str, Any], split: str, category: str) -> dict[str, Any]:
    band = report["legal_score_sharpness_abstain_band"]
    metrics = band[split]["per_category_metrics"][category]
    application = band[f"{split}_application"]["per_category"][category]
    return {
        "category": category,
        "language": "python",
        "correlation": metrics.get("prediction_oracle_correlation"),
        "brier": metrics.get("brier"),
        "ece": metrics.get("ece"),
        "active_rows": application.get("active_row_count"),
        "total_rows": application.get("total_row_count"),
        "active_coverage": application.get("active_coverage"),
        "actual_pass_count": application.get("active_actual_pass_count"),
        "actual_fail_count": application.get("active_actual_fail_count"),
        "predicted_pass_count": application.get("active_predicted_pass_count"),
        "predicted_fail_count": application.get("active_predicted_fail_count"),
        "score_min": metrics.get("score_min"),
        "score_max": metrics.get("score_max"),
        "score_mean": metrics.get("score_mean"),
        "active_gate_passed": application.get("active_gate_passed"),
        "active_gate_blockers": application.get("active_gate_blockers"),
        "point_metric_passed": point_metric_passed(metrics, application),
    }


def point_metric_passed(metrics: dict[str, Any], application: dict[str, Any]) -> bool:
    variance_class = application.get("active_oracle_variance_class") or metrics.get(
        "oracle_variance_class"
    )
    if variance_class == "mixed":
        predicted_pass = int(application.get("active_predicted_pass_count") or 0)
        predicted_fail = int(application.get("active_predicted_fail_count") or 0)
        corr = metrics.get("prediction_oracle_correlation")
        return bool(predicted_pass > 0 and predicted_fail > 0 and corr is not None and corr >= TARGET)
    if variance_class == "all_pass":
        return bool(
            application.get("active_predicted_fail_count") == 0
            and metrics.get("brier") is not None
            and metrics["brier"] <= 0.05
        )
    if variance_class == "all_fail":
        return bool(
            application.get("active_predicted_pass_count") == 0
            and metrics.get("brier") is not None
            and metrics["brier"] <= 0.05
        )
    return False


def compact_substrate_cell(report: dict[str, Any], category: str) -> dict[str, Any]:
    cell_key = f"{category}::python"
    cell = report["after"]["holdoutReportingOnly"]["perCell"][cell_key]
    metrics = cell["metrics"]
    return {
        "category": category,
        "language": "python",
        "correlation": metrics.get("prediction_oracle_correlation"),
        "brier": metrics.get("brier"),
        "ece": metrics.get("fixed_bin_ece"),
        "smooth_ece": metrics.get("smooth_ece"),
        "active_rows": cell.get("activeRowCount"),
        "total_rows": cell.get("rowCount"),
        "active_coverage": cell.get("coverage"),
        "actual_pass_count": cell.get("selectedPassCount"),
        "actual_fail_count": cell.get("selectedFailCount"),
        "selection": cell.get("selection"),
        "point_metric_passed": (
            metrics.get("prediction_oracle_correlation") is not None
            and metrics["prediction_oracle_correlation"] >= TARGET
        ),
    }


def focused_negative_summary(reports: dict[str, dict[str, Any]]) -> dict[str, Any]:
    summary = {}
    for name, item in reports.items():
        payload = item["payload"]
        summary[name] = {
            "path": item["path"],
            "sha256": item["sha256"],
            "all_passed": payload.get("all_passed", payload.get("allPassed")),
            "repair_promoted": payload.get("repair_promoted"),
            "cell_repair_passed": payload.get("cell_repair_passed")
            or payload.get("conclusion", {}).get("cell_repair_diagnostic_passed"),
            "residual_blocker": payload.get("residual_blocker")
            or payload.get("conclusion", {}).get("residual_blocker")
            or payload.get("conclusion", {}).get("reason_gate_move_not_allowed"),
            "support_decision": payload.get("support_decision"),
        }
    return summary


def pair_calibration_summary(reports: dict[str, dict[str, Any]]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for name, item in reports.items():
        payload = item["payload"]
        out[name] = {
            "path": item["path"],
            "sha256": item["sha256"],
            "artifactKind": payload.get("artifactKind"),
            "ok": payload.get("ok"),
            "policy": payload.get("policy"),
            "promotionRecommendation": payload.get("promotionRecommendation")
            or payload.get("nextRecommendation")
            or payload.get("recommendation"),
            "promotionGate": payload.get("promotionGate")
            or payload.get("nextRecommendation", {}).get("promotionGate"),
        }
    return out


def determine_repair(
    before: dict[str, Any],
    substrate_after: dict[str, Any],
    focused: dict[str, Any],
    pair_cal: dict[str, Any],
) -> dict[str, Any]:
    blockers = []
    for category in WEAK_CELLS:
        if before[category]["point_metric_passed"] is not True:
            blockers.append(
                f"{category}::python current point metric remains below target"
            )
        if substrate_after[category]["point_metric_passed"] is not True:
            blockers.append(
                f"{category}::python per-cell calibration substrate did not clear target"
            )

    if focused["delete_test_call_simple_signal"]["repair_promoted"] is not False:
        blockers.append("delete_test_call focused negative-control state unexpected")
    if focused["subtle_flip_holdout_only_rescue"]["repair_promoted"] is not False:
        blockers.append("subtle_flip focused negative-control state unexpected")
    if pair_cal["conditional_pair_admission"]["promotionRecommendation"].get(
        "recommendedAction"
    ) != "retain_negative_no_pair_admission_policy_beat_baseline":
        blockers.append("conditional pair admission no-change finding unexpected")
    if pair_cal["per_cell_calibration_substrate"]["promotionRecommendation"].get(
        "promotionGate", {}
    ).get("promotionEligible") is not False:
        blockers.append("per-cell calibration substrate promotion gate unexpected")

    return {
        "repair_promoted": False,
        "promoted_policy": "no_change_existing_evidence_not_legally_promotable",
        "blockers": blockers,
        "typed_blocker": "TASK_PY_G_134_NO_EXISTING_LEGAL_WEAK_CELL_REPAIR",
        "next_smallest_evidence_action": (
            "Generate or materialize split-stable additional prediction/oracle rows "
            "for the three weak cells, starting with delete_test_call failure-bearing "
            "calibration support, then rerun #392 and this #611 FSV. Existing pair, "
            "Mendeleev, and per-cell calibration substrates remain diagnostic until "
            "they improve holdout weak-cell point metrics without ECE/Brier regression."
        ),
    }


def edge_case_audit(
    g392: dict[str, Any],
    g610: dict[str, Any],
    g150: dict[str, Any],
    pair_reports: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    g392_band = g392["legal_score_sharpness_abstain_band"]
    substrate = pair_reports["per_cell_calibration_substrate"]["payload"]
    pair_admission = pair_reports["conditional_pair_admission"]["payload"]
    cases = {
        "current_610_blocks_burn_in": {
            "expected": False,
            "actual": g610.get("burn_in_start_allowed"),
        },
        "current_150_blocks_support_policy": {
            "expected": "blocked",
            "actual": g150.get("support_policy"),
        },
        "delete_test_call_has_no_calibration_failure_support": {
            "expected": 0,
            "actual": g392_band["calibration"]["per_category_metrics"][
                "delete_test_call"
            ]["actual_fail_count"],
        },
        "delete_test_call_holdout_has_one_failure": {
            "expected": 1,
            "actual": g392_band["holdout"]["per_category_metrics"][
                "delete_test_call"
            ]["actual_fail_count"],
        },
        "conditional_pair_admission_changes_no_rows": {
            "expected": False,
            "actual": pair_admission["recommendation"]["changedRows"],
        },
        "per_cell_substrate_not_promotion_eligible": {
            "expected": False,
            "actual": substrate["nextRecommendation"]["promotionGate"][
                "promotionEligible"
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
    require_aiwonder_path(FSV_ROOT, "fsv_root", must_exist=False)
    run_dir.mkdir(parents=True, exist_ok=False)

    g392 = load_verified_json(G392_REPORT, G392_SHA256, "g392_report")
    g610 = load_verified_json(G610_REPORT, G610_SHA256, "g610_report")
    g150 = load_verified_json(G150_REPORT, G150_SHA256, "g150_report")
    focused_reports = load_report_set(FOCUSED_NEGATIVE_REPORTS)
    pair_reports = load_report_set(PAIR_AND_CALIBRATION_REPORTS)

    before = {
        category: compact_g392_cell(g392, "holdout", category)
        for category in WEAK_CELLS
    }
    calibration_before = {
        category: compact_g392_cell(g392, "calibration", category)
        for category in WEAK_CELLS
    }
    substrate_payload = pair_reports["per_cell_calibration_substrate"]["payload"]
    substrate_after = {
        category: compact_substrate_cell(substrate_payload, category)
        for category in WEAK_CELLS
    }
    after_promoted = before
    focused_summary = focused_negative_summary(focused_reports)
    pair_summary = pair_calibration_summary(pair_reports)
    decision = determine_repair(before, substrate_after, focused_summary, pair_summary)
    edge_cases = edge_case_audit(g392, g610, g150, pair_reports)

    source_truth = {
        "g392_report": {"path": str(G392_REPORT), "sha256": sha256_file(G392_REPORT)},
        "g610_report": {"path": str(G610_REPORT), "sha256": sha256_file(G610_REPORT)},
        "g150_report": {"path": str(G150_REPORT), "sha256": sha256_file(G150_REPORT)},
        "focused_negative_reports": {
            name: {"path": item["path"], "sha256": item["sha256"]}
            for name, item in focused_reports.items()
        },
        "pair_and_calibration_reports": {
            name: {"path": item["path"], "sha256": item["sha256"]}
            for name, item in pair_reports.items()
        },
    }
    all_sources_verified = (
        source_truth["g392_report"]["sha256"] == G392_SHA256
        and source_truth["g610_report"]["sha256"] == G610_SHA256
        and source_truth["g150_report"]["sha256"] == G150_SHA256
    )
    all_passed = all_sources_verified and all(
        case["passed"] for case in edge_cases.values()
    )

    report = {
        "schema_version": 1,
        "issue": 611,
        "task_id": "TASK-PY-G-134",
        "run_id": run_id,
        "started_at": started.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "repair_promoted": decision["repair_promoted"],
        "promoted_policy": decision["promoted_policy"],
        "typed_blocker": decision["typed_blocker"],
        "target": TARGET,
        "policy": {
            "no_bucket_consolidation": True,
            "no_holdout_threshold_tuning": True,
            "no_new_embedder_or_model_download": True,
            "no_mounted_drive_evidence": True,
            "holdout_used_for_reporting_only": True,
        },
        "source_of_truth": source_truth,
        "before_current_active_392": before,
        "calibration_split_reference": calibration_before,
        "candidate_after_per_cell_calibration_substrate_reporting_only": substrate_after,
        "after_promoted_policy": after_promoted,
        "focused_negative_controls": focused_summary,
        "pair_and_calibration_diagnostics": pair_summary,
        "decision": decision,
        "edge_cases": edge_cases,
        "rerun_order": [
            "#611 remains not promoted",
            "do not rerun #150/#610 for promotion until new rows/signals change #392 point estimates",
            "after a future promoted repair: rerun #150, then rerun #610, then start #167 only if burn_in_start_allowed=true",
        ],
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
        "repair_promoted": report["repair_promoted"],
        "typed_blocker": report["typed_blocker"],
        "weak_cell_after_promoted": {
            category: {
                "correlation": after_promoted[category]["correlation"],
                "brier": after_promoted[category]["brier"],
                "ece": after_promoted[category]["ece"],
                "active_rows": after_promoted[category]["active_rows"],
                "point_metric_passed": after_promoted[category]["point_metric_passed"],
            }
            for category in WEAK_CELLS
        },
    }
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback)
    print(json.dumps({"report": str(report_path), "sha256": report_sha}, indent=2))


if __name__ == "__main__":
    main()
