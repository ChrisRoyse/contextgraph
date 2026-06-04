from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest

from ..python_panel_oracle_mi_probe import (
    ACTIVE_EMBEDDERS,
    CodeStateAccumulator,
    PanelMiError,
    finalize_dataset,
    standardize_and_reduce,
)


def test_finalize_dataset_fails_closed_when_active_slot_dim_missing(tmp_path: Path) -> None:
    ingest_manifest = {
        "slot_dims": {slot: 3 for slot in ACTIVE_EMBEDDERS if slot != "e1"},
        "workspace_state_selector": "candidate",
        "record_counts": {"workspace_state_filter_skipped_records": 128},
        "slot_record_counts": {"e10": 4},
    }
    accumulators = {
        ("task-001", "known_good", "test_patch"): CodeStateAccumulator(
            task_instance_id="task-001",
            mutation_category="known_good",
            workspace_state="test_patch",
            oracle_all_passed=True,
            repo="demo/repo",
        )
    }

    with pytest.raises(PanelMiError, match="PANEL_MI_SLOT_DIM_MISSING"):
        finalize_dataset(
            accumulators=accumulators,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            input_manifest={"oracle": {}, "forward_cache": {}},
            ingest_manifest=ingest_manifest,
        )


def test_finalize_dataset_fails_closed_when_active_slot_dim_invalid(tmp_path: Path) -> None:
    ingest_manifest = {
        "slot_dims": {slot: 3 for slot in ACTIVE_EMBEDDERS},
        "workspace_state_selector": "candidate",
    }
    ingest_manifest["slot_dims"]["e1"] = 0
    accumulators = {
        ("task-001", "known_good", "test_patch"): CodeStateAccumulator(
            task_instance_id="task-001",
            mutation_category="known_good",
            workspace_state="test_patch",
            oracle_all_passed=True,
            repo="demo/repo",
        )
    }

    with pytest.raises(PanelMiError, match="PANEL_MI_SLOT_DIM_INVALID"):
        finalize_dataset(
            accumulators=accumulators,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            input_manifest={"oracle": {}, "forward_cache": {}},
            ingest_manifest=ingest_manifest,
        )


def test_standardize_reduce_is_independent_of_evaluation_seed() -> None:
    rng = np.random.default_rng(759)
    X = rng.normal(0.0, 1.0, size=(48, 32)).astype(np.float32)
    y = np.asarray([idx % 2 for idx in range(48)], dtype=np.int64)

    reduced_a, info_a = standardize_and_reduce(X, y, max_components=8, seed=844)
    reduced_b, info_b = standardize_and_reduce(X, y, max_components=8, seed=845)

    assert np.allclose(reduced_a, reduced_b)
    assert info_a["method"] == info_b["method"]
    assert info_a["output_dim"] == 8
    assert info_b["output_dim"] == 8
    assert info_a["reduction_seed_policy"] == "fixed_independent_of_evaluation_seed"
    assert info_b["reduction_seed_policy"] == "fixed_independent_of_evaluation_seed"
