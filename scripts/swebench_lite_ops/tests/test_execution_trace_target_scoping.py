"""Integration proof for the INV-010 #774 target-file scoping fix.

Runs the ACTUAL in-container trace harness (build_trace_runner_source) as a
subprocess — exactly as it runs inside the SWE-bench Docker image — against a
synthetic repo that reproduces the deployed failure mode:
  * pkg/__init__.py: framework import-time boilerplate (the django/__init__.py
    analogue that exhausted the budget in production).
  * pkg/calc.py: the diff-touched source file carrying a correct or off-by-one
    function — the execution that must diverge between pass and fail.
  * test_calc.py: the test asserting the behaviour.

X+X=Y: scoping the trace to {calc.py, test_calc.py} must (a) EXCLUDE the
__init__.py boilerplate and (b) make the correct vs buggy traces DIVERGE
(different loop line-counts + an AssertionError exception event on the failing
run). The legacy repo_root mode is shown to still capture the boilerplate.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

from ..python_execution_trace_smoke import build_trace_runner_source


INIT_SRC = textwrap.dedent(
    """
    # Framework import-time boilerplate (django/__init__.py analogue).
    _WARMUP = [i * i for i in range(200)]
    VERSION = "9.9.9"
    def setup():
        return sum(_WARMUP)
    setup()
    """
)

CALC_CORRECT = textwrap.dedent(
    """
    def compute(n):
        total = 0
        for i in range(n):       # correct: sum 0..n-1
            total += i
        return total
    """
)

CALC_BUGGY = textwrap.dedent(
    """
    def compute(n):
        total = 0
        for i in range(n + 1):   # off-by-one: sum 0..n
            total += i
        return total
    """
)

TEST_SRC = textwrap.dedent(
    """
    from pkg.calc import compute
    def test_compute():
        assert compute(5) == 10
    """
)


def _make_repo(root: Path, calc_src: str) -> Path:
    (root / "pkg").mkdir(parents=True, exist_ok=True)
    (root / "pkg" / "__init__.py").write_text(INIT_SRC, encoding="utf-8")
    (root / "pkg" / "calc.py").write_text(calc_src, encoding="utf-8")
    (root / "test_calc.py").write_text(TEST_SRC, encoding="utf-8")
    return root


def _run_harness(repo: Path, target_files: str | None, tmp: Path, tag: str) -> dict:
    harness = tmp / f"harness_{tag}.py"
    harness.write_text(build_trace_runner_source(), encoding="utf-8")
    out = tmp / f"trace_{tag}.json"
    env = dict(os.environ)
    env["CONTEXTGRAPH_TRACE_OUTPUT"] = str(out)
    env["CONTEXTGRAPH_TRACE_REPO_ROOT"] = str(repo)
    env["CONTEXTGRAPH_TRACE_MAX_EVENTS"] = "5000"
    env["PYTHONPATH"] = str(repo) + os.pathsep + env.get("PYTHONPATH", "")
    if target_files is not None:
        env["CONTEXTGRAPH_TRACE_TARGET_FILES"] = target_files
    else:
        env.pop("CONTEXTGRAPH_TRACE_TARGET_FILES", None)
    proc = subprocess.run(
        [sys.executable, str(harness), "test_calc.py", "-p", "no:cacheprovider", "-q"],
        cwd=str(repo), env=env, capture_output=True, text=True, timeout=120,
    )
    assert out.is_file(), f"harness wrote no trace ({tag}): {proc.stderr[-2000:]}"
    return json.loads(out.read_text(encoding="utf-8"))


def _files_in_trace(trace: dict) -> set[str]:
    return {e["file"] for e in trace.get("trace_events", [])}


@pytest.mark.skipif(__import__("importlib").util.find_spec("pytest") is None, reason="pytest required")
def test_target_scoping_excludes_boilerplate_and_diverges(tmp_path: Path) -> None:
    targets = "pkg/calc.py\ntest_calc.py"
    ok = _run_harness(_make_repo(tmp_path / "ok", CALC_CORRECT), targets, tmp_path, "ok")
    bug = _run_harness(_make_repo(tmp_path / "bug", CALC_BUGGY), targets, tmp_path, "bug")

    # (a) scope_mode recorded and boilerplate EXCLUDED on both runs.
    assert ok["runtime_metrics"]["scope_mode"] == "target_files", ok["runtime_metrics"]
    ok_files = _files_in_trace(ok)
    bug_files = _files_in_trace(bug)
    assert "pkg/calc.py" in ok_files, ok_files
    assert "pkg/__init__.py" not in ok_files, ok_files  # boilerplate excluded
    assert "pkg/__init__.py" not in bug_files, bug_files

    # (b) the diff-touched file's execution DIVERGES between pass and fail:
    # the off-by-one loop body executes one extra iteration.
    def loop_line_hits(trace):
        return sum(1 for e in trace["trace_events"] if e["file"] == "pkg/calc.py" and e["event"] == "line")
    assert loop_line_hits(bug) > loop_line_hits(ok), (loop_line_hits(ok), loop_line_hits(bug))

    # (c) the failing run raises AssertionError (captured exception event); the
    # passing run does not.
    ok_exc = ok["steps"]["event_counts"].get("exception", 0)
    bug_exc = bug["steps"]["event_counts"].get("exception", 0)
    assert bug_exc >= 1, bug["steps"]["event_counts"]
    assert ok_exc == 0, ok["steps"]["event_counts"]
    assert ok["runtime_metrics"]["pytest_exit_code"] == 0
    assert bug["runtime_metrics"]["pytest_exit_code"] != 0


def test_legacy_repo_root_mode_still_captures_boilerplate(tmp_path: Path) -> None:
    # Without targets, the old behaviour records the framework's import-time
    # __init__.py events -- the deployed defect this fix addresses.
    trace = _run_harness(_make_repo(tmp_path / "legacy", CALC_CORRECT), None, tmp_path, "legacy")
    assert trace["runtime_metrics"]["scope_mode"] == "repo_root"
    assert "pkg/__init__.py" in _files_in_trace(trace), _files_in_trace(trace)
