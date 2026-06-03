from __future__ import annotations

import json
from pathlib import Path

from ..python_bugsinpy_unclassified_taxonomy_triage import run_triage, semantic_patch_text, triage_classes
from ..python_bugsinpy_failure_mode_catalog_probe import parse_patch_files


def write_bug(root: Path, project: str, bug_id: str, patch: str) -> None:
    bug_dir = root / "projects" / project / "bugs" / bug_id
    bug_dir.mkdir(parents=True)
    (bug_dir / "bug.info").write_text(
        'python_version="3.11"\n'
        'buggy_commit_id="buggy"\n'
        'fixed_commit_id="fixed"\n'
        'test_file="tests/test_bug.py"\n',
        encoding="utf-8",
    )
    (bug_dir / "bug_patch.txt").write_text(patch, encoding="utf-8")


def test_triage_classes_detect_type_validation() -> None:
    patch = """diff --git a/pkg/encoder.py b/pkg/encoder.py
--- a/pkg/encoder.py
+++ b/pkg/encoder.py
@@ -1,2 +1,3 @@
-def encode(obj):
-    return obj
+def encode(obj):
+    if isinstance(obj, str):
+        return json.dumps(obj)
"""
    record = type("Record", (), {"project": "demo", "bug_id": "1", "bug_info": {}, "patch_text": patch})()
    classes, reasons = triage_classes(record, parse_patch_files(patch))
    assert "type_validation_serialization" in classes
    assert reasons["type_validation_serialization"]


def test_semantic_patch_text_drops_git_index_metadata() -> None:
    patch = """diff --git a/demo.py b/demo.py
index abc123..def456 100644
--- a/demo.py
+++ b/demo.py
@@ -1 +1 @@
-x = 1
+x = 2
"""
    assert "index abc123" not in semantic_patch_text(patch)


def test_run_triage_splits_other_rows(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/encoder.py b/demo/encoder.py
--- a/demo/encoder.py
+++ b/demo/encoder.py
@@ -1,2 +1,3 @@
-def encode(obj):
-    return obj
+def encode(obj):
+    if isinstance(obj, str):
+        return json.dumps(obj)
""",
    )
    write_bug(
        bugs_root,
        "demo",
        "2",
        """diff --git a/demo/plain.py b/demo/plain.py
--- a/demo/plain.py
+++ b/demo/plain.py
@@ -1,2 +1,2 @@
-x = y
+x = z
""",
    )
    out = tmp_path / "out"
    args = type(
        "Args",
        (),
        {"bugsinpy_root": bugs_root, "output_root": out, "sample_size": 0, "min_support": 1},
    )()
    manifest = run_triage(args)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    assert manifest["passes"] is True
    assert summary["counts"]["source_other_unclassified_count"] == 2
    assert summary["counts"]["assigned_count"] == 1
    assert summary["counts"]["unassigned_count"] == 1
    assert summary["taxonomy_support"]["type_validation_serialization"] == 1
