from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import pytest

from ..python_swebench_verified_cleaned_manifest import (
    EXPECTED_CATEGORIES,
    VerifiedCleanedManifestError,
    run_export,
)
from ..oracle_events import sha256_prefixed_text


def write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def make_dataset_row(task_id: str = "demo__repo-1") -> dict[str, object]:
    patch = "diff --git a/pkg/mod.py b/pkg/mod.py\n--- a/pkg/mod.py\n+++ b/pkg/mod.py\n@@ -1 +1 @@\n-old\n+new\n"
    test_patch = (
        "diff --git a/tests/test_mod.py b/tests/test_mod.py\n"
        "--- a/tests/test_mod.py\n"
        "+++ b/tests/test_mod.py\n"
        "@@ -1 +1 @@\n"
        "-assert old\n"
        "+assert new\n"
    )
    return {
        "instance_id": task_id,
        "repo": "demo/repo",
        "base_commit": "a" * 40,
        "patch": patch,
        "test_patch": test_patch,
        "dataset": "princeton-nlp/SWE-bench_Verified",
        "dataset_split": "test",
    }


def mutation_patch(category: str) -> str:
    return (
        f"diff --git a/pkg/{category}.py b/pkg/{category}.py\n"
        f"--- a/pkg/{category}.py\n"
        f"+++ b/pkg/{category}.py\n"
        "@@ -1 +1 @@\n"
        "-value = 1\n"
        f"+value = '{category}'\n"
    )


def make_corpus(
    tmp_path: Path,
    *,
    omit_category: str | None = None,
    corrupt_hash: bool = False,
    crlf_category: str | None = None,
) -> tuple[Path, Path]:
    corpus = tmp_path / "corpus"
    task_id = "demo__repo-1"
    entries = []
    for category in EXPECTED_CATEGORIES:
        if category == omit_category:
            continue
        patch_text = mutation_patch(category)
        patch_bytes = patch_text.encode("utf-8")
        if category == crlf_category:
            patch_text = patch_text.replace("\n", "\r\n")
            patch_bytes = patch_text.encode("utf-8")
        patch_path = Path("mutations") / task_id / f"{category}.patch"
        full_path = corpus / patch_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_bytes(patch_bytes)
        entries.append(
            {
                "task_id": task_id,
                "repo": "demo/repo",
                "language": "python",
                "category": category,
                "bucket": "holdout",
                "patch_path": patch_path.as_posix(),
                "patch_sha256": "sha256:" + "0" * 64
                if corrupt_hash and category == "known_good"
                else "sha256:" + hashlib.sha256(patch_bytes).hexdigest(),
                "oracle_verdict_sha256": "sha256:" + "1" * 64,
            }
        )
    write_json(
        corpus / "index.json",
        {
            "schema_version": 1,
            "complete": True,
            "dataset_name": "princeton-nlp/SWE-bench_Verified",
            "dataset_split": "test",
            "tasks_dir": str(tmp_path / "tasks"),
            "entries": entries,
        },
    )
    tasks = tmp_path / "tasks"
    row = make_dataset_row(task_id)
    write_json(
        tasks / f"{task_id}.json",
        {
            "instance_id": task_id,
            "repo": row["repo"],
            "base_commit": row["base_commit"],
            "oracle_patch_sha256": sha256_prefixed_text(str(row["patch"])),
            "official_test_patch_sha256": sha256_prefixed_text(str(row["test_patch"])),
        },
    )
    return corpus, tasks


def args(tmp_path: Path, corpus: Path, tasks: Path | None = None, **overrides: object) -> argparse.Namespace:
    data = {
        "corpus_root": corpus,
        "output_root": tmp_path / "out",
        "tasks_dir": tasks,
        "dataset_name": "princeton-nlp/SWE-bench_Verified",
        "dataset_split": "test",
        "expected_rows": len(EXPECTED_CATEGORIES),
        "allow_non_aiwonder_roots": True,
    }
    data.update(overrides)
    return argparse.Namespace(**data)


def test_exports_verified_corpus_to_cleaned_manifest(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path)
    result = run_export(args(tmp_path, corpus, tasks), dataset_rows=[make_dataset_row()])

    manifest_path = tmp_path / "out" / "cleaned_manifest.json"
    readback_path = tmp_path / "out" / "swebench_verified_cleaned_manifest_readback.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    readback = json.loads(readback_path.read_text(encoding="utf-8"))

    assert result["passes"] is True
    assert manifest["row_count"] == 8
    assert manifest["task_count"] == 1
    assert manifest["category_counts"] == {category: 1 for category in EXPECTED_CATEGORIES}
    assert {row["mutation_category"] for row in manifest["rows"]} == set(EXPECTED_CATEGORIES)
    assert all(row["source"]["target_oracle_labels_used_as_features"] is False for row in manifest["rows"])
    assert readback["checks"]["all_patch_hashes_verified"] is True


def test_rejects_missing_task_category(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path, omit_category="swap_variable")
    with pytest.raises(VerifiedCleanedManifestError) as exc:
        run_export(args(tmp_path, corpus, tasks, expected_rows=0), dataset_rows=[make_dataset_row()])
    assert exc.value.code == "SWEBENCH_VERIFIED_TASK_CATEGORY_INCOMPLETE"


def test_rejects_patch_hash_mismatch(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path, corrupt_hash=True)
    with pytest.raises(VerifiedCleanedManifestError) as exc:
        run_export(args(tmp_path, corpus, tasks), dataset_rows=[make_dataset_row()])
    assert exc.value.code == "SWEBENCH_VERIFIED_PATCH_SHA_MISMATCH"


def test_preserves_crlf_patch_bytes_for_hash_verification(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path, crlf_category="wrong_file")
    result = run_export(args(tmp_path, corpus, tasks), dataset_rows=[make_dataset_row()])

    manifest = json.loads((tmp_path / "out" / "cleaned_manifest.json").read_text(encoding="utf-8"))
    wrong_file = next(row for row in manifest["rows"] if row["mutation_category"] == "wrong_file")

    assert result["passes"] is True
    assert "\r\n" in wrong_file["patch"]["text"]
    assert wrong_file["patch"]["sha256"] == "sha256:" + hashlib.sha256(wrong_file["patch"]["text"].encode("utf-8")).hexdigest()


def test_rejects_task_manifest_metadata_mismatch(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path)
    task_manifest = tasks / "demo__repo-1.json"
    data = json.loads(task_manifest.read_text(encoding="utf-8"))
    data["base_commit"] = "b" * 40
    write_json(task_manifest, data)
    with pytest.raises(VerifiedCleanedManifestError) as exc:
        run_export(args(tmp_path, corpus, tasks), dataset_rows=[make_dataset_row()])
    assert exc.value.code == "SWEBENCH_VERIFIED_TASK_MANIFEST_METADATA_MISMATCH"


def test_uses_index_tasks_dir_when_argument_omitted(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path)
    task_manifest = tasks / "demo__repo-1.json"
    data = json.loads(task_manifest.read_text(encoding="utf-8"))
    data["oracle_patch_sha256"] = "sha256:" + "2" * 64
    write_json(task_manifest, data)
    with pytest.raises(VerifiedCleanedManifestError) as exc:
        run_export(args(tmp_path, corpus, None), dataset_rows=[make_dataset_row()])
    assert exc.value.code == "SWEBENCH_VERIFIED_TASK_MANIFEST_METADATA_MISMATCH"


def test_rejects_non_aiwonder_root_without_test_override(tmp_path: Path) -> None:
    corpus, tasks = make_corpus(tmp_path)
    with pytest.raises(VerifiedCleanedManifestError) as exc:
        run_export(args(tmp_path, corpus, tasks, allow_non_aiwonder_roots=False), dataset_rows=[make_dataset_row()])
    assert exc.value.code == "SWEBENCH_VERIFIED_CLEANED_MANIFEST_NOT_AIWONDER_ROOT"
