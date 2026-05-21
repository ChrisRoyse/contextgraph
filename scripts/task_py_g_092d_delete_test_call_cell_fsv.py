#!/usr/bin/env python3
"""Focused FSV for TASK-PY-G-092D delete_test_call active-cell repair.

This consumes the current TASK-PY-G-392 row-diagnostic report plus the
SWE-bench 300x8 corpus index. It proves whether a split-legal repair exists
for the delete_test_call::python weak cell, or writes a retained blocker FSV.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SOURCE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/"
    "run-20260521T171548Z/task_py_g_392_temporal_trajectory_features_fsv.json"
)
EXPECTED_SOURCE_REPORT_SHA256 = (
    "1081128ec04064ea89f2be7ab7677117e965afdbce1ed029ba39e683b8adfd8c"
)
DEFAULT_FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-092d-delete-test-call-cell-repair-fsv"
)
TARGET_CATEGORY = "delete_test_call"
TARGET_HOLDOUT_FAIL_TASK_ID = "sympy__sympy-18189"
INDEX_PATH = Path(
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json"
)
CORPUS_DIR = Path(
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511"
)
REPORT_NAME = "task_py_g_092d_delete_test_call_cell_repair_fsv.json"
READBACK_NAME = "task_py_g_092d_delete_test_call_cell_repair_readback.json"
TARGET_SIDE_PREFIXES = ("oracle:", "docker:", "test_phase:", "code_state_outcome:")
RUNTIME_HINT_TOKENS = ("q5", "shift", "collection", "runtime", "test_collection")
AIWONDER_PATH_PREFIX = "/zfs/"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_0600(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def require_aiwonder_path(path: Path, name: str, *, must_exist: bool) -> None:
    text = str(path)
    if "/mnt/d" in text:
        raise ValueError(f"{name} must not point at legacy /mnt/d: {path}")
    if not text.startswith(AIWONDER_PATH_PREFIX):
        raise ValueError(f"{name} must be an aiwonder /zfs path: {path}")
    if must_exist and not path.exists():
        raise FileNotFoundError(f"{name} does not exist: {path}")


def category_rows(report: dict[str, Any], key: str) -> list[dict[str, Any]]:
    rows = [row for row in report[key] if row["category"] == TARGET_CATEGORY]
    rows.sort(key=lambda row: row["row_index"])
    return rows


def active_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [row for row in rows if row["active_after_abstain"]]


def active_misses(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [row for row in active_rows(rows) if not row["correct_before_abstain"]]


def actual_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "pass": sum(1 for row in rows if row["actual_oracle_pass"]),
        "fail": sum(1 for row in rows if not row["actual_oracle_pass"]),
    }


def predicted_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "pass": sum(1 for row in rows if row["predicted_pass"]),
        "fail": sum(1 for row in rows if not row["predicted_pass"]),
    }


def first_evidence(row: dict[str, Any], needle: str) -> str | None:
    return next(
        (item for item in row.get("evidence_key_sample", []) if needle in item),
        None,
    )


def evidence_flags(row: dict[str, Any]) -> dict[str, bool]:
    evidence = row.get("evidence_key_sample", [])
    lower_evidence = [item.lower() for item in evidence]
    return {
        "has_skill_kind": any("skill_kind:" in item for item in evidence),
        "has_target_side_prefix": any(
            item.startswith(TARGET_SIDE_PREFIXES) for item in evidence
        ),
        "has_q5_or_runtime_hint": any(
            any(token in item for token in RUNTIME_HINT_TOKENS)
            for item in lower_evidence
        ),
        "sample_appears_complete": row.get("evidence_key_count", 0) <= len(evidence),
    }


def compact_row(row: dict[str, Any], index_by_task: dict[str, dict[str, Any]]) -> dict[str, Any]:
    entry = index_by_task.get(row["task_id"], {})
    patch_path = CORPUS_DIR / row["patch_path"]
    return {
        "row_index": row["row_index"],
        "task_id": row["task_id"],
        "repo": row["repo"],
        "actual_oracle_pass": row["actual_oracle_pass"],
        "predicted_pass": row["predicted_pass"],
        "active_after_abstain": row["active_after_abstain"],
        "score_in_abstain_band": row["score_in_abstain_band"],
        "predicted_oracle_pass": row["predicted_oracle_pass"],
        "correct_before_abstain": row["correct_before_abstain"],
        "ece_bin_index": row["ece_bin_index"],
        "cause_bin_score": row["cause_bin_score"],
        "patch_path": row["patch_path"],
        "patch_sha256": sha256_file(patch_path) if patch_path.exists() else None,
        "source_path": entry.get("source_path"),
        "source_site_note": entry.get("source_site_note"),
        "mutation_note": entry.get("mutation_note"),
        "skill_kind_evidence": first_evidence(row, "skill_kind:"),
        "slot_label_evidence": first_evidence(row, "|slot:"),
        "evidence_flags": evidence_flags(row),
    }


def parse_source_path(mutation_note: str) -> str | None:
    match = re.search(r"source_path=file:([^ ]+)", mutation_note)
    return match.group(1) if match else None


def parse_source_site_note(mutation_note: str) -> str | None:
    marker = "source_site_note=delete_test_call: replaced assertion beginning with `"
    if marker not in mutation_note:
        return None
    return mutation_note.split(marker, 1)[1].split("`", 1)[0]


def is_test_path(path: str | None) -> bool:
    if path is None:
        return False
    parts = Path(path).parts
    if any(part in {"test", "tests"} for part in parts):
        return True
    basename = parts[-1] if parts else path
    return basename in {"test.py", "tests.py"} or basename.startswith(
        "test_"
    ) or basename.endswith("_test.py")


def enrich_index_entries(index: dict[str, Any]) -> list[dict[str, Any]]:
    enriched = []
    for entry in index["entries"]:
        if entry["category"] != TARGET_CATEGORY:
            continue
        mutation_note = entry.get("mutation_note", "")
        source_path = parse_source_path(mutation_note)
        source_site_note = parse_source_site_note(mutation_note)
        enriched.append(
            {
                **entry,
                "source_path": source_path,
                "source_site_note": source_site_note,
                "source_parse_ok": source_path is not None
                and source_site_note is not None,
                "source_is_test_path": is_test_path(source_path),
            }
        )
    return enriched


def split_index_summary(entries: list[dict[str, Any]]) -> dict[str, Any]:
    summary: dict[str, Any] = {}
    for bucket in ("train", "calibration", "holdout"):
        split = [entry for entry in entries if entry["bucket"] == bucket]
        plain_assert = [
            entry for entry in split if entry["source_site_note"] == "assert "
        ]
        self_assert = [
            entry for entry in split if entry["source_site_note"] == "self.assert"
        ]
        non_test_plain_assert = [
            entry for entry in plain_assert if not entry["source_is_test_path"]
        ]
        summary[bucket] = {
            "total": len(split),
            "pass": sum(1 for entry in split if entry["oracle_all_passed"]),
            "fail": sum(1 for entry in split if not entry["oracle_all_passed"]),
            "plain_assert": pass_fail_counts(plain_assert),
            "self_assert": pass_fail_counts(self_assert),
            "non_test_plain_assert": pass_fail_counts(non_test_plain_assert),
        }
    return summary


def pass_fail_counts(entries: list[dict[str, Any]]) -> dict[str, int]:
    return {
        "total": len(entries),
        "pass": sum(1 for entry in entries if entry["oracle_all_passed"]),
        "fail": sum(1 for entry in entries if not entry["oracle_all_passed"]),
    }


def compact_index_entry(entry: dict[str, Any]) -> dict[str, Any]:
    return {
        "bucket": entry["bucket"],
        "task_id": entry["task_id"],
        "repo": entry["repo"],
        "oracle_all_passed": entry["oracle_all_passed"],
        "source_path": entry["source_path"],
        "source_site_note": entry["source_site_note"],
        "source_parse_ok": entry["source_parse_ok"],
        "source_is_test_path": entry["source_is_test_path"],
        "patch_path": entry["patch_path"],
        "patch_sha256": entry["patch_sha256"],
        "oracle_per_test_count": entry.get("oracle_per_test_count"),
    }


def evidence_token_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts = {token: 0 for token in RUNTIME_HINT_TOKENS}
    counts["target_side_prefix"] = 0
    for row in rows:
        for item in row.get("evidence_key_sample", []):
            item_lower = item.lower()
            for token in RUNTIME_HINT_TOKENS:
                if token in item_lower:
                    counts[token] += 1
            if item.startswith(TARGET_SIDE_PREFIXES):
                counts["target_side_prefix"] += 1
    return counts


def same_score_calibration_rows(
    calibration: list[dict[str, Any]], target_score: float, tolerance: float = 1e-9
) -> list[dict[str, Any]]:
    return [
        row
        for row in calibration
        if abs(float(row["predicted_oracle_pass"]) - target_score) <= tolerance
    ]


def parse_coverage(entries: list[dict[str, Any]]) -> dict[str, Any]:
    failures = [entry for entry in entries if not entry["source_parse_ok"]]
    return {
        "total": len(entries),
        "parsed": len(entries) - len(failures),
        "failed": len(failures),
        "failed_sample": [compact_index_entry(entry) for entry in failures[:8]],
    }


def main() -> int:
    started_at = datetime.now(timezone.utc)
    run_id = f"run-{started_at.strftime('%Y%m%dT%H%M%SZ')}"
    source_report = Path(os.environ.get("CG_TASK_PY_G_092D_SOURCE_REPORT", SOURCE_REPORT))
    fsv_root = Path(os.environ.get("CG_TASK_PY_G_092D_FSV_ROOT", DEFAULT_FSV_ROOT))

    require_aiwonder_path(source_report, "source report", must_exist=True)
    require_aiwonder_path(INDEX_PATH, "corpus index", must_exist=True)
    require_aiwonder_path(CORPUS_DIR, "corpus dir", must_exist=True)
    require_aiwonder_path(fsv_root, "FSV root", must_exist=False)

    report = load_json(source_report)
    index = load_json(INDEX_PATH)
    source_sha256 = sha256_file(source_report)
    if source_sha256 != EXPECTED_SOURCE_REPORT_SHA256:
        raise ValueError(
            "source report sha256 drifted: "
            f"expected {EXPECTED_SOURCE_REPORT_SHA256}, got {source_sha256}"
        )

    run_dir = fsv_root / run_id
    run_dir.mkdir(parents=True, exist_ok=False)
    calibration = category_rows(
        report, "score_sharpness_abstain_calibration_row_diagnostics"
    )
    holdout = category_rows(report, "score_sharpness_abstain_holdout_row_diagnostics")
    all_eval_rows = calibration + holdout
    index_entries = enrich_index_entries(index)
    index_by_task = {entry["task_id"]: entry for entry in index_entries}
    index_parse_coverage = parse_coverage(index_entries)

    class_support = report["feature_source"]["class_support_by_category"][TARGET_CATEGORY]
    before_metrics = report["legal_score_sharpness_abstain_band"]["holdout"][
        "per_category_metrics"
    ][TARGET_CATEGORY]
    abstain_model = report["legal_score_sharpness_abstain_band"]["model"]
    holdout_misses = active_misses(holdout)
    calibration_misses = active_misses(calibration)
    holdout_fail_rows = [row for row in holdout if not row["actual_oracle_pass"]]
    target_fail_rows = [
        row for row in holdout_fail_rows if row["task_id"] == TARGET_HOLDOUT_FAIL_TASK_ID
    ]
    target_score = (
        float(target_fail_rows[0]["predicted_oracle_pass"]) if target_fail_rows else -1.0
    )
    same_score_calibration = same_score_calibration_rows(calibration, target_score)
    train_fail_entries = [
        entry
        for entry in index_entries
        if entry["bucket"] == "train" and not entry["oracle_all_passed"]
    ]
    holdout_fail_entries = [
        entry
        for entry in index_entries
        if entry["bucket"] == "holdout" and not entry["oracle_all_passed"]
    ]

    edge_audits = {
        "source_report_sha256_matches_expected": True,
        "source_report_all_passed": report.get("all_passed") is True,
        "active_paths_do_not_include_mnt_d": all(
            "/mnt/d" not in path for path in report["active_input_paths"]
        ),
        "configured_paths_are_aiwonder_zfs": all(
            str(path).startswith(AIWONDER_PATH_PREFIX)
            and "/mnt/d" not in str(path)
            for path in (source_report, INDEX_PATH, CORPUS_DIR, fsv_root)
        ),
        "index_source_site_parse_coverage_complete": index_parse_coverage["failed"] == 0,
        "diagnostics_cover_calibration_and_holdout": len(calibration) == 30
        and len(holdout) == 30,
        "calibration_has_zero_failures_for_target_category": class_support[
            "calibration_fail"
        ]
        == 0,
        "holdout_has_exactly_one_target_failure": len(holdout_fail_rows) == 1
        and bool(target_fail_rows),
        "current_policy_predicts_every_holdout_row_pass": predicted_counts(holdout)
        == {"pass": 30, "fail": 0},
        "runtime_q5_test_collection_evidence_absent_from_eval_samples": all(
            not evidence_flags(row)["has_q5_or_runtime_hint"] for row in all_eval_rows
        ),
        "target_side_prefixes_absent_from_eval_samples": all(
            not evidence_flags(row)["has_target_side_prefix"] for row in all_eval_rows
        ),
    }

    simple_signal_tests = {
        "plain_assert_replaced_by_pass": split_index_summary(index_entries),
        "same_score_calibration_rows": {
            "target_score": target_score,
            "calibration_count": len(same_score_calibration),
            "actual": actual_counts(same_score_calibration),
            "rows": [
                compact_row(row, index_by_task) for row in same_score_calibration[:8]
            ],
        },
        "train_and_holdout_fail_examples": {
            "train": [compact_index_entry(entry) for entry in train_fail_entries],
            "holdout": [compact_index_entry(entry) for entry in holdout_fail_entries],
        },
    }

    edge_case_simulations = {
        "holdout_only_fail_flip_rejected": {
            "before": {
                "candidate": "flip sympy__sympy-18189 from pass to fail",
                "would_fix_holdout_false_positive": bool(target_fail_rows),
            },
            "after": {
                "accepted": False,
                "reason": "calibration has zero delete_test_call failures; promotion would tune on holdout",
            },
        },
        "plain_assert_rule_rejected": {
            "before": {
                "candidate": "fail any delete_test_call row replacing plain assert with pass",
                "train_non_test_plain_assert": simple_signal_tests[
                    "plain_assert_replaced_by_pass"
                ]["train"]["non_test_plain_assert"],
                "calibration_non_test_plain_assert": simple_signal_tests[
                    "plain_assert_replaced_by_pass"
                ]["calibration"]["non_test_plain_assert"],
            },
            "after": {
                "accepted": False,
                "reason": "signal is overwhelmingly pass-skewed and would create many false failures",
            },
        },
        "q5_runtime_signal_missing": {
            "before": {
                "candidate": "use test-collection or Q5 shift-log consequence evidence",
                "eval_evidence_token_counts": evidence_token_counts(all_eval_rows),
            },
            "after": {
                "accepted": False,
                "reason": "the current #392 row diagnostics expose no live-safe Q5/test-collection evidence for this cell",
            },
        },
        "target_side_labels_rejected": {
            "before": {
                "candidate": "use oracle/docker/test_phase labels directly",
                "excluded_prefixes": report["feature_source"]["target_label_prefixes_excluded"],
            },
            "after": {
                "accepted": False,
                "reason": "target-side labels are correctly excluded from prediction inputs",
            },
        },
    }

    residual_blocker = {
        "code": "TASK_PY_G_092D_NO_SPLIT_LEGAL_DELETE_TEST_CALL_FAILURE_SIGNAL",
        "message": (
            "delete_test_call::python has zero calibration failures and the broad "
            "AST/mutation-note signal is pass-skewed. The only holdout failure is "
            "sympy__sympy-18189, where a source assert was replaced by pass, but "
            "that simple signal matches many passing rows. A legal repair needs a "
            "new train/calibration-supported live-safe consequence feature, likely "
            "assertion-duty/context or Q5/test-collection/shift-log evidence."
        ),
    }

    all_passed = all(edge_audits.values())
    focused = {
        "schema_version": 1,
        "issue": 567,
        "run_id": run_id,
        "started_at": started_at.isoformat().replace("+00:00", "Z"),
        "all_passed": all_passed,
        "repair_promoted": False,
        "cell_repair_passed": False,
        "residual_blocker": residual_blocker,
        "source_report_path": str(source_report),
        "source_report_sha256": source_sha256,
        "expected_source_report_sha256": EXPECTED_SOURCE_REPORT_SHA256,
        "index_path": str(INDEX_PATH),
        "active_input_paths": report["active_input_paths"],
        "abstain_model": {
            "abstain_low": abstain_model["abstain_low"],
            "abstain_high": abstain_model["abstain_high"],
            "min_active_coverage": abstain_model["min_active_coverage"],
        },
        "before_after": {
            "policy": "unchanged because no split-legal repair exists",
            "before_holdout_delete_test_call_metrics": before_metrics,
            "after_holdout_delete_test_call_metrics": before_metrics,
            "calibration_active_misses_before": [
                compact_row(row, index_by_task) for row in calibration_misses
            ],
            "holdout_active_misses_before": [
                compact_row(row, index_by_task) for row in holdout_misses
            ],
            "holdout_active_misses_after": [
                compact_row(row, index_by_task) for row in holdout_misses
            ],
        },
        "support": {
            "class_support_by_category": class_support,
            "calibration_rows": len(calibration),
            "holdout_rows": len(holdout),
            "calibration_actual": actual_counts(calibration),
            "holdout_actual": actual_counts(holdout),
            "calibration_predicted": predicted_counts(calibration),
            "holdout_predicted": predicted_counts(holdout),
            "calibration_active_rows": len(active_rows(calibration)),
            "holdout_active_rows": len(active_rows(holdout)),
            "calibration_active_miss_count": len(calibration_misses),
            "holdout_active_miss_count": len(holdout_misses),
            "holdout_fail_rows": [
                compact_row(row, index_by_task) for row in holdout_fail_rows
            ],
        },
        "hypotheses": {
            "q5_shift_log_or_test_collection_evidence": {
                "verdict": "absent_from_current_feature_surface",
                "evidence_token_counts": evidence_token_counts(all_eval_rows),
            },
            "test_call_or_assertion_ast_deletion_feature": {
                "verdict": "present_but_not_discriminative",
                "evidence": simple_signal_tests["plain_assert_replaced_by_pass"],
            },
            "selector_or_abstain_threshold": {
                "verdict": "no_calibration_support",
                "evidence": simple_signal_tests["same_score_calibration_rows"],
            },
        },
        "simple_signal_tests": simple_signal_tests,
        "index_source_site_parse_coverage": index_parse_coverage,
        "edge_case_simulations": edge_case_simulations,
        "edge_audits": edge_audits,
    }

    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, focused)
    readback = load_json(report_path)
    readback_report = {
        "schema_version": 1,
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "readback_all_passed": readback["all_passed"],
        "readback_repair_promoted": readback["repair_promoted"],
        "readback_cell_repair_passed": readback["cell_repair_passed"],
        "readback_residual_blocker": readback["residual_blocker"],
        "readback_edge_audits": readback["edge_audits"],
        "readback_holdout_active_miss_count": readback["support"][
            "holdout_active_miss_count"
        ],
        "readback_calibration_fail_support": readback["support"][
            "class_support_by_category"
        ]["calibration_fail"],
    }
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, readback_report)
    print(
        json.dumps(
            {
                "report_path": str(report_path),
                "readback_path": str(readback_path),
                "report_sha256": readback_report["report_sha256"],
                "all_passed": all_passed,
                "repair_promoted": False,
                "residual_blocker": residual_blocker["code"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if all_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
