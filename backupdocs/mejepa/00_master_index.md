# ME-JEPA Master Index

Created: 2026-04-29

This directory is the planning source for the system described in:

```text
/home/cabdru/leapablestrategy/docs/steveideasplan/14_the_single_most_powerful_thing.md
```

The target build is **ME-JEPA**: a Multi-Embedder Joint-Embedding Predictive Architecture for human career trajectories, built on Context Graph. It uses the existing multi-embedder substrate, adds recruiting-specific embedders, creates a longitudinal trajectory latent dataset, trains a latent-space predictor, and exposes product read-out heads behind per-output verification.

The system is not "a recruiting app." It is a trajectory world model over de-identified, outcome-labeled recruiting records.

## Non-negotiable Constraints

- No workarounds or silent fallbacks.
- No mock storage, mock database checks, or tests that pass against a broken system.
- If a required dependency, model, column family, source record, legal artifact, consent flag, or model artifact is missing, the system must error with a typed error and structured logs.
- Full State Verification is required for every state-changing operation.
- Return values are not proof. The source of truth must be read independently after execution.
- Edge cases must prove both the error and the before/after system state.
- Happy-path testing must use real ingested records when the real corpus is available. Synthetic control scenarios are allowed only when they are persisted through the real storage path as first-class records, not mocks.

## Document Map

| Doc | Purpose |
| --- | --- |
| [01 Research Context](01_research_context.md) | External JEPA, collapse, compliance, and documentation research |
| [02 Source Material Synthesis](02_source_material_synthesis.md) | What the Steve plan and current Context Graph repo imply |
| [03 System Architecture](03_system_architecture.md) | End-to-end ME-JEPA system architecture |
| [04 Data Contract And Privacy Gate](04_data_contract_and_privacy_gate.md) | Legal/privacy gating and de-identification contract |
| [05 Latent Panel And Schema](05_latent_panel_and_schema.md) | E1-E14 + Erec1-Erec5 panel and materialized latent record schema |
| [06 Sequence Dataset And Training Objective](06_sequence_dataset_and_training_objective.md) | Trajectory construction, masking, losses, and training stages |
| [07 Storage Sources Of Truth](07_storage_sources_of_truth.md) | Physical storage plan and source-of-truth tables/CFs |
| [08 Product Readout Heads](08_product_readout_heads.md) | Product heads on top of the foundation predictor |
| [09 Failure Policy And Observability](09_failure_policy_and_observability.md) | Fail-fast behavior, logging, no fallback policy |
| [10 Full State Verification Protocol](10_full_state_verification_protocol.md) | Manual FSV harnesses and evidence requirements |
| [11 Implementation Roadmap](11_implementation_roadmap.md) | Ordered build plan with gates |
| [12 Risks And Open Questions](12_risks_and_open_questions.md) | Risk register and unresolved decisions |
| [13 References](13_references.md) | Web research and local source references |

## Build Thesis

The 17 artifacts from the recruiting strategy docs should not be built as 17 separate foundations. They become heads over one trained ME-JEPA predictor:

```text
verified corpus
  -> privacy triage
  -> typed career events
  -> 19-embedder latent panel
  -> pairwise interaction features
  -> longitudinal trajectories
  -> ME-JEPA predictor
  -> product heads
  -> G_tau guard
  -> audit log and FSV evidence
```

The foundation value is the shared predictor and auditably materialized latent dataset. The products are read-outs.

## Immediate Gate

Before any real recruiting corpus work:

1. Title to the data must be documented.
2. Privacy policies and consent posture must be reviewed.
3. Client confidentiality constraints must be reviewed.
4. The data must not touch the ME-JEPA pipeline until Stage A privacy triage exists and passes FSV.

If any gate fails, the system stops.

