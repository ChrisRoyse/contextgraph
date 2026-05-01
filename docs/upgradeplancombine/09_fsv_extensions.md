# 09 — FSV Extensions: FSV-9 / 10 / 11 / 12 Harnesses

This document specifies the four new full-state-verification harnesses
that pair with the new persisted records of v2.0. They follow the
existing FSV-1 through FSV-8 protocol verbatim:

> 1. Define the source of truth.
> 2. Print before counts/state.
> 3. Execute the trigger.
> 4. Re-read the source of truth through a separate code path.
> 5. Decode and verify expected values.
> 6. Run at least three edge cases.
> 7. Print after counts/state.
> 8. Write a dated evidence report.

## 1. FSV-9: Pairwise readings integrity

**Source of truth.** `dj_pairwise_readings` rows in the bundle's
RocksDB.

**Pre-state.** Count of pairwise rows expected =
$n_{\text{events}} \times \binom{N_d}{2}$ for the domain $d$.

**Trigger.** End-of-bundle FSV pass after panel materialisation
completes for all events.

**Independent re-read.**

1. For each `dj_normalized_states` event $e_i$ in the bundle, count
   matching `dj_pairwise_readings` rows via prefix scan with the
   event uuid prefix. Assert count = $\binom{N_d}{2}$.
2. For each pair row, recompute
   $\rho_{j,k}^{\cos} = \cos(\phi_j(e_i), \phi_k(e_i))$ from the
   persisted instrument readings (read via separate code path
   `get_instrument_reading`). Assert
   $\lvert \text{persisted} - \text{recomputed} \rvert \le 10^{-4}$.
3. For each pair row, assert both `instrument_j_artifact_hash` and
   `instrument_k_artifact_hash` match the current
   `dj_instrument_registry` entries.
4. For each pair row, assert the `kinds_emitted` bitset is consistent
   with the domain pack's `pair_kinds` declarations for both
   instruments.

**Edge cases.** (≥ 3 required by FSV protocol)

- **EC-9-A**: Inject a missing pair row by deleting one row from a
  test DB and re-running FSV-9. Expected: typed error
  `PAIRWISE_ROW_COUNT_MISMATCH` with the offending event uuid.
- **EC-9-B**: Inject a cosine drift by writing a perturbed value.
  Expected: typed error `PAIRWISE_COSINE_DRIFT` with offending
  pair uuid.
- **EC-9-C**: Inject an instrument hash mismatch by re-registering
  an instrument. Expected: typed error
  `PAIRWISE_INSTRUMENT_HASH_DRIFT`.
- **EC-9-D**: Inject an asymmetric ordering ($j > k$) by hand-
  writing a malformed row. Expected: typed error
  `PAIRWISE_ASYMMETRIC_ORDERING`.

**Evidence report.** `fsv_evidence/fsv9_pairwise_readings_integrity.json`
with: per-domain pre/post counts, per-edge-case pre/post state, the
list of any rows that failed checks (empty on success), and the wall
time of the audit pass.

## 2. FSV-10: Constellation integrity

**Source of truth.** `dj_constellations` rows in the bundle's RocksDB.

**Pre-state.** Per-domain expected constellation count from the TOML
`[[constellation.subjects]]` blocks (default: 1 global + N subjects,
each with $N_d$ centroids).

**Trigger.** Run after `build-constellation` invocation; re-run at
end-of-bundle.

**Independent re-read.**

1. For each persisted constellation row, re-read the row by
   `get_constellation(domain, subject, modality)`.
2. Re-load all reference events from `dj_normalized_states` using the
   persisted selector (TOML field+value).
3. Re-read all `dj_instrument_readings` for those events.
4. Re-run the centroid construction protocol from
   `04_constellation_design.md` §3 (zero-norm reject, per-sample L2,
   mean, mean-L2, LOO).
5. Verify
   $\lVert c_m^{\text{rebuild}} - c_m^{\text{persisted}} \rVert_2 \le 10^{-5}$.
6. Verify `instrument_artifact_hash` matches current
   `dj_instrument_registry`.
7. Recompute LOO stability and verify within $\pm 10^{-4}$.
8. Verify `reference_set_count` matches the recomputed selector hit
   count.

**Edge cases.**

- **EC-10-A**: Delete one reference event between persistence and
  audit. Expected: `CONSTELLATION_REFERENCE_SET_DRIFT`.
- **EC-10-B**: Re-register an instrument with a new hash. Expected:
  `CONSTELLATION_INSTRUMENT_HASH_MISMATCH`.
- **EC-10-C**: Build a constellation with a single reference event
  (count < 5). Expected:
  `CONSTELLATION_REFERENCE_SET_TOO_SMALL`.
- **EC-10-D**: Build a constellation with reference events all
  generating zero-norm projections. Expected:
  `CONSTELLATION_ZERO_NORM_MAJORITY`.

**Evidence report.** `fsv_evidence/fsv10_constellation_integrity.json`.

## 3. FSV-11: $G_\tau$ verifiability

**Source of truth.** `dj_guard_decisions` rows in the bundle's
RocksDB, plus the corresponding `dj_predictions`,
`dj_constellations`, and `dj_threshold_calibrations` rows.

**Pre-state.** Count of guard decisions expected: $K_d$ per plan
trace × number of plan traces.

**Trigger.** End-of-bundle FSV pass after planning completes.

**Independent re-read.**

For every `dj_guard_decisions` row in the bundle:

1. Re-read the matching `dj_predictions` row's `predicted_panel`.
2. Re-read the matching `dj_constellations` row via the row's
   `constellation_uuid`.
3. Recompute every modality's
   $\cos(\phi_m(\hat s_{t+1}), c_m)$ in fp32; verify match to
   persisted `cosine_to_centroid_per_modality` within $10^{-4}$.
4. Re-read the matching `dj_threshold_calibrations` row(s); verify
   `tau_per_modality` matches the persisted thresholds.
5. Recompute `gtau_verdict` from the cosines and taus; verify match
   to persisted.
6. Recompute `verdict := utility_verdict AND gtau_verdict`; verify
   match.

**Edge cases.**

- **EC-11-A**: Inject a corrupted threshold (mutate
  `dj_threshold_calibrations` row). Expected:
  `GTAU_THRESHOLD_DRIFT`.
- **EC-11-B**: Inject a corrupted cosine (mutate one
  `cosine_to_centroid_per_modality` field). Expected:
  `GTAU_COSINE_DRIFT`.
- **EC-11-C**: Inject a verdict mismatch (set `verdict = Accept`
  but with one $\cos < \tau$). Expected: `GTAU_VERDICT_INCONSISTENT`.
- **EC-11-D**: Reference a missing constellation
  (`constellation_uuid` not in `dj_constellations`). Expected:
  `GUARD_CONSTELLATION_MISSING`.

**Evidence report.** `fsv_evidence/fsv11_gtau_verifiability.json`.

## 4. FSV-12: Signal-yield audit

**Source of truth.** `dj_audit_log.signal_yield` field across every
row in the bundle.

**Pre-state.** Per-stage expected signal-yield row counts from
`03_signal_yield_math.md` §7.

**Trigger.** End-of-bundle FSV pass after `compute-mc-ratio`
completes.

**Independent re-read.**

1. Read every `dj_audit_log` row.
2. Group by `operation_kind`; sum `signal_yield`.
3. For each `operation_kind` $\kappa$, assert:
   $$
   \sum_{\text{row} \in \kappa} \text{signal\_yield}
   \;=\; \text{count}(\kappa) \times \text{expected\_yield}(\kappa)
   $$
   where `expected_yield` is the lookup table from
   `03_signal_yield_math.md` §7.
4. Compute the bundle-level MC ratio = total signals / event count;
   verify match to the value reported in `paper_tables/table_mc_ratio.csv`.

**Edge cases.**

- **EC-12-A**: Inject a wrong yield value on one row. Expected:
  `SIGNAL_YIELD_TABLE_DRIFT`.
- **EC-12-B**: Inject a row with `operation_kind` not in the lookup
  table. Expected: `SIGNAL_YIELD_UNKNOWN_OPERATION`.
- **EC-12-C**: Mismatch event count between
  `compute-mc-ratio` output and the bundle's event count. Expected:
  `SIGNAL_YIELD_EVENT_COUNT_MISMATCH`.

**Evidence report.** `fsv_evidence/fsv12_signal_yield_audit.json`.

## 5. FSV harness ordering at end-of-bundle

After existing FSV-1 through FSV-8 complete:

```
FSV-9  (pairwise readings)         <- depends on FSV-3 (panel)
FSV-10 (constellation integrity)   <- depends on FSV-2, FSV-3
FSV-11 (gtau verifiability)        <- depends on FSV-9, FSV-10, FSV-7
FSV-12 (signal-yield audit)        <- depends on FSV-1..FSV-11
```

Each stage's output is its own JSON file under `fsv_evidence/`; the
stage halts the bundle on the first failure, but writes the partial
evidence so an operator can diagnose. The `fsv_report.json` aggregator
sums success / failure across all 12 stages.

## 6. Tier B / Tier C / Release-C interaction

The free local CI gate (smokedoc 23) extends:

- Tier A (`smoke-a`): runs FSV-1..8 on a 120-event smoke bundle (as
  today). Tier A does **not** run the new harnesses (the bundle
  doesn't have v2 schema; Tier A keeps a v1 baseline path for
  comparability).
- Tier B (`smoke-b`): runs FSV-1..12 on a 1{,}000-event v2 bundle.
- Tier C (`release-c` → renamed `release-d` in the v2 release
  candidate): runs FSV-1..12 on five v2 paper-small bundles plus the
  multi-seed aggregate.

The Tier B integration test
`dynamicjepa_research_smoke_fsv` is extended to assert all 12 FSV
checks pass; total integration-test wall time grows from 190.92 s
(v1) to an estimated 250 s (v2; the extra ~60 s covers FSV-9 cosine
recomputation across 45{,}000 pairs and FSV-10 LOO recomputation).

## 7. Output schemas (FSV evidence files)

Each `fsv_evidence/fsv{N}_*.json` file:

```json
{
  "fsv_id": "FSV-9",
  "fsv_check": "pairwise_readings_integrity",
  "schema_version": 1,
  "bundle_schema_version": 2,
  "started_at_unix_ms": 1779000000000,
  "ended_at_unix_ms": 1779000060000,
  "domain": "career_taxonomy",
  "pre_state": { /* per-CF row counts as JSON */ },
  "post_state": { /* per-CF row counts as JSON */ },
  "checks": [
    { "name": "pair_count_per_event", "passed": 1000, "failed": 0 },
    { "name": "cosine_recompute_within_tolerance", "passed": 45000, "failed": 0 },
    { "name": "instrument_hash_match", "passed": 45000, "failed": 0 },
    { "name": "kinds_emitted_bitset_consistent", "passed": 45000, "failed": 0 }
  ],
  "edge_cases": [
    { "id": "EC-9-A", "expected": "PAIRWISE_ROW_COUNT_MISMATCH", "observed": "passed", "evidence_path": "..." },
    /* ... */
  ],
  "status": "passed"
}
```

The `aggregate_fsv_report.json` sums across all 12 harnesses.

## 8. Failure-handling discipline

Per existing FSV protocol: a single failed check halts FSV (writes
the partial evidence) but does *not* delete or repair the bundle
(per CLAUDE.md no-destructive-action rule). The operator inspects
the JSON, fixes the underlying defect, and re-runs.

The `fsv_check` field is one of a fixed enum so paper-table
generators can categorise failures across releases for drift
tracking.

## 9. Cross-references

- Pairwise feature spec → `05_pairwise_features_design.md` §2, §7
- Constellation spec → `04_constellation_design.md` §3, §5
- Guard predicate spec → `06_guard_upgrade_g_tau.md` §3, §7
- Signal-yield lookup table → `03_signal_yield_math.md` §7
- Edge-case typed-error catalog → `04` §9, `05` §9, `06` §9
- Test plan → `14_test_plan.md` §3.5–3.8
