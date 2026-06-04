from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from .. import python_embedder_candidate_mi_screen as screen
from ..python_embedder_candidate_mi_screen import CandidateMiScreenError, run_candidate_screen


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


def write_candidate(path: Path, rows: list[dict], values: np.ndarray, *, feature_field: str = "features", forbidden: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    out = []
    for idx, row in enumerate(rows):
        features = {
            "signal": float(values[idx, 0]),
            "aux": float(values[idx, 1]),
        }
        payload = {
            "task_id": row["task_instance_id"],
            "category": row["mutation_category"],
            feature_field: features,
        }
        if forbidden and idx == 0:
            payload["actual_oracle_pass"] = bool(row["oracle_all_passed"])
        out.append(payload)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in out), encoding="utf-8")


def fixture(tmp_path: Path) -> tuple[Path, Path, Path]:
    rng = np.random.default_rng(844)
    rows: list[dict] = []
    y: list[int] = []
    categories = ["compile_error", "off_by_one", "swap_variable"]
    for task_idx in range(48):
        for cat_idx, category in enumerate(categories):
            label = 1 if (task_idx + cat_idx) % 2 == 0 else 0
            rows.append(
                {
                    "row_index": len(rows),
                    "task_instance_id": f"task-{task_idx:03d}",
                    "repo": "demo/repo",
                    "mutation_category": category,
                    "workspace_state": f"mutation_{category}",
                    "oracle_all_passed": bool(label),
                }
            )
            y.append(label)
    y_arr = np.asarray(y, dtype=np.int64)
    weak_panel = rng.normal(0.0, 1.0, size=(len(rows), 6)).astype(np.float32)
    signal_candidate = np.column_stack(
        [
            y_arr.astype(np.float32) + rng.normal(0.0, 0.05, size=len(rows)),
            rng.normal(0.0, 1.0, size=len(rows)),
        ]
    ).astype(np.float32)
    noise_candidate = rng.normal(0.0, 1.0, size=(len(rows), 2)).astype(np.float32)
    dataset_root = tmp_path / "dataset"
    signal_path = tmp_path / "signal.jsonl"
    noise_path = tmp_path / "noise.jsonl"
    write_dataset(
        dataset_root,
        X=weak_panel,
        y=y_arr,
        rows=rows,
        feature_slices={"e1": {"start": 0, "stop": 3}, "e6": {"start": 3, "stop": 6}},
    )
    write_candidate(signal_path, rows, signal_candidate)
    write_candidate(noise_path, rows, noise_candidate)
    return dataset_root, signal_path, noise_path


def make_args(tmp_path: Path, dataset_root: Path, candidates: list[str], **overrides: object) -> Namespace:
    values = {
        "dataset_root": dataset_root,
        "candidate": candidates,
        "feature_field": "features",
        "output_parent": tmp_path / "fsv",
        "state_parent": tmp_path / "state",
        "run_id": "run-test-candidate-mi",
        "seed": 844,
        "replicate_seed": [],
        "require_seed_stability": False,
        "holdout_fraction": 0.25,
        "kraskov_neighbors": 3,
        "min_delta_bits": 0.005,
        "novelty_max_corr": 0.99,
        "run_mine": False,
        "mine_epochs": 8,
        "mine_batch_size": 32,
        "mine_learning_rate": 1.0e-4,
        "mine_hidden_dim": 16,
        "mine_device": "cpu",
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_candidate_screen_selects_signal_and_mirrors_state(tmp_path: Path) -> None:
    dataset_root, signal_path, noise_path = fixture(tmp_path)
    result = run_candidate_screen(
        make_args(
            tmp_path,
            dataset_root,
            [f"trace_signal={signal_path}", f"trace_noise={noise_path}"],
        )
    )
    manifest_path = Path(result["manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    summary_rows = [
        json.loads(line)
        for line in Path(manifest["artifacts"]["candidate_matrix_summaries"]).read_text(encoding="utf-8").splitlines()
    ]
    by_id = {row["candidate_id"]: row for row in summary_rows}
    state_manifest = Path(result["state_root"]) / manifest_path.name

    assert manifest["doctrine_policy"]["measurement_only"] is True
    assert manifest["doctrine_policy"]["slot_identity_preserved"] is True
    assert manifest["doctrine_policy"]["flat_vector_concat_used"] is False
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False
    assert by_id["trace_signal"]["augmented_delta_bits_vs_baseline"] > by_id["trace_noise"]["augmented_delta_bits_vs_baseline"]
    assert "trace_signal" in manifest["selected_candidates"]
    assert state_manifest.read_text(encoding="utf-8") == manifest_path.read_text(encoding="utf-8")


def test_candidate_screen_accepts_per_candidate_feature_fields(tmp_path: Path) -> None:
    dataset_root, _signal_path, noise_path = fixture(tmp_path)
    rows = [
        json.loads(line)
        for line in (dataset_root / "dataset_rows.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    y = np.load(dataset_root / "panel_oracle_dataset.npz")["y"]
    live_path = tmp_path / "live.jsonl"
    live_values = np.column_stack([y.astype(np.float32), np.ones_like(y, dtype=np.float32)])
    write_candidate(live_path, rows, live_values, feature_field="live_features")

    result = run_candidate_screen(
        make_args(
            tmp_path,
            dataset_root,
            [f"live_signal:live_features={live_path}", f"trace_noise={noise_path}"],
        )
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    summary_rows = [
        json.loads(line)
        for line in Path(manifest["artifacts"]["candidate_matrix_summaries"]).read_text(encoding="utf-8").splitlines()
    ]
    by_id = {row["candidate_id"]: row for row in summary_rows}

    assert by_id["live_signal"]["source_summary"]["feature_field"] == "live_features"
    assert by_id["trace_noise"]["source_summary"]["feature_field"] == "features"
    assert "live_signal" in manifest["selected_candidates"]


def test_candidate_screen_rejects_forbidden_per_candidate_feature_field(tmp_path: Path) -> None:
    dataset_root, signal_path, _noise_path = fixture(tmp_path)

    with pytest.raises(CandidateMiScreenError, match="CANDIDATE_MI_FEATURE_FIELD_FORBIDDEN"):
        run_candidate_screen(make_args(tmp_path, dataset_root, [f"bad:oracle_all_passed={signal_path}"]))


def test_candidate_screen_rejects_target_side_source_rows(tmp_path: Path) -> None:
    dataset_root, signal_path, _noise_path = fixture(tmp_path)
    rows = [
        json.loads(line)
        for line in (dataset_root / "dataset_rows.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    y = np.load(dataset_root / "panel_oracle_dataset.npz")["y"]
    values = np.column_stack([y.astype(np.float32), np.ones_like(y, dtype=np.float32)])
    bad_path = tmp_path / "bad.jsonl"
    write_candidate(bad_path, rows, values, forbidden=True)

    with pytest.raises(CandidateMiScreenError, match="CANDIDATE_MI_FORBIDDEN_KEY_IN_SOURCE_ROW"):
        run_candidate_screen(make_args(tmp_path, dataset_root, [f"bad={bad_path}"]))

    assert signal_path.is_file()


def test_candidate_screen_rejects_flat_vector_dataset(tmp_path: Path) -> None:
    dataset_root, signal_path, _noise_path = fixture(tmp_path)
    manifest_path = dataset_root / "dataset_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["flat_vector_concat_used"] = True
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    with pytest.raises(CandidateMiScreenError, match="CANDIDATE_MI_FLAT_VECTOR_CONCAT_USED"):
        run_candidate_screen(make_args(tmp_path, dataset_root, [f"signal={signal_path}"]))


def test_candidate_screen_seed_stability_gate_controls_greedy_selection(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    dataset_root, signal_path, noise_path = fixture(tmp_path)

    def fake_classifier_lower_bound_safe(*, X: object, y: object, label: str, seed: int, holdout_fraction: float) -> dict:
        del X, y, holdout_fraction
        bits = 0.10
        if "candidate:stable:standalone" in label:
            bits = 0.04
        elif "candidate:unstable:standalone" in label:
            bits = 0.04
        elif "candidate:stable:baseline_plus_candidate" in label:
            bits = 0.16
        elif "candidate:unstable:baseline_plus_candidate" in label:
            bits = 0.20
        elif "seed_stability:stable:augmented" in label:
            bits = 0.16
        elif "seed_stability:unstable:augmented" in label:
            bits = 0.20 if seed == 844 else 0.101
        elif "greedy_step_candidate:stable" in label:
            bits = 0.16
        elif "greedy_step_candidate:unstable" in label:
            bits = 0.20 if seed == 844 else 0.101
        return {
            "label": label,
            "valid": True,
            "seed": seed,
            "lower_bound_bits": bits,
            "target_entropy_bits": 1.0,
            "cross_entropy_bits": 1.0 - bits,
        }

    monkeypatch.setattr(screen, "classifier_lower_bound_safe", fake_classifier_lower_bound_safe)
    result = run_candidate_screen(
        make_args(
            tmp_path,
            dataset_root,
            [f"unstable={signal_path}", f"stable={noise_path}"],
            replicate_seed=[845],
            require_seed_stability=True,
            novelty_max_corr=1.1,
        )
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    summary_rows = [
        json.loads(line)
        for line in Path(manifest["artifacts"]["candidate_matrix_summaries"]).read_text(encoding="utf-8").splitlines()
    ]
    by_id = {row["candidate_id"]: row for row in summary_rows}
    seed_rows = [
        json.loads(line)
        for line in Path(manifest["artifacts"]["candidate_seed_stability"]).read_text(encoding="utf-8").splitlines()
    ]

    assert manifest["decision_rule"]["require_seed_stability"] is True
    assert manifest["decision_rule"]["replicate_seeds"] == [844, 845]
    assert by_id["unstable"]["utility_gate_pass"] is True
    assert by_id["unstable"]["seed_stability_gate_pass"] is False
    assert by_id["unstable"]["tier6_precheck_pass"] is False
    assert by_id["stable"]["seed_stability_gate_pass"] is True
    assert manifest["selected_candidates"] == ["stable"]
    assert {row["seed"] for row in seed_rows if row["candidate_id"] == "unstable"} == {844, 845}
