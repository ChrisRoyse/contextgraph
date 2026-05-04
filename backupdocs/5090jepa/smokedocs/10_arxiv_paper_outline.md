# 10 - arXiv Paper Outline

Working title:

```text
Dynamic/ME-JEPA: Source-of-Truth Verified Latent World Modeling on a Single RTX 5090
```

## Abstract

State the problem: latent predictive architectures are promising, but systems
that make their data, training, planning, and failure behavior auditable are
rare. Present Dynamic/ME-JEPA as a domain-pack-driven runtime with full-state
verification. Report control-domain and career-taxonomy benchmark results on a
single RTX 5090.

## 1. Introduction

- Why latent prediction instead of reconstruction.
- Why auditability matters for career/recruiting domains.
- Why single-GPU feasibility matters.
- Contributions.

## 2. Related Work

- JEPA / I-JEPA / V-JEPA.
- VICReg and collapse prevention.
- Occupational/career taxonomies: O*NET, ESCO.
- Auditable ML systems and model/data provenance.

## 3. System

- Domain packs.
- Typed event records.
- Multi-instrument latent panels.
- Trajectory/dataset compiler.
- JEPA trainer and artifact registry.
- Prediction/planning/surprise.
- Full State Verification.

## 4. ME-JEPA Career Benchmark

- Public taxonomy data sources.
- Event schema.
- Action schema.
- Outcome schema.
- Provenance and license manifest.
- Why this is a benchmark, not production recruiting.

## 5. Experiments

- Runtime universality across counter_world, gridworld, career_taxonomy.
- Career next-state prediction.
- Action-conditioned planning.
- Ablations.
- Failure/audit stress tests.
- 5090 feasibility.

## 6. Results

Use generated tables:

- domain runtime table;
- prediction metrics;
- ablations;
- edge-case/audit behavior;
- single-GPU performance.

## 7. Discussion

- What the system proves.
- What remains unproven without licensed real recruiting data.
- Failure modes.
- Generalization to real data after privacy gates.

## 8. Limitations

- Public-taxonomy benchmark is not real hiring outcome prediction.
- Small models are demonstration-scale.
- Current strict model-quality audit gaps must be disclosed if unresolved.
- Privacy/legal gates are only as strong as their source documents and FSV.

## 9. Reproducibility

- Hardware/software.
- Data sources and hashes.
- Command log.
- Evidence bundle schema.
- How to rerun smoke and full experiments.

## Appendix A - Domain Pack Specs

Include counter_world, gridworld, and career_taxonomy excerpts.

## Appendix B - Full State Verification Evidence

Summarize:

- CF counts;
- artifact hashes;
- decoded rows;
- edge cases.

## Appendix C - Failed Runs

Include intentional failures and root-cause diagnostics.

