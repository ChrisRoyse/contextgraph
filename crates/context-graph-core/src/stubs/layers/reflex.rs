//! Stub implementation of the Reflex layer.
//!
//! The Reflex layer provides pattern-matched fast responses.
//! This stub returns NotImplemented error - no mock data in production (AP-007).
//!
//! # Latency Budget
//! Real implementation: 100us max (very fast)
//! Stub implementation: Fails fast with error

use async_trait::async_trait;
use std::time::Duration;

use crate::error::{CoreError, CoreResult};
use crate::traits::NervousLayer;
use crate::types::{LayerId, LayerInput, LayerOutput};

use super::helpers::StubLayerConfig;

/// Stub implementation of the Reflex layer.
#[derive(Debug, Clone, Default)]
pub struct StubReflexLayer {
    _config: StubLayerConfig,
}

impl StubReflexLayer {
    /// Create a new stub reflex layer.
    pub fn new() -> Self {
        Self {
            _config: StubLayerConfig::default(),
        }
    }
}

#[async_trait]
impl NervousLayer for StubReflexLayer {
    async fn process(&self, _input: LayerInput) -> CoreResult<LayerOutput> {
        // FAIL FAST - No mock data in production (AP-007)
        Err(CoreError::NotImplemented(
            "L2 ReflexLayer requires real implementation. \
             See: docs2/codestate/sherlockplans/agent4-bio-nervous-research.md"
                .into(),
        ))
    }

    fn latency_budget(&self) -> Duration {
        Duration::from_micros(100)
    }

    fn layer_id(&self) -> LayerId {
        LayerId::Reflex
    }

    fn layer_name(&self) -> &'static str {
        "Reflex Layer [NOT IMPLEMENTED]"
    }

    async fn health_check(&self) -> CoreResult<bool> {
        // Stub is NOT healthy - requires real implementation
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stubs::layers::helpers::test_input;

    #[tokio::test]
    async fn test_reflex_layer_fails_fast() {
        let layer = StubReflexLayer::new();
        let input = test_input("test input");

        let result = layer.process(input).await;
        assert!(result.is_err(), "Stub should fail fast");

        let err = result.unwrap_err();
        assert!(matches!(err, CoreError::NotImplemented(_)));
    }

    #[tokio::test]
    async fn test_reflex_layer_health_check_returns_false() {
        let layer = StubReflexLayer::new();
        assert!(
            !layer.health_check().await.unwrap(),
            "Stub should report unhealthy"
        );
    }

    #[test]
    fn test_reflex_layer_properties() {
        let layer = StubReflexLayer::new();
        assert_eq!(layer.layer_id(), LayerId::Reflex);
        assert_eq!(layer.latency_budget(), Duration::from_micros(100));
        assert!(layer.layer_name().contains("NOT IMPLEMENTED"));
    }
}
