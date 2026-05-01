# 13 — Implementation Phases (6-Phase Roadmap)

This document is the high-level roadmap. The detailed per-PR task
breakdowns are in the `2X_*` series (file-level implementation
plans). This file enumerates phases, exit criteria, dependencies, and
estimated wall time on the documented host.

## 1. Phase summary

| Phase | Name | Wall time est. | Exit criterion |
| --- | --- | --- | --- |
| 1 | Foundation: schema bump, new CFs, JSON shapes | ~1 week | v2 binary opens v2 RocksDB; existing v1 tests pass on v1 binary unchanged |
| 2 | Pairwise features layer | ~1 week | FSV-9 passes; `table_mc_ratio.csv` Slice-1 column populated |
| 3 | Constellation + $G_\tau$ guard | ~1.5 weeks | FSV-10/11 pass; `table_constellation_guard.csv` populated; combined verdict logic in plan trace |
| 4 | MC ratio + audit-log instrumentation | ~3 days | FSV-12 passes; `compute-mc-ratio` produces full per-stage breakdown |
| 5 | Pairwise MI audit + paper-table integration | ~1 week | `audit-pairwise-mi` runs; `table_pairwise_mi_summary.csv` and heatmap emitted; release-D gate adds verdict assertion |
| 6 | Cross-domain transfer pilot (deferred to v2.1) | ~1 week | Bridge domain pack + pilot results in `transfer_results.json` |

Total v2.0 wall time estimate: **~4 weeks** of focused effort.
Phase 6 is v2.1, after v2.0 release.

## 2. Phase 1 — Foundation

### Deliverables

- `crates/context-graph-storage/src/dynamicjepa/column_families.rs` —
  three new CF constants, descriptor wiring, `DJ_CF_COUNT = 28`.
- `crates/context-graph-core/src/dynamicjepa/...` — three new typed
  records (`PairwiseReading`, `ConstellationCentroid`,
  `ThresholdCalibration`); three new id wrappers
  (`PairwiseReadingId`, `ConstellationId`, `ThresholdCalibrationId`).
- `run_manifest.json` and `aggregate_manifest.json` schema fields:
  `bundle_schema_version: 2`.
- `dj_audit_log` row schema gains `signal_yield: u32`. Existing
  v1 binaries deserialise as `Default::default()` = 0; v2 binaries
  fill it in.
- `dj_latent_panels` row schema gains `pairwise_reading_ids: Vec<...>`.
  Default empty for v1 panels; populated for v2.
- `dj_guard_decisions` row schema gains the $G_\tau$ fields
  specified in `06_guard_upgrade_g_tau.md` §2.
- `dj_plan_traces` row schema gains `no_accepted_candidate: bool` and
  `constellation_uuid_used: Option<...>`.
- v2 binary refuses to open v1 RocksDB / v1 manifest (typed error
  `BUNDLE_SCHEMA_VERSION_MISMATCH` from manifest pre-check).

### Exit criteria

- `cargo check -p context-graph-cli --all-features` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- A v2 binary creates a v2 RocksDB with all 28 CFs initialised on
  empty.
- A v1 binary continues to read v1 bundles unchanged.
- A v2 binary running against a v1 manifest fails closed with the
  typed error before opening RocksDB.

### Dependencies

None. Phase 1 is self-contained.

### Open implementation questions

- Should the schema-version pre-check live in
  `RocksDbTeleologicalStore::open()` or in the CLI command
  dispatcher? **Decision:** in the dispatcher, so non-CLI callers
  (MCP server, integration tests) can override for fixture-creation
  paths.
- Should the new optional fields default to `None` or to typed
  zero-values? **Decision:** `None` for `Option`s, default-zero for
  `u32` `signal_yield`, empty `Vec` for `pairwise_reading_ids`. The
  serde discipline already accommodates this.

## 3. Phase 2 — Pairwise features layer

### Deliverables

- `dj_pairwise_readings` write path: extend
  `put_panel_with_readings_batch` to a new
  `put_panel_with_readings_and_pairwise_batch` (rename or refactor).
- Pairwise feature computation: 7 pair-kinds per
  `05_pairwise_features_design.md` §1, default kinds emitted only
  when both instruments declare them in the TOML
  `[[instrument]] pair_kinds` list.
- TOML domain pack v2 schema: `[[instrument]].pair_kinds` field
  added; validator rejects domain packs lacking the always-on
  `cosine` pair-kind.
- Domain packs `counter_world.v2.toml`,
  `gridworld_5x5.v2.toml`, `career_taxonomy.v2.toml` updated.
- `inspect-cf --cf dj_pairwise_readings` decodes correctly.
- FSV-9 harness implemented and integrated into the
  `dynamicjepa research-smoke` orchestrator.
- Edge cases EC-9-A through EC-9-D implemented as integration tests
  in `crates/context-graph-cli/tests/dynamicjepa_phase9_fsv.rs`
  (new file).

### Exit criteria

- A v2 paper-small career bundle's `dj_pairwise_readings` CF has
  exactly 45{,}000 rows.
- FSV-9 reports 0 failures on a clean run; EC-9-A..D all produce
  the expected typed errors.
- `cargo test -p context-graph-cli --test dynamicjepa_research_smoke_fsv`
  passes; total wall time stays under 250 s.

### Dependencies

- Phase 1 complete (CFs registered).

## 4. Phase 3 — Constellation + $G_\tau$ guard

### Deliverables

- `dj_constellations` write path: `build-constellation` subcommand
  implemented and tested.
- `dj_threshold_calibrations` write path: `calibrate-threshold` and
  `recalibrate-threshold` subcommands implemented and tested.
- Phase-1 centroid construction protocol per
  `04_constellation_design.md` §3.
- Threshold calibration per
  `04_constellation_design.md` §6 (10th-percentile of disjoint
  same-session-band).
- $G_\tau$ predicate evaluation in `plan_next_action`; combined
  verdict per `06_guard_upgrade_g_tau.md` §3.
- Plan-trace upgrades per `06_guard_upgrade_g_tau.md` §8.
- TOML domain pack v2 `[constellation.global]` and
  `[[constellation.subjects]]` sections; validator integration.
- FSV-10 and FSV-11 harnesses implemented; EC-10-A..D, EC-11-A..D
  integration tests.
- `list-constellations`, `get-constellation` MCP tool wrappers.

### Exit criteria

- A v2 paper-small career bundle's `dj_constellations` CF has 50
  rows (5 constellations × 10 modalities).
- A v2 paper-small career bundle's `dj_threshold_calibrations` CF
  has 50 rows.
- A career-A1 plan trace has 5 `dj_guard_decisions` rows, each with
  the new $G_\tau$ fields populated. Combined verdict matches
  utility AND $G_\tau$.
- FSV-10 and FSV-11 report 0 failures.

### Dependencies

- Phase 1 complete (CFs registered).
- Phase 2 complete (pairwise readings, used by predictor's
  predicted-panel + the constellation centroid math).

### Open implementation questions

- **Subject-default LOO vs.\ global LOO.** Subject-specific
  constellations are typically built from smaller event subsets
  (e.g., 250 events with `cluster=software` vs. 1000 events
  global), so LOO stability may be lower. **Decision:** keep the
  default `require_loo_min = 0.85` for both; subjects with fewer
  events get rejected at registration time, which is the right
  failure mode.
- **Phase-2 conditioning ($\mathcal{L}_{\mathrm{TCT}}$).** Out of
  scope for Phase 3; deferred to v2.1. The decision rationale is in
  `04_constellation_design.md` §10.

## 5. Phase 4 — MC ratio and audit-log signal-yield

### Deliverables

- `dj_audit_log` row's `signal_yield: u32` field populated by every
  audit-log writer in the runtime. Lookup table in code matches
  `03_signal_yield_math.md` §7 verbatim.
- `compute-mc-ratio` subcommand implemented; emits
  `paper_tables/table_mc_ratio.csv` per
  `12_paper_tables_and_plots.md` §1.1.
- `signal_density_per_event_distribution.svg` plot generation.
- `aggregate_manifest.json` aggregator picks up the
  `table_mc_ratio_multiseed.csv` file.
- FSV-12 harness implemented; EC-12-A..C integration tests.

### Exit criteria

- A v2 paper-small career bundle reports MC ratio between 100 and
  130 (within $\pm 10$ of the constructive 114).
- The 5-seed multi-seed aggregate has matching MC ratios across
  seeds (variance < 1.0 per domain).
- FSV-12 reports 0 failures.

### Dependencies

- Phase 2 complete (pairwise readings contribute to MC ratio).
- Phase 3 complete (guard decisions contribute to MC ratio).

## 6. Phase 5 — Pairwise MI audit + release-D

### Deliverables

- `audit-pairwise-mi` subcommand implemented per
  `10_pairwise_mi_audit.md` §3 with the KSG-1 estimator (default).
- `pairwise_mi_heatmap.svg` plot generation per
  `10_pairwise_mi_audit.md` §4.3.
- `paper_tables/table_pairwise_mi_summary.csv` generation.
- Release-D local CI gate
  (`scripts/5090jepa/local_ci_gate.sh release-d`) added; identical
  to release-C plus the audit step.
- `reference/reference_release_v2.0.0.json` captured from a passed
  v2 release-D aggregate.
- v2 release-candidate archive packager
  (`scripts/5090jepa/prepare_release_candidate_archive_v2.sh`)
  producing `mejepa_5090_artifact_v2.0.0_rc1`.
- v2 Docker + Apptainer container build with deterministic-tar
  verification.
- Acceptance: ECAPA-style cross-encoder triangulation is **not**
  in this phase (no second-encoder family in scope for v2.0).

### Exit criteria

- `audit-pairwise-mi --domain career_taxonomy --sample-size 1000`
  produces the `table_pairwise_mi_summary.csv` row with
  $n_{\text{eff,strict}} \in [9.0, 10.0]$ on the canonical career
  bundle.
- `release-d` gate runs end-to-end on the documented host, producing
  the v2 release-candidate archive.
- v2 release reference is committed and the next `release-d` run
  matches the reference within tolerance.

### Dependencies

- Phase 4 complete (paper-table generation pipeline ready for new
  rows).

## 7. Phase 6 — Cross-domain transfer (deferred to v2.1)

### Deliverables

- `counter_to_grid.v1.toml` bridge domain pack.
- Bridge instrument mappings in `crates/context-graph-storage/...`.
- Phase-1 / Phase-2 / Phase-3 / Phase-4 bundle generation per
  `11_cross_domain_transfer.md` §4.
- `paper_tables/table_cross_domain_transfer.csv` and the
  corresponding box plot.
- Statistical reporting (BCa CI, sign-flip permutation against
  pairwise-features ablation).

### Exit criteria

- Pilot completes successfully and produces the
  `transfer_results.json` summary.
- The hypothesis test from `11_cross_domain_transfer.md` §5 returns
  a definitive (positive or negative) result.

### Dependencies

- Phase 1–5 complete (full v2.0 runtime).
- v2.0 released and reference frozen, so v2.1 changes are
  cleanly comparable.

## 8. Dependency graph

```
       Phase 1 (Foundation)
         |
         |
         +----> Phase 2 (Pairwise features)
         |        |
         |        +----> Phase 3 (Constellation + Guard)
         |        |        |
         |        |        +----> Phase 4 (MC ratio audit log)
         |        |        |        |
         |        |        |        +----> Phase 5 (MI audit + release-D)
         |        |        |        |        |
         |        |        |        |        +----> Phase 6 (transfer pilot, v2.1)
```

Phase 4's "audit-log signal_yield population" technically can start
during Phase 2 since the audit-log writers for pairwise materialisation
are added in Phase 2. But the *aggregator* (compute-mc-ratio) is
Phase 4. This dependency loosening is fine for parallelisation.

## 9. Suggested PR sequence

The smallest reviewable units, in order:

| # | PR title | Phase | Files touched (approx) |
| ---: | --- | ---: | ---: |
| 1 | feat(storage): add 3 new CFs, schema-version pre-check | 1 | 5 |
| 2 | feat(core): add 3 new typed records + ID wrappers | 1 | 8 |
| 3 | feat(audit): signal\_yield field on audit log; lookup table | 1 | 6 |
| 4 | feat(panel): pairwise readings compute-on-write + write path | 2 | 12 |
| 5 | feat(toml): pair\_kinds and constellation TOML schema | 2 | 6 |
| 6 | feat(cli): inspect-cf decoders for new CFs | 1 / 2 | 4 |
| 7 | feat(cli): build-constellation subcommand + persistence | 3 | 9 |
| 8 | feat(cli): calibrate-threshold + recalibrate-threshold | 3 | 7 |
| 9 | feat(plan): combined utility + G\_tau guard predicate | 3 | 11 |
| 10 | feat(fsv): FSV-9 harness + tests | 2 | 4 |
| 11 | feat(fsv): FSV-10 + FSV-11 harnesses + tests | 3 | 6 |
| 12 | feat(cli): compute-mc-ratio subcommand + plot | 4 | 6 |
| 13 | feat(fsv): FSV-12 harness + tests | 4 | 3 |
| 14 | feat(cli): audit-pairwise-mi subcommand + KSG estimator | 5 | 10 |
| 15 | feat(release): release-D local CI gate + v2 packager | 5 | 8 |
| 16 | docs: research paper v1.1 update | post-5 | 14 |
| 17 | feat(transfer): bridge domain pack + cross-domain pilot | 6 (v2.1) | ~12 |

That's 17 PRs total. Per ME-JEPA's existing review discipline (one
focused unit per PR), this is achievable in ~4 weeks of focused
effort.

## 10. Risk-aware ordering

Each phase has a "killable" boundary: if a phase reveals an
unanticipated defect, the upgrade can stop at the previous phase
boundary without breaking the existing v1 runtime. Specifically:

- After Phase 1: v2 binaries refuse v1 bundles, but v1 still works.
- After Phase 2: pairwise features measurable but not yet used for
  guarding. Useful as a standalone DDA contribution.
- After Phase 3: full $G_\tau$ guard. v2.0 release possible without
  Phase 5's MI audit if the audit reveals operational issues.
- After Phase 4: full MC ratio measurement. v2.0 release possible.
- After Phase 5: v2.0 fully baked, release-D candidate ready.
- After Phase 6: v2.1.

The fail-soft ordering means a partial upgrade is always shippable as
v2.0.0-rcN.

## 11. What's in the matching `2X_*` series

The detailed file-level implementation plans (per
`23_*` / `24_*` etc.\ files in this directory) provide:

- Per-file diffs (which functions to add, which to extend).
- Per-test stub (test file path, function name, key assertions).
- Per-error typed code definitions (where in the code, what
  message).
- Per-PR exit criteria and review checklist.

Read order for an implementer:
1. This file (`13_implementation_phases.md`) for high-level scope.
2. `20_implementation_plan_overview.md` for the per-PR critical
   path.
3. `21_storage_implementation.md` ... `26_release_implementation.md`
   for the per-area file-level diffs.
4. `27_first_commit_checklist.md` for the literal "first PR" tasks.
