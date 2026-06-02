from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_panelgraph_phase1_audit import PanelGraphPhase1Error, run_audit


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def patch_text(files: list[str]) -> str:
    parts = []
    for idx, name in enumerate(files, start=1):
        parts.append(
            "\n".join(
                [
                    f"diff --git a/{name} b/{name}",
                    f"--- a/{name}",
                    f"+++ b/{name}",
                    "@@ -1,2 +1,2 @@",
                    " def f():",
                    f"-    return {idx}",
                    f"+    return {idx + 10}",
                    "",
                ]
            )
        )
    return "\n".join(parts)


def seed_fixture(root: Path, *, include_multi_file: bool = True) -> tuple[Path, Path, Path]:
    corpus = root / "corpus"
    mutations = corpus / "mutations"
    task_rows = [
        ("task-single", "off_by_one", "single.py", ["pkg/single.py"], False, 0.20, "holdout"),
        ("task-multi", "known_good", "known_good.py", ["pkg/a.py", "pkg/b.py"], True, 0.75, "holdout"),
        ("task-cal", "swap_variable", "cal.py", ["pkg/c.py", "pkg/d.py"], False, 0.60, "calibration"),
    ]
    entries = []
    ast_rows = []
    for task_id, category, patch_name, files, actual, _score, bucket in task_rows:
        if not include_multi_file and task_id == "task-multi":
            files = ["pkg/a.py"]
        patch_path = mutations / task_id / f"{category}.patch"
        patch_path.parent.mkdir(parents=True, exist_ok=True)
        patch_path.write_text(patch_text(files), encoding="utf-8")
        entries.append(
            {
                "task_id": task_id,
                "repo": "org/repo",
                "category": category,
                "bucket": bucket,
                "patch_path": str(patch_path.relative_to(corpus)),
                "oracle_all_passed": actual,
            }
        )
        state = "test_patch" if category == "known_good" else f"mutation_{category}"
        for file_index, rel in enumerate(files):
            ast_rows.append(chunk(task_id, state, category, rel, f"candidate-{task_id}-{file_index}"))
            ast_rows.append(chunk(task_id, "base", "known_good", rel, f"base-{task_id}-{file_index}"))
    write_json(corpus / "index.json", {"entries": entries})
    ast = root / "python_ast_chunks.jsonl"
    write_jsonl(ast, ast_rows)
    write_json(
        root / "ship-gate-fsv.json",
        {
            "execute_and_inspect": {
                "holdout_predictions": [
                    pred("task-single", "off_by_one", False, 0.20),
                    pred("task-multi", "known_good", True, 0.75),
                ],
                "calibration_predictions": [
                    pred("task-cal", "swap_variable", False, 0.60),
                ],
            }
        },
    )
    return corpus, ast, root / "ship-gate-fsv.json"


def chunk(task_id: str, state: str, category: str, relative_path: str, suffix: str) -> dict:
    return {
        "task_instance_id": task_id,
        "workspace_state": state,
        "mutation_category": category,
        "relative_path": relative_path,
        "chunk_id": "sha256:" + hashlib64(suffix),
        "chunk_kind": "function",
        "name": "f",
        "qualname": "f",
        "line_start": 1,
        "line_end": 2,
        "depth": 0,
        "legacy_provenance_path": "/mnt/d/legacy/path.py",
    }


def hashlib64(seed: str) -> str:
    import hashlib

    return hashlib.sha256(seed.encode("utf-8")).hexdigest()


def pred(task_id: str, category: str, actual: bool, score: float) -> dict:
    return {
        "task_id": task_id,
        "repo": "org/repo",
        "category": category,
        "actual_oracle_pass": actual,
        "predicted_oracle_pass": score,
        "predicted_pass": score >= 0.5,
    }


def args(tmp_path: Path, corpus: Path, ast: Path, ship: Path, **overrides: object) -> Namespace:
    values = {
        "corpus_root": corpus,
        "ast_chunks": ast,
        "ship_gate_report": ship,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "run_id": "run-test-panelgraph",
        "min_multi_file_split_eval_rows": 1,
        "min_mapping_coverage": 0.95,
        "allow_non_aiwonder_roots": True,
    }
    values.update(overrides)
    return Namespace(**values)


def test_panelgraph_phase1_writes_live_and_eval_artifacts_without_mounted_paths(tmp_path: Path):
    corpus, ast, ship = seed_fixture(tmp_path)
    result = run_audit(args(tmp_path, corpus, ast, ship))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    live_rows = [json.loads(line) for line in Path(manifest["artifacts"]["live_shape_rows"]).read_text(encoding="utf-8").splitlines()]
    eval_rows = [json.loads(line) for line in Path(manifest["artifacts"]["offline_eval_rows"]).read_text(encoding="utf-8").splitlines()]

    assert manifest["passes"] is True
    assert manifest["summary"]["support_summary"]["multi_file_split_eval_rows"] == 2
    assert manifest["summary"]["support_summary"]["input_legacy_mounted_drive_field_hits"] > 0
    assert manifest["decision"]["panelgraph_support_present"] is True
    assert manifest["decision"]["full_panelgraph_schema_change_allowed"] is False
    assert "per_chunk_verdict_rows_unavailable" in manifest["decision"]["phase2_blockers"]
    assert Path(result["state_manifest"]).read_text(encoding="utf-8") == Path(result["manifest"]).read_text(encoding="utf-8")
    assert all("actual_oracle_pass" not in row["live_features"] for row in live_rows)
    assert any(row["shape_class"] == "multi_file_multi_chunk" for row in live_rows)
    assert len(eval_rows) == 3
    for path in [Path(result["manifest"]), Path(manifest["artifacts"]["live_shape_rows"]), Path(manifest["artifacts"]["offline_eval_rows"])]:
        text = path.read_text(encoding="utf-8")
        assert "/mnt/d/" not in text
        assert "/mnt/c/" not in text


def test_insufficient_multifile_support_blocks_phase2(tmp_path: Path):
    corpus, ast, ship = seed_fixture(tmp_path, include_multi_file=False)
    result = run_audit(args(tmp_path, corpus, ast, ship, min_multi_file_split_eval_rows=2))
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["decision"]["panelgraph_support_present"] is True
    assert "insufficient_multi_file_split_eval_rows" in manifest["decision"]["phase2_blockers"]
    assert manifest["decision"]["full_panelgraph_schema_change_recommended"] is False


def test_empty_index_fails_closed(tmp_path: Path):
    corpus = tmp_path / "corpus"
    write_json(corpus / "index.json", {"entries": []})
    ast = tmp_path / "chunks.jsonl"
    ast.write_text("", encoding="utf-8")
    ship = tmp_path / "ship.json"
    write_json(ship, {"execute_and_inspect": {"holdout_predictions": [], "calibration_predictions": []}})

    with pytest.raises(PanelGraphPhase1Error, match="PANELGRAPH_PHASE1_INDEX_ENTRIES_MISSING"):
        run_audit(args(tmp_path, corpus, ast, ship))


def test_mounted_absolute_patch_path_rejected(tmp_path: Path):
    corpus, ast, ship = seed_fixture(tmp_path)
    payload = json.loads((corpus / "index.json").read_text(encoding="utf-8"))
    payload["entries"][0]["patch_path"] = "/mnt/d/contextgraph/bad.patch"
    write_json(corpus / "index.json", payload)

    with pytest.raises(PanelGraphPhase1Error, match="PANELGRAPH_PHASE1_MOUNTED_DRIVE_FORBIDDEN"):
        run_audit(args(tmp_path, corpus, ast, ship))
