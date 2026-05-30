from __future__ import annotations

import json
from argparse import Namespace
from pathlib import Path

import pytest

from ..python_execution_trace_plan import (
    ExecutionTracePlanError,
    classify_eval_script,
    command_family,
    run_plan,
)


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_corpus(root: Path, rows: list[dict[str, str]]) -> None:
    entries = []
    for row in rows:
        entries.append(
            {
                "task_id": row["task_id"],
                "repo": row["repo"],
                "category": row["category"],
                "patch_path": f"mutations/{row['task_id']}/{row['category']}.patch",
                "oracle_all_passed": True,
                "oracle_verdict_sha256": "sha256:" + ("a" * 64),
            }
        )
    write_json(root / "index.json", {"schema_version": 1, "entries": entries})


def script_with_command(command: str) -> list[str]:
    return [
        "source /opt/miniconda3/bin/activate",
        "conda activate testbed",
        "cd /testbed",
        ": 'START TEST OUTPUT'",
        command,
        ": 'END TEST OUTPUT'",
        "git checkout base tests.py",
    ]


def test_command_family_direct_pytest_is_currently_supported():
    result = command_family("pytest -rA tests/test_widget.py")

    assert result["family"] == "pytest_direct"
    assert result["support_status"] == "supported_current"
    assert result["handler"] == "pytest_sys_settrace_v1"
    assert result["blocker_code"] is None


def test_classify_django_runtests_gets_typed_handler_blocker():
    result = classify_eval_script(script_with_command("./tests/runtests.py migrations.test_commands"))

    assert result["command_family"] == "django_runtests"
    assert result["support_status"] == "handler_required"
    assert result["blocker_code"] == "EXECUTION_TRACE_PLAN_UNSUPPORTED_DJANGO_RUNTESTS"


def test_classify_python_module_pytest_is_not_claimed_supported_yet():
    result = classify_eval_script(script_with_command("python -m pytest -rA tests/test_widget.py"))

    assert result["command_family"] == "python_module_pytest"
    assert result["support_status"] == "handler_required"
    assert result["blocker_code"] == "EXECUTION_TRACE_PLAN_UNSUPPORTED_PYTHON_MODULE_PYTEST"


def test_classify_multiple_test_commands_fails_closed():
    result = classify_eval_script(
        [
            ": 'START TEST OUTPUT'",
            "pytest tests/a.py",
            "pytest tests/b.py",
            ": 'END TEST OUTPUT'",
        ]
    )

    assert result["command_family"] == "multiple_test_commands"
    assert result["support_status"] == "unsupported"
    assert result["blocker_code"] == "EXECUTION_TRACE_PLAN_MULTIPLE_TEST_COMMANDS"


def test_git_apply_heredoc_with_pytest_text_is_setup_not_test_command():
    result = classify_eval_script(
        [
            ": 'START TEST OUTPUT'",
            "git apply -v - <<'EOF_123'\n+import pytest\nEOF_123",
            "pytest -rA lib/matplotlib/tests/test_artist.py",
            ": 'END TEST OUTPUT'",
        ]
    )

    assert result["command_family"] == "pytest_direct"
    assert result["support_status"] == "supported_current"
    assert result["test_commands"] == ["pytest -rA lib/matplotlib/tests/test_artist.py"]


def test_run_plan_with_fixture_writes_fsv_and_state_mirrors(tmp_path: Path):
    corpus_root = tmp_path / "corpus"
    rows = [
        {"task_id": "org__repo-1", "repo": "org/repo", "category": "known_good"},
        {"task_id": "org__repo-1", "repo": "org/repo", "category": "off_by_one"},
        {"task_id": "django__django-1", "repo": "django/django", "category": "known_good"},
        {"task_id": "django__django-1", "repo": "django/django", "category": "subtle_flip"},
    ]
    write_corpus(corpus_root, rows)
    fixture = tmp_path / "spec-fixture.json"
    write_json(
        fixture,
        {
            "tasks": {
                "org__repo-1": script_with_command("pytest -rA tests/test_widget.py"),
                "django__django-1": script_with_command("./tests/runtests.py migrations.test_commands"),
            }
        },
    )

    result = run_plan(
        Namespace(
            dataset_name="fixture",
            split="test",
            corpus_root=corpus_root,
            output_root=tmp_path / "fsv",
            state_root=tmp_path / "state",
            run_id="run-test",
            expected_corpus_rows=4,
            namespace="swebench",
            arch="x86_64",
            tag="latest",
            spec_fixture=fixture,
            allow_non_aiwonder_roots=True,
        )
    )

    manifest = json.loads(Path(result["manifest"]).read_text(encoding="utf-8"))
    state_manifest = json.loads(
        (tmp_path / "state" / "run-test" / "trace-plan" / "execution_trace_command_plan_manifest.json")
        .read_text(encoding="utf-8")
    )
    row_plan = [
        json.loads(line)
        for line in Path(result["row_plan"]).read_text(encoding="utf-8").splitlines()
    ]
    unsupported = [
        json.loads(line)
        for line in Path(result["unsupported_command_rows"]).read_text(encoding="utf-8").splitlines()
    ]

    assert manifest == state_manifest
    assert manifest["summary"]["task_command_family_counts"] == {
        "django_runtests": 1,
        "pytest_direct": 1,
    }
    assert manifest["summary"]["row_support_status_counts"] == {
        "handler_required": 2,
        "supported_current": 2,
    }
    assert manifest["summary"]["current_supported_trace_fraction"] == 0.5
    assert len(row_plan) == 4
    assert len(unsupported) == 2
    assert all("oracle_all_passed" not in item for item in row_plan)
    assert "typed_trace_handlers_required" in manifest["decision"]["decision_reasons"]
    assert manifest["doctrine_policy"]["target_labels_used_as_live_inputs"] is False


def test_non_aiwonder_roots_fail_closed_without_local_override(tmp_path: Path):
    corpus_root = tmp_path / "corpus"
    write_corpus(
        corpus_root,
        [{"task_id": "org__repo-1", "repo": "org/repo", "category": "known_good"}],
    )
    fixture = tmp_path / "spec-fixture.json"
    write_json(fixture, {"tasks": {"org__repo-1": script_with_command("pytest tests/test_widget.py")}})

    with pytest.raises(ExecutionTracePlanError, match="EXECUTION_TRACE_PLAN_NON_AIWONDER_DURABLE_PATH"):
        run_plan(
            Namespace(
                dataset_name="fixture",
                split="test",
                corpus_root=corpus_root,
                output_root=tmp_path / "fsv",
                state_root=tmp_path / "state",
                run_id="run-test",
                expected_corpus_rows=1,
                namespace="swebench",
                arch="x86_64",
                tag="latest",
                spec_fixture=fixture,
                allow_non_aiwonder_roots=False,
            )
        )
