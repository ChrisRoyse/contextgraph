//! CLI inspection tools for persisted UTL learner-state records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Args, Subcommand};
use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_bytes,
    sha256_json, update_m_trace, ComputationEnvelope, GoalCentroid, LearnerAuditEntry,
    LearnerConstellation, LearnerDeltaLog, LearnerFingerprint, LearnerGoalState, LearnerKSleep,
    LearnerModality, LearnerModalityCentroid, LearnerProfile, LearnerRetrievalLog,
    LearnerStateComponents, ObservationEnvelope, LEARNER_BASELINE_SELECTOR_REGULATED,
};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::{
    embed_calibration_sample, learner_embedder_specs, preflight_learner_assets,
    state_vector_from_outputs, synthetic_calibration_fixture, LearnerEmbedderSlot,
    CALIBRATION_DATASET_MANIFEST, UTL_PLANNED_TOTAL_EMBEDDERS,
};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand, Debug)]
pub enum UtlCommands {
    /// List the learner-state E15-E21 embedder registry and calibration datasets.
    ListEmbedders,
    /// Print the deterministic local calibration fixture summary.
    Fixture,
    /// Inspect required E15-E21 model files and real calibration data.
    PreflightAssets(UtlAssetPreflightArgs),
    /// Count rows in the learner-state source-of-truth column families.
    Count(UtlStorageArgs),
    /// Store a deterministic synthetic learner session and read it back.
    RecordSynthetic(UtlRecordSyntheticArgs),
    /// Fetch one persisted UTL delta log.
    GetDelta(UtlGetSessionArgs),
    /// Fetch one persisted M(t) trace.
    GetM(UtlGetTraceArgs),
}

#[derive(Args, Debug)]
pub struct UtlStorageArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,
}

#[derive(Args, Debug)]
pub struct UtlAssetPreflightArgs {
    /// Root containing E15-E21 model asset directories.
    #[arg(long, default_value = "models")]
    pub models_root: PathBuf,

    /// Root containing downloaded real calibration datasets.
    #[arg(long, default_value = "data/utl_calibration")]
    pub calibration_root: PathBuf,

    /// Return JSON with ready=false instead of failing when assets are missing.
    #[arg(long)]
    pub allow_missing: bool,
}

#[derive(Args, Debug)]
pub struct UtlRecordSyntheticArgs {
    /// Path to the RocksDB data directory. Created by RocksDB when missing.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Optional deterministic learner UUID.
    #[arg(long)]
    pub learner_id: Option<Uuid>,

    /// Optional deterministic trace UUID.
    #[arg(long)]
    pub trace_id: Option<Uuid>,

    /// Synthetic session timestamp.
    #[arg(long, default_value_t = 1_700_000_000)]
    pub session_ts: u64,

    /// Delete existing learner-state records before writing the synthetic rows.
    #[arg(long)]
    pub clear_existing: bool,
}

#[derive(Args, Debug)]
pub struct UtlGetSessionArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learner UUID.
    #[arg(long)]
    pub learner_id: Uuid,

    /// Session timestamp.
    #[arg(long)]
    pub session_ts: u64,
}

#[derive(Args, Debug)]
pub struct UtlGetTraceArgs {
    /// Path to the RocksDB data directory.
    #[arg(long, env = "CONTEXT_GRAPH_STORAGE_PATH")]
    pub storage: PathBuf,

    /// Learner UUID.
    #[arg(long)]
    pub learner_id: Uuid,

    /// Trace UUID.
    #[arg(long)]
    pub trace_id: Uuid,
}

pub async fn handle_utl_command(action: UtlCommands) -> i32 {
    match run(action).await {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            );
            0
        }
        Err(e) => {
            eprintln!("utl command FAILED: {:#}", e);
            1
        }
    }
}

async fn run(action: UtlCommands) -> Result<serde_json::Value> {
    match action {
        UtlCommands::ListEmbedders => Ok(json!({
            "source_of_truth": {
                "content_registry": "context_graph_embeddings::types::ModelId::production",
                "learner_registry": "context_graph_embeddings::learner::LearnerEmbedderSlot::all",
                "fixture_manifest": "crates/context-graph-embeddings/fixtures/utl_calibration"
            },
            "planned_total_embedders": UTL_PLANNED_TOTAL_EMBEDDERS,
            "content_embedder_count": ModelId::production().len(),
            "learner_embedder_count": learner_embedder_specs().len(),
            "legacy_content_variants_not_counted": [{
                "id": "Entity",
                "reason": "legacy E11 MiniLM compatibility variant; production E11 is Kepler"
            }],
            "content_embedders": content_embedder_specs_json(),
            "learner_embedders": learner_embedder_specs().iter().map(|spec| json!({
                "slot": spec.slot_number,
                "id": spec.slot.as_str(),
                "modality": spec.modality.as_str(),
                "model_name": spec.model_name,
                "model_path": spec.model_path,
                "output_dimension": spec.output_dimension,
                "scalar_heads": spec.scalar_heads,
            })).collect::<Vec<_>>(),
            "calibration_datasets": CALIBRATION_DATASET_MANIFEST.iter().map(|dataset| json!({
                "name": dataset.name,
                "modality": dataset.modality,
                "used_for": dataset.used_for,
                "access": dataset.access,
                "license": dataset.license,
            })).collect::<Vec<_>>(),
        })),
        UtlCommands::Fixture => {
            let fixture = synthetic_calibration_fixture();
            Ok(json!({
                "source_of_truth": {
                    "rust": "context_graph_embeddings::learner::synthetic_calibration_fixture",
                    "checked_in_manifest": "crates/context-graph-embeddings/fixtures/utl_calibration/phase0_synthetic_fixture.json",
                },
                "fixture_id": fixture.fixture_id,
                "sample_count": fixture.samples.len(),
                "samples": fixture.samples.iter().map(|sample| json!({
                    "sample_id": sample.sample_id,
                    "label": sample.label,
                    "input_slots": sample.inputs.keys().map(|slot| slot.as_str()).collect::<Vec<_>>(),
                    "expected_components": sample.expected_components,
                })).collect::<Vec<_>>()
            }))
        }
        UtlCommands::PreflightAssets(args) => {
            let report = preflight_learner_assets(&args.models_root, &args.calibration_root)?;
            let value = serde_json::to_value(&report)?;
            if !report.ready && !args.allow_missing {
                anyhow::bail!(
                    "UTL learner asset preflight failed:\n{}",
                    serde_json::to_string_pretty(&value)?
                );
            }
            Ok(value)
        }
        UtlCommands::Count(args) => {
            let store = open_store(&args.storage)?;
            Ok(json!({
                "source_of_truth": source_of_truth(),
                "counts": counts(&store).await?,
            }))
        }
        UtlCommands::RecordSynthetic(args) => {
            let store = open_store(&args.storage)?;
            let cleared = if args.clear_existing {
                store.clear_all_learner_state_records().await?
            } else {
                0
            };
            let before_counts = counts(&store).await?;
            let learner_id = args
                .learner_id
                .unwrap_or_else(|| Uuid::from_u128(0xaaaaaaaa_aaaa_4aaa_8aaa_aaaaaaaaaaaa));
            let trace_id = args
                .trace_id
                .unwrap_or_else(|| Uuid::from_u128(0xbbbbbbbb_bbbb_4bbb_8bbb_bbbbbbbbbbbb));
            let synthetic = synthetic_records(learner_id, trace_id, args.session_ts)?;

            store.store_learner_profile(&synthetic.profile).await?;
            store
                .store_learner_fingerprint(&synthetic.fingerprint)
                .await?;
            store.store_learner_delta_log(&synthetic.delta_log).await?;
            store.store_learner_m_trace(&synthetic.m_trace).await?;
            store
                .store_learner_constellation(&synthetic.constellation)
                .await?;
            store
                .store_learner_goal_state(&synthetic.goal_state)
                .await?;
            store
                .store_learner_retrieval_log(&synthetic.retrieval_log)
                .await?;
            store.store_learner_k_sleep(&synthetic.k_sleep).await?;
            store.store_goal_centroid(&synthetic.goal_centroid).await?;
            store
                .store_learner_audit_entry(&synthetic.audit_entry)
                .await?;

            let after_counts = counts(&store).await?;
            let readback_profile = store
                .get_learner_profile(learner_id)
                .await?
                .with_context(|| "synthetic learner profile missing after write")?;
            let readback_fingerprint = store
                .get_learner_fingerprint(learner_id, args.session_ts)
                .await?
                .with_context(|| "synthetic learner fingerprint missing after write")?;
            let readback_delta = store
                .get_learner_delta_log(learner_id, args.session_ts)
                .await?
                .with_context(|| "synthetic delta log missing after write")?;
            let readback_m = store
                .get_learner_m_trace(learner_id, trace_id)
                .await?
                .with_context(|| "synthetic M trace missing after write")?;
            let readback_constellation = store
                .get_learner_constellation(learner_id, LEARNER_BASELINE_SELECTOR_REGULATED)
                .await?
                .with_context(|| "synthetic learner constellation missing after write")?;
            let readback_goal_state = store
                .get_learner_goal_state(learner_id, synthetic.skill_id)
                .await?
                .with_context(|| "synthetic learner goal state missing after write")?;
            let readback_retrieval = store
                .get_learner_retrieval_log(learner_id, trace_id, args.session_ts)
                .await?
                .with_context(|| "synthetic retrieval log missing after write")?;
            let readback_k_sleep = store
                .get_learner_k_sleep(learner_id, args.session_ts)
                .await?
                .with_context(|| "synthetic k_sleep row missing after write")?;
            let readback_goal_centroid = store
                .get_goal_centroid(synthetic.skill_id, LearnerModality::AffectText)
                .await?
                .with_context(|| "synthetic goal centroid missing after write")?;

            Ok(json!({
                "source_of_truth": source_of_truth(),
                "cleared_before_write": cleared,
                "before_counts": before_counts,
                "after_counts": after_counts,
                "learner_id": learner_id.to_string(),
                "trace_id": trace_id.to_string(),
                "session_ts": args.session_ts,
                "readback": {
                    "profile": render_profile(&readback_profile),
                    "fingerprint": render_fingerprint(&readback_fingerprint),
                    "delta_log": render_delta_log(&readback_delta),
                    "m_trace": render_m_trace(&readback_m),
                    "learner_constellation": render_constellation(&readback_constellation),
                    "learner_goal_state": render_goal_state(&readback_goal_state),
                    "learner_retrieval_log": render_retrieval_log(&readback_retrieval),
                    "learner_k_sleep": render_k_sleep(&readback_k_sleep),
                    "goal_centroid": render_goal_centroid(&readback_goal_centroid),
                }
            }))
        }
        UtlCommands::GetDelta(args) => {
            let store = open_store(&args.storage)?;
            let delta = store
                .get_learner_delta_log(args.learner_id, args.session_ts)
                .await?;
            Ok(match delta {
                Some(delta) => json!({
                    "source_of_truth": source_of_truth(),
                    "found": true,
                    "delta_log": render_delta_log(&delta),
                }),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "found": false,
                    "learner_id": args.learner_id.to_string(),
                    "session_ts": args.session_ts,
                }),
            })
        }
        UtlCommands::GetM(args) => {
            let store = open_store(&args.storage)?;
            let trace = store
                .get_learner_m_trace(args.learner_id, args.trace_id)
                .await?;
            Ok(match trace {
                Some(trace) => json!({
                    "source_of_truth": source_of_truth(),
                    "found": true,
                    "m_trace": render_m_trace(&trace),
                }),
                None => json!({
                    "source_of_truth": source_of_truth(),
                    "found": false,
                    "learner_id": args.learner_id.to_string(),
                    "trace_id": args.trace_id.to_string(),
                }),
            })
        }
    }
}

fn open_store(storage: &PathBuf) -> Result<RocksDbTeleologicalStore> {
    RocksDbTeleologicalStore::open_with_config(storage, TeleologicalStoreConfig::default())
        .with_context(|| format!("opening RocksDbTeleologicalStore at {}", storage.display()))
}

fn content_embedder_specs_json() -> Vec<serde_json::Value> {
    ModelId::production()
        .iter()
        .map(|model| {
            json!({
                "slot": content_slot_number(*model),
                "id": model.as_str(),
                "model_name": content_model_name(*model),
                "model_repo": model.model_repo(),
                "model_path": model.directory_name(),
                "native_dimension": model.dimension(),
                "projected_dimension": model.projected_dimension(),
                "max_tokens": model.max_tokens(),
                "custom": model.is_custom(),
            })
        })
        .collect()
}

fn content_slot_number(model: ModelId) -> u8 {
    match model {
        ModelId::Semantic => 1,
        ModelId::TemporalRecent => 2,
        ModelId::TemporalPeriodic => 3,
        ModelId::TemporalPositional => 4,
        ModelId::Causal => 5,
        ModelId::Sparse => 6,
        ModelId::Code => 7,
        ModelId::Graph => 8,
        ModelId::Hdc => 9,
        ModelId::Contextual => 10,
        ModelId::Kepler => 11,
        ModelId::LateInteraction => 12,
        ModelId::Splade => 13,
        ModelId::BgeM3Dense => 14,
        ModelId::Entity => 11,
    }
}

fn content_model_name(model: ModelId) -> &'static str {
    match model {
        ModelId::Semantic => "intfloat/e5-large-v2",
        ModelId::TemporalRecent => "exponential recency basis",
        ModelId::TemporalPeriodic => "Fourier periodic basis",
        ModelId::TemporalPositional => "sinusoidal positional basis",
        ModelId::Causal => "nomic-ai/nomic-embed-text-v1.5",
        ModelId::Sparse => "naver/splade-cocondenser-ensembledistil",
        ModelId::Code => "Qodo/Qodo-Embed-1-1.5B",
        ModelId::Graph => "intfloat/e5-large-v2 graph/sentence embedding",
        ModelId::Hdc => "hyperdimensional computing encoder",
        ModelId::Contextual => "intfloat/e5-base-v2 contextual paraphrase",
        ModelId::Entity => "legacy sentence-transformers/all-MiniLM-L6-v2",
        ModelId::Kepler => "THU-KEG/KEPLER-Wiki5M-KE",
        ModelId::LateInteraction => "colbert-ir/colbertv2.0",
        ModelId::Splade => "prithivida/Splade_PP_en_v1",
        ModelId::BgeM3Dense => "BAAI/bge-m3 dense head",
    }
}

async fn counts(store: &RocksDbTeleologicalStore) -> Result<serde_json::Value> {
    Ok(json!({
        "learner_profile": store.count_learner_profiles().await?,
        "learner_constellations": store.count_learner_constellations().await?,
        "fingerprints_learner": store.count_learner_fingerprints().await?,
        "learner_m_per_trace": store.count_learner_m_traces().await?,
        "learner_state_history": store.count_learner_state_history().await?,
        "learner_goal_states": store.count_learner_goal_states().await?,
        "learner_retrieval_log": store.count_learner_retrieval_logs().await?,
        "learner_k_sleep": store.count_learner_k_sleep().await?,
        "goal_centroids": store.count_goal_centroids().await?,
        "learner_delta_log": store.count_learner_delta_logs().await?,
        "learner_audit": store.count_learner_audit_entries().await?,
    }))
}

struct SyntheticRecords {
    skill_id: Uuid,
    profile: LearnerProfile,
    fingerprint: LearnerFingerprint,
    delta_log: LearnerDeltaLog,
    m_trace: context_graph_core::learner::LearnerMTrace,
    constellation: LearnerConstellation,
    goal_state: LearnerGoalState,
    retrieval_log: LearnerRetrievalLog,
    k_sleep: LearnerKSleep,
    goal_centroid: GoalCentroid,
    audit_entry: LearnerAuditEntry,
}

fn synthetic_records(
    learner_id: Uuid,
    trace_id: Uuid,
    session_ts: u64,
) -> Result<SyntheticRecords> {
    let skill_id = Uuid::from_u128(0xffffeeee_dddd_4ccc_8bbb_aaaaaaaaaaaa);
    let modalities = LearnerEmbedderSlot::all()
        .iter()
        .map(|slot| slot.modality())
        .chain(std::iter::once(LearnerModality::SelfReport))
        .collect::<BTreeSet<_>>();
    let profile = LearnerProfile::new(
        learner_id,
        "synthetic-all-e15-e21-learner".into(),
        "consented-local-first".into(),
        modalities,
        Some(session_ts),
    )?;

    let fixture = synthetic_calibration_fixture();
    let sample = fixture
        .samples
        .iter()
        .find(|sample| sample.sample_id == "regulated-baseline")
        .with_context(|| "regulated synthetic calibration sample missing")?;
    let outputs = embed_calibration_sample(sample)?;
    let state_vector = state_vector_from_outputs(
        learner_id,
        session_ts,
        &outputs,
        BTreeMap::from([
            ("environment".into(), "quiet-dev-workstation".into()),
            ("fixture_id".into(), fixture.fixture_id.into()),
            ("sample_id".into(), sample.sample_id.into()),
            ("task".into(), "synthetic-all-e15-e21-utl-fsv".into()),
        ]),
    )?;
    let state_centroid = state_vector.clone();

    let mut observation_envelopes = Vec::with_capacity(outputs.len());
    let mut modality_embeddings = Vec::with_capacity(outputs.len());
    let mut modality_centroids = Vec::with_capacity(outputs.len());
    for output in &outputs {
        let observation_id = Uuid::from_u128(
            0xcccccccc_cccc_4ccc_8ccc_cccccccccc00 + u128::from(output.slot.slot_number()),
        );
        let raw_hash =
            sha256_bytes(format!("{}:{}", sample.sample_id, output.slot.as_str()).as_bytes());
        let envelope = ObservationEnvelope::new(
            observation_id,
            learner_id,
            session_ts,
            output.modality,
            "consented-local-first".into(),
            raw_hash,
            "synthetic-preprocess-v1".into(),
            output.embedder_version.clone(),
            "thresholds-default-pending-calibration-v1".into(),
            Vec::new(),
        )?;
        observation_envelopes.push(envelope);
        modality_embeddings.push(output.to_modality_embedding(observation_id));
        modality_centroids.push(LearnerModalityCentroid {
            modality: output.modality,
            vector: output.vector.clone(),
            scalar_mean: output.mean_scalar(),
            sample_count: 1,
        });
    }

    let components: LearnerStateComponents = state_vector.components.clone();
    let fingerprint = LearnerFingerprint {
        learner_id,
        session_ts,
        observation_envelopes,
        modality_embeddings,
        state_vector,
    };

    let delta_s = compute_delta_s_from_text(
        "ephemeral means temporary",
        "ephemeral means lasting only a short time",
        Some("temporary short lived concept"),
        0.2,
        Some(0.7),
    )?;
    let delta_c = compute_delta_c(
        &[0.62, 0.74, 0.86],
        components.hrv_coherence,
        0.82,
        0.05,
        None,
    )?;
    let delta_e = compute_delta_e(&components)?;
    let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None)?;
    let output_hash = sha256_json(&computation)?;
    let provenance = ComputationEnvelope::new(
        Uuid::from_u128(0xdddddddd_dddd_4ddd_8ddd_dddddddddddd),
        learner_id,
        session_ts,
        fingerprint
            .observation_envelopes
            .iter()
            .map(|envelope| envelope.observation_id)
            .collect(),
        "thresholds-default-pending-calibration-v1".into(),
        output_hash.clone(),
    )?;
    let delta_log = LearnerDeltaLog {
        learner_id,
        session_ts,
        computation,
        provenance,
    };
    let m_trace = update_m_trace(
        None,
        learner_id,
        trace_id,
        session_ts,
        &delta_log.computation,
        Some(true),
    )?;
    let constellation = LearnerConstellation {
        learner_id,
        selector_kind: LEARNER_BASELINE_SELECTOR_REGULATED,
        label: "regulated-baseline-synthetic".into(),
        sample_count: 1,
        session_ts_start: session_ts,
        session_ts_end: session_ts,
        modality_centroids,
        state_centroid: state_centroid.clone(),
        created_at: Utc::now(),
    };
    constellation.validate()?;
    let goal_state = LearnerGoalState {
        learner_id,
        skill_id,
        state_vector: state_centroid.clone(),
    };
    let retrieval_log = LearnerRetrievalLog {
        learner_id,
        trace_id,
        ts: session_ts,
        correct: true,
        score: 0.86,
        state_at_retrieval: state_centroid.clone(),
    };
    let k_sleep = LearnerKSleep {
        learner_id,
        session_ts,
        k: components.k_sleep,
        slow_wave_minutes: 80,
    };
    let text_output = outputs
        .iter()
        .find(|output| output.modality == LearnerModality::AffectText)
        .with_context(|| "synthetic affect-text output missing")?;
    let goal_centroid = GoalCentroid {
        skill_id,
        modality: LearnerModality::AffectText,
        vector: text_output.vector.clone(),
    };
    let audit_entry = LearnerAuditEntry {
        audit_id: Uuid::from_u128(0xeeeeeeee_eeee_4eee_8eee_eeeeeeeeeeee),
        learner_id,
        ts: session_ts,
        action: "record_synthetic_utl_session".into(),
        target_cf: "learner_delta_log".into(),
        result_sha256: output_hash,
        parent_audit_id: None,
    };

    Ok(SyntheticRecords {
        skill_id,
        profile,
        fingerprint,
        delta_log,
        m_trace,
        constellation,
        goal_state,
        retrieval_log,
        k_sleep,
        goal_centroid,
        audit_entry,
    })
}

fn source_of_truth() -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "format": "version_byte + bincode",
        "column_families": [
            "learner_profile",
            "learner_constellations",
            "fingerprints_learner",
            "learner_m_per_trace",
            "learner_state_history",
            "learner_goal_states",
            "learner_retrieval_log",
            "learner_k_sleep",
            "goal_centroids",
            "learner_delta_log",
            "learner_audit"
        ]
    })
}

fn render_profile(profile: &LearnerProfile) -> serde_json::Value {
    json!({
        "learner_id": profile.learner_id.to_string(),
        "handle": profile.handle,
        "consent_state": profile.consent_state,
        "modalities_enabled": profile.modalities_enabled.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
        "calibration_session_ts": profile.calibration_session_ts,
    })
}

fn render_fingerprint(fingerprint: &LearnerFingerprint) -> serde_json::Value {
    json!({
        "learner_id": fingerprint.learner_id.to_string(),
        "session_ts": fingerprint.session_ts,
        "observation_envelopes": fingerprint.observation_envelopes.len(),
        "modality_embeddings": fingerprint.modality_embeddings.iter().map(|m| json!({
            "modality": m.modality.as_str(),
            "vector_len": m.vector.len(),
            "scalar": m.scalar,
            "source_observation_id": m.source_observation_id.to_string(),
        })).collect::<Vec<_>>(),
        "state_vector_len": fingerprint.state_vector.values.len(),
        "components": {
            "plasticity_window": fingerprint.state_vector.components.plasticity_window,
            "hrv_coherence": fingerprint.state_vector.components.hrv_coherence,
            "valence": fingerprint.state_vector.components.valence,
            "arousal": fingerprint.state_vector.components.arousal,
            "stress_floor": fingerprint.state_vector.components.stress_floor,
            "k_sleep": fingerprint.state_vector.components.k_sleep,
        }
    })
}

fn render_delta_log(log: &LearnerDeltaLog) -> serde_json::Value {
    json!({
        "learner_id": log.learner_id.to_string(),
        "session_ts": log.session_ts,
        "l": log.computation.l,
        "diagnostic_state": log.computation.diagnostic_state.as_str(),
        "delta_s": log.computation.delta_s,
        "delta_c": log.computation.delta_c,
        "delta_e": log.computation.delta_e,
        "provenance": {
            "computation_id": log.provenance.computation_id.to_string(),
            "parent_observation_ids": log.provenance.parent_observation_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "threshold_version": log.provenance.threshold_version,
            "output_sha256": log.provenance.output_sha256,
        }
    })
}

fn render_m_trace(trace: &context_graph_core::learner::LearnerMTrace) -> serde_json::Value {
    json!({
        "learner_id": trace.learner_id.to_string(),
        "trace_id": trace.trace_id.to_string(),
        "m_value": trace.m_value,
        "last_update_ts": trace.last_update_ts,
        "decay_rate": trace.decay_rate,
        "num_retrievals": trace.num_retrievals,
        "next_review_ts": trace.next_review_ts,
    })
}

fn render_constellation(constellation: &LearnerConstellation) -> serde_json::Value {
    json!({
        "learner_id": constellation.learner_id.to_string(),
        "selector_kind": constellation.selector_kind,
        "label": constellation.label,
        "sample_count": constellation.sample_count,
        "session_ts_start": constellation.session_ts_start,
        "session_ts_end": constellation.session_ts_end,
        "modality_centroids": constellation.modality_centroids.iter().map(|centroid| json!({
            "modality": centroid.modality.as_str(),
            "vector_len": centroid.vector.len(),
            "scalar_mean": centroid.scalar_mean,
            "sample_count": centroid.sample_count,
        })).collect::<Vec<_>>(),
        "state_centroid_len": constellation.state_centroid.values.len(),
    })
}

fn render_goal_state(goal: &LearnerGoalState) -> serde_json::Value {
    json!({
        "learner_id": goal.learner_id.to_string(),
        "skill_id": goal.skill_id.to_string(),
        "state_vector_len": goal.state_vector.values.len(),
        "components": {
            "plasticity_window": goal.state_vector.components.plasticity_window,
            "hrv_coherence": goal.state_vector.components.hrv_coherence,
            "valence": goal.state_vector.components.valence,
            "arousal": goal.state_vector.components.arousal,
            "stress_floor": goal.state_vector.components.stress_floor,
            "k_sleep": goal.state_vector.components.k_sleep,
        }
    })
}

fn render_retrieval_log(log: &LearnerRetrievalLog) -> serde_json::Value {
    json!({
        "learner_id": log.learner_id.to_string(),
        "trace_id": log.trace_id.to_string(),
        "ts": log.ts,
        "correct": log.correct,
        "score": log.score,
        "state_vector_len": log.state_at_retrieval.values.len(),
    })
}

fn render_k_sleep(value: &LearnerKSleep) -> serde_json::Value {
    json!({
        "learner_id": value.learner_id.to_string(),
        "session_ts": value.session_ts,
        "k": value.k,
        "slow_wave_minutes": value.slow_wave_minutes,
    })
}

fn render_goal_centroid(centroid: &GoalCentroid) -> serde_json::Value {
    json!({
        "skill_id": centroid.skill_id.to_string(),
        "modality": centroid.modality.as_str(),
        "vector_len": centroid.vector.len(),
    })
}
