from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from ..python_topology_h0_feasibility import TopologyH0FeasibilityError, load_dataset, run_audit


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


def density_fixture(root: Path) -> Path:
    rng = np.random.default_rng(779)
    feature_slices = {
        "e1": {"start": 0, "stop": 4},
        "e6": {"start": 4, "stop": 8},
    }
    categories = ["known_good", "off_by_one", "subtle_flip", "compile_error"]
    vectors: list[np.ndarray] = []
    labels: list[int] = []
    rows: list[dict] = []
    row_index = 0
    for task_index in range(80):
        task = f"task-{task_index:03d}"
        for category_index, category in enumerate(categories):
            label = 1 if category_index % 2 == 0 else 0
            scale = 0.04 if label == 1 else 1.20
            vector = rng.normal(0.0, scale, size=8).astype(np.float32)
            vectors.append(vector)
            labels.append(label)
            rows.append(
                {
                    "row_index": row_index,
                    "task_instance_id": task,
                    "mutation_category": category,
                    "workspace_state": "test_patch" if category == "known_good" else f"mutation_{category}",
                    "oracle_all_passed": bool(label),
                }
            )
            row_index += 1
    write_dataset(root, X=np.vstack(vectors), y=np.asarray(labels), rows=rows, feature_slices=feature_slices)
    return root


def make_args(tmp_path: Path, dataset_root: Path, **overrides: object) -> Namespace:
    values = {
        "dataset_root": dataset_root,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-topology-h0",
        "holdout_fraction": 0.25,
        "split_count": 1,
        "seed": 779,
        "components_per_slot": 3,
        "knn_k": 5,
        "batch_size": 64,
        "ridge_alpha": 1.0,
        "min_pearson_delta": -1.0,
        "min_topology_auc": 0.70,
        "min_holdout_rows": 40,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_density_topology_probe_writes_mirrored_fsv(tmp_path: Path):
    dataset_root = density_fixture(tmp_path / "dataset")
    result = run_audit(make_args(tmp_path, dataset_root))
    manifest_path = Path(result["manifest"])
    state_manifest_path = Path(result["state_manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    assert manifest["slot_identity_preserved"] is True
    assert manifest["flat_vector_concat_used"] is False
    assert manifest["target_labels_used_as_live_inputs"] is False
    assert manifest["topnets_slot_promotion_allowed"] is False
    assert manifest["summary"]["topology_only_mean"]["auc"] >= 0.70
    assert manifest["decision"]["promotion_allowed"] is False
    assert state_manifest_path.read_text(encoding="utf-8") == manifest_path.read_text(encoding="utf-8")
    split_metrics = Path(manifest["artifacts"]["split_metrics"]).read_text(encoding="utf-8").splitlines()
    assert len(split_metrics) == 1


def test_flat_vector_manifest_is_rejected(tmp_path: Path):
    dataset_root = density_fixture(tmp_path / "dataset")
    manifest_path = dataset_root / "dataset_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["flat_vector_concat_used"] = True
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(TopologyH0FeasibilityError, match="TOPOLOGY_H0_FLAT_VECTOR_CONCAT_USED"):
        load_dataset(dataset_root, allow_non_aiwonder_roots=True)


def test_missing_active_slice_fails_closed(tmp_path: Path):
    dataset_root = density_fixture(tmp_path / "dataset")
    manifest_path = dataset_root / "dataset_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["active_embedder_ids"] = ["e1", "e14"]
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(TopologyH0FeasibilityError, match="TOPOLOGY_H0_ACTIVE_SLICE_MISSING"):
        load_dataset(dataset_root, allow_non_aiwonder_roots=True)
