from __future__ import annotations

import json
import signal
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_execution_trace_pytest_batch import (
    SWEBENCH_TIMEOUT_BACKSTOP_BUFFER_S,
    ExecutionTracePytestBatchError,
    effective_instance_timeout,
    load_runnable_rows,
    run_batch,
    select_rows,
    watchdog_timeout,
)


def test_watchdog_fires_strictly_after_swebench_timeout():
    # #824: when a row watchdog is set, swebench's exec timeout must finish first.
    args = Namespace(instance_timeout=1800, row_timeout=900)
    swebench_to = effective_instance_timeout(args)
    backstop = watchdog_timeout(args)
    assert swebench_to == 900
    assert backstop == 900 + SWEBENCH_TIMEOUT_BACKSTOP_BUFFER_S
    assert backstop > swebench_to  # the watchdog never races swebench's own timeout


def test_watchdog_disabled_when_no_row_timeout():
    args = Namespace(instance_timeout=1800, row_timeout=None)
    assert watchdog_timeout(args) is None
    assert effective_instance_timeout(args) == 1800


def test_watchdog_still_above_swebench_for_small_row_timeout():
    args = Namespace(instance_timeout=1800, row_timeout=5)
    assert effective_instance_timeout(args) == 5
    assert watchdog_timeout(args) == 5 + SWEBENCH_TIMEOUT_BACKSTOP_BUFFER_S


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_jsonl(path: Path, rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in rows),
        encoding="utf-8",
    )


def runnable_row(
    task_id: str,
    category: str,
    row_index: int,
    *,
    command_family: str = "pytest_direct",
    handler: str = "pytest_sys_settrace_v1",
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "task_id": task_id,
        "repo": task_id.split("__", 1)[0] + "/repo",
        "category": category,
        "row_index": row_index,
        "command_family": command_family,
        "support_status": "supported_current",
        "handler": handler,
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
        "command_family": "pytest_direct",
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
        "row_timeout": None,
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


def fake_trace_runner_asserts_started_progress(args: Namespace) -> dict[str, object]:
    progress_path = Path(args.output_root).parent / "direct_pytest_trace_batch_progress.json"
    state_progress_path = Path(args.state_root).parent / "direct_pytest_trace_batch_progress.json"
    selected_path = Path(args.output_root).parent / "selected_direct_pytest_rows.jsonl"
    state_selected_path = Path(args.state_root).parent / "selected_direct_pytest_rows.jsonl"
    fsv_progress = json.loads(progress_path.read_text(encoding="utf-8"))
    state_progress = json.loads(state_progress_path.read_text(encoding="utf-8"))

    assert fsv_progress == state_progress
    assert selected_path.read_text(encoding="utf-8") == state_selected_path.read_text(encoding="utf-8")
    assert "org__repo-1" in selected_path.read_text(encoding="utf-8")
    assert fsv_progress["phase"] == "row_started"
    assert fsv_progress["counts"] == {
        "selected_rows": 1,
        "attempted_rows": 0,
        "succeeded_rows": 0,
        "failed_rows": 0,
        "remaining_rows": 1,
    }
    assert fsv_progress["current_row_key"]["task_id"] == args.task_id
    assert fsv_progress["current_row_key"]["category"] == args.category
    return fake_trace_runner(args)


def fake_trace_runner_asserts_django(args: Namespace) -> dict[str, object]:
    assert args.command_family == "django_runtests"
    return fake_trace_runner(args)


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


def test_load_runnable_rows_accepts_django_family_when_requested(tmp_path: Path):
    rows_path = tmp_path / "rows.jsonl"
    row = runnable_row(
        "django__django-1",
        "known_good",
        0,
        command_family="django_runtests",
        handler="python_script_sys_settrace_v1",
    )
    write_jsonl(rows_path, [row])

    rows = load_runnable_rows(rows_path, command_family="django_runtests")

    assert rows[0]["command_family"] == "django_runtests"
    assert rows[0]["handler"] == "python_script_sys_settrace_v1"


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


def test_run_batch_django_mode_writes_family_specific_artifacts(tmp_path: Path):
    rows_path = tmp_path / "runnable-django.jsonl"
    rows = [
        runnable_row(
            "django__django-1",
            "known_good",
            0,
            command_family="django_runtests",
            handler="python_script_sys_settrace_v1",
        )
    ]
    write_jsonl(rows_path, rows)

    result = run_batch(
        batch_args(tmp_path, rows_path, command_family="django_runtests"),
        trace_runner=fake_trace_runner_asserts_django,
    )

    manifest_path = Path(result["manifest"])
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    state_manifest = json.loads(
        (tmp_path / "state" / "run-test" / "django-runtests-batch" / "django_runtests_trace_batch_manifest.json")
        .read_text(encoding="utf-8")
    )

    assert result["ok"] is True
    assert result["command_family"] == "django_runtests"
    assert manifest_path.name == "django_runtests_trace_batch_manifest.json"
    assert manifest == state_manifest
    assert manifest["artifact_kind"] == "execution_trace_django_runtests_batch"
    assert manifest["summary"]["django_runtests_trace_rows_generated"] == 1
    assert manifest["decision"]["django_runtests_batch_complete"] is True
    assert Path(result["progress"]).name == "django_runtests_trace_batch_progress.json"


def test_run_batch_row_timeout_bounds_smoke_instance_timeout(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])
    observed: list[int] = []

    def runner(args: Namespace) -> dict[str, object]:
        observed.append(int(args.instance_timeout))
        return fake_trace_runner(args)

    result = run_batch(
        batch_args(tmp_path, rows_path, instance_timeout=1800, row_timeout=5),
        trace_runner=runner,
    )

    assert result["ok"] is True
    assert observed == [5]


def test_run_batch_keeps_smaller_instance_timeout_when_row_timeout_is_larger(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])
    observed: list[int] = []

    def runner(args: Namespace) -> dict[str, object]:
        observed.append(int(args.instance_timeout))
        return fake_trace_runner(args)

    result = run_batch(
        batch_args(tmp_path, rows_path, instance_timeout=3, row_timeout=5),
        trace_runner=runner,
    )

    assert result["ok"] is True
    assert observed == [3]


def test_run_batch_writes_progress_heartbeat_before_and_after_rows(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])

    result = run_batch(
        batch_args(tmp_path, rows_path),
        trace_runner=fake_trace_runner_asserts_started_progress,
    )

    progress_path = Path(result["progress"])
    state_progress_path = tmp_path / "state" / "run-test" / "pytest-batch" / progress_path.name
    progress_history_path = Path(result["progress_history"])
    state_history_path = tmp_path / "state" / "run-test" / "pytest-batch" / progress_history_path.name
    progress = json.loads(progress_path.read_text(encoding="utf-8"))
    state_progress = json.loads(state_progress_path.read_text(encoding="utf-8"))
    history = [json.loads(line) for line in progress_history_path.read_text(encoding="utf-8").splitlines()]
    state_history = [json.loads(line) for line in state_history_path.read_text(encoding="utf-8").splitlines()]
    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))

    assert progress == state_progress
    assert history == state_history
    assert [row["phase"] for row in history] == ["selected", "row_started", "row_completed", "batch_finalizing"]
    assert progress["phase"] == "batch_finalizing"
    assert progress["counts"] == {
        "selected_rows": 1,
        "attempted_rows": 1,
        "succeeded_rows": 1,
        "failed_rows": 0,
        "remaining_rows": 0,
    }
    assert progress["last_completed_row_key"]["task_id"] == "org__repo-1"
    assert progress["last_completed_artifacts"]["state_trace_sha256"] == progress["last_completed_artifacts"]["fsv_trace_sha256"]
    assert manifest["artifacts"]["progress"] == str(progress_path)
    assert manifest["artifacts"]["progress_history"] == str(progress_history_path)


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
    progress = json.loads(Path(result["progress"]).read_text(encoding="utf-8"))
    history = [json.loads(line) for line in Path(result["progress_history"]).read_text(encoding="utf-8").splitlines()]

    assert result["ok"] is False
    assert len(failed) == 1
    assert failed[0]["task_id"] == "org__repo-2"
    assert "one_or_more_direct_pytest_trace_rows_failed" in manifest["decision"]["decision_reasons"]
    assert progress["counts"]["attempted_rows"] == 2
    assert progress["counts"]["failed_rows"] == 1
    assert progress["last_completed_artifacts"]["error"] == "RuntimeError: synthetic trace failure"
    assert [row["phase"] for row in history].count("row_completed") == 2


def test_run_batch_sigterm_writes_failed_ledgers_manifest_and_cleanup(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])
    cleanup_calls: list[tuple[str, str]] = []

    def runner(_args: Namespace) -> dict[str, object]:
        signal.raise_signal(signal.SIGTERM)
        raise AssertionError("SIGTERM handler should interrupt the row")

    def cleanup(row: dict[str, object], _args: Namespace, reason: str) -> dict[str, object]:
        cleanup_calls.append((str(row["task_id"]), reason))
        return {
            "attempted": True,
            "removed": True,
            "container_name": "sweb.eval.org__repo-1.row-0000-org__repo-1-known_good",
            "reason": reason,
        }

    with pytest.raises(ExecutionTracePytestBatchError, match="STOPPED_AFTER_ROW_FAILURE"):
        run_batch(
            batch_args(tmp_path, rows_path),
            trace_runner=runner,
            container_cleanup=cleanup,
        )

    fsv_root = tmp_path / "fsv" / "run-test" / "pytest-batch"
    state_root = tmp_path / "state" / "run-test" / "pytest-batch"
    manifest_path = fsv_root / "direct_pytest_trace_batch_manifest.json"
    failed_rows_path = fsv_root / "failed_direct_pytest_rows.jsonl"
    selected_rows_path = fsv_root / "selected_direct_pytest_rows.jsonl"
    batch_rows_path = fsv_root / "direct_pytest_trace_batch_rows.jsonl"
    trace_index_path = fsv_root / "generated_trace_index.jsonl"
    artifact_hashes_path = fsv_root / "artifact_hashes.json"

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    state_manifest = json.loads((state_root / manifest_path.name).read_text(encoding="utf-8"))
    failed = [json.loads(line) for line in failed_rows_path.read_text(encoding="utf-8").splitlines()]
    batch_rows = [json.loads(line) for line in batch_rows_path.read_text(encoding="utf-8").splitlines()]
    history = [
        json.loads(line)
        for line in (fsv_root / "direct_pytest_trace_batch_progress.jsonl").read_text(encoding="utf-8").splitlines()
    ]

    assert cleanup_calls == [("org__repo-1", "EXECUTION_TRACE_PYTEST_BATCH_INTERRUPTED")]
    assert manifest == state_manifest
    assert selected_rows_path.read_text(encoding="utf-8") == (state_root / selected_rows_path.name).read_text(encoding="utf-8")
    assert failed_rows_path.read_text(encoding="utf-8") == (state_root / failed_rows_path.name).read_text(encoding="utf-8")
    assert batch_rows_path.read_text(encoding="utf-8") == (state_root / batch_rows_path.name).read_text(encoding="utf-8")
    assert trace_index_path.read_text(encoding="utf-8") == ""
    assert artifact_hashes_path.is_file()
    assert (state_root / "artifact_hashes.json").is_file()
    assert len(failed) == 1
    assert failed == batch_rows
    assert failed[0]["error_code"] == "EXECUTION_TRACE_PYTEST_BATCH_INTERRUPTED"
    assert failed[0]["error_type"] == "ExecutionTracePytestBatchInterrupted"
    assert failed[0]["container_cleanup"]["removed"] is True
    assert manifest["summary"]["attempted_rows"] == 1
    assert manifest["summary"]["failed_rows"] == 1
    assert manifest["summary"]["succeeded_rows"] == 0
    assert manifest["decision"]["promotion_allowed"] is False
    assert "one_or_more_direct_pytest_trace_rows_failed" in manifest["decision"]["decision_reasons"]
    assert [row["phase"] for row in history] == ["selected", "row_started", "row_completed", "batch_finalizing"]


def test_non_aiwonder_roots_fail_closed_without_local_override(tmp_path: Path):
    rows_path = tmp_path / "runnable.jsonl"
    write_jsonl(rows_path, [runnable_row("org__repo-1", "known_good", 0)])

    with pytest.raises(ExecutionTracePytestBatchError, match="NON_AIWONDER_DURABLE_PATH"):
        run_batch(
            batch_args(tmp_path, rows_path, allow_non_aiwonder_roots=False),
            trace_runner=fake_trace_runner,
        )
