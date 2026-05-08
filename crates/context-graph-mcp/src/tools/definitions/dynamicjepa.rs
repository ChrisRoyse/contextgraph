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
                string_prop("candidateActionJson", "Optional durable JSON file containing one actual pending action to score alongside declared candidates."),
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
            tool_names::DYNAMICJEPA_BUILD_CONSTELLATION,
            "Build immutable DynamicJEPA constellation centroids from persisted instrument readings.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                string_default("subject", "global", "Subject id, or global."),
                string_optional("sourceEventSelector", "Reference selector such as first:30 or all."),
                string_optional("builtByRunId", "Optional build/verification run UUID."),
            ]),
            &["dbPath", "domain"],
        ),
        tool(
            tool_names::DYNAMICJEPA_LIST_CONSTELLATIONS,
            "List persisted DynamicJEPA constellation centroids.",
            props(&[
                db_path(),
                string_optional("domain", "Optional domain id filter."),
                string_optional("subject", "Optional subject id filter."),
                limit(),
                offset(),
            ]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_GET_CONSTELLATION,
            "Read one DynamicJEPA constellation centroid.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                string_default("subject", "global", "Subject id, or global."),
                string_prop("modality", "Instrument id or 1-based modality ordinal."),
            ]),
            &["dbPath", "domain", "modality"],
        ),
        tool(
            tool_names::DYNAMICJEPA_CALIBRATE_THRESHOLD,
            "Calibrate DynamicJEPA G_tau thresholds from a disjoint held-out event set.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                string_default("subject", "global", "Subject id, or global."),
                string_default("modality", "all", "Instrument id, 1-based ordinal, or all."),
                string_optional("calibrationEventSelector", "Held-out selector such as last:30."),
                integer_optional("percentile", "Override percentile in [0,100]."),
            ]),
            &["dbPath", "domain"],
        ),
        tool(
            tool_names::DYNAMICJEPA_RECALIBRATE_THRESHOLD,
            "Supersede an existing DynamicJEPA G_tau threshold calibration.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                string_default("subject", "global", "Subject id, or global."),
                string_default("modality", "all", "Instrument id, 1-based ordinal, or all."),
                string_optional("calibrationEventSelector", "Held-out selector such as last:30."),
                string_prop("supersedes", "Prior threshold calibration UUID."),
                string_prop("reason", "Operator-visible recalibration reason."),
                integer_optional("percentile", "Override percentile in [0,100]."),
            ]),
            &["dbPath", "domain", "supersedes", "reason"],
        ),
        tool(
            tool_names::DYNAMICJEPA_COMPUTE_MC_RATIO,
            "Aggregate DynamicJEPA audit-log signal_yield rows into MC-ratio paper evidence.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                path_prop(
                    "outputDir",
                    "Fresh output directory for table_mc_ratio.csv and plot evidence.",
                ),
            ]),
            &["dbPath", "domain", "outputDir"],
        ),
        tool(
            tool_names::DYNAMICJEPA_AUDIT_PAIRWISE_MI,
            "Estimate pairwise mutual information across persisted instrument readings and write release-D evidence.",
            props(&[
                db_path(),
                string_prop("domain", "Registered domain pack id."),
                string_default("domainVersion", "1.0.0", "Domain pack version."),
                integer_default(
                    "sampleSize",
                    50,
                    1000,
                    "Number of persisted events to audit; must meet the KSG support floor.",
                ),
                string_default("estimator", "ksg", "MI estimator; Phase 5 supports ksg."),
                integer_default("ksgK", 1, 5, "K nearest neighbours for KSG-1."),
                integer_default(
                    "bootstrapIters",
                    1,
                    1000,
                    "Deterministic bootstrap iterations for confidence intervals.",
                ),
                integer_default("seed", 0, 20260501, "Deterministic audit seed."),
                path_prop("outputDir", "Fresh output directory for pairwise MI artifacts."),
            ]),
            &["dbPath", "domain", "outputDir"],
        ),
        tool(
            tool_names::DYNAMICJEPA_CROSS_DOMAIN_TRANSFER,
            "Run the counter_world to gridworld DynamicJEPA cross-domain transfer pilot.",
            props(&[
                path_prop(
                    "outputRoot",
                    "Fresh output root for transfer_results.json, paper table, plots, and bridge artifacts.",
                ),
                integer_array_default(
                    "seeds",
                    &[42, 43, 44, 45, 46],
                    "Paired transfer seeds.",
                ),
                integer_default("sourceEvents", 20, 1000, "Counter-world bridge events per seed."),
                integer_default("targetEvents", 20, 200, "Gridworld bridge events per seed."),
                integer_default(
                    "bootstrapIters",
                    1,
                    10000,
                    "Bootstrap iterations for seed-level confidence intervals.",
                ),
                integer_default("trainEpochs", 1, 160, "Training epochs per bridge predictor."),
                integer_default("batchSize", 1, 64, "Training batch size per bridge predictor."),
                integer_default(
                    "maxSecondsPerTraining",
                    1,
                    120,
                    "Maximum wall-clock seconds per bridge training run.",
                ),
                number_positive_default(
                    "learningRate",
                    0.001,
                    "AdamW learning rate for bridge predictor training.",
                ),
                number_positive_default(
                    "stoppingTarget",
                    0.20,
                    "Required val_latent_mse convergence target.",
                ),
            ]),
            &["outputRoot"],
        ),
        tool(
            tool_names::DYNAMICJEPA_BUILD_SEMANTIC_INDEX,
            "Build a persisted compiler/LSP-backed semantic index for a real repair repository.",
            props(&[
                path_prop("repo", "Repair repository root. Must be a real git checkout."),
                path_prop("output", "Output JSON file for the semantic index."),
                string_array_optional(
                    "languages",
                    "Optional language allow-list. Omit to require every detected source language.",
                ),
                integer_default(
                    "maxFiles",
                    1,
                    5000,
                    "Maximum indexed source files; exceeding this fails closed.",
                ),
            ]),
            &["repo", "output"],
        ),
        tool(
            tool_names::DYNAMICJEPA_VALIDATE_CORPUS_DIVERSITY,
            "Validate real SWE-loop DynamicJEPA corpus diversity from persisted RocksDB rows.",
            props(&[
                db_path(),
                integer_default("minRawEvents", 1, 60, "Minimum persisted raw events."),
                integer_default(
                    "minToolFamilies",
                    1,
                    3,
                    "Minimum distinct tool families in dj_actions.",
                ),
                integer_default(
                    "minLanguages",
                    1,
                    2,
                    "Minimum distinct AST/source languages in dj_actions.",
                ),
                integer_default(
                    "minPatchDeltas",
                    1,
                    3,
                    "Minimum distinct patch delta classes in dj_actions.",
                ),
                integer_default(
                    "minCompilerChecked",
                    0,
                    1,
                    "Minimum rows with compiler_semantic_status=checked.",
                ),
                path_optional("output", "Optional evidence JSON file to write and read back."),
            ]),
            &["dbPath"],
        ),
        tool(
            tool_names::DYNAMICJEPA_ATTRIBUTE_TEST_DELTA,
            "Attribute verifier/test deltas from real coverage and failure-signature evidence.",
            props(&[
                path_prop("repo", "Repair repository root used to validate changed paths."),
                path_prop("coverageJson", "Coverage JSON from the real test/coverage tool."),
                path_prop(
                    "changedFilesJson",
                    "JSON file containing changed_files and optional touched_symbols arrays.",
                ),
                path_prop(
                    "failuresBeforeJson",
                    "JSON file containing verifier/test failures before the action.",
                ),
                path_prop(
                    "failuresAfterJson",
                    "JSON file containing verifier/test failures after the action.",
                ),
                path_prop("output", "Output attribution evidence JSON file."),
            ]),
            &[
                "repo",
                "coverageJson",
                "changedFilesJson",
                "failuresBeforeJson",
                "failuresAfterJson",
                "output",
            ],
        ),
        tool(
            tool_names::DYNAMICJEPA_COMPARE_SHADOW_UTILITY,
            "Compare candidate-vs-active DynamicJEPA live-shadow utility evidence from persisted rows.",
            props(&[
                db_path(),
                string_prop("candidateArtifactId", "Candidate model artifact UUID."),
                string_prop("activeArtifactId", "Active/baseline model artifact UUID."),
                number_positive_default(
                    "minMargin",
                    0.000001,
                    "Minimum positive utility margin required for promotion.",
                ),
                path_optional("output", "Optional evidence JSON file to write and read back."),
            ]),
            &["dbPath", "candidateArtifactId", "activeArtifactId"],
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
            "Decode rows from one DynamicJEPA column family, or decode one exact key when keyHex is supplied.",
            props(&[
                db_path(),
                string_prop("cf", "DynamicJEPA CF name."),
                string_optional("keyHex", "Exact RocksDB key as hex from a previous inspect-cf row."),
                limit(),
                offset(),
            ]),
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

fn path_optional(name: &'static str, description: &'static str) -> (&'static str, Value) {
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

fn string_array_optional(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({
            "type": "array",
            "items": {"type": "string", "minLength": 1},
            "description": description
        }),
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

fn integer_optional(name: &'static str, description: &'static str) -> (&'static str, Value) {
    (
        name,
        json!({"type": "integer", "minimum": 0, "maximum": 100, "description": description}),
    )
}

fn integer_default(
    name: &'static str,
    minimum: u64,
    default: u64,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "integer", "minimum": minimum, "default": default, "description": description}),
    )
}

fn integer_array_default(
    name: &'static str,
    default: &[u64],
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({
            "type": "array",
            "items": {"type": "integer", "minimum": 0},
            "minItems": 1,
            "default": default,
            "description": description
        }),
    )
}

fn number_positive_default(
    name: &'static str,
    default: f64,
    description: &'static str,
) -> (&'static str, Value) {
    (
        name,
        json!({"type": "number", "exclusiveMinimum": 0.0, "default": default, "description": description}),
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
