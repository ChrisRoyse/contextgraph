//! Factory construction and model path tests.

use std::path::PathBuf;

use crate::config::GpuConfig;
use crate::models::factory::DefaultModelFactory;
use crate::traits::ModelFactory;

#[test]
fn test_factory_new() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert_eq!(factory.models_dir(), &PathBuf::from("./models"));
    assert!(factory.gpu_config().enabled);
}

#[test]
fn test_supported_models_count() {
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    assert_eq!(factory.supported_models().len(), 15);
}
