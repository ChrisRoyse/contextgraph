# 25 — FSV Harnesses Implementation Plan

This document is the file-level diff plan for the four new FSV
harnesses (PRs 10, 11, 13 of `20_implementation_plan_overview.md`).

## 1. The shared FSV crate

The new harnesses live alongside the existing FSV-1..8 in the
runtime's verifier module. Two files:

- `crates/context-graph-dynamicjepa/src/fsv/mod.rs` (existing,
  extended)
- `crates/context-graph-dynamicjepa/src/fsv/harnesses/{phase9,phase10,phase11,phase12}.rs`
  (4 NEW files)

Each new harness exposes a single public function:

```rust
pub async fn run_fsv_phaseN(
    store: &RocksDbTeleologicalStore,
    domain_pack: &DomainPack,
    bundle_root: &Path,
    fsv_evidence_dir: &Path,
) -> Result<FsvPhaseResult, FsvError>;
```

The result type carries pre/post counts, check-by-check counts, edge
case results, and a final `passed: bool`. Same shape as the
existing FSV-1..8 results.

## 2. FSV-9: Pairwise readings integrity (PR 10)

`crates/context-graph-dynamicjepa/src/fsv/harnesses/phase9.rs` (NEW)

Algorithm pseudocode (mirrors `09_fsv_extensions.md` §1):

```text
fn run_fsv_phase9(store, domain_pack, bundle_root, fsv_evidence_dir):
  # Pre-state
  pre_pairwise_count = inspect-counts.dj_pairwise_readings
  expected_count = n_events * C(N_d, 2)

  # Check 1: row-count per event
  pair_count_per_event_failures = []
  for event_id in normalized_states:
    rows = list_pairwise_readings_for_event(store, event_id)
    if len(rows) != C(N_d, 2):
      pair_count_per_event_failures.append((event_id, len(rows)))

  # Check 2: cosine recompute within tolerance (1e-4)
  cosine_drift_failures = []
  for pair in iter_pairwise_readings(store):
    j_reading = get_instrument_reading(pair.event_id, pair.instrument_j)
    k_reading = get_instrument_reading(pair.event_id, pair.instrument_k)
    recomputed = cosine_similarity(j_reading.vector, k_reading.vector)
    if abs(pair.cosine_agreement - recomputed) > 1e-4:
      cosine_drift_failures.append(pair.pairwise_id)

  # Check 3: instrument-hash match
  hash_drift_failures = []
  for pair in iter_pairwise_readings(store):
    j_artifact = registry.get(pair.instrument_j).artifact_hash
    k_artifact = registry.get(pair.instrument_k).artifact_hash
    if j_artifact != pair.instrument_j_artifact_hash or
       k_artifact != pair.instrument_k_artifact_hash:
      hash_drift_failures.append(pair.pairwise_id)

  # Check 4: kinds_emitted bitset consistent with TOML declaration
  bitset_inconsistencies = []
  for pair in iter_pairwise_readings(store):
    j_inst = domain_pack.declared_instruments[pair.instrument_j]
    k_inst = domain_pack.declared_instruments[pair.instrument_k]
    expected_kinds = set(j_inst.pair_kinds) & set(k_inst.pair_kinds)

    actual_kinds = pair.kinds_emitted.iter_kinds()

    for kind in expected_kinds:
      if kind not in actual_kinds:
        # Both declare it, but pair didn't emit it; fail
        bitset_inconsistencies.append((pair.pairwise_id, kind))
    for kind in actual_kinds:
      if kind not in expected_kinds and kind != "cosine_agreement":
        # Pair emitted it but only one declared it; fail
        bitset_inconsistencies.append((pair.pairwise_id, kind))

  # Edge cases (run as inject-then-expect; in production runs they all pass
  # because no injection is performed; in PR 10 integration tests the
  # injections are performed)
  edge_cases = run_phase9_edge_cases(store, ...)

  # Post-state
  post_pairwise_count = inspect-counts.dj_pairwise_readings  # should equal pre
  passed = len(failures) == 0 for all checks AND len(edge_cases.failed) == 0

  # Persist evidence
  evidence = FsvPhase9Evidence {
    fsv_id: "FSV-9",
    fsv_check: "pairwise_readings_integrity",
    schema_version: 1,
    bundle_schema_version: 2,
    started_at_unix_ms,
    ended_at_unix_ms,
    domain: domain_pack.id,
    pre_state: { dj_pairwise_readings: pre_pairwise_count, ... },
    post_state: { dj_pairwise_readings: post_pairwise_count, ... },
    checks: [
      { name: "pair_count_per_event", passed: n_events - len(pair_count_per_event_failures), failed: len(pair_count_per_event_failures) },
      { name: "cosine_recompute_within_tolerance", passed: pair_count - len(cosine_drift_failures), failed: len(cosine_drift_failures) },
      { name: "instrument_hash_match", passed: pair_count - len(hash_drift_failures), failed: len(hash_drift_failures) },
      { name: "kinds_emitted_bitset_consistent", passed: pair_count - len(bitset_inconsistencies), failed: len(bitset_inconsistencies) },
    ],
    edge_cases: edge_cases.results,
    status: if passed then "passed" else "failed"
  }

  write_evidence_file(fsv_evidence_dir.join("fsv9_pairwise_readings_integrity.json"), evidence)

  return FsvPhaseResult { passed, evidence }
```

### 2.1 Edge-case integration tests (PR 10)

`crates/context-graph-cli/tests/dynamicjepa_phase9_fsv.rs` (NEW)

```rust
// 5 tests; each builds a v2 career-smoke bundle, then injects a defect
// and verifies the documented typed error.

#[test]
fn dynamicjepa_phase9_pairwise_readings_integrity_passes() {
    let bundle_root = run_v2_smoke_bundle();
    let result = run_fsv_phase9_blocking(&bundle_root);
    assert!(result.passed, "FSV-9 baseline should pass");
}

#[test]
fn dynamicjepa_phase9_pairwise_count_mismatch_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_missing_pairwise_row(&bundle_root);
    let result = run_fsv_phase9_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "PAIRWISE_ROW_COUNT_MISMATCH");
}

#[test]
fn dynamicjepa_phase9_pairwise_cosine_drift_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_pairwise_cosine_drift(&bundle_root);
    let result = run_fsv_phase9_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "PAIRWISE_COSINE_DRIFT");
}

#[test]
fn dynamicjepa_phase9_pairwise_instrument_hash_drift_fails_typed() {
    // ...
}

#[test]
fn dynamicjepa_phase9_pairwise_asymmetric_ordering_fails_typed() {
    // ...
}
```

The injection helpers (`inject_*`) are test fixtures that
deliberately mutate the RocksDB after a successful smoke run, then
re-invoke FSV-9. This pattern matches the existing FSV-1..8 edge-
case tests.

## 3. FSV-10: Constellation integrity (PR 11)

`crates/context-graph-dynamicjepa/src/fsv/harnesses/phase10.rs` (NEW)

```text
fn run_fsv_phase10(store, domain_pack, bundle_root, fsv_evidence_dir):
  expected_constellations = []
  if domain_pack.constellation_global is some:
    expected_constellations.append(("global", N_d_modalities))
  for subject in domain_pack.constellation_subjects:
    expected_constellations.append((subject.id, len(subject.modalities)))

  # Check 1: row count
  expected_rows = sum(n_modalities for (_, n_modalities) in expected_constellations)
  actual_rows = inspect-counts.dj_constellations
  count_match = (expected_rows == actual_rows)

  # Check 2: each row has artifact_hash matching registry
  hash_drift = []
  for centroid in iter_constellations(store):
    expected = registry.get(centroid.modality_id).artifact_hash
    if centroid.instrument_artifact_hash != expected:
      hash_drift.append(centroid.constellation_id)

  # Check 3: rebuild centroid from same source events
  rebuild_drift = []
  for centroid in iter_constellations(store):
    selector = SourceEventSelector::Field(...)  # derived from centroid.subject_id
    states = list_normalized_states_matching(store, selector)
    readings = states.iter().map(|s| get_reading(s.event_id, centroid.modality_id))
    rebuilt = construct_centroid_from_readings(readings)
    if l2_distance(rebuilt, centroid.centroid) > 1e-5:
      rebuild_drift.append(centroid.constellation_id)

  # Check 4: LOO recompute within tolerance
  loo_drift = []
  for centroid in iter_constellations(store):
    rebuilt_loo = compute_loo_stability(...)
    if abs(rebuilt_loo - centroid.loo_stability) > 1e-4:
      loo_drift.append(centroid.constellation_id)

  # Check 5: reference_set_count matches selector hit count
  ref_count_drift = []
  for centroid in iter_constellations(store):
    actual_count = count_states_matching(store, centroid_selector)
    if actual_count != centroid.reference_set_count:
      ref_count_drift.append(centroid.constellation_id)

  # Edge cases (run in PR 11 tests; production passes them)
  edge_cases = run_phase10_edge_cases(store, ...)

  # Persist evidence
  ...
```

### 3.1 Edge-case integration tests (PR 11)

`crates/context-graph-cli/tests/dynamicjepa_phase10_fsv.rs` (NEW)

```rust
#[test]
fn dynamicjepa_phase10_constellation_integrity_passes() { ... }

#[test]
fn dynamicjepa_phase10_reference_set_drift_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_delete_one_event(&bundle_root);  // simulates reference set drift
    let result = run_fsv_phase10_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "CONSTELLATION_REFERENCE_SET_DRIFT");
}

#[test]
fn dynamicjepa_phase10_instrument_hash_mismatch_fails_typed() { ... }

#[test]
fn dynamicjepa_phase10_reference_set_too_small_fails_typed() { ... }

#[test]
fn dynamicjepa_phase10_zero_norm_majority_fails_typed() { ... }
```

## 4. FSV-11: $G_\tau$ verifiability (PR 11)

`crates/context-graph-dynamicjepa/src/fsv/harnesses/phase11.rs` (NEW)

```text
fn run_fsv_phase11(store, domain_pack, bundle_root, fsv_evidence_dir):
  # For each guard decision in the bundle
  for guard in iter_guard_decisions(store):
    if guard.constellation_uuid is None:
      # v1-style guard, skip
      continue

    # Step 1: locate the matching prediction
    prediction = get_prediction(store, guard.candidate_action_id)?
    if prediction is None:
      record_failure("MISSING_PREDICTION_FOR_GUARD")
      continue

    predicted_panel = prediction.predicted_panel

    # Step 2: locate the constellation
    constellation_modalities = list_modalities_for_constellation(store, guard.constellation_uuid)?

    # Step 3: recompute each modality's cosine
    for modality_id, persisted_cos in guard.cosine_to_centroid_per_modality:
      centroid = get_constellation(store, ..., modality_id)?
      recomputed = cosine_similarity(
        predicted_panel.slot_for(modality_id),
        centroid.centroid
      )
      if abs(persisted_cos - recomputed) > 1e-4:
        record_failure("GTAU_COSINE_DRIFT", guard.guard_id)

    # Step 4: recompute thresholds from threshold_calibrations
    for modality_id, persisted_tau in guard.tau_per_modality:
      cal = get_threshold_calibration(store, ..., modality_id)?
      if abs(persisted_tau - cal.tau) > 1e-6:
        record_failure("GTAU_THRESHOLD_DRIFT", guard.guard_id)

    # Step 5: recompute gtau_verdict
    actual_pass = all(persisted_cos >= persisted_tau for (cos, tau) in zip(...))
    if guard.gtau_verdict != GuardVerdict::from_bool(actual_pass):
      record_failure("GTAU_VERDICT_INCONSISTENT", guard.guard_id)

    # Step 6: recompute combined verdict
    expected_verdict = combine(guard.utility_verdict, guard.gtau_verdict)
    if expected_verdict != guard.verdict:
      record_failure("COMBINED_VERDICT_INCONSISTENT", guard.guard_id)

  # Persist evidence
  ...
```

### 4.1 Edge-case integration tests (PR 11)

`crates/context-graph-cli/tests/dynamicjepa_phase11_fsv.rs` (NEW)

```rust
#[test]
fn dynamicjepa_phase11_gtau_verifiability_passes() { ... }

#[test]
fn dynamicjepa_phase11_threshold_drift_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_threshold_drift(&bundle_root);  // mutate dj_threshold_calibrations row
    let result = run_fsv_phase11_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "GTAU_THRESHOLD_DRIFT");
}

#[test]
fn dynamicjepa_phase11_cosine_drift_fails_typed() { ... }

#[test]
fn dynamicjepa_phase11_verdict_inconsistent_fails_typed() { ... }

#[test]
fn dynamicjepa_phase11_constellation_missing_fails_typed() { ... }
```

## 5. FSV-12: Signal-yield audit (PR 13)

`crates/context-graph-dynamicjepa/src/fsv/harnesses/phase12.rs` (NEW)

```text
fn run_fsv_phase12(store, domain_pack, bundle_root, fsv_evidence_dir):
  # Read every audit-log row, group by operation kind, sum signal_yield
  yield_by_op = aggregate_signal_yield_by_operation(store)
  row_count_by_op = count_audit_rows_by_operation(store)

  # Check 1: every operation_kind in the lookup table has consistent yield
  unknown_ops = []
  per_op_inconsistencies = []
  domain_info = DomainPackInfo::from_domain_pack(domain_pack)

  for op, total_yield in yield_by_op:
    expected_per_row = signal_yield_for_operation(op, &domain_info)
    rows = row_count_by_op[op]

    if expected_per_row == 0 and total_yield > 0:
      # Either unknown op or our table is wrong
      unknown_ops.append(op)
      continue

    if total_yield != rows * expected_per_row:
      per_op_inconsistencies.append((op, expected_per_row * rows, total_yield))

  # Check 2: bundle-level MC ratio matches the aggregated value
  total_signals = sum(yield_by_op.values())
  event_count = count_normalized_states_for_domain(store, domain_pack.id)
  computed_mc_ratio = total_signals / event_count

  table_path = bundle_root.join("paper_tables/table_mc_ratio.csv")
  csv_mc_ratio = read_mc_ratio_csv(table_path).mc_ratio_measured
  mc_ratio_drift = abs(computed_mc_ratio - csv_mc_ratio) > 0.01

  # Edge cases (PR 13 tests)
  edge_cases = run_phase12_edge_cases(store, ...)

  # Persist evidence
  ...
```

### 5.1 Edge-case integration tests (PR 13)

`crates/context-graph-cli/tests/dynamicjepa_phase12_fsv.rs` (NEW)

```rust
#[test]
fn dynamicjepa_phase12_signal_yield_audit_passes() { ... }

#[test]
fn dynamicjepa_phase12_signal_yield_table_drift_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_audit_yield_drift(&bundle_root);  // mutate one signal_yield value
    let result = run_fsv_phase12_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "SIGNAL_YIELD_TABLE_DRIFT");
}

#[test]
fn dynamicjepa_phase12_unknown_operation_fails_typed() {
    let bundle_root = run_v2_smoke_bundle();
    inject_audit_unknown_operation(&bundle_root);
    let result = run_fsv_phase12_blocking(&bundle_root);
    assert!(!result.passed);
    assert_eq!(result.first_failure_code, "SIGNAL_YIELD_UNKNOWN_OPERATION");
}

#[test]
fn dynamicjepa_phase12_event_count_mismatch_fails_typed() { ... }
```

## 6. Wiring into research-smoke

`crates/context-graph-cli/src/commands/research_smoke.rs` (extended)

After the existing FSV-1..8 invocations, add:

```rust
// Existing FSV-1..8 unchanged

// New FSV-9..12 (only run for v2 bundles)
if bundle_schema_version == 2 {
    let phase9 = run_fsv_phase9(&store, &domain_pack, &bundle_root, &fsv_evidence_dir).await?;
    fsv_results.push(("FSV-9", phase9));

    let phase10 = run_fsv_phase10(&store, &domain_pack, &bundle_root, &fsv_evidence_dir).await?;
    fsv_results.push(("FSV-10", phase10));

    let phase11 = run_fsv_phase11(&store, &domain_pack, &bundle_root, &fsv_evidence_dir).await?;
    fsv_results.push(("FSV-11", phase11));

    let phase12 = run_fsv_phase12(&store, &domain_pack, &bundle_root, &fsv_evidence_dir).await?;
    fsv_results.push(("FSV-12", phase12));
}

// Aggregate FSV report
write_aggregate_fsv_report(&bundle_root, &fsv_results)?;
```

## 7. The `aggregate_fsv_report.json` extension

`crates/context-graph-dynamicjepa/src/fsv/aggregate.rs` (extended)

The existing `AggregateFsvReport` struct gains four new
`HashMap<String, FsvPhaseResult>` entries for FSV-9..12. The
serialised JSON gains four new top-level fields.

## 8. Wall-time impact

| Harness | Wall-time on RTX 5090 (1{,}000-event career bundle) |
| --- | ---: |
| FSV-9  | ~25 s (45,000 cosine recomputes + JSON I/O) |
| FSV-10 | ~5 s  (50 centroids × LOO recompute) |
| FSV-11 | ~2 s  (5 guard rows × multi-modality recompute) |
| FSV-12 | < 1 s (audit-log scan + sum) |
| **Total** | **~33 s** added to existing FSV pass |

The existing FSV-1..8 pass is ~165 s; with v2 additions, ~200 s.
The integration test wall-time goes from 191 s to ~250 s as
documented in `14_test_plan.md` §1.

## 9. Test fixture builders

A shared test fixture file
`crates/context-graph-cli/tests/fixtures/v2_smoke_bundle.rs` (NEW)
provides:

```rust
pub fn build_v2_smoke_bundle(
    domain: &str,
    event_count: usize,
    schema_version: u32,
) -> BundleHandle {
    // Runs research-smoke, returns the bundle root
}

pub fn inject_missing_pairwise_row(bundle: &BundleHandle) -> InjectionEvidence;
pub fn inject_pairwise_cosine_drift(bundle: &BundleHandle) -> InjectionEvidence;
pub fn inject_delete_one_event(bundle: &BundleHandle) -> InjectionEvidence;
pub fn inject_threshold_drift(bundle: &BundleHandle) -> InjectionEvidence;
pub fn inject_audit_yield_drift(bundle: &BundleHandle) -> InjectionEvidence;
// ... 5 more for the various edge cases
```

These helpers are the test fixture infrastructure. They mutate the
RocksDB directly (bypassing the v2 binary's typed-error guards),
producing intentionally-corrupt bundles for edge-case verification.

## 10. PR 10 / 11 / 13 LoC budget

| PR | Files | LoC | Tests |
| --- | --- | ---: | ---: |
| 10 | `phase9.rs`, `phase9_fsv.rs` test | ~350 | 5 integration |
| 11 | `phase10.rs`, `phase11.rs`, `phase10_fsv.rs` + `phase11_fsv.rs` tests | ~600 | 10 integration |
| 13 | `phase12.rs`, `phase12_fsv.rs` test | ~250 | 4 integration |
| Shared fixture | `v2_smoke_bundle.rs` injection helpers | ~300 | 0 (used by tests) |
