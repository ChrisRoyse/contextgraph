//! DynamicJEPA MCP tool definitions (5090jepa Phase 9).

use serde_json::{json, Map, Value};

use crate::tools::names as tool_names;
use crate::tools::types::ToolDefinition;

pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            tool_names::DYNAMICJEPA_REGISTER_DOMAIN_PACK,
            "Register a strict DynamicJEPA domain-pack TOML into the physical RocksDB source of truth.",
            props(&[db_path(), path_prop("file", "Domain-pack TOML path.")]),
            &["dbPath", "file"],
        ),
        tool(
            tool_names::DYNAMICJEPA_LIST_DOMAIN_PACKS,
            "List registered DynamicJEPA domain packs from RocksDB.",
            props(&[db_path(), limit(), offset()]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_DOMAIN_PACK,
            "Read one registered DynamicJEPA domain pack by id and version.",
            props(&[
                db_path(),
                string_prop("id", "Domain pack id, for example counter_world."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
            ]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_INGEST_EVENT,
            "Ingest a JSONL fixture and run its registered DynamicJEPA adapter.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_prop("adapter", "Registered adapter id."),
                path_prop("file", "JSONL fixture path."),
            ]),
            &["dbPath", "domain", "adapter", "file"],
        ),
        tool(
            tool_names::DYNAMICJEPA_RUN_ADAPTER,
            "Run the registered adapter for one already-persisted raw event.",
            props(&[
                db_path(),
                string_prop("eventId", "Raw event UUID whose adapter run should complete."),
            ]),
            &["dbPath", "eventId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_MATERIALIZE_PANEL,
            "Materialize one latent panel or every pending panel for a domain.",
            props(&[
                db_path(),
                string_optional("transitionId", "Optional transition UUID to materialize."),
                bool_default("allPending", false, "When true, materialize every pending transition for domain."),
                string_optional("domain", "Domain id required when allPending is true."),
            ]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_PANEL,
            "Read one latent panel, optionally including linked instrument readings.",
            props(&[
                db_path(),
                string_prop("panelId", "Panel UUID to read."),
                bool_default("includeReadings", false, "Include linked instrument reading rows."),
            ]),
            &["dbPath", "panelId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_LIST_INSTRUMENT_READINGS,
            "List instrument readings for one raw event.",
            props(&[db_path(), string_prop("eventId", "Raw event UUID.")]),
            &["dbPath", "eventId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_CREATE_BINDING,
            "Persist a deterministic binding between two decoded DynamicJEPA source-of-truth rows.",
            props(&[
                db_path(),
                string_prop("leftCf", "Left DynamicJEPA CF name."),
                string_prop("leftKey", "Left source-of-truth key as hex."),
                string_prop("rightCf", "Right DynamicJEPA CF name."),
                string_prop("rightKey", "Right source-of-truth key as hex."),
                string_default("method", "explicit_mapping", "Binding method."),
                string_default("kind", "event_to_trajectory", "Binding kind."),
                number_default("score", 1.0, "Binding confidence score in [0,1]."),
            ]),
            &["dbPath", "leftCf", "leftKey", "rightCf", "rightKey"],
        ),
        tool(
            tool_names::DYNAMICJEPA_LIST_BINDINGS,
            "List persisted DynamicJEPA bindings.",
            props(&[
                db_path(),
                string_optional("entity", "Optional CF name or key-hex filter."),
                limit(),
                offset(),
            ]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_COMPILE_TRAJECTORIES,
            "Compile latent panels and transitions into deterministic trajectories.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("policy", "by_domain_session", "Trajectory segmentation policy."),
            ]),
            &["dbPath", "domain"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_TRAJECTORY,
            "Read one compiled DynamicJEPA trajectory.",
            props(&[db_path(), string_prop("id", "Trajectory UUID.")]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_LIST_TRAJECTORIES,
            "List compiled trajectories for a domain.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                limit(),
                offset(),
            ]),
            &["dbPath", "domain"],
        ),
        tool(
            tool_names::DYNAMICJEPA_COMPILE_DATASET,
            "Compile one-step trajectory rows into a persisted dataset shard.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("policy", "one_step", "Dataset compiler policy."),
                string_default("split", "train", "Dataset split: train, val, or test."),
            ]),
            &["dbPath", "domain"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_DATASET_SHARD,
            "Read one persisted DynamicJEPA dataset shard.",
            props(&[
                db_path(),
                string_prop("datasetId", "Dataset UUID."),
                string_prop("shardId", "Shard UUID."),
            ]),
            &["dbPath", "datasetId", "shardId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_INSPECT_DATASET_ROW,
            "Inspect one dataset row and its linked source-of-truth records.",
            props(&[
                db_path(),
                string_prop("datasetId", "Dataset UUID."),
                string_prop("shardId", "Shard UUID."),
                integer_min("row", 0, "Zero-based row index."),
            ]),
            &["dbPath", "datasetId", "shardId", "row"],
        ),
        tool(
            tool_names::DYNAMICJEPA_TRAIN,
            "Train a CUDA-only tiny DynamicJEPA model from persisted dataset shards and register the artifact.",
            props(&[
                db_path(),
                string_prop("datasetId", "Dataset UUID."),
                path_prop("config", "Strict tiny training config JSON."),
                path_prop("artifactRoot", "Artifact root directory."),
            ]),
            &["dbPath", "datasetId", "config", "artifactRoot"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_TRAINING_RUN,
            "Read one persisted DynamicJEPA training run.",
            props(&[db_path(), string_prop("id", "Training run UUID.")]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_ARTIFACT,
            "Read one model artifact and optionally recompute artifact file hashes.",
            props(&[
                db_path(),
                string_prop("id", "Model artifact UUID."),
                bool_default("verifyFiles", false, "Recompute file hashes and compare to registry."),
            ]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_PREDICT,
            "Persist one hash-verified DynamicJEPA prediction.",
            props(&[
                db_path(),
                string_prop("artifactId", "Model artifact UUID."),
                string_prop("panelId", "Input panel UUID."),
                string_prop("actionId", "Candidate action UUID."),
            ]),
            &["dbPath", "artifactId", "panelId", "actionId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_PLAN,
            "Persist a plan trace with candidate actions, predictions, guard decisions, and selected action.",
            props(&[
                db_path(),
                string_prop("artifactId", "Model artifact UUID."),
                string_prop("panelId", "Current panel UUID."),
                string_prop("skillId", "Skill policy UUID."),
            ]),
            &["dbPath", "artifactId", "panelId", "skillId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_RECORD_SURPRISE,
            "Compare a prediction against an observed outcome/panel and persist surprise when threshold is breached.",
            props(&[
                db_path(),
                string_prop("predictionId", "Prediction UUID."),
                string_prop("observedOutcomeId", "Observed outcome UUID."),
                string_prop("observedPanelId", "Observed panel UUID."),
            ]),
            &["dbPath", "predictionId", "observedOutcomeId", "observedPanelId"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_PREDICTION,
            "Read one persisted DynamicJEPA prediction.",
            props(&[db_path(), string_prop("id", "Prediction UUID.")]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_PLAN_TRACE,
            "Read one persisted DynamicJEPA plan trace.",
            props(&[
                db_path(),
                string_prop("id", "Plan trace UUID."),
                bool_default("includePredictions", false, "Include referenced prediction rows."),
                bool_default("includeGuards", false, "Include referenced guard decision rows."),
            ]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_SURPRISE,
            "Read one persisted DynamicJEPA surprise event.",
            props(&[db_path(), string_prop("id", "Surprise event UUID.")]),
            &["dbPath", "id"],
        ),
        tool(
            tool_names::DYNAMICJEPA_INSPECT_COUNTS,
            "Count every DynamicJEPA column family in the physical RocksDB source of truth.",
            props(&[db_path()]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_INSPECT_CF,
            "Decode rows from one DynamicJEPA column family.",
            props(&[db_path(), string_prop("cf", "DynamicJEPA CF name."), limit(), offset()]),
            &["dbPath", "cf"],
        ),
    ]
}

fn tool(
    name: &'static str,
    description: &'static str,
    properties: Value,
    required: &[&str],
) -> ToolDefinition {
    ToolDefinition::new(
        name,
        description,
        json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }),
    )
}

fn props(items: &[(&'static str, Value)]) -> Value {
    let mut map = Map::new();
    for (name, value) in items {
        map.insert((*name).to_string(), value.clone());
    }
    Value::Object(map)
}

fn db_path() -> (&'static str, Value) {
    path_prop("dbPath", "Path to the RocksDB data directory.")
}

fn path_prop(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({"type": "string", "minLength": 1, "description": description}),
    )
}

fn string_prop(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({"type": "string", "minLength": 1, "description": description}),
    )
}

fn string_optional(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({"type": "string", "minLength": 1, "description": description}),
    )
}

fn string_default(
    name: &'static str,
    default: &'static str,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "string", "minLength": 1, "default": default, "description": description}),
    )
}

fn bool_default(
    name: &'static str,
    default: bool,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "boolean", "default": default, "description": description}),
    )
}

fn number_default(
    name: &'static str,
    default: f64,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "number", "minimum": 0.0, "maximum": 1.0, "default": default, "description": description}),
    )
}

fn integer_min(
    name: &'static str,
    minimum: u64,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "integer", "minimum": minimum, "description": description}),
    )
}

fn limit() -> (&'static str, Value) {
    (
        "limit",
        json!({"type": "integer", "minimum": 1, "maximum": 1000000, "default": 100, "description": "Maximum decoded rows to return."}),
    )
}

fn offset() -> (&'static str, Value) {
    (
        "offset",
        json!({"type": "integer", "minimum": 0, "default": 0, "description": "Number of rows to skip."}),
    )
}
