from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from ..python_causal_delta_feasibility import CausalDeltaFeasibilityError, run_audit


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
        "artifacts": {
            "dataset_npz": str(npz),
            "state_dataset_rows_jsonl": str(rows_path),
        },
    }
    (root / "dataset_manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def make_args(tmp_path: Path, dataset_root: Path, **overrides: object) -> Namespace:
    values = {
        "dataset_root": dataset_root,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-feasibility",
        "baseline_category": "known_good",
        "holdout_fraction": 0.25,
        "seed": 17,
        "ridge_alpha": 1.0,
        "delta_epsilon": 1.0e-8,
        "active_slot_threshold_fraction": 0.40,
        "min_category_macro_auc": 0.70,
        "min_category_accuracy": 0.50,
        "max_median_active_slots": 2.0,
        "min_delta_rows": 8,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def sparse_separable_fixture(root: Path, *, noisy: bool = False) -> Path:
    rng = np.random.default_rng(773)
    tasks = [f"task-{idx:03d}" for idx in range(24)]
    categories = ["known_good", "off_by_one", "subtle_flip", "compile_error"]
    feature_slices = {
        "e1": {"start": 0, "stop": 3},
        "e6": {"start": 3, "stop": 6},
        "e9": {"start": 6, "stop": 9},
    }
    vectors: list[np.ndarray] = []
    labels: list[int] = []
    rows: list[dict] = []
    row_index = 0
    for task_index, task in enumerate(tasks):
        baseline = rng.normal(0.0, 0.02, size=9).astype(np.float32)
        for category in categories:
            vector = np.array(baseline, copy=True)
            label = 1
            if category == "off_by_one":
                vector[0] += 5.0
                label = task_index % 2
            elif category == "subtle_flip":
                vector[3] += 5.0
                label = (task_index + 1) % 2
            elif category == "compile_error":
                vector[6] += 5.0
                label = 0
            if noisy and category != "known_good":
                vector += np.ones(9, dtype=np.float32) * 2.5
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


def test_sparse_separable_deltas_recommend_full_training_and_mirror_state(tmp_path: Path):
    dataset_root = sparse_separable_fixture(tmp_path / "dataset")
    result = run_audit(make_args(tmp_path, dataset_root))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    state_manifest = Path(result["state_manifest"])
    delta_rows = Path(manifest["artifacts"]["mutation_delta_rows"]).read_text(encoding="utf-8").splitlines()

    assert result["full_cde_training_recommended"] is True
    assert manifest["decision"]["full_cde_training_recommended"] is True
    assert manifest["decision"]["promotion_allowed"] is False
    assert manifest["slot_identity_preserved"] is True
    assert manifest["flat_vector_concat_used"] is False
    assert manifest["target_labels_used_as_live_inputs"] is False
    assert manifest["sparsity"]["median_active_slot_count"] == 1.0
    assert manifest["separability"]["macro_one_vs_rest_auc"] >= 0.70
    assert len(delta_rows) == 72
    assert state_manifest.read_text(encoding="utf-8") == Path(result["manifest"]).read_text(encoding="utf-8")


def test_dense_unseparable_deltas_block_full_training(tmp_path: Path):
    dataset_root = sparse_separable_fixture(tmp_path / "dataset", noisy=True)
    result = run_audit(
        make_args(
            tmp_path,
            dataset_root,
            run_id="run-test-blocked",
            active_slot_threshold_fraction=0.01,
            max_median_active_slots=1.0,
        )
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["decision"]["full_cde_training_recommended"] is False
    assert "delta_not_sparse_enough_by_active_slot_count" in manifest["decision"]["full_training_blockers"]
    assert manifest["decision"]["promotion_allowed"] is False


def test_missing_baseline_fails_closed(tmp_path: Path):
    dataset_root = sparse_separable_fixture(tmp_path / "dataset")
    rows_path = dataset_root / "dataset_rows.jsonl"
    rows = [json.loads(line) for line in rows_path.read_text(encoding="utf-8").splitlines()]
    rows = [row for row in rows if not (row["task_instance_id"] == "task-000" and row["mutation_category"] == "known_good")]
    for index, row in enumerate(rows):
        row["row_index"] = index
    X = np.load(dataset_root / "panel_oracle_dataset.npz")["X"][1:]
    y = np.load(dataset_root / "panel_oracle_dataset.npz")["y"][1:]
    feature_slices = {
        "e1": {"start": 0, "stop": 3},
        "e6": {"start": 3, "stop": 6},
        "e9": {"start": 6, "stop": 9},
    }
    write_dataset(dataset_root, X=X, y=y, rows=rows, feature_slices=feature_slices)

    with pytest.raises(CausalDeltaFeasibilityError, match="CAUSAL_DELTA_BASELINE_ROW_MISSING_FOR_TASK"):
        run_audit(make_args(tmp_path, dataset_root, run_id="run-test-missing-baseline"))


def test_flat_vector_manifest_is_rejected(tmp_path: Path):
    dataset_root = sparse_separable_fixture(tmp_path / "dataset")
    manifest_path = dataset_root / "dataset_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["flat_vector_concat_used"] = True
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(CausalDeltaFeasibilityError, match="CAUSAL_DELTA_FLAT_VECTOR_CONCAT_USED"):
        run_audit(make_args(tmp_path, dataset_root, run_id="run-test-flat-vector"))
