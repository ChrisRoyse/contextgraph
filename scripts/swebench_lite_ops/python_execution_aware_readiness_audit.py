#!/usr/bin/env python3
"""Execution-aware feature readiness audit for INV-010 / #774."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
TASK_ID = "INV-010"
ARTIFACT_KIND = "execution_aware_readiness_audit"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/execution-aware-audit/inv-010-v1")
DEFAULT_TRACE_ROOT = None
ELIGIBLE_TRACE_KEYS = {
    "coverage",
    "execution_trace",
    "line_coverage",
    "runtime_metrics",
    "steps",
    "trace_events",
    "variable_states",
}
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "completed_ids",
    "completed_instances",
    "error_ids",
    "error_instances",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_verdict_sha256",
    "resolved_ids",
    "resolved_instances",
    "unresolved_ids",
    "unresolved_instances",
}
CONTESTED_MIXED_CELLS = {"off_by_one", "subtle_flip", "swap_variable"}


class ExecutionAwareReadinessAuditError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise ExecutionAwareReadinessAuditError(f"{code}{suffix}")


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "EXECUTION_AWARE_READINESS_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    tmp.replace(path)


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    count = 0
    with tmp.open("w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(canonical_json(row) + "\n")
            count += 1
    tmp.replace(path)
    return count


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, {"path": str(path)})
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ExecutionAwareReadinessAuditError(f"{code}: invalid JSON at {path}: {exc}") from exc


def _require_object(value: Any, label: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "EXECUTION_AWARE_READINESS_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    require(
        isinstance(value, list),
        "EXECUTION_AWARE_READINESS_BAD_INPUT",
        {"label": label, "actual_type": type(value).__name__},
    )
    return value


def coerce_prediction_row(value: Any, index: int, split: str) -> dict[str, Any]:
    require(
        isinstance(value, dict),
        "EXECUTION_AWARE_READINESS_ROW_NOT_OBJECT",
        {"split": split, "index": index, "actual_type": type(value).__name__},
    )
    task_id = str(value.get("task_id") or "")
    repo = str(value.get("repo") or "")
    category = str(value.get("category") or "")
    require(task_id, "EXECUTION_AWARE_READINESS_ROW_TASK_ID_MISSING", {"split": split, "index": index})
    require(repo, "EXECUTION_AWARE_READINESS_ROW_REPO_MISSING", {"split": split, "index": index})
    require(category, "EXECUTION_AWARE_READINESS_ROW_CATEGORY_MISSING", {"split": split, "index": index})
    actual = value.get("actual_oracle_pass")
    require(
        isinstance(actual, (bool, int, float)),
        "EXECUTION_AWARE_READINESS_ROW_LABEL_MISSING",
        {"split": split, "index": index, "task_id": task_id},
    )
    return {
        "row_index": index,
        "split": split,
        "task_id": task_id,
        "repo": repo,
        "category": category,
        "actual_oracle_pass": bool(actual),
    }


def raw_prediction_rows(execute: dict[str, Any], split_label: str) -> tuple[str, list[dict[str, Any]]]:
    for key in (
        f"{split_label}_raw_predictions",
        f"{split_label}_predictions",
        f"{split_label}_rank_lift_predictions",
    ):
        if key in execute and execute.get(key) is not None:
            rows = _require_array(execute.get(key), key)
            return key, [coerce_prediction_row(row, idx, split_label) for idx, row in enumerate(rows)]
    raise ExecutionAwareReadinessAuditError(
        f"EXECUTION_AWARE_READINESS_MISSING_ROWS: split={split_label}"
    )


def load_ship_gate_rows(path: Path) -> dict[str, Any]:
    payload = read_json(path, "EXECUTION_AWARE_READINESS_SHIP_GATE_REPORT_MISSING")
    execute = _require_object(payload.get("execute_and_inspect"), "execute_and_inspect")
    calibration_source, calibration_rows = raw_prediction_rows(execute, "calibration")
    holdout_source, holdout_rows = raw_prediction_rows(execute, "holdout")
    return {
        "calibration_source": calibration_source,
        "holdout_source": holdout_source,
        "calibration_rows": calibration_rows,
        "holdout_rows": holdout_rows,
    }


def load_corpus_index(corpus_root: Path, expected_rows: int | None) -> list[dict[str, Any]]:
    index_path = corpus_root / "index.json"
    doc = read_json(index_path, "EXECUTION_AWARE_READINESS_CORPUS_INDEX_MISSING")
    entries = _require_array(doc.get("entries"), "index.entries")
    if expected_rows is not None:
        require(
            len(entries) == expected_rows,
            "EXECUTION_AWARE_READINESS_CORPUS_ROW_COUNT_MISMATCH",
            {"expected": expected_rows, "actual": len(entries), "index_path": str(index_path)},
        )
    out: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for idx, raw in enumerate(entries):
        row = _require_object(raw, f"index.entries[{idx}]")
        task_id = str(row.get("task_id") or "")
        repo = str(row.get("repo") or "")
        category = str(row.get("category") or "")
        require(task_id, "EXECUTION_AWARE_READINESS_INDEX_TASK_ID_MISSING", {"index": idx})
        require(repo, "EXECUTION_AWARE_READINESS_INDEX_REPO_MISSING", {"index": idx, "task_id": task_id})
        require(category, "EXECUTION_AWARE_READINESS_INDEX_CATEGORY_MISSING", {"index": idx, "task_id": task_id})
        key = (task_id, category)
        require(
            key not in seen,
            "EXECUTION_AWARE_READINESS_INDEX_DUPLICATE_ROW",
            {"task_id": task_id, "category": category},
        )
        seen.add(key)
        out.append(
            {
                "row_index": idx,
                "task_id": task_id,
                "repo": repo,
                "category": category,
                "patch_path": str(row.get("patch_path") or ""),
                "oracle_all_passed_present": "oracle_all_passed" in row,
                "oracle_verdict_sha256_present": "oracle_verdict_sha256" in row,
                "oracle_per_test_count_present": "oracle_per_test_count" in row,
            }
        )
    return out


def file_kind(path: Path) -> str:
    lowered = path.name.lower()
    parts = {part.lower() for part in path.parts}
    if "swebench-runs" in parts and lowered.endswith(".json"):
        return "oracle_summary_json"
    if "swebench-runs" in parts and lowered.endswith(".jsonl") and lowered.startswith("predictions-"):
        return "oracle_prediction_patch_jsonl"
    if "swebench-verify-runs" in parts and lowered.endswith(".json"):
        return "oracle_verify_summary_json"
    if "swebench-verify-runs" in parts and lowered == "predictions.jsonl":
        return "oracle_verify_prediction_patch_jsonl"
    if any(token in lowered for token in ("trace", "coverage", "runtime")):
        return "candidate_dynamic_json"
    return "other"


def read_limited_jsonish(path: Path) -> Any | None:
    try:
        if path.suffix == ".json":
            return json.loads(path.read_text(encoding="utf-8"))
        if path.suffix == ".jsonl":
            with path.open("r", encoding="utf-8") as handle:
                for line in handle:
                    if line.strip():
                        return json.loads(line)
    except (OSError, json.JSONDecodeError):
        return None
    return None


def classify_dynamic_candidate(path: Path, payload: Any) -> dict[str, Any]:
    keys: set[str] = set(payload) if isinstance(payload, dict) else set()
    eligible_keys = sorted(keys & ELIGIBLE_TRACE_KEYS)
    target_side_keys = sorted(keys & TARGET_SIDE_KEYS)
    kind = file_kind(path)
    target_side_only = kind.startswith("oracle_") or (bool(target_side_keys) and not eligible_keys)
    eligible = kind == "candidate_dynamic_json" and bool(eligible_keys) and not target_side_only
    return {
        "path": str(path),
        "kind": kind,
        "top_level_keys": sorted(keys)[:40],
        "eligible_execution_feature_keys": eligible_keys,
        "target_side_keys": target_side_keys,
        "target_side_only": target_side_only,
        "eligible_live_execution_feature": eligible,
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
    }


def discover_artifacts(roots: list[Path], *, max_inventory: int) -> tuple[list[dict[str, Any]], dict[str, int]]:
    require(bool(roots), "EXECUTION_AWARE_READINESS_SCAN_ROOTS_EMPTY")
    inventory: list[dict[str, Any]] = []
    counts: Counter[str] = Counter()
    for root in roots:
        require(root.is_dir(), "EXECUTION_AWARE_READINESS_SCAN_ROOT_MISSING", {"root": str(root)})
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            kind = file_kind(path)
            if kind == "other":
                continue
            counts[kind] += 1
            if len(inventory) >= max_inventory:
                continue
            payload = read_limited_jsonish(path)
            item = classify_dynamic_candidate(path, payload)
            item["scan_root"] = str(root)
            inventory.append(item)
    return inventory, dict(sorted(counts.items()))


def mapped_trace_rows(entries: list[dict[str, Any]], inventory: list[dict[str, Any]]) -> dict[str, Any]:
    eligible_paths = [
        item["path"]
        for item in inventory
        if item.get("eligible_live_execution_feature") is True
    ]
    mapped: set[tuple[str, str]] = set()
    for row in entries:
        task_id = row["task_id"]
        category = row["category"]
        for path in eligible_paths:
            if task_id in path and category in path:
                mapped.add((task_id, category))
                break
    by_cell: Counter[str] = Counter(category for _, category in mapped)
    return {
        "eligible_execution_trace_rows": len(mapped),
        "eligible_execution_trace_rows_by_cell": dict(sorted(by_cell.items())),
        "eligible_execution_trace_keys": [
            {"task_id": task_id, "category": category}
            for task_id, category in sorted(mapped)
        ],
        "eligible_execution_trace_sample": [
            {"task_id": task_id, "category": category}
            for task_id, category in sorted(mapped)[:20]
        ],
    }


def holdout_coverage(holdout_rows: list[dict[str, Any]], mapped: dict[str, Any]) -> dict[str, Any]:
    mapped_pairs = {
        (item["task_id"], item["category"])
        for item in mapped.get("eligible_execution_trace_keys", [])
    }
    holdout_count = len(holdout_rows)
    contested = [row for row in holdout_rows if row["category"] in CONTESTED_MIXED_CELLS]
    holdout_mapped = sum((row["task_id"], row["category"]) in mapped_pairs for row in holdout_rows)
    contested_mapped = sum((row["task_id"], row["category"]) in mapped_pairs for row in contested)
    return {
        "holdout_rows": holdout_count,
        "holdout_rows_with_trace": holdout_mapped,
        "contested_mixed_holdout_rows": len(contested),
        "contested_mixed_holdout_rows_with_trace": contested_mapped,
        "contested_mixed_cells": sorted(CONTESTED_MIXED_CELLS),
    }


def write_artifact_hashes(root: Path, paths: list[Path]) -> Path:
    rows = []
    for path in sorted(paths):
        if path.is_file():
            rows.append({"path": str(path), "sha256": sha256_file(path), "bytes": path.stat().st_size})
    out = root / "artifact_hashes.json"
    write_json(out, {"artifact_count": len(rows), "artifacts": rows})
    return out


def build_manifest(
    *,
    ship_gate_report: Path,
    corpus_root: Path,
    trace_roots: list[Path],
    state_run_root: Path,
    run_id: str,
    ship_gate: dict[str, Any],
    entries: list[dict[str, Any]],
    inventory: list[dict[str, Any]],
    artifact_counts: dict[str, int],
    trace_mapping: dict[str, Any],
    trace_inventory_path: Path,
    missing_rows_path: Path,
) -> dict[str, Any]:
    rows_total = len(entries)
    trace_rows = int(trace_mapping["eligible_execution_trace_rows"])
    trace_fraction = (trace_rows / rows_total) if rows_total else 0.0
    ready = trace_fraction >= 0.95
    decision_reasons: list[str] = []
    if trace_rows == 0:
        decision_reasons.append("no_eligible_live_execution_trace_artifacts_found")
    if artifact_counts.get("oracle_summary_json", 0) or artifact_counts.get("oracle_verify_summary_json", 0):
        decision_reasons.append("only_target_side_oracle_artifacts_available_for_runtime_outcomes")
    if trace_fraction < 0.95:
        decision_reasons.append("trace_coverage_below_promotion_threshold")
    if not decision_reasons:
        decision_reasons.append("execution_trace_source_ready_for_training_probe")
    corpus_counts = Counter(row["category"] for row in entries)
    ship_gate_counts = Counter(row["category"] for row in ship_gate["holdout_rows"])
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "task_id": TASK_ID,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "ship_gate_report_path": str(ship_gate_report),
        "ship_gate_report_sha256": sha256_file(ship_gate_report),
        "corpus_root": str(corpus_root),
        "corpus_index_path": str(corpus_root / "index.json"),
        "corpus_index_sha256": sha256_file(corpus_root / "index.json"),
        "trace_scan_roots": [str(path) for path in trace_roots],
        "raw_prediction_sources": {
            "calibration": ship_gate["calibration_source"],
            "holdout": ship_gate["holdout_source"],
        },
        "row_counts": {
            "corpus_rows": rows_total,
            "calibration_rows": len(ship_gate["calibration_rows"]),
            "holdout_rows": len(ship_gate["holdout_rows"]),
            "eligible_execution_trace_rows": trace_rows,
        },
        "cell_counts": {
            "corpus": dict(sorted(corpus_counts.items())),
            "holdout": dict(sorted(ship_gate_counts.items())),
        },
        "artifact_inventory_counts": artifact_counts,
        "trace_coverage": {
            **trace_mapping,
            "eligible_execution_trace_fraction": (
                float(round(trace_fraction, 10)) if math.isfinite(trace_fraction) else None
            ),
            **holdout_coverage(ship_gate["holdout_rows"], trace_mapping),
        },
        "decision": {
            "execution_aware_pretraining_ready": ready,
            "trace_generation_required": not ready,
            "proxy_feature_lift_measured": False,
            "per_cell_oracle_correlation_lift": None,
            "promotion_allowed": False,
            "canonical_issue": "#774",
            "decision_reasons": decision_reasons,
        },
        "doctrine_policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "oracle_verdicts_classified_target_side_only": True,
            "predictor_weights_changed": False,
            "aiwonder_durable_root_required": True,
        },
        "minimum_trace_generation_contract": {
            "row_key": ["task_id", "category", "candidate_workspace_state"],
            "required_live_feature_fields_any": sorted(ELIGIBLE_TRACE_KEYS),
            "forbidden_live_feature_fields": sorted(TARGET_SIDE_KEYS),
            "required_rows": rows_total,
            "required_cells": sorted(corpus_counts),
            "source_root": "/zfs/archive/contextgraph/state/execution-traces/inv-010-v1/<run-id>/",
            "fsv_root": "/zfs/archive/contextgraph/fsv/inv-010-execution-aware-fsv/<run-id>/",
        },
        "artifacts": {
            "trace_inventory": str(trace_inventory_path),
            "missing_trace_rows": str(missing_rows_path),
            "state_run_root": str(state_run_root),
        },
    }


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    corpus_root = Path(args.corpus_root)
    trace_root_arg = getattr(args, "trace_root", DEFAULT_TRACE_ROOT)
    trace_roots = [Path(trace_root_arg)] if trace_root_arg else []
    ship_gate_report = Path(args.ship_gate_report)
    require_archive_path(output_root, "output_root", bool(args.allow_non_aiwonder_roots))
    require_archive_path(state_root, "state_root", bool(args.allow_non_aiwonder_roots))
    for trace_root in trace_roots:
        require_archive_path(trace_root, "trace_root", bool(args.allow_non_aiwonder_roots))
    run_id = str(args.run_id or f"run-{utc_stamp()}")
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    ship_gate = load_ship_gate_rows(ship_gate_report)
    entries = load_corpus_index(corpus_root, args.expected_corpus_rows)
    inventory, artifact_counts = discover_artifacts(
        [corpus_root, *trace_roots],
        max_inventory=int(args.max_inventory_rows),
    )
    trace_mapping = mapped_trace_rows(entries, inventory)
    trace_inventory_path = fsv_run_root / "execution_artifact_inventory.jsonl"
    missing_rows_path = fsv_run_root / "missing_execution_trace_rows.jsonl"
    write_jsonl(trace_inventory_path, inventory)
    mapped_pairs = {
        (item["task_id"], item["category"])
        for item in trace_mapping.get("eligible_execution_trace_keys", [])
    }
    missing_rows = [
        {"task_id": row["task_id"], "repo": row["repo"], "category": row["category"]}
        for row in entries
        if (row["task_id"], row["category"]) not in mapped_pairs
    ]
    write_jsonl(missing_rows_path, missing_rows)
    manifest = build_manifest(
        ship_gate_report=ship_gate_report,
        corpus_root=corpus_root,
        trace_roots=trace_roots,
        state_run_root=state_run_root,
        run_id=run_id,
        ship_gate=ship_gate,
        entries=entries,
        inventory=inventory,
        artifact_counts=artifact_counts,
        trace_mapping=trace_mapping,
        trace_inventory_path=trace_inventory_path,
        missing_rows_path=missing_rows_path,
    )
    manifest_path = fsv_run_root / "execution_aware_readiness_manifest.json"
    write_json(manifest_path, manifest)
    state_manifest_path = state_run_root / manifest_path.name
    state_inventory_path = state_run_root / trace_inventory_path.name
    state_missing_path = state_run_root / missing_rows_path.name
    write_json(state_manifest_path, manifest)
    write_jsonl(state_inventory_path, [json.loads(line) for line in trace_inventory_path.read_text(encoding="utf-8").splitlines()])
    write_jsonl(state_missing_path, [json.loads(line) for line in missing_rows_path.read_text(encoding="utf-8").splitlines()])
    hash_path = write_artifact_hashes(
        fsv_run_root,
        [manifest_path, trace_inventory_path, missing_rows_path, state_manifest_path, state_inventory_path, state_missing_path],
    )
    state_hash_path = state_run_root / hash_path.name
    write_json(state_hash_path, read_json(hash_path, "EXECUTION_AWARE_READINESS_HASH_JSON_INVALID"))
    return {
        "ok": True,
        "run_id": run_id,
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "manifest": str(manifest_path),
        "trace_inventory": str(trace_inventory_path),
        "missing_trace_rows": str(missing_rows_path),
        "artifact_hashes": str(hash_path),
        "decision": manifest["decision"],
        "trace_coverage": manifest["trace_coverage"],
        "artifact_inventory_counts": manifest["artifact_inventory_counts"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Audit execution-aware feature readiness for #774")
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument(
        "--trace-root",
        type=Path,
        default=DEFAULT_TRACE_ROOT,
        help="Optional execution-trace state root to scan in addition to the corpus root.",
    )
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--expected-corpus-rows", type=int, default=2400)
    parser.add_argument("--max-inventory-rows", type=int, default=500)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = run_audit(args)
    except ExecutionAwareReadinessAuditError as exc:
        print(json.dumps({"ok": False, "error": str(exc)}, indent=2, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
