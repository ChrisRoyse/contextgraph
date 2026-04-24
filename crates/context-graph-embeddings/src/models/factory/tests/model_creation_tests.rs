//! Model creation tests for all 14 models.

use std::path::PathBuf;

use crate::config::GpuConfig;
use crate::error::EmbeddingError;
use crate::models::factory::DefaultModelFactory;
use crate::traits::{ModelFactory, SingleModelConfig};
use crate::types::ModelId;

#[test]
fn test_create_all_13_models() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    for model_id in ModelId::all() {
        // E14 BgeM3Dense: native Candle module is pending (phase B of the E14
        // integration). The factory path is stubbed to return a clear error so
        // test coverage stays honest rather than silently passing with a mock.
        if *model_id == ModelId::BgeM3Dense {
            let result = factory.create_model(*model_id, &config);
            assert!(
                result.is_err(),
                "BgeM3Dense creation should error until native module lands"
            );
            continue;
        }

        let result = factory.create_model(*model_id, &config);
        assert!(
            result.is_ok(),
            "Failed to create {:?}: {:?}",
            model_id,
            result.err()
        );

        let model = result.unwrap();
        // ModelId::Entity is a legacy alias — the factory creates a KeplerModel,
        // which reports ModelId::Kepler. All other models match their requested ID.
        let expected = if *model_id == ModelId::Entity {
            ModelId::Kepler
        } else {
            *model_id
        };
        assert_eq!(
            model.model_id(),
            expected,
            "Model ID mismatch for {:?}",
            model_id
        );
    }
}

#[test]
fn test_create_with_invalid_config_fails() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig {
        max_batch_size: 0, // Invalid
        ..Default::default()
    };

    let result = factory.create_model(ModelId::Semantic, &config);
    assert!(result.is_err());

    match result {
        Err(EmbeddingError::ConfigError { message }) => {
            assert!(message.contains("max_batch_size"));
        }
        other => panic!("Expected ConfigError, got {:?}", other.err()),
    }
}
