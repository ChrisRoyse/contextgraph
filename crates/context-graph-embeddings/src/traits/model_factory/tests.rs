//! Tests for the model factory module.
//!
//! Contains test implementations and integration tests.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use crate::error::{EmbeddingError, EmbeddingResult};
    use crate::traits::model_factory::{
        get_memory_estimate, ModelFactory, QuantizationMode, SingleModelConfig,
    };
    use crate::traits::EmbeddingModel;
    use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

    // =========================================================================
    // Test Model Implementation
    // =========================================================================

    /// Test implementation of EmbeddingModel for factory testing.
    struct TestModel {
        model_id: ModelId,
        #[allow(dead_code)]
        config: SingleModelConfig,
        initialized: AtomicBool,
    }

    impl TestModel {
        fn new(model_id: ModelId, config: SingleModelConfig) -> Self {
            Self {
                model_id,
                config,
                initialized: AtomicBool::new(false),
            }
        }
    }

    #[async_trait::async_trait]
    impl EmbeddingModel for TestModel {
        fn model_id(&self) -> ModelId {
            self.model_id
        }

        fn supported_input_types(&self) -> &[InputType] {
            &[InputType::Text]
        }

        async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
            if !self.is_initialized() {
                return Err(EmbeddingError::NotInitialized {
                    model_id: self.model_id,
                });
            }

            self.validate_input(input)?;

            let dim = self.dimension();
            let vector: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001).sin()).collect();
            Ok(ModelEmbedding::new(self.model_id, vector, 100))
        }

        fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }
    }

    // =========================================================================
    // Test Factory Implementation
    // =========================================================================

    /// Test factory that creates TestModel instances.
    struct TestFactory;

    impl ModelFactory for TestFactory {
        fn create_model(
            &self,
            model_id: ModelId,
            config: &SingleModelConfig,
        ) -> EmbeddingResult<Box<dyn EmbeddingModel>> {
            config.validate()?;

            if !self.supports_model(model_id) {
                return Err(EmbeddingError::ModelNotFound { model_id });
            }

            Ok(Box::new(TestModel::new(model_id, config.clone())))
        }

        fn supported_models(&self) -> &[ModelId] {
            ModelId::all()
        }

        fn estimate_memory(&self, model_id: ModelId) -> usize {
            get_memory_estimate(model_id)
        }
    }

    // =========================================================================
    // FACTORY TRAIT TESTS
    // =========================================================================

    #[test]
    fn test_factory_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TestFactory>();
    }

    #[test]
    fn test_factory_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TestFactory>();
    }

    #[test]
    fn test_factory_supported_models_all_14() {
        let factory = TestFactory;
        let models = factory.supported_models();
        assert_eq!(models.len(), 14);

        // Verify all 14 models are present
        for model_id in ModelId::all() {
            assert!(models.contains(model_id), "Missing model: {:?}", model_id);
        }
    }

    #[test]
    fn test_factory_supports_model() {
        let factory = TestFactory;
        for model_id in ModelId::all() {
            assert!(
                factory.supports_model(*model_id),
                "Factory should support {:?}",
                model_id
            );
        }
    }

    #[test]
    fn test_factory_create_model_succeeds() {
        let factory = TestFactory;
        let config = SingleModelConfig::default();

        for model_id in ModelId::all() {
            let result = factory.create_model(*model_id, &config);
            assert!(
                result.is_ok(),
                "Failed to create {:?}: {:?}",
                model_id,
                result.err()
            );

            let model = result.unwrap();
            assert_eq!(model.model_id(), *model_id);
            assert!(!model.is_initialized()); // Not loaded yet
        }
    }

    #[test]
    fn test_factory_create_model_with_invalid_config_fails() {
        let factory = TestFactory;
        let config = SingleModelConfig {
            max_batch_size: 0,
            ..Default::default()
        };

        let result = factory.create_model(ModelId::Semantic, &config);
        assert!(result.is_err());
        match result {
            Err(EmbeddingError::ConfigError { .. }) => {}
            _ => panic!("Expected ConfigError"),
        }
    }

    #[test]
    fn test_factory_estimate_memory_nonzero() {
        let factory = TestFactory;

        for model_id in ModelId::all() {
            let estimate = factory.estimate_memory(*model_id);
            assert!(
                estimate > 0,
                "Memory estimate for {:?} should be > 0",
                model_id
            );
        }
    }

    #[test]
    fn test_factory_estimate_memory_quantized() {
        let factory = TestFactory;
        let model_id = ModelId::Semantic;

        let base = factory.estimate_memory(model_id);
        let fp16 = factory.estimate_memory_quantized(model_id, QuantizationMode::Fp16);
        let int8 = factory.estimate_memory_quantized(model_id, QuantizationMode::Int8);

        assert_eq!(fp16, (base as f32 * 0.5) as usize);
        assert_eq!(int8, (base as f32 * 0.25) as usize);
    }

    // =========================================================================
    // OBJECT SAFETY TESTS
    // =========================================================================

    #[test]
    fn test_factory_trait_object_in_arc() {
        let factory: Arc<dyn ModelFactory> = Arc::new(TestFactory);
        assert!(factory.supports_model(ModelId::Semantic));
    }

    #[test]
    fn test_factory_trait_object_in_box() {
        let factory: Box<dyn ModelFactory> = Box::new(TestFactory);
        assert_eq!(factory.supported_models().len(), 14);
    }
}
