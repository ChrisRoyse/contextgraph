# 14 — Test Plan

This document is the comprehensive test-coverage matrix for v2.0. It
follows the existing project's three-tier discipline: unit tests
(per-crate), integration tests (real CLI binary, real RocksDB), and
local CI gate (release-D successor of release-C).

## 1. Test pyramid summary

| Tier | Where | Count target | Wall time |
| --- | --- | ---: | ---: |
| Unit | per-crate `#[test]` and `#[cfg(test)] mod tests` | $+$ ~150 new | <30 s total |
| Integration | `crates/context-graph-cli/tests/dynamicjepa_phase{N}_fsv.rs` | $+$ 4 new files | ~60 s combined |
| FSV smoke (Tier B) | `local_ci_gate.sh smoke-b` | extends existing | 250 s (v1 was 191 s) |
| FSV release (Tier D) | `local_ci_gate.sh release-d` | extends existing | $\sim$2 hours |

Existing v1 tests (5{,}184 in the codebase as of v1) remain green;
v2 adds ~150 unit and 4 integration files.

## 2. Unit-test coverage per new component

### 2.1 `dj_pairwise_readings` storage

`crates/context-graph-storage/src/dynamicjepa/pairwise_store.rs` (new
file) — tests:

- `test_pairwise_key_format_24_bytes_fixed`
- `test_pairwise_prefix_extractor_returns_event_uuid`
- `test_pairwise_write_then_read_roundtrip`
- `test_pairwise_batch_atomicity` — verify all $\binom{N}{2}$ rows
  for an event are written together or none.
- `test_pairwise_count_matches_combinatorial`
- `test_pairwise_asymmetric_ordering_rejected_at_write`
- `test_pairwise_kinds_emitted_bitset_round_trip`
- `test_pairwise_per_event_prefix_scan`
- `test_pairwise_inspection_returns_decoded_records`
- `test_pairwise_lookup_by_full_key`

### 2.2 `dj_constellations` storage

`crates/context-graph-storage/src/dynamicjepa/constellation_store.rs`
(new) — tests:

- `test_constellation_key_format_36_bytes_fixed`
- `test_constellation_subject_id_hash_stable`
- `test_constellation_centroid_round_trip_json`
- `test_constellation_l2_norm_invariant`
- `test_constellation_loo_recompute_within_tolerance`
- `test_constellation_zero_norm_majority_rejected`
- `test_constellation_reference_set_too_small_rejected`
- `test_constellation_instrument_hash_match_required`
- `test_constellation_calibration_percentile_default_is_10`
- `test_constellation_subject_field_undeclared_rejected`
- `test_constellation_subject_value_undeclared_rejected`

### 2.3 `dj_threshold_calibrations` storage

`crates/context-graph-storage/src/dynamicjepa/threshold_store.rs`
(new) — tests:

- `test_threshold_key_format_40_bytes_fixed`
- `test_threshold_supersede_seq_increments`
- `test_threshold_calibration_set_disjoint_required`
- `test_threshold_recalibrate_requires_supersede`
- `test_threshold_history_preserved_on_supersede`
- `test_threshold_current_row_is_max_supersede_seq`
- `test_threshold_percentile_within_calibration_set`
- `test_threshold_recalibrate_reason_required`

### 2.4 Audit-log signal-yield

`crates/context-graph-storage/src/dynamicjepa/audit.rs` (extended) —
tests:

- `test_signal_yield_for_each_known_operation_kind`
- `test_signal_yield_unknown_operation_kind_returns_zero`
- `test_signal_yield_lookup_table_matches_doc_03`
- `test_audit_record_v1_deserialises_with_default_yield_zero`
- `test_audit_record_v2_yield_round_trip`

### 2.5 Combined guard predicate

`crates/context-graph-dynamicjepa/src/guard.rs` (new) — tests:

- `test_gtau_predicate_all_pass_returns_accept`
- `test_gtau_predicate_one_fail_returns_reject_with_modality`
- `test_combined_verdict_utility_only_when_no_constellation`
- `test_combined_verdict_both_required_when_constellation_present`
- `test_combined_verdict_records_failed_modalities_list`
- `test_combined_verdict_no_accepted_candidate_flag`
- `test_combined_verdict_selected_action_is_max_utility_among_accepts`
- `test_constellation_selection_matches_subject_field_value`
- `test_constellation_selection_falls_back_to_global`
- `test_combined_verdict_audit_record_signal_yield_correct`

### 2.6 KSG estimator

`crates/context-graph-dynamicjepa/src/mi_audit/ksg.rs` (new) — tests:

- `test_ksg_two_independent_gaussians_returns_near_zero`
- `test_ksg_perfectly_correlated_returns_high_mi`
- `test_ksg_one_hot_pair_returns_predicted_value`
- `test_ksg_k_5_default`
- `test_ksg_bootstrap_seed_deterministic`
- `test_ksg_sample_size_too_small_rejected`
- `test_ksg_degenerate_input_rejected`

### 2.7 MC-ratio aggregation

`crates/context-graph-cli/src/commands/mc_ratio.rs` (new) — tests:

- `test_mc_ratio_total_signals_equals_audit_log_sum`
- `test_mc_ratio_per_stage_breakdown_correct`
- `test_mc_ratio_known_count_matches_constructive_for_career`
- `test_mc_ratio_v1_bundle_returns_v1_baseline`
- `test_mc_ratio_signal_yield_drift_detected`

## 3. Integration-test files (new)

### 3.1 `tests/dynamicjepa_phase9_fsv.rs`

Mirrors existing `dynamicjepa_phaseN_fsv.rs` files. Tests:

- `dynamicjepa_phase9_pairwise_readings_integrity_passes`
- `dynamicjepa_phase9_pairwise_count_mismatch_fails_typed`
- `dynamicjepa_phase9_pairwise_cosine_drift_fails_typed`
- `dynamicjepa_phase9_pairwise_instrument_hash_drift_fails_typed`
- `dynamicjepa_phase9_pairwise_asymmetric_ordering_fails_typed`

Uses real CLI binary, real RocksDB, real O\*NET ZIP; fixtures
handcrafted to inject defects.

### 3.2 `tests/dynamicjepa_phase10_fsv.rs`

Mirrors above. Tests:

- `dynamicjepa_phase10_constellation_integrity_passes`
- `dynamicjepa_phase10_reference_set_drift_fails_typed`
- `dynamicjepa_phase10_instrument_hash_mismatch_fails_typed`
- `dynamicjepa_phase10_reference_set_too_small_fails_typed`
- `dynamicjepa_phase10_zero_norm_majority_fails_typed`

### 3.3 `tests/dynamicjepa_phase11_fsv.rs`

- `dynamicjepa_phase11_gtau_verifiability_passes`
- `dynamicjepa_phase11_threshold_drift_fails_typed`
- `dynamicjepa_phase11_cosine_drift_fails_typed`
- `dynamicjepa_phase11_verdict_inconsistent_fails_typed`
- `dynamicjepa_phase11_constellation_missing_fails_typed`

### 3.4 `tests/dynamicjepa_phase12_fsv.rs`

- `dynamicjepa_phase12_signal_yield_audit_passes`
- `dynamicjepa_phase12_signal_yield_table_drift_fails_typed`
- `dynamicjepa_phase12_unknown_operation_fails_typed`
- `dynamicjepa_phase12_event_count_mismatch_fails_typed`

### 3.5 Updated `dynamicjepa_research_smoke_fsv.rs`

The existing integration test gains:

- `research_smoke_v2_runs_all_12_fsv_harnesses`
- `research_smoke_v2_career_bundle_has_45000_pairwise_rows`
- `research_smoke_v2_career_bundle_has_50_constellation_rows`
- `research_smoke_v2_career_bundle_has_50_threshold_rows`
- `research_smoke_v2_career_bundle_mc_ratio_in_expected_range`
- `research_smoke_v2_pairwise_mi_audit_runs_during_release_d_only`
  (optional, gated by env var to skip in Tier A/B)

Total integration test wall time grows from 191 s (v1) to ~250 s (v2).

## 4. Backwards-compatibility tests

`tests/v2_v1_isolation_fsv.rs` (new) — tests:

- `v2_binary_refuses_v1_run_manifest_typed_error`
- `v2_binary_refuses_v1_aggregate_manifest_typed_error`
- `v1_binary_refuses_v2_bundle_typed_error_on_open`
- `v2_binary_creates_v2_bundle_with_default_schema_version_2`
- `v2_aggregator_refuses_mixed_v1_v2_bundles`

## 5. Local CI gate updates

### 5.1 Tier A (`smoke-a`) — unchanged

Tier A continues to test v1 baseline with v1 binary against v1
bundles. This preserves the v1 reference for comparability.

### 5.2 Tier B (`smoke-b`) — extended

```text
existing tier B steps unchanged
+ run dynamicjepa_phase{9,10,11,12}_fsv tests
+ run research_smoke_v2 tests
+ verify all new v2 paper tables emitted
+ verify FSV evidence files present
```

Wall time: ~5 minutes (was 4 minutes).

### 5.3 Tier D (`release-d`) — new

Successor to release-C, extends with:

```text
existing release-C steps:
  5 paper-small seeds 42-46
  multiseed-aggregate
  release reference check
  deterministic Docker artifact pair
  container verify

NEW v2.0 steps:
  + audit-pairwise-mi --domain career_taxonomy --sample-size 1000
  + check pairwise MI verdict against reference
  + compute-mc-ratio per-domain
  + check MC ratio against reference (within tolerance)
  + verify all 4 new paper tables present in aggregate
  + container verify uses v2 SIF + Docker tar
  + new manual edge audit batch (FSV-9..12 edges)
```

Wall time: ~3 hours (was 2 hours; adds ~1 hour for the MI audit
+ extra FSV checks).

## 6. Determinism tests

The v1 paper documents three determinism caveats: GPU non-determinism
(PyTorch / Candle), kernel ordering, and floating-point variance in
mixed precision. v2 inherits these. Additional v2 determinism tests:

- `test_pairwise_cosine_deterministic_across_reruns_within_tolerance`
  ($\le 10^{-4}$)
- `test_constellation_centroid_deterministic_across_reruns_within_tolerance`
  ($\le 10^{-5}$)
- `test_threshold_calibration_deterministic_when_calibration_set_fixed`
- `test_signal_yield_deterministic_for_known_operation_count`
  (bit-equal)
- `test_ksg_estimate_deterministic_with_fixed_seed`
  (bit-equal across re-runs on same host)

The cross-host portability story is the same as v1: structural and
tolerance-based, not byte-equal.

## 7. Test fixture additions

New TOML fixtures under `configs/dynamicjepa/v2/`:

- `domain_packs/counter_world.v2.toml`
- `domain_packs/gridworld_5x5.v2.toml`
- `domain_packs/career_taxonomy.v2.toml` (with constellation
  declarations + pair\_kinds)
- `verification/career_taxonomy_constellation_global.json` (expected
  centroid head + tail for FSV-10 reference)
- `verification/career_taxonomy_threshold_calibration.json` (expected
  $\tau$ per modality)
- `verification/career_taxonomy_pairwise_mi_expected.json` (expected
  $n_{\text{eff}}$ per pair-kind)

## 8. Mutation testing (optional)

For Phase 5's KSG estimator and the constellation construction, run
a one-shot mutation test (`cargo mutants`) to verify test coverage
catches obvious bugs. Mutation-test target: ≥ 90 % kill rate on
arithmetic operators in the centroid math and KSG estimator.

This is not a release gate; it is a one-time confidence-builder
during Phase 3 / Phase 5 implementation.

## 9. Stress / performance tests

Two performance regressions guarded by integration tests:

- `test_pairwise_materialisation_under_50ms_per_event` — verifies
  pairwise computation stays within budget on the documented host.
- `test_audit_pairwise_mi_under_120s_for_1000_events` — verifies
  KSG audit fits release-D wall-time budget.

## 10. Coverage targets

| Area | Coverage target |
| --- | ---: |
| `pairwise_store.rs` | ≥ 90 % |
| `constellation_store.rs` | ≥ 90 % |
| `threshold_store.rs` | ≥ 90 % |
| `guard.rs` (combined verdict) | ≥ 90 % |
| `mi_audit/ksg.rs` | ≥ 85 % |
| `mc_ratio.rs` | ≥ 90 % |
| Existing v1 code | unchanged (no regression) |

Existing v1 coverage levels are preserved; the upgrade does not
remove or weaken any existing test.
