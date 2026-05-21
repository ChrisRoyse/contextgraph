#!/usr/bin/env python3
"""Focused FSV for TASK-PY-G-092S subtle_flip active-cell repair.

This script consumes the current TASK-PY-G-392 row-diagnostic report and writes
an issue-scoped report explaining whether a split-legal subtle_flip repair can
be promoted from calibration evidence.
"""

from __future__ import annotations

import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_SOURCE_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
)
SOURCE_REPORT_NAME = "task_py_g_392_temporal_trajectory_features_fsv.json"
DEFAULT_FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092s-subtle-flip-cell-repair-fsv"
)
TARGET_TASK_ID = "sympy__sympy-16792"
TARGET_CATEGORY = "subtle_flip"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    data = json.dumps(value, indent=2, sort_keys=True).encode("utf-8")
    path.write_bytes(data)
    os.chmod(path, 0o600)


def latest_source_report() -> Path:
    reports = sorted(DEFAULT_SOURCE_ROOT.glob(f"run-*/{SOURCE_REPORT_NAME}"))
    if not reports:
        raise FileNotFoundError(f"no {SOURCE_REPORT_NAME} files under {DEFAULT_SOURCE_ROOT}")
    return reports[-1]


def subtle_rows(report: dict[str, Any], key: str) -> list[dict[str, Any]]:
    rows = [
        row
        for row in report[key]
        if row["category"] == TARGET_CATEGORY
    ]
    rows.sort(key=lambda row: row["row_index"])
    return rows


def active_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [row for row in rows if row["active_after_abstain"]]


def active_misses(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [
        row
        for row in active_rows(rows)
        if not row["correct_before_abstain"]
    ]


def low_score_active_fail_rows(rows: list[dict[str, Any]], abstain_low: float) -> list[dict[str, Any]]:
    return [
        row
        for row in active_rows(rows)
        if not row["predicted_pass"] and row["predicted_oracle_pass"] < abstain_low
    ]


def compact_row(row: dict[str, Any]) -> dict[str, Any]:
    evidence = row.get("evidence_key_sample", [])
    return {
        "row_index": row["row_index"],
        "task_id": row["task_id"],
        "actual_oracle_pass": row["actual_oracle_pass"],
        "predicted_pass": row["predicted_pass"],
        "active_after_abstain": row["active_after_abstain"],
        "predicted_oracle_pass": row["predicted_oracle_pass"],
        "correct_before_abstain": row["correct_before_abstain"],
        "ece_bin_index": row["ece_bin_index"],
        "cause_bin_score": row["cause_bin_score"],
        "patch_path": row["patch_path"],
        "skill_kind_evidence": next(
            (item for item in evidence if "skill_kind:" in item),
            None,
        ),
        "slot_label_evidence": next(
            (item for item in evidence if "|slot:" in item and "|label:" in item),
            None,
        ),
    }


def count_actual(rows: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "pass": sum(1 for row in rows if row["actual_oracle_pass"]),
        "fail": sum(1 for row in rows if not row["actual_oracle_pass"]),
    }


def main() -> int:
    source_report = (
        Path(os.environ["CG_TASK_PY_G_092S_SOURCE_REPORT"])
        if "CG_TASK_PY_G_092S_SOURCE_REPORT" in os.environ
        else latest_source_report()
    )
    fsv_root = Path(os.environ.get("CG_TASK_PY_G_092S_FSV_ROOT", DEFAULT_FSV_ROOT))
    started_at = datetime.now(timezone.utc)
    run_id = f"run-{started_at.strftime('%Y%m%dT%H%M%SZ')}"
    run_dir = fsv_root / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    report = json.loads(source_report.read_text())
    abstain_model = report["legal_score_sharpness_abstain_band"]["model"]
    abstain_low = float(abstain_model["abstain_low"])
    abstain_high = float(abstain_model["abstain_high"])
    calibration = subtle_rows(report, "score_sharpness_abstain_calibration_row_diagnostics")
    holdout = subtle_rows(report, "score_sharpness_abstain_holdout_row_diagnostics")
    calibration_active_misses = active_misses(calibration)
    holdout_active_misses = active_misses(holdout)
    calibration_low_active_fail = low_score_active_fail_rows(calibration, abstain_low)
    holdout_low_active_fail = low_score_active_fail_rows(holdout, abstain_low)
    target_rows = [
        row for row in holdout_active_misses if row["task_id"] == TARGET_TASK_ID
    ]

    active_paths = report["active_input_paths"]
    patch_path = Path(active_paths[0]) / target_rows[0]["patch_path"] if target_rows else None
    patch_sha256 = sha256_file(patch_path) if patch_path else None
    patch_text = patch_path.read_text() if patch_path else ""

    edge_audits = {
        "source_report_all_passed": report.get("all_passed") is True,
        "active_paths_do_not_include_mnt_d": all("/mnt/d" not in path for path in active_paths),
        "diagnostics_cover_calibration_and_holdout": len(calibration) == 30 and len(holdout) == 30,
        "calibration_has_no_active_subtle_flip_misses": len(calibration_active_misses) == 0,
        "holdout_has_exactly_one_active_subtle_flip_miss": len(holdout_active_misses) == 1
        and bool(target_rows),
        "calibration_low_score_active_fail_region_has_zero_passes": count_actual(
            calibration_low_active_fail
        )
        == {"pass": 0, "fail": len(calibration_low_active_fail)},
        "target_patch_readback_contains_live_patch": "dump_rs" in patch_text
        and "dimensions" in patch_text,
    }

    hypotheses = {
        "comparison_or_operator_evidence": {
            "verdict": "missing_specific_signal",
            "evidence": (
                "The target row exposes broad surfaces such as call_api, assignment, "
                "boolean_logic, and numeric_boundary_or_comparison, but no live-safe "
                "operator-direction/default-argument/metadata-propagation label."
            ),
        },
        "causal_or_temporal_asymmetry": {
            "verdict": "not_load_bearing_for_target_row",
            "evidence": (
                "The target row's top slot signature is e13/e6/e9/e12; E2/E3/E4 "
                "temporal coordinates are present in the state but do not appear in "
                "the row's strongest active evidence signature."
            ),
        },
        "usefulness_weighted_skill_policy": {
            "verdict": "failure_skill_overpowering_constructive_patch",
            "evidence": (
                "The active miss carries added failure_skill evidence from delete_test_call, "
                "off_by_one, and over_engineer skill families, while calibration low-score "
                "active subtle_flip rows are all true failures."
            ),
        },
    }

    residual_blocker = {
        "code": "TASK_PY_G_092S_NO_CALIBRATION_SUPPORTED_SUBTLE_FLIP_REPAIR",
        "message": (
            "Do not promote a holdout-only rescue for sympy__sympy-16792. Calibration has "
            "zero active subtle_flip misses and zero pass examples in the low-score active "
            "fail region, so the smallest legal next repair is new live-safe constructive "
            "refactor/default-argument metadata evidence, then rerun #392."
        ),
    }

    all_passed = all(edge_audits.values())
    focused = {
        "schema_version": 1,
        "issue": 566,
        "run_id": run_id,
        "started_at": started_at.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "repair_promoted": False,
        "cell_repair_passed": False,
        "residual_blocker": residual_blocker,
        "source_report_path": str(source_report),
        "source_report_sha256": sha256_file(source_report),
        "active_input_paths": active_paths,
        "abstain_model": {
            "abstain_low": abstain_low,
            "abstain_high": abstain_high,
        },
        "before_after": {
            "policy": "unchanged because no calibration-supported repair exists",
            "calibration_active_misses_before": [
                compact_row(row) for row in calibration_active_misses
            ],
            "holdout_active_misses_before": [
                compact_row(row) for row in holdout_active_misses
            ],
            "holdout_active_misses_after": [
                compact_row(row) for row in holdout_active_misses
            ],
        },
        "support": {
            "calibration_subtle_flip_rows": len(calibration),
            "holdout_subtle_flip_rows": len(holdout),
            "calibration_active_rows": len(active_rows(calibration)),
            "holdout_active_rows": len(active_rows(holdout)),
            "calibration_active_miss_count": len(calibration_active_misses),
            "holdout_active_miss_count": len(holdout_active_misses),
            "calibration_low_score_active_fail_region": {
                "count": len(calibration_low_active_fail),
                "actual": count_actual(calibration_low_active_fail),
                "rows": [compact_row(row) for row in calibration_low_active_fail],
            },
            "holdout_low_score_active_fail_region": {
                "count": len(holdout_low_active_fail),
                "actual": count_actual(holdout_low_active_fail),
                "rows": [compact_row(row) for row in holdout_low_active_fail],
            },
        },
        "target_patch": {
            "path": str(patch_path) if patch_path else None,
            "sha256": patch_sha256,
            "contains_dimensions_helper": "dimensions" in patch_text,
            "contains_dump_rs_default_change": "dump_rs" in patch_text
            and "empty=False" in patch_text,
        },
        "hypotheses": hypotheses,
        "edge_audits": edge_audits,
    }

    report_path = run_dir / "task_py_g_092s_subtle_flip_cell_repair_fsv.json"
    write_json_0600(report_path, focused)
    readback = json.loads(report_path.read_text())
    readback_report = {
        "schema_version": 1,
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "readback_all_passed": readback["all_passed"],
        "readback_repair_promoted": readback["repair_promoted"],
        "readback_cell_repair_passed": readback["cell_repair_passed"],
        "readback_residual_blocker": readback["residual_blocker"],
        "readback_edge_audits": readback["edge_audits"],
        "readback_holdout_active_miss_count": readback["support"]["holdout_active_miss_count"],
        "readback_calibration_active_miss_count": readback["support"][
            "calibration_active_miss_count"
        ],
    }
    readback_path = run_dir / "task_py_g_092s_subtle_flip_cell_repair_readback.json"
    write_json_0600(readback_path, readback_report)
    print(
        json.dumps(
            {
                "report_path": str(report_path),
                "readback_path": str(readback_path),
                "report_sha256": readback_report["report_sha256"],
                "all_passed": all_passed,
                "residual_blocker": residual_blocker["code"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if all_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
