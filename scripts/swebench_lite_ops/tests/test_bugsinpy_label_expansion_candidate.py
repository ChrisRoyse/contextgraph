from __future__ import annotations

import json
from pathlib import Path

from ..python_auto_label_discovery import source_surface_labels
from ..python_bugsinpy_label_expansion_candidate import run_candidate_expansion


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


def write_registry(path: Path, labels: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as fh:
        for label_id in labels:
            fh.write(json.dumps({"label_id": label_id}) + "\n")


def test_source_surface_labels_emit_numeric_precision_family() -> None:
    labels = source_surface_labels(
        {
            "source_text": "return np.allclose(values.astype('float64'), expected, rtol=1e-7)",
            "relative_path": "pkg/math.py",
            "node_kind": "Return",
            "chunk_kind": "patch_hunk_line",
        }
    )
    assert "numeric_precision:floating_point_precision_surface" in labels
    assert "numeric_precision:dtype_or_numpy_surface" in labels
    assert "numeric_precision:tolerance_threshold_surface" in labels


def test_candidate_expansion_covers_float_precision_rows(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/metrics.py b/demo/metrics.py
--- a/demo/metrics.py
+++ b/demo/metrics.py
@@ -1,2 +1,3 @@
-def ok(values, expected):
-    return values == expected
+def ok(values, expected):
+    return np.allclose(values.astype('float64'), expected, rtol=1e-7)
""",
    )
    write_bug(
        bugs_root,
        "demo",
        "2",
        """diff --git a/demo/other.py b/demo/other.py
--- a/demo/other.py
+++ b/demo/other.py
@@ -1,2 +1,2 @@
-def name(value):
-    return value
+def name(value):
+    return str(value)
""",
    )
    registry = tmp_path / "accepted.jsonl"
    write_registry(registry, ["ast_surface:return_value_surface"])
    out = tmp_path / "out"
    args = type(
        "Args",
        (),
        {
            "bugsinpy_root": bugs_root,
            "target_class": "floating_point_precision",
            "accepted_registry": registry,
            "output_root": out,
            "sample_size": 0,
            "min_support": 1,
        },
    )()
    manifest = run_candidate_expansion(args)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    assert manifest["passes"] is True
    assert summary["counts"]["target_class_bug_count"] == 1
    assert summary["counts"]["candidate_registry_rows"] >= 2
    assert summary["fractions"]["candidate_class_uncovered_fraction"] == 0.0


def test_candidate_expansion_passes_with_support_without_full_coverage(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/metrics.py b/demo/metrics.py
--- a/demo/metrics.py
+++ b/demo/metrics.py
@@ -1,2 +1,3 @@
-def ok(values, expected):
-    return values == expected
+def ok(values, expected):
+    return np.allclose(values.astype('float64'), expected, rtol=1e-7)
""",
    )
    write_bug(
        bugs_root,
        "demo",
        "2",
        """diff --git a/demo/encoder.py b/demo/encoder.py
--- a/demo/encoder.py
+++ b/demo/encoder.py
@@ -1,3 +1,3 @@
 def encode(obj):
     if isinstance(obj, (str, int, float)):
-        return obj
+        return encoder(obj)
""",
    )
    registry = tmp_path / "accepted.jsonl"
    write_registry(registry, ["ast_surface:return_value_surface"])
    out = tmp_path / "out"
    args = type(
        "Args",
        (),
        {
            "bugsinpy_root": bugs_root,
            "target_class": "floating_point_precision",
            "accepted_registry": registry,
            "output_root": out,
            "sample_size": 0,
            "min_support": 1,
        },
    )()
    manifest = run_candidate_expansion(args)
    summary = json.loads((out / "summary.json").read_text(encoding="utf-8"))
    assert manifest["passes"] is True
    assert summary["decision"]["candidate_label_axis_defined"] is True
    assert summary["decision"]["full_class_coverage"] is False
    assert summary["counts"]["target_class_bug_count"] == 2
    assert summary["counts"]["candidate_class_covered_count"] == 1
