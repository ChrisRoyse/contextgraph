# DynamicJEPA Implementation Plan

This directory is the build specification for turning the existing Context Graph repository into a generalized DynamicJEPA system: a universal, domain-pack-driven world model that stores real observed state/action/outcome transitions, builds multi-instrument latent panels, learns action-conditioned latent dynamics, and exposes planning, verification, and operational tools through MCP and CLI.

The plan starts from the repository as it exists now. Context Graph already has a Rust workspace, a 14-embedder storage and retrieval stack, RocksDB-backed teleological storage, versioned bincode records, MCP tool definitions and dispatch, CLI command modules, learner-state records, constellations, contrastive mining, typed-edge training records, and Full State Verification documentation. DynamicJEPA should extend those surfaces instead of creating a parallel application.

## Non-Negotiable Build Contracts

- No fallback behavior. Missing instruments, missing domain packs, missing model artifacts, corrupt records, version mismatches, and unavailable CUDA/model resources must return structured errors.
- No mock-data tests. Tests and manual verification use real RocksDB instances and deterministic synthetic data with known expected outputs.
- Full State Verification is required for every persisted behavior. The source of truth must be named, written, independently read, and printed with before/after state.
- All persisted records use explicit version bytes and fail on version mismatch. No silent migration and no best-effort decoding.
- Storage writes that logically belong together use RocksDB write batches. Partial state is a bug unless explicitly modeled as a failed run record.
- Model artifacts on disk are not sufficient proof. The source-of-truth artifact registry row must contain the artifact hash, path, training run id, dataset id, and evaluation status.
- The system is universal by configuration, not by special cases. Domain-specific behavior lives in registered domain packs, adapter specs, and instrument definitions.

## Implementation Plan Map

- [01_current_context_graph_inventory.md](01_current_context_graph_inventory.md): What exists now and what should be reused.
- [02_target_module_and_crate_layout.md](02_target_module_and_crate_layout.md): Exact crate/module/file layout to add.
- [03_core_data_contracts.md](03_core_data_contracts.md): Core Rust records, ids, validation, and typed errors.
- [04_storage_schema_and_migrations.md](04_storage_schema_and_migrations.md): RocksDB column families, keys, values, indexes, and migration plan.
- [05_domain_pack_registry.md](05_domain_pack_registry.md): Domain-pack format, validation, registry, and built-in starter packs.
- [06_adapter_and_instrumentation_pipeline.md](06_adapter_and_instrumentation_pipeline.md): Event ingestion, adapter execution, and instrument materialization.
- [07_latent_panel_binding_and_constellations.md](07_latent_panel_binding_and_constellations.md): Latent panels, binding, cross-domain identity, and constellation use.
- [08_trajectory_dataset_compiler.md](08_trajectory_dataset_compiler.md): Trajectory building and dataset shard compilation.
- [09_dynamic_jepa_model_training_and_artifacts.md](09_dynamic_jepa_model_training_and_artifacts.md): JEPA objectives, training runs, inference, and artifact registry.
- [10_memory_planning_skills_and_guards.md](10_memory_planning_skills_and_guards.md): Memory, planning, skill policy, guard rails, and surprise handling.
- [11_mcp_cli_api_spec.md](11_mcp_cli_api_spec.md): MCP tools and CLI commands to expose the system.
- [12_full_state_verification_manual_tests.md](12_full_state_verification_manual_tests.md): Happy-path and edge-case manual verification protocol.
- [13_observability_failure_policy.md](13_observability_failure_policy.md): Error taxonomy, structured logs, run records, and operator evidence.
- [14_phase_task_breakdown.md](14_phase_task_breakdown.md): Ordered implementation tasks and acceptance criteria.
- [15_open_questions_decisions.md](15_open_questions_decisions.md): Decisions to lock before coding later phases.

## Target Product Shape

DynamicJEPA should make Context Graph capable of this loop:

1. Register a domain pack that declares state fields, action fields, outcome fields, instruments, adapters, invariants, and objectives.
2. Ingest real events from a domain-specific source.
3. Persist raw events and normalized state/action/outcome records.
4. Run required instruments and persist a multi-instrument latent panel.
5. Bind entities, actions, goals, and contexts across events and domains.
6. Compile trajectories and dataset shards from stored records.
7. Train or update an action-conditioned JEPA predictor.
8. Register model artifacts with hashes and evaluation evidence.
9. Predict future latent states and outcomes for candidate actions.
10. Plan using predictions, persisted memory, skills, and guard decisions.
11. Record prediction outcomes, surprise events, and verification evidence.

## Definition Of Done

The project is not done when an API returns success. It is done when every major capability has a real source-of-truth record and an independent read path proving that record exists and matches the expected state.

Minimum build-level proof:

- `cargo test --workspace --all-features` passes without mock-only tests.
- A fresh temporary RocksDB database can register a domain pack, ingest a deterministic event, materialize a panel, bind entities, compile a trajectory, export a dataset shard, register a model artifact, run a prediction, record a plan trace, and persist a verification run.
- The CLI can print counts and decoded records from every DynamicJEPA column family.
- The MCP `get_capability_matrix` surface includes DynamicJEPA tools, source-of-truth CFs, and runtime counts.
- Manual FSV logs include before/after CF counts, decoded records, artifact hashes, and explicit edge-case outcomes.
