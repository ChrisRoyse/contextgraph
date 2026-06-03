from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import numpy as np
import pytest

from ..python_panelgraph_chunk_verdict_probe import PanelGraphChunkVerdictError, run_probe


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def sha256_bytes(payload: bytes) -> str:
    import hashlib

    return "sha256:" + hashlib.sha256(payload).hexdigest()


def vector_sha(values: list[float]) -> str:
    return sha256_bytes(np.asarray(values, dtype="<f4").tobytes())


def seed_eval_rows(root: Path) -> Path:
    rows = [
        eval_row("calibration", "task-pass", "known_good", True, 0.31, ["chunk-pass-a", "chunk-pass-b"]),
        eval_row("calibration", "task-fail", "off_by_one", False, 0.82, ["chunk-fail-a", "chunk-fail-b"]),
        eval_row("holdout", "task-holdout", "swap_variable", True, 0.44, ["chunk-holdout-a", "chunk-holdout-b"]),
    ]
    path = root / "panelgraph_phase1_offline_eval_rows.jsonl"
    write_jsonl(path, rows)
    return path


def eval_row(split: str, task_id: str, category: str, actual: bool, score: float, chunks: list[str]) -> dict:
    return {
        "schema_version": 2,
        "artifact_kind": "panelgraph_phase1_offline_eval_row",
        "split": split,
        "task_id": task_id,
        "repo": "org/repo",
        "category": category,
        "shape_class": "multi_file_multi_chunk",
        "patch_touched_file_count": 2,
        "patch_touched_chunk_count": len(chunks),
        "candidate_state_chunk_count": 8,
        "primary_chunk_ids": ["sha256:" + chunk for chunk in chunks],
        "predicted_oracle_pass": score,
        "actual_oracle_pass": actual,
        "absolute_error": abs(score - (1.0 if actual else 0.0)),
        "offline_eval_labels_not_live_inputs": True,
    }


def seed_active_index(root: Path, chunk_ids: list[str], *, omit: tuple[str, str] | None = None) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    active_index = root / "active_index.jsonl"
    rows = []
    for embedder_id in ("e1", "e6"):
        record_path = root / f"{embedder_id}.jsonl"
        offset = 0
        with record_path.open("wb") as handle:
            for chunk_id in chunk_ids:
                if omit == (chunk_id, embedder_id):
                    continue
                signed = 1.0 if "pass" in chunk_id or "holdout" in chunk_id else -1.0
                if embedder_id == "e6":
                    signed *= 0.5
                values = [signed, signed + 0.25]
                payload = {
                    "cache_key_sha256": sha256_bytes(f"{chunk_id}:{embedder_id}".encode()),
                    "chunk": {"task_instance_id": "task", "mutation_category": "known_good", "workspace_state": "test_patch"},
                    "key": {"chunk_id": chunk_id, "embedder_id": embedder_id},
                    "output": {"vector": values},
                }
                raw = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
                handle.write(raw + b"\n")
                rows.append(
                    {
                        "schema_version": 1,
                        "artifact_kind": "python_ast_chunk_forward_v3_index",
                        "chunk_id": chunk_id,
                        "embedder_id": embedder_id,
                        "record_path": str(record_path),
                        "byte_offset": offset,
                        "byte_len": len(raw),
                        "record_sha256": sha256_bytes(raw),
                        "vector_sha256": vector_sha(values),
                        "vector_len": len(values),
                    }
                )
                offset += len(raw) + 1
    write_jsonl(active_index, rows)
    return active_index


def make_args(tmp_path: Path, eval_rows: Path, active_index: Path, **overrides: object) -> Namespace:
    values = {
        "phase1_root": eval_rows.parent,
        "phase1_eval_rows": eval_rows,
        "active_index": active_index,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-chunk-verdict",
        "active_embedder_ids": "e1,e6",
        "ridge_alpha": 1.0,
        "allow_non_aiwonder_roots": True,
        "clean": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_chunk_verdict_probe_writes_contract_rows_and_state_mirror(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    chunk_ids = [
        "sha256:chunk-pass-a",
        "sha256:chunk-pass-b",
        "sha256:chunk-fail-a",
        "sha256:chunk-fail-b",
        "sha256:chunk-holdout-a",
        "sha256:chunk-holdout-b",
    ]
    active_index = seed_active_index(tmp_path / "index", chunk_ids)
    result = run_probe(make_args(tmp_path, eval_rows, active_index))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    verdict_rows = [
        json.loads(line)
        for line in Path(result["per_chunk_verdict_rows"]).read_text(encoding="utf-8").splitlines()
    ]

    assert result["passes"] is True
    assert result["verdict_row_count"] == 6
    assert manifest["policy"]["per_slot_heads"] is True
    assert manifest["policy"]["slot_identity_preserved"] is True
    assert manifest["policy"]["flat_vector_concat_used"] is False
    assert manifest["policy"]["target_labels_used_as_live_inputs"] is False
    assert manifest["summary"]["clone_audit"]["row_level_scores_used_as_features"] is False
    assert Path(result["state_manifest"]).read_text(encoding="utf-8") == Path(result["manifest"]).read_text(encoding="utf-8")
    assert len(verdict_rows) == 6
    assert all(set(row).isdisjoint({"actual_oracle_pass", "oracle_all_passed"}) for row in verdict_rows)
    assert all(0.0 <= row["predicted_oracle_pass"] <= 1.0 for row in verdict_rows)
    assert {row["score_source"] for row in verdict_rows} == {"calibration_only_per_slot_ridge_chunk_head"}


def test_missing_active_index_slot_fails_closed(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    chunk_ids = [
        "sha256:chunk-pass-a",
        "sha256:chunk-pass-b",
        "sha256:chunk-fail-a",
        "sha256:chunk-fail-b",
        "sha256:chunk-holdout-a",
        "sha256:chunk-holdout-b",
    ]
    active_index = seed_active_index(tmp_path / "index", chunk_ids, omit=("sha256:chunk-holdout-b", "e6"))

    with pytest.raises(PanelGraphChunkVerdictError, match="PANELGRAPH_CHUNK_VERDICT_ACTIVE_INDEX_COVERAGE_INCOMPLETE"):
        run_probe(make_args(tmp_path, eval_rows, active_index, run_id="run-test-missing-slot"))


def test_target_keys_in_eval_output_contract_are_rejected_by_shape(tmp_path: Path):
    eval_rows = seed_eval_rows(tmp_path / "phase1")
    payload = [
        json.loads(line)
        for line in eval_rows.read_text(encoding="utf-8").splitlines()
    ]
    payload[0]["primary_chunk_ids"] = []
    write_jsonl(eval_rows, payload)
    active_index = seed_active_index(tmp_path / "index", ["sha256:chunk-fail-a", "sha256:chunk-fail-b"])

    with pytest.raises(PanelGraphChunkVerdictError, match="PANELGRAPH_CHUNK_VERDICT_PRIMARY_CHUNKS_MISSING"):
        run_probe(make_args(tmp_path, eval_rows, active_index, run_id="run-test-empty-primary"))
