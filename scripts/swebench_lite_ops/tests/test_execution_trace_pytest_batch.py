from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_execution_trace_pytest_batch import (
    ExecutionTracePytestBatchError,
    load_runnable_rows,
    run_batch,
    select_rows,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def runnable_row(task_id: str, category: str, row_index: int) -> dict[str, object]:
    return {
        "schema_version": 1,
        "task_id": task_id,
        "repo": task_id.split("__", 1)[0] + "/repo",
        "category": category,
        "row_index": row_index,
        "command_family": "pytest_direct",
        "support_status": "supported_current",
        "handler": "pytest_sys_settrace_v1",
    }


def batch_args(tmp_path: Path, rows_path: Path, **overrides: object) -> Namespace:
    values = {
        "dataset_name": "fixture",
        "split": "test",
        "corpus_root": tmp_path / "corpus",
        "runnable_rows": rows_path,
        "output_root": tmp_path / "fsv",
        "state_root": tmp_path / "state",
        "venv_python": tmp_path / "venv" / "bin" / "python3",
        "run_id": "run-test",
        "expected_corpus_rows": 2400,
        "expected_runnable_rows": None,
        "offset": 0,
        "limit": None,
        "categories": None,
        "task_ids": None,
        "namespace": "swebench",
        "arch": "x86_64",
        "tag": "latest",
        "instance_timeout": 1800,
        "max_events": 20,
        "continue_on_error": False,
        "allow_non_aiwonder_roots": True,
    }
    values.update(overrides)
    return Namespace(**values)


def fake_trace_runner(args: Namespace) -> dict[str, object]:
    rel = Path(args.task_id) / f"{args.category}.execution-trace.json"
    state_trace = Path(args.state_root) / args.run_id / "trace-smoke" / rel
    fsv_trace = Path(args.output_root) / args.run_id / "trace-smoke" / "extracted-traces" / rel
    payload = {
        "schema_version": 1,
        "task_id": args.task_id,
        "category": args.category,
        "candidate_workspace_state": args.category,
        "coverage": {"lines_executed": 3, "files_touched": 1, "events_seen": 4},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 5, "pytest_exit_code": 1},
        "steps": {"event_counts": {"line": 3}},
        "trace_events": [{"event": "line", "file": "pkg/mod.py", "line": 1, "function": "f"}],
        "variable_states": [{"event_index": 0, "local_names": ["x"]}],
    }
    write_json(state_trace, payload)
    write_json(fsv_trace, payload)
    return {
        "ok": True,
        "state_trace": str(state_trace),
        "fsv_trace": str(fsv_trace),
        "trace_summary": {"trace_events_recorded": 1, "lines_executed": 3},
    }


def test_load_runnable_rows_rejects_target_side_fields(tmp_path: Path):
    rows_path = tmp_path / "rows.jsonl"
    row = runnable_row("org__repo-1", "known_good", 0)
    row["oracle_all_passed"] = True
    write_jsonl(rows_path, [row])

    with pytest.raises(ExecutionTracePytestBatchError, match="TARGET_SIDE_FIELD"):
        load_runnable_rows(rows_path)


def test_load_runnable_rows_rejects_non_pytest_family(tmp_path: Path):
    rows_path = tmp_path / "rows.jsonl"
    row = runnable_row("org__repo-1", "known_good", 0)
    row["command_family"] = "django_runtests"
    write_jsonl(rows_path, [row])

    with pytest.raises(ExecutionTracePytestBatchError, match="UNSUPPORTED_COMMAND_FAMILY"):
        load_runnable_rows(rows_path)


def test_select_rows_applies_category_offset_and_limit():
    rows = [
        runnable_row("org__repo-1", "known_good", 0),
        runnable_row("org__repo-2", "known_good", 1),
        runnable_row("org__repo-3", "off_by_one", 2),
    ]

    selected = select_rows(rows, offset=1, limit=1, categories=["known_good"], task_ids=None)

    assert [row["task_id"] for row in selected] == ["org__repo-2"]


def test_run_batch_with_fake_runner_writes_generated_traces_and_state_mirror(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    rows = [
        runnable_row("org__repo-1", "known_good", 0),
        runnable_row("org__repo-1", "off_by_one", 1),
    ]
    write_jsonl(rows_path, rows)

    result = run_batch(batch_args(tmp_path, rows_path), trace_runner=fake_trace_runner)

    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    state_manifest = json.loads(
        (tmp_path / "state" / "run-test" / "pytest-batch" / "direct_pytest_trace_batch_manifest.json")
        .read_text(encoding="utf-8")
    )
    trace_index = [
        json.loads(line)
        for line in Path(result["generated_trace_index"]).read_text(encoding="utf-8").splitlines()
    ]

    assert result["ok"] is True
    assert manifest == state_manifest
    assert manifest["summary"]["succeeded_rows"] == 2
    assert manifest["summary"]["direct_pytest_trace_fraction_of_corpus"] == 0.0008333333
    assert len(trace_index) == 2
    for row in trace_index:
        assert Path(row["state_trace"]).is_file()
        assert Path(row["fsv_trace"]).is_file()
        assert row["state_trace_sha256"] == row["fsv_trace_sha256"]
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False


def test_run_batch_continue_on_error_records_failed_rows(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    rows = [
        runnable_row("org__repo-1", "known_good", 0),
        runnable_row("org__repo-2", "off_by_one", 1),
    ]
    write_jsonl(rows_path, rows)

    def runner(args: Namespace) -> dict[str, object]:
        if args.task_id == "org__repo-2":
            raise RuntimeError("synthetic trace failure")
        return fake_trace_runner(args)

    result = run_batch(
        batch_args(tmp_path, rows_path, continue_on_error=True),
        trace_runner=runner,
    )
    failed = [
        json.loads(line)
        for line in Path(result["failed_rows"]).read_text(encoding="utf-8").splitlines()
    ]
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert result["ok"] is False
    assert len(failed) == 1
    assert failed[0]["task_id"] == "org__repo-2"
    assert "one_or_more_direct_pytest_trace_rows_failed" in manifest["decision"]["decision_reasons"]


def test_non_aiwonder_roots_fail_closed_without_local_override(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])

    with pytest.raises(ExecutionTracePytestBatchError, match="NON_AIWONDER_DURABLE_PATH"):
        run_batch(
            batch_args(tmp_path, rows_path, allow_non_aiwonder_roots=False),
            trace_runner=fake_trace_runner,
        )
