"""Proof that the scoped tracer recovers oracle-discriminative signal that the
import-boilerplate tracer destroys (INV-010 / #774 root-cause fix).

X+X=Y design (real code, no mocks): the SAME test harness runs a CORRECT and a
BUGGY version of a target function. The oracle-equivalent label is "did the test
assertion pass". A useful execution trace MUST differ between the two. We prove:
  1. Scoped tracing captures the in-project target execution and the correct vs
     buggy runs DIVERGE (exception event + uncaught type + coverage).
  2. When the signal file is OUT of scope (the real failure mode: budget burned on
     imports, target never recorded), the trace is near-empty and identical across
     correct/buggy -> zero signal. This is exactly why the deployed traces read 0.036 bits.
"""
from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from ..python_scoped_execution_tracer import ScopedTracer


CORRECT_SRC = textwrap.dedent(
    '''
    import json  # out-of-scope import noise (stdlib) — must NOT consume the budget
    import collections

    def compute(n):
        total = 0
        for i in range(n):       # correct: sum 0..n-1
            total += i
        return total

    def run_test():
        _ = json.dumps({"warmup": list(range(50))})
        _ = collections.Counter("the quick brown fox")
        assert compute(5) == 10   # 0+1+2+3+4 == 10  -> PASSES
    '''
)

BUGGY_SRC = textwrap.dedent(
    '''
    import json
    import collections

    def compute(n):
        total = 0
        for i in range(n + 1):   # off-by-one bug: sum 0..n
            total += i
        return total

    def run_test():
        _ = json.dumps({"warmup": list(range(50))})
        _ = collections.Counter("the quick brown fox")
        assert compute(5) == 10   # 0+1+2+3+4+5 == 15 != 10 -> FAILS
    '''
)


def _load_run_test(path: Path, src: str):
    path.write_text(src, encoding="utf-8")
    ns: dict = {"__file__": str(path), "__name__": "_traced_target"}
    code = compile(src, str(path), "exec")
    exec(code, ns)  # noqa: S102 - executing our own generated real code
    return ns["run_test"]


def test_scoped_trace_diverges_between_correct_and_buggy(tmp_path: Path) -> None:
    correct_path = tmp_path / "target_correct.py"
    buggy_path = tmp_path / "target_buggy.py"
    correct = _load_run_test(correct_path, CORRECT_SRC)
    buggy = _load_run_test(buggy_path, BUGGY_SRC)

    tr_ok = ScopedTracer(include_paths=[str(tmp_path)], max_events=5000)
    tr_bug = ScopedTracer(include_paths=[str(tmp_path)], max_events=5000)
    tr_ok.run(correct)
    tr_bug.run(buggy)
    s_ok = tr_ok.result.summary()
    s_bug = tr_bug.result.summary()

    # Both captured real in-project execution (not zero, not just imports).
    assert s_ok["events_recorded"] > 5, s_ok
    assert s_bug["events_recorded"] > 5, s_bug
    # The correct run passes (no uncaught exception); the buggy run raises.
    assert tr_ok.result.raised is None, s_ok
    assert tr_bug.result.raised == "AssertionError", s_bug
    # The buggy run records an exception event; the correct one does not.
    assert s_bug["exception_event_count"] >= 1, s_bug
    assert s_ok["exception_event_count"] == 0, s_ok
    # Divergent coverage: the buggy loop executes one extra iteration line-hit.
    assert s_ok["lines_executed"] != s_bug["lines_executed"] or s_ok["trace_event_types"] != s_bug["trace_event_types"]


def test_out_of_scope_target_yields_no_signal(tmp_path: Path) -> None:
    # Reproduces the deployed failure mode: the signal file is NOT in scope, so the
    # tracer records nothing useful and correct vs buggy are indistinguishable.
    correct = _load_run_test(tmp_path / "c.py", CORRECT_SRC)
    buggy = _load_run_test(tmp_path / "b.py", BUGGY_SRC)
    bogus_scope = str(tmp_path / "nonexistent_subdir")
    tr_ok = ScopedTracer(include_paths=[bogus_scope], max_events=5000)
    tr_bug = ScopedTracer(include_paths=[bogus_scope], max_events=5000)
    tr_ok.run(correct)
    tr_bug.run(buggy)
    s_ok = tr_ok.result.summary()
    s_bug = tr_bug.result.summary()
    # Nothing in scope -> no events recorded -> no discriminative signal.
    assert s_ok["events_recorded"] == 0 and s_bug["events_recorded"] == 0
    assert s_ok["trace_event_types"] == s_bug["trace_event_types"] == {}


def test_excludes_site_packages_even_when_under_include(tmp_path: Path) -> None:
    pkg = tmp_path / "site-packages"
    pkg.mkdir()
    tr = ScopedTracer(include_paths=[str(tmp_path)], max_events=100)
    assert tr._in_scope(str(tmp_path / "app.py")) is True
    assert tr._in_scope(str(pkg / "numpy" / "core.py")) is False  # excluded substring wins


def test_constructor_fails_closed_on_bad_inputs() -> None:
    with pytest.raises(ValueError, match="SCOPED_TRACER_NO_INCLUDE_PATHS"):
        ScopedTracer(include_paths=[], max_events=100)
    with pytest.raises(ValueError, match="SCOPED_TRACER_BAD_MAX_EVENTS"):
        ScopedTracer(include_paths=["/tmp"], max_events=0)


def test_budget_truncates_and_flags(tmp_path: Path) -> None:
    correct = _load_run_test(tmp_path / "t.py", CORRECT_SRC)
    tr = ScopedTracer(include_paths=[str(tmp_path)], max_events=3)
    tr.run(correct)
    s = tr.result.summary()
    assert s["truncated"] is True
    assert s["events_recorded"] == 3
    assert s["events_seen"] > 3  # saw more than it could record
