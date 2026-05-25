from __future__ import annotations

import textwrap
from pathlib import Path

from ..python_delete_test_call_candidate_expansion_core import (
    detect_pre_oracle_pass_hypothesis,
    select_candidates,
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
