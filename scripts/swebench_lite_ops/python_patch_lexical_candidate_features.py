#!/usr/bin/env python3
"""Build #844 candidate rows from raw patch lexical hashes.

This is a candidate-feature exporter, not a predictor. It emits sparse signed
hash features from patch bytes plus patch-text shape counters, then mirrors the
source rows under the #844 FSV/state roots for the candidate MI screen.
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

try:
    from .python_patch_lexical_baseline import lexical_tokens
except ImportError:  # pragma: no cover - direct script execution on aiwonder.
    import sys

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from swebench_lite_ops.python_patch_lexical_baseline import lexical_tokens  # type: ignore


TASK_ID = "INV-032"
CANONICAL_ISSUE = "#844"
ARTIFACT_KIND = "patch_lexical_candidate_feature_rows"
SCHEMA_VERSION = 1
FORMULA_VERSION = "patch_lexical_hash_candidate_v1"
CANDIDATE_ID = "patch_lexical_hash"
AIWONDER_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/embedder-candidate-screening-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/embedder-candidate-screening/inv-032-v1")
FEATURE_KEY_RE = re.compile(r"^[A-Za-z0-9_.:/@|+=, -]{1,240}$")


class PatchLexicalCandidateFeatureError(RuntimeError):
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
        raise PatchLexicalCandidateFeatureError(f"{code}{suffix}")


def reject_mounted_drive(label: str, value: str) -> None:
    require(
        not (value.startswith("/mnt/c/") or value.startswith("/mnt/d/") or "/mnt/c/" in value or "/mnt/d/" in value),
        "PATCH_LEXICAL_CANDIDATE_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_aiwonder_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    reject_mounted_drive(label, str(path))
    if allow_non_aiwonder_roots:
        return
    require(
        any(str(path).startswith(root) for root in AIWONDER_ROOTS),
        "PATCH_LEXICAL_CANDIDATE_NON_AIWONDER_PATH",
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


def read_json(path: Path) -> Any:
    require(path.is_file(), "PATCH_LEXICAL_CANDIDATE_JSON_MISSING", str(path))
    return json.loads(path.read_text(encoding="utf-8"))


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    require(path.read_text(encoding="utf-8") == text, "PATCH_LEXICAL_CANDIDATE_WRITE_READBACK_MISMATCH", str(path))


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


def required_string(row: dict[str, Any], key: str, context: Any) -> str:
    value = row.get(key)
    require(isinstance(value, str) and value, "PATCH_LEXICAL_CANDIDATE_REQUIRED_STRING", {"key": key, "context": context})
    return value


def add_feature(features: dict[str, float], name: str, value: float) -> None:
    require(FEATURE_KEY_RE.match(name) is not None, "PATCH_LEXICAL_CANDIDATE_FEATURE_NAME_INVALID", name)
    number = float(value)
    require(math.isfinite(number), "PATCH_LEXICAL_CANDIDATE_FEATURE_NONFINITE", {"name": name, "value": value})
    if abs(number) > 0.0:
        features[name] = number


def line_shape_features(text: str) -> dict[str, float]:
    lines = text.splitlines()
    prefixes = Counter()
    for line in lines:
        if line.startswith("+") and not line.startswith("+++"):
            prefixes["add"] += 1
        elif line.startswith("-") and not line.startswith("---"):
            prefixes["del"] += 1
        elif line.startswith("@@"):
            prefixes["hunk"] += 1
        elif line.startswith("diff ") or line.startswith("index ") or line.startswith("---") or line.startswith("+++"):
            prefixes["meta"] += 1
        else:
            prefixes["ctx"] += 1
    token_count = len(lexical_tokens(text))
    unique_token_count = len(set(lexical_tokens(text)))
    features: dict[str, float] = {}
    add_feature(features, "shape:line_count_log1p", math.log1p(len(lines)))
    add_feature(features, "shape:byte_count_log1p", math.log1p(len(text.encode("utf-8", errors="replace"))))
    add_feature(features, "shape:token_count_log1p", math.log1p(token_count))
    add_feature(features, "shape:unique_token_count_log1p", math.log1p(unique_token_count))
    denom = max(1.0, float(len(lines)))
    for prefix in ("add", "del", "hunk", "meta", "ctx"):
        add_feature(features, f"line:{prefix}:count_log1p", math.log1p(prefixes[prefix]))
        add_feature(features, f"line:{prefix}:ratio", prefixes[prefix] / denom)
    add_feature(features, "line:add_minus_del", float(prefixes["add"] - prefixes["del"]))
    add_feature(features, "line:add_del_total_log1p", math.log1p(prefixes["add"] + prefixes["del"]))
    return features


def sparse_hash_features(text: str, *, hash_dim: int, token_limit: int) -> tuple[dict[str, float], dict[str, int]]:
    require(hash_dim >= 64, "PATCH_LEXICAL_CANDIDATE_HASH_DIM_TOO_SMALL", hash_dim)
    tokens = lexical_tokens(text)
    if token_limit > 0:
        tokens = tokens[:token_limit]
    require(tokens, "PATCH_LEXICAL_CANDIDATE_NO_TOKENS")
    values: Counter[int] = Counter()
    for token in tokens:
        digest = hashlib.sha256(token.encode("utf-8")).digest()
        column = int.from_bytes(digest[:8], "little") % hash_dim
        sign = 1 if (digest[8] & 1) == 0 else -1
        values[column] += sign
    norm = math.sqrt(float(len(tokens)))
    features: dict[str, float] = {}
    for column, value in sorted(values.items()):
        add_feature(features, f"lexhash:{column:04d}", float(value) / norm)
    return features, {"token_count": len(tokens), "unique_token_count": len(set(tokens)), "nonzero_hash_columns": len(values)}


def load_corpus_entries(corpus_root: Path, *, allow_non_aiwonder_roots: bool) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require_aiwonder_path(corpus_root, "corpus_root", allow_non_aiwonder_roots)
    index_path = corpus_root / "index.json"
    payload = read_json(index_path)
    entries = payload.get("entries")
    require(isinstance(entries, list) and entries, "PATCH_LEXICAL_CANDIDATE_INDEX_ENTRIES_MISSING")
    rows: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    category_counts: Counter[str] = Counter()
    for index, raw in enumerate(entries):
        require(isinstance(raw, dict), "PATCH_LEXICAL_CANDIDATE_INDEX_ROW_NOT_OBJECT", index)
        task_id = required_string(raw, "task_id", index)
        repo = required_string(raw, "repo", index)
        category = required_string(raw, "category", index)
        patch_rel = required_string(raw, "patch_path", index)
        patch_sha256 = required_string(raw, "patch_sha256", index)
        reject_mounted_drive("patch_path", patch_rel)
        key = (task_id, category)
        require(key not in seen, "PATCH_LEXICAL_CANDIDATE_DUPLICATE_ROW_KEY", {"task_id": task_id, "category": category})
        seen.add(key)
        patch_path = corpus_root / patch_rel
        require(patch_path.is_file(), "PATCH_LEXICAL_CANDIDATE_PATCH_MISSING", str(patch_path))
        actual_sha = sha256_file(patch_path)
        require(actual_sha == patch_sha256, "PATCH_LEXICAL_CANDIDATE_PATCH_HASH_MISMATCH", {"path": str(patch_path), "expected": patch_sha256, "actual": actual_sha})
        rows.append({"task_id": task_id, "repo": repo, "category": category, "patch_path": patch_path})
        category_counts[category] += 1
    return rows, {
        "index_path": str(index_path),
        "index_sha256": sha256_file(index_path),
        "entry_count": len(rows),
        "category_counts": dict(sorted(category_counts.items())),
        "target_side_fields_ignored": True,
    }


def build_feature_rows(corpus_rows: list[dict[str, Any]], *, hash_dim: int, token_limit: int) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    out: list[dict[str, Any]] = []
    hash_nonzero_counts: list[int] = []
    token_counts: list[int] = []
    for source_index, row in enumerate(corpus_rows):
        text = Path(row["patch_path"]).read_text(encoding="utf-8", errors="replace")
        require(text.strip(), "PATCH_LEXICAL_CANDIDATE_EMPTY_PATCH_TEXT", str(row["patch_path"]))
        features = line_shape_features(text)
        hashed, token_summary = sparse_hash_features(text, hash_dim=hash_dim, token_limit=token_limit)
        features.update(hashed)
        hash_nonzero_counts.append(token_summary["nonzero_hash_columns"])
        token_counts.append(token_summary["token_count"])
        out.append(
            {
                "schema_version": SCHEMA_VERSION,
                "artifact_kind": ARTIFACT_KIND,
                "formula_version": FORMULA_VERSION,
                "candidate_id": CANDIDATE_ID,
                "source_row_index": source_index,
                "task_instance_id": str(row["task_id"]),
                "task_id": str(row["task_id"]),
                "mutation_category": str(row["category"]),
                "category": str(row["category"]),
                "repo": str(row["repo"]),
                "features": features,
            }
        )
    feature_names = sorted({name for row in out for name in (row.get("features") or {})})
    require(feature_names, "PATCH_LEXICAL_CANDIDATE_FEATURES_EMPTY")
    return out, {
        "hash_dim": hash_dim,
        "token_limit": token_limit,
        "feature_count": len(feature_names),
        "hash_nonzero_count_min": min(hash_nonzero_counts),
        "hash_nonzero_count_max": max(hash_nonzero_counts),
        "hash_nonzero_count_mean": sum(hash_nonzero_counts) / len(hash_nonzero_counts),
        "token_count_min": min(token_counts),
        "token_count_max": max(token_counts),
        "token_count_mean": sum(token_counts) / len(token_counts),
    }


def nonconstant_count(rows: list[dict[str, Any]], feature_names: list[str]) -> int:
    out = 0
    for name in feature_names:
        values = [float((row.get("features") or {}).get(name, 0.0)) for row in rows]
        first = values[0]
        if any(abs(value - first) > 1.0e-12 for value in values[1:]):
            out += 1
    return out


def matrix_sha256(rows: list[dict[str, Any]], feature_names: list[str]) -> str:
    sparse_payload = []
    for row in rows:
        feature_map = row.get("features") or {}
        sparse_payload.append([[name, float(feature_map[name])] for name in feature_names if name in feature_map])
    return sha256_bytes(canonical_json(sparse_payload).encode("utf-8"))


def mirror_to_state(output_root: Path, state_root: Path, paths: list[Path]) -> list[Path]:
    copied = []
    for path in paths:
        target = state_root / path.relative_to(output_root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        require(sha256_file(path) == sha256_file(target), "PATCH_LEXICAL_CANDIDATE_STATE_HASH_MISMATCH", {"source": str(path), "state": str(target)})
        copied.append(target)
    return copied


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


def run_feature_build(args: argparse.Namespace) -> dict[str, Any]:
    corpus_root = Path(args.corpus_root)
    output_parent = Path(args.output_parent)
    state_parent = Path(args.state_parent)
    allow_non_aiwonder = bool(args.allow_non_aiwonder_roots)
    require_aiwonder_path(corpus_root, "corpus_root", allow_non_aiwonder)
    require_aiwonder_path(output_parent, "output_parent", allow_non_aiwonder)
    require_aiwonder_path(state_parent, "state_parent", allow_non_aiwonder)
    run_id = str(args.run_id or f"run-{utc_stamp()}-patch-lexical-candidate")
    output_root = output_parent / run_id
    state_root = state_parent / run_id
    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "PATCH_LEXICAL_CANDIDATE_OUTPUT_ROOT_EXISTS", str(output_root))
    require(not state_root.exists(), "PATCH_LEXICAL_CANDIDATE_STATE_ROOT_EXISTS", str(state_root))
    output_root.mkdir(parents=True, exist_ok=False)
    state_root.mkdir(parents=True, exist_ok=False)

    corpus_rows, corpus_summary = load_corpus_entries(corpus_root, allow_non_aiwonder_roots=allow_non_aiwonder)
    feature_rows, lexical_summary = build_feature_rows(corpus_rows, hash_dim=int(args.hash_dim), token_limit=int(args.token_limit))
    feature_names = sorted({name for row in feature_rows for name in (row.get("features") or {})})
    nonconstant = nonconstant_count(feature_rows, feature_names)
    require(nonconstant > 0, "PATCH_LEXICAL_CANDIDATE_FEATURE_MATRIX_CONSTANT")
    feature_path = output_root / "patch_lexical_candidate_features.jsonl"
    manifest_path = output_root / "patch_lexical_candidate_features_manifest.json"
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
        "row_count": len(feature_rows),
        "feature_count": len(feature_names),
        "nonconstant_feature_count": nonconstant,
        "feature_matrix_sha256": matrix_sha256(feature_rows, feature_names),
        "category_counts": dict(sorted(Counter(row["mutation_category"] for row in feature_rows).items())),
        "corpus": corpus_summary,
        "lexical_summary": lexical_summary,
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
    parser.add_argument("--corpus-root", type=Path, default=DEFAULT_CORPUS_ROOT)
    parser.add_argument("--output-parent", type=Path, default=DEFAULT_OUTPUT_PARENT)
    parser.add_argument("--state-parent", type=Path, default=DEFAULT_STATE_PARENT)
    parser.add_argument("--run-id", default=None)
    parser.add_argument("--hash-dim", type=int, default=1024)
    parser.add_argument("--token-limit", type=int, default=8192)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        print(json.dumps(run_feature_build(args), indent=2, sort_keys=True))
        return 0
    except PatchLexicalCandidateFeatureError as exc:
        print(f"PATCH_LEXICAL_CANDIDATE_FEATURES_FAILED: {exc}", flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
