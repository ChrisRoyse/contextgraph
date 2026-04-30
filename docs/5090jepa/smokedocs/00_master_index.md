# 00 - ME-JEPA 5090 Research Smoke Master Index

This directory defines the research-grade smoke plan for turning the completed
Phase 10 DynamicJEPA runtime into a 5090-feasible ME-JEPA demonstrator.

The key correction is scope: Phase 10 proves the auditable DynamicJEPA runtime.
The ME-JEPA paper demo should use that runtime to show career-domain latent
dynamics without overclaiming unavailable recruiting data. If licensed
recruiting/career records are absent, the paper-safe path is a public,
provenanced career-taxonomy benchmark grounded in O*NET/ESCO-style occupational
data and controlled career-transition events.

## Target Claim

> Dynamic/ME-JEPA is an auditable, domain-pack-driven latent world-model runtime
> that can ingest typed event streams, materialize multi-instrument panels, train
> action-conditioned JEPA predictors, plan over candidate actions, record
> surprise, and prove every stage through source-of-truth storage readback on a
> single RTX 5090.

## Non-Negotiable Rules

- No fallback behavior. Missing data, missing licenses, corrupt records, missing
  artifacts, unavailable CUDA, or failed hashes must produce typed errors.
- No mock data. Controlled data is allowed only when persisted through the real
  pipeline as first-class records with provenance.
- Every result used in the paper must have a source of truth: RocksDB CF rows,
  artifact files, hashes, command logs, decoded records, and before/after counts.
- The demo must distinguish "public taxonomy benchmark" from "real recruiting
  corpus." Do not imply real recruiting predictive validity without licensed
  real data and privacy gates.
- Every smoke run writes a research bundle that can be attached to a paper,
  appendix, or reproducibility artifact.

## Document Map

Docs `00-13` are the original ME-JEPA smoke plan and current-state record.
Docs `14-35` specify the **monolith v1.0 release**: one repo (with
ClipCannon vendored at `clipcannon/` since 2026-04-30), one binary surface,
one paper, one product. Start at `14` for the monolith master plan, gap
matrix, and dependency graph; then `26` for the architecture; then `35` for
the 16-week roadmap.

| File | Purpose |
| --- | --- |
| `00_master_index.md` | Scope, target claim, document map |
| `01_research_thesis_and_claims.md` | Paper thesis, novelty, claims, non-claims |
| `02_5090_capability_envelope.md` | What is feasible on RTX 5090 32GB |
| `03_research_bundle_schema.md` | Required run artifact tree and JSON fields |
| `04_smoke_ladder_and_gates.md` | Ordered smoke gates before the full run |
| `05_career_taxonomy_benchmark.md` | Public/provenanced career-domain demo design |
| `06_experiment_matrix_and_ablations.md` | Experiments, ablations, controls, acceptance |
| `07_metrics_tables_and_figures.md` | Metrics, paper tables, plots, analysis outputs |
| `08_full_state_verification_for_research.md` | FSV protocol adapted for paper evidence |
| `09_runbook_iteration_loop.md` | Concrete command sequence and iteration loop |
| `10_arxiv_paper_outline.md` | Proposed arXiv paper structure |
| `11_risks_open_questions_and_decisions.md` | Risks, unresolved items, locked choices |
| `12_references.md` | Local and external references |
| `13_current_implementation_and_benchmark_status.md` | Implemented smoke surface and latest FSV benchmark evidence |
| `14_remaining_work_master_plan.md` | **Monolith master plan**: gap matrix, dependency graph, completion criteria |
| `15_multiseed_completion_and_statistical_reporting.md` | Multi-seed paper-small with BCa CI + paired sign-flip permutation tests |
| `16_decoder_head_and_topk_metrics.md` | Decoder head + top-k occupation + skill-vector reconstruction |
| `17_paper_scaling_and_full_run.md` | 1K -> 10K -> 50K scaling ladder, VRAM/runtime budget |
| `18_esco_crosswalk_and_replication_study.md` | ESCO replication + O*NET <-> ISCO crosswalk decoder |
| `19_paper_drafting_figures_and_supplementary.md` | **One-paper** drafting (DDA + ME-JEPA + FSV + TCT in one manuscript) |
| `20_reproducibility_artifact_and_release.md` | Apptainer + Docker, Zenodo DOI, ACM artifact-badging packet |
| `21_compliance_ethics_broader_impact.md` | NIST AI RMF mapping, O*NET attribution, dual-use disclosure |
| `22_hyperparameters_and_collapse_diagnostics.md` | LR/latent-dim/batch sweeps + rank/alignment/uniformity/SIGReg-residual |
| `23_ci_smoke_regression_and_drift_detection.md` | Per-PR / nightly / release CI tiers + reference-hash drift gating |
| `24_multistep_rollout_and_planning_horizon.md` | Optional v1.1 H>1 rollouts (direct vs autoregressive) |
| `25_privacy_gate_real_data_phase12.md` | Post-v1.0 Phase 12 privacy gate (real subject-level data) |
| `26_closed_loop_master_architecture.md` | **Monolith architecture**: one process, six layers, in-host Rust↔Python boundary |
| `27_dda_to_mejepa_panel_bridge.md` | DDA signals as ME-JEPA instrument readings; `dda_signals` adapter |
| `28_constellation_guard_for_mejepa_outputs.md` | Per-output Constellation Guard wrapping predict/plan/surprise/decoder |
| `29_unified_provenance_bundle_schema.md` | Single bundle schema covering DDA + ClipCannon + ME-JEPA + TCT |
| `30_mutual_information_embedder_audit.md` | KSG/MINE pairwise MI audit measuring 91-signal upper bound tightness |
| `31_monolith_definition_and_scope.md` | **What the monolith is**: one repo, one product, brand selection (Teleox) |
| `32_merged_repo_structure.md` | Merged repo layout with vendored `clipcannon/`; cross-language Makefile, Dockerfile, .gitignore |
| `33_unified_surface.md` | One CLI binary, one MCP server, one HTTP API; supervisor architecture for Rust↔Python |
| `34_single_product_gtm.md` | **One product, three buyer framings, four deployment tiers**; replaces fragmented portfolio |
| `35_roadmap_to_v1_monolith.md` | 16-week critical path to v1.0 release event |
| `reports/` | Dated operator reports for paper-small runs, failures, fixes, and multi-seed results |

## Source Of Truth For The Smoke Program

Primary:

- Fresh RocksDB DBs under `tmp/mejepa_research_runs/<run_id>/dbs/` when
  using `dynamicjepa research-smoke`.
- DynamicJEPA CFs listed by `dynamicjepa inspect-counts`.
- `dj_verification_runs` rows from Phase 10 verifiers and ME-JEPA smoke runs.
- Decoded rows from `dynamicjepa inspect-cf` and `get-*` commands.

Secondary:

- Artifact files under `tmp/mejepa_research_runs/<run_id>/artifacts/`.
- Research bundle files under `tmp/mejepa_research_runs/<run_id>/`.
- SHA-256 hashes for domain packs, fixtures, data sources, model artifacts,
  command logs, and evaluation reports.

Passing a smoke gate requires both command success and independent readback from
the source of truth.

## Current Implemented Demo Path

The current ready path is an O*NET 30.2-backed smoke run:

```bash
CUDA_COMPUTE_CAP=120 ./target/debug/context-graph dynamicjepa research-smoke \
  --run-id "$RUN_ID" \
  --run-root tmp/mejepa_research_runs \
  --fixtures-root configs/dynamicjepa \
  --onet-text-zip tmp/onet_probe/db_30_2_text.zip \
  --system-specs docs2/systemspecs.md \
  --json
```

The latest promoted paper-small evidence is the three-run explicit-seed set:

- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed42_v2`;
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed43_v2`;
- `tmp/mejepa_research_runs/20260430T_paper_small_1k_seed44_v1`.

Each bundle uses real O*NET 30.2 taxonomy rows to generate 1,000 career events,
then verifies counter_world, gridworld_5x5, career_taxonomy, same-seed
reproducibility, and career A0-A5 ablations with RocksDB and artifact
readbacks. Dated findings and failure analysis are recorded under
`docs/5090jepa/smokedocs/reports/`.
