//! Omni-directional inference engine
//!
//! TASK-CAUSAL-001: Implements the omni_infer tool for causal reasoning.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Inference Directions
//!
//! - Forward: A -> B (what effect does A have on B?)
//! - Backward: B -> A (what caused B?)
//! - Bidirectional: A <-> B (how do A and B influence each other?)
//! - Bridge: Cross-domain inference (how does domain X affect domain Y?)
//! - Abduction: Best hypothesis (what best explains the observation?)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Direction for omni_infer.
///
/// Per constitution (line 539), the `omni_infer` tool supports 5 inference directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InferenceDirection {
    /// A -> B (effect of A on B)
    Forward,
    /// B -> A (cause of B)
    Backward,
    /// A <-> B (mutual influence)
    Bidirectional,
    /// Cross-domain bridging
    Bridge,
    /// Best hypothesis for observation
    Abduction,
}

impl InferenceDirection {
    /// Get the string representation for MCP/JSON.
    pub fn as_str(&self) -> &'static str {
        match self {
            InferenceDirection::Forward => "forward",
            InferenceDirection::Backward => "backward",
            InferenceDirection::Bidirectional => "bidirectional",
            InferenceDirection::Bridge => "bridge",
            InferenceDirection::Abduction => "abduction",
        }
    }

    /// Check if this direction requires a target node.
    pub fn requires_target(&self) -> bool {
        matches!(
            self,
            InferenceDirection::Forward
                | InferenceDirection::Backward
                | InferenceDirection::Bidirectional
        )
    }

    /// Get a description of the inference direction.
    pub fn description(&self) -> &'static str {
        match self {
            InferenceDirection::Forward => {
                "Forward inference: What effect does source have on target?"
            }
            InferenceDirection::Backward => "Backward inference: What caused the target?",
            InferenceDirection::Bidirectional => {
                "Bidirectional inference: How do source and target influence each other?"
            }
            InferenceDirection::Bridge => "Bridge inference: Cross-domain causal relationships",
            InferenceDirection::Abduction => {
                "Abduction: Best hypothesis to explain the observation"
            }
        }
    }
}

impl std::str::FromStr for InferenceDirection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "forward" => Ok(InferenceDirection::Forward),
            "backward" => Ok(InferenceDirection::Backward),
            "bidirectional" => Ok(InferenceDirection::Bidirectional),
            "bridge" => Ok(InferenceDirection::Bridge),
            "abduction" => Ok(InferenceDirection::Abduction),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for InferenceDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Result of causal inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceResult {
    /// Direction used for this inference
    pub direction: InferenceDirection,
    /// Source node UUID
    pub source: Uuid,
    /// Target node UUID
    pub target: Uuid,
    /// Causal strength [0, 1] - how strong is the causal relationship
    pub strength: f32,
    /// Confidence in the inference [0, 1] - how sure are we
    pub confidence: f32,
    /// Path through the causal graph (node UUIDs)
    pub path: Vec<Uuid>,
    /// Human-readable explanation of the inference
    pub explanation: String,
}

impl InferenceResult {
    /// Create a new inference result.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        direction: InferenceDirection,
        source: Uuid,
        target: Uuid,
        strength: f32,
        confidence: f32,
        path: Vec<Uuid>,
        explanation: String,
    ) -> Self {
        Self {
            direction,
            source,
            target,
            strength: strength.clamp(0.0, 1.0),
            confidence: confidence.clamp(0.0, 1.0),
            path,
            explanation,
        }
    }

    /// Check if this is a high-confidence result.
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Check if this is a strong causal relationship.
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.7
    }

    /// Get the path length (number of hops).
    pub fn path_length(&self) -> usize {
        if self.path.len() <= 1 {
            0
        } else {
            self.path.len() - 1
        }
    }

    /// Check if this is a direct (single-hop) relationship.
    pub fn is_direct(&self) -> bool {
        self.path_length() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inference_direction_str() {
        assert_eq!(InferenceDirection::Forward.as_str(), "forward");
        assert_eq!(InferenceDirection::Backward.as_str(), "backward");
        assert_eq!(InferenceDirection::Bidirectional.as_str(), "bidirectional");
        assert_eq!(InferenceDirection::Bridge.as_str(), "bridge");
        assert_eq!(InferenceDirection::Abduction.as_str(), "abduction");
    }

    #[test]
    fn test_inference_direction_from_str() {
        assert_eq!(
            "forward".parse::<InferenceDirection>(),
            Ok(InferenceDirection::Forward)
        );
        assert_eq!(
            "BACKWARD".parse::<InferenceDirection>(),
            Ok(InferenceDirection::Backward)
        );
        assert_eq!("invalid".parse::<InferenceDirection>(), Err(()));
    }

    #[test]
    fn test_inference_direction_requires_target() {
        assert!(InferenceDirection::Forward.requires_target());
        assert!(InferenceDirection::Backward.requires_target());
        assert!(InferenceDirection::Bidirectional.requires_target());
        assert!(!InferenceDirection::Bridge.requires_target());
        assert!(!InferenceDirection::Abduction.requires_target());
    }

    #[test]
    fn test_inference_result_helpers() {
        let result = InferenceResult::new(
            InferenceDirection::Forward,
            Uuid::new_v4(),
            Uuid::new_v4(),
            0.85,
            0.9,
            vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
            "Test".to_string(),
        );

        assert!(result.is_high_confidence());
        assert!(result.is_strong());
        assert_eq!(result.path_length(), 2);
        assert!(!result.is_direct());
    }
}
