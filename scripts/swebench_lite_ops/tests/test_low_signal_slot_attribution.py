from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pytest

from ..python_low_signal_slot_attribution import (
    SlotAttributionError,
    auc_score,
    build_probe,
    compute_attributions,
    load_dataset,
    pearson_corr,
)


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
    categories = ["compile_error", "subtle_flip", "known_good", "delete_test_call"]
    rows = []
    for index, label in enumerate(y.tolist()):
        rows.append(
            {
                "row_index": index,
                "task_instance_id": f"task-{index // len(categories):03d}",
                "mutation_category": categories[index % len(categories)],
                "workspace_state": f"mutation_{categories[index % len(categories)]}",
                "oracle_all_passed": bool(label),
            }
        )
    return rows


def test_signal_slot_has_larger_attribution_than_noise_slot(tmp_path: Path):
    rng = np.random.default_rng(9)
    n = 400
    y = rng.integers(0, 2, size=n, dtype=np.int64)
    signal = y.reshape(-1, 1).astype(np.float32) + rng.normal(0.0, 0.05, size=(n, 3)).astype(np.float32)
    noise = rng.normal(0.0, 1.0, size=(n, 3)).astype(np.float32)
    X = np.concatenate([signal, noise], axis=1)
    feature_slices = {
        "e9": {"start": 0, "stop": 3},
        "e14": {"start": 3, "stop": 6},
    }
    write_dataset(tmp_path, X=X, y=y, rows=synthetic_rows(y), feature_slices=feature_slices)

    dataset = load_dataset(tmp_path, allow_non_aiwonder_roots=True)
    probe, Z_holdout, y_holdout = build_probe(
        dataset,
        holdout_fraction=0.25,
        seed=11,
        components_per_slot=3,
        ridge_alpha=1.0,
    )
    _, summaries, _ = compute_attributions(dataset, probe, Z_holdout, y_holdout, ["e9", "e14", "e2"])
    by_slot = {row["slot"]: row for row in summaries}

    assert by_slot["E9"]["measured"] is True
    assert by_slot["E14"]["measured"] is True
    assert by_slot["E2"]["measured"] is False
    assert by_slot["E2"]["reason"] == "temporal_slot_absent_from_static_ast_dataset"
    assert by_slot["E9"]["per_cell_median_attribution"] > 0.05
    assert by_slot["E9"]["per_cell_median_attribution"] > by_slot["E14"]["per_cell_median_attribution"]
    assert by_slot["E9"]["holdout_corr_delta_when_dropped"] is None


def test_load_dataset_fails_when_active_slice_is_missing(tmp_path: Path):
    X = np.ones((16, 4), dtype=np.float32)
    y = np.asarray([0, 1] * 8, dtype=np.int64)
    rows = synthetic_rows(y)
    write_dataset(tmp_path, X=X, y=y, rows=rows, feature_slices={"e9": {"start": 0, "stop": 2}})
    manifest = json.loads((tmp_path / "dataset_manifest.json").read_text(encoding="utf-8"))
    manifest["active_embedder_ids"] = ["e9", "e14"]
    (tmp_path / "dataset_manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(SlotAttributionError, match="SLOT_ATTR_ACTIVE_SLICE_MISSING"):
        load_dataset(tmp_path, allow_non_aiwonder_roots=True)


def test_metric_helpers_fail_closed_on_degenerate_inputs():
    y = np.ones((8,), dtype=np.int64)
    scores = np.linspace(0.0, 1.0, 8)

    assert pearson_corr(y, scores) is None
    assert auc_score(y, scores) is None
