use crate::config::EmbedderRegistration;
use crate::digest::verify_registration_digest;
use crate::embedder_id::{EmbedderId, EmbedderKind};
use crate::error::{EmbedError, EmbedResult};
use crate::types::{EmbedderInput, EmbedderOutput};
use async_trait::async_trait;
use context_graph_core::memory::ast::Language as AstLanguage;
use context_graph_embeddings::models::custom::HdcModel;
use context_graph_embeddings::models::pretrained::BgeM3DenseModel;
use context_graph_embeddings::models::{
    CodeModel, ContextualModel, GraphModel, LateInteractionModel, SemanticModel, SparseModel,
    TemporalPeriodicModel, TemporalPositionalModel, TemporalRecentModel,
};
use context_graph_embeddings::traits::{EmbeddingModel, SingleModelConfig};
use context_graph_embeddings::types::{ModelId, ModelInput};
use std::path::{Path, PathBuf};

pub const SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS: &[EmbedderId] = &[
    EmbedderId::E1,
    EmbedderId::E6,
    EmbedderId::E7,
    EmbedderId::E8,
    EmbedderId::E10,
    EmbedderId::E12,
    EmbedderId::E13,
    EmbedderId::E14,
];

pub const SUPPORTED_ALGORITHMIC_FORWARD_EMBEDDERS: &[EmbedderId] = &[
    EmbedderId::E2,
    EmbedderId::E3,
    EmbedderId::E4,
    EmbedderId::E9,
];

pub const SUPPORTED_FORWARD_EMBEDDERS: &[EmbedderId] = &[
    EmbedderId::E1,
    EmbedderId::E2,
    EmbedderId::E3,
    EmbedderId::E4,
    EmbedderId::E6,
    EmbedderId::E7,
    EmbedderId::E8,
    EmbedderId::E9,
    EmbedderId::E10,
    EmbedderId::E12,
    EmbedderId::E13,
    EmbedderId::E14,
];

#[async_trait]
pub trait EmbedderForward: Send + Sync {
    fn embedder(&self) -> EmbedderId;
    fn model_version(&self) -> &str;
    fn artifact_root(&self) -> &Path;
    async fn forward(&self, input: &EmbedderInput) -> EmbedResult<EmbedderOutput>;
    fn supports_true_batch(&self) -> bool {
        false
    }
    async fn forward_true_batch(
        &self,
        inputs: &[EmbedderInput],
    ) -> EmbedResult<Vec<EmbedderOutput>> {
        if inputs.is_empty() {
            return Err(EmbedError::true_batch_empty(self.embedder()));
        }
        Err(EmbedError::true_batch_unsupported(
            self.embedder(),
            inputs.len(),
            "this embedder wrapper has no native true-batch forward implementation",
        ))
    }
}

pub struct PretrainedEmbedderForward {
    embedder: EmbedderId,
    model_version: String,
    model_root: PathBuf,
    model: Box<dyn EmbeddingModel>,
}

impl PretrainedEmbedderForward {
    pub async fn load(registration: &EmbedderRegistration) -> EmbedResult<Self> {
        registration.validate()?;
        if registration.kind != EmbedderKind::ContentPretrained {
            return Err(EmbedError::forward(
                registration.embedder,
                format!(
                    "slot kind {:?} is not a neural content pretrained embedder",
                    registration.kind
                ),
                "route deterministic and learner-state slots through their dedicated loaders; neural forward wrappers are only for active content pretrained slots",
            ));
        }
        if !SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS.contains(&registration.embedder) {
            return Err(EmbedError::forward(
                registration.embedder,
                "no active neural forward wrapper is registered for this slot",
                "wire the slot to a real context-graph-embeddings model before requesting forward inference",
            ));
        }
        verify_runtime_artifacts(registration)?;
        let files = verify_registration_digest(registration)?;
        if files.is_empty() {
            return Err(EmbedError::forward(
                registration.embedder,
                "digest verification returned zero files",
                "pin every model artifact needed for the forward pass in models_config.toml",
            ));
        }
        let model_root = PathBuf::from(&registration.path);
        let model = construct_model(registration.embedder, &model_root)?;
        model.load().await.map_err(|err| {
            EmbedError::forward(
                registration.embedder,
                format!("model load failed at {}: {err}", model_root.display()),
                "inspect the registered tokenizer/config/safetensors files and CUDA driver state; no CPU or hash fallback is used",
            )
        })?;
        if !model.is_initialized() {
            return Err(EmbedError::forward(
                registration.embedder,
                "model load returned without initialized state",
                "fix the concrete embedding model load implementation so initialized state is observable before forward inference",
            ));
        }
        Ok(Self {
            embedder: registration.embedder,
            model_version: registration.manifest_sha256.clone(),
            model_root,
            model,
        })
    }
}

pub struct AlgorithmicEmbedderForward {
    embedder: EmbedderId,
    model_version: String,
    artifact_root: PathBuf,
    model: Box<dyn EmbeddingModel>,
}

impl AlgorithmicEmbedderForward {
    pub fn load(embedder: EmbedderId) -> EmbedResult<Self> {
        if !SUPPORTED_ALGORITHMIC_FORWARD_EMBEDDERS.contains(&embedder) {
            return Err(EmbedError::forward(
                embedder,
                "slot is not an active algorithmic content embedder",
                "use AlgorithmicEmbedderForward only for E2/E3/E4/E9; pretrained and learner-state slots have separate loaders",
            ));
        }
        let model = construct_algorithmic_model(embedder)?;
        if !model.is_initialized() {
            return Err(EmbedError::forward(
                embedder,
                "algorithmic model constructed without initialized state",
                "fix the concrete custom embedding model so mathematical embedders are immediately ready for inference",
            ));
        }
        let model_id = embedder_model_id(embedder)?;
        Ok(Self {
            embedder,
            model_version: format!(
                "context-graph-embeddings:{}:{}",
                env!("CARGO_PKG_VERSION"),
                model_id.as_str()
            ),
            artifact_root: PathBuf::from(format!("algorithmic/{}", model_id.as_str())),
            model,
        })
    }
}

#[async_trait]
impl EmbedderForward for AlgorithmicEmbedderForward {
    fn embedder(&self) -> EmbedderId {
        self.embedder
    }

    fn model_version(&self) -> &str {
        &self.model_version
    }

    fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    async fn forward(&self, input: &EmbedderInput) -> EmbedResult<EmbedderOutput> {
        input.validate()?;
        if input.embedder != self.embedder {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "input embedder {} does not match loaded wrapper {}",
                    input.embedder, self.embedder
                ),
                "load and call the wrapper for the same EmbedderId; cross-slot forwarding is rejected",
            ));
        }
        let model_input = model_input_for_embedder(self.embedder, input)?;
        let embedding = self.model.embed(&model_input).await.map_err(|err| {
            EmbedError::forward(
                self.embedder,
                format!("algorithmic forward failed for source_id={}: {err}", input.source_id),
                "inspect the algorithmic embedder input contract; no zero vector or current-time fallback is permitted",
            )
        })?;
        let model_id = embedder_model_id(self.embedder)?;
        if embedding.model_id != model_id {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "model id mismatch: wrapper expected {:?}, concrete model returned {:?}",
                    model_id, embedding.model_id
                ),
                "fix the EmbedderId to ModelId mapping before persisting outputs",
            ));
        }
        let output = EmbedderOutput {
            embedder: self.embedder,
            source_id: input.source_id.clone(),
            vector: embedding.vector,
            model_version: self.model_version.clone(),
            precision_class: "algorithmic_real_forward".to_string(),
        };
        output.validate()?;
        Ok(output)
    }
}

#[async_trait]
impl EmbedderForward for PretrainedEmbedderForward {
    fn embedder(&self) -> EmbedderId {
        self.embedder
    }

    fn model_version(&self) -> &str {
        &self.model_version
    }

    fn artifact_root(&self) -> &Path {
        &self.model_root
    }

    fn supports_true_batch(&self) -> bool {
        self.model.supports_true_batch()
    }

    async fn forward(&self, input: &EmbedderInput) -> EmbedResult<EmbedderOutput> {
        input.validate()?;
        if input.embedder != self.embedder {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "input embedder {} does not match loaded wrapper {}",
                    input.embedder, self.embedder
                ),
                "load and call the wrapper for the same EmbedderId; cross-slot forwarding is rejected",
            ));
        }
        let model_input = model_input_for_embedder(self.embedder, input)?;
        let embedding = self.model.embed(&model_input).await.map_err(|err| {
            EmbedError::forward(
                self.embedder,
                format!("forward pass failed for source_id={}: {err}", input.source_id),
                "inspect tokenizer/model compatibility and input modality; no deterministic vector fallback is permitted",
            )
        })?;
        let model_id = embedder_model_id(self.embedder)?;
        if embedding.model_id != model_id {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "model id mismatch: wrapper expected {:?}, concrete model returned {:?}",
                    model_id, embedding.model_id
                ),
                "fix the EmbedderId to ModelId mapping before persisting outputs",
            ));
        }
        if embedding.vector.len() != self.embedder.dimension() {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "forward vector dimension {}, expected {}",
                    embedding.vector.len(),
                    self.embedder.dimension()
                ),
                "fix model projection/output dimensions before using this embedder slot",
            ));
        }
        let output = EmbedderOutput {
            embedder: self.embedder,
            source_id: input.source_id.clone(),
            vector: embedding.vector,
            model_version: self.model_version.clone(),
            precision_class: "candle_real_forward".to_string(),
        };
        output.validate()?;
        Ok(output)
    }

    async fn forward_true_batch(
        &self,
        inputs: &[EmbedderInput],
    ) -> EmbedResult<Vec<EmbedderOutput>> {
        if inputs.is_empty() {
            return Err(EmbedError::true_batch_empty(self.embedder));
        }
        let mut model_inputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            input.validate()?;
            if input.embedder != self.embedder {
                return Err(EmbedError::forward(
                    self.embedder,
                    format!(
                        "input embedder {} does not match loaded wrapper {}",
                        input.embedder, self.embedder
                    ),
                    "load and call the wrapper for the same EmbedderId; cross-slot batch forwarding is rejected",
                ));
            }
            model_inputs.push(model_input_for_embedder(self.embedder, input)?);
        }

        let embeddings = self
            .model
            .embed_true_batch(&model_inputs)
            .await
            .map_err(|err| match err {
                context_graph_embeddings::EmbeddingError::TrueBatchEmpty { .. } => {
                    EmbedError::true_batch_empty(self.embedder)
                }
                context_graph_embeddings::EmbeddingError::TrueBatchUnsupported { .. } => {
                    EmbedError::true_batch_unsupported(self.embedder, inputs.len(), err.to_string())
                }
                other => EmbedError::forward(
                    self.embedder,
                    format!("true-batch forward pass failed: {other}"),
                    "inspect tokenizer/model compatibility and input modality; no deterministic vector fallback is permitted",
                ),
            })?;
        if embeddings.len() != inputs.len() {
            return Err(EmbedError::forward(
                self.embedder,
                format!(
                    "true-batch output count {}, expected {}",
                    embeddings.len(),
                    inputs.len()
                ),
                "fix concrete model batch row alignment before persisting outputs",
            ));
        }

        let model_id = embedder_model_id(self.embedder)?;
        let mut outputs = Vec::with_capacity(inputs.len());
        for (input, embedding) in inputs.iter().zip(embeddings) {
            if embedding.model_id != model_id {
                return Err(EmbedError::forward(
                    self.embedder,
                    format!(
                        "model id mismatch: wrapper expected {:?}, concrete model returned {:?}",
                        model_id, embedding.model_id
                    ),
                    "fix the EmbedderId to ModelId mapping before persisting outputs",
                ));
            }
            if embedding.vector.len() != self.embedder.dimension() {
                return Err(EmbedError::forward(
                    self.embedder,
                    format!(
                        "true-batch vector dimension {}, expected {}",
                        embedding.vector.len(),
                        self.embedder.dimension()
                    ),
                    "fix model projection/output dimensions before using this embedder slot",
                ));
            }
            let output = EmbedderOutput {
                embedder: self.embedder,
                source_id: input.source_id.clone(),
                vector: embedding.vector,
                model_version: self.model_version.clone(),
                precision_class: "candle_real_true_batch_forward".to_string(),
            };
            output.validate()?;
            outputs.push(output);
        }
        Ok(outputs)
    }
}

pub fn embedder_model_id(embedder: EmbedderId) -> EmbedResult<ModelId> {
    match embedder {
        EmbedderId::E1 => Ok(ModelId::Semantic),
        EmbedderId::E2 => Ok(ModelId::TemporalRecent),
        EmbedderId::E3 => Ok(ModelId::TemporalPeriodic),
        EmbedderId::E4 => Ok(ModelId::TemporalPositional),
        EmbedderId::E6 => Ok(ModelId::Sparse),
        EmbedderId::E7 => Ok(ModelId::Code),
        EmbedderId::E8 => Ok(ModelId::Graph),
        EmbedderId::E9 => Ok(ModelId::Hdc),
        EmbedderId::E10 => Ok(ModelId::Contextual),
        EmbedderId::E12 => Ok(ModelId::LateInteraction),
        EmbedderId::E13 => Ok(ModelId::Splade),
        EmbedderId::E14 => Ok(ModelId::BgeM3Dense),
        other => Err(EmbedError::forward(
            other,
            "slot is not backed by an active context-graph-embeddings pretrained model",
            "use SUPPORTED_FORWARD_EMBEDDERS and do not forward retired/deterministic/learner-state slots through neural wrappers",
        )),
    }
}

fn construct_model(embedder: EmbedderId, root: &Path) -> EmbedResult<Box<dyn EmbeddingModel>> {
    let config = SingleModelConfig::cuda_fp16();
    let model: Box<dyn EmbeddingModel> = match embedder {
        EmbedderId::E1 => Box::new(SemanticModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("SemanticModel construction failed: {err}"),
                "fix E1 model config before loading",
            )
        })?),
        EmbedderId::E6 => Box::new(SparseModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("SparseModel construction failed: {err}"),
                "fix E6 model config before loading",
            )
        })?),
        EmbedderId::E7 => Box::new(CodeModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("CodeModel construction failed: {err}"),
                "fix E7 model config before loading",
            )
        })?),
        EmbedderId::E8 => Box::new(GraphModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("GraphModel construction failed: {err}"),
                "fix E8 model config before loading",
            )
        })?),
        EmbedderId::E10 => Box::new(ContextualModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("ContextualModel construction failed: {err}"),
                "fix E10 model config before loading",
            )
        })?),
        EmbedderId::E12 => Box::new(LateInteractionModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("LateInteractionModel construction failed: {err}"),
                "fix E12 model config before loading",
            )
        })?),
        EmbedderId::E13 => Box::new(SparseModel::new_splade(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("SpladeModel construction failed: {err}"),
                "fix E13 model config before loading",
            )
        })?),
        EmbedderId::E14 => Box::new(BgeM3DenseModel::new(root, config).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("BgeM3DenseModel construction failed: {err}"),
                "fix E14 model config before loading",
            )
        })?),
        other => {
            return Err(EmbedError::forward(
                other,
                "slot is not backed by an active pretrained context-graph-embeddings model",
                "construct only E1/E6/E7/E8/E10/E12/E13/E14 through the pretrained loader",
            ))
        }
    };
    Ok(model)
}

fn construct_algorithmic_model(embedder: EmbedderId) -> EmbedResult<Box<dyn EmbeddingModel>> {
    let model: Box<dyn EmbeddingModel> = match embedder {
        EmbedderId::E2 => Box::new(TemporalRecentModel::new()),
        EmbedderId::E3 => Box::new(TemporalPeriodicModel::new()),
        EmbedderId::E4 => Box::new(TemporalPositionalModel::new()),
        EmbedderId::E9 => Box::new(HdcModel::new(3, 42).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("HdcModel construction failed: {err}"),
                "fix E9 algorithmic hypervector parameters before loading",
            )
        })?),
        other => {
            return Err(EmbedError::forward(
                other,
                "slot is not backed by an active algorithmic context-graph-embeddings model",
                "construct only E2/E3/E4/E9 through the algorithmic loader",
            ))
        }
    };
    Ok(model)
}

fn verify_runtime_artifacts(registration: &EmbedderRegistration) -> EmbedResult<()> {
    let root = Path::new(&registration.path);
    for artifact in runtime_artifacts(registration.embedder)? {
        if !registration
            .weight_files
            .iter()
            .any(|pinned| pinned == artifact)
        {
            return Err(EmbedError::forward(
                registration.embedder,
                format!(
                    "runtime artifact {artifact:?} is not pinned in models_config.toml"
                ),
                "regenerate the Phase 1b models config so every file needed by the concrete forward loader is digest-pinned",
            ));
        }
        let path = root.join(artifact);
        if !path.is_file() {
            return Err(EmbedError::forward(
                registration.embedder,
                format!("runtime artifact missing at {}", path.display()),
                "restore the exact model artifact on D: or regenerate the registration from the verified model directory",
            ));
        }
    }
    Ok(())
}

fn runtime_artifacts(embedder: EmbedderId) -> EmbedResult<&'static [&'static str]> {
    match embedder {
        EmbedderId::E1 | EmbedderId::E8 | EmbedderId::E10 | EmbedderId::E12 | EmbedderId::E14 => {
            Ok(&["config.json", "model.safetensors", "tokenizer.json"])
        }
        EmbedderId::E6 | EmbedderId::E13 => Ok(&[
            "config.json",
            "model.safetensors",
            "sparse_projection.safetensors",
            "tokenizer.json",
        ]),
        EmbedderId::E7 => Ok(&[
            "config.json",
            "model-00001-of-00002.safetensors",
            "model-00002-of-00002.safetensors",
            "tokenizer.json",
        ]),
        other => Err(EmbedError::forward(
            other,
            "slot has no pretrained runtime artifact contract",
            "pretrained artifact verification is valid only for E1/E6/E7/E8/E10/E12/E13/E14",
        )),
    }
}

fn model_input_for_embedder(
    embedder: EmbedderId,
    input: &EmbedderInput,
) -> EmbedResult<ModelInput> {
    if embedder == EmbedderId::E7 {
        let language = AstLanguage::from_path(&input.source_id).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!(
                    "E7 code forward requires source_id with a supported source extension; source_id={:?}; {}: {err}",
                    input.source_id,
                    err.code()
                ),
                "set source_id to the AST chunk's source path, e.g. src/lib.rs, so the code model receives an explicit language",
            )
        })?;
        return ModelInput::code(&input.text, language.slug()).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("failed to build code ModelInput: {err}"),
                "pass non-empty code text and a source_id carrying a supported language extension",
            )
        });
    }
    if matches!(embedder, EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4) {
        let instruction = temporal_instruction(input).ok_or_else(|| {
            EmbedError::forward(
                embedder,
                "temporal algorithmic embedder input did not contain an explicit timestamp instruction",
                "prefix the chunk text or source_id with timestamp:<RFC3339> or epoch:<seconds> so the forward pass is reproducible and cache-safe",
            )
        })?;
        return ModelInput::text_with_instruction(&input.text, instruction).map_err(|err| {
            EmbedError::forward(
                embedder,
                format!("failed to build temporal ModelInput: {err}"),
                "pass non-empty text and an explicit timestamp instruction",
            )
        });
    }
    ModelInput::text(&input.text).map_err(|err| {
        EmbedError::forward(
            embedder,
            format!("failed to build text ModelInput: {err}"),
            "pass non-empty text for this pretrained embedder slot",
        )
    })
}

fn temporal_instruction(input: &EmbedderInput) -> Option<&str> {
    for candidate in [input.source_id.as_str(), input.text.as_str()] {
        for line in candidate.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("timestamp:") || trimmed.starts_with("epoch:") {
                return Some(trimmed);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_forward_mapping_matches_embedder_dimensions() {
        for embedder in SUPPORTED_FORWARD_EMBEDDERS {
            let model_id = embedder_model_id(*embedder).unwrap();
            assert_eq!(model_id.projected_dimension(), embedder.dimension());
        }
    }

    #[test]
    fn unsupported_forward_slot_fails_closed() {
        let err = embedder_model_id(EmbedderId::E15).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_FORWARD_FAILED");
    }

    #[tokio::test]
    async fn algorithmic_temporal_forward_requires_explicit_timestamp() {
        let forward = AlgorithmicEmbedderForward::load(EmbedderId::E2).unwrap();
        let input = EmbedderInput {
            embedder: EmbedderId::E2,
            text: "synthetic temporal chunk".to_string(),
            source_id: "synthetic.rs#0".to_string(),
        };
        let err = forward.forward(&input).await.unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_FORWARD_FAILED");
    }

    #[tokio::test]
    async fn algorithmic_forward_outputs_real_custom_vectors() {
        let input = EmbedderInput {
            embedder: EmbedderId::E9,
            text: "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            source_id: "src/lib.rs#0".to_string(),
        };
        let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
        let output = forward.forward(&input).await.unwrap();
        assert_eq!(output.vector.len(), EmbedderId::E9.dimension());
        assert!(output.vector.iter().any(|value| *value != 0.0));
        assert_eq!(output.precision_class, "algorithmic_real_forward");
    }

    #[tokio::test]
    async fn true_batch_empty_fails_closed_at_embedder_boundary() {
        let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
        let err = forward.forward_true_batch(&[]).await.unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_TRUE_BATCH_EMPTY");
    }

    #[tokio::test]
    async fn algorithmic_true_batch_rejects_without_single_forward_loop() {
        let forward = AlgorithmicEmbedderForward::load(EmbedderId::E9).unwrap();
        assert!(!forward.supports_true_batch());
        let input = EmbedderInput {
            embedder: EmbedderId::E9,
            text: "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            source_id: "src/lib.rs#0".to_string(),
        };
        let err = forward.forward_true_batch(&[input]).await.unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_TRUE_BATCH_UNSUPPORTED");
    }
}
