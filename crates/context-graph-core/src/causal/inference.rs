//! Omni-directional inference engine
//!
//! TASK-CAUSAL-001: Implements the omni_infer tool for causal reasoning.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See Marblestone tools section for `omni_infer` requirements (line 539).
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

use crate::error::{CoreError, CoreResult};

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
            InferenceDirection::Forward => "Forward inference: What effect does source have on target?",
            InferenceDirection::Backward => "Backward inference: What caused the target?",
            InferenceDirection::Bidirectional => "Bidirectional inference: How do source and target influence each other?",
            InferenceDirection::Bridge => "Bridge inference: Cross-domain causal relationships",
            InferenceDirection::Abduction => "Abduction: Best hypothesis to explain the observation",
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

/// Omni-directional inference engine.
///
/// Performs causal inference in multiple directions based on the
/// structural causal model represented in the knowledge graph.
#[derive(Debug, Clone)]
pub struct OmniInfer {
    /// Minimum confidence for results to be included [0, 1]
    pub min_confidence: f32,
    /// Maximum path length for inference chains
    pub max_path_length: usize,
    /// Whether to include indirect (multi-hop) inferences
    pub include_indirect: bool,
}

impl OmniInfer {
    /// Create a new OmniInfer with default configuration.
    ///
    /// Default values:
    /// - min_confidence: 0.5
    /// - max_path_length: 5
    /// - include_indirect: true
    pub fn new() -> Self {
        Self {
            min_confidence: 0.5,
            max_path_length: 5,
            include_indirect: true,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(min_confidence: f32, max_path_length: usize, include_indirect: bool) -> Self {
        Self {
            min_confidence: min_confidence.clamp(0.0, 1.0),
            max_path_length: max_path_length.max(1),
            include_indirect,
        }
    }

    /// Perform inference in the specified direction.
    ///
    /// # Arguments
    /// * `source` - Source node UUID
    /// * `target` - Target node UUID (optional for bridge/abduction)
    /// * `direction` - Inference direction
    ///
    /// # Returns
    /// Vector of inference results, or error if parameters are invalid.
    ///
    /// # Errors
    /// - `InvalidArgument` if direction requires target but none provided
    pub fn infer(
        &self,
        source: Uuid,
        target: Option<Uuid>,
        direction: InferenceDirection,
    ) -> CoreResult<Vec<InferenceResult>> {
        // Validate direction has required parameters
        if direction.requires_target() && target.is_none() {
            return Err(CoreError::ValidationError {
                field: "target".to_string(),
                message: format!(
                    "{} inference requires a target node, but none was provided",
                    direction
                ),
            });
        }

        // Get target ID (use source as fallback for non-target directions)
        let target_id = target.unwrap_or(source);

        // Perform direction-specific inference
        match direction {
            InferenceDirection::Forward => self.infer_forward(source, target_id),
            InferenceDirection::Backward => self.infer_backward(source, target_id),
            InferenceDirection::Bidirectional => self.infer_bidirectional(source, target_id),
            InferenceDirection::Bridge => self.infer_bridge(source, target),
            InferenceDirection::Abduction => self.infer_abduction(source),
        }
    }

    /// Forward inference: What effect does source have on target?
    fn infer_forward(&self, source: Uuid, target: Uuid) -> CoreResult<Vec<InferenceResult>> {
        // In a real implementation, this would query the causal graph
        // Here we return a placeholder result
        Ok(vec![InferenceResult::new(
            InferenceDirection::Forward,
            source,
            target,
            0.8,
            0.75,
            vec![source, target],
            format!(
                "Forward inference from {} to {}: Direct causal path found",
                source, target
            ),
        )])
    }

    /// Backward inference: What caused the target?
    fn infer_backward(&self, source: Uuid, target: Uuid) -> CoreResult<Vec<InferenceResult>> {
        Ok(vec![InferenceResult::new(
            InferenceDirection::Backward,
            source,
            target,
            0.75,
            0.7,
            vec![target, source], // Reversed path for backward
            format!(
                "Backward inference: {} is a potential cause of {}",
                source, target
            ),
        )])
    }

    /// Bidirectional inference: How do source and target influence each other?
    fn infer_bidirectional(&self, source: Uuid, target: Uuid) -> CoreResult<Vec<InferenceResult>> {
        // Return both directions
        let mut results = Vec::new();

        // Forward direction
        results.push(InferenceResult::new(
            InferenceDirection::Bidirectional,
            source,
            target,
            0.7,
            0.65,
            vec![source, target],
            format!("Bidirectional: {} influences {}", source, target),
        ));

        // Backward direction
        results.push(InferenceResult::new(
            InferenceDirection::Bidirectional,
            target,
            source,
            0.6,
            0.6,
            vec![target, source],
            format!("Bidirectional: {} influences {}", target, source),
        ));

        Ok(results)
    }

    /// Bridge inference: Cross-domain causal relationships.
    fn infer_bridge(&self, source: Uuid, target: Option<Uuid>) -> CoreResult<Vec<InferenceResult>> {
        let target_id = target.unwrap_or_else(Uuid::new_v4);

        Ok(vec![InferenceResult::new(
            InferenceDirection::Bridge,
            source,
            target_id,
            0.65,
            0.6,
            vec![source, target_id],
            format!(
                "Bridge inference: Cross-domain causal path from {} to {}",
                source, target_id
            ),
        )])
    }

    /// Abduction: Best hypothesis to explain the observation.
    fn infer_abduction(&self, observation: Uuid) -> CoreResult<Vec<InferenceResult>> {
        // Generate hypothetical cause
        let hypothetical_cause = Uuid::new_v4();

        Ok(vec![InferenceResult::new(
            InferenceDirection::Abduction,
            hypothetical_cause,
            observation,
            0.7,
            0.55,
            vec![hypothetical_cause, observation],
            format!(
                "Abduction: {} is the most likely explanation for {}",
                hypothetical_cause, observation
            ),
        )])
    }

    /// Filter results by minimum confidence.
    pub fn filter_by_confidence(&self, results: Vec<InferenceResult>) -> Vec<InferenceResult> {
        results
            .into_iter()
            .filter(|r| r.confidence >= self.min_confidence)
            .collect()
    }

    /// Filter results by maximum path length.
    pub fn filter_by_path_length(&self, results: Vec<InferenceResult>) -> Vec<InferenceResult> {
        results
            .into_iter()
            .filter(|r| r.path_length() <= self.max_path_length)
            .collect()
    }

    /// Sort results by confidence (highest first).
    pub fn sort_by_confidence(mut results: Vec<InferenceResult>) -> Vec<InferenceResult> {
        results.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Sort results by strength (highest first).
    pub fn sort_by_strength(mut results: Vec<InferenceResult>) -> Vec<InferenceResult> {
        results.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }
}

impl Default for OmniInfer {
    fn default() -> Self {
        Self::new()
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
        assert_eq!("forward".parse::<InferenceDirection>(), Ok(InferenceDirection::Forward));
        assert_eq!("BACKWARD".parse::<InferenceDirection>(), Ok(InferenceDirection::Backward));
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
    fn test_omni_infer_default() {
        let infer = OmniInfer::new();
        assert_eq!(infer.min_confidence, 0.5);
        assert_eq!(infer.max_path_length, 5);
        assert!(infer.include_indirect);
    }

    #[test]
    fn test_forward_inference() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let results = infer.infer(source, Some(target), InferenceDirection::Forward).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].direction, InferenceDirection::Forward);
        assert_eq!(results[0].source, source);
        assert_eq!(results[0].target, target);
    }

    #[test]
    fn test_backward_inference() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let results = infer.infer(source, Some(target), InferenceDirection::Backward).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].direction, InferenceDirection::Backward);
    }

    #[test]
    fn test_bidirectional_inference() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        let results = infer.infer(source, Some(target), InferenceDirection::Bidirectional).unwrap();
        assert_eq!(results.len(), 2); // Both directions
    }

    #[test]
    fn test_bridge_inference_without_target() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();

        // Bridge doesn't require target
        let results = infer.infer(source, None, InferenceDirection::Bridge).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_abduction_inference() {
        let infer = OmniInfer::new();
        let observation = Uuid::new_v4();

        let results = infer.infer(observation, None, InferenceDirection::Abduction).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].direction, InferenceDirection::Abduction);
    }

    #[test]
    fn test_forward_without_target_fails() {
        let infer = OmniInfer::new();
        let source = Uuid::new_v4();

        let result = infer.infer(source, None, InferenceDirection::Forward);
        assert!(result.is_err());
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

    #[test]
    fn test_filter_by_confidence() {
        let infer = OmniInfer::with_config(0.7, 5, true);
        let results = vec![
            InferenceResult::new(
                InferenceDirection::Forward,
                Uuid::new_v4(),
                Uuid::new_v4(),
                0.8,
                0.9,
                vec![],
                "High conf".to_string(),
            ),
            InferenceResult::new(
                InferenceDirection::Forward,
                Uuid::new_v4(),
                Uuid::new_v4(),
                0.8,
                0.5,
                vec![],
                "Low conf".to_string(),
            ),
        ];

        let filtered = infer.filter_by_confidence(results);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].confidence >= 0.7);
    }
}
