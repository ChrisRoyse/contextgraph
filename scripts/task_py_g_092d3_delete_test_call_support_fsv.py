#!/usr/bin/env python3
"""TASK-PY-G-092D3 split-legal delete_test_call support FSV.

The current delete_test_call cell has zero calibration failures, so feature work
would be holdout tuning. This harness materializes a deterministic support
overlay that uses only train/calibration rows: each fold moves one train failure
into calibration and swaps one calibration pass back into train. Holdout rows
are read for accounting only and are never selected into support folds.
"""

from __future__ import annotations

import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


INDEX_PATH = Path(
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json"
)
INDEX_SHA256 = "9bf8c80ea105936787186d178541b0d2fa8675d9e282d8ef8ba0fb5ed6da597f"
G392_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
G392_SHA256 = "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c"
D567_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092d-delete-test-call-cell-repair-fsv/"
    "run-20260521T181604Z/task_py_g_092d_delete_test_call_cell_repair_fsv.json"
)
D567_SHA256 = "7ee938fcead66002e283dfe315a8d95aa14b7194e0b89b7bc78a1a440c9a0d5d"
P564_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092p-ship-gate-power-triage-fsv/"
    "run-20260521T183128Z/task_py_g_092p_ship_gate_power_triage_fsv.json"
)
P564_SHA256 = "43a85441ce92b535b9cdce1d488d7c1745389155db58217643766c5c57d3121a"
SUPPORT_ROOT = Path(
    "/zfs/archive/contextgraph/state/splits/task-py-g-092d3-delete-test-call-support-v2"
)
INVALIDATED_SUPPORT_ROOT = Path(
    "/zfs/archive/contextgraph/state/splits/task-py-g-092d3-delete-test-call-support-v1"
)
FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092d3-delete-test-call-support-fsv"
)
MANIFEST_NAME = "delete_test_call_support_manifest.json"
FOLDS_NAME = "delete_test_call_support_folds.jsonl"
ROWS_NAME = "delete_test_call_support_rows.jsonl"
INVALIDATION_NAME = "INVALIDATED_DO_NOT_USE.json"
REPORT_NAME = "task_py_g_092d3_delete_test_call_support_fsv.json"
READBACK_NAME = "task_py_g_092d3_delete_test_call_support_readback.json"
TARGET_CATEGORY = "delete_test_call"
ALLOWED_PREFIXES = (
    "/zfs/archive/contextgraph/corpus/",
    "/zfs/archive/contextgraph/fsv/",
    "/zfs/archive/contextgraph/state/splits/",
)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def require_aiwonder_path(path: Path, name: str, *, must_exist: bool) -> None:
    resolved = path.resolve(strict=must_exist)
    text = str(resolved)
    if "/mnt/d" in text:
        raise ValueError(f"{name} must not use legacy mounted-drive path: {path}")
    if not any(text.startswith(prefix) for prefix in ALLOWED_PREFIXES):
        raise ValueError(f"{name} must be under an allowed aiwonder root: {path}")


def load_verified_json(path: Path, expected_sha: str, name: str) -> dict[str, Any]:
    require_aiwonder_path(path, name, must_exist=True)
    actual = sha256_file(path)
    if actual != expected_sha:
        raise ValueError(f"{name} sha drift: expected {expected_sha}, got {actual}")
    return json.loads(path.read_text(encoding="utf-8"))


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def write_jsonl_0600(path: Path, rows: list[dict[str, Any]]) -> None:
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )
    os.chmod(path, 0o600)


def compact_row(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "bucket": entry["bucket"],
        "category": entry["category"],
        "task_id": entry["task_id"],
        "repo": entry["repo"],
        "oracle_all_passed": entry["oracle_all_passed"],
        "oracle_per_test_count": entry.get("oracle_per_test_count"),
        "patch_path": entry["patch_path"],
        "patch_sha256": entry["patch_sha256"],
        "mutation_note": entry["mutation_note"],
    }


def row_id(entry: dict[str, Any]) -> str:
    return f"{entry['bucket']}:{entry['category']}:{entry['task_id']}:{entry['patch_sha256']}"


def support_counts(rows: list[dict[str, Any]]) -> dict[str, dict[str, int]]:
    counts = {
        "train": {"total": 0, "pass": 0, "fail": 0},
        "calibration": {"total": 0, "pass": 0, "fail": 0},
        "holdout": {"total": 0, "pass": 0, "fail": 0},
    }
    for row in rows:
        bucket = row["bucket"]
        counts[bucket]["total"] += 1
        if row["oracle_all_passed"]:
            counts[bucket]["pass"] += 1
        else:
            counts[bucket]["fail"] += 1
    return counts


def apply_moves(
    base_rows: list[dict[str, Any]], promoted_failure: dict[str, Any], swapped_pass: dict[str, Any]
) -> dict[str, Any]:
    output = []
    moves = {
        row_id(promoted_failure): "calibration",
        row_id(swapped_pass): "train",
    }
    for row in base_rows:
        changed = dict(row)
        rid = row_id(row)
        if rid in moves:
            changed["bucket"] = moves[rid]
        output.append(changed)
    return {
        "after_counts": support_counts(output),
        "moves": [
            {
                "row_id": row_id(promoted_failure),
                "from": "train",
                "to": "calibration",
                "reason": "train failure used as calibration support in this fold",
                "row": compact_row(promoted_failure),
            },
            {
                "row_id": row_id(swapped_pass),
                "from": "calibration",
                "to": "train",
                "reason": "size-preserving pass replacement; selected without holdout labels",
                "row": compact_row(swapped_pass),
            },
        ],
    }


def build_folds(rows: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    before = support_counts(rows)
    train_failures = sorted(
        [row for row in rows if row["bucket"] == "train" and not row["oracle_all_passed"]],
        key=lambda row: (-int(row.get("oracle_per_test_count") or 0), row["task_id"]),
    )
    calibration_passes = sorted(
        [row for row in rows if row["bucket"] == "calibration" and row["oracle_all_passed"]],
        key=lambda row: row["task_id"],
    )
    fold_count = min(len(train_failures), len(calibration_passes))
    folds = []
    for index in range(fold_count):
        promoted = train_failures[index]
        swapped = calibration_passes[index]
        applied = apply_moves(rows, promoted, swapped)
        after = applied["after_counts"]
        folds.append(
            {
                "fold_id": f"delete_test_call_support_fold_{index:02d}",
                "before_counts": before,
                "after_counts": after,
                "moves": applied["moves"],
                "holdout_unchanged": after["holdout"] == before["holdout"],
                "train_has_failure_support": after["train"]["fail"] >= 1,
                "calibration_has_failure_support": after["calibration"]["fail"] >= 1,
                "selection_uses_holdout_rows": False,
            }
        )
    decision = {
        "eligible": len(folds) >= 2
        and all(fold["holdout_unchanged"] for fold in folds)
        and all(fold["train_has_failure_support"] for fold in folds)
        and all(fold["calibration_has_failure_support"] for fold in folds),
        "fold_count": len(folds),
        "train_failure_count": len(train_failures),
        "calibration_pass_count": len(calibration_passes),
        "before_counts": before,
    }
    return folds, decision


def edge_case_audit() -> dict[str, Any]:
    base_pass = {
        "bucket": "calibration",
        "category": TARGET_CATEGORY,
        "task_id": "synthetic_pass",
        "repo": "synthetic/repo",
        "oracle_all_passed": True,
        "oracle_per_test_count": 1,
        "patch_path": "synthetic/pass.patch",
        "patch_sha256": "sha256:pass",
        "mutation_note": "synthetic pass",
    }
    base_fail = {
        **base_pass,
        "bucket": "train",
        "task_id": "synthetic_fail",
        "oracle_all_passed": False,
        "patch_sha256": "sha256:fail",
        "mutation_note": "synthetic fail",
    }
    second_fail = {**base_fail, "task_id": "synthetic_fail_2", "patch_sha256": "sha256:fail2"}
    two_fail_rows = [base_fail, second_fail, base_pass, {**base_pass, "task_id": "synthetic_pass_2"}]
    happy_folds, happy_decision = build_folds(two_fail_rows)
    no_fail_folds, no_fail_decision = build_folds([base_pass, {**base_pass, "task_id": "p2"}])
    no_calib_pass_folds, no_calib_pass_decision = build_folds([base_fail, second_fail])
    holdout_fail = {**base_fail, "bucket": "holdout", "task_id": "holdout_fail"}
    holdout_only_folds, holdout_only_decision = build_folds([holdout_fail, base_pass])
    return {
        "happy_two_train_failures": {
            "before": support_counts(two_fail_rows),
            "after": [fold["after_counts"] for fold in happy_folds],
            "passed": happy_decision["eligible"] is True,
        },
        "empty_failure_support_rejects": {
            "before": support_counts([base_pass, {**base_pass, "task_id": "p2"}]),
            "after": {"fold_count": len(no_fail_folds), "eligible": no_fail_decision["eligible"]},
            "passed": len(no_fail_folds) == 0 and no_fail_decision["eligible"] is False,
        },
        "no_calibration_pass_rejects_size_preserving_swap": {
            "before": support_counts([base_fail, second_fail]),
            "after": {
                "fold_count": len(no_calib_pass_folds),
                "eligible": no_calib_pass_decision["eligible"],
            },
            "passed": len(no_calib_pass_folds) == 0 and no_calib_pass_decision["eligible"] is False,
        },
        "holdout_failure_never_selected": {
            "before": support_counts([holdout_fail, base_pass]),
            "after": {
                "fold_count": len(holdout_only_folds),
                "eligible": holdout_only_decision["eligible"],
            },
            "passed": len(holdout_only_folds) == 0 and holdout_only_decision["eligible"] is False,
        },
    }


def deterministic_json(value: dict[str, Any]) -> str:
    return json.dumps(value, indent=2, sort_keys=True)


def ensure_state_file(path: Path, text: str) -> dict[str, Any]:
    before = {"exists": path.exists(), "sha256": sha256_file(path) if path.exists() else None}
    if path.exists() and path.read_text(encoding="utf-8") != text:
        raise ValueError(f"existing state file differs from deterministic content: {path}")
    path.write_text(text, encoding="utf-8")
    os.chmod(path, 0o600)
    after = {"exists": path.exists(), "sha256": sha256_file(path)}
    return {"before": before, "after": after}


def main() -> int:
    started = datetime.now(timezone.utc)
    run_id = f"run-{started.strftime('%Y%m%dT%H%M%SZ')}"
    require_aiwonder_path(SUPPORT_ROOT, "SUPPORT_ROOT", must_exist=False)
    require_aiwonder_path(INVALIDATED_SUPPORT_ROOT, "INVALIDATED_SUPPORT_ROOT", must_exist=False)
    require_aiwonder_path(FSV_ROOT, "FSV_ROOT", must_exist=False)
    SUPPORT_ROOT.mkdir(parents=True, exist_ok=True)
    run_dir = FSV_ROOT / run_id
    run_dir.mkdir(parents=True, exist_ok=False)

    index = load_verified_json(INDEX_PATH, INDEX_SHA256, "INDEX")
    g392 = load_verified_json(G392_REPORT, G392_SHA256, "G392_REPORT")
    d567 = load_verified_json(D567_REPORT, D567_SHA256, "D567_REPORT")
    p564 = load_verified_json(P564_REPORT, P564_SHA256, "P564_REPORT")
    entries = index["entries"]
    delete_rows = [row for row in entries if row["category"] == TARGET_CATEGORY]
    row_ids = [row_id(row) for row in delete_rows]
    duplicate_row_ids = sorted({rid for rid in row_ids if row_ids.count(rid) > 1})
    if duplicate_row_ids:
        raise ValueError(f"duplicate delete_test_call row ids: {duplicate_row_ids[:3]}")
    folds, decision = build_folds(delete_rows)
    support_rows = []
    for fold in folds:
        for move in fold["moves"]:
            support_rows.append(
                {
                    "fold_id": fold["fold_id"],
                    "move": {"from": move["from"], "to": move["to"]},
                    "row_id": move["row_id"],
                    "row": move["row"],
                }
            )
    support_failure_rows = [
        {"role": "train_calibration_delete_test_call_failures", "row": compact_row(row)}
        for row in delete_rows
        if row["bucket"] in {"train", "calibration"} and not row["oracle_all_passed"]
    ]
    manifest = {
        "schema_version": 1,
        "issue": 570,
        "task_id": "TASK-PY-G-092D3",
        "policy_id": "delete_test_call_leave_one_train_failure_support_v2",
        "source_index": {"path": str(INDEX_PATH), "sha256": sha256_file(INDEX_PATH)},
        "source_reports": {
            "g392": {"path": str(G392_REPORT), "sha256": sha256_file(G392_REPORT)},
            "d567": {"path": str(D567_REPORT), "sha256": sha256_file(D567_REPORT)},
            "p564": {"path": str(P564_REPORT), "sha256": sha256_file(P564_REPORT)},
        },
        "invalidated_predecessor": {
            "support_root": (
                "/zfs/archive/contextgraph/state/splits/"
                "task-py-g-092d3-delete-test-call-support-v1"
            ),
            "reason": "v1 persisted row-level holdout failure labels in reusable state",
        },
        "target_category": TARGET_CATEGORY,
        "before_counts": decision["before_counts"],
        "support_decision": {
            **decision,
            "unblocks_issue": "#569" if decision["eligible"] else None,
            "constraints_for_569": [
                "Use both support folds; do not select a rule from a single fold only.",
                "Keep holdout rows evaluation-only.",
                "If fold consensus fails, leave #569 blocked with the measured residual.",
            ],
        },
        "folds": folds,
        "train_calibration_failures": support_failure_rows,
        "holdout_policy": {
            "row_level_holdout_labels_persisted": False,
            "allowed_use": "aggregate accounting only; holdout rows remain evaluation-only",
        },
    }
    manifest_text = deterministic_json(manifest) + "\n"
    folds_text = "".join(json.dumps(fold, sort_keys=True) + "\n" for fold in folds)
    rows_text = "".join(
        json.dumps(row, sort_keys=True) + "\n"
        for row in support_rows + support_failure_rows
    )
    manifest_result = ensure_state_file(SUPPORT_ROOT / MANIFEST_NAME, manifest_text)
    folds_result = ensure_state_file(SUPPORT_ROOT / FOLDS_NAME, folds_text)
    rows_result = ensure_state_file(SUPPORT_ROOT / ROWS_NAME, rows_text)
    invalidation_result = None
    if INVALIDATED_SUPPORT_ROOT.exists():
        invalidation = {
            "schema_version": 1,
            "invalidated_at": "2026-05-21T18:47:00Z",
            "invalidated_by_issue": 570,
            "invalidated_support_root": str(INVALIDATED_SUPPORT_ROOT),
            "replacement_support_root": str(SUPPORT_ROOT),
            "reason": "v1 persisted row-level holdout failure labels in reusable state",
        }
        invalidation_result = ensure_state_file(
            INVALIDATED_SUPPORT_ROOT / INVALIDATION_NAME,
            deterministic_json(invalidation) + "\n",
        )

    edge_cases = edge_case_audit()
    source_match = {
        "g392_delete_support": g392["feature_source"]["class_support_by_category"][
            TARGET_CATEGORY
        ],
        "d567_residual": d567.get("residual_blocker"),
        "p564_next": p564["admissible_gate_moving_policy"]["p0_order"][0]["issue"],
    }
    all_passed = (
        decision["eligible"] is True
        and len(delete_rows) == 300
        and decision["before_counts"]["train"] == {"total": 240, "pass": 238, "fail": 2}
        and decision["before_counts"]["calibration"] == {"total": 30, "pass": 30, "fail": 0}
        and decision["before_counts"]["holdout"] == {"total": 30, "pass": 29, "fail": 1}
        and all(
            row["row"]["bucket"] != "holdout"
            for row in support_rows + support_failure_rows
        )
        and all(item["passed"] for item in edge_cases.values())
        and source_match["p564_next"] == "#570"
        and p564["burn_in_admissible_now"] is False
    )
    report = {
        "schema_version": 1,
        "issue": 570,
        "task_id": "TASK-PY-G-092D3",
        "run_id": run_id,
        "started_at": started.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "support_materialized": decision["eligible"],
        "support_root": str(SUPPORT_ROOT),
        "source_match": source_match,
        "source_of_truth_before_after": {
            "manifest": manifest_result,
            "folds": folds_result,
            "rows": rows_result,
            "invalidated_predecessor": invalidation_result,
        },
        "support_manifest_sha256": sha256_text(manifest_text),
        "support_folds_sha256": sha256_text(folds_text),
        "support_rows_sha256": sha256_text(rows_text),
        "support_decision": manifest["support_decision"],
        "edge_case_audit": edge_cases,
    }
    report_path = run_dir / REPORT_NAME
    readback_path = run_dir / READBACK_NAME
    write_json_0600(report_path, report)
    readback = {
        "schema_version": 1,
        "run_id": run_id,
        "all_passed": all_passed,
        "support_materialized": decision["eligible"],
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "support_manifest_path": str(SUPPORT_ROOT / MANIFEST_NAME),
        "support_manifest_sha256": sha256_file(SUPPORT_ROOT / MANIFEST_NAME),
        "support_folds_path": str(SUPPORT_ROOT / FOLDS_NAME),
        "support_folds_sha256": sha256_file(SUPPORT_ROOT / FOLDS_NAME),
        "support_rows_path": str(SUPPORT_ROOT / ROWS_NAME),
        "support_rows_sha256": sha256_file(SUPPORT_ROOT / ROWS_NAME),
        "fold_count": len(folds),
        "next_gate_issue": "#569" if decision["eligible"] else None,
    }
    write_json_0600(readback_path, readback)
    print(json.dumps(readback, indent=2, sort_keys=True))
    return 0 if all_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
