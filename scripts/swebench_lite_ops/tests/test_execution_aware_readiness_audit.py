from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_execution_aware_readiness_audit import (
    ExecutionAwareReadinessAuditError,
    run_audit,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows), encoding="utf-8")


def row(task_id: str, repo: str, category: str, actual: bool = True) -> dict:
    return {
        "task_id": task_id,
        "repo": repo,
        "category": category,
        "predicted_oracle_pass": 0.75 if actual else 0.25,
        "actual_oracle_pass": actual,
    }


def report(rows: list[dict]) -> dict:
    return {
        "execute_and_inspect": {
            "calibration_predictions": rows,
            "holdout_raw_predictions": rows,
        }
    }


def write_corpus(root: Path, rows: list[dict]) -> None:
    entries = []
    for item in rows:
        entries.append(
            {
                "task_id": item["task_id"],
                "repo": item["repo"],
                "category": item["category"],
                "patch_path": f"mutations/{item['task_id']}/{item['category']}.patch",
                "oracle_all_passed": item["actual_oracle_pass"],
                "oracle_verdict_sha256": "sha256:" + ("a" * 64),
                "oracle_per_test_count": 3,
            }
        )
    write_json(root / "index.json", {"schema_version": 1, "entries": entries})


def run_fixture(tmp_path: Path, *, corpus_rows: list[dict]) -> dict:
    corpus_root = tmp_path / "corpus"
    write_corpus(corpus_root, corpus_rows)
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(fsv, report(corpus_rows))
    return run_audit(
        Namespace(
            ship_gate_report=fsv,
            corpus_root=corpus_root,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_corpus_rows=len(corpus_rows),
            max_inventory_rows=500,
            allow_non_aiwonder_roots=True,
        )
    )


def test_no_execution_trace_artifacts_requires_trace_generation(tmp_path: Path):
    rows = [row("org__repo-1", "org/repo", "off_by_one", True)]
    result = run_fixture(tmp_path, corpus_rows=rows)
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["decision"]["execution_aware_pretraining_ready"] is False
    assert manifest["decision"]["trace_generation_required"] is True
    assert "no_eligible_live_execution_trace_artifacts_found" in manifest["decision"]["decision_reasons"]
    assert manifest["trace_coverage"]["eligible_execution_trace_rows"] == 0
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False


def test_oracle_summary_is_classified_target_side_only(tmp_path: Path):
    rows = [row("org__repo-1", "org/repo", "subtle_flip", False)]
    corpus_root = tmp_path / "corpus"
    write_corpus(corpus_root, rows)
    write_json(
        corpus_root / "swebench-runs" / "mejepa-subtle_flip.run.json",
        {"resolved_ids": [], "unresolved_ids": ["org__repo-1"], "completed_ids": ["org__repo-1"]},
    )
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(fsv, report(rows))
    result = run_audit(
        Namespace(
            ship_gate_report=fsv,
            corpus_root=corpus_root,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_corpus_rows=1,
            max_inventory_rows=500,
            allow_non_aiwonder_roots=True,
        )
    )
    inventory = [
        json.loads(line)
        for line in Path(result["trace_inventory"]).read_text(encoding="utf-8").splitlines()
    ]
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert inventory[0]["kind"] == "oracle_summary_json"
    assert inventory[0]["target_side_only"] is True
    assert inventory[0]["eligible_live_execution_feature"] is False
    assert manifest["decision"]["execution_aware_pretraining_ready"] is False
    assert "only_target_side_oracle_artifacts_available_for_runtime_outcomes" in manifest["decision"]["decision_reasons"]


def test_valid_trace_files_make_rows_ready(tmp_path: Path):
    rows = [
        row("org__repo-1", "org/repo", "off_by_one", True),
        row("org__repo-2", "org/repo", "swap_variable", False),
    ]
    corpus_root = tmp_path / "corpus"
    write_corpus(corpus_root, rows)
    for item in rows:
        write_json(
            corpus_root / "execution-traces" / item["task_id"] / f"{item['category']}-trace.json",
            {
                "task_id": item["task_id"],
                "category": item["category"],
                "trace_events": [{"line": 1, "event": "call"}],
                "coverage": {"lines_executed": 1},
            },
        )
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(fsv, report(rows))
    result = run_audit(
        Namespace(
            ship_gate_report=fsv,
            corpus_root=corpus_root,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_corpus_rows=2,
            max_inventory_rows=500,
            allow_non_aiwonder_roots=True,
        )
    )
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert manifest["trace_coverage"]["eligible_execution_trace_rows"] == 2
    assert manifest["trace_coverage"]["holdout_rows_with_trace"] == 2
    assert manifest["decision"]["execution_aware_pretraining_ready"] is True
    assert manifest["decision"]["trace_generation_required"] is False


def test_missing_corpus_index_fails_closed(tmp_path: Path):
    fsv = tmp_path / "ship-gate-fsv.json"
    rows = [row("org__repo-1", "org/repo", "off_by_one", True)]
    write_json(fsv, report(rows))
    with pytest.raises(ExecutionAwareReadinessAuditError, match="EXECUTION_AWARE_READINESS_CORPUS_INDEX_MISSING"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                corpus_root=tmp_path / "missing-corpus",
                output_root=tmp_path / "fsv",
                state_root=tmp_path / "state",
                run_id="run-test",
                expected_corpus_rows=1,
                max_inventory_rows=500,
                allow_non_aiwonder_roots=True,
            )
        )


def test_durable_roots_require_aiwonder_without_test_escape(tmp_path: Path):
    rows = [row("org__repo-1", "org/repo", "off_by_one", True)]
    corpus_root = tmp_path / "corpus"
    write_corpus(corpus_root, rows)
    fsv = tmp_path / "ship-gate-fsv.json"
    write_json(fsv, report(rows))
    with pytest.raises(ExecutionAwareReadinessAuditError, match="EXECUTION_AWARE_READINESS_NON_AIWONDER_DURABLE_PATH"):
        run_audit(
            Namespace(
                ship_gate_report=fsv,
                corpus_root=corpus_root,
                output_root=tmp_path / "fsv",
                state_root=Path("/zfs/archive/contextgraph/state/execution-aware-audit/inv-010-v1"),
                run_id="run-test",
                expected_corpus_rows=1,
                max_inventory_rows=500,
                allow_non_aiwonder_roots=False,
            )
        )
