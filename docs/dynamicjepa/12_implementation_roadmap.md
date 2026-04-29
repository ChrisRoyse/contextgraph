# Implementation Roadmap

## Phase 0: Documentation And Boundary

Deliverables:

- DynamicJEPA docs,
- source-of-truth table/CF plan,
- typed error taxonomy,
- first domain-pack selection.

Acceptance:

- docs exist,
- links resolve,
- no implementation ambiguity about source of truth.

## Phase 1: Core Types And Storage

Deliverables:

- domain, instrument, adapter, event, action, state, panel, trajectory, artifact, guard, audit types,
- storage namespace,
- read/write APIs,
- typed errors.

Acceptance:

- physical rows can be written and read,
- missing CF/table fails,
- serialization roundtrip tests pass,
- FSV-1 starts passing.

## Phase 2: Domain Pack Runtime

Deliverables:

- domain pack registration,
- instrument requirement validation,
- adapter declaration,
- benchmark/harness declaration.

Acceptance:

- FSV-1 passes,
- invalid packs fail with no mutation.

## Phase 3: Instrument Registry

Deliverables:

- artifact hashing,
- dimension contracts,
- preprocessing contracts,
- sample validation,
- version immutability.

Acceptance:

- FSV-2 passes,
- missing instrument does not fallback.

## Phase 4: Event To Panel Pipeline

Deliverables:

- event ingest,
- adapter execution,
- panel materialization,
- pairwise feature materialization.

Acceptance:

- FSV-3 passes,
- shapes and hashes verified.

## Phase 5: Binding And Trajectories

Deliverables:

- binding records,
- trajectory builder,
- leakage checks,
- split manifests.

Acceptance:

- FSV-4 and FSV-5 pass.

## Phase 6: Tiny Dynamic Predictor

Deliverables:

- minimal predictor,
- tiny real dataset,
- artifact registry,
- evaluation report.

Acceptance:

- FSV-6 passes,
- shuffled-target and leakage checks fail as expected.

## Phase 7: Planner And Skill Library

Deliverables:

- planner,
- skill records,
- precondition/outcome checks,
- approved simulator or low-risk real environment.

Acceptance:

- FSV-7 passes.

## Phase 8: Guard And Surprise Loop

Deliverables:

- guard decisions,
- surprise events,
- retraining queue,
- regression suite.

Acceptance:

- FSV-8 passes,
- surprise produces actionable diagnostics.

## Phase 9: Additional Domain Packs

Order:

1. existing Shakespeare/text tutor,
2. code/repo world,
3. ME-JEPA recruiting,
4. robotics/video sandbox,
5. finance/time-series or scientific workflow.

Acceptance:

- each domain pack passes its own FSV harnesses,
- shared runtime stays stable.

