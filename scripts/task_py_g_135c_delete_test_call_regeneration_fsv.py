#!/usr/bin/env python3
"""TASK-PY-G-135C delete_test_call regeneration feasibility FSV.

The current 300x8 corpus has only three delete_test_call oracle failures and
zero calibration failures. This harness does not tune a predictor. It reads the
current corpus index plus canonical aiwonder source files and classifies the
existing category into assertion-duty subfamilies, then emits a deterministic
candidate plan for new oracle rows.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import re
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


TASK_ID = "TASK-PY-G-135C"
TARGET_CATEGORY = "delete_test_call"
INDEX_PATH = Path(
    os.environ.get(
        "CG_SWEBENCH_INDEX",
        "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json",
    )
)
SOURCE_ROOT = Path(
    os.environ.get(
        "CG_AST_SOURCE_ROOT",
        "/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/source-files-300x8-20260515T013205Z",
    )
)
FSV_ROOT = Path(os.environ.get("CG_FSV", "/zfs/archive/contextgraph/fsv")) / "task-py-g-135c-delete-test-call-regeneration-fsv"
PLAN_ROOT = Path(
    os.environ.get(
        "CG_DELETE_TEST_CALL_REGEN_PLAN_ROOT",
        "/zfs/archive/contextgraph/state/regeneration-plans/task-py-g-135c-delete-test-call-regeneration-v1",
    )
)
REPORT_NAME = "task_py_g_135c_delete_test_call_regeneration_fsv.json"
READBACK_NAME = "task_py_g_135c_delete_test_call_regeneration_readback.json"
PLAN_NAME = "delete_test_call_regeneration_candidates.jsonl"
PLAN_MANIFEST_NAME = "delete_test_call_regeneration_manifest.json"

ASSERT_PREFIXES = (
    ("with pytest.raises", True),
    ("pytest.raises", False),
    ("pytest.assume", False),
    ("self.assert", False),
    ("assert ", False),
    ("assert(", False),
)


def utc_stamp() -> str:
    return dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")


def utc_now_iso() -> str:
    return dt.datetime.now(dt.UTC).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_text(payload: Any) -> str:
    return hashlib.sha256(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")).hexdigest()


def read_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json_0600(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    os.chmod(path, 0o600)


def write_jsonl_0600(path: Path, rows: list[dict[str, Any]]) -> None:
    with path.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True, separators=(",", ":")))
            handle.write("\n")
    os.chmod(path, 0o600)


def stable_partition(value: str) -> str:
    bucket = int(hashlib.sha256(value.encode("utf-8")).hexdigest()[:8], 16) % 10
    if bucket == 0:
        return "holdout"
    if bucket == 1:
        return "calibration"
    return "train"


def parse_mutation_note(note: str) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for key in (
        "category",
        "repo",
        "base_commit",
        "source_path",
        "candidate_scope",
        "patch_builder",
        "source_site_offset",
        "source_site_len",
    ):
        match = re.search(rf"(?:^|\s){re.escape(key)}=([^\s]+)", note)
        if match:
            out[key] = match.group(1)
    if "source_site_note=" in note:
        full_note = note.split("source_site_note=", 1)[1].strip()
        out["source_site_note_full"] = full_note
        prefix_match = re.search(r"replaced assertion beginning with `([^`]*)`", full_note)
        out["source_site_prefix"] = prefix_match.group(1) if prefix_match else full_note
    source_path = str(out.get("source_path") or "")
    out["source_relative_path"] = source_path[5:] if source_path.startswith("file:") else source_path
    try:
        out["source_site_offset_int"] = int(out.get("source_site_offset") or -1)
    except ValueError:
        out["source_site_offset_int"] = -1
    try:
        out["source_site_len_int"] = int(out.get("source_site_len") or 0)
    except ValueError:
        out["source_site_len_int"] = 0
    return out


def file_kind(source_path: str) -> str:
    path = source_path.lower()
    parts = [part for part in Path(path).parts if part]
    name = Path(path).name
    if any(part in {"test", "tests", "testing"} for part in parts):
        return "test"
    if name.startswith("test_") or name.endswith("_test.py") or "/test_" in path:
        return "test"
    if any(part in {"docs", "doc"} for part in parts):
        return "docs"
    if any(part in {"config", "configs", "settings"} for part in parts):
        return "config"
    return "source"


def assertion_kind(source_site: str) -> str:
    text = source_site.strip()
    if text.startswith("with pytest.raises") or text.startswith("pytest.raises"):
        return "pytest_raises"
    if text.startswith("pytest.assume"):
        return "pytest_assume"
    if text.startswith("self.assert"):
        return "self_assert"
    if text.startswith("assert"):
        return "plain_assert"
    return "other_assertion_like"


def sublabel_for(kind: str, fkind: str) -> str:
    if kind == "plain_assert" and fkind == "source":
        return "production_plain_assert_deleted"
    if kind == "self_assert":
        return "test_self_assert_deleted"
    if kind == "pytest_raises":
        return "pytest_raises_deleted"
    return "other_assertion_like_deleted"


def source_path_for_row(task_id: str, relative_path: str) -> Path:
    return SOURCE_ROOT / task_id / "base" / relative_path


def matched_prefix(line: str) -> tuple[str, bool] | None:
    stripped = line.lstrip()
    for prefix, removes_block in ASSERT_PREFIXES:
        if not stripped.startswith(prefix):
            continue
        if prefix == "self.assert":
            rest = stripped[len(prefix) :]
            if not rest or not rest[0].isalpha() or "(" not in rest:
                continue
        return prefix, removes_block
    return None


def line_offsets(text: str):
    start = 0
    for line in text.splitlines(keepends=True):
        end = start + len(line)
        yield start, end, line
        start = end
    if not text.endswith("\n") and start == 0:
        yield 0, len(text), text


def find_assertion_candidates(text: str) -> list[dict[str, Any]]:
    candidates: list[dict[str, Any]] = []
    for start, end, line in line_offsets(text):
        prefix = matched_prefix(line.rstrip("\n"))
        if prefix is None:
            continue
        line_text = line.rstrip("\n")
        kind = assertion_kind(line_text.lstrip())
        candidates.append(
            {
                "source_site_offset": start,
                "source_site_len": len(line),
                "source_site_preview": line_text.strip()[:160],
                "matched_prefix": prefix[0],
                "removes_context_block": prefix[1],
                "assertion_kind": kind,
            }
        )
    return candidates


def compact_existing_row(row: dict[str, Any], parsed: dict[str, Any]) -> dict[str, Any]:
    return {
        "bucket": row.get("bucket") or row.get("split"),
        "task_id": row.get("task_id"),
        "repo": row.get("repo"),
        "oracle_all_passed": row.get("oracle_all_passed"),
        "oracle_per_test_count": row.get("oracle_per_test_count"),
        "patch_path": row.get("patch_path"),
        "patch_sha256": row.get("patch_sha256"),
        "source_path": parsed.get("source_relative_path"),
        "source_site_offset": parsed.get("source_site_offset_int"),
        "source_site_len": parsed.get("source_site_len_int"),
        "source_site_note": parsed.get("source_site_note_full"),
        "source_site_prefix": parsed.get("source_site_prefix"),
    }


def load_delete_rows() -> list[dict[str, Any]]:
    payload = read_json(INDEX_PATH)
    entries = payload.get("entries") if isinstance(payload, dict) else payload
    rows = [row for row in entries if row.get("category") == TARGET_CATEGORY]
    rows.sort(key=lambda row: (str(row.get("bucket") or row.get("split")), str(row.get("task_id")), str(row.get("patch_sha256"))))
    return rows


def classify_existing(rows: list[dict[str, Any]]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    counts: dict[str, dict[str, Counter[str]]] = defaultdict(lambda: defaultdict(Counter))
    examples: dict[str, dict[str, Any]] = {}
    parsed_rows: list[dict[str, Any]] = []
    source_readback_failures: list[dict[str, Any]] = []
    selected_slice_mismatches: list[dict[str, Any]] = []
    for row in rows:
        parsed = parse_mutation_note(str(row.get("mutation_note") or ""))
        source_path = str(parsed.get("source_relative_path") or "")
        site = str(parsed.get("source_site_prefix") or parsed.get("source_site_note_full") or "")
        fkind = file_kind(source_path)
        akind = assertion_kind(site)
        sublabel = sublabel_for(akind, fkind)
        bucket = str(row.get("bucket") or row.get("split") or "unknown")
        outcome = "pass" if bool(row.get("oracle_all_passed")) else "fail"
        counts[sublabel][bucket][outcome] += 1
        counts[sublabel]["all"][outcome] += 1
        examples.setdefault(sublabel, compact_existing_row(row, parsed))
        source_file = source_path_for_row(str(row.get("task_id") or ""), source_path)
        source_exists = source_file.exists()
        source_sha256 = sha256_file(source_file) if source_exists else None
        selected_slice_matches = False
        if source_exists:
            text = source_file.read_text(encoding="utf-8", errors="replace")
            offset = int(parsed.get("source_site_offset_int") or -1)
            length = int(parsed.get("source_site_len_int") or 0)
            selected = text[offset : offset + length] if offset >= 0 and length > 0 else ""
            selected_slice_matches = selected.lstrip().startswith(site.strip())
            if not selected_slice_matches and len(selected_slice_mismatches) < 12:
                selected_slice_mismatches.append(
                    {
                        "task_id": row.get("task_id"),
                        "source_path": source_path,
                        "expected_prefix": site,
                        "actual_slice": selected[:160],
                    }
                )
        elif len(source_readback_failures) < 12:
            source_readback_failures.append({"task_id": row.get("task_id"), "source_path": source_path, "path": str(source_file)})
        parsed_rows.append(
            {
                **compact_existing_row(row, parsed),
                "file_kind": fkind,
                "assertion_kind": akind,
                "assertion_duty_sublabel": sublabel,
                "outcome": outcome,
                "source_file_exists": source_exists,
                "source_file_sha256": source_sha256,
                "selected_slice_matches": selected_slice_matches,
            }
        )
    normalized_counts = {
        sublabel: {
            bucket: {"pass": counter.get("pass", 0), "fail": counter.get("fail", 0), "total": counter.get("pass", 0) + counter.get("fail", 0)}
            for bucket, counter in sorted(bucket_map.items())
        }
        for sublabel, bucket_map in sorted(counts.items())
    }
    return (
        {
            "row_count": len(rows),
            "counts_by_sublabel_bucket_outcome": normalized_counts,
            "examples_by_sublabel": examples,
            "source_readback_failures": source_readback_failures,
            "selected_slice_mismatches": selected_slice_mismatches,
            "selected_slice_alignment_policy": "diagnostic_only_historical_offsets_may_target_mutation_state",
            "source_readback_all_existing_rows": not source_readback_failures,
        },
        parsed_rows,
    )


def build_regeneration_plan(parsed_rows: list[dict[str, Any]]) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    plan_rows: list[dict[str, Any]] = []
    source_file_cache: dict[Path, tuple[str, str, list[dict[str, Any]]]] = {}
    current_selected_keys = {
        (
            str(row["task_id"]),
            str(row["source_path"]),
            int(row["source_site_offset"] or -1),
        )
        for row in parsed_rows
    }
    current_fail_source_files = {
        (str(row["task_id"]), str(row["source_path"]))
        for row in parsed_rows
        if row["outcome"] == "fail"
    }
    counts_by_split: dict[str, Counter[str]] = defaultdict(Counter)
    counts_by_sublabel: Counter[str] = Counter()
    failure_source_candidate_counts: Counter[str] = Counter()
    readback_failures: list[dict[str, Any]] = []
    for row in parsed_rows:
        task_id = str(row["task_id"])
        source_path = str(row["source_path"])
        source_file = source_path_for_row(task_id, source_path)
        if source_file not in source_file_cache:
            if not source_file.exists():
                readback_failures.append({"task_id": task_id, "source_path": source_path, "path": str(source_file)})
                continue
            text = source_file.read_text(encoding="utf-8", errors="replace")
            source_file_cache[source_file] = (text, sha256_file(source_file), find_assertion_candidates(text))
        _, source_sha256, candidates = source_file_cache[source_file]
        for candidate_index, candidate in enumerate(candidates):
            selected_key = (task_id, source_path, int(candidate["source_site_offset"]))
            if selected_key in current_selected_keys:
                continue
            fkind = file_kind(source_path)
            sublabel = sublabel_for(str(candidate["assertion_kind"]), fkind)
            row_id_payload = {
                "task_id": task_id,
                "source_path": source_path,
                "source_sha256": source_sha256,
                "source_site_offset": candidate["source_site_offset"],
                "source_site_len": candidate["source_site_len"],
                "assertion_duty_sublabel": sublabel,
                "task_id_seed": "task-py-g-135c-v1",
            }
            row_id = "sha256:" + sha256_text(row_id_payload)
            split = stable_partition(row_id)
            likely_failure_source = (task_id, source_path) in current_fail_source_files
            plan = {
                "schema_version": 1,
                "artifact_kind": "delete_test_call_regeneration_candidate",
                "task_id": TASK_ID,
                "candidate_row_id": row_id,
                "source_task_id": task_id,
                "repo": row["repo"],
                "source_path": source_path,
                "source_file_sha256": "sha256:" + source_sha256,
                "source_site_offset": candidate["source_site_offset"],
                "source_site_len": candidate["source_site_len"],
                "source_site_preview": candidate["source_site_preview"],
                "candidate_index_in_source_file": candidate_index,
                "file_kind": fkind,
                "assertion_kind": candidate["assertion_kind"],
                "assertion_duty_sublabel": sublabel,
                "stable_partition": split,
                "oracle_status": "not_run",
                "oracle_required": True,
                "selected_from_existing_failure_source_file": likely_failure_source,
                "expected_patch_builder": "git_blob_sparse",
                "active_path_policy": "aiwonder_source_root_only",
                "zero_vector_or_oracle_label_live_input_used": False,
            }
            plan_rows.append(plan)
            counts_by_split[split][sublabel] += 1
            counts_by_sublabel[sublabel] += 1
            if likely_failure_source:
                failure_source_candidate_counts[split] += 1
    plan_rows.sort(key=lambda row: (row["stable_partition"], row["source_task_id"], row["source_path"], row["source_site_offset"]))
    return (
        {
            "candidate_count": len(plan_rows),
            "counts_by_sublabel": dict(sorted(counts_by_sublabel.items())),
            "counts_by_split_sublabel": {split: dict(sorted(counter.items())) for split, counter in sorted(counts_by_split.items())},
            "failure_source_candidate_counts_by_split": dict(sorted(failure_source_candidate_counts.items())),
            "source_readback_failures": readback_failures[:16],
            "oracle_required_before_label_use": True,
            "regeneration_ready_for_oracle_batch": len(plan_rows) > 0 and not readback_failures,
        },
        plan_rows,
    )


def write_plan(run_dir: Path, plan_rows: list[dict[str, Any]], plan_summary: dict[str, Any]) -> dict[str, Any]:
    PLAN_ROOT.mkdir(parents=True, exist_ok=True)
    plan_path = PLAN_ROOT / PLAN_NAME
    run_plan_path = run_dir / PLAN_NAME
    write_jsonl_0600(plan_path, plan_rows)
    write_jsonl_0600(run_plan_path, plan_rows)
    manifest = {
        "schema_version": 1,
        "artifact_kind": "delete_test_call_regeneration_manifest",
        "task_id": TASK_ID,
        "created_at_utc": utc_now_iso(),
        "plan_path": str(plan_path),
        "plan_sha256": sha256_file(plan_path),
        "run_plan_path": str(run_plan_path),
        "run_plan_sha256": sha256_file(run_plan_path),
        **plan_summary,
    }
    write_json_0600(PLAN_ROOT / PLAN_MANIFEST_NAME, manifest)
    write_json_0600(run_dir / PLAN_MANIFEST_NAME, manifest)
    return manifest


def decision(existing: dict[str, Any], plan_manifest: dict[str, Any]) -> dict[str, Any]:
    counts = existing["counts_by_sublabel_bucket_outcome"]
    promoted_failure_bearing: list[str] = []
    missing_calibration_support: list[str] = []
    for sublabel, bucket_map in counts.items():
        total_fail = int((bucket_map.get("all") or {}).get("fail") or 0)
        if total_fail <= 0:
            continue
        promoted_failure_bearing.append(sublabel)
        calibration_fail = int((bucket_map.get("calibration") or {}).get("fail") or 0)
        if calibration_fail <= 0:
            missing_calibration_support.append(sublabel)
    return {
        "all_passed": bool(existing["source_readback_all_existing_rows"]) and bool(plan_manifest["regeneration_ready_for_oracle_batch"]),
        "existing_category_learnable_without_regeneration": len(missing_calibration_support) == 0 and bool(promoted_failure_bearing),
        "regeneration_complete": False,
        "regeneration_plan_materialized": bool(plan_manifest["regeneration_ready_for_oracle_batch"]),
        "oracle_batch_required": True,
        "failure_bearing_sublabels_observed": promoted_failure_bearing,
        "failure_bearing_sublabels_missing_calibration_support": missing_calibration_support,
        "typed_blockers": [
            "TASK_PY_G_135C_DELETE_TEST_CALL_ZERO_CALIBRATION_FAILURE_SUPPORT"
        ]
        if missing_calibration_support
        else [],
        "next_action": (
            "run Docker oracle on the deterministic regeneration candidate plan, then rebuild AST chunks/forward mask/labels and rerun #392/#611"
        ),
    }


def readback(report_path: Path, plan_manifest: dict[str, Any]) -> dict[str, Any]:
    plan_path = Path(str(plan_manifest["plan_path"]))
    lines = 0
    missing_oracle_required = 0
    mounted_path_rows = 0
    with plan_path.open("r", encoding="utf-8") as handle:
        for raw in handle:
            if not raw.strip():
                continue
            lines += 1
            row = json.loads(raw)
            if row.get("oracle_required") is not True or row.get("oracle_status") != "not_run":
                missing_oracle_required += 1
            if "/mnt/d" in json.dumps(row) or "/mnt/c" in json.dumps(row):
                mounted_path_rows += 1
    return {
        "artifact_kind": "delete_test_call_regeneration_readback",
        "task_id": TASK_ID,
        "report_path": str(report_path),
        "report_sha256": "sha256:" + sha256_file(report_path),
        "plan_path": str(plan_path),
        "plan_sha256": "sha256:" + sha256_file(plan_path),
        "plan_line_count": lines,
        "expected_plan_line_count": int(plan_manifest["candidate_count"]),
        "plan_line_count_matches_manifest": lines == int(plan_manifest["candidate_count"]),
        "bad_oracle_required_rows": missing_oracle_required,
        "mounted_path_rows": mounted_path_rows,
        "readback_passed": lines == int(plan_manifest["candidate_count"]) and missing_oracle_required == 0 and mounted_path_rows == 0,
    }


def main() -> int:
    run_dir = FSV_ROOT / f"run-{utc_stamp()}"
    run_dir.mkdir(parents=True, exist_ok=True)
    rows = load_delete_rows()
    existing, parsed_rows = classify_existing(rows)
    plan_summary, plan_rows = build_regeneration_plan(parsed_rows)
    plan_manifest = write_plan(run_dir, plan_rows, plan_summary)
    dec = decision(existing, plan_manifest)
    report = {
        "schema_version": 1,
        "artifact_kind": "task_py_g_135c_delete_test_call_regeneration_fsv",
        "task_id": TASK_ID,
        "created_at_utc": utc_now_iso(),
        "inputs": {
            "index_path": str(INDEX_PATH),
            "index_sha256": "sha256:" + sha256_file(INDEX_PATH),
            "source_root": str(SOURCE_ROOT),
            "fsv_run_dir": str(run_dir),
            "plan_root": str(PLAN_ROOT),
        },
        "existing_classification": existing,
        "regeneration_plan_manifest": plan_manifest,
        "decision": dec,
        "all_passed": bool(dec["all_passed"]),
    }
    report_path = run_dir / REPORT_NAME
    write_json_0600(report_path, report)
    rb = readback(report_path, plan_manifest)
    readback_path = run_dir / READBACK_NAME
    write_json_0600(readback_path, rb)
    report["readback"] = rb
    report["all_passed"] = bool(report["all_passed"]) and bool(rb["readback_passed"])
    write_json_0600(report_path, report)
    print(
        json.dumps(
            {
                "all_passed": report["all_passed"],
                "report_path": str(report_path),
                "report_sha256": "sha256:" + sha256_file(report_path),
                "readback_path": str(readback_path),
                "readback_sha256": "sha256:" + sha256_file(readback_path),
                "plan_path": plan_manifest["plan_path"],
                "plan_sha256": plan_manifest["plan_sha256"],
                "decision": report["decision"],
                "plan_summary": {
                    "candidate_count": plan_manifest["candidate_count"],
                    "counts_by_sublabel": plan_manifest["counts_by_sublabel"],
                    "failure_source_candidate_counts_by_split": plan_manifest["failure_source_candidate_counts_by_split"],
                },
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if report["all_passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
