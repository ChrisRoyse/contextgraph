from __future__ import annotations

import json
from pathlib import Path

import pytest

from ..python_execution_trace_smoke import (
    ExecutionTraceSmokeError,
    TRACE_BEGIN,
    TRACE_END,
    build_trace_runner_source,
    extract_trace_payload,
    load_corpus_entry,
    transform_eval_script_list,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def test_trace_runner_source_avoids_future_annotations_for_python36_testbeds():
    source = build_trace_runner_source()

    assert "from __future__ import annotations" not in source
    compile(source, "contextgraph_trace_pytest.py", "exec")


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
