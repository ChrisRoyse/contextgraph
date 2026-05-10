use context_graph_mejepa_embedders::{
    digest_manifest_for_embedder, verify_all_registration_digests, EmbedderId, EmbedderKind,
    EmbedderRegistration, ModelsConfig,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

type ExampleResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Copy)]
struct SlotSpec {
    id: EmbedderId,
    name: &'static str,
    path: &'static str,
    repo: Option<&'static str>,
    files: &'static [&'static str],
    identity: Option<ModelIdentity>,
}

#[derive(Debug, Clone, Copy)]
struct ModelIdentity {
    config_file: &'static str,
    model_type: Option<&'static str>,
    hidden_size: Option<u64>,
    numeric_fields: &'static [(&'static str, u64)],
    null_fields: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct RegistrationEvidence {
    embedder: EmbedderId,
    name: String,
    path: String,
    repo: Option<String>,
    file_count: usize,
    byte_count: u64,
    manifest_sha256: String,
}

fn main() -> ExampleResult<()> {
    let models_root = std::env::var("CONTEXTGRAPH_MODELS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/mnt/d/cabdru/contextgraph/models"));
    let output_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| models_root.join("mejepa_models_config.toml"));
    let evidence_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG_EVIDENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(
                "/tmp/contextgraph-mejepa-models-config-materialization/models-config-materialization.json",
            )
        });

    let before = json!({
        "models_root": models_root.display().to_string(),
        "output_path": output_path.display().to_string(),
        "output_exists": output_path.exists(),
    });

    let (config, registrations) = build_config(&models_root)?;
    config.validate()?;
    let config_text = toml::to_string_pretty(&config)?;
    write_checked(&output_path, config_text.as_bytes())?;

    let readback = ModelsConfig::load(&output_path)?;
    let digest_readback = verify_all_registration_digests(&readback)?;
    require(
        "all active registrations verified after disk readback",
        digest_readback.len() == EmbedderId::required_registrations().len(),
    )?;

    let evidence = json!({
        "source_of_truth": {
            "models_config": output_path.display().to_string(),
            "models_root": models_root.display().to_string(),
        },
        "before_state": before,
        "after_state": {
            "config_exists": output_path.exists(),
            "registration_count": readback.embedders.len(),
            "digest_verified_count": digest_readback.len(),
            "retired": ["e5", "e11"],
            "registrations": registrations,
        },
        "passes": true,
    });
    let evidence_text = serde_json::to_string_pretty(&evidence)?;
    write_checked(&evidence_path, evidence_text.as_bytes())?;
    let evidence_readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence readback points at materialized config",
        evidence_readback["source_of_truth"]["models_config"] == output_path.display().to_string(),
    )?;

    println!(
        "Phase 1b models_config materialized: {}",
        output_path.display()
    );
    println!("Evidence: {}", evidence_path.display());
    println!("{evidence_text}");
    Ok(())
}

fn build_config(models_root: &Path) -> ExampleResult<(ModelsConfig, Vec<RegistrationEvidence>)> {
    let mut embedders = BTreeMap::new();
    let mut evidence = Vec::new();

    for spec in slot_specs() {
        let (registration, registration_evidence) = registration_from_spec(models_root, spec)?;
        embedders.insert(spec.id.slug().to_string(), registration);
        evidence.push(registration_evidence);
    }

    Ok((
        ModelsConfig {
            schema_version: 1,
            embedders,
        },
        evidence,
    ))
}

fn registration_from_spec(
    models_root: &Path,
    spec: SlotSpec,
) -> ExampleResult<(EmbedderRegistration, RegistrationEvidence)> {
    let kind = spec.id.kind();
    if kind == EmbedderKind::ContentDeterministic {
        let reg = EmbedderRegistration {
            embedder: spec.id,
            name: spec.name.to_string(),
            kind,
            path: String::new(),
            repo: None,
            dimension: spec.id.dimension(),
            weight_files: Vec::new(),
            manifest_sha256: String::new(),
        };
        reg.validate()?;
        return Ok((
            reg,
            RegistrationEvidence {
                embedder: spec.id,
                name: spec.name.to_string(),
                path: String::new(),
                repo: None,
                file_count: 0,
                byte_count: 0,
                manifest_sha256: String::new(),
            },
        ));
    }

    let base_dir = models_root.join(spec.path);
    if let Some(identity) = spec.identity {
        verify_model_identity(&base_dir, identity)?;
    }
    let files: Vec<String> = spec.files.iter().map(|file| (*file).to_string()).collect();
    let (manifest_sha256, file_digests) = digest_manifest_for_embedder(spec.id, &base_dir, &files)?;
    let byte_count = file_digests.iter().map(|file| file.size_bytes).sum();
    let reg = EmbedderRegistration {
        embedder: spec.id,
        name: spec.name.to_string(),
        kind,
        path: base_dir.display().to_string(),
        repo: spec.repo.map(str::to_string),
        dimension: spec.id.dimension(),
        weight_files: files,
        manifest_sha256: manifest_sha256.clone(),
    };
    reg.validate()?;
    Ok((
        reg,
        RegistrationEvidence {
            embedder: spec.id,
            name: spec.name.to_string(),
            path: base_dir.display().to_string(),
            repo: spec.repo.map(str::to_string),
            file_count: file_digests.len(),
            byte_count,
            manifest_sha256,
        },
    ))
}

fn verify_model_identity(base_dir: &Path, identity: ModelIdentity) -> ExampleResult<()> {
    let path = base_dir.join(identity.config_file);
    let value: Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    if let Some(expected) = identity.model_type {
        let model_type = value
            .get("model_type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{} missing model_type", path.display()))?;
        if model_type != expected {
            return Err(format!(
                "{} model_type mismatch: expected {expected}, got {model_type}",
                path.display()
            )
            .into());
        }
    }
    if let Some(expected) = identity.hidden_size {
        let actual = value
            .get("hidden_size")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{} missing hidden_size", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{} hidden_size mismatch: expected {expected}, got {actual}",
                path.display()
            )
            .into());
        }
    }
    for (field, expected) in identity.numeric_fields {
        let actual = value
            .get(*field)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{} missing numeric field {field}", path.display()))?;
        if actual != *expected {
            return Err(format!(
                "{} numeric field {field} mismatch: expected {expected}, got {actual}",
                path.display()
            )
            .into());
        }
    }
    for field in identity.null_fields {
        let actual = value
            .get(*field)
            .ok_or_else(|| format!("{} missing nullable field {field}", path.display()))?;
        if !actual.is_null() {
            return Err(format!(
                "{} nullable field {field} mismatch: expected null, got {actual}",
                path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn write_checked(path: &Path, bytes: &[u8]) -> ExampleResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("byte readback", fs::read(path)? == bytes)?;
    Ok(())
}

fn require(label: &str, condition: bool) -> ExampleResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("models_config materialization invariant failed: {label}").into())
    }
}

fn transformer_identity(model_type: &'static str, hidden_size: Option<u64>) -> ModelIdentity {
    ModelIdentity {
        config_file: "config.json",
        model_type: Some(model_type),
        hidden_size,
        numeric_fields: &[],
        null_fields: &[],
    }
}

fn numeric_identity(
    numeric_fields: &'static [(&'static str, u64)],
    null_fields: &'static [&'static str],
) -> ModelIdentity {
    ModelIdentity {
        config_file: "config.json",
        model_type: None,
        hidden_size: None,
        numeric_fields,
        null_fields,
    }
}

fn slot_specs() -> Vec<SlotSpec> {
    vec![
        pretrained(
            EmbedderId::E1,
            "semantic",
            "semantic",
            "intfloat/e5-large-v2",
            &["model.safetensors"],
            Some(transformer_identity("bert", Some(1024))),
        ),
        deterministic(EmbedderId::E2, "temporal_recent"),
        deterministic(EmbedderId::E3, "temporal_periodic"),
        deterministic(EmbedderId::E4, "temporal_positional"),
        pretrained(
            EmbedderId::E6,
            "sparse",
            "sparse",
            "naver/splade-cocondenser-ensembledistil",
            &["model.safetensors", "sparse_projection.safetensors"],
            Some(transformer_identity("bert", Some(768))),
        ),
        pretrained(
            EmbedderId::E7,
            "code",
            "code-1536",
            "Qodo/Qodo-Embed-1-1.5B",
            &[
                "model-00001-of-00002.safetensors",
                "model-00002-of-00002.safetensors",
            ],
            Some(transformer_identity("qwen2", Some(1536))),
        ),
        pretrained(
            EmbedderId::E8,
            "graph",
            "semantic",
            "intfloat/e5-large-v2",
            &["model.safetensors"],
            Some(transformer_identity("bert", Some(1024))),
        ),
        deterministic(EmbedderId::E9, "hdc"),
        pretrained(
            EmbedderId::E10,
            "contextual",
            "contextual-e5-base",
            "intfloat/e5-base-v2",
            &["model.safetensors"],
            Some(transformer_identity("bert", Some(768))),
        ),
        pretrained(
            EmbedderId::E12,
            "late_interaction",
            "late-interaction",
            "colbert-ir/colbertv2.0",
            &["model.safetensors"],
            Some(transformer_identity("bert", Some(768))),
        ),
        pretrained(
            EmbedderId::E13,
            "splade_v3",
            "splade-v3",
            "prithivida/Splade_PP_en_v1",
            &["model.safetensors", "sparse_projection.safetensors"],
            Some(transformer_identity("bert", Some(768))),
        ),
        pretrained(
            EmbedderId::E14,
            "bge_m3_dense",
            "bge-m3-dense",
            "BAAI/bge-m3",
            &["pytorch_model.bin"],
            Some(transformer_identity("xlm-roberta", Some(1024))),
        ),
        pretrained(
            EmbedderId::E15,
            "affect_speech",
            "learner-state/affect-speech",
            "audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim",
            &["model.safetensors", "preprocessor_config.json"],
            Some(transformer_identity("wav2vec2", Some(1024))),
        ),
        pretrained(
            EmbedderId::E16,
            "affect_face",
            "learner-state/affect-face",
            "github:CMU-MultiComp-Lab/OpenFace-3.0 + hf:nutPace/openface_weights",
            &[
                "OpenFace/interface.py",
                "OpenFace/model/MLT.py",
                "OpenFace/weights/Alignment_RetinaFace.pth",
                "OpenFace/weights/Landmark_98.pkl",
                "OpenFace/weights/MTL_backbone.pth",
            ],
            None,
        ),
        pretrained(
            EmbedderId::E17,
            "affect_text",
            "learner-state/affect-text",
            "sentence-transformers/all-MiniLM-L6-v2",
            &["model.safetensors", "tokenizer.json"],
            Some(transformer_identity("bert", Some(384))),
        ),
        pretrained(
            EmbedderId::E18,
            "ppg",
            "learner-state/ppg",
            "zenodo:10.5281/zenodo.13983110 + github:Nokia-Bell-Labs/papagei-foundation-model",
            &["config.json", "papagei_s.pt"],
            None,
        ),
        pretrained(
            EmbedderId::E19,
            "eda",
            "learner-state/eda",
            "official:WESAD University of Siegen + local real-data stress-head training",
            &["config.json", "wesad_cnn.safetensors"],
            None,
        ),
        pretrained(
            EmbedderId::E20,
            "eeg",
            "learner-state/eeg",
            "braindecode/labram-pretrained",
            &["model.safetensors"],
            Some(numeric_identity(
                &[("n_outputs", 0), ("n_times", 3000)],
                &["n_chans", "sfreq"],
            )),
        ),
        pretrained(
            EmbedderId::E21,
            "eeg_artifact_robust",
            "learner-state/eeg-robust",
            "braindecode/eegpt-pretrained",
            &["model.safetensors"],
            Some(numeric_identity(
                &[
                    ("n_outputs", 1),
                    ("n_chans", 62),
                    ("n_times", 1000),
                    ("sfreq", 250),
                ],
                &[],
            )),
        ),
    ]
}

fn pretrained(
    id: EmbedderId,
    name: &'static str,
    path: &'static str,
    repo: &'static str,
    files: &'static [&'static str],
    identity: Option<ModelIdentity>,
) -> SlotSpec {
    SlotSpec {
        id,
        name,
        path,
        repo: Some(repo),
        files,
        identity,
    }
}

fn deterministic(id: EmbedderId, name: &'static str) -> SlotSpec {
    SlotSpec {
        id,
        name,
        path: "",
        repo: None,
        files: &[],
        identity: None,
    }
}
