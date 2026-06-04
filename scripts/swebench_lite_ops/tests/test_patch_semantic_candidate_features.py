from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_patch_semantic_candidate_features import (
    PatchSemanticCandidateFeatureError,
    run_feature_build,
    sha256_file,
)


def write_corpus(root: Path, *, hash_mismatch: bool = False) -> Path:
    rows = [
        (
            "task-001",
            "known_good",
            Path("mutations/task-001/known_good.patch"),
            """diff --git a/tests/test_widget.py b/tests/test_widget.py
--- a/tests/test_widget.py
+++ b/tests/test_widget.py
@@ -1,4 +1,7 @@
 import pytest
 
 def test_widget(tmp_path):
-    assert widget(1) == 2
+    with pytest.raises(ValueError):
+        widget(None)
+    assert widget(2) == 3
""",
            True,
        ),
        (
            "task-001",
            "off_by_one",
            Path("mutations/task-001/off_by_one.patch"),
            """diff --git a/src/widget.py b/src/widget.py
--- a/src/widget.py
+++ b/src/widget.py
@@ -10,5 +10,5 @@
 def widget(value):
-    if value >= 10:
+    if value > 10:
         return value + 1
     return value - 1
""",
            False,
        ),
        (
            "task-002",
            "subtle_flip",
            Path("mutations/task-002/subtle_flip.patch"),
            """diff --git a/src/config.py b/src/config.py
--- a/src/config.py
+++ b/src/config.py
@@ -1,3 +1,5 @@
+import os
 ENABLED = True
-TIMEOUT = 30
+TIMEOUT = int(os.environ.get("TIMEOUT", "30"))
""",
            False,
        ),
    ]
    entries = []
    for task_id, category, rel, text, label in rows:
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        entries.append(
            {
                "task_id": task_id,
                "repo": "demo/repo",
                "category": category,
                "patch_path": str(rel),
                "patch_sha256": "sha256:not-real" if hash_mismatch and not entries else sha256_file(path),
                "oracle_all_passed": label,
                "pytest_exit_code": 0 if label else 1,
            }
        )
    index = {"schema_version": 1, "complete": True, "entries": entries}
    root.mkdir(parents=True, exist_ok=True)
    (root / "index.json").write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return root


def make_args(tmp_path: Path, corpus_root: Path, **overrides: object) -> Namespace:
    values = {
        "corpus_root": corpus_root,
        "output_parent": tmp_path / "fsv",
        "state_parent": tmp_path / "state",
        "run_id": "run-test-patch-semantic-candidate",
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_patch_semantic_candidate_features_emit_sanitized_rows(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus")
    result = run_feature_build(make_args(tmp_path, corpus_root))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    rows = [json.loads(line) for line in Path(result["candidate_features"]).read_text(encoding="utf-8").splitlines()]
    state_feature_path = Path(result["state_root"]) / "patch_semantic_candidate_features.jsonl"
    payload_text = Path(result["candidate_features"]).read_text(encoding="utf-8")

    assert manifest["passes"] is True
    assert manifest["row_count"] == 3
    assert manifest["feature_count"] > 50
    assert manifest["nonconstant_feature_count"] > 0
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False
    assert manifest["doctrine_policy"]["flat_vector_concat_used"] is False
    assert state_feature_path.read_text(encoding="utf-8") == payload_text
    assert rows[0]["task_instance_id"] == "task-001"
    assert rows[0]["mutation_category"] == "known_good"
    assert rows[0]["features"]["path:test:file_ratio"] == 1.0
    assert rows[0]["features"]["test_intent:pytest:add_count_log1p"] > 0.0
    assert rows[1]["features"]["construct:comparison:add_count_log1p"] > 0.0
    assert "oracle_all_passed" not in payload_text
    assert "actual_oracle_pass" not in payload_text
    assert "pytest_exit_code" not in payload_text


def test_patch_semantic_candidate_features_reject_hash_mismatch(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus", hash_mismatch=True)

    with pytest.raises(PatchSemanticCandidateFeatureError, match="PATCH_SEMANTIC_CANDIDATE_PATCH_HASH_MISMATCH"):
        run_feature_build(make_args(tmp_path, corpus_root))


def test_patch_semantic_candidate_features_reject_mounted_path(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus")
    index_path = corpus_root / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8"))
    index["entries"][0]["patch_path"] = "/mnt/d/bad.patch"
    index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(PatchSemanticCandidateFeatureError, match="PATCH_SEMANTIC_CANDIDATE_MOUNTED_DRIVE_FORBIDDEN"):
        run_feature_build(make_args(tmp_path, corpus_root))
