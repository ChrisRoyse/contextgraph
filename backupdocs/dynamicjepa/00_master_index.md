# DynamicJEPA Master Index

Created: 2026-04-29

DynamicJEPA is the generalized version of the ME-JEPA idea. ME-JEPA is a vertical foundation model over human career trajectories. DynamicJEPA is a domain-portable world-model substrate that can ingest many kinds of state, action, time, entity, and outcome data, bind them through registered frozen instruments, learn predictive latent dynamics, and expose guarded read-out heads.

The goal is not a single magic model that understands every possible thing. The goal is a universal runtime contract:

```text
domain data
  -> typed events and actions
  -> registered frozen instruments
  -> materialized latent panels
  -> dynamic trajectory/state graph
  -> JEPA-style latent predictor
  -> planner/read-out heads
  -> verifier/guard
  -> audited physical state
```

## Non-Negotiable Constraints

- No workarounds or silent fallbacks.
- No mock storage or mock database verification.
- Missing instruments, malformed states, broken adapters, absent source records, unregistered artifacts, or failed audit writes must produce typed errors.
- Full State Verification is required for every state-changing stage.
- Return values are not proof. The source of truth must be independently read and decoded.
- Edge-case tests must print before and after state and prove expected mutation or no-mutation.
- Dynamic adaptation must be auditable. No unlogged online model mutation.

## Document Map

| Doc | Purpose |
| --- | --- |
| [01 Research Context](01_research_context.md) | Web-researched foundations and lessons |
| [02 Core Thesis](02_core_thesis.md) | What "universal and dynamic" means |
| [03 System Architecture](03_system_architecture.md) | End-to-end DynamicJEPA architecture |
| [04 Universal State And Action Schema](04_universal_state_and_action_schema.md) | Typed event/state/action contract |
| [05 Instrument Registry](05_instrument_registry.md) | Frozen embedder/model registry and domain packs |
| [06 Latent Panel And Binding](06_latent_panel_and_binding.md) | Multi-instrument latent representation |
| [07 Dynamic Predictor And Objectives](07_dynamic_predictor_and_objectives.md) | JEPA losses, action conditioning, uncertainty |
| [08 Memory Planning And Skills](08_memory_planning_and_skills.md) | Episodic memory, planner, skill library |
| [09 Domain Packs](09_domain_packs.md) | How new domains plug in |
| [10 Storage Sources Of Truth](10_storage_sources_of_truth.md) | Physical state, artifacts, and audit records |
| [11 Full State Verification And Evaluation](11_full_state_verification_and_evaluation.md) | Manual FSV and benchmark strategy |
| [12 Implementation Roadmap](12_implementation_roadmap.md) | Ordered build plan |
| [13 Risks And Open Questions](13_risks_and_open_questions.md) | Risks and decisions |
| [14 References](14_references.md) | Web and local references |
| [Implementation Plan](implementationplan/00_master_implementation_plan.md) | Repository-specific build specification from the current Context Graph codebase |

## What You Need To Build

At a high level:

1. A universal event/action/state schema.
2. A domain-pack registry.
3. A frozen instrument registry.
4. A latent panel materializer.
5. A cross-modal binding layer.
6. A trajectory and state-graph builder.
7. A JEPA-style latent dynamics predictor.
8. A planner over predicted latent futures.
9. A memory system with episodic, semantic, procedural, and artifact memory.
10. A verifier/guard layer for every prediction and action.
11. A physical storage layer with independent readback.
12. FSV harnesses that prove every stage works.
