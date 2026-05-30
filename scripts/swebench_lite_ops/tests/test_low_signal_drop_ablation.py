from __future__ import annotations

import json
from pathlib import Path

import numpy as np

from ..python_low_signal_drop_ablation import RUNNER_CONTRACT_BOUNDED_STATIC, RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION, run_pipeline
from ..python_low_signal_slot_attribution import load_dataset


def write_dataset(root: Path, *, X: np.ndarray, y: np.ndarray, rows: list[dict], feature_slices: dict[str, dict]) -> None:
    root.mkdir(parents=True, exist_ok=True)
    npz = root / "panel_oracle_dataset.npz"
    np.savez_compressed(npz, X=X.astype(np.float32), y=y.astype(np.int64))
    rows_path = root / "dataset_rows.jsonl"
    rows_path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")
    manifest = {
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


def synthetic_rows(y: np.ndarray) -> list[dict]:
    categories = ["compile_error", "subtle_flip", "known_good", "off_by_one"]
    rows = []
    for index, label in enumerate(y.tolist()):
        category = categories[index % len(categories)]
        rows.append(
            {
                "row_index": index,
                "task_instance_id": f"task-{index // len(categories):03d}",
                "mutation_category": category,
                "workspace_state": f"mutation_{category}",
                "oracle_all_passed": bool(label),
            }
        )
    return rows


def test_static_drop_ablation_measures_signal_slot_and_keeps_active_unreplaceable(tmp_path: Path):
    rng = np.random.default_rng(746)
    n = 480
    y = rng.integers(0, 2, size=n, dtype=np.int64)
    signal = y.reshape(-1, 1).astype(np.float32) + rng.normal(0.0, 0.04, size=(n, 4)).astype(np.float32)
    noise = rng.normal(0.0, 1.0, size=(n, 4)).astype(np.float32)
    X = np.concatenate([signal, noise], axis=1)
    feature_slices = {
        "e9": {"start": 0, "stop": 4},
        "e14": {"start": 4, "stop": 8},
    }
    dataset_root = tmp_path / "dataset"
    write_dataset(dataset_root, X=X, y=y, rows=synthetic_rows(y), feature_slices=feature_slices)

    import argparse

    args = argparse.Namespace(
        dataset_root=dataset_root,
        output_parent=tmp_path / "fsv",
        state_parent=tmp_path / "state-parent",
        output_root=tmp_path / "fsv" / "run-test",
        state_root=tmp_path / "state" / "run-test",
        run_id="run-test",
        candidate_slot=["e9", "e14", "e2"],
        holdout_fraction=0.25,
        components_per_slot=4,
        ridge_alpha=1.0,
        seed=33,
        runner_contract=RUNNER_CONTRACT_BOUNDED_STATIC,
        clean=True,
        allow_clean_state_root=True,
        allow_non_aiwonder_roots=True,
    )
    readback = run_pipeline(args)
    by_slot = {row["slot"]: row for row in readback["summary_rows"]}

    assert readback["passes"] is True
    assert by_slot["E9"]["measured"] is True
    assert by_slot["E14"]["measured"] is True
    assert by_slot["E2"]["measured"] is False
    assert by_slot["E2"]["reason"] == "temporal_slot_absent_from_static_ast_dataset"
    assert by_slot["E9"]["holdout_corr_delta_when_dropped"] > 0.1
    assert by_slot["E9"]["replaceable"] is False
    assert by_slot["E14"]["replaceable"] is False
    assert all(case["pass"] is True for case in readback["cases"])


def full_forward_cache_dataset(tmp_path: Path) -> Path:
    rng = np.random.default_rng(812)
    n = 360
    y = rng.integers(0, 2, size=n, dtype=np.int64)
    slots = ["e1", "e10", "e12", "e13", "e14", "e6", "e7", "e8", "e9"]
    blocks = []
    feature_slices = {}
    cursor = 0
    for slot in slots:
        if slot == "e9":
            block = y.reshape(-1, 1).astype(np.float32) + rng.normal(0.0, 0.05, size=(n, 2)).astype(np.float32)
        else:
            block = rng.normal(0.0, 1.0, size=(n, 2)).astype(np.float32)
        blocks.append(block)
        feature_slices[slot] = {"start": cursor, "stop": cursor + block.shape[1]}
        cursor += block.shape[1]
    dataset_root = tmp_path / "dataset-full"
    write_dataset(dataset_root, X=np.concatenate(blocks, axis=1), y=y, rows=synthetic_rows(y), feature_slices=feature_slices)
    return dataset_root


def test_forward_cache_contract_writes_model_artifacts_and_delta_is_authoritative(tmp_path: Path):
    dataset_root = full_forward_cache_dataset(tmp_path)

    import argparse

    args = argparse.Namespace(
        dataset_root=dataset_root,
        output_parent=tmp_path / "fsv",
        state_parent=tmp_path / "state-parent",
        output_root=tmp_path / "fsv" / "run-prod",
        state_root=tmp_path / "state" / "run-prod",
        run_id="run-prod",
        candidate_slot=["e9", "e14"],
        holdout_fraction=0.25,
        components_per_slot=2,
        ridge_alpha=1.0,
        seed=812,
        runner_contract=RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
        clean=True,
        allow_clean_state_root=True,
        allow_non_aiwonder_roots=True,
    )
    readback = run_pipeline(args)
    by_slot = {row["slot"]: row for row in readback["summary_rows"]}

    assert readback["passes"] is True
    assert readback["runner_contract"] == RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION
    assert by_slot["E9"]["authoritative_for_holdout_corr_delta"] is True
    assert by_slot["E14"]["authoritative_for_holdout_corr_delta"] is True
    assert len(readback["model_artifacts"]) == 3
    for artifact in readback["model_artifacts"]:
        assert Path(artifact["model_npz"]).is_file()
        assert Path(artifact["manifest_json"]).is_file()
        assert Path(artifact["state_model_npz"]).is_file()
        assert Path(artifact["state_manifest_json"]).is_file()
    assert all(case["pass"] is True for case in readback["cases"])


def test_forward_cache_contract_rejects_unknown_or_absent_slot(tmp_path: Path):
    dataset_root = full_forward_cache_dataset(tmp_path)

    import argparse
    import pytest

    args = argparse.Namespace(
        dataset_root=dataset_root,
        output_parent=tmp_path / "fsv",
        state_parent=tmp_path / "state-parent",
        output_root=tmp_path / "fsv" / "run-prod",
        state_root=tmp_path / "state" / "run-prod",
        run_id="run-prod",
        candidate_slot=["e9", "e2"],
        holdout_fraction=0.25,
        components_per_slot=2,
        ridge_alpha=1.0,
        seed=812,
        runner_contract=RUNNER_CONTRACT_FORWARD_CACHE_PRODUCTION,
        clean=True,
        allow_clean_state_root=True,
        allow_non_aiwonder_roots=True,
    )
    with pytest.raises(Exception, match="DROP_ABLATION_FORWARD_CACHE_SLOT_NOT_MATERIALIZED"):
        run_pipeline(args)


def test_loader_contract_rejects_flat_vector_source(tmp_path: Path):
    y = np.asarray([0, 1] * 16, dtype=np.int64)
    X = np.ones((32, 4), dtype=np.float32)
    rows = synthetic_rows(y)
    write_dataset(tmp_path, X=X, y=y, rows=rows, feature_slices={"e9": {"start": 0, "stop": 2}})
    manifest = json.loads((tmp_path / "dataset_manifest.json").read_text(encoding="utf-8"))
    manifest["flat_vector_concat_used"] = True
    (tmp_path / "dataset_manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    import pytest

    with pytest.raises(Exception, match="SLOT_ATTR_FLAT_VECTOR_CONCAT_USED"):
        load_dataset(tmp_path, allow_non_aiwonder_roots=True)
