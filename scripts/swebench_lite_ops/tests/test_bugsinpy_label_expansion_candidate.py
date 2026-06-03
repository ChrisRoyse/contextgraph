from __future__ import annotations

import json
from pathlib import Path

from ..python_auto_label_discovery import source_surface_labels
from ..python_bugsinpy_label_expansion_candidate import registry_class_hits, run_candidate_expansion


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


def test_source_surface_labels_emit_resource_lifecycle_family() -> None:
    labels = source_surface_labels(
        {
            "source_text": "with open(path) as handle: session.close()",
            "relative_path": "pkg/io.py",
            "node_kind": "With",
            "chunk_kind": "patch_hunk_line",
        }
    )
    assert "resource_lifecycle:resource_leak_surface" in labels
    assert "resource_lifecycle:context_manager_surface" in labels
    assert "resource_lifecycle:open_close_surface" in labels
    assert "resource_lifecycle:resource_handle_surface" in labels


def test_source_surface_labels_emit_state_aliasing_family() -> None:
    labels = source_surface_labels(
        {
            "source_text": "items = values.copy(); items.append(next_value)",
            "relative_path": "pkg/state.py",
            "node_kind": "Assign",
            "chunk_kind": "patch_hunk_line",
        }
    )
    assert "state_aliasing:memory_aliasing_mutation_surface" in labels
    assert "state_aliasing:copy_view_surface" in labels
    assert "state_aliasing:mutable_container_surface" in labels


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


def test_candidate_expansion_covers_resource_rows(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/io.py b/demo/io.py
--- a/demo/io.py
+++ b/demo/io.py
@@ -1,2 +1,4 @@
 def read(path):
-    return open(path).read()
+    with open(path) as handle:
+        return handle.read()
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
            "target_class": "resource_leak",
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
    assert summary["counts"]["candidate_class_covered_count"] == 1
    candidate_rows = (out / "candidate_registry_expansion.jsonl").read_text(encoding="utf-8")
    assert "resource_lifecycle:resource_leak_surface" in candidate_rows


def test_candidate_expansion_covers_state_aliasing_rows(tmp_path: Path) -> None:
    bugs_root = tmp_path / "BugsInPy"
    write_bug(
        bugs_root,
        "demo",
        "1",
        """diff --git a/demo/state.py b/demo/state.py
--- a/demo/state.py
+++ b/demo/state.py
@@ -1,2 +1,4 @@
 def add(values, item):
-    values.append(item)
+    copied = values.copy()
+    copied.append(item)
+    return copied
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
            "target_class": "memory_aliasing_mutation",
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
    assert summary["counts"]["candidate_class_covered_count"] == 1
    candidate_rows = (out / "candidate_registry_expansion.jsonl").read_text(encoding="utf-8")
    assert "state_aliasing:memory_aliasing_mutation_surface" in candidate_rows


def test_state_aliasing_registry_hits_ignore_variable_reference_swap() -> None:
    hits = registry_class_hits(
        "memory_aliasing_mutation",
        ["mutation_micro:variable_reference_swap", "state_aliasing:shared_reference_surface"],
    )
    assert hits == ["state_aliasing:shared_reference_surface"]


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
    assert summary["counts"]["unassigned_rows"] == 1
    unassigned = json.loads((out / "unassigned_rows.jsonl").read_text(encoding="utf-8"))
    assert unassigned["retriage_reason"] == "context_only_float_type_reference"
