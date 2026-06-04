#!/usr/bin/env python3
"""Build #844 candidate features from PanelGraph live-shape rows.

This script is measurement-only. It converts `panelgraph_phase1_live_shape_rows`
into the candidate JSONL contract consumed by
`python_embedder_candidate_mi_screen.py`, preserving row identity while adding
structural categorical counts that are not present in the bare `live_features`
map.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
from collections import Counter
from collections.abc import Iterable
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TASK_ID = "INV-032"
CANONICAL_ISSUE = "#844"
ARTIFACT_KIND = "panelgraph_candidate_feature_rows"
SCHEMA_VERSION = 1
FORMULA_VERSION = "panelgraph_structural_counts_v1"
CANDIDATE_ID = "panelgraph_structural_counts"
AIWONDER_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/embedder-candidate-screening-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/embedder-candidate-screening/inv-032-v1")
FEATURE_KEY_RE = re.compile(r"^[A-Za-z0-9_.:/@|+=, -]{1,240}$")
SAFE_TOKEN_RE = re.compile(r"[^A-Za-z0-9_.:/@|+=, -]+")
FORBIDDEN_KEYS = {
    "actual_oracle_pass",
    "captured_exception",
    "completed_ids",
    "completed_instances",
    "error_ids",
    "error_instances",
    "execution_outcome",
    "label",
    "oracle_all_passed",
    "oracle_exception",
    "oracle_label",
    "oracle_outcome",
    "oracle_passed",
    "oracle_per_test_count",
    "oracle_verdict_sha256",
    "predicted_oracle_pass",
    "predicted_pass",
    "pytest_args",
    "pytest_exit_code",
    "resolved_ids",
    "resolved_instances",
    "target",
    "unresolved_ids",
    "unresolved_instances",
    "y",
}


class PanelGraphCandidateFeatureError(RuntimeError):
    pass


def utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def require(condition: bool, code: str, detail: Any = None) -> None:
    if not condition:
        suffix = "" if detail is None else f": {detail}"
        raise PanelGraphCandidateFeatureError(f"{code}{suffix}")


def reject_mounted_drive(label: str, value: str) -> None:
    require(
        not (value.startswith("/mnt/c/") or value.startswith("/mnt/d/") or "/mnt/c/" in value or "/mnt/d/" in value),
        "PANELGRAPH_CANDIDATE_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_aiwonder_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    reject_mounted_drive(label, str(path))
    if allow_non_aiwonder_roots:
        return
    require(
        any(str(path).startswith(root) for root in AIWONDER_ROOTS),
        "PANELGRAPH_CANDIDATE_NON_AIWONDER_PATH",
        {label: str(path), "allowed_roots": AIWONDER_ROOTS},
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_bytes(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    require(path.read_text(encoding="utf-8") == text, "PANELGRAPH_CANDIDATE_WRITE_READBACK_MISMATCH", str(path))


def write_json(path: Path, value: Any) -> None:
    write_text_checked(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    lines = []
    count = 0
    for row in rows:
        lines.append(canonical_json(row) + "\n")
        count += 1
    write_text_checked(path, "".join(lines))
    return count


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    require(path.is_file(), "PANELGRAPH_CANDIDATE_SOURCE_MISSING", str(path))
    rows: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_no, line in enumerate(handle, start=1):
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                raise PanelGraphCandidateFeatureError(f"PANELGRAPH_CANDIDATE_SOURCE_JSON_INVALID: {path}:{line_no}: {exc}") from exc
            require(isinstance(row, dict), "PANELGRAPH_CANDIDATE_SOURCE_ROW_NOT_OBJECT", {"line_no": line_no})
            rows.append(row)
    require(rows, "PANELGRAPH_CANDIDATE_SOURCE_EMPTY", str(path))
    return rows


def find_forbidden_keys(value: Any, *, max_list_items: int = 10) -> set[str]:
    hits: set[str] = set()

    def visit(item: Any) -> None:
        if isinstance(item, dict):
            for key, child in item.items():
                if str(key) in FORBIDDEN_KEYS:
                    hits.add(str(key))
                visit(child)
        elif isinstance(item, list):
            for child in item[:max_list_items]:
                visit(child)

    visit(value)
    return hits


def safe_token(value: Any, *, max_len: int = 96) -> str:
    raw = str(value or "unknown").strip().lower()
    raw = SAFE_TOKEN_RE.sub("_", raw)
    raw = raw.strip(" _:/")
    return (raw[:max_len] or "unknown")


def add_feature(features: dict[str, float], name: str, value: float) -> None:
    require(FEATURE_KEY_RE.match(name) is not None, "PANELGRAPH_CANDIDATE_FEATURE_NAME_INVALID", name)
    number = float(value)
    require(math.isfinite(number), "PANELGRAPH_CANDIDATE_FEATURE_NONFINITE", {"name": name, "value": value})
    features[name] = number


def add_counter_features(features: dict[str, float], prefix: str, counts: Any) -> None:
    require(isinstance(counts, dict), "PANELGRAPH_CANDIDATE_COUNTS_NOT_OBJECT", prefix)
    for raw_key, raw_value in sorted(counts.items()):
        require(isinstance(raw_value, (int, float)) and not isinstance(raw_value, bool), "PANELGRAPH_CANDIDATE_COUNT_NON_NUMERIC", {prefix: raw_key})
        key = safe_token(raw_key)
        value = float(raw_value)
        add_feature(features, f"{prefix}:{key}:count", value)
        add_feature(features, f"{prefix}:{key}:present", 1.0 if value > 0.0 else 0.0)


def numeric_live_features(row: dict[str, Any]) -> dict[str, float]:
    raw = row.get("live_features")
    require(isinstance(raw, dict) and raw, "PANELGRAPH_CANDIDATE_LIVE_FEATURES_MISSING")
    out: dict[str, float] = {}
    for raw_name, raw_value in sorted(raw.items()):
        require(isinstance(raw_value, (int, float)) and not isinstance(raw_value, bool), "PANELGRAPH_CANDIDATE_LIVE_FEATURE_NON_NUMERIC", raw_name)
        name = safe_token(raw_name)
        value = float(raw_value)
        add_feature(out, f"live:{name}", value)
        add_feature(out, f"live_log1p:{name}", math.log1p(max(0.0, value)))
    return out


def path_features(row: dict[str, Any]) -> dict[str, float]:
    paths = row.get("sample_touched_files") or []
    require(isinstance(paths, list), "PANELGRAPH_CANDIDATE_SAMPLE_PATHS_NOT_LIST")
    cleaned = [str(path) for path in paths if isinstance(path, str)]
    for path in cleaned:
        reject_mounted_drive("sample_touched_file", path)
    features: dict[str, float] = {}
    depths = [len([part for part in path.split("/") if part]) for path in cleaned]
    top_dirs = [safe_token(path.split("/", 1)[0]) for path in cleaned if path]
    suffixes = [safe_token(Path(path).suffix.lstrip(".") or "noext") for path in cleaned]
    basenames = [Path(path).name.lower() for path in cleaned]
    joined = "\n".join(cleaned).lower()
    count = max(1, len(cleaned))
    add_feature(features, "path:sample_count", float(len(cleaned)))
    add_feature(features, "path:max_depth", float(max(depths) if depths else 0))
    add_feature(features, "path:mean_depth", float(sum(depths) / len(depths)) if depths else 0.0)
    add_feature(features, "path:unique_top_dir_count", float(len(set(top_dirs))))
    add_feature(features, "path:py_file_ratio", sum(1 for suffix in suffixes if suffix == "py") / count)
    add_feature(features, "path:test_path_present", 1.0 if any("test" in path for path in basenames) or "/tests/" in joined or joined.startswith("tests/") else 0.0)
    add_feature(features, "path:doc_path_present", 1.0 if "/doc" in joined or joined.startswith("doc") else 0.0)
    add_feature(features, "path:setup_path_present", 1.0 if "setup.py" in joined or "pyproject.toml" in joined or "setup.cfg" in joined else 0.0)
    add_feature(features, "path:init_file_present", 1.0 if "__init__.py" in joined else 0.0)
    add_feature(features, "path:migration_path_present", 1.0 if "migration" in joined else 0.0)
    for top_dir, value in sorted(Counter(top_dirs).items()):
        add_feature(features, f"path_top:{top_dir}:count", float(value))
    for suffix, value in sorted(Counter(suffixes).items()):
        add_feature(features, f"path_suffix:{suffix}:count", float(value))
    return features


def ratio_features(live: dict[str, float]) -> dict[str, float]:
    hunk_count = live.get("patch_hunk_count", 0.0)
    file_count = live.get("patch_touched_file_count", 0.0)
    chunk_count = live.get("patch_touched_chunk_count", 0.0)
    mapped_hunks = live.get("mapped_hunk_count", 0.0)
    unmapped_hunks = live.get("unmapped_hunk_count", 0.0)
    state_chunks = live.get("candidate_state_chunk_count", 0.0)
    state_files = live.get("candidate_state_file_count", 0.0)
    return {
        "ratio:mapped_hunks_per_hunk": mapped_hunks / max(1.0, hunk_count),
        "ratio:unmapped_hunks_per_hunk": unmapped_hunks / max(1.0, hunk_count),
        "ratio:touched_chunks_per_hunk": chunk_count / max(1.0, hunk_count),
        "ratio:touched_chunks_per_file": chunk_count / max(1.0, file_count),
        "ratio:state_chunks_per_state_file": state_chunks / max(1.0, state_files),
        "indicator:all_hunks_mapped": 1.0 if hunk_count > 0.0 and mapped_hunks == hunk_count else 0.0,
        "indicator:any_unmapped_hunk": 1.0 if unmapped_hunks > 0.0 else 0.0,
        "indicator:multi_file": 1.0 if file_count >= 2.0 else 0.0,
        "indicator:multi_chunk": 1.0 if chunk_count >= 2.0 else 0.0,
    }


def features_for_row(row: dict[str, Any]) -> dict[str, float]:
    forbidden = sorted(find_forbidden_keys(row))
    require(not forbidden, "PANELGRAPH_CANDIDATE_FORBIDDEN_SOURCE_KEY", forbidden[:20])
    features = numeric_live_features(row)
    live_raw = {key.replace("live:", "", 1): value for key, value in features.items() if key.startswith("live:")}
    for name, value in ratio_features(live_raw).items():
        add_feature(features, name, value)
    add_feature(features, f"shape:{safe_token(row.get('shape_class'))}:present", 1.0)
    add_counter_features(features, "change_kind", row.get("change_kind_counts") or {})
    add_counter_features(features, "diff_side", row.get("target_side_counts") or {})
    add_counter_features(features, "chunk_kind", row.get("primary_chunk_kind_counts") or {})
    for name, value in path_features(row).items():
        add_feature(features, name, value)
    require(features, "PANELGRAPH_CANDIDATE_FEATURES_EMPTY")
    return features


def row_key(row: dict[str, Any]) -> tuple[str, str]:
    task_id = str(row.get("task_id") or row.get("task_instance_id") or "")
    category = str(row.get("category") or row.get("mutation_category") or "")
    require(task_id and category, "PANELGRAPH_CANDIDATE_ROW_KEY_MISSING", row)
    return task_id, category


def nonconstant_count(rows: list[dict[str, Any]], feature_names: list[str]) -> int:
    out = 0
    for name in feature_names:
        values = [float((row.get("features") or {}).get(name, 0.0)) for row in rows]
        first = values[0]
        if any(abs(value - first) > 1.0e-12 for value in values[1:]):
            out += 1
    return out


def matrix_sha256(rows: list[dict[str, Any]], feature_names: list[str]) -> str:
    payload = []
    for row in rows:
        feature_map = row.get("features") or {}
        payload.append([float(feature_map.get(name, 0.0)) for name in feature_names])
    return sha256_bytes(canonical_json(payload).encode("utf-8"))


def build_feature_rows(source_rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for source_index, row in enumerate(source_rows):
        key = row_key(row)
        require(key not in seen, "PANELGRAPH_CANDIDATE_DUPLICATE_ROW_KEY", {"task_id": key[0], "category": key[1]})
        seen.add(key)
        features = features_for_row(row)
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": ARTIFACT_KIND,
                "formula_version": FORMULA_VERSION,
                "candidate_id": CANDIDATE_ID,
                "source_row_index": source_index,
                "task_instance_id": key[0],
                "task_id": key[0],
                "mutation_category": key[1],
                "category": key[1],
                "repo": str(row.get("repo") or ""),
                "workspace_state": str(row.get("workspace_state") or ""),
                "features": features,
            }
        )
    return out


def artifact_hashes(paths: list[Path]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": f"{ARTIFACT_KIND}_hashes",
        "created_at_utc": utc_now(),
        "artifacts": [
            {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256_file(path)}
            for path in sorted(paths)
            if path.is_file()
        ],
    }


def mirror_to_state(output_root: Path, state_root: Path, paths: list[Path]) -> list[Path]:
    copied = []
    for path in paths:
        target = state_root / path.relative_to(output_root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        require(sha256_file(path) == sha256_file(target), "PANELGRAPH_CANDIDATE_STATE_MIRROR_HASH_MISMATCH", {"source": str(path), "state": str(target)})
        copied.append(target)
    return copied


def run_feature_build(args: argparse.Namespace) -> dict[str, Any]:
    source_path = Path(args.live_shape_rows)
    output_parent = Path(args.output_parent)
    state_parent = Path(args.state_parent)
    allow_non_aiwonder = bool(args.allow_non_aiwonder_roots)
    require_aiwonder_path(source_path, "live_shape_rows", allow_non_aiwonder)
    require_aiwonder_path(output_parent, "output_parent", allow_non_aiwonder)
    require_aiwonder_path(state_parent, "state_parent", allow_non_aiwonder)
    run_id = str(args.run_id or f"run-{utc_stamp()}-panelgraph-candidate-features")
    output_root = output_parent / run_id
    state_root = state_parent / run_id
    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "PANELGRAPH_CANDIDATE_OUTPUT_ROOT_EXISTS", str(output_root))
    require(not state_root.exists(), "PANELGRAPH_CANDIDATE_STATE_ROOT_EXISTS", str(state_root))
    output_root.mkdir(parents=True, exist_ok=False)
    state_root.mkdir(parents=True, exist_ok=False)

    source_rows = read_jsonl(source_path)
    feature_rows = build_feature_rows(source_rows)
    feature_names = sorted({name for row in feature_rows for name in (row.get("features") or {})})
    nonconstant = nonconstant_count(feature_rows, feature_names)
    require(nonconstant > 0, "PANELGRAPH_CANDIDATE_FEATURE_MATRIX_CONSTANT")

    feature_path = output_root / "panelgraph_candidate_features.jsonl"
    manifest_path = output_root / "panelgraph_candidate_features_manifest.json"
    write_jsonl(feature_path, feature_rows)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": f"{ARTIFACT_KIND}_manifest",
        "formula_version": FORMULA_VERSION,
        "task_id": TASK_ID,
        "canonical_issue": CANONICAL_ISSUE,
        "candidate_id": CANDIDATE_ID,
        "run_id": run_id,
        "created_at_utc": utc_now(),
        "passes": True,
        "source_path": str(source_path),
        "source_sha256": sha256_file(source_path),
        "source_row_count": len(source_rows),
        "row_count": len(feature_rows),
        "feature_count": len(feature_names),
        "nonconstant_feature_count": nonconstant,
        "feature_matrix_sha256": matrix_sha256(feature_rows, feature_names),
        "category_counts": dict(sorted(Counter(row["mutation_category"] for row in feature_rows).items())),
        "feature_presence": {
            "min": min(sum(1 for row in feature_rows if name in (row.get("features") or {})) for name in feature_names),
            "max": max(sum(1 for row in feature_rows if name in (row.get("features") or {})) for name in feature_names),
        },
        "doctrine_policy": {
            "measurement_only": True,
            "slot_identity_preserved": True,
            "flat_vector_concat_used": False,
            "target_labels_used_as_live_inputs": False,
            "candidate_features_are_separate_slots": True,
            "predictor_weights_changed": False,
            "inner_llm_used": False,
        },
        "artifacts": {
            "candidate_features": str(feature_path),
            "candidate_features_sha256": sha256_file(feature_path),
            "state_root": str(state_root),
        },
    }
    write_json(manifest_path, manifest)
    copied = mirror_to_state(output_root, state_root, [feature_path, manifest_path])
    hashes_path = output_root / "artifact_hashes.json"
    write_json(hashes_path, artifact_hashes([feature_path, manifest_path, *copied]))
    state_hashes_path = state_root / hashes_path.name
    write_json(state_hashes_path, json.loads(hashes_path.read_text(encoding="utf-8")))
    return {
        "ok": True,
        "run_id": run_id,
        "output_root": str(output_root),
        "state_root": str(state_root),
        "candidate_features": str(feature_path),
        "manifest": str(manifest_path),
        "artifact_hashes": str(hashes_path),
        "row_count": len(feature_rows),
        "feature_count": len(feature_names),
        "nonconstant_feature_count": nonconstant,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live-shape-rows", type=Path, required=True)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        print(json.dumps(run_feature_build(args), indent=2, sort_keys=True))
        return 0
    except PanelGraphCandidateFeatureError as exc:
        print(f"PANELGRAPH_CANDIDATE_FEATURES_FAILED: {exc}", flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
