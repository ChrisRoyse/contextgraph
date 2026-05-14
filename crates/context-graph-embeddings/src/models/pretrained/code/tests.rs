//! Core tests for the CodeModel.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::models::pretrained::code::CodeModel;
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::traits::{EmbeddingModel, SingleModelConfig};

    pub(crate) fn create_test_model() -> CodeModel {
        let model_path = pretrained_test_model_path("code-1536");
        CodeModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create CodeModel")
    }

    #[test]
    fn test_new_creates_unloaded_model() {
        let model = create_test_model();
        assert!(!model.is_initialized());
        assert!(model.supports_true_batch());
    }
}
