from __future__ import annotations

from ..oracle_events import sha256_prefixed_text
from ..python_full_ast_corpus import patch_path_summary, skipped_auxiliary_patch_log


def diff(path: str) -> str:
    return (
        f"diff --git a/{path} b/{path}\n"
        f"--- a/{path}\n"
        f"+++ b/{path}\n"
        "@@ -1 +1 @@\n"
        "-old\n"
        "+new\n"
    )


def patch_obj(text: str) -> dict[str, str]:
    return {"text": text, "sha256": sha256_prefixed_text(text)}


def test_non_python_auxiliary_test_patch_has_no_ast_paths() -> None:
    row = {
        "row_id": "demo__repo-1::known_good",
        "task_id": "demo__repo-1",
        "repo": "demo/repo",
        "base_commit": "a" * 40,
        "mutation_category": "known_good",
        "gold_patch": patch_obj(diff("pkg/module.py")),
        "patch": patch_obj(diff("pkg/module.py")),
        "test_patch": patch_obj(diff("tests/data/valid_urls.txt")),
    }

    summary = patch_path_summary(row)

    assert summary["gold_new"] == ["pkg/module.py"]
    assert summary["mutation_new"] == ["pkg/module.py"]
    assert summary["test_new"] == []
    assert summary["test_old"] == []


def test_skipped_auxiliary_patch_log_records_no_python_reason() -> None:
    path_summary = {
        "gold_old": ["pkg/module.py"],
        "gold_new": ["pkg/module.py"],
        "mutation_old": ["pkg/module.py"],
        "mutation_new": ["pkg/module.py"],
        "test_old": [],
        "test_new": [],
    }

    log = skipped_auxiliary_patch_log(
        task_id="demo__repo-1",
        state="test_patch",
        field="test_patch",
        paths_key="test_new",
        path_summary=path_summary,
    )

    assert log["skipped"] is True
    assert log["skip_reason"] == "no_python_patch_paths"
    assert log["patch_path"] is None
    assert log["path_summary"] == path_summary
