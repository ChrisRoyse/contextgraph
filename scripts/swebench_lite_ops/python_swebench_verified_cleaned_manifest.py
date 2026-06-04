from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter, defaultdict
from pathlib import Path, PurePosixPath
from typing import Any, Iterable

from .core import SwebenchOpsError, ensure_file, sha256_file, utc_now, write_json_checked
from .oracle_events import canonical_json, sha256_prefixed_text


DEFAULT_DATASET_NAME = "princeton-nlp/SWE-bench_Verified"
DEFAULT_DATASET_SPLIT = "test"
EXPECTED_CATEGORIES = (
    "compile_error",
    "delete_test_call",
    "known_good",
    "off_by_one",
    "over_engineer",
    "subtle_flip",
    "swap_variable",
    "wrong_file",
)
FORBIDDEN_MOUNT_PREFIXES = ("/mnt/c", "/mnt/d")
ARTIFACT_KIND = "swebench_verified_cleaned_manifest_export"


class VerifiedCleanedManifestError(SwebenchOpsError):
    pass


def require(condition: bool, code: str, message: str, details: dict[str, Any] | None = None) -> None:
    if not condition:
        raise VerifiedCleanedManifestError(
            code,
            message,
            "Fix the SWE-bench Verified cleaned-manifest export before AST/forward-cache use.",
            details or {},
        )


def reject_mounted_path(path: Path, field: str, *, allow_non_aiwonder_roots: bool) -> Path:
    resolved = path.expanduser().resolve(strict=False)
    text = resolved.as_posix()
    if any(text == prefix or text.startswith(prefix + "/") for prefix in FORBIDDEN_MOUNT_PREFIXES):
        raise VerifiedCleanedManifestError(
            "SWEBENCH_VERIFIED_CLEANED_MANIFEST_MOUNTED_PATH_FORBIDDEN",
            f"{field} must not be on a mounted Windows drive",
            "Use aiwonder /zfs roots or a local test tempdir.",
            {"field": field, "path": text},
        )
    if not allow_non_aiwonder_roots:
        require(
            text.startswith("/zfs/archive/contextgraph/") or text.startswith("/zfs/hot/contextgraph/"),
            "SWEBENCH_VERIFIED_CLEANED_MANIFEST_NOT_AIWONDER_ROOT",
            f"{field} must be under aiwonder ContextGraph roots",
            {"field": field, "path": text},
        )
    return resolved


def safe_relative_path(value: str, field: str) -> PurePosixPath:
    path = PurePosixPath(value)
    require(not path.is_absolute(), "SWEBENCH_VERIFIED_PATCH_PATH_ABSOLUTE", f"{field} is absolute", {"path": value})
    require(".." not in path.parts, "SWEBENCH_VERIFIED_PATCH_PATH_ESCAPES", f"{field} escapes corpus root", {"path": value})
    require(bool(path.parts), "SWEBENCH_VERIFIED_PATCH_PATH_EMPTY", f"{field} is empty", {"path": value})
    return path


def read_json(path: Path, code: str) -> Any:
    ensure_file(path, code, "required JSON artifact")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise VerifiedCleanedManifestError(
            code,
            f"invalid JSON at {path}",
            "Regenerate the upstream artifact before exporting the cleaned manifest.",
            {"path": str(path), "error": str(exc)},
        ) from exc


def read_patch_text(corpus_root: Path, entry: dict[str, Any]) -> tuple[str, str]:
    rel = safe_relative_path(str(entry.get("patch_path") or ""), "entry.patch_path")
    path = corpus_root / rel.as_posix()
    ensure_file(path, "SWEBENCH_VERIFIED_PATCH_FILE_MISSING", "mutation patch")
    raw = path.read_bytes()
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise VerifiedCleanedManifestError(
            "SWEBENCH_VERIFIED_PATCH_UTF8_INVALID",
            "mutation patch bytes are not valid UTF-8",
            "Regenerate the upstream corpus patch before exporting the cleaned manifest.",
            {"task_id": entry.get("task_id"), "category": entry.get("category"), "path": str(path), "error": str(exc)},
        ) from exc
    actual = "sha256:" + hashlib.sha256(raw).hexdigest()
    expected = str(entry.get("patch_sha256") or "")
    require(
        actual == expected,
        "SWEBENCH_VERIFIED_PATCH_SHA_MISMATCH",
        "mutation patch bytes do not match index patch_sha256",
        {"task_id": entry.get("task_id"), "category": entry.get("category"), "path": str(path), "expected": expected, "actual": actual},
    )
    return text, str(rel)


def load_dataset_rows(dataset_name: str, dataset_split: str) -> list[dict[str, Any]]:
    from datasets import load_dataset

    return [dict(row) for row in load_dataset(dataset_name, split=dataset_split)]


def dataset_index(rows: Iterable[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    by_id: dict[str, dict[str, Any]] = {}
    for row in rows:
        instance_id = str(row.get("instance_id") or "")
        require(bool(instance_id), "SWEBENCH_VERIFIED_DATASET_INSTANCE_ID_MISSING", "dataset row is missing instance_id")
        require(
            instance_id not in by_id,
            "SWEBENCH_VERIFIED_DATASET_INSTANCE_ID_DUPLICATE",
            "dataset rows contain duplicate instance_id",
            {"instance_id": instance_id},
        )
        missing = [field for field in ("repo", "base_commit", "patch", "test_patch") if not row.get(field)]
        require(
            not missing,
            "SWEBENCH_VERIFIED_DATASET_FIELD_MISSING",
            "dataset row is missing required patch metadata",
            {"instance_id": instance_id, "missing": missing},
        )
        by_id[instance_id] = row
    return by_id


def load_task_manifest(tasks_dir: Path | None, task_id: str) -> dict[str, Any] | None:
    if tasks_dir is None:
        return None
    path = tasks_dir / f"{task_id}.json"
    if not path.is_file():
        return None
    payload = read_json(path, "SWEBENCH_VERIFIED_TASK_MANIFEST_INVALID")
    require(
        payload.get("instance_id") == task_id,
        "SWEBENCH_VERIFIED_TASK_MANIFEST_ID_MISMATCH",
        "task manifest belongs to a different task",
        {"task_id": task_id, "manifest": str(path), "observed": payload.get("instance_id")},
    )
    return payload


def validate_task_manifest(task_manifest: dict[str, Any] | None, dataset_row: dict[str, Any], task_id: str) -> None:
    if task_manifest is None:
        return
    expected_patch_sha = sha256_prefixed_text(str(dataset_row["patch"]))
    expected_test_sha = sha256_prefixed_text(str(dataset_row["test_patch"]))
    checks = {
        "repo": str(dataset_row["repo"]),
        "base_commit": str(dataset_row["base_commit"]),
        "oracle_patch_sha256": expected_patch_sha,
        "official_test_patch_sha256": expected_test_sha,
    }
    mismatches = {
        key: {"expected": expected, "observed": task_manifest.get(key)}
        for key, expected in checks.items()
        if task_manifest.get(key) != expected
    }
    require(
        not mismatches,
        "SWEBENCH_VERIFIED_TASK_MANIFEST_METADATA_MISMATCH",
        "task manifest metadata does not match dataset source-of-truth",
        {"task_id": task_id, "mismatches": mismatches},
    )


def group_entries(index_doc: dict[str, Any]) -> dict[str, dict[str, dict[str, Any]]]:
    entries = index_doc.get("entries")
    require(isinstance(entries, list) and entries, "SWEBENCH_VERIFIED_INDEX_ENTRIES_EMPTY", "corpus index entries are empty")
    grouped: dict[str, dict[str, dict[str, Any]]] = defaultdict(dict)
    for entry in entries:
        task_id = str(entry.get("task_id") or "")
        category = str(entry.get("category") or "")
        require(bool(task_id and category), "SWEBENCH_VERIFIED_INDEX_ENTRY_KEY_MISSING", "index entry missing task/category", {"entry": entry})
        require(str(entry.get("language") or "") == "python", "SWEBENCH_VERIFIED_NON_PYTHON_ENTRY", "Verified export only supports Python entries", {"task_id": task_id, "category": category, "language": entry.get("language")})
        require(category in EXPECTED_CATEGORIES, "SWEBENCH_VERIFIED_UNEXPECTED_CATEGORY", "index entry category is not in the 8-category contract", {"task_id": task_id, "category": category})
        require(category not in grouped[task_id], "SWEBENCH_VERIFIED_DUPLICATE_TASK_CATEGORY", "index has duplicate task/category entry", {"task_id": task_id, "category": category})
        grouped[task_id][category] = entry
    for task_id, categories in grouped.items():
        missing = sorted(set(EXPECTED_CATEGORIES) - set(categories))
        require(
            not missing,
            "SWEBENCH_VERIFIED_TASK_CATEGORY_INCOMPLETE",
            "task is missing one or more mutation categories",
            {"task_id": task_id, "missing": missing},
        )
    return dict(sorted(grouped.items()))


def build_cleaned_row(
    *,
    corpus_root: Path,
    task_id: str,
    category: str,
    entry: dict[str, Any],
    dataset_row: dict[str, Any],
    task_manifest: dict[str, Any] | None,
) -> dict[str, Any]:
    mutation_patch, mutation_patch_path = read_patch_text(corpus_root, entry)
    official_patch = str(dataset_row["patch"])
    test_patch = str(dataset_row["test_patch"])
    official_patch_sha = sha256_prefixed_text(official_patch)
    test_patch_sha = sha256_prefixed_text(test_patch)
    require(
        str(entry.get("repo")) == str(dataset_row["repo"]),
        "SWEBENCH_VERIFIED_ENTRY_REPO_MISMATCH",
        "corpus index repo does not match dataset source-of-truth",
        {"task_id": task_id, "entry_repo": entry.get("repo"), "dataset_repo": dataset_row.get("repo")},
    )
    validate_task_manifest(task_manifest, dataset_row, task_id)
    row = {
        "row_id": f"{task_id}::{category}",
        "task_id": task_id,
        "repo": str(dataset_row["repo"]),
        "base_commit": str(dataset_row["base_commit"]),
        "language": "python",
        "mutation_category": category,
        "bucket": str(entry.get("bucket") or ""),
        "gold_patch": {"text": official_patch, "sha256": official_patch_sha},
        "test_patch": {"text": test_patch, "sha256": test_patch_sha},
        "patch": {"text": mutation_patch, "sha256": str(entry["patch_sha256"])},
        "source": {
            "dataset_name": str(dataset_row.get("dataset") or DEFAULT_DATASET_NAME),
            "dataset_split": str(dataset_row.get("dataset_split") or DEFAULT_DATASET_SPLIT),
            "corpus_patch_path": mutation_patch_path,
            "corpus_oracle_verdict_sha256": str(entry.get("oracle_verdict_sha256") or ""),
            "oracle_label_target_side_only": True,
            "target_oracle_labels_used_as_features": False,
        },
    }
    row["row_sha256"] = sha256_prefixed_text(canonical_json({key: row[key] for key in sorted(row)}))
    return row


def mounted_hits(value: Any, path: str = "$") -> list[dict[str, str]]:
    hits: list[dict[str, str]] = []
    if isinstance(value, dict):
        for key, child in value.items():
            hits.extend(mounted_hits(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for idx, child in enumerate(value):
            hits.extend(mounted_hits(child, f"{path}[{idx}]"))
    elif isinstance(value, str):
        for prefix in FORBIDDEN_MOUNT_PREFIXES:
            if value == prefix or value.startswith(prefix + "/"):
                hits.append({"path": path, "value": value})
    return hits


def run_export(args: argparse.Namespace, dataset_rows: list[dict[str, Any]] | None = None) -> dict[str, Any]:
    allow_non_aiwonder = bool(getattr(args, "allow_non_aiwonder_roots", False))
    corpus_root = reject_mounted_path(Path(args.corpus_root), "corpus_root", allow_non_aiwonder_roots=allow_non_aiwonder)
    output_root = reject_mounted_path(Path(args.output_root), "output_root", allow_non_aiwonder_roots=allow_non_aiwonder)
    output_root.mkdir(parents=True, exist_ok=True)
    index_path = corpus_root / "index.json"
    index_doc = read_json(index_path, "SWEBENCH_VERIFIED_CORPUS_INDEX_INVALID")
    require(index_doc.get("complete") is True, "SWEBENCH_VERIFIED_CORPUS_INCOMPLETE", "corpus index is not complete")
    tasks_dir = getattr(args, "tasks_dir", None) or index_doc.get("tasks_dir")
    task_manifest_root = (
        reject_mounted_path(Path(tasks_dir), "tasks_dir", allow_non_aiwonder_roots=allow_non_aiwonder)
        if tasks_dir
        else None
    )
    grouped = group_entries(index_doc)
    rows_by_id = dataset_index(dataset_rows if dataset_rows is not None else load_dataset_rows(args.dataset_name, args.dataset_split))

    cleaned_rows: list[dict[str, Any]] = []
    for task_id, categories in grouped.items():
        require(task_id in rows_by_id, "SWEBENCH_VERIFIED_DATASET_TASK_MISSING", "dataset source is missing a corpus task", {"task_id": task_id})
        task_manifest = load_task_manifest(task_manifest_root, task_id)
        for category in EXPECTED_CATEGORIES:
            cleaned_rows.append(
                build_cleaned_row(
                    corpus_root=corpus_root,
                    task_id=task_id,
                    category=category,
                    entry=categories[category],
                    dataset_row=rows_by_id[task_id],
                    task_manifest=task_manifest,
                )
            )

    expected_rows = int(getattr(args, "expected_rows", 0) or 0)
    if expected_rows:
        require(
            len(cleaned_rows) == expected_rows,
            "SWEBENCH_VERIFIED_CLEANED_ROW_COUNT_MISMATCH",
            "exported row count does not match --expected-rows",
            {"expected_rows": expected_rows, "actual_rows": len(cleaned_rows)},
        )
    category_counts = Counter(row["mutation_category"] for row in cleaned_rows)
    manifest = {
        "schema_version": 1,
        "artifact_kind": ARTIFACT_KIND,
        "created_at_utc": utc_now(),
        "dataset_name": args.dataset_name,
        "dataset_split": args.dataset_split,
        "row_count": len(cleaned_rows),
        "task_count": len(grouped),
        "category_counts": dict(sorted(category_counts.items())),
        "rows": cleaned_rows,
        "source_of_truth": {
            "corpus_root": str(corpus_root),
            "corpus_index": str(index_path),
            "tasks_dir": str(task_manifest_root) if task_manifest_root else None,
            "cleaned_manifest": str(output_root / "cleaned_manifest.json"),
            "readback": str(output_root / "swebench_verified_cleaned_manifest_readback.json"),
        },
        "doctrine": {
            "target_oracle_labels_used_as_features": False,
            "inner_llm_used": False,
            "flat_vector_concat_used": False,
            "slot_identity_preserved": True,
            "aiwonder_durable_root_required": not allow_non_aiwonder,
        },
    }
    hits = mounted_hits(manifest)
    require(
        not hits,
        "SWEBENCH_VERIFIED_CLEANED_MANIFEST_MOUNTED_PATH_HIT",
        "cleaned manifest contains forbidden mounted-drive paths",
        {"hits": hits[:20], "hit_count": len(hits)},
    )
    manifest_path = output_root / "cleaned_manifest.json"
    write_json_checked(manifest_path, manifest)
    readback = {
        "schema_version": 1,
        "artifact_kind": "swebench_verified_cleaned_manifest_readback",
        "created_at_utc": utc_now(),
        "source_of_truth": manifest["source_of_truth"],
        "checks": {
            "corpus_index_sha256": sha256_file(index_path),
            "cleaned_manifest_sha256": sha256_file(manifest_path),
            "row_count": len(cleaned_rows),
            "task_count": len(grouped),
            "category_counts": dict(sorted(category_counts.items())),
            "mounted_path_hit_count": 0,
            "all_patch_hashes_verified": True,
            "all_task_categories_complete": True,
        },
        "passes": True,
    }
    write_json_checked(output_root / "swebench_verified_cleaned_manifest_readback.json", readback)
    return {"ok": True, **readback}


def add_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--corpus-root", type=Path, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--evidence-root", type=Path, required=True)
    parser.add_argument("--tasks-dir", type=Path, default=None)
    parser.add_argument("--dataset-name", default=DEFAULT_DATASET_NAME)
    parser.add_argument("--dataset-split", default=DEFAULT_DATASET_SPLIT)
    parser.add_argument("--expected-rows", type=int, default=0)
    parser.add_argument("--allow-non-aiwonder-roots", action="store_true")


def command_swebench_verified_cleaned_manifest(args: argparse.Namespace) -> dict[str, Any]:
    return run_export(args)
