# 6. Full-State Verification

Full-state verification (FSV) is the project's central methodological
contribution: every claim in §7 is paired with a physical readback
from a *different* code path than the one that wrote it, and every
edge case is verified to fail closed without mutating the filesystem.

## 6.1 The FSV Rule

For every operation, the rule is the same:

1. Define the source of truth (RocksDB CF row(s), file path(s), table
   row(s)).
2. Print the *before* counts and state.
3. Execute the trigger command.
4. Re-read the source of truth through a separate code path
   (`inspect-counts`, `inspect-cf`, `get-*`, file SHA-256 recomputation).
5. Decode and verify the expected values.
6. Run at least three edge cases.
7. Print the *after* counts and state.
8. Write a dated evidence report to
   `docs/5090jepa/smokedocs/reports/`.

The rule is enforced at three levels: (a) the integration test
`dynamicjepa_research_smoke_fsv.rs` (real CLI binary, real RocksDB,
real O\*NET ZIP, real CUDA training); (b) the `local_ci_gate.sh`
script for tier A/B/C runs; (c) the manual edge-FSV log files under
`tmp/mejepa_local_ci_runs/<run_id>_manual_fsv/` with before/after
state printed to disk.

## 6.2 FSV Harnesses

The eight named FSV harnesses (FSV-1 through FSV-8) cover the
runtime's stages. Their definitions and edge cases are reproduced from
[backupdocs/dynamicjepa/11_full_state_verification_and_evaluation.md](../../backupdocs/dynamicjepa/11_full_state_verification_and_evaluation.md):

- **FSV-1 Domain pack registration.** Edges: duplicate domain ID,
  missing required instrument declaration, invalid event kind.
- **FSV-2 Instrument registration.** Edges: missing artifact file,
  wrong declared dimension, unregistered preprocessing hash.
- **FSV-3 Event → panel.** Edges: empty payload, unsupported modality,
  required instrument unavailable.
- **FSV-4 Binding and trajectory.** Edges: duplicate binding, time-order
  violation, missing state ID.
- **FSV-5 Predictor dataset.** Edges: zero eligible rows, train/test
  leakage, revoked policy row.
- **FSV-6 Predictor training.** Edges: missing source dataset, artifact
  hash mismatch, eval report missing.
- **FSV-7 Planning and skill execution.** Edges: violated precondition,
  forbidden outcome predicted, missing outcome readback.
- **FSV-8 Guard and surprise.** Edges: missing constellation, anti-pole
  violation, audit write failure.

Each FSV-N has a corresponding test in
`crates/context-graph-cli/tests/dynamicjepa_phase{3,4,5,6,7,8}_fsv.rs`.
The full integration suite is `dynamicjepa_research_smoke_fsv.rs`,
which runs the real CLI binary against real RocksDB stores and the real
O\*NET 30.2 ZIP. Latest result: **10 passed, 0 failed, 190.92 s** on
the final release-C rerun
([reports/20260430_seed45_46_release_aggregate_fsv.md](../smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md)).

## 6.3 The Eleven Edge Cases per Career Bundle

Every research-smoke career bundle persists 11 edge-case rows in
`paper_tables/table_edge_cases.csv`. The five data-shape edges are
reproduced in §4.4. The remaining six are CLI- and infrastructure-shape:

| Edge | Expected error | Bundle effect |
| --- | --- | --- |
| `paper_small` without `--onet-text-zip` | typed `VALIDATION` | no bundle dir created |
| `--career-event-count` without `--onet-text-zip` | typed `VALIDATION` | no bundle dir created |
| bad O\*NET ZIP path | typed `VALIDATION` | failed bundle preserved with `run_manifest.json` and `fsv_report.json` |
| unsafe ZIP member path (`../evil.txt`) | typed `VALIDATION` | failed bundle preserved; preflight rejects before extraction |
| incomplete ZIP file count | typed `VALIDATION` (file count mismatch) | failed bundle preserved with preflight |
| too-small `--career-event-count` (<15) | typed `VALIDATION` | no bundle dir created |

(Source:
[reports/20260430_onet_zip_preflight_fsv.md](../smokedocs/reports/20260430_onet_zip_preflight_fsv.md);
[reports/20260430_paper_small_1k_fsv.md](../smokedocs/reports/20260430_paper_small_1k_fsv.md).)

## 6.4 The Multi-Seed Aggregate Edge Cases

`dynamicjepa multiseed-aggregate` adds another set of fail-closed
edges. All edges printed before/after state and the output root
remained absent unless a real aggregate was being built (no overwrite
of an existing root):

| Edge | Trigger | Observed behavior |
| --- | --- | --- |
| existing output root | re-run with same `--output-root` | typed `VALIDATION`; no overwrite; root remained at zero files |
| count mismatch | five bundles with `--expected-bundle-count 6` | typed `VALIDATION`; no output root created |
| duplicate seed | seed42 v1 and seed42 v2 in same set | typed `VALIDATION`; no output root created |
| missing required seed | provide bundles for {42,43,44,45} only with required {42–46} | typed `VALIDATION` naming `missing required career training seeds: [46]` |
| zero matched bundles | empty bundle list | typed `VALIDATION` `matched zero child bundles`; no output |
| empty `table_seed_failures.csv` | aggregate with zero failed seeds | allowed *only* for that table; metric tables still must have data rows |

(Source:
[reports/20260430_doc15_multiseed_aggregate_fsv.md](../smokedocs/reports/20260430_doc15_multiseed_aggregate_fsv.md).)

## 6.5 Release-C and SIF Edge Cases

The release-C local CI gate
([smokedoc 23](../smokedocs/23_ci_smoke_regression_and_drift_detection.md))
adds release-level FSV. Latest run id `20260501T_release_c_gate_v2`,
status `passed`. Manual edge logs are at
`tmp/mejepa_local_ci_runs/20260501T_release_c_manual_edge_fsv/`:

| Edge | Trigger | Observed |
| --- | --- | --- |
| invalid reference kind | `kind=not_a_release_reference` | exit 1, error `SCHEMA`, reference unchanged |
| missing aggregate path | aggregate path absent | exit 1, error `VALIDATION`, path absent before/after |
| numeric drift | reference mean changed to 999.0 | exit 1, error `RELEASE_REFERENCE_MISMATCH`, first diff `numeric_out_of_tolerance` |

Apptainer/SIF edges
([reports/20260501_sif_release_candidate_fsv.md](../smokedocs/reports/20260501_sif_release_candidate_fsv.md);
[reports/20260501_release_candidate_archive_fsv.md](../smokedocs/reports/20260501_release_candidate_archive_fsv.md)):

| Edge | Trigger | Observed |
| --- | --- | --- |
| existing release output dir | re-run packager with same release id | exit 1, `RELEASE_ARCHIVE_OUTPUT_DIR_EXISTS`, dir size unchanged |
| missing Docker tar | nonexistent tar path | exit 1, `RELEASE_ARCHIVE_DOCKER_TAR_1_MISSING`, no output created |
| `--require-apptainer` without SIF | flag with no path | exit 1, `RELEASE_ARCHIVE_APPTAINER_REQUIRED`, no output created |
| invalid release ID (path traversal) | non-conforming `--release-id` | exit 1, `RELEASE_ARCHIVE_RELEASE_ID_INVALID`, no path derived |
| missing required SIF | `--require-apptainer` with absent path | exit 1, `RELEASE_ARCHIVE_APPTAINER_SIF_MISSING`, no output |
| invalid SIF (wrong content) | passing a JSON in `--apptainer-sif` | exit 1, `RELEASE_ARCHIVE_APPTAINER_INSPECT_FAILED`, no output (validation moved before output dir creation) |

Each was manually triggered and the pre/post filesystem state was
written to a `.log` file inside the manual-edge-fsv directory.

## 6.6 Read-Only Inspection Hardening

A subtle defect was found during smoke-B FSV: `inspect-counts` and
`inspect-cf` previously used the standard writer/open path, which
would *create* a missing RocksDB directory rather than fail. A
read-only inspection tool that creates the source of truth makes FSV
unsound.

Fix
([reports/20260501_local_smoke_b_tier_fsv.md](../smokedocs/reports/20260501_local_smoke_b_tier_fsv.md)):
the `TeleologicalStoreConfig.create_missing_column_families` flag was
added; `inspect-counts` and `inspect-cf` now open existing stores
*only*. A hidden `--allow-missing` flag was added to the
DynamicJEPA verifier so that explicit before-state reads on a
not-yet-created DB return `db_exists=false` with zero counts and *do
not* create the directory. The MCP wrapper for inspection enforces
strict `allow_missing=false`.

## 6.7 Reproducibility Hashes (Reference Bundle)

The release-C aggregate bundle is the reference for §7. Its physical
hashes are:

| File | SHA-256 |
| --- | --- |
| `aggregate_manifest.json` | `734a698c7ed946c292cf9005c4083f7205a2a37b271037870418103eb68670e0` |
| `aggregate_fsv_report.json` | `9b1bc096c850fbd87d44447f53d33da58c35cd4337292ffbde82f63cc6b6a9b6` |
| `paper_tables/table_prediction_metrics_multiseed.csv` | `6bd81ee3b53b90c495263378c492a7fed2b123cbcfd5007ee0a14a99702608e7` |
| `paper_tables/table_ablation_multiseed.csv` | `1934260ce6d0a39b6d04d0c857e097ff3d19c3e407fca6b00760548e5936c6a9` |
| `paper_tables/table_paired_ablation_tests.csv` | `26218e9705c2170b25a7251f51234a4fb18b02eed9ab92339189f22a6dd88d80` |
| `paper_tables/table_seed_failures.csv` | `8c4e8bfb875f646993fb453e8e35203faca6fd99c41373ed8ef1ed455ec52d53` |
| `plots/multiseed_val_mse_box.svg` | `69db6c4d2ec8b837100f23fbfe74c633c2c1aa29d0a981806dcf57c4123ed77c` |
| `plots/multiseed_paired_delta_forest.svg` | `79f8050a1cf0c8c6b4f544f71461ecb2e8cc0672c987b14f1b84ee1b93bb04e4` |

(These are the v4 promoted aggregate from the `20260430T` series. The
release-C v2 aggregate has the same row counts, source-bundle
SHA-256s, and BCa parameters; cross-aggregate consistency is verified
in `aggregate_manifest.json` and reproduced in §7.)
