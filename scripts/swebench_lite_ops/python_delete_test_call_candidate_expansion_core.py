"""Shared helpers for TASK-PY-G-135E candidate expansion."""

from __future__ import annotations

import argparse
import json
import os
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable

from .core import SwebenchOpsError, sha256_file
from .python_delete_test_call_regeneration import has_mounted_drive_fragment


TASK_ID = "TASK-PY-G-135E"
ARTIFACT_KIND = "delete_test_call_candidate_expansion_plan"

DEFAULT_TARGETS: dict[tuple[str, str], int] = {
    ("holdout", "other_assertion_like_deleted"): 8,
    ("holdout", "production_plain_assert_deleted"): 8,
    ("holdout", "pytest_raises_deleted"): 4,
    ("holdout", "test_self_assert_deleted"): 3,
    ("calibration", "other_assertion_like_deleted"): 2,
    ("calibration", "production_plain_assert_deleted"): 6,
    ("calibration", "pytest_raises_deleted"): 2,
    ("calibration", "test_self_assert_deleted"): 6,
    ("train", "other_assertion_like_deleted"): 2,
    ("train", "production_plain_assert_deleted"): 4,
    ("train", "pytest_raises_deleted"): 2,
    ("train", "test_self_assert_deleted"): 4,
}

TARGET_ORDER = (
    ("holdout", "other_assertion_like_deleted"),
    ("holdout", "production_plain_assert_deleted"),
    ("holdout", "pytest_raises_deleted"),
    ("holdout", "test_self_assert_deleted"),
    ("calibration", "production_plain_assert_deleted"),
    ("calibration", "test_self_assert_deleted"),
    ("calibration", "other_assertion_like_deleted"),
    ("calibration", "pytest_raises_deleted"),
    ("train", "production_plain_assert_deleted"),
    ("train", "test_self_assert_deleted"),
    ("train", "other_assertion_like_deleted"),
    ("train", "pytest_raises_deleted"),
)


class CandidateExpansionError(SwebenchOpsError):
    def __init__(self, code: str, message: str, details: dict[str, Any] | None = None):
        super().__init__(
            code,
            message,
            "Inspect the candidate patches, existing oracle outcomes, and fixed expansion quotas before rerunning.",
            details or {},
        )


def validate_paths(args: argparse.Namespace) -> None:
    for name in (
        "candidate_patches",
        "existing_label_root",
        "existing_outcome_search_root",
        "next_action_report",
        "output_root",
        "readback_root",
        "evidence_root",
    ):
        path = getattr(args, name)
        if path is not None and has_mounted_drive_fragment(path):
            raise CandidateExpansionError(
                "DTC_EXPANSION_MOUNTED_PATH_REJECTED",
                f"{name} points at mounted-drive lineage: {path}",
                {"name": name, "path": str(path)},
            )
    for path in args.existing_outcomes_jsonl or []:
        if has_mounted_drive_fragment(path):
            raise CandidateExpansionError(
                "DTC_EXPANSION_MOUNTED_PATH_REJECTED",
                f"existing outcome path points at mounted-drive lineage: {path}",
                {"name": "existing_outcomes_jsonl", "path": str(path)},
            )
    if not args.candidate_patches.is_file():
        raise CandidateExpansionError(
            "DTC_EXPANSION_PATCHES_MISSING",
            f"candidate patches JSONL missing: {args.candidate_patches}",
            {"candidate_patches": str(args.candidate_patches)},
        )
    if not args.next_action_report.is_file():
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_MISSING",
            f"next-action report missing: {args.next_action_report}",
            {"next_action_report": str(args.next_action_report)},
        )


def target_quotas(overrides: list[str] | None) -> dict[tuple[str, str], int]:
    targets = dict(DEFAULT_TARGETS)
    for raw in overrides or []:
        if "=" not in raw or ":" not in raw.split("=", 1)[0]:
            raise CandidateExpansionError(
                "DTC_EXPANSION_BAD_TARGET",
                f"target override must be split:sublabel=count: {raw}",
                {"target": raw},
            )
        lhs, value_text = raw.split("=", 1)
        split, sublabel = lhs.split(":", 1)
        try:
            value = int(value_text)
        except ValueError as exc:
            raise CandidateExpansionError(
                "DTC_EXPANSION_BAD_TARGET_COUNT",
                f"target count is not an integer: {raw}",
                {"target": raw},
            ) from exc
        if value < 0:
            raise CandidateExpansionError(
                "DTC_EXPANSION_NEGATIVE_TARGET_COUNT",
                f"target count must be non-negative: {raw}",
                {"target": raw},
            )
        targets[(split.strip(), sublabel.strip())] = value
    return targets


def read_next_action(path: Path) -> dict[str, Any]:
    payload = read_json(path, "DTC_EXPANSION_NEXT_ACTION_BAD_JSON")
    if payload.get("recommended_action_kind") != "generate_more_split_stable_candidate_oracle_rows":
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_KIND_UNEXPECTED",
            "next-action report does not request more split-stable candidate oracle rows",
            {"path": str(path), "recommended_action_kind": payload.get("recommended_action_kind")},
        )
    if payload.get("holdout_tuning_used") is not False:
        raise CandidateExpansionError(
            "DTC_EXPANSION_NEXT_ACTION_HOLDOUT_TUNED",
            "next-action report is not holdout-clean",
            {"path": str(path), "holdout_tuning_used": payload.get("holdout_tuning_used")},
        )
    return payload


def load_candidate_patches(path: Path) -> list[dict[str, Any]]:
    rows = read_jsonl(path, "DTC_EXPANSION_PATCHES_BAD_JSONL")
    seen: set[str] = set()
    for line_number, row in enumerate(rows, start=1):
        candidate_id = require_string(row, "candidate_row_id", path, line_number)
        if candidate_id in seen:
            raise CandidateExpansionError(
                "DTC_EXPANSION_DUPLICATE_PATCH_ID",
                f"duplicate candidate_row_id in patch rows: {candidate_id}",
                {"path": str(path), "line_number": line_number, "candidate_row_id": candidate_id},
            )
        seen.add(candidate_id)
        for field in ("source_task_id", "stable_partition", "assertion_duty_sublabel", "source_path", "patch_path"):
            require_string(row, field, path, line_number)
        if row.get("oracle_status") != "ready":
            raise CandidateExpansionError(
                "DTC_EXPANSION_PATCH_NOT_READY",
                f"candidate patch row is not ready: {candidate_id}",
                {"path": str(path), "line_number": line_number, "oracle_status": row.get("oracle_status")},
            )
        for field in ("source_file", "patch_path"):
            value = Path(str(row.get(field)))
            if has_mounted_drive_fragment(value):
                raise CandidateExpansionError(
                    "DTC_EXPANSION_PATCH_MOUNTED_PATH",
                    f"{field} contains mounted-drive lineage",
                    {"candidate_row_id": candidate_id, "field": field, "value": str(value)},
                )
    return rows


def load_existing_candidate_ids(args: argparse.Namespace) -> tuple[set[str], dict[str, Any]]:
    ids: set[str] = set()
    sources: dict[str, Any] = {}
    label_path = args.existing_label_root / "candidate_code_state_constellations.jsonl"
    if label_path.is_file():
        label_rows = read_jsonl(label_path, "DTC_EXPANSION_EXISTING_LABELS_BAD_JSONL")
        label_ids = {str(row.get("candidate_row_id")) for row in label_rows if row.get("candidate_row_id")}
        ids.update(label_ids)
        sources["existing_label_rows"] = {"path": str(label_path), "count": len(label_ids), "sha256": sha256_file(label_path)}
    else:
        sources["existing_label_rows"] = {"path": str(label_path), "count": 0, "missing": True}

    outcome_paths = resolve_existing_outcome_paths(args)
    outcome_ids: set[str] = set()
    for path in outcome_paths:
        rows = read_jsonl(path, "DTC_EXPANSION_EXISTING_OUTCOMES_BAD_JSONL")
        outcome_ids.update(str(row.get("candidate_row_id")) for row in rows if row.get("candidate_row_id"))
    ids.update(outcome_ids)
    sources["existing_outcome_rows"] = {
        "paths": [str(path) for path in outcome_paths],
        "distinct_candidate_count": len(outcome_ids),
    }
    return ids, sources


def resolve_existing_outcome_paths(args: argparse.Namespace) -> list[Path]:
    paths = list(args.existing_outcomes_jsonl or [])
    if args.existing_outcome_search_root.is_dir():
        paths.extend(
            sorted(
                args.existing_outcome_search_root.glob(
                    "task-py-g-135c-delete-test-call-regeneration-v3-oracle-outcomes*/"
                    "delete_test_call_oracle_outcomes.jsonl"
                )
            )
        )
    unique: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path)
        if key in seen:
            continue
        if not path.is_file():
            raise CandidateExpansionError(
                "DTC_EXPANSION_EXISTING_OUTCOME_MISSING",
                f"existing outcome JSONL missing: {path}",
                {"path": str(path)},
            )
        unique.append(path)
        seen.add(key)
    return unique


def select_candidates(
    candidates: list[dict[str, Any]],
    existing_ids: set[str],
    targets: dict[tuple[str, str], int],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, int]], Counter[tuple[str, str]]]:
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    for row in candidates:
        if row["candidate_row_id"] in existing_ids:
            continue
        key = (str(row["stable_partition"]), str(row["assertion_duty_sublabel"]))
        if key in targets:
            grouped[key].append(row)
    for key in grouped:
        grouped[key].sort(key=selection_sort_key)

    selected: list[dict[str, Any]] = []
    shortfalls: dict[str, dict[str, int]] = {}
    available_counts: Counter[tuple[str, str]] = Counter({key: len(value) for key, value in grouped.items()})
    ordered_keys = list(TARGET_ORDER) + sorted(set(targets) - set(TARGET_ORDER))
    for key in ordered_keys:
        target = targets.get(key, 0)
        if target <= 0:
            continue
        available = grouped.get(key, [])
        take = min(target, len(available))
        selected.extend(available[:take])
        if take < target:
            shortfalls["|".join(key)] = {"target": target, "selected": take, "available": len(available)}
    return selected, shortfalls, available_counts


def selection_sort_key(row: dict[str, Any]) -> tuple[Any, ...]:
    return (
        str(row.get("source_task_id") or ""),
        str(row.get("source_path") or ""),
        int(row.get("source_site_effective_offset") or row.get("source_site_offset") or 0),
        str(row.get("candidate_row_id") or ""),
    )


def annotate_selected_rows(selected: list[dict[str, Any]]) -> list[dict[str, Any]]:
    ranks: Counter[tuple[str, str]] = Counter()
    out: list[dict[str, Any]] = []
    for ordinal, row in enumerate(selected):
        key = (str(row["stable_partition"]), str(row["assertion_duty_sublabel"]))
        ranks[key] += 1
        out.append(
            {
                "schema_version": 1,
                "task_id": TASK_ID,
                "artifact_kind": "delete_test_call_candidate_expansion_selected_row",
                "selection_ordinal": ordinal,
                "selection_rank_in_group": ranks[key],
                "selection_group": "|".join(key),
                "candidate_row_id": row["candidate_row_id"],
                "source_task_id": row["source_task_id"],
                "repo": row.get("repo"),
                "stable_partition": row["stable_partition"],
                "mutation_category": row.get("mutation_category"),
                "assertion_duty_sublabel": row["assertion_duty_sublabel"],
                "assertion_kind": row.get("assertion_kind"),
                "file_kind": row.get("file_kind"),
                "source_path": row["source_path"],
                "source_file": row.get("source_file"),
                "source_file_sha256": row.get("source_file_sha256"),
                "source_site_effective_offset": row.get("source_site_effective_offset"),
                "source_site_offset": row.get("source_site_offset"),
                "patch_path": row["patch_path"],
                "patch_sha256": row.get("patch_sha256"),
                "selection_policy": "fixed_split_sublabel_quota_no_oracle_label_selection_v1",
                "holdout_tuning_used": False,
                "target_oracle_label_used": False,
                "target_oracle_labels_used_as_features": False,
            }
        )
    return out


def build_batch_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    batches: list[list[dict[str, Any]]] = []
    batch_tasks: list[set[str]] = []
    for row in rows:
        batch_index = next((index for index, tasks in enumerate(batch_tasks) if row["source_task_id"] not in tasks), None)
        if batch_index is None:
            batch_index = len(batches)
            batches.append([])
            batch_tasks.append(set())
        batches[batch_index].append(row)
        batch_tasks[batch_index].add(row["source_task_id"])
        row["expansion_batch_index"] = batch_index
        row["expansion_batch_key"] = f"delete-test-call-135e-expansion-batch-{batch_index:04d}"

    return [
        {
            "schema_version": 1,
            "artifact_kind": "delete_test_call_oracle_batch",
            "batch_index": index,
            "batch_key": f"delete-test-call-135e-expansion-batch-{index:04d}",
            "prediction_count": len(batch),
            "source_task_ids": [row["source_task_id"] for row in batch],
            "candidate_row_ids": [row["candidate_row_id"] for row in batch],
            "patch_paths": [row["patch_path"] for row in batch],
            "selection_policy": "TASK-PY-G-135E fixed split/sublabel expansion",
        }
        for index, batch in enumerate(batches)
    ]


def validate_selection(
    selected_rows: list[dict[str, Any]],
    batch_rows: list[dict[str, Any]],
    shortfalls: dict[str, dict[str, int]],
) -> list[str]:
    failures: list[str] = []
    candidate_ids = [row["candidate_row_id"] for row in selected_rows]
    if not selected_rows:
        failures.append("DTC_EXPANSION_SELECTED_ROWS_EMPTY")
    if len(set(candidate_ids)) != len(candidate_ids):
        failures.append("DTC_EXPANSION_DUPLICATE_SELECTED_CANDIDATE")
    if shortfalls:
        failures.append("DTC_EXPANSION_TARGET_SHORTFALL")
    if duplicate_task_batch_count(batch_rows):
        failures.append("DTC_EXPANSION_BATCH_DUPLICATE_TASK")
    for row in selected_rows:
        for field in ("source_file", "patch_path"):
            value = row.get(field)
            if value and has_mounted_drive_fragment(Path(str(value))):
                failures.append("DTC_EXPANSION_MOUNTED_SELECTED_PATH")
                break
    return sorted(set(failures))


def read_json(path: Path, code: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CandidateExpansionError(code, f"invalid JSON: {path}", {"path": str(path), "error": str(exc)}) from exc
    if not isinstance(payload, dict):
        raise CandidateExpansionError(code, f"JSON root is not an object: {path}", {"path": str(path)})
    return payload


def read_jsonl(path: Path, code: str) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise CandidateExpansionError(
                    code,
                    f"invalid JSONL at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number, "error": str(exc)},
                ) from exc
            if not isinstance(row, dict):
                raise CandidateExpansionError(
                    code,
                    f"JSONL row is not an object at {path}:{line_number}",
                    {"path": str(path), "line_number": line_number},
                )
            rows.append(row)
    return rows


def require_string(row: dict[str, Any], key: str, path: Path, line_number: int) -> str:
    value = row.get(key)
    if not isinstance(value, str) or not value:
        raise CandidateExpansionError(
            "DTC_EXPANSION_REQUIRED_FIELD_MISSING",
            f"required string field missing: {key}",
            {"path": str(path), "line_number": line_number, "key": key},
        )
    return value


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    if path.read_text(encoding="utf-8") != text:
        raise CandidateExpansionError(
            "DTC_EXPANSION_TEXT_READBACK_MISMATCH",
            f"text readback mismatch at {path}",
            {"path": str(path)},
        )


def write_jsonl_checked(path: Path, rows: Iterable[dict[str, Any]]) -> None:
    write_text_checked(
        path,
        "".join(json.dumps(row, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n" for row in rows),
    )


def tree_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"exists": False, "path": str(path)}
    if path.is_file():
        return {"exists": True, "path": str(path), "kind": "file", "size_bytes": path.stat().st_size}
    files = [child for child in path.rglob("*") if child.is_file()]
    return {
        "exists": True,
        "path": str(path),
        "kind": "dir",
        "file_count": len(files),
        "total_size_bytes": sum(child.stat().st_size for child in files),
    }


def count_lines(path: Path) -> int:
    with path.open("r", encoding="utf-8") as handle:
        return sum(1 for _ in handle)


def duplicate_task_batch_count(batch_rows: list[dict[str, Any]]) -> int:
    count = 0
    for row in batch_rows:
        task_ids = list(row.get("source_task_ids") or [])
        if len(task_ids) != len(set(task_ids)):
            count += 1
    return count


def mounted_drive_hit_count(paths: list[Path]) -> int:
    count = 0
    for path in paths:
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        count += text.count("/mnt/c") + text.count("/mnt/d")
    return count


def target_map(counter: Counter[tuple[str, str]] | dict[tuple[str, str], int]) -> dict[str, int]:
    return {"|".join(key): int(value) for key, value in sorted(counter.items())}
