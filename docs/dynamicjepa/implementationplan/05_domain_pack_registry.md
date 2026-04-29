# Domain Pack Registry

Domain packs make the system generalized. They declare how a domain represents state, actions, outcomes, instruments, adapters, objectives, datasets, planner constraints, and verification policies.

## File Format

Use TOML for operator-authored domain packs and bincode for persisted records.

Example deployment path:

```text
configs/dynamicjepa/domain_packs/counter_world.v1.toml
```

The registered RocksDB row is the runtime source of truth, not the TOML file. The TOML file is an input artifact whose hash is stored inside the registry row.

## Required Sections

```toml
[domain]
id = "counter_world"
version = "1.0.0"
title = "Counter World Verification Domain"
schema_version = 1

[[state.fields]]
name = "counter"
type = "i64"
required = true

[[action.fields]]
name = "delta"
type = "i64"
required = true

[[outcome.fields]]
name = "next_counter"
type = "i64"
required = true

[[instruments]]
id = "counter_scalar"
kind = "scalar"
input = "state.counter"
shape = [1]
required = true

[[objectives]]
id = "predict_next_counter"
kind = "latent_delta"
input_panel = "state_action"
target = "outcome.next_counter"
```

## Registration Workflow

1. CLI or MCP receives a domain pack file path.
2. Read the file and compute SHA-256.
3. Parse TOML into `DomainPackSource`.
4. Canonicalize into `DomainPack`.
5. Validate schemas, instruments, adapters, objectives, and verification policy.
6. Write a RocksDB batch:
   - primary domain pack row;
   - name/version index row;
   - instrument registry rows;
   - adapter registry rows;
   - audit row.
7. Independently read back the primary row and name/version index.
8. Print decoded domain pack id, version, instrument count, adapter count, objective count, and source hash.

## Validation Rules

Reject:

- empty domain id or version;
- duplicate field names;
- duplicate instrument ids;
- unsupported field type;
- required instrument without a resolvable adapter or built-in materializer;
- objective target that does not exist;
- planner policy referencing undeclared action fields;
- verification policy missing expected source-of-truth CFs;
- non-ASCII ids;
- environment-variable substitution in domain packs.

## Built-In Starter Packs

Create these as real config files during implementation:

- `counter_world.v1.toml`: deterministic synthetic verification domain.
- `context_graph_memory.v1.toml`: maps existing memory fingerprints, graph edges, source metadata, and search outcomes into DynamicJEPA events.
- `learner_state.v1.toml`: maps existing learner profiles, session states, delta logs, and retrieval logs into DynamicJEPA panels.

Later packs:

- `career_trajectory.v1.toml`: ME-JEPA recruiting use case.
- `time_series.v1.toml`: universal sensor/time-series events.
- `tabular_business_event.v1.toml`: CRM/ops records.
- `code_repository.v1.toml`: code changes, tests, build outcomes.

## Registry APIs

Core:

- `DomainPack::validate()`
- `DomainPack::canonical_hash()`
- `DomainPack::instrument_order()`
- `DomainPack::required_instruments()`

Storage:

- `register_domain_pack(pack, source_hash)`
- `get_domain_pack(domain_pack_id)`
- `get_domain_pack_by_name_version(name, version)`
- `list_domain_packs()`
- `count_domain_packs()`

MCP:

- `dynamicjepa_register_domain_pack`
- `dynamicjepa_get_domain_pack`
- `dynamicjepa_list_domain_packs`

CLI:

- `context-graph dynamicjepa register-domain --file ...`
- `context-graph dynamicjepa get-domain --id ...`
- `context-graph dynamicjepa list-domains`

## Source-Of-Truth Verification

Happy path:

- before: count domain packs, instrument registry rows, adapter registry rows;
- action: register `counter_world.v1.toml`;
- after: counts increase by exact expected values;
- read: primary row, name/version index row, every instrument registry row;
- evidence: print decoded JSON and source SHA-256.

Edge cases:

- empty TOML file: error, counts unchanged;
- duplicate instrument id: error, counts unchanged;
- objective references missing field: error, counts unchanged.
