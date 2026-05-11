use crate::config::EmbedderRegistration;
use crate::digest::verify_registration_digest;
use crate::embedder_id::{EmbedderId, EmbedderKind};
use crate::error::{EmbedError, EmbedResult};
use crate::types::{EmbedderInput, EmbedderOutput};
use async_trait::async_trait;
use context_graph_core::memory::ast::Language as AstLanguage;
use context_graph_embeddings::models::{
    BgeM3DenseModel, CodeModel, ContextualModel, GraphModel, LateInteractionModel, SemanticModel,
    SparseModel,
};
use context_graph_embeddings::traits::{EmbeddingModel, SingleModelConfig};
use context_graph_embeddings::types::{ModelId, ModelInput};
use std::path::{Path, PathBuf};

pub const SUPPORTED_FORWARD_EMBEDDERS: &[EmbedderId] = &[
    EmbedderId::E1,
    EmbedderId::E6,
    EmbedderId::E7,
    EmbedderId::E8,
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
        if !SUPPORTED_FORWARD_EMBEDDERS.contains(&registration.embedder) {
            return Err(EmbedError::forward(
                registration.embedder,
                "no active neural forward wrapper is registered for this slot",
                "wire the slot to a real context-graph-embeddings model before requesting forward inference",
            ));
        }
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
}

pub fn embedder_model_id(embedder: EmbedderId) -> EmbedResult<ModelId> {
    match embedder {
        EmbedderId::E1 => Ok(ModelId::Semantic),
        EmbedderId::E6 => Ok(ModelId::Sparse),
        EmbedderId::E7 => Ok(ModelId::Code),
        EmbedderId::E8 => Ok(ModelId::Graph),
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
            EmbedError::forward(embedder, format!("SemanticModel construction failed: {err}"), "fix E1 model config before loading")
        })?),
        EmbedderId::E6 => Box::new(SparseModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("SparseModel construction failed: {err}"), "fix E6 model config before loading")
        })?),
        EmbedderId::E7 => Box::new(CodeModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("CodeModel construction failed: {err}"), "fix E7 model config before loading")
        })?),
        EmbedderId::E8 => Box::new(GraphModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("GraphModel construction failed: {err}"), "fix E8 model config before loading")
        })?),
        EmbedderId::E10 => Box::new(ContextualModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("ContextualModel construction failed: {err}"), "fix E10 model config before loading")
        })?),
        EmbedderId::E12 => Box::new(LateInteractionModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("LateInteractionModel construction failed: {err}"), "fix E12 model config before loading")
        })?),
        EmbedderId::E13 => Box::new(SparseModel::new_splade(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("SpladeModel construction failed: {err}"), "fix E13 model config before loading")
        })?),
        EmbedderId::E14 => Box::new(BgeM3DenseModel::new(root, config).map_err(|err| {
            EmbedError::forward(embedder, format!("BgeM3DenseModel construction failed: {err}"), "fix E14 model config before loading")
        })?),
        other => return Err(embedder_model_id(other).unwrap_err()),
    };
    Ok(model)
}

fn model_input_for_embedder(embedder: EmbedderId, input: &EmbedderInput) -> EmbedResult<ModelInput> {
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
    ModelInput::text(&input.text).map_err(|err| {
        EmbedError::forward(
            embedder,
            format!("failed to build text ModelInput: {err}"),
            "pass non-empty text for this pretrained embedder slot",
        )
    })
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
        let err = embedder_model_id(EmbedderId::E2).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_EMBED_FORWARD_FAILED");
    }
}
