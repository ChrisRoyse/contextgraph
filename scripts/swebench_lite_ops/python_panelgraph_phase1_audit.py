#!/usr/bin/env python3
"""BLINDSPOT-004 / #726 PanelGraph Phase 1 measurement audit.

This is a measurement-only runner. It audits whether the current SWE-bench Lite
Python artifacts expose enough multi-file / multi-chunk structure to justify the
schema-changing PanelGraph prototype, and it records the missing prerequisite if
per-chunk verdict rows are not available.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import shutil
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

try:
    from .python_diff_chunk_map_core import parse_unified_diff, overlap
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_diff_chunk_map_core import parse_unified_diff, overlap  # type: ignore


SCHEMA_VERSION = 1
TASK_ID = "BLINDSPOT-004"
ARTIFACT_KIND = "panelgraph_phase1_measurement"
FORMULA_VERSION = "panelgraph_phase1_v1"
AIWONDER_ARCHIVE_ROOT = "/zfs/archive/contextgraph/"
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_AST_CHUNKS = Path("/zfs/archive/contextgraph/corpus/python-ast-chunks-v1/python_ast_chunks.jsonl")
DEFAULT_SHIP_GATE_REPORT = Path(
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/"
    "run-20260521T132308Z/ship-gate-fsv.json"
)
DEFAULT_OUTPUT_ROOT = Path("/zfs/archive/contextgraph/fsv/blindspot-004-panelgraph-phase1-fsv")
DEFAULT_STATE_ROOT = Path("/zfs/archive/contextgraph/state/panelgraph-phase1/blindspot-004-v1")
TARGET_SIDE_KEYS = {
    "actual_oracle_pass",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_per_test_count",
    "oracle_verdict_sha256",
    "predicted_oracle_pass",
    "predicted_pass",
}


class PanelGraphPhase1Error(RuntimeError):
    pass


@dataclass(frozen=True)
class PredictionRow:
    split: str
    task_id: str
    category: str
    repo: str
    label: int
    score: float

    @property
    def key(self) -> tuple[str, str]:
        return (self.task_id, self.category)


@dataclass(frozen=True)
class AstIndexes:
    chunks_by_key: dict[tuple[str, str, str, str], list[dict[str, Any]]]
    state_counts: dict[tuple[str, str, str], dict[str, int]]
    legacy_mounted_field_hits: int
    row_count: int


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise PanelGraphPhase1Error(f"{code}{suffix}")


def reject_mounted_drive(label: str, value: str) -> None:
    require(
        not (value.startswith("/mnt/c/") or value.startswith("/mnt/d/")),
        "PANELGRAPH_PHASE1_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_archive_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    if allow_non_aiwonder_roots:
        return
    require(
        str(path).startswith(AIWONDER_ARCHIVE_ROOT),
        "PANELGRAPH_PHASE1_NON_AIWONDER_DURABLE_PATH",
        {label: str(path), "required_prefix": AIWONDER_ARCHIVE_ROOT},
    )
    reject_mounted_drive(label, str(path))


def read_json(path: Path, code: str) -> Any:
    require(path.is_file(), code, str(path))
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise PanelGraphPhase1Error(f"{code}: invalid JSON at {path}: {exc}") from exc


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


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


def workspace_state_for_category(category: str) -> str:
    return "test_patch" if category == "known_good" else f"mutation_{category}"


def safe_patch_path(corpus_root: Path, patch_path: str) -> Path:
    reject_mounted_drive("patch_path", patch_path)
    path = Path(patch_path)
    if path.is_absolute():
        require(str(path).startswith(str(corpus_root)), "PANELGRAPH_PHASE1_PATCH_OUTSIDE_CORPUS", str(path))
        return path
    return corpus_root / path


def load_index(corpus_root: Path) -> list[dict[str, Any]]:
    payload = read_json(corpus_root / "index.json", "PANELGRAPH_PHASE1_INDEX_MISSING")
    entries = payload.get("entries") if isinstance(payload, dict) else None
    require(isinstance(entries, list) and entries, "PANELGRAPH_PHASE1_INDEX_ENTRIES_MISSING")
    rows: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for idx, raw in enumerate(entries):
        require(isinstance(raw, dict), "PANELGRAPH_PHASE1_INDEX_ROW_NOT_OBJECT", idx)
        task_id = required_string(raw, "task_id", idx)
        category = required_string(raw, "category", idx)
        repo = required_string(raw, "repo", idx)
        bucket = required_string(raw, "bucket", idx)
        patch_path = required_string(raw, "patch_path", idx)
        key = (task_id, category)
        require(key not in seen, "PANELGRAPH_PHASE1_DUPLICATE_INDEX_ROW", {"task_id": task_id, "category": category})
        seen.add(key)
        rows.append(
            {
                "row_index": idx,
                "task_id": task_id,
                "category": category,
                "repo": repo,
                "bucket": bucket,
                "patch_path": patch_path,
                "workspace_state": workspace_state_for_category(category),
            }
        )
    return rows


def required_string(row: dict[str, Any], key: str, index: int) -> str:
    value = row.get(key)
    require(isinstance(value, str) and value, "PANELGRAPH_PHASE1_REQUIRED_STRING", {"key": key, "index": index})
    reject_mounted_drive(key, value)
    return value


def load_ship_gate_rows(path: Path) -> dict[tuple[str, str], PredictionRow]:
    payload = read_json(path, "PANELGRAPH_PHASE1_SHIP_GATE_REPORT_MISSING")
    execute = payload.get("execute_and_inspect")
    require(isinstance(execute, dict), "PANELGRAPH_PHASE1_SHIP_GATE_EXECUTE_MISSING")
    out: dict[tuple[str, str], PredictionRow] = {}
    for split, key in (("calibration", "calibration_predictions"), ("holdout", "holdout_predictions")):
        raw_rows = execute.get(key)
        require(isinstance(raw_rows, list) and raw_rows, "PANELGRAPH_PHASE1_SHIP_GATE_SPLIT_MISSING", split)
        for index, raw in enumerate(raw_rows):
            require(isinstance(raw, dict), "PANELGRAPH_PHASE1_SHIP_GATE_ROW_NOT_OBJECT", {"split": split, "index": index})
            task_id = required_string(raw, "task_id", index)
            category = required_string(raw, "category", index)
            repo = required_string(raw, "repo", index)
            label = raw.get("actual_oracle_pass")
            score = raw.get("predicted_oracle_pass")
            require(isinstance(label, bool), "PANELGRAPH_PHASE1_SHIP_GATE_LABEL_MISSING", {"split": split, "index": index})
            require(isinstance(score, (int, float)) and math.isfinite(float(score)), "PANELGRAPH_PHASE1_SHIP_GATE_SCORE_MISSING", {"split": split, "index": index})
            row = PredictionRow(split=split, task_id=task_id, category=category, repo=repo, label=1 if label else 0, score=float(score))
            require(row.key not in out, "PANELGRAPH_PHASE1_DUPLICATE_SHIP_GATE_ROW", {"task_id": task_id, "category": category})
            out[row.key] = row
    return out


def load_ast_indexes(path: Path) -> AstIndexes:
    require(path.is_file(), "PANELGRAPH_PHASE1_AST_CHUNKS_MISSING", str(path))
    chunks: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    state_counts: dict[tuple[str, str, str], dict[str, int]] = defaultdict(lambda: {"chunks": 0, "files": 0})
    state_files: dict[tuple[str, str, str], set[str]] = defaultdict(set)
    legacy_hits = 0
    row_count = 0
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise PanelGraphPhase1Error(f"PANELGRAPH_PHASE1_AST_CHUNK_JSON_ERROR: {path}:{line_no}: {exc}") from exc
            require(isinstance(row, dict), "PANELGRAPH_PHASE1_AST_CHUNK_ROW_NOT_OBJECT", line_no)
            task_id = str(row.get("task_instance_id") or "")
            state = str(row.get("workspace_state") or "")
            category = str(row.get("mutation_category") or "")
            relative_path = str(row.get("relative_path") or "")
            chunk_id = str(row.get("chunk_id") or "")
            require(task_id and state and category and relative_path and chunk_id, "PANELGRAPH_PHASE1_AST_CHUNK_KEY_MISSING", line_no)
            minimal = {
                "chunk_id": chunk_id,
                "chunk_kind": str(row.get("chunk_kind") or "unknown"),
                "name": row.get("name") if isinstance(row.get("name"), str) else None,
                "qualname": row.get("qualname") if isinstance(row.get("qualname"), str) else None,
                "line_start": int(row.get("line_start") or 0),
                "line_end": int(row.get("line_end") or 0),
                "depth": int(row.get("depth") or 0),
            }
            chunks[(task_id, state, category, relative_path)].append(minimal)
            state_key = (task_id, state, category)
            state_counts[state_key]["chunks"] += 1
            state_files[state_key].add(relative_path)
            row_count += 1
            for key in ("legacy_dataschema_path", "legacy_provenance_path"):
                value = row.get(key)
                if isinstance(value, str) and ("/mnt/c/" in value or "/mnt/d/" in value):
                    legacy_hits += 1
    for chunk_list in chunks.values():
        chunk_list.sort(key=lambda item: (item["line_start"], item["line_end"], item["chunk_kind"], item["chunk_id"]))
    for key, files in state_files.items():
        state_counts[key]["files"] = len(files)
    return AstIndexes(chunks_by_key=dict(chunks), state_counts=dict(state_counts), legacy_mounted_field_hits=legacy_hits, row_count=row_count)


def map_patch_to_chunks(entry: dict[str, Any], corpus_root: Path, indexes: AstIndexes) -> dict[str, Any]:
    patch_file = safe_patch_path(corpus_root, entry["patch_path"])
    require(patch_file.is_file(), "PANELGRAPH_PHASE1_PATCH_FILE_MISSING", str(patch_file))
    patch_text = patch_file.read_text(encoding="utf-8")
    hunks = parse_unified_diff(patch_text)
    touched_files: set[str] = set()
    primary_chunk_ids: set[str] = set()
    primary_chunk_kinds: Counter[str] = Counter()
    target_sides: Counter[str] = Counter()
    change_kinds: Counter[str] = Counter()
    unmapped_hunks: list[dict[str, Any]] = []
    for hunk in hunks:
        target_path = hunk.new_path or hunk.old_path
        if target_path:
            touched_files.add(target_path)
        old_key = (entry["task_id"], "base", "known_good", hunk.old_path or "")
        new_key = (entry["task_id"], entry["workspace_state"], entry["category"], hunk.new_path or "")
        old_reference_lines = hunk.old_changed or tuple(line_number for line_number, _ in hunk.old_expected)
        old_chunks = overlap(indexes.chunks_by_key.get(old_key, []), old_reference_lines)
        new_chunks = overlap(indexes.chunks_by_key.get(new_key, []), hunk.new_changed)
        primary = new_chunks if new_chunks else old_chunks
        target_side = "new" if new_chunks else "old" if old_chunks else "unmapped"
        change_kind = "add" if not hunk.old_changed else "delete" if not hunk.new_changed else "modify"
        target_sides[target_side] += 1
        change_kinds[change_kind] += 1
        if not primary:
            unmapped_hunks.append(
                {
                    "hunk_index": hunk.index,
                    "file": target_path,
                    "change_kind": change_kind,
                    "old_line_count": len(hunk.old_changed),
                    "new_line_count": len(hunk.new_changed),
                }
            )
            continue
        for chunk in primary:
            primary_chunk_ids.add(str(chunk["chunk_id"]))
            primary_chunk_kinds[str(chunk["chunk_kind"])] += 1
    state_key = (entry["task_id"], entry["workspace_state"], entry["category"])
    state_counts = indexes.state_counts.get(state_key, {"chunks": 0, "files": 0})
    live_features = {
        "patch_hunk_count": len(hunks),
        "patch_touched_file_count": len(touched_files),
        "patch_touched_chunk_count": len(primary_chunk_ids),
        "candidate_state_chunk_count": int(state_counts["chunks"]),
        "candidate_state_file_count": int(state_counts["files"]),
        "mapped_hunk_count": len(hunks) - len(unmapped_hunks),
        "unmapped_hunk_count": len(unmapped_hunks),
    }
    return {
        "live_features": live_features,
        "shape_class": shape_class(live_features),
        "target_side_counts": dict(sorted(target_sides.items())),
        "change_kind_counts": dict(sorted(change_kinds.items())),
        "primary_chunk_kind_counts": dict(sorted(primary_chunk_kinds.items())),
        "sample_touched_files": sorted(touched_files)[:8],
        "sample_primary_chunk_ids": sorted(primary_chunk_ids)[:8],
        "sample_unmapped_hunks": unmapped_hunks[:8],
        "patch_sha256": sha256_file(patch_file),
    }


def shape_class(features: dict[str, int]) -> str:
    multi_file = features["patch_touched_file_count"] >= 2
    multi_chunk = features["patch_touched_chunk_count"] >= 2
    if multi_file and multi_chunk:
        return "multi_file_multi_chunk"
    if multi_file:
        return "multi_file_single_or_zero_chunk"
    if multi_chunk:
        return "single_file_multi_chunk"
    return "single_file_single_or_zero_chunk"


def build_rows(
    index_rows: list[dict[str, Any]],
    corpus_root: Path,
    ast_indexes: AstIndexes,
    predictions: dict[tuple[str, str], PredictionRow],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], dict[str, Any]]:
    live_rows: list[dict[str, Any]] = []
    eval_rows: list[dict[str, Any]] = []
    mapping_errors: list[dict[str, Any]] = []
    for entry in index_rows:
        try:
            mapped = map_patch_to_chunks(entry, corpus_root, ast_indexes)
        except PanelGraphPhase1Error as exc:
            mapped = {
                "live_features": {
                    "patch_hunk_count": 0,
                    "patch_touched_file_count": 0,
                    "patch_touched_chunk_count": 0,
                    "candidate_state_chunk_count": 0,
                    "candidate_state_file_count": 0,
                    "mapped_hunk_count": 0,
                    "unmapped_hunk_count": 1,
                },
                "shape_class": "unmapped",
                "target_side_counts": {},
                "change_kind_counts": {},
                "primary_chunk_kind_counts": {},
                "sample_touched_files": [],
                "sample_primary_chunk_ids": [],
                "sample_unmapped_hunks": [{"error": str(exc)[:240]}],
                "patch_sha256": None,
            }
            mapping_errors.append({"task_id": entry["task_id"], "category": entry["category"], "error": str(exc)[:240]})
        live = {
            "schema_version": SCHEMA_VERSION,
            "artifact_kind": "panelgraph_phase1_live_shape_row",
            "formula_version": FORMULA_VERSION,
            "task_id": entry["task_id"],
            "repo": entry["repo"],
            "category": entry["category"],
            "bucket": entry["bucket"],
            "workspace_state": entry["workspace_state"],
            "shape_class": mapped["shape_class"],
            "live_features": mapped["live_features"],
            "live_feature_keys": sorted(mapped["live_features"].keys()),
            "target_side_labels_excluded_from_live_features": True,
            "patch_sha256": mapped["patch_sha256"],
            "target_side_counts": mapped["target_side_counts"],
            "change_kind_counts": mapped["change_kind_counts"],
            "primary_chunk_kind_counts": mapped["primary_chunk_kind_counts"],
            "sample_touched_files": mapped["sample_touched_files"],
            "sample_primary_chunk_ids": mapped["sample_primary_chunk_ids"],
            "sample_unmapped_hunks": mapped["sample_unmapped_hunks"],
        }
        forbidden_live_keys = TARGET_SIDE_KEYS.intersection(live["live_features"])
        require(not forbidden_live_keys, "PANELGRAPH_PHASE1_TARGET_KEYS_IN_LIVE_FEATURES", sorted(forbidden_live_keys))
        live_rows.append(live)
        prediction = predictions.get((entry["task_id"], entry["category"]))
        if prediction is not None:
            eval_rows.append(
                {
                    "schema_version": SCHEMA_VERSION,
                    "artifact_kind": "panelgraph_phase1_offline_eval_row",
                    "formula_version": FORMULA_VERSION,
                    "task_id": entry["task_id"],
                    "repo": entry["repo"],
                    "category": entry["category"],
                    "split": prediction.split,
                    "shape_class": mapped["shape_class"],
                    "patch_touched_file_count": mapped["live_features"]["patch_touched_file_count"],
                    "patch_touched_chunk_count": mapped["live_features"]["patch_touched_chunk_count"],
                    "candidate_state_chunk_count": mapped["live_features"]["candidate_state_chunk_count"],
                    "predicted_oracle_pass": prediction.score,
                    "actual_oracle_pass": bool(prediction.label),
                    "absolute_error": abs(prediction.score - prediction.label),
                    "offline_eval_labels_not_live_inputs": True,
                }
            )
    return live_rows, eval_rows, {"mapping_errors": mapping_errors}


def summarize(live_rows: list[dict[str, Any]], eval_rows: list[dict[str, Any]], ast_indexes: AstIndexes) -> dict[str, Any]:
    shape_counts = Counter(row["shape_class"] for row in live_rows)
    split_shape_counts = Counter(f"{row['split']}::{row['shape_class']}" for row in eval_rows)
    multi_file_rows = [row for row in live_rows if row["live_features"]["patch_touched_file_count"] >= 2]
    multi_chunk_rows = [row for row in live_rows if row["live_features"]["patch_touched_chunk_count"] >= 2]
    eval_by_shape: dict[str, dict[str, Any]] = {}
    for shape in sorted({row["shape_class"] for row in eval_rows}):
        rows = [row for row in eval_rows if row["shape_class"] == shape]
        errors = [float(row["absolute_error"]) for row in rows]
        labels = [1 if row["actual_oracle_pass"] else 0 for row in rows]
        scores = [float(row["predicted_oracle_pass"]) for row in rows]
        eval_by_shape[shape] = {
            "row_count": len(rows),
            "mean_absolute_error": mean(errors),
            "pearson": pearson(scores, labels),
        }
    single_file_eval = [row for row in eval_rows if int(row["patch_touched_file_count"]) <= 1]
    multi_file_eval = [row for row in eval_rows if int(row["patch_touched_file_count"]) >= 2]
    error_gap = None
    if single_file_eval and multi_file_eval:
        error_gap = mean([float(row["absolute_error"]) for row in multi_file_eval]) - mean([float(row["absolute_error"]) for row in single_file_eval])
    mapped_rows = [row for row in live_rows if int(row["live_features"]["mapped_hunk_count"]) > 0]
    support_summary = {
        "corpus_rows": len(live_rows),
        "split_eval_rows": len(eval_rows),
        "ast_chunk_rows": ast_indexes.row_count,
        "shape_counts": dict(sorted(shape_counts.items())),
        "split_shape_counts": dict(sorted(split_shape_counts.items())),
        "multi_file_corpus_rows": len(multi_file_rows),
        "multi_chunk_corpus_rows": len(multi_chunk_rows),
        "multi_file_split_eval_rows": len(multi_file_eval),
        "mapping_coverage_fraction": len(mapped_rows) / len(live_rows) if live_rows else 0.0,
        "input_legacy_mounted_drive_field_hits": ast_indexes.legacy_mounted_field_hits,
    }
    return {
        "support_summary": support_summary,
        "offline_eval_by_shape": eval_by_shape,
        "multi_file_vs_single_file_mean_absolute_error_delta": error_gap,
    }


def mean(values: list[float]) -> float | None:
    if not values:
        return None
    return sum(values) / len(values)


def pearson(scores: list[float], labels: list[int]) -> float | None:
    if len(scores) < 2 or len(scores) != len(labels):
        return None
    mean_x = sum(scores) / len(scores)
    mean_y = sum(labels) / len(labels)
    var_x = sum((value - mean_x) ** 2 for value in scores)
    var_y = sum((value - mean_y) ** 2 for value in labels)
    if var_x <= 0.0 or var_y <= 0.0:
        return None
    cov = sum((x - mean_x) * (y - mean_y) for x, y in zip(scores, labels))
    return cov / math.sqrt(var_x * var_y)


def decide(summary: dict[str, Any], *, min_multi_file_rows: int, min_mapping_coverage: float) -> dict[str, Any]:
    support = summary["support_summary"]
    blockers: list[str] = []
    if support["mapping_coverage_fraction"] < min_mapping_coverage:
        blockers.append("patch_to_chunk_mapping_coverage_below_threshold")
    if support["multi_file_split_eval_rows"] < min_multi_file_rows:
        blockers.append("insufficient_multi_file_split_eval_rows")
    blockers.append("per_chunk_verdict_rows_unavailable")
    return {
        "phase1_measured": True,
        "panelgraph_support_present": support["multi_file_corpus_rows"] > 0 and support["multi_chunk_corpus_rows"] > 0,
        "chunk_aggregation_discrepancy_measured": False,
        "full_panelgraph_schema_change_recommended": False,
        "full_panelgraph_schema_change_allowed": False,
        "next_required_artifact": "per_chunk_verdict_rows_joined_by_task_category_chunk_id",
        "phase2_blockers": blockers,
    }


def artifact_hashes(path: Path, artifacts: dict[str, Path]) -> dict[str, str]:
    hashes = {name: sha256_file(artifact) for name, artifact in sorted(artifacts.items())}
    write_json(path, hashes)
    return hashes


def copy_to_state(fsv_paths: list[Path], fsv_root: Path, state_root: Path) -> list[Path]:
    copied: list[Path] = []
    for path in fsv_paths:
        target = state_root / path.relative_to(fsv_root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        copied.append(target)
    return copied


def mounted_drive_hits(paths: Iterable[Path]) -> int:
    hits = 0
    for path in paths:
        text = path.read_text(encoding="utf-8")
        hits += text.count("/mnt/c/") + text.count("/mnt/d/")
    return hits


def run_audit(args: argparse.Namespace) -> dict[str, Any]:
    corpus_root = Path(args.corpus_root)
    ast_chunks = Path(args.ast_chunks)
    ship_gate_report = Path(args.ship_gate_report)
    output_root = Path(args.output_root)
    state_root = Path(args.state_root)
    run_id = args.run_id or f"run-{utc_stamp()}-panelgraph-phase1"
    fsv_run_root = output_root / run_id
    state_run_root = state_root / run_id
    for label, path in (
        ("corpus_root", corpus_root),
        ("ast_chunks", ast_chunks),
        ("ship_gate_report", ship_gate_report),
        ("output_root", output_root),
        ("state_root", state_root),
    ):
        require_archive_path(path, label, args.allow_non_aiwonder_roots)
    require(not fsv_run_root.exists(), "PANELGRAPH_PHASE1_FSV_ROOT_EXISTS", str(fsv_run_root))
    require(not state_run_root.exists(), "PANELGRAPH_PHASE1_STATE_ROOT_EXISTS", str(state_run_root))
    fsv_run_root.mkdir(parents=True, exist_ok=False)
    state_run_root.mkdir(parents=True, exist_ok=False)

    index_rows = load_index(corpus_root)
    predictions = load_ship_gate_rows(ship_gate_report)
    ast_indexes = load_ast_indexes(ast_chunks)
    live_rows, eval_rows, diagnostics = build_rows(index_rows, corpus_root, ast_indexes, predictions)
    summary = summarize(live_rows, eval_rows, ast_indexes)
    decision = decide(
        summary,
        min_multi_file_rows=int(args.min_multi_file_split_eval_rows),
        min_mapping_coverage=float(args.min_mapping_coverage),
    )

    live_rows_path = fsv_run_root / "panelgraph_phase1_live_shape_rows.jsonl"
    eval_rows_path = fsv_run_root / "panelgraph_phase1_offline_eval_rows.jsonl"
    mapping_errors_path = fsv_run_root / "panelgraph_phase1_mapping_errors.jsonl"
    manifest_path = fsv_run_root / "panelgraph_phase1_manifest.json"
    write_jsonl(live_rows_path, live_rows)
    write_jsonl(eval_rows_path, eval_rows)
    write_jsonl(mapping_errors_path, diagnostics["mapping_errors"])
    output_hits_before_manifest = mounted_drive_hits([live_rows_path, eval_rows_path, mapping_errors_path])
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": ARTIFACT_KIND,
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "issue": 726,
        "created_at_utc": utc_now(),
        "run_id": run_id,
        "inputs": {
            "corpus_root": str(corpus_root),
            "ast_chunks": str(ast_chunks),
            "ship_gate_report": str(ship_gate_report),
        },
        "artifacts": {
            "live_shape_rows": str(live_rows_path),
            "offline_eval_rows": str(eval_rows_path),
            "mapping_errors": str(mapping_errors_path),
        },
        "summary": summary,
        "decision": decision,
        "doctrine": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "predictor_weights_changed": False,
            "panel_schema_changed": False,
            "inner_llm_used": False,
            "aiwonder_durable_root_required": not args.allow_non_aiwonder_roots,
        },
        "mounted_drive_hits_in_outputs_before_manifest": output_hits_before_manifest,
        "passes": output_hits_before_manifest == 0,
    }
    write_json(manifest_path, manifest)
    fsv_paths = [live_rows_path, eval_rows_path, mapping_errors_path, manifest_path]
    state_paths = copy_to_state(fsv_paths, fsv_run_root, state_run_root)
    hash_path = fsv_run_root / "artifact_hashes.json"
    all_hashes = artifact_hashes(
        hash_path,
        {f"fsv/{path.name}": path for path in fsv_paths} | {f"state/{path.name}": path for path in state_paths},
    )
    shutil.copy2(hash_path, state_run_root / hash_path.name)
    final_paths = [*fsv_paths, hash_path, *state_paths, state_run_root / hash_path.name]
    final_hits = mounted_drive_hits(final_paths)
    manifest["mounted_drive_hits_in_outputs"] = final_hits
    manifest["artifact_hashes"] = str(hash_path)
    manifest["state_run_root"] = str(state_run_root)
    manifest["state_mirror_byte_identical"] = True
    manifest["passes"] = final_hits == 0
    write_json(manifest_path, manifest)
    shutil.copy2(manifest_path, state_run_root / manifest_path.name)
    manifest["state_mirror_byte_identical"] = all(
        (state_run_root / path.name).read_bytes() == path.read_bytes() for path in fsv_paths
    )
    manifest["passes"] = final_hits == 0 and manifest["state_mirror_byte_identical"]
    write_json(manifest_path, manifest)
    shutil.copy2(manifest_path, state_run_root / manifest_path.name)
    artifact_hashes(
        hash_path,
        {f"fsv/{path.name}": path for path in fsv_paths} | {f"state/{path.name}": path for path in state_paths},
    )
    shutil.copy2(hash_path, state_run_root / hash_path.name)
    return {
        "manifest": str(manifest_path),
        "state_manifest": str(state_run_root / manifest_path.name),
        "artifact_hashes": str(hash_path),
        "fsv_run_root": str(fsv_run_root),
        "state_run_root": str(state_run_root),
        "passes": manifest["passes"],
        "decision": decision,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--ast-chunks", type=Path, default=DEFAULT_AST_CHUNKS)
    parser.add_argument("--ship-gate-report", type=Path, default=DEFAULT_SHIP_GATE_REPORT)
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument("--state-root", type=Path, default=DEFAULT_STATE_ROOT)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--min-multi-file-split-eval-rows", type=int, default=30)
    parser.add_argument("--min-mapping-coverage", type=float, default=0.95)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    return parser.parse_args()


def main() -> int:
    result = run_audit(parse_args())
    print(canonical_json(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
