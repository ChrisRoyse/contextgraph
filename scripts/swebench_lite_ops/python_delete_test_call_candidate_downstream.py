"""Candidate-keyed downstream readback for TASK-PY-G-135C."""

from __future__ import annotations

import argparse
import json
import shutil
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, finite_float, sha256_file, utc_now, write_json_checked
from .smoke_intelligence_matrices import tree_state


TASK_ID = "TASK-PY-G-135C"
ARTIFACT_KIND = "delete_test_call_candidate_keyed_downstream_readback"

DEFAULT_LABEL_ROOT = Path("/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-labels-v1")
DEFAULT_BASELINE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-134-active-point-metric-repair-fsv/"
    "run-20260522T130250Z/task_py_g_134_active_point_metric_repair_fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path(
    "/zfs/archive/contextgraph/state/labels/task-py-g-135c-candidate-keyed-downstream-v1"
)
DEFAULT_READBACK_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-135c-candidate-keyed-downstream-fsv/manual-readback"
)

TARGET_SIDE_MARKERS = ("oracle", "docker", "test_outcome", "target_side", "ground_truth")


class CandidateDownstreamError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the candidate-keyed #615 label root and downstream consumer assumptions before rerunning #392/#611.",
            details or {},
        )


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--label-root", type=Path, default=DEFAULT_LABEL_ROOT)
    parser.add_argument("--baseline-report", type=Path, default=DEFAULT_BASELINE_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--readback-root", "--fsv-root", dest="readback_root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_READBACK_ROOT)
    parser.add_argument("--expected-candidate-count", type=int, default=52)
    parser.add_argument("--min-calibration-failures-per-sublabel", type=int, default=1)
    parser.add_argument("--clean-output", action="store_true")


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise CandidateDownstreamError(code, message, details)


def reject_mounted_drive(name: str, path: Path) -> None:
    text = str(path)
    if text.startswith("/mnt/c") or text.startswith("/mnt/d"):
        raise CandidateDownstreamError(
            "DTC_DOWNSTREAM_MOUNTED_DRIVE_PATH",
            f"{name} points at a mounted-drive path",
            {"name": name, "path": text},
        )


def read_json(path: Path, code: str) -> dict[str, Any]:
    if not path.is_file():
        raise CandidateDownstreamError(code, f"required JSON missing: {path}", {"path": str(path)})
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CandidateDownstreamError(
            "DTC_DOWNSTREAM_BAD_JSON",
            f"invalid JSON at {path}",
            {"path": str(path), "error": str(exc)},
        ) from exc
    require(isinstance(payload, dict), "DTC_DOWNSTREAM_JSON_OBJECT_REQUIRED", f"{path} must contain a JSON object")
    return payload


def iter_jsonl(path: Path, code: str) -> Iterable[dict[str, Any]]:
    if not path.is_file():
        raise CandidateDownstreamError(code, f"required JSONL missing: {path}", {"path": str(path)})
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, 1):
            if not line.strip():
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateDownstreamError(
                    "DTC_DOWNSTREAM_BAD_JSONL",
                    f"invalid JSONL at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            require(isinstance(value, dict), "DTC_DOWNSTREAM_JSONL_OBJECT_REQUIRED", f"{path}:{line_number} is not an object")
            yield value


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


def load_rows(label_root: Path) -> tuple[dict[str, Any], dict[str, Any], list[dict[str, Any]], list[dict[str, Any]]]:
    manifest = read_json(label_root / "candidate_keyed_label_rebuild_manifest.json", "DTC_DOWNSTREAM_MANIFEST_MISSING")
    report = read_json(label_root / "candidate_keyed_label_rebuild_report.json", "DTC_DOWNSTREAM_REPORT_MISSING")
    require(manifest.get("passes") is True, "DTC_DOWNSTREAM_MANIFEST_NOT_PASSING", "candidate label manifest is not passing")
    require(report.get("passes") is True, "DTC_DOWNSTREAM_REPORT_NOT_PASSING", "candidate label report is not passing")
    code_rows = list(
        iter_jsonl(
            label_root / "candidate_code_state_constellations.jsonl",
            "DTC_DOWNSTREAM_CODE_STATES_MISSING",
        )
    )
    failure_rows = list(
        iter_jsonl(
            label_root / "candidate_failure_evidence_sets.jsonl",
            "DTC_DOWNSTREAM_FAILURE_EVIDENCE_MISSING",
        )
    )
    return manifest, report, code_rows, failure_rows


def validate_code_rows(rows: list[dict[str, Any]], expected_count: int) -> list[str]:
    failures: list[str] = []
    candidate_ids = [str(row.get("candidate_row_id") or "") for row in rows]
    if expected_count > 0 and len(rows) != expected_count:
        failures.append("DTC_DOWNSTREAM_CANDIDATE_COUNT_MISMATCH")
    if len(set(candidate_ids)) != len(candidate_ids):
        failures.append("DTC_DOWNSTREAM_DUPLICATE_CANDIDATE_ROW_ID")
    required = ("candidate_row_id", "source_task_id", "mutation_category", "assertion_duty_sublabel", "partition")
    missing_required = sum(1 for row in rows for field in required if not row.get(field))
    if missing_required:
        failures.append("DTC_DOWNSTREAM_REQUIRED_FIELD_MISSING")
    if any(row.get("mutation_category") != "delete_test_call" for row in rows):
        failures.append("DTC_DOWNSTREAM_UNEXPECTED_MUTATION_CATEGORY")
    if target_leakage_count(rows):
        failures.append("DTC_DOWNSTREAM_TARGET_LABEL_IN_LIVE_INPUT")
    return failures


def target_leakage_count(rows: list[dict[str, Any]]) -> int:
    count = 0
    for row in rows:
        for label in row.get("live_predictor_labels") or []:
            label_text = str(label).lower()
            if any(marker in label_text for marker in TARGET_SIDE_MARKERS):
                count += 1
    return count


def split_support(rows: list[dict[str, Any]], min_calibration_failures: int) -> dict[str, Any]:
    counts: Counter[tuple[str, str, str]] = Counter()
    for row in rows:
        status = "pass" if bool(row.get("oracle_all_passed")) else "fail"
        counts[(str(row.get("partition")), str(row.get("assertion_duty_sublabel")), status)] += 1
    failure_bearing = sorted({sublabel for (_part, sublabel, status), value in counts.items() if status == "fail" and value})
    missing = []
    for sublabel in failure_bearing:
        if counts[("train", sublabel, "fail")] < 1:
            missing.append({"sublabel": sublabel, "partition": "train", "fail_count": counts[("train", sublabel, "fail")]})
        if counts[("calibration", sublabel, "fail")] < min_calibration_failures:
            missing.append(
                {
                    "sublabel": sublabel,
                    "partition": "calibration",
                    "fail_count": counts[("calibration", sublabel, "fail")],
                    "required": min_calibration_failures,
                }
            )
    return {
        "passes": not missing,
        "failure_bearing_sublabels": failure_bearing,
        "missing_train_or_calibration_failure_support": missing,
        "support_rows": [
            {"partition": part, "assertion_duty_sublabel": sublabel, "status": status, "count": count}
            for (part, sublabel, status), count in sorted(counts.items())
        ],
    }


def collapse_groups(rows: list[dict[str, Any]], fields: tuple[str, ...]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, ...], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[tuple(str(row.get(field) or "") for field in fields)].append(row)
    out = []
    for key, members in sorted(grouped.items()):
        out.append(
            {
                "key_fields": list(fields),
                "key": list(key),
                "candidate_count": len(members),
                "candidate_row_ids": sorted(str(row.get("candidate_row_id")) for row in members),
                "assertion_duty_sublabels": sorted({str(row.get("assertion_duty_sublabel")) for row in members}),
                "partitions": dict(sorted(Counter(str(row.get("partition")) for row in members).items())),
                "oracle_status_counts": dict(sorted(Counter("pass" if row.get("oracle_all_passed") else "fail" for row in members).items())),
                "would_collapse_candidate_identity": len(members) > 1,
            }
        )
    return out


def label_effect_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    labels = sorted({str(label) for row in rows for label in row.get("live_predictor_labels") or []})
    out = []
    for label in labels:
        present = [row for row in rows if label in (row.get("live_predictor_labels") or [])]
        absent = [row for row in rows if label not in (row.get("live_predictor_labels") or [])]
        present_fail = sum(1 for row in present if not row.get("oracle_all_passed"))
        absent_fail = sum(1 for row in absent if not row.get("oracle_all_passed"))
        present_rate = present_fail / len(present) if present else None
        absent_rate = absent_fail / len(absent) if absent else None
        out.append(
            {
                "label_id": label,
                "candidate_support": len(present),
                "candidate_fail_count": present_fail,
                "candidate_pass_count": len(present) - present_fail,
                "candidate_fail_rate": finite_float(present_rate) if present_rate is not None else None,
                "absent_candidate_count": len(absent),
                "absent_fail_rate": finite_float(absent_rate) if absent_rate is not None else None,
                "fail_rate_delta_present_minus_absent": finite_float(present_rate - absent_rate)
                if present_rate is not None and absent_rate is not None
                else None,
                "sublabel_counts": dict(sorted(Counter(str(row.get("assertion_duty_sublabel")) for row in present).items())),
                "partition_counts": dict(sorted(Counter(str(row.get("partition")) for row in present).items())),
            }
        )
    out.sort(key=lambda row: (-(row["candidate_support"]), row["label_id"]))
    return out


def baseline_summary(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"present": False, "path": str(path)}
    payload = read_json(path, "DTC_DOWNSTREAM_BASELINE_BAD_JSON")
    before = (payload.get("before_current_active_392") or {}).get("delete_test_call") or {}
    edge = payload.get("edge_cases") or {}
    old_support = edge.get("delete_test_call_has_no_calibration_failure_support") or {}
    return {
        "present": True,
        "path": str(path),
        "sha256": sha256_file(path),
        "repair_promoted": payload.get("repair_promoted"),
        "typed_blocker": payload.get("typed_blocker"),
        "decision": payload.get("decision"),
        "old_delete_test_call_metric": {
            "correlation": before.get("correlation"),
            "brier": before.get("brier"),
            "ece": before.get("ece"),
            "actual_fail_count": before.get("actual_fail_count"),
            "actual_pass_count": before.get("actual_pass_count"),
            "active_rows": before.get("active_rows"),
            "point_metric_passed": before.get("point_metric_passed"),
        },
        "old_calibration_failure_support": old_support.get("actual"),
    }


def legacy_consumer_compatibility(label_root: Path, rows: list[dict[str, Any]]) -> dict[str, Any]:
    task_category_groups = collapse_groups(rows, ("source_task_id", "mutation_category"))
    task_category_unique = len(task_category_groups)
    panel_b_indexable = sum(1 for row in rows if (row.get("task_instance_id") or row.get("task_id")) and row.get("mutation_category"))
    pair_surface_loadable = sum(1 for row in rows if row.get("task_instance_id") and row.get("workspace_state"))
    return {
        "python_pair_surface_mining": {
            "status": "incompatible_without_candidate_adapter",
            "reason": "consumer expects task_instance_id/workspace_state and paired base/target rows",
            "candidate_rows": len(rows),
            "rows_with_task_instance_id_and_workspace_state": pair_surface_loadable,
            "default_filename_present": (label_root / "code_state_constellations.jsonl").is_file(),
        },
        "panel_b_model_scores": {
            "status": "incompatible_without_candidate_adapter",
            "reason": "consumer indexes labels by (task_id, category) and candidate root uses candidate-prefixed files",
            "candidate_rows": len(rows),
            "rows_indexable_by_existing_loader_if_renamed": panel_b_indexable,
            "naive_task_category_unique": task_category_unique,
            "candidate_identity_rows_lost_by_naive_key": len(rows) - task_category_unique,
            "default_code_state_filename_present": (label_root / "code_state_constellations.jsonl").is_file(),
            "default_failure_filename_present": (label_root / "failure_evidence_sets.jsonl").is_file(),
        },
        "label_bridge_rs": {
            "status": "incompatible_without_candidate_bridge_manifest",
            "reason": "Rust bridge expects learning_bridge_manifest.json and non-candidate path names",
            "learning_bridge_manifest_present": (label_root / "learning_bridge_manifest.json").is_file(),
            "candidate_manifest_present": (label_root / "candidate_keyed_label_rebuild_manifest.json").is_file(),
        },
    }


def command_python_delete_test_call_candidate_downstream(args: argparse.Namespace) -> dict[str, Any]:
    started_at = utc_now()
    for name, path in (
        ("label_root", args.label_root),
        ("baseline_report", args.baseline_report),
        ("output_root", args.output_root),
        ("readback_root", args.readback_root),
    ):
        reject_mounted_drive(name, path)
    require(args.expected_candidate_count >= 0, "DTC_DOWNSTREAM_EXPECTED_COUNT_INVALID", "--expected-candidate-count must be >= 0")
    require(
        args.min_calibration_failures_per_sublabel > 0,
        "DTC_DOWNSTREAM_MIN_CALIBRATION_SUPPORT_INVALID",
        "--min-calibration-failures-per-sublabel must be positive",
    )

    before = {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)}
    if args.clean_output and args.output_root.exists():
        shutil.rmtree(args.output_root)
    args.output_root.mkdir(parents=True, exist_ok=True)
    args.readback_root.mkdir(parents=True, exist_ok=True)

    manifest, label_report, code_rows, failure_rows = load_rows(args.label_root)
    validation_failures = validate_code_rows(code_rows, args.expected_candidate_count)
    support = split_support(code_rows, args.min_calibration_failures_per_sublabel)
    if not support["passes"]:
        validation_failures.append("DTC_DOWNSTREAM_SPLIT_SUPPORT_GATE_FAILED")

    row_path = args.output_root / "candidate_downstream_rows.jsonl"
    collapse_task_path = args.output_root / "candidate_collapse_groups_task_category.jsonl"
    collapse_sublabel_path = args.output_root / "candidate_collapse_groups_task_category_sublabel.jsonl"
    effects_path = args.output_root / "candidate_live_label_effects.jsonl"
    report_path = args.output_root / "candidate_keyed_downstream_report.json"
    readback_path = args.readback_root / "candidate_keyed_downstream_readback.json"

    downstream_rows = [
        {
            "candidate_row_id": row.get("candidate_row_id"),
            "source_task_id": row.get("source_task_id"),
            "mutation_category": row.get("mutation_category"),
            "assertion_duty_sublabel": row.get("assertion_duty_sublabel"),
            "partition": row.get("partition"),
            "oracle_all_passed": bool(row.get("oracle_all_passed")),
            "workspace_state": row.get("workspace_state"),
            "repo": row.get("repo"),
            "chunk_count": row.get("chunk_count"),
            "live_predictor_label_count": len(row.get("live_predictor_labels") or []),
            "target_supervision_label_count": len(row.get("target_supervision_labels") or []),
            "failure_evidence_present": False,
            "target_oracle_labels_used_as_features": bool(row.get("target_oracle_labels_used_as_features")),
            "slot_identity_preserved": bool(row.get("slot_identity_preserved")),
            "flat_vector_concat_used": bool(row.get("flat_vector_concat_used")),
        }
        for row in code_rows
    ]
    failure_ids = {str(row.get("candidate_row_id")) for row in failure_rows}
    for row in downstream_rows:
        row["failure_evidence_present"] = str(row.get("candidate_row_id")) in failure_ids

    task_groups = collapse_groups(code_rows, ("source_task_id", "mutation_category"))
    sublabel_groups = collapse_groups(code_rows, ("source_task_id", "mutation_category", "assertion_duty_sublabel"))
    effects = label_effect_rows(code_rows)
    line_counts = {
        "candidate_downstream_rows": write_jsonl(row_path, downstream_rows),
        "candidate_collapse_groups_task_category": write_jsonl(collapse_task_path, task_groups),
        "candidate_collapse_groups_task_category_sublabel": write_jsonl(collapse_sublabel_path, sublabel_groups),
        "candidate_live_label_effects": write_jsonl(effects_path, effects),
    }
    stable_hashes = {
        "candidate_downstream_rows": sha256_file(row_path),
        "candidate_collapse_groups_task_category": sha256_file(collapse_task_path),
        "candidate_collapse_groups_task_category_sublabel": sha256_file(collapse_sublabel_path),
        "candidate_live_label_effects": sha256_file(effects_path),
    }

    baseline = baseline_summary(args.baseline_report)
    calibration_failures = sum(
        item["count"]
        for item in support["support_rows"]
        if item["partition"] == "calibration" and item["status"] == "fail"
    )
    task_category_unique = len(task_groups)
    compatibility = legacy_consumer_compatibility(args.label_root, code_rows)
    movement = {
        "old_delete_test_call_calibration_failures": baseline.get("old_calibration_failure_support"),
        "new_candidate_keyed_calibration_failures": calibration_failures,
        "old_no_calibration_failure_blocker_falsified": baseline.get("old_calibration_failure_support") == 0
        and calibration_failures > 0,
        "candidate_identity_rows": len(code_rows),
        "naive_task_category_unique_rows": task_category_unique,
        "candidate_identity_rows_lost_by_naive_task_category_key": len(code_rows) - task_category_unique,
        "old_metric": baseline.get("old_delete_test_call_metric"),
        "new_metric": None,
        "metric_rerun_status": "not_rerun_by_this_readback",
        "next_required_action": "Rerun #392/#611 with a candidate-keyed adapter over candidate_row_id plus delete_test_call::<assertion_duty_sublabel>.",
    }
    summary = {
        "candidate_count": len(code_rows),
        "failure_evidence_rows": len(failure_rows),
        "distinct_candidate_ids": len({str(row.get("candidate_row_id")) for row in code_rows}),
        "naive_task_category_unique_rows": task_category_unique,
        "naive_task_category_sublabel_unique_rows": len(sublabel_groups),
        "candidate_identity_rows_lost_by_naive_task_category_key": len(code_rows) - task_category_unique,
        "target_leakage_live_label_count": target_leakage_count(code_rows),
        "split_support_gate": support,
        "validation_failures": validation_failures,
        "movement": movement,
        "legacy_consumer_compatibility": compatibility,
        "policy": {
            "selection_split": "none",
            "holdout_tuning_used": False,
            "candidate_row_id_keyed": True,
            "category_key_policy": "delete_test_call::<assertion_duty_sublabel>",
            "target_labels_are_supervision_only": True,
            "slot_identity_preserved": True,
        },
    }
    passes = not validation_failures and len(code_rows) > task_category_unique and support["passes"]
    report = {
        "artifact_kind": f"{ARTIFACT_KIND}_report",
        "schema_version": 1,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "passes": passes,
        "source_of_truth": {
            "label_root": str(args.label_root),
            "baseline_report": str(args.baseline_report),
            "output_root": str(args.output_root),
            "readback_root": str(args.readback_root),
        },
        "input_hashes": {
            "candidate_keyed_manifest": sha256_file(args.label_root / "candidate_keyed_label_rebuild_manifest.json"),
            "candidate_keyed_report": sha256_file(args.label_root / "candidate_keyed_label_rebuild_report.json"),
            "candidate_code_state_constellations": sha256_file(args.label_root / "candidate_code_state_constellations.jsonl"),
            "candidate_failure_evidence_sets": sha256_file(args.label_root / "candidate_failure_evidence_sets.jsonl"),
            "baseline_report": sha256_file(args.baseline_report) if args.baseline_report.is_file() else None,
        },
        "label_manifest_line_counts": manifest.get("line_counts"),
        "label_report_summary": (label_report.get("summary") or {}),
        "summary": summary,
        "baseline": baseline,
        "output_artifacts": {
            "candidate_downstream_rows": str(row_path),
            "candidate_collapse_groups_task_category": str(collapse_task_path),
            "candidate_collapse_groups_task_category_sublabel": str(collapse_sublabel_path),
            "candidate_live_label_effects": str(effects_path),
        },
        "line_counts": line_counts,
        "stable_hashes": stable_hashes,
    }
    write_json_checked(report_path, report)
    readback = {
        "artifact_kind": f"{ARTIFACT_KIND}_readback",
        "schema_version": 1,
        "task_id": TASK_ID,
        "started_at_utc": started_at,
        "completed_at_utc": utc_now(),
        "passes": passes and read_json(report_path, "DTC_DOWNSTREAM_REPORT_READBACK_MISSING").get("passes") is True,
        "before": before,
        "after": {"output_root": tree_state(args.output_root), "readback_root": tree_state(args.readback_root)},
        "report_path": str(report_path),
        "report_sha256": sha256_file(report_path),
        "line_counts": line_counts,
        "stable_hashes": stable_hashes,
        "summary": summary,
    }
    write_json_checked(readback_path, readback)
    return {
        "ok": passes,
        "all_passed": passes,
        "command": "python-delete-test-call-candidate-downstream",
        "checked_at": utc_now(),
        "report_path": str(report_path),
        "readback_path": str(readback_path),
        "report_sha256": sha256_file(report_path),
        "readback_sha256": sha256_file(readback_path),
        "summary": summary,
    }
