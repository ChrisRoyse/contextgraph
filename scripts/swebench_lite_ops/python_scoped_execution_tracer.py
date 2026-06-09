#!/usr/bin/env python3
"""Scope-filtered execution tracer for INV-010 (#774) — root-cause fix.

PROBLEM (measured, run-20260609T-trace-feature-mi): the existing INV-010 traces
carry only ~0.036 bits about the Docker oracle — no better than the static panel
— because `sys.settrace` records from process start, exhausts its event budget on
interpreter/framework IMPORT boilerplate (e.g. thousands of `django/__init__.py`
module-load lines), and never reaches the test execution where pass/fail diverges.
Forensic evidence: 2340/2459 traces truncated; recorded events are all
import/module-load events; events_seen ~2.79M but only the first ~5000 recorded.

FIX (convergent best practice across TRACED ICSE'24, the E.T. paper arXiv:2503.05703,
python-hunter `stdlib=False`, bottrace `--exclude site-packages`, marin pytracer,
PyTraceAutopsy `include_stdlib:false`): SCOPE the tracer to the project's own source
files. Only record events whose filename is under an include path AND not under an
excluded path (stdlib / site-packages / frozen importlib). Out-of-scope frames are
skipped entirely (return None local tracer), so the event budget is spent on the
in-scope test execution — where the oracle-discriminative signal lives.

This module is import-safe and dependency-free (stdlib only). It is the trace
PRODUCER; feature extraction / MI measurement live in the measurement harnesses.
"""
from __future__ import annotations

import os
import sys
from dataclasses import dataclass, field
from typing import Any, Callable, Optional

# Default exclusions: anything not in the project is noise for correctness signal.
DEFAULT_EXCLUDE_SUBSTRINGS = (
    "site-packages",
    "dist-packages",
    "<frozen",
    "<string>",
    "<built-in>",
    "importlib",
)


def _normalize(path: str) -> str:
    try:
        return os.path.realpath(path)
    except (OSError, ValueError):
        return path


@dataclass
class ScopedTraceResult:
    """The structured trace, scoped to in-project execution."""

    events_recorded: int = 0
    events_seen: int = 0  # in-scope events observed before any budget cap
    truncated: bool = False
    files_touched: set[str] = field(default_factory=set)
    lines_executed: set[str] = field(default_factory=set)  # "file:line"
    event_type_counts: dict[str, int] = field(default_factory=dict)
    exception_types: list[str] = field(default_factory=list)
    functions_entered: list[str] = field(default_factory=list)
    raised: Optional[str] = None  # uncaught exception type from the traced call

    def summary(self) -> dict[str, Any]:
        et = self.event_type_counts
        files = len(self.files_touched)
        lines = len(self.lines_executed)
        return {
            "events_recorded": self.events_recorded,
            "events_seen": self.events_seen,
            "truncated": self.truncated,
            "files_touched": files,
            "lines_executed": lines,
            "trace_event_types": dict(sorted(et.items())),
            "exception_event_count": int(et.get("exception", 0)),
            "distinct_exception_types": sorted(set(self.exception_types)),
            "line_coverage_density": (lines / files) if files else 0.0,
            "uncaught_exception_type": self.raised,
            "in_scope_only": True,
        }


class ScopedTracer:
    """sys.settrace tracer that records ONLY in-scope (project) frames.

    include_paths: directory/file prefixes whose code should be traced.
    exclude_substrings: substrings that force a frame out of scope even if it
        matches an include path (stdlib/site-packages/frozen importlib).
    max_events: budget; spent on IN-SCOPE events only (the whole point).
    """

    def __init__(
        self,
        include_paths: list[str],
        *,
        exclude_substrings: tuple[str, ...] = DEFAULT_EXCLUDE_SUBSTRINGS,
        max_events: int = 5000,
        capture_call_only: bool = False,
    ) -> None:
        if not include_paths:
            raise ValueError("SCOPED_TRACER_NO_INCLUDE_PATHS: at least one include path is required")
        if max_events <= 0:
            raise ValueError(f"SCOPED_TRACER_BAD_MAX_EVENTS: {max_events}")
        self._include = tuple(_normalize(p) for p in include_paths)
        self._exclude = tuple(exclude_substrings)
        self._max_events = int(max_events)
        self._capture_call_only = capture_call_only
        self.result = ScopedTraceResult()
        self._scope_cache: dict[str, bool] = {}

    def _in_scope(self, filename: Optional[str]) -> bool:
        if not filename:
            return False
        cached = self._scope_cache.get(filename)
        if cached is not None:
            return cached
        norm = _normalize(filename)
        verdict = False
        if not any(sub in norm for sub in self._exclude):
            verdict = any(norm.startswith(inc) for inc in self._include)
        self._scope_cache[filename] = verdict
        return verdict

    def _record(self, frame: Any, event: str) -> None:
        res = self.result
        res.events_seen += 1
        if res.events_recorded >= self._max_events:
            res.truncated = True
            return
        res.events_recorded += 1
        res.event_type_counts[event] = res.event_type_counts.get(event, 0) + 1
        code = frame.f_code
        filename = code.co_filename
        res.files_touched.add(_normalize(filename))
        res.lines_executed.add(f"{_normalize(filename)}:{frame.f_lineno}")
        if event == "call":
            res.functions_entered.append(code.co_name)

    def _dispatch(self, frame: Any, event: str, arg: Any):
        # Local trace fn: only called for frames we opted into (in-scope).
        if event == "exception" and arg is not None:
            exc_type = getattr(arg[0], "__name__", str(arg[0]))
            self.result.exception_types.append(exc_type)
        if self._capture_call_only and event == "line":
            return self._dispatch
        self._record(frame, event)
        return self._dispatch

    def _global(self, frame: Any, event: str, arg: Any):
        # Global trace fn: decide scope at 'call'. Return local tracer only for
        # in-scope frames -> out-of-scope frames (imports, stdlib) are skipped
        # entirely, so the budget is never burned on boilerplate.
        if event != "call":
            return None
        if not self._in_scope(frame.f_code.co_filename):
            return None
        self._record(frame, "call")
        return self._dispatch

    def run(self, fn: Callable[[], Any]) -> ScopedTraceResult:
        """Trace a zero-arg callable. Captures an uncaught exception type without
        suppressing it from the result (re-raises are surfaced as `raised`)."""
        prev = sys.gettrace()
        sys.settrace(self._global)
        try:
            fn()
        except BaseException as exc:  # record, do not hide — fail-visible
            self.result.raised = type(exc).__name__
        finally:
            sys.settrace(prev)
        return self.result
