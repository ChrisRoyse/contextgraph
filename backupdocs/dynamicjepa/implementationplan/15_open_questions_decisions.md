# Open Questions And Decisions

These decisions should be locked before later phases. Phase 1 and Phase 2 can start with conservative defaults, but model and planning work need the answers.

## Artifact Root

Decision needed:

- where should DynamicJEPA artifacts live by default?

Recommendation:

- configurable `CONTEXT_GRAPH_DYNAMICJEPA_ARTIFACT_ROOT`;
- default under the existing Context Graph data directory if one is already configured;
- never silently use current working directory.

## New Crate In Default Members

Decision needed:

- should `context-graph-dynamicjepa` be in `default-members` immediately?

Recommendation:

- add the crate in Phase 7;
- keep it out of `default-members` until it compiles on the target CUDA stack and has FSV tests;
- once stable, add it to `default-members`.

## Domain Pack Format Strictness

Decision needed:

- TOML only or TOML plus JSON/YAML?

Recommendation:

- TOML only for v1 operator-authored packs;
- JSON only for event fixtures and CLI outputs;
- reject YAML to avoid ambiguous typing.

## Hypothetical Actions

Decision needed:

- should planner candidate actions be stored in the same `CF_DJ_ACTIONS` as observed actions?

Recommendation:

- yes, with `action_origin = observed | hypothetical | planned`;
- this keeps prediction and planning source-of-truth reads simple;
- guard policies can reject hypothetical actions before execution.

## Failed Validation Records

Decision needed:

- when should failures produce persisted failed run rows?

Recommendation:

- pre-ingestion validation failure: no domain data row, CLI/MCP error only;
- raw event already persisted then adapter fails: failed adapter run row;
- training started then fails: failed training run row;
- planning started then every candidate rejected: plan trace row with rejected status;
- verification mismatch: verification run row with failed status.

## Model Architecture V1

Decision needed:

- how large should the first trainable model be?

Recommendation:

- tiny deterministic architecture first for FSV;
- support CPU/CUDA through Candle only if the existing project policy allows the selected execution path;
- no pretend model. If Candle/model initialization fails, training fails.

## Existing Store Integration

Decision needed:

- integrate DynamicJEPA CFs directly into `RocksDbTeleologicalStore` or create a separate store wrapper?

Recommendation:

- integrate CF opening into the existing physical RocksDB database;
- expose a `DynamicJepaStore` facade that wraps the same DB handle;
- avoid separate DBs until there is a measured operational need.

## Relationship To Existing Learning Events

Decision needed:

- should existing `CF_LEARNING_EVENTS` be migrated?

Recommendation:

- no migration in v1;
- create `learner_state.v1` domain pack that reads existing learning/learner CFs and writes DynamicJEPA panels/trajectories that reference the original ids.

## Privacy And Compliance Gates

Decision needed:

- are privacy gates core DynamicJEPA or domain-pack policies?

Recommendation:

- implement generic policy hooks in DynamicJEPA;
- encode domain-specific restrictions in domain packs;
- ME-JEPA career trajectory pack must include stricter provenance and retention rules before any model training.

## Minimum Production Bar

Do not move beyond counter-world verification until these are true:

- domain registration FSV works;
- event ingestion FSV works;
- panel materialization FSV works;
- trajectory and dataset FSV works;
- artifact registry hash verification works;
- prediction and planning records are independently readable;
- all edge cases leave source-of-truth state exactly as expected.
