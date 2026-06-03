from __future__ import annotations

import argparse
import json
from pathlib import Path

import pytest

from ..python_swebench_verified_preflight import (
    SwebenchVerifiedPreflightError,
    run_preflight,
)


def row(instance_id: str, *, repo: str = "demo/repo", test_patch: str | None = None) -> dict[str, str]:
    return {
        "instance_id": instance_id,
        "repo": repo,
        "base_commit": "abc123",
        "patch": "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1 +1 @@\n-old\n+new\n",
        "test_patch": test_patch
        or (
            "diff --git a/tests/test_a.py b/tests/test_a.py\n"
            "--- a/tests/test_a.py\n"
            "+++ b/tests/test_a.py\n"
            "@@ -1 +1 @@\n"
            f"-old_{instance_id}\n"
            f"+new_{instance_id}\n"
        ),
        "problem_statement": "fix the demo bug",
        "FAIL_TO_PASS": json.dumps(["tests/test_a.py::test_fix"]),
        "PASS_TO_PASS": json.dumps(["tests/test_a.py::test_existing"]),
        "created_at": "2024-01-01T00:00:00Z",
        "version": "1.0",
        "difficulty": "15 min - 1 hour",
        "environment_setup_commit": "def456",
    }


def args(tmp_path: Path, expected_rows: int = 2) -> argparse.Namespace:
    lite = tmp_path / "lite"
    lite.mkdir()
    (lite / "demo__repo-1.json").write_text("{}", encoding="utf-8")
    return argparse.Namespace(
        dataset_name="princeton-nlp/SWE-bench_Verified",
        dataset_split="test",
        expected_rows=expected_rows,
        output_root=tmp_path / "out",
        tasks_output_dir=tmp_path / "tasks",
        lite_tasks_dir=lite,
        allow_existing_output=False,
    )


def test_preflight_materializes_verified_tasks_and_overlap(tmp_path: Path) -> None:
    summary = run_preflight(args(tmp_path), rows=[row("demo__repo-1"), row("demo__repo-2", repo="other/repo")])

    assert summary["passes"] is True
    assert summary["row_count"] == 2
    assert summary["generated_task_manifest_count"] == 2
    assert summary["lite_overlap_count"] == 1
    task = json.loads((tmp_path / "tasks" / "demo__repo-1.json").read_text(encoding="utf-8"))
    assert task["dataset"] == "princeton-nlp/SWE-bench_Verified"
    assert task["official_test_patch_sha256"].startswith("sha256:")
    assert (tmp_path / "out" / "dataset_rows.canonical.jsonl").read_text(encoding="utf-8").count("\n") == 2


def test_preflight_rejects_missing_required_field(tmp_path: Path) -> None:
    bad = row("demo__repo-1")
    bad["patch"] = ""

    with pytest.raises(SwebenchVerifiedPreflightError) as exc_info:
        run_preflight(args(tmp_path, expected_rows=1), rows=[bad])

    assert exc_info.value.code == "SWEBENCH_VERIFIED_REQUIRED_FIELD_MISSING"


def test_preflight_rejects_duplicate_instance_ids(tmp_path: Path) -> None:
    with pytest.raises(SwebenchVerifiedPreflightError) as exc_info:
        run_preflight(args(tmp_path), rows=[row("demo__repo-1"), row("demo__repo-1")])

    assert exc_info.value.code == "SWEBENCH_VERIFIED_DUPLICATE_INSTANCE_ID"


def test_preflight_rejects_malformed_test_lists(tmp_path: Path) -> None:
    bad = row("demo__repo-1")
    bad["FAIL_TO_PASS"] = json.dumps({"not": "a-list"})

    with pytest.raises(SwebenchVerifiedPreflightError) as exc_info:
        run_preflight(args(tmp_path, expected_rows=1), rows=[bad])

    assert exc_info.value.code == "SWEBENCH_VERIFIED_TEST_LIST_INVALID"
