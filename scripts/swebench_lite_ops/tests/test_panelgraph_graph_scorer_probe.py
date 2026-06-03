from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from ..python_panelgraph_graph_scorer_probe import PanelGraphGraphScorerError, run_probe


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def sha256_bytes(payload: bytes) -> str:
    import hashlib

    return "sha256:" + hashlib.sha256(payload).hexdigest()


def vector_sha(values: list[float]) -> str:
    return sha256_bytes(np.asarray(values, dtype="<f4").tobytes())


def eval_row(split: str, task_id: str, actual: bool, row_score: float, chunks: list[str], shape: str = "multi_file_multi_chunk") -> dict:
    return {
        "schema_version": 2,
        "artifact_kind": "panelgraph_phase1_offline_eval_row",
        "split": split,
        "task_id": task_id,
        "repo": "org/repo",
        "category": "known_good" if actual else "off_by_one",
        "shape_class": shape,
        "patch_touched_file_count": 2 if shape == "multi_file_multi_chunk" else 1,
        "patch_touched_chunk_count": len(chunks),
        "candidate_state_chunk_count": len(chunks) + 3,
        "primary_chunk_ids": ["sha256:" + chunk for chunk in chunks],
        "predicted_oracle_pass": row_score,
        "actual_oracle_pass": actual,
        "absolute_error": abs(row_score - (1.0 if actual else 0.0)),
        "offline_eval_labels_not_live_inputs": True,
    }


def seed_eval_rows(root: Path) -> Path:
    rows = [
        eval_row("calibration", "cal-pass-a", True, 0.45, ["pass-a-1", "pass-a-2"]),
        eval_row("calibration", "cal-pass-b", True, 0.40, ["pass-b-1", "pass-b-2"]),
        eval_row("calibration", "cal-fail-a", False, 0.60, ["fail-a-1", "fail-a-2"]),
        eval_row("calibration", "cal-fail-b", False, 0.55, ["fail-b-1", "fail-b-2"]),
        eval_row("holdout", "hold-pass", True, 0.35, ["hold-pass-1", "hold-pass-2"]),
        eval_row("holdout", "hold-fail", False, 0.65, ["hold-fail-1", "hold-fail-2"]),
    ]
    path = root / "panelgraph_phase1_offline_eval_rows.jsonl"
    write_jsonl(path, rows)
    return path


def seed_chunk_scores(path: Path, eval_rows: Path, *, leak_target: bool = False) -> Path:
    rows = []
    for raw in eval_rows.read_text(encoding="utf-8").splitlines():
        row = json.loads(raw)
        actual = bool(row["actual_oracle_pass"])
        score = 0.75 if actual else 0.25
        for chunk_id in row["primary_chunk_ids"]:
            payload = {
                "schema_version": 1,
                "artifact_kind": "panelgraph_per_chunk_verdict_row",
                "formula_version": "panelgraph_chunk_verdict_v1",
                "task_id": row["task_id"],
                "category": row["category"],
                "chunk_id": chunk_id,
                "predicted_oracle_pass": score,
            }
            if leak_target:
                payload["actual_oracle_pass"] = actual
            rows.append(payload)
    write_jsonl(path, rows)
    return path


def seed_active_index(root: Path, eval_rows: Path, *, omit_chunk: str | None = None) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    active_index = root / "active_index.jsonl"
    record_path = root / "e9.jsonl"
    index_rows = []
    offset = 0
    with record_path.open("wb") as handle:
        for raw in eval_rows.read_text(encoding="utf-8").splitlines():
            row = json.loads(raw)
            actual = bool(row["actual_oracle_pass"])
            base = 1.0 if actual else -1.0
            for chunk_id in row["primary_chunk_ids"]:
                if chunk_id == omit_chunk:
                    continue
                values = [base, base * 0.5, 0.25]
                payload = {
                    "cache_key_sha256": sha256_bytes(f"{chunk_id}:e9".encode()),
                    "key": {"chunk_id": chunk_id, "embedder_id": "e9"},
                    "output": {"vector": values},
                }
                raw_payload = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
                handle.write(raw_payload + b"\n")
                index_rows.append(
                    {
                        "schema_version": 1,
                        "artifact_kind": "python_ast_chunk_forward_v3_index",
                        "chunk_id": chunk_id,
                        "embedder_id": "e9",
                        "record_path": str(record_path),
                        "byte_offset": offset,
                        "byte_len": len(raw_payload),
                        "record_sha256": sha256_bytes(raw_payload),
                        "vector_sha256": vector_sha(values),
                        "vector_len": len(values),
                    }
                )
                offset += len(raw_payload) + 1
    write_jsonl(active_index, index_rows)
    return active_index


def make_args(tmp_path: Path, eval_rows: Path, chunk_scores: Path, active_index: Path, **overrides: object) -> Namespace:
    values = {
        "phase1_root": eval_rows.parent,
        "chunk_verdict_root": chunk_scores.parent,
        "phase1_eval_rows": eval_rows,
        "per_chunk_verdict_rows": chunk_scores,
        "active_index": active_index,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-graph-scorer",
        "active_embedder_ids": "e9",
        "ridge_alpha": 1.0,
        "primary_variant": "graph_vector_summary",
        "min_holdout_pearson_lift": 0.005,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_graph_scorer_writes_holdout_delta_and_state_mirror(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    chunk_scores = seed_chunk_scores(tmp_path / "chunk" / "panelgraph_per_chunk_verdict_rows.jsonl", eval_rows)
    active_index = seed_active_index(tmp_path / "index", eval_rows)

    result = run_probe(make_args(tmp_path, eval_rows, chunk_scores, active_index))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    predictions = [
        json.loads(line)
        for line in (Path(result["fsv_run_root"]) / "panelgraph_graph_scorer_predictions.jsonl").read_text(encoding="utf-8").splitlines()
    ]

    assert result["passes"] is True
    assert manifest["policy"]["measurement_only"] is True
    assert manifest["policy"]["slot_identity_preserved"] is True
    assert manifest["policy"]["flat_vector_concat_used"] is False
    assert manifest["policy"]["target_labels_used_as_live_inputs"] is False
    assert "holdout_pearson_delta_vs_row_level" in manifest["decision"]
    assert manifest["summary"]["metrics"]["row_level_baseline"]["holdout"]["row_count"] == 2
    assert Path(result["state_manifest"]).read_text(encoding="utf-8") == Path(result["manifest"]).read_text(encoding="utf-8")
    assert all("actual_oracle_pass" not in row for row in predictions)
    assert all("graph_vector_summary" in row["scores"] for row in predictions)


def test_graph_scorer_rejects_incomplete_active_index(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    chunk_scores = seed_chunk_scores(tmp_path / "chunk" / "panelgraph_per_chunk_verdict_rows.jsonl", eval_rows)
    active_index = seed_active_index(tmp_path / "index", eval_rows, omit_chunk="sha256:hold-fail-2")

    with pytest.raises(PanelGraphGraphScorerError, match="PANELGRAPH_GRAPH_SCORER_ACTIVE_INDEX_COVERAGE_INCOMPLETE"):
        run_probe(make_args(tmp_path, eval_rows, chunk_scores, active_index, run_id="run-test-missing-vector"))


def test_graph_scorer_rejects_target_label_leakage_in_chunk_scores(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    chunk_scores = seed_chunk_scores(tmp_path / "chunk" / "panelgraph_per_chunk_verdict_rows.jsonl", eval_rows, leak_target=True)
    active_index = seed_active_index(tmp_path / "index", eval_rows)

    with pytest.raises(PanelGraphGraphScorerError, match="PANELGRAPH_GRAPH_SCORER_TARGET_KEYS_IN_CHUNK_SCORE_ROWS"):
        run_probe(make_args(tmp_path, eval_rows, chunk_scores, active_index, run_id="run-test-leak"))
