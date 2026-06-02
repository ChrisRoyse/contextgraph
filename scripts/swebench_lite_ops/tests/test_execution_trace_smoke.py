from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

from ..python_execution_trace_smoke import (
    ExecutionTraceSmokeError,
    TRACE_BEGIN,
    TRACE_END,
    build_python_script_trace_runner_source,
    build_trace_runner_source,
    classify_missing_test_output,
    detect_swebench_timeout,
    extract_trace_payload,
    load_corpus_entry,
    payload_execution_outcome,
    transform_eval_script_list,
    validate_trace_payload,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _run_pytest_trace_runner(
    tmp_path: Path,
    *,
    fake_pytest_pkg: str,
    repo_root: Path,
    argv: list[str] | None = None,
    max_events: int = 5000,
) -> dict:
    """Execute the REAL generated runner in a subprocess against a synthetic repo.

    `fake_pytest_pkg` is written to a directory placed first on PYTHONPATH so it
    shadows any installed pytest. This exercises the actual runner code path
    (import + settrace + payload write), not a mock.
    """
    runner_path = tmp_path / "contextgraph_trace_pytest.py"
    runner_path.write_text(build_trace_runner_source(), encoding="utf-8")
    fake_lib = tmp_path / "fakelib"
    pkg_dir = fake_lib / "pytest"
    pkg_dir.mkdir(parents=True, exist_ok=True)
    (pkg_dir / "__init__.py").write_text(fake_pytest_pkg, encoding="utf-8")
    output_path = tmp_path / "trace_out.json"
    env = dict(os.environ)
    env["PYTHONPATH"] = str(fake_lib) + os.pathsep + env.get("PYTHONPATH", "")
    env["CONTEXTGRAPH_TRACE_OUTPUT"] = str(output_path)
    env["CONTEXTGRAPH_TRACE_REPO_ROOT"] = str(repo_root)
    env["CONTEXTGRAPH_TRACE_MAX_EVENTS"] = str(max_events)
    completed = subprocess.run(
        [sys.executable, "-S", str(runner_path), *(argv or [])],
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert output_path.is_file(), (
        f"runner did not write payload; rc={completed.returncode} "
        f"stdout={completed.stdout[-2000:]} stderr={completed.stderr[-2000:]}"
    )
    return json.loads(output_path.read_text(encoding="utf-8"))


def test_runner_captures_import_syntax_error_as_compile_outcome(tmp_path: Path):
    # #823: a compile_error mutation breaks a module on the `import pytest` path.
    # The runner must capture the SyntaxError as a truthful outcome and STILL
    # write a payload (it used to crash before settrace/finally).
    repo_root = tmp_path / "testbed"
    repo_root.mkdir()
    fake_pytest = (
        "from . import broken  # noqa: F401\n"
        "def main(argv):\n"
        "    return 0\n"
    )
    # broken submodule has a real syntax error -> SyntaxError at `import pytest`.
    runner_path = tmp_path / "contextgraph_trace_pytest.py"
    runner_path.write_text(build_trace_runner_source(), encoding="utf-8")
    fake_lib = tmp_path / "fakelib"
    pkg_dir = fake_lib / "pytest"
    pkg_dir.mkdir(parents=True, exist_ok=True)
    (pkg_dir / "__init__.py").write_text(fake_pytest, encoding="utf-8")
    (pkg_dir / "broken.py").write_text("def _bad_(:\n    pass\n", encoding="utf-8")
    output_path = tmp_path / "trace_out.json"
    env = dict(os.environ)
    env["PYTHONPATH"] = str(fake_lib) + os.pathsep + env.get("PYTHONPATH", "")
    env["CONTEXTGRAPH_TRACE_OUTPUT"] = str(output_path)
    env["CONTEXTGRAPH_TRACE_REPO_ROOT"] = str(repo_root)
    env["CONTEXTGRAPH_TRACE_MAX_EVENTS"] = "100"
    completed = subprocess.run(
        [sys.executable, "-S", str(runner_path), "-rA", "test_x.py"],
        env=env,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert output_path.is_file(), f"payload not written; stderr={completed.stderr[-2000:]}"
    payload = json.loads(output_path.read_text(encoding="utf-8"))
    assert payload["execution_outcome"] == "import_error"
    captured = payload["captured_exception"]
    assert captured["type"] == "SyntaxError"
    assert captured["filename"].endswith("broken.py")
    assert captured["lineno"] == 1
    # The validator must accept this truthful no-execution payload.
    validate_trace_payload(payload)
    assert payload_execution_outcome(payload) == "import_error"


def test_runner_executes_and_scopes_trace_to_repo_root(tmp_path: Path):
    # Happy path: fake pytest.main executes a repo module; only repo lines are
    # traced (scoping). The fake pytest's own lines (outside repo_root) must NOT
    # appear in line_coverage.
    repo_root = tmp_path / "testbed"
    repo_root.mkdir()
    (repo_root / "repomod.py").write_text(
        "def run():\n"
        "    total = 0\n"
        "    for i in range(3):\n"
        "        total += i\n"
        "    return total\n",
        encoding="utf-8",
    )
    fake_pytest = (
        "import sys\n"
        "def main(argv):\n"
        "    sys.path.insert(0, " + repr(str(repo_root)) + ")\n"
        "    import repomod\n"
        "    assert repomod.run() == 3\n"
        "    return 0\n"
    )
    payload = _run_pytest_trace_runner(
        tmp_path, fake_pytest_pkg=fake_pytest, repo_root=repo_root, argv=["-rA"], max_events=500
    )
    assert payload["execution_outcome"] == "executed"
    assert payload["coverage"]["lines_executed"] > 0
    locations = [row["location"] for row in payload["line_coverage"]]
    assert all(loc.startswith("repomod.py:") for loc in locations), locations
    files = {ev["file"] for ev in payload["trace_events"]}
    assert files == {"repomod.py"}, files
    validate_trace_payload(payload)


def test_pytest_runner_stops_tracing_at_max_events(tmp_path: Path):
    repo_root = tmp_path / "testbed"
    repo_root.mkdir()
    (repo_root / "busy.py").write_text(
        "def run():\n"
        "    total = 0\n"
        "    for i in range(1000):\n"
        "        total += i\n"
        "    return total\n",
        encoding="utf-8",
    )
    fake_pytest = (
        "import sys\n"
        "def main(argv):\n"
        "    sys.path.insert(0, " + repr(str(repo_root)) + ")\n"
        "    import busy\n"
        "    assert busy.run() == 499500\n"
        "    return 0\n"
    )

    payload = _run_pytest_trace_runner(
        tmp_path, fake_pytest_pkg=fake_pytest, repo_root=repo_root, argv=["-rA"], max_events=5
    )

    assert len(payload["trace_events"]) == 5
    assert payload["coverage"]["events_recorded"] == 5
    assert payload["coverage"]["truncated"] is True
    assert payload["runtime_metrics"]["trace_stopped_at_max_events"] is True
    validate_trace_payload(payload)


def test_validate_rejects_non_execution_outcome_without_captured_exception():
    payload = {
        "execution_outcome": "import_error",
        "captured_exception": None,
        "coverage": {"lines_executed": 0},
        "line_coverage": [],
        "runtime_metrics": {},
        "steps": {},
        "trace_events": [],
    }
    with pytest.raises(ExecutionTraceSmokeError) as exc:
        validate_trace_payload(payload)
    assert "WITHOUT_CAPTURED_EXCEPTION" in str(exc.value)


def test_validate_rejects_captured_exception_without_location():
    payload = {
        "execution_outcome": "compile_error",
        "captured_exception": {"type": "SyntaxError", "message": "bad"},
        "coverage": {"lines_executed": 0},
        "line_coverage": [],
        "runtime_metrics": {},
        "steps": {},
        "trace_events": [],
    }
    with pytest.raises(ExecutionTraceSmokeError) as exc:
        validate_trace_payload(payload)
    assert "CAPTURED_EXCEPTION_LOCATION_MISSING" in str(exc.value)


def test_validate_rejects_executed_outcome_with_empty_trace():
    payload = {
        "execution_outcome": "executed",
        "captured_exception": None,
        "coverage": {"lines_executed": 0},
        "line_coverage": [],
        "runtime_metrics": {},
        "steps": {},
        "trace_events": [],
    }
    with pytest.raises(ExecutionTraceSmokeError) as exc:
        validate_trace_payload(payload)
    assert "TRACE_EVENTS_EMPTY" in str(exc.value)


def _write_run_log(tmp_path: Path, text: str) -> tuple[Path, Path]:
    log_dir = tmp_path / "log_dir"
    log_dir.mkdir(parents=True, exist_ok=True)
    (log_dir / "run_instance.log").write_text(text, encoding="utf-8")
    return log_dir, log_dir / "test_output.txt"


def test_classify_missing_output_detects_interruption(tmp_path: Path):
    log_dir, test_output = _write_run_log(
        tmp_path, "Creating container...\nEXECUTION_TRACE_PYTEST_BATCH_INTERRUPTED: SIGALRM\n"
    )
    code, detail = classify_missing_test_output(log_dir, test_output, 900)
    assert code == "EXECUTION_TRACE_SMOKE_RUN_INTERRUPTED_BEFORE_OUTPUT"
    assert detail["instance_timeout_s"] == 900


def test_classify_missing_output_detects_timeout(tmp_path: Path):
    log_dir, test_output = _write_run_log(tmp_path, "Test timed out after 1800s\n")
    code, _ = classify_missing_test_output(log_dir, test_output, 1800)
    assert code == "EXECUTION_TRACE_SMOKE_INSTANCE_TIMED_OUT_BEFORE_OUTPUT"


def test_classify_missing_output_detects_eval_error(tmp_path: Path):
    log_dir, test_output = _write_run_log(tmp_path, "Error in evaluating model for x\nTraceback...\n")
    code, _ = classify_missing_test_output(log_dir, test_output, 1800)
    assert code == "EXECUTION_TRACE_SMOKE_RUN_EVALUATION_ERROR_BEFORE_OUTPUT"


def test_detect_swebench_timeout_marker():
    out = "test_requests.py .......F\nTimeout error: 90 seconds exceeded.\n"
    assert detect_swebench_timeout(out) == "Timeout error: 90 seconds exceeded."
    assert detect_swebench_timeout(">>>>> Test Timed Out after 1800s") is not None
    assert detect_swebench_timeout("all good\n1 passed\n") is None


def test_classify_missing_output_no_log(tmp_path: Path):
    log_dir = tmp_path / "empty"
    log_dir.mkdir()
    code, detail = classify_missing_test_output(log_dir, log_dir / "test_output.txt", 1800)
    assert code == "EXECUTION_TRACE_SMOKE_TEST_OUTPUT_MISSING_NO_RUN_LOG"
    assert detail["log_present"] is False


def test_trace_runner_source_avoids_future_annotations_for_python36_testbeds():
    source = build_trace_runner_source()

    assert "from __future__ import annotations" not in source
    compile(source, "contextgraph_trace_pytest.py", "exec")


def test_python_script_trace_runner_source_avoids_future_annotations_for_python36_testbeds():
    source = build_python_script_trace_runner_source()

    assert "from __future__ import annotations" not in source
    compile(source, "contextgraph_trace_python_script.py", "exec")


def test_transform_wraps_single_pytest_command_after_trace_runner():
    eval_script = [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": 'START TEST OUTPUT'",
        "pytest -rA test_requests.py",
        ": 'END TEST OUTPUT'",
        "git checkout base test_requests.py",
    ]

    transformed, metadata = transform_eval_script_list(eval_script, max_events=12)

    wrapped_index = transformed.index("python /tmp/contextgraph_trace_pytest.py -rA test_requests.py")
    end_index = transformed.index(": 'END TEST OUTPUT'")
    begin_marker_index = transformed.index("  echo CONTEXTGRAPH_EXECUTION_TRACE_BEGIN")
    assert metadata["original_test_command"] == "pytest -rA test_requests.py"
    assert metadata["wrapped_test_command"] == "python /tmp/contextgraph_trace_pytest.py -rA test_requests.py"
    assert any(line == "export CONTEXTGRAPH_TRACE_MAX_EVENTS=12" for line in transformed)
    assert wrapped_index > transformed.index("CONTEXTGRAPH_TRACE_PY")
    assert begin_marker_index > end_index


def test_transform_exports_soft_timeout_when_requested():
    eval_script = [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": 'START TEST OUTPUT'",
        "pytest -rA test_requests.py",
        ": 'END TEST OUTPUT'",
    ]

    transformed, metadata = transform_eval_script_list(eval_script, max_events=12, soft_timeout_seconds=44)

    assert metadata["soft_timeout_seconds"] == 44
    assert any(line == "export CONTEXTGRAPH_TRACE_SOFT_TIMEOUT_SECONDS=44" for line in transformed)


def test_transform_wraps_django_runtests_command_after_trace_runner():
    eval_script = [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": '>>>>> Start Test Output'",
        "./tests/runtests.py --verbosity 2 --settings=test_sqlite --parallel 1 test_utils.tests",
        ": '>>>>> End Test Output'",
        "git checkout base tests/runtests.py",
    ]

    transformed, metadata = transform_eval_script_list(eval_script, max_events=12, command_family="django_runtests")

    wrapped = (
        "python /tmp/contextgraph_trace_python_script.py ./tests/runtests.py "
        "--verbosity 2 --settings=test_sqlite --parallel 1 test_utils.tests"
    )
    wrapped_index = transformed.index(wrapped)
    end_index = transformed.index(": '>>>>> End Test Output'")
    begin_marker_index = transformed.index("  echo CONTEXTGRAPH_EXECUTION_TRACE_BEGIN")
    assert metadata["command_family"] == "django_runtests"
    assert metadata["handler"] == "python_script_sys_settrace_v1"
    assert metadata["script_path"] == "./tests/runtests.py"
    assert metadata["script_args"] == ["--verbosity", "2", "--settings=test_sqlite", "--parallel", "1", "test_utils.tests"]
    assert any(line == "export CONTEXTGRAPH_TRACE_MAX_EVENTS=12" for line in transformed)
    assert wrapped_index > transformed.index("CONTEXTGRAPH_TRACE_SCRIPT_PY")
    assert begin_marker_index > end_index


def test_transform_wraps_sympy_bin_test_command_after_trace_runner():
    eval_script = [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": '>>>>> Start Test Output'",
        "PYTHONWARNINGS='ignore::UserWarning,ignore::SyntaxWarning' bin/test -C --verbose sympy/printing/tests/test_ccode.py",
        ": '>>>>> End Test Output'",
        "git checkout base sympy/printing/tests/test_ccode.py",
    ]

    transformed, metadata = transform_eval_script_list(eval_script, max_events=12, command_family="sympy_bin_test")

    wrapped = (
        "env PYTHONWARNINGS=ignore::UserWarning,ignore::SyntaxWarning "
        "python /tmp/contextgraph_trace_python_script.py bin/test "
        "-C --verbose sympy/printing/tests/test_ccode.py"
    )
    wrapped_index = transformed.index(wrapped)
    end_index = transformed.index(": '>>>>> End Test Output'")
    begin_marker_index = transformed.index("  echo CONTEXTGRAPH_EXECUTION_TRACE_BEGIN")
    assert metadata["command_family"] == "sympy_bin_test"
    assert metadata["handler"] == "python_script_sys_settrace_v1"
    assert metadata["script_path"] == "bin/test"
    assert metadata["script_args"] == ["-C", "--verbose", "sympy/printing/tests/test_ccode.py"]
    assert metadata["preserved_env"] == ["PYTHONWARNINGS=ignore::UserWarning,ignore::SyntaxWarning"]
    assert any(line == "export CONTEXTGRAPH_TRACE_MAX_EVENTS=12" for line in transformed)
    assert any(line == "export CONTEXTGRAPH_TRACE_EXIT_AT_MAX_EVENTS=1" for line in transformed)
    assert wrapped_index > transformed.index("CONTEXTGRAPH_TRACE_SCRIPT_PY")
    assert begin_marker_index > end_index


def test_transform_rejects_multiple_sympy_bin_test_commands():
    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_UNSUPPORTED_TEST_COMMAND"):
        transform_eval_script_list(
            [
                "bin/test -C --verbose sympy/tests/test_a.py",
                "bin/test -C --verbose sympy/tests/test_b.py",
            ],
            max_events=10,
            command_family="sympy_bin_test",
        )


def test_transform_wraps_tox_current_env_passthrough_with_pytest_runner():
    eval_script = [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": '>>>>> Start Test Output'",
        "tox --current-env -epy39 -v -- tests/test_ext_viewcode.py tests/test_domain_std.py",
        ": '>>>>> End Test Output'",
        "git checkout base sphinx/ext/viewcode.py",
    ]

    transformed, metadata = transform_eval_script_list(eval_script, max_events=12, command_family="tox")

    wrapped = "python /tmp/contextgraph_trace_pytest.py tests/test_ext_viewcode.py tests/test_domain_std.py"
    wrapped_index = transformed.index(wrapped)
    end_index = transformed.index(": '>>>>> End Test Output'")
    begin_marker_index = transformed.index("  echo CONTEXTGRAPH_EXECUTION_TRACE_BEGIN")
    assert metadata["command_family"] == "tox"
    assert metadata["handler"] == "tox_current_env_pytest_sys_settrace_v1"
    assert metadata["tox_executable"] == "tox"
    assert metadata["tox_args"] == ["--current-env", "-epy39", "-v", "--", "tests/test_ext_viewcode.py", "tests/test_domain_std.py"]
    assert metadata["tox_passthrough_args"] == ["tests/test_ext_viewcode.py", "tests/test_domain_std.py"]
    assert metadata["pytest_args"] == ["tests/test_ext_viewcode.py", "tests/test_domain_std.py"]
    assert any(line == "export CONTEXTGRAPH_TRACE_MAX_EVENTS=12" for line in transformed)
    assert wrapped_index > transformed.index("CONTEXTGRAPH_TRACE_PY")
    assert begin_marker_index > end_index


def test_transform_wraps_tox_current_env_with_preserved_env_prefix():
    transformed, metadata = transform_eval_script_list(
        [
            ": '>>>>> Start Test Output'",
            "PYTHONWARNINGS=ignore tox --current-env -epy39 -v -- tests/test_ext_viewcode.py",
            ": '>>>>> End Test Output'",
        ],
        max_events=12,
        command_family="tox",
    )

    assert "CONTEXTGRAPH_TRACE_PY" in transformed
    assert "env PYTHONWARNINGS=ignore python /tmp/contextgraph_trace_pytest.py tests/test_ext_viewcode.py" in transformed
    assert metadata["preserved_env"] == ["PYTHONWARNINGS=ignore"]


def test_transform_rejects_tox_without_current_env():
    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_UNSUPPORTED_TOX_REQUIRES_CURRENT_ENV"):
        transform_eval_script_list(
            [
                ": '>>>>> Start Test Output'",
                "tox -epy39 -v -- tests/test_ext_viewcode.py",
                ": '>>>>> End Test Output'",
            ],
            max_events=10,
            command_family="tox",
        )


def test_transform_rejects_tox_without_passthrough_tests():
    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_UNSUPPORTED_TOX_REQUIRES_PASSTHROUGH"):
        transform_eval_script_list(
            [
                ": '>>>>> Start Test Output'",
                "tox --current-env -epy39 -v",
                ": '>>>>> End Test Output'",
            ],
            max_events=10,
            command_family="tox",
        )


def test_python_script_runner_executes_synthetic_script_and_writes_trace(tmp_path: Path):
    repo = tmp_path / "repo"
    script = repo / "tests" / "runtests.py"
    script.parent.mkdir(parents=True)
    (script.parent / "helper_for_runtests.py").write_text("expected_value = 4\n", encoding="utf-8")
    script.write_text(
        "\n".join(
            [
                "import sys",
                "from helper_for_runtests import expected_value",
                "value = 2",
                "if sys.argv[1] != 'expected':",
                "    raise SystemExit(5)",
                "result = value + 2",
                "raise SystemExit(0 if result == expected_value else 6)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    runner = tmp_path / "contextgraph_trace_python_script.py"
    output = tmp_path / "trace.json"
    runner.write_text(build_python_script_trace_runner_source(), encoding="utf-8")
    env = dict(os.environ)
    env.update(
        {
            "CONTEXTGRAPH_TRACE_OUTPUT": str(output),
            "CONTEXTGRAPH_TRACE_REPO_ROOT": str(repo),
            "CONTEXTGRAPH_TRACE_MAX_EVENTS": "50",
        }
    )

    completed = subprocess.run(
        [sys.executable, str(runner), str(script), "expected"],
        check=False,
        env=env,
        text=True,
        capture_output=True,
    )

    assert completed.returncode == 0
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["producer"] == "contextgraph.execution_trace_smoke.python_script_sys_settrace"
    assert payload["runtime_metrics"]["python_script_exit_code"] == 0
    assert payload["runtime_metrics"]["script_dir"] == str(script.parent)
    assert payload["steps"]["script_args"] == ["expected"]
    assert payload["coverage"]["lines_executed"] > 0
    assert payload["trace_events"]


def test_python_script_runner_stops_tracing_at_max_events(tmp_path: Path):
    repo = tmp_path / "repo"
    script = repo / "tests" / "runtests.py"
    script.parent.mkdir(parents=True)
    script.write_text(
        "\n".join(
            [
                "total = 0",
                "for i in range(1000):",
                "    total += i",
                "raise SystemExit(0 if total == 499500 else 6)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    runner = tmp_path / "contextgraph_trace_python_script.py"
    output = tmp_path / "trace.json"
    runner.write_text(build_python_script_trace_runner_source(), encoding="utf-8")
    env = dict(os.environ)
    env.update(
        {
            "CONTEXTGRAPH_TRACE_OUTPUT": str(output),
            "CONTEXTGRAPH_TRACE_REPO_ROOT": str(repo),
            "CONTEXTGRAPH_TRACE_MAX_EVENTS": "6",
        }
    )

    completed = subprocess.run(
        [sys.executable, str(runner), str(script)],
        check=False,
        env=env,
        text=True,
        capture_output=True,
        timeout=120,
    )

    assert completed.returncode == 0
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["producer"] == "contextgraph.execution_trace_smoke.python_script_sys_settrace"
    assert len(payload["trace_events"]) == 6
    assert payload["coverage"]["events_recorded"] == 6
    assert payload["coverage"]["truncated"] is True
    assert payload["runtime_metrics"]["trace_stopped_at_max_events"] is True
    validate_trace_payload(payload)


def test_python_script_runner_can_exit_at_max_events(tmp_path: Path):
    repo = tmp_path / "repo"
    script = repo / "bin" / "test"
    script.parent.mkdir(parents=True)
    script.write_text(
        "\n".join(
            [
                "total = 0",
                "for i in range(1000):",
                "    total += i",
                "raise SystemExit(9)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    runner = tmp_path / "contextgraph_trace_python_script.py"
    output = tmp_path / "trace.json"
    runner.write_text(build_python_script_trace_runner_source(), encoding="utf-8")
    env = dict(os.environ)
    env.update(
        {
            "CONTEXTGRAPH_TRACE_OUTPUT": str(output),
            "CONTEXTGRAPH_TRACE_REPO_ROOT": str(repo),
            "CONTEXTGRAPH_TRACE_MAX_EVENTS": "6",
            "CONTEXTGRAPH_TRACE_EXIT_AT_MAX_EVENTS": "1",
        }
    )

    completed = subprocess.run(
        [sys.executable, str(runner), str(script)],
        check=False,
        env=env,
        text=True,
        capture_output=True,
        timeout=120,
    )

    assert completed.returncode == 0
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["execution_outcome"] == "max_events_reached"
    assert payload["captured_exception"]["type"] == "TraceMaxEventsReached"
    assert payload["runtime_metrics"]["python_script_exit_code"] == 0
    assert payload["runtime_metrics"]["exit_at_max_events"] is True
    assert payload["runtime_metrics"]["trace_stopped_at_max_events"] is True
    assert payload["coverage"]["truncated"] is True
    assert len(payload["trace_events"]) == 6
    validate_trace_payload(payload)


def test_python_script_runner_writes_payload_on_soft_timeout(tmp_path: Path):
    repo = tmp_path / "repo"
    script = repo / "tests" / "runtests.py"
    script.parent.mkdir(parents=True)
    script.write_text(
        "\n".join(
            [
                "import time",
                "total = 0",
                "for i in range(10):",
                "    total += i",
                "try:",
                "    time.sleep(20)",
                "except Exception:",
                "    raise SystemExit(7)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    runner = tmp_path / "contextgraph_trace_python_script.py"
    output = tmp_path / "trace.json"
    runner.write_text(build_python_script_trace_runner_source(), encoding="utf-8")
    env = dict(os.environ)
    env.update(
        {
            "CONTEXTGRAPH_TRACE_OUTPUT": str(output),
            "CONTEXTGRAPH_TRACE_REPO_ROOT": str(repo),
            "CONTEXTGRAPH_TRACE_MAX_EVENTS": "500",
            "CONTEXTGRAPH_TRACE_SOFT_TIMEOUT_SECONDS": "1",
        }
    )

    completed = subprocess.run(
        [sys.executable, str(runner), str(script)],
        check=False,
        env=env,
        text=True,
        capture_output=True,
        timeout=20,
    )

    assert completed.returncode == 124
    payload = json.loads(output.read_text(encoding="utf-8"))
    assert payload["execution_outcome"] == "run_timeout"
    assert payload["coverage"]["execution_outcome"] == "run_timeout"
    assert payload["runtime_metrics"]["python_script_exit_code"] == 124
    assert payload["runtime_metrics"]["soft_timeout_s"] == 1
    assert payload["captured_exception"]["type"] == "TraceSoftTimeout"
    assert payload["trace_events"]
    validate_trace_payload(payload)


def test_transform_rejects_non_pytest_command():
    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_UNSUPPORTED_TEST_COMMAND"):
        transform_eval_script_list(["./tests/runtests.py migrations.test_commands"], max_events=10)


def test_transform_rejects_multiple_pytest_commands():
    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_UNSUPPORTED_TEST_COMMAND"):
        transform_eval_script_list(["pytest tests/a.py", "pytest tests/b.py"], max_events=10)


def test_extract_trace_payload_validates_live_trace_keys():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [{"event": "line", "file": "pkg/mod.py", "line": 1, "function": "f"}],
    }
    output = f"before\n{TRACE_BEGIN}\n{json.dumps(payload)}\n{TRACE_END}\nafter\n"

    parsed = extract_trace_payload(output)

    assert parsed["coverage"]["lines_executed"] == 1
    assert parsed["trace_events"][0]["event"] == "line"


def test_extract_trace_payload_tolerates_bash_xtrace_noise():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [{"event": "line", "file": "pkg/mod.py", "line": 1, "function": "f"}],
    }
    output = "\n".join(
        [
            TRACE_BEGIN,
            "+ echo CONTEXTGRAPH_EXECUTION_TRACE_BEGIN",
            "+ cat /tmp/contextgraph_execution_trace.json",
            json.dumps(payload),
            TRACE_END,
        ]
    )

    parsed = extract_trace_payload(output)

    assert parsed["coverage"]["events_seen"] == 2


def test_extract_trace_payload_strips_xtrace_fragment_inside_json_body():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [
            {"event": "line", "file": "pkg/mod.py", "line": 1, "function": "f"},
            {"event": "return", "file": "pkg/mod.py", "line": 2, "function": "f"},
        ],
    }
    encoded = json.dumps(payload)
    split_at = encoded.index('"return"')
    contaminated = (
        encoded[:split_at]
        + "+ git checkout d16bfe05 astropy/modeling/tests/test_separable.py\n"
        + "Updated 2 paths from 5831e739\n"
        + encoded[split_at:]
    )
    output = f"{TRACE_BEGIN}\n{contaminated}\n{TRACE_END}\n"

    parsed = extract_trace_payload(output)

    assert len(parsed["trace_events"]) == 2
    assert parsed["trace_events"][1]["event"] == "return"


def test_extract_trace_payload_strips_git_checkout_output_fragment_inside_json_body():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [
            {"event": "call", "file": "pkg/mod.py", "line": 1, "function": "f"},
            {"event": "line", "file": "pkg/mod.py", "line": 2, "function": "f"},
        ],
    }
    encoded = json.dumps(payload)
    split_at = encoded.index('"line"')
    contaminated = encoded[:split_at] + "Updated 2 paths from 5831e739\n" + encoded[split_at:]
    output = f"{TRACE_BEGIN}\n{contaminated}\n{TRACE_END}\n"

    parsed = extract_trace_payload(output)

    assert len(parsed["trace_events"]) == 2
    assert parsed["trace_events"][1]["event"] == "line"


def test_extract_trace_payload_strips_git_checkout_fragment_inside_json_number():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:412", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [
            {"event": "line", "file": "pkg/mod.py", "line": 412, "function": "f"},
        ],
    }
    encoded = json.dumps(payload)
    split_at = encoded.index('"line": 412') + len('"line": 41')
    contaminated = encoded[:split_at] + "+ git checkout 475cffd tests/admin_views/tests.py\n" + encoded[split_at:]
    output = f"{TRACE_BEGIN}\n{contaminated}\n{TRACE_END}\n"

    parsed = extract_trace_payload(output)

    assert parsed["trace_events"][0]["line"] == 412


def test_extract_trace_payload_repairs_log_wrapped_json_string():
    payload = {
        "coverage": {"lines_executed": 1, "files_touched": 1, "events_seen": 2},
        "line_coverage": [{"location": "pkg/mod.py:1", "count": 1}],
        "runtime_metrics": {"elapsed_ms": 3, "pytest_exit_code": 0},
        "steps": {"event_counts": {"line": 1}},
        "trace_events": [
            {"event": "line", "file": "pylint/config/utils.py", "line": 144, "function": "f"},
            {"event": "return", "file": "pkg/mod.py", "line": 2, "function": "f"},
        ],
    }
    encoded = json.dumps(payload)
    split_at = encoded.index("pylint") + 1
    contaminated = encoded[:split_at] + "\n" + encoded[split_at:]
    output = f"{TRACE_BEGIN}\n{contaminated}\n{TRACE_END}\n"

    parsed = extract_trace_payload(output)

    assert parsed["trace_events"][0]["file"] == "pylint/config/utils.py"


def test_extract_trace_payload_fails_closed_without_events():
    payload = {
        "coverage": {"lines_executed": 0},
        "line_coverage": [],
        "runtime_metrics": {},
        "steps": {},
        "trace_events": [],
    }
    output = f"{TRACE_BEGIN}\n{json.dumps(payload)}\n{TRACE_END}\n"

    with pytest.raises(ExecutionTraceSmokeError, match="EXECUTION_TRACE_SMOKE_TRACE_EVENTS_EMPTY"):
        extract_trace_payload(output)


def test_extract_trace_payload_infers_pytest_collection_syntax_error():
    payload = {
        "coverage": {"events_recorded": 0, "events_seen": 0, "files_touched": 0, "lines_executed": 0},
        "line_coverage": [],
        "producer": "contextgraph.execution_trace_smoke.pytest_sys_settrace",
        "runtime_metrics": {"elapsed_ms": 113, "max_events": 5000, "pytest_exit_code": 4},
        "schema_version": 1,
        "steps": {"event_counts": {}, "pytest_args": ["-rA", "lib/matplotlib/tests/test_rcparams.py"]},
        "trace_events": [],
        "variable_states": [],
    }
    output = "\n".join(
        [
            "ImportError while loading conftest '/testbed/lib/matplotlib/tests/conftest.py'.",
            '  File "/testbed/lib/matplotlib/__init__.py", line 1464',
            "    def _bad_(:",
            "              ^",
            "SyntaxError: invalid syntax",
            TRACE_BEGIN,
            json.dumps(payload),
            TRACE_END,
        ]
    )

    parsed = extract_trace_payload(output)

    assert parsed["execution_outcome"] == "compile_error"
    assert parsed["coverage"]["execution_outcome"] == "compile_error"
    assert parsed["runtime_metrics"]["execution_outcome"] == "compile_error"
    captured = parsed["captured_exception"]
    assert captured["type"] == "SyntaxError"
    assert captured["filename"] == "/testbed/lib/matplotlib/__init__.py"
    assert captured["lineno"] == 1464
    assert captured["text"] == "def _bad_(:"
    assert parsed["steps"]["non_execution_inference"]["source"] == "pytest_collection_syntax_error_output"


def test_load_corpus_entry_resolves_patch_and_hash(tmp_path: Path):
    corpus = tmp_path / "corpus"
    patch = corpus / "mutations" / "org__repo-1" / "known_good.patch"
    patch.parent.mkdir(parents=True)
    patch.write_text("diff --git a/a.py b/a.py\n", encoding="utf-8")
    write_json(
        corpus / "index.json",
        {
            "entries": [
                {
                    "task_id": "org__repo-1",
                    "category": "known_good",
                    "repo": "org/repo",
                    "patch_path": "mutations/org__repo-1/known_good.patch",
                }
            ]
        },
    )

    row = load_corpus_entry(corpus, "org__repo-1", "known_good")

    assert row["patch_abspath"] == str(patch)
    assert row["patch_sha256"].startswith("sha256:")
