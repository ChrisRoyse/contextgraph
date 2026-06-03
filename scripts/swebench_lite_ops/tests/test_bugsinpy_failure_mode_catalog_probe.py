from __future__ import annotations

import json
from pathlib import Path

from ..python_bugsinpy_failure_mode_catalog_probe import (
    classify_bug,
    iter_bugs,
    label_has_registry_token,
    parse_patch_files,
    registry_hits_by_bug_class,
    run_probe,
)


def write_bug(root: Path, project: str, bug_id: str, patch: str, info: str | None = None) -> None:
    bug_dir = root / "projects" / project / "bugs" / bug_id
    bug_dir.mkdir(parents=True)
    (bug_dir / "bug.info").write_text(
        info
        or 'python_version="3.11"\n'
        'buggy_commit_id="buggy"\n'
        'fixed_commit_id="fixed"\n'
        'test_file="tests/test_bug.py"\n',
        encoding="utf-8",
    )
    (bug_dir / "bug_patch.txt").write_text(patch, encoding="utf-8")


def write_registry(path: Path, labels: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for label in labels:
            fh.write(json.dumps({"label_id": label}) + "\n")


def test_classify_bug_detects_async_and_resource() -> None:
    patch = """diff --git a/pkg/server.py b/pkg/server.py
--- a/pkg/server.py
+++ b/pkg/server.py
@@ -1,2 +1,4 @@
-result = handler()
+async def call_handler():
+    result = await handler()
+    resource.close()
"""
    record = type(
        "Record",
        (),
        {
            "project": "demo",
            "bug_id": "1",
            "bug_info": {"test_file": "tests/test_async.py"},
            "patch_text": patch,
        },
    )()
    files = parse_patch_files(patch)
    classes, reasons = classify_bug(record, files)
    assert "async_await_misuse" in classes
    assert "resource_leak" in classes
    assert reasons["async_await_misuse"]


def test_run_probe_reports_class_specific_gap(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/worker.py b/demo/worker.py
--- a/demo/worker.py
+++ b/demo/worker.py
@@ -1,2 +1,3 @@
-def work():
-    return run()
+async def work():
+    return await run()
""",
    )
    write_bug(
        bugs_root,
        "demo",
        "2",
        """diff --git a/demo/io.py b/demo/io.py
--- a/demo/io.py
+++ b/demo/io.py
@@ -1,2 +1,4 @@
 def read(path):
-    return open(path).read()
+    with open(path) as fh:
+        return fh.read()
""",
    )
    registry = tmp_path / "accepted.jsonl"
    write_registry(registry, ["ast_surface:function_contract_surface", "ast_surface:call_api_surface"])
    out = tmp_path / "out"

    args = type(
        "Args",
        (),
        {
            "bugsinpy_root": bugs_root,
            "accepted_registry": registry,
            "output_root": out,
            "sample_size": 50,
        },
    )()
    manifest = run_probe(args)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    assert manifest["line_counts"]["bug_rows"] == 2
    assert summary["counts"]["sample_count"] == 2
    assert summary["fractions"]["generic_level0_unlabeled_fraction"] == 0.0
    assert summary["fractions"]["class_specific_unlabeled_fraction"] == 1.0
    assert summary["decision"]["classification"] == "structural_blind_spot_major_corpus_expansion_required"


def test_run_probe_counts_class_specific_registry_hit(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/worker.py b/demo/worker.py
--- a/demo/worker.py
+++ b/demo/worker.py
@@ -1,2 +1,3 @@
-def work():
-    return run()
+async def work():
+    return await run()
""",
    )
    registry = tmp_path / "accepted.jsonl"
    write_registry(registry, ["ast_surface:function_contract_surface", "runtime_surface:async_missing_await"])
    out = tmp_path / "out"
    args = type(
        "Args",
        (),
        {
            "bugsinpy_root": bugs_root,
            "accepted_registry": registry,
            "output_root": out,
            "sample_size": 50,
        },
    )()
    run_probe(args)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    assert summary["fractions"]["class_specific_unlabeled_fraction"] == 0.0
    assert summary["decision"]["classification"] == "catalog_gaps_minor_defer"


def test_registry_token_matching_avoids_generic_substrings() -> None:
    assert label_has_registry_token("runtime_surface:async_missing_await", "async")
    assert label_has_registry_token("runtime_surface:async_missing_await", "missing_await")
    assert not label_has_registry_token("dominant_surface:numeric_boundary_or_comparison_surface", "nan")
    hits = registry_hits_by_bug_class(
        {
            "dominant_surface:numeric_boundary_or_comparison_surface",
            "code_state_identity:mutation_category:off_by_one",
            "ast_surface:import_dependency_surface",
        }
    )
    assert hits["floating_point_precision"] == []
    assert hits["memory_aliasing_mutation"] == []
    assert hits["test_only_pass_mocked_dependency"] == []


def test_iter_bugs_skips_missing_patch(tmp_path: Path) -> None:
    bug_dir = tmp_path / "BugsInPy" / "projects" / "demo" / "bugs" / "1"
    bug_dir.mkdir(parents=True)
    (bug_dir / "bug.info").write_text('python_version="3.11"\n', encoding="utf-8")
    write_bug(tmp_path / "BugsInPy", "demo", "2", "diff --git a/a.py b/a.py\n")
    records = iter_bugs(tmp_path / "BugsInPy")
    assert [record.record_id for record in records] == ["demo:2"]
