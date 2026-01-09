//! Common helper types and functions for stub layers.
//!
//! This module contains shared utilities used across all stub layer implementations.

/// Stub layer configuration (placeholder for future extension).
#[derive(Debug, Clone, Default)]
pub struct StubLayerConfig {
    // Reserved for future configuration options
}

/// Compute a deterministic hash from input string for reproducible stub behavior.
/// Same input always produces same output.
///
/// # Arguments
/// * `input` - The input string to hash
///
/// # Returns
/// A deterministic u64 hash value
#[allow(dead_code)] // Reserved for future deterministic stub implementations
pub fn compute_input_hash(input: &str) -> u64 {
    let mut hash: u64 = 0;
    for (i, byte) in input.bytes().enumerate() {
        hash = hash.wrapping_add((byte as u64).wrapping_mul((i as u64).wrapping_add(1)));
    }
    hash
}

/// Helper to create test input for layer testing.
#[cfg(test)]
pub fn test_input(content: &str) -> crate::types::LayerInput {
    crate::types::LayerInput::new("test-request-123".to_string(), content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_input_hash_determinism() {
        let hash1 = compute_input_hash("test string");
        let hash2 = compute_input_hash("test string");
        let hash3 = compute_input_hash("different string");

        assert_eq!(hash1, hash2, "Same input should produce same hash");
        assert_ne!(
            hash1, hash3,
            "Different input should produce different hash"
        );
    }

    #[test]
    fn test_stub_layer_config_default() {
        let config = StubLayerConfig::default();
        // Should compile and create default config
        let _ = format!("{:?}", config);
    }
}
