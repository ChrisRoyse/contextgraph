from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from ..python_delete_test_call_candidate_expansion_core import (
    detect_pre_oracle_pass_hypothesis,
    select_candidates,
)
from ..core import SwebenchOpsError
from ..python_delete_test_call_regeneration import (
    generate_source_extension_candidates,
    materialize_candidate,
)
from ..python_delete_test_call_oracle_executor import (
    filter_existing_outcome_batches,
    validate_existing_outcomes,
)


def write_source(tmp_path: Path, name: str, text: str) -> Path:
    path = tmp_path / name
    path.write_text(textwrap.dedent(text), encoding="utf-8")
    return path


def line_containing(path: Path, needle: str) -> int:
    for index, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if needle in line:
            return index
    raise AssertionError(f"line not found: {needle}")


def candidate_row(path: Path, candidate_id: str, selected_line: int) -> dict:
    return {
        "candidate_row_id": candidate_id,
        "source_task_id": "synthetic__task",
        "stable_partition": "holdout",
        "assertion_duty_sublabel": "other_assertion_like_deleted",
        "source_path": path.name,
        "source_file": str(path),
        "patch_path": f"/zfs/archive/contextgraph/synthetic/{candidate_id}.patch",
        "source_site_effective_offset": selected_line,
        "selected_statement": {"start_line": selected_line},
    }


def hypothesis_kinds(payload: dict) -> set[str]:
    return {item["kind"] for item in payload["hypotheses"]}


def source_extension_rows(tmp_path: Path, relative_path: str, text: str) -> tuple[Path, list[dict], dict]:
    source_root = tmp_path / "source"
    source = source_root / "synthetic__task" / "base" / relative_path
    source.parent.mkdir(parents=True)
    source.write_text(textwrap.dedent(text), encoding="utf-8")
    rows, scan = generate_source_extension_candidates(
        source_root=source_root,
        task_meta={"synthetic__task": {"stable_partition": "holdout", "repo": "synthetic/repo"}},
        existing_spans=set(),
        max_files=None,
        max_candidates=None,
    )
    return source_root, rows, scan


def test_detects_redundant_plain_and_unittest_assertion(tmp_path: Path):
    source = write_source(
        tmp_path,
        "test_redundant.py",
        """
        import unittest

        class TestThing(unittest.TestCase):
            def test_value(self):
                value = 1
                assert value == 1
                self.assertEqual(value, 1)
        """,
    )
    row = candidate_row(source, "sha256:redundant", line_containing(source, "assert value == 1"))

    payload = detect_pre_oracle_pass_hypothesis(row)

    assert payload["analysis_status"] == "evaluated"
    assert "redundant_assertion_same_scope" in hypothesis_kinds(payload)
    assert payload["target_oracle_labels_used_as_features"] is False


def test_detects_tautological_assertions(tmp_path: Path):
    source = write_source(
        tmp_path,
        "test_tautology.py",
        """
        def test_tautology():
            assert len([1, 2, 3]) >= 0
        """,
    )
    row = candidate_row(source, "sha256:tautology", line_containing(source, "assert len"))

    payload = detect_pre_oracle_pass_hypothesis(row)

    assert "tautological_assertion" in hypothesis_kinds(payload)
    assert payload["highest_confidence"] == 0.9


def test_require_pass_hypothesis_selects_only_structural_candidates(tmp_path: Path):
    load_bearing = write_source(
        tmp_path,
        "test_load_bearing.py",
        """
        def test_load_bearing():
            value = 1
            assert value == 1
        """,
    )
    tautology = write_source(
        tmp_path,
        "test_selected.py",
        """
        def test_selected():
            assert True
        """,
    )
    bad = candidate_row(load_bearing, "sha256:aaa-load-bearing", line_containing(load_bearing, "assert value"))
    good = candidate_row(tautology, "sha256:zzz-tautology", line_containing(tautology, "assert True"))
    good["source_task_id"] = "synthetic__task_late"

    selected, shortfalls, available = select_candidates(
        [bad, good],
        existing_ids=set(),
        targets={("holdout", "other_assertion_like_deleted"): 1},
        prefer_pass_hypothesis=True,
        require_pass_hypothesis=True,
    )

    assert shortfalls == {}
    assert available[("holdout", "other_assertion_like_deleted")] == 1
    assert [row["candidate_row_id"] for row in selected] == ["sha256:zzz-tautology"]
    assert hypothesis_kinds(selected[0]["pre_oracle_pass_hypothesis"]) == {"tautological_assertion"}


def test_source_extension_generates_materializable_tautology_candidate(tmp_path: Path):
    source_root, rows, scan = source_extension_rows(
        tmp_path,
        "tests/test_generated.py",
        """
        def test_identity():
            value = 1
            assert value == value
        """,
    )

    assert scan["parsed_files"] == 1
    assert len(rows) == 1
    row = rows[0]
    assert row["task_id"] == "TASK-PY-G-135I"
    assert row["stable_partition"] == "holdout"
    assert row["assertion_duty_sublabel"] == "other_assertion_like_deleted"
    assert row["target_oracle_labels_used_as_features"] is False
    assert row["pre_oracle_pass_hypothesis"]["primary_kind"] == "tautological_assertion"

    materialized = materialize_candidate(row, 0, source_root, tmp_path / "patches")

    assert materialized["oracle_status"] == "ready"
    assert Path(materialized["patch_path"]).is_file()
    assert materialized["selected_statement"]["text_preview"].strip() == "assert value == value"


def test_source_extension_generates_materializable_pytest_raises_call_body(tmp_path: Path):
    source_root, rows, scan = source_extension_rows(
        tmp_path,
        "tests/test_raises.py",
        """
        import pytest

        def test_bad_input():
            with pytest.raises(ValueError):
                int("not-an-int")
        """,
    )

    assert scan["parsed_files"] == 1
    assert len(rows) == 1
    row = rows[0]
    assert row["assertion_duty_sublabel"] == "pytest_raises_deleted"
    assert row["pre_oracle_pass_hypothesis"]["primary_kind"] == "pytest_raises_negative_path_deleted"

    materialized = materialize_candidate(row, 0, source_root, tmp_path / "patches")

    assert materialized["oracle_status"] == "ready"
    preview_first_line = materialized["selected_statement"]["text_preview"].splitlines()[0].strip()
    assert preview_first_line == "with pytest.raises(ValueError):"


def test_source_extension_generates_redundant_self_assert_candidate(tmp_path: Path):
    source_root, rows, scan = source_extension_rows(
        tmp_path,
        "tests/test_redundant_self_assert.py",
        """
        import unittest

        class TestThing(unittest.TestCase):
            def test_value(self):
                value = 1
                self.assertEqual(value, 1)
                self.assertEqual(value, 1)
        """,
    )

    assert scan["parsed_files"] == 1
    redundant_rows = [
        row
        for row in rows
        if row["assertion_duty_sublabel"] == "test_self_assert_deleted"
        and row["pre_oracle_pass_hypothesis"]["primary_kind"] == "redundant_assertion_same_scope"
    ]
    assert len(redundant_rows) == 2

    materialized = materialize_candidate(redundant_rows[0], 0, source_root, tmp_path / "patches")

    assert materialized["oracle_status"] == "ready"
    assert materialized["selected_statement"]["text_preview"].strip() == "self.assertEqual(value, 1)"


def test_source_extension_generates_guarded_production_assert_candidate(tmp_path: Path):
    source_root, rows, scan = source_extension_rows(
        tmp_path,
        "pkg/module.py",
        """
        def parse(value):
            if not isinstance(value, str):
                raise TypeError("value")
            assert isinstance(value, str)
            return value
        """,
    )

    assert scan["parsed_files"] == 1
    assert len(rows) == 1
    row = rows[0]
    assert row["assertion_duty_sublabel"] == "production_plain_assert_deleted"
    assert row["pre_oracle_pass_hypothesis"]["primary_kind"] == "production_assert_redundant_guarded_invariant"

    materialized = materialize_candidate(row, 0, source_root, tmp_path / "patches")

    assert materialized["oracle_status"] == "ready"
    assert materialized["selected_statement"]["text_preview"].strip() == "assert isinstance(value, str)"


def test_oracle_resume_filters_completed_and_trims_partial_batches(tmp_path: Path):
    candidates = {
        "sha256:c1": {"source_task_id": "task__one", "patch_path": str(tmp_path / "one.patch")},
        "sha256:c2": {"source_task_id": "task__two", "patch_path": str(tmp_path / "two.patch")},
        "sha256:c3": {"source_task_id": "task__three", "patch_path": str(tmp_path / "three.patch")},
    }
    selected_batches = [
        {
            "batch_index": 0,
            "batch_key": "batch-0000",
            "candidate_row_ids": ["sha256:c1", "sha256:c2"],
            "source_task_ids": ["task__one", "task__two"],
            "patch_paths": [candidates["sha256:c1"]["patch_path"], candidates["sha256:c2"]["patch_path"]],
            "prediction_count": 2,
        },
        {
            "batch_index": 1,
            "batch_key": "batch-0001",
            "candidate_row_ids": ["sha256:c3"],
            "source_task_ids": ["task__three"],
            "patch_paths": [candidates["sha256:c3"]["patch_path"]],
            "prediction_count": 1,
        },
    ]

    pending, skipped = filter_existing_outcome_batches(
        selected_batches,
        existing_outcome_ids={"sha256:c1", "sha256:c3"},
        candidates=candidates,
    )

    assert skipped == 1
    assert len(pending) == 1
    assert pending[0]["batch_key"] == "batch-0000"
    assert pending[0]["candidate_row_ids"] == ["sha256:c2"]
    assert pending[0]["source_task_ids"] == ["task__two"]
    assert pending[0]["patch_paths"] == [candidates["sha256:c2"]["patch_path"]]
    assert pending[0]["prediction_count"] == 1


def test_oracle_resume_rejects_duplicate_existing_outcome_ids(tmp_path: Path):
    with pytest.raises(SwebenchOpsError) as exc_info:
        validate_existing_outcomes(
            [{"candidate_row_id": "sha256:dup"}, {"candidate_row_id": "sha256:dup"}],
            tmp_path / "delete_test_call_oracle_outcomes.jsonl",
        )

    assert exc_info.value.code == "DELETE_TEST_CALL_ORACLE_EXISTING_OUTCOME_DUPLICATE_ID"
