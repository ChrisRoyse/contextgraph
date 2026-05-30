from __future__ import annotations

from ..python_low_signal_embedder_audit import (
    build_priority_rows,
    fixture_auc_manifest,
    fixture_mi_manifest,
    validate_priority_rows,
)


def rows_by_slot(rows: list[dict]) -> dict[str, dict]:
    return {row["slot"]: row for row in rows}


def test_empty_slots_are_authoritative_replacement_targets():
    rows = build_priority_rows(mi_manifest=fixture_mi_manifest(), auc_manifest=fixture_auc_manifest())
    by_slot = rows_by_slot(rows)

    assert by_slot["E5"]["replaceable"] is True
    assert by_slot["E5"]["authoritative_for_replacement"] is True
    assert by_slot["E11"]["replaceable"] is True
    assert by_slot["E11"]["authoritative_for_replacement"] is True


def test_active_static_slots_are_not_replaceable_from_proxy_evidence():
    rows = build_priority_rows(mi_manifest=fixture_mi_manifest(), auc_manifest=fixture_auc_manifest())
    by_slot = rows_by_slot(rows)

    assert by_slot["E9"]["decision"] == "highest_priority_active_slot_for_full_audit"
    assert by_slot["E9"]["replaceable"] is False
    assert by_slot["E9"]["measurements"]["mi_delta_from_global_bits_proxy"] < 0.0
    assert by_slot["E14"]["replaceable"] is False
    assert by_slot["E14"]["measurements"]["test_bugfix_transition_auc_proxy"] < 0.6


def test_temporal_slots_remain_blocked_until_live_variance_exists():
    rows = build_priority_rows(mi_manifest=fixture_mi_manifest(), auc_manifest=fixture_auc_manifest())
    by_slot = rows_by_slot(rows)

    for slot in ("E2", "E3", "E4"):
        assert by_slot[slot]["replaceable"] is False
        assert by_slot[slot]["decision"] == "blocked_keep_temporal_until_live_variance_and_drop_ablation"
        assert "phase_3_live_temporal_variance_required" in by_slot[slot]["blockers"]


def test_validation_cases_capture_conservative_contract():
    rows = build_priority_rows(mi_manifest=fixture_mi_manifest(), auc_manifest=fixture_auc_manifest())
    cases = validate_priority_rows(rows)

    assert cases
    assert all(case["pass"] is True for case in cases)
