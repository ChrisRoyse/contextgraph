from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_patch_lexical_candidate_features import PatchLexicalCandidateFeatureError, run_feature_build, sha256_file


def write_corpus(root: Path, *, hash_mismatch: bool = False) -> Path:
    entries = []
    rows = [
        ("task-001", "known_good", "repair_ok stable", True),
        ("task-001", "off_by_one", "broken_regression unstable", False),
        ("task-002", "subtle_flip", "predicate flipped condition", False),
    ]
    for task_id, category, signal, label in rows:
        rel = Path("mutations") / task_id / f"{category}.patch"
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        text = f"""diff --git a/{task_id}.py b/{task_id}.py
--- a/{task_id}.py
+++ b/{task_id}.py
@@ -1,2 +1,2 @@
-old_value = 1
+new_value = 2  # {signal} {category}
"""
        path.write_text(text, encoding="utf-8")
        entries.append(
            {
                "task_id": task_id,
                "repo": "demo/repo",
                "category": category,
                "bucket": "train",
                "patch_path": str(rel),
                "patch_sha256": "sha256:not-real" if hash_mismatch and not entries else sha256_file(path),
                "oracle_all_passed": label,
                "oracle_exception": None,
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
        "run_id": "run-test-patch-lexical-candidate",
        "hash_dim": 128,
        "hash_dims": None,
        "candidate_id": "patch_lexical_hash",
        "token_limit": 256,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_patch_lexical_candidate_features_emit_sanitized_sparse_rows(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus")
    result = run_feature_build(make_args(tmp_path, corpus_root))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    rows = [json.loads(line) for line in Path(result["candidate_features"]).read_text(encoding="utf-8").splitlines()]
    state_feature_path = Path(result["state_root"]) / "patch_lexical_candidate_features.jsonl"
    payload_text = Path(result["candidate_features"]).read_text(encoding="utf-8")

    assert manifest["passes"] is True
    assert manifest["row_count"] == 3
    assert manifest["feature_count"] > 20
    assert manifest["nonconstant_feature_count"] > 0
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False
    assert manifest["doctrine_policy"]["flat_vector_concat_used"] is False
    assert state_feature_path.read_text(encoding="utf-8") == payload_text
    assert rows[0]["task_instance_id"] == "task-001"
    assert rows[0]["mutation_category"] == "known_good"
    assert any(name.startswith("lexhash:") for name in rows[0]["features"])
    assert rows[0]["features"]["shape:token_count_log1p"] > 0.0
    assert "oracle_all_passed" not in payload_text
    assert "actual_oracle_pass" not in payload_text
    assert "pytest_exit_code" not in payload_text


def test_patch_lexical_candidate_features_emit_multiscale_candidate(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus")
    result = run_feature_build(
        make_args(
            tmp_path,
            corpus_root,
            run_id="run-test-patch-lexical-multiscale-candidate",
            hash_dims="128,256",
            candidate_id="patch_lexical_multiscale_hash",
        )
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    rows = [json.loads(line) for line in Path(result["candidate_features"]).read_text(encoding="utf-8").splitlines()]
    payload_text = Path(result["candidate_features"]).read_text(encoding="utf-8")

    assert manifest["passes"] is True
    assert manifest["candidate_id"] == "patch_lexical_multiscale_hash"
    assert manifest["lexical_summary"]["hash_dims"] == [128, 256]
    assert rows[0]["candidate_id"] == "patch_lexical_multiscale_hash"
    assert any(name.startswith("lexhash128:") for name in rows[0]["features"])
    assert any(name.startswith("lexhash256:") for name in rows[0]["features"])
    assert not any(name.startswith("lexhash:") for name in rows[0]["features"])
    assert "oracle_all_passed" not in payload_text
    assert "pytest_exit_code" not in payload_text


def test_patch_lexical_candidate_features_reject_hash_mismatch(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus", hash_mismatch=True)

    with pytest.raises(PatchLexicalCandidateFeatureError, match="PATCH_LEXICAL_CANDIDATE_PATCH_HASH_MISMATCH"):
        run_feature_build(make_args(tmp_path, corpus_root))


def test_patch_lexical_candidate_features_reject_mounted_path(tmp_path: Path) -> None:
    corpus_root = write_corpus(tmp_path / "corpus")
    index_path = corpus_root / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8"))
    index["entries"][0]["patch_path"] = "/mnt/c/bad.patch"
    index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(PatchLexicalCandidateFeatureError, match="PATCH_LEXICAL_CANDIDATE_MOUNTED_DRIVE_FORBIDDEN"):
        run_feature_build(make_args(tmp_path, corpus_root))
