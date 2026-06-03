from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from ..python_patch_lexical_baseline import PatchLexicalBaselineError, load_dataset, run_audit, sha256_file


def write_dataset(root: Path, *, X: np.ndarray, y: np.ndarray, rows: list[dict], feature_slices: dict[str, dict]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    npz = root / "panel_oracle_dataset.npz"
    np.savez_compressed(npz, X=X.astype(np.float32), y=y.astype(np.int64))
    rows_path = root / "dataset_rows.jsonl"
    rows_path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")
    manifest = {
        "artifact_kind": "test_panel_oracle_dataset",
        "passes": True,
        "slot_identity_preserved": True,
        "flat_vector_concat_used": False,
        "row_count": int(X.shape[0]),
        "feature_dim": int(X.shape[1]),
        "active_embedder_ids": list(feature_slices),
        "feature_slices": feature_slices,
        "target_bits": 1.0,
        "artifacts": {
            "dataset_npz": str(npz),
            "state_dataset_rows_jsonl": str(rows_path),
        },
    }
    (root / "dataset_manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_corpus(root: Path, rows: list[dict]) -> None:
    patch_dir = root / "mutations"
    patch_dir.mkdir(parents=True, exist_ok=True)
    entries = []
    for row in rows:
        task = row["task_instance_id"]
        category = row["mutation_category"]
        label = bool(row["oracle_all_passed"])
        rel = Path("mutations") / task / f"{category}.patch"
        path = root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        signal = "repair_ok stable" if label else "broken_regression unstable"
        text = f"""diff --git a/{task}.py b/{task}.py
--- a/{task}.py
+++ b/{task}.py
@@ -1,2 +1,2 @@
-old_value = {row['row_index']}
+new_value = {row['row_index']}  # {signal} {category}
"""
        path.write_text(text, encoding="utf-8")
        entries.append(
            {
                "task_id": task,
                "repo": row["repo"],
                "category": category,
                "bucket": "train",
                "patch_path": str(rel),
                "patch_sha256": sha256_file(path),
                "oracle_all_passed": label,
            }
        )
    index = {
        "complete": True,
        "schema_version": 1,
        "corpus_version": "test",
        "corpus_sha256": "sha256:test",
        "entries": entries,
    }
    (root / "index.json").write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def lexical_fixture(root: Path) -> tuple[Path, Path]:
    rng = np.random.default_rng(757)
    feature_slices = {
        "e1": {"start": 0, "stop": 3},
        "e6": {"start": 3, "stop": 6},
    }
    categories = ["off_by_one", "subtle_flip", "swap_variable"]
    vectors: list[np.ndarray] = []
    labels: list[int] = []
    rows: list[dict] = []
    row_index = 0
    for task_index in range(36):
        task = f"task-{task_index:03d}"
        for category_index, category in enumerate(categories):
            label = 1 if (task_index + category_index) % 2 == 0 else 0
            vectors.append(rng.normal(0.0, 1.0, size=6).astype(np.float32))
            labels.append(label)
            rows.append(
                {
                    "row_index": row_index,
                    "task_instance_id": task,
                    "repo": "demo/repo",
                    "mutation_category": category,
                    "workspace_state": f"mutation_{category}",
                    "oracle_all_passed": bool(label),
                }
            )
            row_index += 1
    dataset_root = root / "dataset"
    corpus_root = root / "corpus"
    write_dataset(dataset_root, X=np.vstack(vectors), y=np.asarray(labels), rows=rows, feature_slices=feature_slices)
    write_corpus(corpus_root, rows)
    return dataset_root, corpus_root


def make_args(tmp_path: Path, dataset_root: Path, corpus_root: Path, **overrides: object) -> Namespace:
    values = {
        "dataset_root": dataset_root,
        "corpus_root": corpus_root,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-lexical",
        "split_count": 2,
        "holdout_fraction": 0.25,
        "seed": 757,
        "components_per_slot": 2,
        "hash_dim": 256,
        "token_limit": 1024,
        "ridge_alpha": 5.0,
        "min_augmented_correlation_delta": -1.0,
        "min_required_cell_delta": -1.0,
        "max_brier_regression": 1.0,
        "source_sample_count": 8,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_patch_lexical_baseline_writes_mirrored_fsv(tmp_path: Path) -> None:
    dataset_root, corpus_root = lexical_fixture(tmp_path)
    result = run_audit(make_args(tmp_path, dataset_root, corpus_root))
    manifest_path = Path(result["manifest"])
    state_manifest_path = Path(result["state_manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    assert manifest["policy"]["slot_identity_preserved"] is True
    assert manifest["policy"]["flat_vector_concat_used"] is False
    assert manifest["policy"]["target_labels_used_as_live_inputs"] is False
    assert manifest["decision"]["bounded_triage_complete"] is True
    assert manifest["lexical_feature_summary"]["nondegenerate_feature_columns"] > 0
    assert state_manifest_path.read_text(encoding="utf-8") == manifest_path.read_text(encoding="utf-8")
    split_rows = Path(manifest["artifacts"]["split_metrics"]).read_text(encoding="utf-8").splitlines()
    cell_rows = Path(manifest["artifacts"]["cell_metrics"]).read_text(encoding="utf-8").splitlines()
    assert len(split_rows) == 2
    assert len(cell_rows) == 6


def test_flat_vector_manifest_is_rejected(tmp_path: Path) -> None:
    dataset_root, _corpus_root = lexical_fixture(tmp_path)
    manifest_path = dataset_root / "dataset_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["flat_vector_concat_used"] = True
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(PatchLexicalBaselineError, match="PATCH_LEXICAL_FLAT_VECTOR_CONCAT_USED"):
        load_dataset(dataset_root, allow_non_aiwonder_roots=True)


def test_patch_hash_mismatch_fails_closed(tmp_path: Path) -> None:
    dataset_root, corpus_root = lexical_fixture(tmp_path)
    index_path = corpus_root / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8"))
    index["entries"][0]["patch_sha256"] = "sha256:not-the-real-hash"
    index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(PatchLexicalBaselineError, match="PATCH_LEXICAL_PATCH_HASH_MISMATCH"):
        run_audit(make_args(tmp_path, dataset_root, corpus_root))
