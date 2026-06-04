from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_panelgraph_candidate_features import PanelGraphCandidateFeatureError, run_feature_build


def write_live_rows(path: Path, *, leak: bool = False) -> list[dict]:
    rows = [
        {
            "artifact_kind": "panelgraph_phase1_live_shape_row",
            "task_id": "task-001",
            "repo": "demo/repo",
            "category": "known_good",
            "workspace_state": "test_patch",
            "shape_class": "single_file_single_or_zero_chunk",
            "live_features": {
                "candidate_state_chunk_count": 11,
                "candidate_state_file_count": 1,
                "mapped_hunk_count": 1,
                "patch_hunk_count": 1,
                "patch_touched_chunk_count": 1,
                "patch_touched_file_count": 1,
                "unmapped_hunk_count": 0,
            },
            "change_kind_counts": {"modify": 1},
            "target_side_counts": {"old": 1},
            "primary_chunk_kind_counts": {"function": 1},
            "sample_touched_files": ["pkg/core.py"],
        },
        {
            "artifact_kind": "panelgraph_phase1_live_shape_row",
            "task_id": "task-001",
            "repo": "demo/repo",
            "category": "delete_test_call",
            "workspace_state": "mutation_delete_test_call",
            "shape_class": "single_file_multi_chunk",
            "live_features": {
                "candidate_state_chunk_count": 28,
                "candidate_state_file_count": 2,
                "mapped_hunk_count": 2,
                "patch_hunk_count": 2,
                "patch_touched_chunk_count": 4,
                "patch_touched_file_count": 1,
                "unmapped_hunk_count": 0,
            },
            "change_kind_counts": {"delete": 1, "modify": 1},
            "target_side_counts": {"new": 1, "old": 1},
            "primary_chunk_kind_counts": {"class": 1, "function": 3},
            "sample_touched_files": ["tests/test_core.py"],
        },
        {
            "artifact_kind": "panelgraph_phase1_live_shape_row",
            "task_id": "task-002",
            "repo": "demo/repo",
            "category": "wrong_file",
            "workspace_state": "mutation_wrong_file",
            "shape_class": "multi_file_multi_chunk",
            "live_features": {
                "candidate_state_chunk_count": 41,
                "candidate_state_file_count": 3,
                "mapped_hunk_count": 1,
                "patch_hunk_count": 2,
                "patch_touched_chunk_count": 5,
                "patch_touched_file_count": 2,
                "unmapped_hunk_count": 1,
            },
            "change_kind_counts": {"add": 1, "modify": 1},
            "target_side_counts": {"new": 2},
            "primary_chunk_kind_counts": {"function": 2, "module": 1},
            "sample_touched_files": ["pkg/core.py", "docs/usage.rst"],
        },
    ]
    if leak:
        rows[0]["actual_oracle_pass"] = True
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")
    return rows


def make_args(tmp_path: Path, source: Path, **overrides: object) -> Namespace:
    values = {
        "live_shape_rows": source,
        "output_parent": tmp_path / "fsv",
        "state_parent": tmp_path / "state",
        "run_id": "run-test-panelgraph-candidate",
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_panelgraph_candidate_features_write_screen_compatible_rows(tmp_path: Path) -> None:
    source = tmp_path / "panelgraph_phase1_live_shape_rows.jsonl"
    write_live_rows(source)

    result = run_feature_build(make_args(tmp_path, source))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    rows = [json.loads(line) for line in Path(result["candidate_features"]).read_text(encoding="utf-8").splitlines()]
    first_features = rows[0]["features"]
    state_feature_path = Path(result["state_root"]) / "panelgraph_candidate_features.jsonl"

    assert manifest["passes"] is True
    assert manifest["row_count"] == 3
    assert manifest["feature_count"] >= 30
    assert manifest["nonconstant_feature_count"] > 0
    assert manifest["doctrine_policy"]["measurement_only"] is True
    assert manifest["doctrine_policy"]["flat_vector_concat_used"] is False
    assert state_feature_path.read_text(encoding="utf-8") == Path(result["candidate_features"]).read_text(encoding="utf-8")
    assert rows[0]["task_instance_id"] == "task-001"
    assert rows[0]["mutation_category"] == "known_good"
    assert "features" in rows[0]
    assert first_features["live:patch_hunk_count"] == 1.0
    assert first_features["shape:single_file_single_or_zero_chunk:present"] == 1.0
    assert first_features["change_kind:modify:count"] == 1.0
    assert first_features["diff_side:old:present"] == 1.0
    assert first_features["chunk_kind:function:count"] == 1.0
    assert first_features["path_suffix:py:count"] == 1.0
    assert "actual_oracle_pass" not in json.dumps(rows)
    assert "oracle_all_passed" not in json.dumps(rows)


def test_panelgraph_candidate_features_reject_target_leakage(tmp_path: Path) -> None:
    source = tmp_path / "leaky_live_rows.jsonl"
    write_live_rows(source, leak=True)

    with pytest.raises(PanelGraphCandidateFeatureError, match="PANELGRAPH_CANDIDATE_FORBIDDEN_SOURCE_KEY"):
        run_feature_build(make_args(tmp_path, source))


def test_panelgraph_candidate_features_reject_duplicate_keys(tmp_path: Path) -> None:
    source = tmp_path / "dupe_live_rows.jsonl"
    rows = write_live_rows(source)
    rows[1]["category"] = rows[0]["category"]
    source.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")

    with pytest.raises(PanelGraphCandidateFeatureError, match="PANELGRAPH_CANDIDATE_DUPLICATE_ROW_KEY"):
        run_feature_build(make_args(tmp_path, source))
