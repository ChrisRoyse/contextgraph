#!/usr/bin/env python3
"""Build #844 candidate rows from semantic patch-structure signals.

This exporter emits measurement-only features from unified diff text: changed
path classes, hunk geometry, test-intent tokens, Python construct counts, and
added/deleted identifier churn. It intentionally ignores oracle/target-side
fields in the corpus index and writes no model state.
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
ARTIFACT_KIND = "patch_semantic_candidate_feature_rows"
SCHEMA_VERSION = 1
FORMULA_VERSION = "patch_semantic_structure_candidate_v1"
CANDIDATE_ID = "patch_semantic_structure"
AIWONDER_ROOTS = ("/zfs/archive/contextgraph/", "/zfs/hot/contextgraph/")
DEFAULT_CORPUS_ROOT = Path("/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511")
DEFAULT_OUTPUT_PARENT = Path("/zfs/archive/contextgraph/fsv/embedder-candidate-screening-fsv")
DEFAULT_STATE_PARENT = Path("/zfs/archive/contextgraph/state/embedder-candidate-screening/inv-032-v1")
FEATURE_KEY_RE = re.compile(r"^[A-Za-z0-9_.:/@|+=, -]{1,240}$")
IDENTIFIER_RE = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
NUMBER_RE = re.compile(r"(?<![A-Za-z_])(?:0x[0-9A-Fa-f]+|\d+\.\d+|\d+)(?![A-Za-z_])")
HUNK_RE = re.compile(r"^@@ -(?P<old_start>\d+)(?:,(?P<old_len>\d+))? \\+(?P<new_start>\d+)(?:,(?P<new_len>\d+))? @@(?P<context>.*)$")
PYTHON_KEYWORDS = {
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "false",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "none",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "true",
    "try",
    "while",
    "with",
    "yield",
}

CONSTRUCT_PATTERNS = {
    "assert": re.compile(r"\bassert\b"),
    "assignment": re.compile(r"(?<![=!<>])=(?!=)"),
    "async": re.compile(r"\basync\b|\bawait\b"),
    "bool_literal": re.compile(r"\b(?:True|False)\b"),
    "call": re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\s*\("),
    "class": re.compile(r"\bclass\s+[A-Za-z_]"),
    "comparison": re.compile(r"==|!=|<=|>=|<|>|\bis\b|\bin\b"),
    "control_if": re.compile(r"\b(?:if|elif|else)\b"),
    "control_loop": re.compile(r"\b(?:for|while)\b"),
    "decorator": re.compile(r"^\s*@"),
    "def": re.compile(r"\bdef\s+[A-Za-z_]"),
    "exception": re.compile(r"\b(?:try|except|finally|raise)\b"),
    "import": re.compile(r"\b(?:import|from)\b"),
    "logical": re.compile(r"\b(?:and|or|not)\b"),
    "none_literal": re.compile(r"\bNone\b"),
    "return": re.compile(r"\breturn\b"),
    "string_literal": re.compile(r"(?P<q>['\"])(?:\\.|(?!\\1).)*\\1"),
}

TEST_INTENT_PATTERNS = {
    "assertion": re.compile(r"\bassert\b|self\.assert[A-Z]|pytest\.raises"),
    "fixture": re.compile(r"\bfixture\b|@pytest\.fixture|\btmp_path\b|\bmonkeypatch\b|\bcaplog\b|\bcapsys\b"),
    "mocking": re.compile(r"\bmock\b|\bpatch\b|MagicMock|Mock\("),
    "parametrize": re.compile(r"parametrize|parameterized"),
    "pytest": re.compile(r"\bpytest\b"),
    "test_name": re.compile(r"\btest_[A-Za-z0-9_]*\b"),
    "unittest": re.compile(r"\bunittest\b|TestCase"),
}

RISK_PATTERNS = {
    "cache": re.compile(r"\bcache\b|\blru_cache\b"),
    "config": re.compile(r"\bconfig\b|\bsettings\b|\boption\b"),
    "database": re.compile(r"\bsql\b|\bquery\b|\bdatabase\b|\btransaction\b"),
    "filesystem": re.compile(r"\bPath\b|\bopen\s*\(|\bos\.path\b|\bshutil\b|\btempfile\b"),
    "json_yaml": re.compile(r"\bjson\b|\byaml\b"),
    "logging": re.compile(r"\blogger\b|\blogging\b|\bwarn(?:ing)?\b"),
    "network": re.compile(r"\bhttp\b|\brequests\b|\bsocket\b|\baiohttp\b|\burlopen\b"),
    "regex": re.compile(r"\bre\b|regex|re\.compile"),
    "subprocess": re.compile(r"\bsubprocess\b|\bos\.system\b|\bshell=True\b"),
    "time_random": re.compile(r"\btime\b|\bdatetime\b|\brandom\b|\buuid\b"),
}


class PatchSemanticCandidateFeatureError(RuntimeError):
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
        raise PatchSemanticCandidateFeatureError(f"{code}{suffix}")


def reject_mounted_drive(label: str, value: str) -> None:
    require(
        not (value.startswith("/mnt/c/") or value.startswith("/mnt/d/") or "/mnt/c/" in value or "/mnt/d/" in value),
        "PATCH_SEMANTIC_CANDIDATE_MOUNTED_DRIVE_FORBIDDEN",
        {label: value},
    )


def require_aiwonder_path(path: Path, label: str, allow_non_aiwonder_roots: bool) -> None:
    reject_mounted_drive(label, str(path))
    if allow_non_aiwonder_roots:
        return
    require(
        any(str(path).startswith(root) for root in AIWONDER_ROOTS),
        "PATCH_SEMANTIC_CANDIDATE_NON_AIWONDER_PATH",
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
    require(path.is_file(), "PATCH_SEMANTIC_CANDIDATE_JSON_MISSING", str(path))
    return json.loads(path.read_text(encoding="utf-8"))


def write_text_checked(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    with tmp.open("w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    tmp.replace(path)
    require(path.read_text(encoding="utf-8") == text, "PATCH_SEMANTIC_CANDIDATE_WRITE_READBACK_MISMATCH", str(path))


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
    require(isinstance(value, str) and value, "PATCH_SEMANTIC_CANDIDATE_REQUIRED_STRING", {"key": key, "context": context})
    return value


def add_feature(features: dict[str, float], name: str, value: float) -> None:
    require(FEATURE_KEY_RE.match(name) is not None, "PATCH_SEMANTIC_CANDIDATE_FEATURE_NAME_INVALID", name)
    number = float(value)
    require(math.isfinite(number), "PATCH_SEMANTIC_CANDIDATE_FEATURE_NONFINITE", {"name": name, "value": value})
    if abs(number) > 0.0:
        features[name] = number


def load_corpus_entries(corpus_root: Path, *, allow_non_aiwonder_roots: bool) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    require_aiwonder_path(corpus_root, "corpus_root", allow_non_aiwonder_roots)
    index_path = corpus_root / "index.json"
    payload = read_json(index_path)
    entries = payload.get("entries")
    require(isinstance(entries, list) and entries, "PATCH_SEMANTIC_CANDIDATE_INDEX_ENTRIES_MISSING")
    rows: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    category_counts: Counter[str] = Counter()
    for index, raw in enumerate(entries):
        require(isinstance(raw, dict), "PATCH_SEMANTIC_CANDIDATE_INDEX_ROW_NOT_OBJECT", index)
        task_id = required_string(raw, "task_id", index)
        repo = required_string(raw, "repo", index)
        category = required_string(raw, "category", index)
        patch_rel = required_string(raw, "patch_path", index)
        patch_sha256 = required_string(raw, "patch_sha256", index)
        reject_mounted_drive("patch_path", patch_rel)
        key = (task_id, category)
        require(key not in seen, "PATCH_SEMANTIC_CANDIDATE_DUPLICATE_ROW_KEY", {"task_id": task_id, "category": category})
        seen.add(key)
        patch_path = corpus_root / patch_rel
        require(patch_path.is_file(), "PATCH_SEMANTIC_CANDIDATE_PATCH_MISSING", str(patch_path))
        actual_sha = sha256_file(patch_path)
        require(
            actual_sha == patch_sha256,
            "PATCH_SEMANTIC_CANDIDATE_PATCH_HASH_MISMATCH",
            {"path": str(patch_path), "expected": patch_sha256, "actual": actual_sha},
        )
        rows.append({"task_id": task_id, "repo": repo, "category": category, "patch_path": patch_path})
        category_counts[category] += 1
    return rows, {
        "index_path": str(index_path),
        "index_sha256": sha256_file(index_path),
        "entry_count": len(rows),
        "category_counts": dict(sorted(category_counts.items())),
        "target_side_fields_ignored": True,
    }


def strip_diff_prefix(path: str) -> str:
    cleaned = path.strip()
    if cleaned in {"/dev/null", "dev/null"}:
        return cleaned
    if cleaned.startswith("a/") or cleaned.startswith("b/"):
        cleaned = cleaned[2:]
    return cleaned


def changed_paths(text: str) -> list[str]:
    paths: list[str] = []
    seen: set[str] = set()
    for line in text.splitlines():
        parts = line.split()
        candidates: list[str] = []
        if line.startswith("diff --git ") and len(parts) >= 4:
            candidates.extend([parts[2], parts[3]])
        elif line.startswith("--- ") and len(parts) >= 2:
            candidates.append(parts[1])
        elif line.startswith("+++ ") and len(parts) >= 2:
            candidates.append(parts[1])
        for candidate in candidates:
            path = strip_diff_prefix(candidate)
            if path and path not in {"/dev/null", "dev/null"} and path not in seen:
                seen.add(path)
                paths.append(path)
    return paths


def split_patch_lines(text: str) -> dict[str, Any]:
    added: list[str] = []
    deleted: list[str] = []
    context = 0
    meta = 0
    hunk_rows: list[dict[str, Any]] = []
    for line in text.splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            added.append(line[1:])
        elif line.startswith("-") and not line.startswith("---"):
            deleted.append(line[1:])
        elif line.startswith("@@"):
            match = HUNK_RE.match(line)
            hunk_rows.append(
                {
                    "old_start": int(match.group("old_start")) if match else 0,
                    "old_len": int(match.group("old_len") or 1) if match else 0,
                    "new_start": int(match.group("new_start")) if match else 0,
                    "new_len": int(match.group("new_len") or 1) if match else 0,
                    "context_len": len((match.group("context") if match else "").strip()),
                }
            )
        elif line.startswith("diff ") or line.startswith("index ") or line.startswith("---") or line.startswith("+++"):
            meta += 1
        else:
            context += 1
    return {"added": added, "deleted": deleted, "context": context, "meta": meta, "hunks": hunk_rows}


def path_features(paths: list[str]) -> dict[str, float]:
    features: dict[str, float] = {}
    require(paths, "PATCH_SEMANTIC_CANDIDATE_CHANGED_PATHS_EMPTY")
    file_count = float(len(paths))
    depth_sum = 0.0
    class_counts: Counter[str] = Counter()
    extension_counts: Counter[str] = Counter()
    for path in paths:
        lower = path.lower()
        parts = [part for part in lower.split("/") if part]
        basename = parts[-1] if parts else lower
        depth_sum += max(0, len(parts) - 1)
        suffix = Path(basename).suffix.lower().lstrip(".") or "none"
        if suffix in {"py", "pyi", "txt", "toml", "yaml", "yml", "json", "ini", "cfg", "md", "rst"}:
            extension_counts[suffix] += 1
        else:
            extension_counts["other"] += 1
        if suffix in {"py", "pyi"}:
            class_counts["python"] += 1
        if (
            basename.startswith("test_")
            or basename.endswith("_test.py")
            or "/tests/" in f"/{lower}/"
            or "/test/" in f"/{lower}/"
        ):
            class_counts["test"] += 1
        if any(part in {"doc", "docs", "documentation"} for part in parts) or suffix in {"md", "rst"}:
            class_counts["docs"] += 1
        if basename in {"pyproject.toml", "setup.py", "setup.cfg", "tox.ini"} or suffix in {"toml", "yaml", "yml", "ini", "cfg"}:
            class_counts["config"] += 1
        if "migrations" in parts or "migration" in basename:
            class_counts["migration"] += 1
        if basename == "__init__.py":
            class_counts["init"] += 1
    add_feature(features, "path:file_count_log1p", math.log1p(file_count))
    add_feature(features, "path:depth_mean", depth_sum / file_count)
    for name in ("python", "test", "docs", "config", "migration", "init"):
        add_feature(features, f"path:{name}:file_ratio", class_counts[name] / file_count)
        add_feature(features, f"path:{name}:file_count_log1p", math.log1p(class_counts[name]))
    for suffix in ("py", "pyi", "toml", "yaml", "yml", "json", "ini", "cfg", "md", "rst", "txt", "none", "other"):
        add_feature(features, f"path:ext:{suffix}:ratio", extension_counts[suffix] / file_count)
    return features


def count_patterns(lines: list[str], patterns: dict[str, re.Pattern[str]]) -> Counter[str]:
    out: Counter[str] = Counter()
    for line in lines:
        for name, pattern in patterns.items():
            matches = pattern.findall(line)
            out[name] += len(matches)
    return out


def identifiers(lines: list[str]) -> Counter[str]:
    out: Counter[str] = Counter()
    for line in lines:
        for match in IDENTIFIER_RE.finditer(line):
            token = match.group(0).lower()
            if token not in PYTHON_KEYWORDS and not token.startswith("__"):
                out[token] += 1
    return out


def line_kind_counts(lines: list[str]) -> Counter[str]:
    counts: Counter[str] = Counter()
    for line in lines:
        stripped = line.strip()
        if not stripped:
            counts["blank"] += 1
        elif stripped.startswith("#"):
            counts["comment"] += 1
        else:
            counts["code"] += 1
        counts["number_literal"] += len(NUMBER_RE.findall(line))
    return counts


def add_signed_counter_features(
    features: dict[str, float],
    *,
    prefix: str,
    names: Iterable[str],
    added: Counter[str],
    deleted: Counter[str],
    denom: float,
) -> None:
    for name in names:
        add_count = float(added[name])
        del_count = float(deleted[name])
        add_feature(features, f"{prefix}:{name}:add_count_log1p", math.log1p(add_count))
        add_feature(features, f"{prefix}:{name}:del_count_log1p", math.log1p(del_count))
        add_feature(features, f"{prefix}:{name}:net_per_changed_line", (add_count - del_count) / denom)
        add_feature(features, f"{prefix}:{name}:churn_per_changed_line", (add_count + del_count) / denom)


def semantic_features(text: str) -> tuple[dict[str, float], dict[str, Any]]:
    require(text.strip(), "PATCH_SEMANTIC_CANDIDATE_EMPTY_PATCH_TEXT")
    parts = split_patch_lines(text)
    added = parts["added"]
    deleted = parts["deleted"]
    hunks = parts["hunks"]
    changed_line_count = float(len(added) + len(deleted))
    denom = max(1.0, changed_line_count)
    features = path_features(changed_paths(text))
    add_feature(features, "line:added_count_log1p", math.log1p(len(added)))
    add_feature(features, "line:deleted_count_log1p", math.log1p(len(deleted)))
    add_feature(features, "line:changed_count_log1p", math.log1p(changed_line_count))
    add_feature(features, "line:add_ratio", float(len(added)) / denom)
    add_feature(features, "line:delete_ratio", float(len(deleted)) / denom)
    add_feature(features, "line:add_minus_delete_per_changed_line", float(len(added) - len(deleted)) / denom)
    add_feature(features, "line:context_count_log1p", math.log1p(float(parts["context"])))
    add_feature(features, "line:meta_count_log1p", math.log1p(float(parts["meta"])))
    add_feature(features, "hunk:count_log1p", math.log1p(len(hunks)))
    if hunks:
        add_feature(features, "hunk:old_len_mean_log1p", math.log1p(sum(row["old_len"] for row in hunks) / len(hunks)))
        add_feature(features, "hunk:new_len_mean_log1p", math.log1p(sum(row["new_len"] for row in hunks) / len(hunks)))
        add_feature(features, "hunk:context_header_len_mean_log1p", math.log1p(sum(row["context_len"] for row in hunks) / len(hunks)))

    construct_added = count_patterns(added, CONSTRUCT_PATTERNS)
    construct_deleted = count_patterns(deleted, CONSTRUCT_PATTERNS)
    test_added = count_patterns(added, TEST_INTENT_PATTERNS)
    test_deleted = count_patterns(deleted, TEST_INTENT_PATTERNS)
    risk_added = count_patterns(added, RISK_PATTERNS)
    risk_deleted = count_patterns(deleted, RISK_PATTERNS)
    kind_added = line_kind_counts(added)
    kind_deleted = line_kind_counts(deleted)
    add_signed_counter_features(
        features,
        prefix="construct",
        names=CONSTRUCT_PATTERNS,
        added=construct_added,
        deleted=construct_deleted,
        denom=denom,
    )
    add_signed_counter_features(
        features,
        prefix="test_intent",
        names=TEST_INTENT_PATTERNS,
        added=test_added,
        deleted=test_deleted,
        denom=denom,
    )
    add_signed_counter_features(
        features,
        prefix="risk",
        names=RISK_PATTERNS,
        added=risk_added,
        deleted=risk_deleted,
        denom=denom,
    )
    add_signed_counter_features(
        features,
        prefix="line_kind",
        names=("blank", "comment", "code", "number_literal"),
        added=kind_added,
        deleted=kind_deleted,
        denom=denom,
    )

    added_ids = identifiers(added)
    deleted_ids = identifiers(deleted)
    added_set = set(added_ids)
    deleted_set = set(deleted_ids)
    union = added_set | deleted_set
    intersection = added_set & deleted_set
    add_feature(features, "identifier:add_unique_log1p", math.log1p(len(added_set)))
    add_feature(features, "identifier:del_unique_log1p", math.log1p(len(deleted_set)))
    add_feature(features, "identifier:union_unique_log1p", math.log1p(len(union)))
    add_feature(features, "identifier:jaccard_overlap", len(intersection) / max(1.0, float(len(union))))
    add_feature(features, "identifier:added_only_ratio", len(added_set - deleted_set) / max(1.0, float(len(union))))
    add_feature(features, "identifier:deleted_only_ratio", len(deleted_set - added_set) / max(1.0, float(len(union))))
    add_feature(features, "identifier:token_net_per_changed_line", float(sum(added_ids.values()) - sum(deleted_ids.values())) / denom)
    add_feature(features, "identifier:token_churn_per_changed_line", float(sum(added_ids.values()) + sum(deleted_ids.values())) / denom)

    summary = {
        "changed_path_count": len(changed_paths(text)),
        "added_line_count": len(added),
        "deleted_line_count": len(deleted),
        "hunk_count": len(hunks),
        "feature_count": len(features),
    }
    return features, summary


def build_feature_rows(corpus_rows: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    out: list[dict[str, Any]] = []
    feature_counts: list[int] = []
    changed_path_counts: list[int] = []
    for source_index, row in enumerate(corpus_rows):
        text = Path(row["patch_path"]).read_text(encoding="utf-8", errors="replace")
        features, summary = semantic_features(text)
        feature_counts.append(summary["feature_count"])
        changed_path_counts.append(summary["changed_path_count"])
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
    require(feature_names, "PATCH_SEMANTIC_CANDIDATE_FEATURES_EMPTY")
    return out, {
        "feature_count_min": min(feature_counts),
        "feature_count_max": max(feature_counts),
        "feature_count_mean": sum(feature_counts) / len(feature_counts),
        "changed_path_count_min": min(changed_path_counts),
        "changed_path_count_max": max(changed_path_counts),
        "changed_path_count_mean": sum(changed_path_counts) / len(changed_path_counts),
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
        require(
            sha256_file(path) == sha256_file(target),
            "PATCH_SEMANTIC_CANDIDATE_STATE_HASH_MISMATCH",
            {"source": str(path), "state": str(target)},
        )
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
    run_id = str(args.run_id or f"run-{utc_stamp()}-patch-semantic-candidate")
    output_root = output_parent / run_id
    state_root = state_parent / run_id
    if bool(args.clean):
        shutil.rmtree(output_root, ignore_errors=True)
        shutil.rmtree(state_root, ignore_errors=True)
    require(not output_root.exists(), "PATCH_SEMANTIC_CANDIDATE_OUTPUT_ROOT_EXISTS", str(output_root))
    require(not state_root.exists(), "PATCH_SEMANTIC_CANDIDATE_STATE_ROOT_EXISTS", str(state_root))
    output_root.mkdir(parents=True, exist_ok=False)
    state_root.mkdir(parents=True, exist_ok=False)

    corpus_rows, corpus_summary = load_corpus_entries(corpus_root, allow_non_aiwonder_roots=allow_non_aiwonder)
    feature_rows, semantic_summary = build_feature_rows(corpus_rows)
    feature_names = sorted({name for row in feature_rows for name in (row.get("features") or {})})
    nonconstant = nonconstant_count(feature_rows, feature_names)
    require(nonconstant > 0, "PATCH_SEMANTIC_CANDIDATE_FEATURE_MATRIX_CONSTANT")
    feature_path = output_root / "patch_semantic_candidate_features.jsonl"
    manifest_path = output_root / "patch_semantic_candidate_features_manifest.json"
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
        "semantic_summary": semantic_summary,
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
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")
    parser.add_argument("--clean", action="store_true")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        print(json.dumps(run_feature_build(args), indent=2, sort_keys=True))
        return 0
    except PatchSemanticCandidateFeatureError as exc:
        print(f"PATCH_SEMANTIC_CANDIDATE_FEATURES_FAILED: {exc}", flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
