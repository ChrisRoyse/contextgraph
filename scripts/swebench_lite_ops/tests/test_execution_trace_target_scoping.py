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

import argparse
import types

from .. import python_execution_trace_smoke as smoke
from ..python_execution_trace_smoke import (
    ExecutionTraceSmokeError,
    _heredoc_script_lines,
    build_trace_runner_source,
    diff_touched_files,
    run_trace_smoke,
)


SAMPLE_PATCH = """diff --git a/src/pkg/calc.py b/src/pkg/calc.py
index 1111..2222 100644
--- a/src/pkg/calc.py
+++ b/src/pkg/calc.py
@@ -1,3 +1,3 @@
-    for i in range(n):
+    for i in range(n + 1):
diff --git a/newfile.py b/newfile.py
new file mode 100644
--- /dev/null
+++ b/newfile.py
"""

TEST_PATCH = """--- a/tests/test_calc.py
+++ b/tests/test_calc.py
@@ -1 +1 @@
-x
+y
"""


def test_diff_touched_files_parses_and_dedups() -> None:
    files = diff_touched_files(SAMPLE_PATCH, TEST_PATCH)
    assert files == ["newfile.py", "src/pkg/calc.py", "tests/test_calc.py"], files
    # /dev/null excluded; a//b/ prefixes stripped; deduped across both --- and +++.


def test_diff_touched_files_empty_inputs() -> None:
    assert diff_touched_files("", "   ") == []


def test_heredoc_emits_target_files_export_only_when_provided() -> None:
    with_targets = _heredoc_script_lines(
        5000, "pytest_direct", 0, ["src/pkg/calc.py", "tests/test_calc.py"]
    )
    joined = "\n".join(with_targets)
    assert "export CONTEXTGRAPH_TRACE_TARGET_FILES=" in joined
    assert "src/pkg/calc.py,tests/test_calc.py" in joined
    # The runner SOURCE always references the env var (it reads it); assert on the
    # shell EXPORT statement, which is only emitted when targets are provided.
    without = "\n".join(_heredoc_script_lines(5000, "pytest_direct", 0, None))
    assert "export CONTEXTGRAPH_TRACE_TARGET_FILES=" not in without
    blanks = "\n".join(_heredoc_script_lines(5000, "pytest_direct", 0, ["  ", ""]))
    assert "export CONTEXTGRAPH_TRACE_TARGET_FILES=" not in blanks  # whitespace filtered


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


def test_run_trace_smoke_computes_and_passes_target_files(tmp_path: Path, monkeypatch) -> None:
    """run_trace_smoke must derive trace targets from the REAL candidate patch +
    test_patch and pass them into transform_eval_script_list. Stubs the Docker /
    dataset deps; intercepts transform to capture the target_files it receives."""
    candidate = (
        "--- a/pkg/mod.py\n+++ b/pkg/mod.py\n@@ -1 +1 @@\n-    return n\n+    return n + 1\n"
    )
    test_patch = "--- a/tests/test_mod.py\n+++ b/tests/test_mod.py\n@@ -1 +1 @@\n-x\n+y\n"
    patch_file = tmp_path / "cand.patch"
    patch_file.write_text(candidate, encoding="utf-8")

    monkeypatch.setattr(smoke, "load_corpus_entry", lambda *a, **k: {"patch_abspath": str(patch_file)})
    monkeypatch.setattr(smoke, "load_dataset_instance", lambda *a, **k: {"test_patch": test_patch})
    fake_runtime = {
        "make_test_spec": lambda *a, **k: types.SimpleNamespace(eval_script_list=["pytest tests/test_mod.py"]),
    }
    monkeypatch.setattr(smoke, "import_swebench_runtime", lambda: fake_runtime)

    captured: dict = {}

    def fake_transform(script, **kw):
        captured["target_files"] = kw.get("target_files")
        raise ExecutionTraceSmokeError("STOP_AFTER_TRANSFORM")

    monkeypatch.setattr(smoke, "transform_eval_script_list", fake_transform)

    args = argparse.Namespace(
        corpus_root=tmp_path / "corpus",
        output_root=tmp_path / "out",
        state_root=tmp_path / "state",
        venv_python=Path(sys.executable),
        allow_non_aiwonder_roots=True,
        run_id="run-target-test",
        task_id="org__repo-1",
        category="subtle_flip",
        dataset_name="SWE-bench/SWE-bench_Lite",
        split="test",
        namespace="ns",
        tag="latest",
        arch="x86_64",
        instance_timeout=60,
        trace_soft_timeout_margin=30,
        max_events=5000,
        command_family="pytest_direct",
    )
    with pytest.raises(ExecutionTraceSmokeError, match="STOP_AFTER_TRANSFORM"):
        run_trace_smoke(args)
    # the candidate's changed source file AND the test file are both targeted
    assert captured["target_files"] == ["pkg/mod.py", "tests/test_mod.py"], captured


def test_run_trace_smoke_fails_closed_when_no_target_files(tmp_path: Path, monkeypatch) -> None:
    # An empty / non-diff candidate patch yields no targets -> fail-visible, not a
    # silent unscoped (boilerplate) trace.
    patch_file = tmp_path / "empty.patch"
    patch_file.write_text("not a diff at all\n", encoding="utf-8")
    monkeypatch.setattr(smoke, "load_corpus_entry", lambda *a, **k: {"patch_abspath": str(patch_file)})
    monkeypatch.setattr(smoke, "load_dataset_instance", lambda *a, **k: {"test_patch": ""})
    monkeypatch.setattr(
        smoke, "import_swebench_runtime",
        lambda: {"make_test_spec": lambda *a, **k: types.SimpleNamespace(eval_script_list=["pytest x"])},
    )
    args = argparse.Namespace(
        corpus_root=tmp_path / "c", output_root=tmp_path / "o", state_root=tmp_path / "s",
        venv_python=Path(sys.executable), allow_non_aiwonder_roots=True, run_id="run-z",
        task_id="org__repo-1", category="known_good", dataset_name="d", split="test",
        namespace="ns", tag="latest", arch="x86_64", instance_timeout=60,
        trace_soft_timeout_margin=30, max_events=5000, command_family="pytest_direct",
    )
    with pytest.raises(ExecutionTraceSmokeError, match="NO_DIFF_TOUCHED_TARGET_FILES"):
        run_trace_smoke(args)
