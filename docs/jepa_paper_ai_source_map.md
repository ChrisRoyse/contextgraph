# JEPA Paper AI Source Map

This document tells a future AI or human writer where to find the material
needed to write the full paper for the DynamicJEPA / ME-JEPA system in this
repository. It is an index, not the paper itself.

## Current Safe Paper Claim

Use this claim unless real licensed career trajectory data is added later:

```text
An auditable O*NET public-taxonomy DynamicJEPA / ME-JEPA demo and
reproducibility artifact on one RTX 5090-class local workstation.
```

Do not claim real recruiting prediction, hiring prediction, labor-market
forecasting, or real-person trajectory modeling. The current career benchmark
is generated from public O*NET taxonomy data, not licensed real career history.

## Primary Source Of Truth

The strongest current evidence bundle is:

```text
Release-C run id:
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2

Release-C manifest:
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_local_ci_manifest.json

Five passed paper-small seed bundles:
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed43
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed44
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed45
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed46

Multi-seed aggregate:
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate

Current local Docker+SIF release candidate:
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz
```

Critical hashes:

```text
Release archive SHA-256:
8cccf4980893fe949db9b50b8111e592e72ab8419438c4cd08e10c8974943ecd

Docker tar SHA-256:
099f0535e9f4f5dc8e07d0afb550ef1e3197657b52ddc7561ef11304f89dd80e

SIF SHA-256:
30b531c77966c46524383adcceea9de7983ee251935a68740c2f3009695f2825

Release reference SHA-256:
f5212d7fbf1da4df0f2dbbf6e520d4a97bc520f3e8d9a4dba1c113b73dd5c614
```

## What To Read First

Use these docs to understand the research story:

```text
docs/5090jepa/smokedocs/00_master_index.md
docs/5090jepa/smokedocs/01_research_thesis_and_claims.md
docs/5090jepa/smokedocs/05_career_taxonomy_benchmark.md
docs/5090jepa/smokedocs/06_experiment_matrix_and_ablations.md
docs/5090jepa/smokedocs/07_metrics_tables_and_figures.md
docs/5090jepa/smokedocs/10_arxiv_paper_outline.md
docs/5090jepa/smokedocs/11_risks_open_questions_and_decisions.md
docs/5090jepa/smokedocs/13_current_implementation_and_benchmark_status.md
```

Use these docs for the implementation model:

```text
docs/dynamicjepa/01_research_context.md
docs/dynamicjepa/02_core_thesis.md
docs/dynamicjepa/03_system_architecture.md
docs/dynamicjepa/04_universal_state_and_action_schema.md
docs/dynamicjepa/05_instrument_registry.md
docs/dynamicjepa/06_latent_panel_and_binding.md
docs/dynamicjepa/07_dynamic_predictor_and_objectives.md
docs/dynamicjepa/09_domain_packs.md
docs/dynamicjepa/10_storage_sources_of_truth.md
docs/dynamicjepa/11_full_state_verification_and_evaluation.md
docs/mejepa/01_research_context.md
docs/mejepa/03_system_architecture.md
docs/mejepa/06_sequence_dataset_and_training_objective.md
docs/mejepa/10_full_state_verification_protocol.md
```

Use these docs for the hardware and release context:

```text
docs/5090jepa/smokedocs/02_5090_capability_envelope.md
docs2/systemspecs.md
docs/5090jepa/smokedocs/20_reproducibility_artifact_and_release.md
docs/5090jepa/smokedocs/23_ci_smoke_regression_and_drift_detection.md
docs/5090jepa/smokedocs/21_compliance_ethics_broader_impact.md
```

## Evidence Reports

The report directory is the best chronological record of what actually
happened:

```text
docs/5090jepa/smokedocs/reports/
```

Most important reports for the paper:

```text
docs/5090jepa/smokedocs/reports/20260430_onet_zip_preflight_fsv.md
docs/5090jepa/smokedocs/reports/20260430_paper_small_1k_fsv.md
docs/5090jepa/smokedocs/reports/20260430_doc15_multiseed_aggregate_fsv.md
docs/5090jepa/smokedocs/reports/20260430_seed45_46_release_aggregate_fsv.md
docs/5090jepa/smokedocs/reports/20260501_local_smoke_b_tier_fsv.md
docs/5090jepa/smokedocs/reports/20260501_release_c_repro_artifact_fsv.md
docs/5090jepa/smokedocs/reports/20260501_release_candidate_archive_fsv.md
docs/5090jepa/smokedocs/reports/20260501_sif_release_candidate_fsv.md
```

Use the reports for root-cause fixes, failed attempts, edge-case audit
evidence, and proof that outputs were read back from disk rather than inferred
from return codes.

## Tables And Figures

The current paper-ready aggregate tables are here:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/table_prediction_metrics_multiseed.csv
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/table_ablation_multiseed.csv
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/table_paired_ablation_tests.csv
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/paper_tables/table_seed_failures.csv
```

The current generated plots are here:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/plots/multiseed_val_mse_box.svg
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/plots/multiseed_paired_delta_forest.svg
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/plots/plot_manifest.json
```

The release archive copies the reference tables into:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/paper_tables_reference/
```

## Data And Provenance

O*NET source and generated fixture provenance:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/data_manifest.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed43/data_manifest.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed44/data_manifest.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed45/data_manifest.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed46/data_manifest.json
```

O*NET attribution and licensing:

```text
THIRD_PARTY_LICENSES/ONET.md
LICENSING.md
LICENSE
```

Do not treat generated O*NET walks as real career histories. They are
deterministic public-taxonomy benchmark fixtures.

## Full State Verification Material

Read the FSV doctrine here:

```text
docs/5090jepa/smokedocs/08_full_state_verification_for_research.md
docs/5090jepa/smokedocs/23_ci_smoke_regression_and_drift_detection.md
docs/dynamicjepa/11_full_state_verification_and_evaluation.md
```

Per-bundle FSV reports:

```text
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed42/fsv_report.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed43/fsv_report.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed44/fsv_report.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed45/fsv_report.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_seed46/fsv_report.json
tmp/mejepa_local_ci_runs/20260501T_release_c_gate_v2_paper_small_multiseed_aggregate/aggregate_fsv_report.json
```

Release archive FSV:

```text
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/release_manifest.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/archive_file_hashes.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/container/container_manifest.json
tmp/mejepa_release_artifacts/mejepa_5090_artifact_v1.0.0_rc5_sif_verified/manifests/sif_verify_aggregate.json
```

## Code Locations

Core CLI and DynamicJEPA command implementation:

```text
crates/context-graph-cli/src/main.rs
crates/context-graph-cli/src/commands/dynamicjepa.rs
```

Storage source of truth:

```text
crates/context-graph-storage/src/teleological/rocksdb_store/store.rs
crates/context-graph-storage/src/teleological/rocksdb_store/types.rs
```

MCP surface:

```text
crates/context-graph-mcp/src/handlers/tools/dynamicjepa_tools.rs
```

Regression and FSV tests:

```text
crates/context-graph-cli/tests/dynamicjepa_phase3_fsv.rs
crates/context-graph-cli/tests/dynamicjepa_phase4_fsv.rs
crates/context-graph-cli/tests/dynamicjepa_phase5_fsv.rs
crates/context-graph-cli/tests/dynamicjepa_phase6_fsv.rs
crates/context-graph-cli/tests/dynamicjepa_phase7_fsv.rs
crates/context-graph-cli/tests/dynamicjepa_phase8_fsv.rs
```

Release and reproduction scripts:

```text
scripts/5090jepa/local_ci_gate.sh
scripts/5090jepa/artifact_evaluation_dry_run.sh
scripts/5090jepa/prepare_release_candidate_archive.sh
reproduce.sh
Dockerfile
Makefile
reference/reference_release_v1.0.0.json
reference/reference_paper_small_v1.json
reference/reference_smoke_hashes.json
reference/tolerances.json
```

## Suggested Paper Structure And Sources

1. Abstract:
   Use `01_research_thesis_and_claims.md`, `13_current_implementation...md`,
   and the Release-C report.
2. Introduction:
   Use `docs/dynamicjepa/01_research_context.md`,
   `docs/mejepa/01_research_context.md`, and smokedoc 01.
3. Method:
   Use dynamic JEPA architecture docs, state/action schema docs, instrument
   registry docs, and `dynamicjepa.rs`.
4. Benchmark:
   Use smokedoc 05, data manifests, and O*NET attribution files.
5. Experiments:
   Use smokedocs 06, 07, 15, 22, the aggregate manifest, and paper tables.
6. Full State Verification:
   Use smokedoc 08, smokedoc 23, FSV reports, and release reports.
7. Results:
   Use the aggregate `paper_tables/` CSVs and `plots/` SVGs.
8. Reproducibility Artifact:
   Use smokedoc 20, release-C report, release candidate report, SIF report,
   `Dockerfile`, `reproduce.sh`, and `prepare_release_candidate_archive.sh`.
9. Ethics and limitations:
   Use smokedocs 11, 21, 25, O*NET license records, and `LICENSING.md`.

## Non-Negotiable Writing Constraints

- Do not imply production readiness for hiring decisions.
- Do not claim private or real career data was used.
- Do not hide failed intermediate runs; use them as engineering evidence.
- State that current release status is local Docker+SIF release candidate until
  Zenodo DOI publication is completed.
- Use exact hashes and paths from the current source of truth when making
  reproducibility claims.

