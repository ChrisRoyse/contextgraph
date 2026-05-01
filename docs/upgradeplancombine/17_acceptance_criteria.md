# 17 — Acceptance Criteria

This document specifies the concrete pass criteria for each phase
and for the v2.0 release-candidate gate. Following the v1 paper's
strict pass/fail discipline.

## 1. Per-phase acceptance

### 1.1 Phase 1 — Foundation

| Criterion | Measurable test |
| --- | --- |
| Three new CFs registered | `inspect-counts` lists 28 CFs on a v2 RocksDB |
| `DJ_CF_COUNT == 28` | Compile-time constant |
| Schema-version pre-check rejects v1 manifests | Integration test `v2_binary_refuses_v1_run_manifest_typed_error` returns expected typed error before opening RocksDB |
| v1 binary still reads v1 bundles | Unit test `v1_baseline_compatibility` uses the v1 bundle path and produces the v1 row counts |
| Cargo lints clean | `cargo clippy --all-targets --all-features -- -D warnings` |

### 1.2 Phase 2 — Pairwise features

| Criterion | Measurable test |
| --- | --- |
| Pairwise compute-on-write integrated | Integration test `research_smoke_v2_career_bundle_has_45000_pairwise_rows` |
| FSV-9 harness emits clean evidence | `dynamicjepa_phase9_fsv` integration suite passes 5/5 |
| All 4 EC-9-A..D edge cases produce typed errors | Each edge fails closed with the documented typed error |
| Pairwise compute time ≤ 50ms / event on RTX 5090 | Stress test `test_pairwise_materialisation_under_50ms_per_event` |
| TOML v2 schema validator rejects malformed packs | Unit test `test_toml_v2_validator_rejects_missing_cosine_kind` |

### 1.3 Phase 3 — Constellation + $G_\tau$

| Criterion | Measurable test |
| --- | --- |
| `build-constellation` produces 50 rows for career\_taxonomy v2 | Integration test `research_smoke_v2_career_bundle_has_50_constellation_rows` |
| `calibrate-threshold` produces 50 rows | Integration test `research_smoke_v2_career_bundle_has_50_threshold_rows` |
| FSV-10 + FSV-11 harnesses emit clean evidence | `dynamicjepa_phase10_fsv` and `dynamicjepa_phase11_fsv` integration suites pass |
| Combined verdict logic correct | Unit test `test_combined_verdict_records_failed_modalities_list` |
| LOO ≥ 0.85 across all global constellations | Verified by FSV-10 evidence |
| Subject-specific constellations build successfully OR fail with documented typed errors | All 4 cluster constellations either pass FSV-10 or are recorded with the `--allow-low-loo` provenance flag |

### 1.4 Phase 4 — MC ratio + signal-yield

| Criterion | Measurable test |
| --- | --- |
| `compute-mc-ratio` produces `table_mc_ratio.csv` | Integration test `research_smoke_v2_career_bundle_mc_ratio_in_expected_range` |
| MC ratio for career\_taxonomy A1 in $[100, 130]$ | Same integration test |
| FSV-12 harness emits clean evidence | `dynamicjepa_phase12_fsv` passes |
| Signal-yield lookup table matches `03_signal_yield_math.md` §7 | Unit test `test_signal_yield_for_each_known_operation_kind` |
| `signal_density_per_event_distribution.svg` generated | Plot manifest entry present |

### 1.5 Phase 5 — Pairwise MI audit + release-D

| Criterion | Measurable test |
| --- | --- |
| `audit-pairwise-mi` produces `table_pairwise_mi_summary.csv` | Integration test `release_d_pairwise_mi_audit_runs` |
| Career\_taxonomy A1 audit: $n_{\text{eff,strict}} \ge 8.0$ | Reference comparison (with documented WEAK/STRONG verdict tolerance) |
| `pairwise_mi_heatmap.svg` generated | Plot manifest entry present |
| Release-D gate runs end-to-end | `local_ci_gate.sh release-d` exits 0 on the documented host |
| v2 release-candidate archive built | `mejepa_5090_artifact_v2.0.0_rc1_sif_verified.tar.gz` exists with sha256 recorded |
| v2 deterministic Docker tar | Two consecutive `docker buildx build` exports hash to the same SHA-256 |
| v2 SIF built via `docker-archive` bootstrap | `singularity inspect`, `singularity sif list`, `singularity exec --nv ... verify-aggregate` all pass |
| Reference reference (release\_v2.0.0.json) captured | File committed at `reference/reference_release_v2.0.0.json` |

### 1.6 Phase 6 — Cross-domain transfer (v2.1)

| Criterion | Measurable test |
| --- | --- |
| Bridge domain pack registered | `counter_to_grid.v1.toml` validates |
| Phase 1 (counter\_world bridge training) runs 5 seeds | `transfer_results.json.phase1_passed_seeds == 5` |
| Phase 2 (gridworld zero-shot eval) produces non-trivial cosine | Transfer cosine $> 0.5$ (above random) |
| Pairwise-features ablation isolates effect | Sign-flip $p \le 0.0625$ on the ablation contrast |
| `table_cross_domain_transfer.csv` emitted | Paper-table generator produces the row |

## 2. Release-D gate full criteria

The `local_ci_gate.sh release-d` invocation (successor of release-C)
passes iff:

```
Tier A passes
+ Tier B passes
+ 5 v2 paper-small career bundles (seeds 42-46) all pass
+ multiseed-aggregate produces v2 aggregate
+ aggregate references all 4 new tables
+ release reference comparison: release_v2.0.0.json passes
+ deterministic Docker tar pair with matching sha256
+ container verify with --gpus all returns diff_count: 0
+ `audit-pairwise-mi` produces 1 row in the aggregate
+ MC ratio measured matches reference within tolerance
+ FSV-1..12 all report status: passed
+ NO unhandled overfull/error in any FSV evidence file
+ v2 release-candidate archive packager runs cleanly
+ archive SHA-256 + SIF SHA-256 + Docker tar SHA-256 all recorded
```

A single failure halts the gate; preserved bundles remain on disk.

## 3. v2.0 release-candidate acceptance

For the published release artifact `mejepa_5090_artifact_v2.0.0_rc1_sif_verified`:

| Criterion | Verifiable by |
| --- | --- |
| 28 CFs in every per-bundle RocksDB | `inspect-counts` output |
| 5-seed multi-seed aggregate passes | `aggregate_manifest.json.status == "passed"` |
| All 12 FSV harnesses produce passed evidence | `aggregate_fsv_report.json.status == "passed"` |
| MC ratio recorded per domain | `paper_tables/table_mc_ratio_multiseed.csv` |
| Pairwise MI audit recorded | `paper_tables/table_pairwise_mi_summary.csv` |
| v2 reference passes drift-check | `reproduce.sh verify-aggregate --reference reference/reference_release_v2.0.0.json` returns `diff_count: 0` |
| SIF + Docker hashes recorded | `release_manifest.json` and `container/container_manifest.json` |
| README documents v1 ↔ v2 relationship | `README.md` references the v1 artifact's preserved DOI |
| Paper.tex compiled to PDF | `paper.pdf` present, header has no v1 artifacts ($*1$, lone $1$) |

## 4. Reviewer-side acceptance (v2 reproduction)

A reviewer with the v2 release-candidate archive can:

1. Extract the archive to a fresh directory.
2. Pull the SIF or load the Docker tar.
3. Run `./reproduce.sh verify-aggregate` against the bundled aggregate
   plus the v2 reference.
4. See the same JSON output the v2 build produced:
   ```json
   {
     "status": "ok",
     "diff_count": 0,
     "manifest": "passed",
     "fsv": "passed",
     "passed_count": 5,
     "rows": {
       "ablation_multiseed_rows": 78,
       "paired_ablation_test_rows": 35,
       "prediction_multiseed_rows": 33,
       "seed_failure_rows": 0,
       "mc_ratio_rows": <expected>,
       "pairwise_mi_summary_rows": <expected>,
       "constellation_guard_rows": <expected>,
       "threshold_calibration_rows": <expected>
     }
   }
   ```
5. Read the v2 paper PDF and find every cited number recoverable from
   a paper-table CSV in the archive.

## 5. Failure-handling discipline

Per CLAUDE.md no-destructive-action rule:

- A failed phase blocks the next phase but does **not** delete or
  repair the partial bundles.
- Failed bundles are preserved on disk; the gate logs their paths
  and the typed error.
- The v2.0 release is *not* cut until all phases 1–5 pass.
- Phase 6 (transfer) is a deferred axis; failure of Phase 6 does
  *not* block v2.0 release; it blocks v2.1.

## 6. Operator regression criteria

For an operator running `dynamicjepa research-smoke --bundle-schema-version 1`
(forced v1 mode on a v2 binary):

| Criterion | Measurable |
| --- | --- |
| v1 paper-small bundle generates correctly | RocksDB has 25 CFs, not 28 |
| All v1 numbers (cosine, MSE, action-contrast) reproduce within tolerance | Reference `reference_release_v1.0.0.json` passes drift-check |
| FSV-1..8 reports passed; FSV-9..12 are skipped | `fsv_report.json.skipped == ["FSV-9", "FSV-10", "FSV-11", "FSV-12"]` |
| No new CFs are written | `inspect-counts` returns 25 not 28 |

This compatibility path is preserved so reviewers comparing v1 and v2
on the same host can rerun v1 reproductions on the v2 binary.

## 7. The "single command verification" criterion

Following the v1 paper's "one-command reproduction" discipline:

```bash
# v2 reproduction
docker run --gpus all \
  -v "$(pwd)/aggregate:/work/agg:ro" \
  contextgraph-mejepa:v2.0.0 \
  verify-aggregate \
    --aggregate /work/agg \
    --reference /workspace/reference/reference_release_v2.0.0.json \
    --json
```

Returns:

```json
{
  "status": "ok",
  "diff_count": 0,
  "schema_version": 2,
  "all_fsv_passed": true,
  "mc_ratio_match": true,
  "pairwise_mi_verdict_match": true
}
```

This is the single criterion a reviewer checks. Anything else is
secondary.

## 8. Cross-version comparison criteria

For a reviewer running both v1 and v2 release-candidates:

| Number | v1 value | v2 expected (within tolerance) |
| --- | --- | --- |
| Career val cosine (mean ± std) | 0.905 ± 0.044 | 0.905 ± 0.044 (predictor unchanged) |
| Career test cosine | 0.868 ± 0.068 | 0.868 ± 0.068 |
| Career val action-contrast | 0.960 ± 0.043 | 0.960 ± 0.043 |
| MC ratio | not reported | $\sim 114$ (constructive); measured $\sim 110$ |
| $n_{\text{eff,strict}}$ | not reported | $\sim 9.5$ |

The predictor numbers are *expected* to match v1 within tolerance
because the predictor architecture and training procedure are
unchanged. If they diverge, that is itself a failure of v2.0
acceptance — it would mean the upgrade introduced a regression in
the predictor pipeline.

## 9. The "no fail-closed escapes" criterion

For every typed error in the v2.0 catalog
(`04_constellation_design.md` §9, `05_pairwise_features_design.md`
§9, `06_guard_upgrade_g_tau.md` §9, `08_cli_and_mcp_surface.md`
§6, `10_pairwise_mi_audit.md` §8):

- Each is triggered by at least one integration test.
- Each produces the documented error code in the bundle's
  `failed_bundle_manifest.json`.
- Each leaves the filesystem un-mutated except for the failed
  bundle itself (which is preserved as evidence).

This is verified by the integration test files
`tests/dynamicjepa_phase{9,10,11,12}_fsv.rs` collectively.

## 10. Final acceptance checklist (v2.0 release-D gate)

```
[ ] Phase 1 done; cargo lints clean
[ ] Phase 2 done; FSV-9 passes; 45,000 pairwise rows in career bundle
[ ] Phase 3 done; FSV-10/11 pass; 50 constellation rows; 50 threshold rows
[ ] Phase 4 done; FSV-12 passes; MC ratio in [100, 130] for career-A1
[ ] Phase 5 done; release-D gate passes; v2 release-candidate built
[ ] All 4 new paper tables emitted in aggregate bundle
[ ] All 3 new plots generated and hashed
[ ] reference_release_v2.0.0.json captured and committed
[ ] tolerances_v2.0.json defined and committed
[ ] paper.tex updated for v1.1 (sections 1, 7, 8 expanded)
[ ] paper.pdf compiles cleanly with v2 numbers
[ ] CITATION.cff v2.0 version string
[ ] README.md documents v1 ↔ v2 relationship
[ ] v2 SIF built and `singularity exec verify-aggregate` returns ok
[ ] v2 Docker tar deterministic across two consecutive builds
[ ] Zenodo record updated for v2.0 (after gate passes)
[ ] v2 release artefact uploaded; v1 artefact preserved
[ ] All cross-references in `00_master_index.md` resolved
```

When all 17 boxes are checked, v2.0 ships.
