from __future__ import annotations

from ..python_low_signal_embedder_audit import (
    build_priority_rows,
    fixture_auc_manifest,
    fixture_attribution_manifest,
    fixture_drop_ablation_manifest,
    fixture_mi_manifest,
    fixture_phase3_summary,
    fixture_preflight_manifest,
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


def build_final_rows() -> list[dict]:
    return build_priority_rows(
        mi_manifest=fixture_mi_manifest(),
        auc_manifest=fixture_auc_manifest(),
        preflight_manifest=fixture_preflight_manifest(),
        attribution_manifest=fixture_attribution_manifest(),
        phase3_summary=fixture_phase3_summary(),
        drop_ablation_manifest=fixture_drop_ablation_manifest(),
        final_ranking=True,
    )


def test_final_ranking_integrates_direct_drop_ablation_and_attribution():
    rows = build_final_rows()
    by_slot = rows_by_slot(rows)

    assert by_slot["E9"]["measurements"]["holdout_corr_delta_when_dropped"] < 0.0
    assert by_slot["E9"]["measurements"]["per_cell_median_attribution"] > 0.005
    assert by_slot["E9"]["decision"] == "keep_by_default_conflicting_signal_detected"
    assert by_slot["E9"]["replacement_queue_candidate"] is False

    assert by_slot["E14"]["measurements"]["holdout_corr_delta_when_dropped"] < 0.0
    assert by_slot["E14"]["measurements"]["per_cell_median_attribution"] < 0.005
    assert by_slot["E14"]["decision"] == "active_low_signal_candidate_pending_downstream_gates"
    assert by_slot["E14"]["replacement_queue_candidate"] is True
    assert by_slot["E14"]["replaceable"] is False


def test_final_ranking_keeps_temporal_slots_by_default_without_live_source():
    rows = build_final_rows()
    by_slot = rows_by_slot(rows)

    for slot in ("E2", "E3", "E4"):
        assert by_slot[slot]["decision"] == "keep_by_default"
        assert by_slot[slot]["replaceable"] is False
        assert by_slot[slot]["authoritative_for_keep_default"] is True
        assert "no_post_683_live_raw_temporal_vector_source" in by_slot[slot]["blockers"]


def test_final_validation_cases_capture_integrated_contract():
    cases = validate_priority_rows(build_final_rows())

    assert cases
    assert all(case["pass"] is True for case in cases)
