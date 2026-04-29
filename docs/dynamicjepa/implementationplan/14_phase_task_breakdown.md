# Phase Task Breakdown

This is the ordered build plan. Each phase should end with source-of-truth verification, not only compilation.

## Phase 0: Lock Contracts And Fixtures

Tasks:

- create `configs/dynamicjepa/domain_packs/counter_world.v1.toml`;
- create counter-world happy path and edge-case fixtures;
- create tiny training config fixture;
- add this implementation plan to the repo;
- decide exact crate name and artifact root config.

Acceptance:

- fixture files parse as TOML/JSON;
- expected count deltas are documented;
- no code path exists yet that claims DynamicJEPA works.

## Phase 1: Core Types

Tasks:

- add `context_graph_core::dynamicjepa`;
- implement ids, record header, domain pack, schema, event, state/action/outcome, instrument, panel, binding, trajectory, dataset, artifact, prediction, planning, verification records;
- add validation methods;
- add `DynamicJepaError`;
- add serialization round-trip tests.

Acceptance:

- `cargo test -p context-graph-core dynamicjepa --all-features`;
- invalid records fail validation with typed errors;
- every record hash is deterministic.

## Phase 2: Storage Data Plane

Tasks:

- add DynamicJEPA CFs and options;
- integrate CF open path;
- add versioned encode/decode helpers;
- add key helpers;
- implement domain pack store;
- implement event/state/action/outcome/transition store;
- implement panel/binding/trajectory/dataset/artifact/prediction/planning/verification stores;
- add inspection/count APIs.

Acceptance:

- fresh DB opens with all DynamicJEPA CFs;
- CF count constant matches actual list;
- every put has an independent get/list/count test;
- corrupt version bytes fail with typed codec error.

## Phase 3: CLI Inspection And Domain Registration

Tasks:

- add `dynamicjepa` CLI command module;
- implement `register-domain`, `list-domains`, `get-domain`, `inspect-counts`, `inspect-cf`;
- wire JSON output.

Acceptance:

- register `counter_world.v1.toml` into a real temp DB;
- read primary row and name/version index row;
- print before/after counts;
- duplicate instrument edge case leaves counts unchanged.

## Phase 4: Adapter And Panel Pipeline

Tasks:

- implement `json_event` adapter;
- persist raw event and adapter run records;
- persist normalized state/action/outcome/transition records;
- implement scalar/categorical/time instruments;
- materialize latent panels;
- add CLI/MCP write/read commands.

Acceptance:

- ingest three counter-world events;
- physically verify raw events, adapter runs, normalized records, readings, and panels;
- empty input and invalid action edge cases match expected count deltas.

## Phase 5: Binding And Trajectories

Tasks:

- implement deterministic binding methods;
- implement binding secondary indexes;
- implement trajectory compiler;
- implement trajectory read/list commands.

Acceptance:

- compile one counter-world trajectory with three ordered transitions;
- read trajectory row and verify order;
- binding with missing evidence fails and leaves primary/index counts unchanged.

## Phase 6: Dataset Compiler

Tasks:

- implement one-step dataset policy;
- implement deterministic split policy;
- implement leakage checks;
- implement negative sampling from real panels where declared;
- add dataset read/inspect commands.

Acceptance:

- counter-world three-event sequence compiles into a shard with two one-step rows;
- read dataset shard and inspect row 0 and row 1;
- insufficient negatives fail explicitly when an objective requires negatives.

## Phase 7: DynamicJEPA Service Crate

Tasks:

- add `context-graph-dynamicjepa` crate;
- implement service facade over storage;
- implement minimal encoder/predictor architecture;
- implement losses for latent delta and action contrast;
- implement artifact writer/registry;
- add model load hash verification.

Acceptance:

- tiny config trains on counter-world dataset;
- training run row and artifact row are persisted;
- artifact file hashes match registry;
- corrupt artifact file fails load and writes no prediction.

## Phase 8: Prediction, Planning, And Surprise

Tasks:

- implement prediction service and records;
- implement candidate action persistence;
- implement skill policy record and simple skill generator;
- implement guard decisions;
- implement plan trace;
- implement surprise event recording.

Acceptance:

- prediction row references actual artifact, panel, and action;
- plan trace references predictions and guard decisions;
- rejected guard creates guard evidence and failed/rejected trace;
- observed outcome beyond threshold creates surprise event linked to prediction and outcome.

## Phase 9: MCP Surface

Tasks:

- add DynamicJEPA MCP tool definitions;
- add handlers and DTOs;
- add dispatch entries;
- update capability matrix;
- update tool count tests.

Acceptance:

- tools/list includes every DynamicJEPA tool;
- each tool has schema and description;
- mutating tools return source-of-truth ids and verification summaries;
- read tools decode records by id.

## Phase 10: End-To-End FSV Harness

Tasks:

- implement `verify-counter-world`;
- persist `VerificationRunRecord`;
- run happy path;
- run empty input, invalid action, and max panel edge cases;
- add evidence log artifacts.

Acceptance:

- a fresh DB can run the full verification command;
- verification run row stores before/after counts and decoded evidence;
- any mismatch fails the command and prints root-cause context.

## Phase 11: Context Graph Domain Packs

Tasks:

- implement `context_graph_memory.v1.toml`;
- implement `learner_state.v1.toml`;
- map existing memories, graph edges, learning events, and learner-state records into DynamicJEPA panels;
- join existing constellations and training records where useful.

Acceptance:

- real existing Context Graph data can compile into DynamicJEPA trajectories;
- source rows are physically verified in original CFs and DynamicJEPA CFs;
- no synthetic-only pathway is required for production data.

## Phase 12: ME-JEPA Career Trajectory Pack

Tasks:

- implement `career_trajectory.v1.toml`;
- map profile, role, company, skill, time, causal, and outcome instruments;
- implement privacy/compliance gates from ME-JEPA docs;
- train and evaluate career trajectory predictors.

Acceptance:

- all records come from licensed/allowed sources;
- every profile/event has provenance;
- model artifacts register with evaluation evidence;
- product readout heads are downstream of verified predictions and panels.
