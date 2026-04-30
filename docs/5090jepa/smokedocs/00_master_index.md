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

## Source Of Truth For The Smoke Program

Primary:

- Fresh RocksDB DB under `/tmp/mejepa-5090-smoke-*`.
- DynamicJEPA CFs listed by `dynamicjepa inspect-counts`.
- `dj_verification_runs` rows from Phase 10 verifiers and ME-JEPA smoke runs.
- Decoded rows from `dynamicjepa inspect-cf` and `get-*` commands.

Secondary:

- Artifact files under `/tmp/mejepa-5090-artifacts-*`.
- Research bundle files under `tmp/mejepa_research_runs/<run_id>/`.
- SHA-256 hashes for domain packs, fixtures, data sources, model artifacts,
  command logs, and evaluation reports.

Passing a smoke gate requires both command success and independent readback from
the source of truth.
