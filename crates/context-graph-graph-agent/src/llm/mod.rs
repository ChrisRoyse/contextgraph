//! LLM wrapper for graph relationship analysis.
//!
//! This module provides the LLM integration for graph relationship detection.
//! It shares the underlying Hermes 2 Pro model with the causal-agent crate
//! via `Arc<CausalDiscoveryLLM>`.
//!
//! ## Grammar-Constrained Output
//!
//! This module uses GBNF grammar constraints for 100% valid JSON output.
//! The graph relationship grammar is separate from the causal analysis grammar.

pub mod prompt;

use std::sync::Arc;

use context_graph_causal_agent::llm::GrammarType;
use context_graph_causal_agent::CausalDiscoveryLLM;
use serde_json::Value;

use crate::error::{GraphAgentError, GraphAgentResult};
use crate::types::{
    ContentDomain, GraphAnalysisResult, GraphLinkDirection, RelationshipCategory, RelationshipType,
};

use prompt::GraphPromptBuilder;

/// Graph relationship analyzer using shared LLM.
///
/// This wraps the `CausalDiscoveryLLM` from causal-agent to share
/// the same Hermes 2 Pro model instance, avoiding duplicate VRAM usage.
pub struct GraphRelationshipLLM {
    /// Shared LLM from causal-agent.
    llm: Arc<CausalDiscoveryLLM>,

    /// Prompt builder for graph analysis.
    prompt_builder: GraphPromptBuilder,
}

impl GraphRelationshipLLM {
    /// Create a new graph relationship analyzer using shared LLM.
    ///
    /// # Arguments
    /// * `shared_llm` - Arc-wrapped CausalDiscoveryLLM (already loaded)
    pub fn new(shared_llm: Arc<CausalDiscoveryLLM>) -> Self {
        Self {
            llm: shared_llm,
            prompt_builder: GraphPromptBuilder::new(),
        }
    }

    /// Create with custom prompt configuration.
    pub fn with_prompt_builder(mut self, prompt_builder: GraphPromptBuilder) -> Self {
        self.prompt_builder = prompt_builder;
        self
    }

    /// Check if the underlying LLM is loaded.
    pub fn is_loaded(&self) -> bool {
        self.llm.is_loaded()
    }

    /// Analyze graph relationship between two memories.
    ///
    /// # Arguments
    /// * `memory_a` - First memory content
    /// * `memory_b` - Second memory content
    ///
    /// # Returns
    /// Analysis result with relationship type, direction, and confidence
    pub async fn analyze_relationship(
        &self,
        memory_a: &str,
        memory_b: &str,
    ) -> GraphAgentResult<GraphAnalysisResult> {
        if !self.is_loaded() {
            return Err(GraphAgentError::LlmNotInitialized);
        }

        let prompt = self.prompt_builder.build_analysis_prompt(memory_a, memory_b);

        // Use the shared LLM with graph grammar for inference
        let response = self
            .llm
            .generate_with_grammar(&prompt, GrammarType::Graph)
            .await
            .map_err(|e| GraphAgentError::LlmInferenceError {
                message: format!("Graph LLM inference failed: {}", e),
            })?;

        // Parse the response - grammar guarantees valid JSON
        self.parse_analysis_response(&response)
    }

    /// Analyze multiple memory pairs in batch.
    ///
    /// # Arguments
    /// * `pairs` - Vector of (memory_a, memory_b) content pairs
    ///
    /// # Returns
    /// Vector of analysis results, one per pair
    pub async fn analyze_batch(
        &self,
        pairs: &[(String, String)],
    ) -> GraphAgentResult<Vec<GraphAnalysisResult>> {
        if !self.is_loaded() {
            return Err(GraphAgentError::LlmNotInitialized);
        }

        if pairs.is_empty() {
            return Ok(Vec::new());
        }

        // Process individually for better results with grammar constraints
        let mut results = Vec::with_capacity(pairs.len());
        for (a, b) in pairs {
            results.push(self.analyze_relationship(a, b).await?);
        }
        Ok(results)
    }

    /// Validate a specific relationship between two memories.
    ///
    /// # Arguments
    /// * `memory_a` - First memory content
    /// * `memory_b` - Second memory content
    /// * `expected_type` - The relationship type to validate
    ///
    /// # Returns
    /// Validation result with confidence
    pub async fn validate_relationship(
        &self,
        memory_a: &str,
        memory_b: &str,
        expected_type: RelationshipType,
    ) -> GraphAgentResult<(bool, f32, String)> {
        if !self.is_loaded() {
            return Err(GraphAgentError::LlmNotInitialized);
        }

        let prompt = self
            .prompt_builder
            .build_validation_prompt(memory_a, memory_b, expected_type);

        let response = self
            .llm
            .generate_with_grammar(&prompt, GrammarType::Validation)
            .await
            .map_err(|e| GraphAgentError::LlmInferenceError {
                message: format!("Graph LLM validation failed: {}", e),
            })?;

        self.parse_validation_response(&response)
    }

    /// Parse single analysis response from LLM.
    ///
    /// With grammar constraints, JSON is guaranteed to be valid.
    fn parse_analysis_response(&self, response: &str) -> GraphAgentResult<GraphAnalysisResult> {
        // Parse as JSON - grammar guarantees valid structure
        let value: Value = serde_json::from_str(response.trim()).map_err(|e| {
            GraphAgentError::LlmResponseParseError {
                message: format!(
                    "JSON parse failed (unexpected with grammar): {}. Raw response: {}",
                    e, response
                ),
            }
        })?;

        self.extract_analysis_from_json(&value, response)
    }

    /// Extract analysis result from parsed JSON.
    fn extract_analysis_from_json(
        &self,
        value: &Value,
        raw_response: &str,
    ) -> GraphAgentResult<GraphAnalysisResult> {
        // Required: has_connection (boolean)
        let has_connection = value
            .get("has_connection")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'has_connection' boolean in LLM response: {}",
                    raw_response
                ),
            })?;

        // Required: direction (string)
        let direction_str = value
            .get("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'direction' string in LLM response: {}",
                    raw_response
                ),
            })?;
        let direction = GraphLinkDirection::from_str(direction_str);

        // Required: relationship_type (string)
        let type_str = value
            .get("relationship_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'relationship_type' string in LLM response: {}",
                    raw_response
                ),
            })?;
        let relationship_type = RelationshipType::from_str(type_str);

        // Optional: category (string, defaults to relationship_type's category)
        let category = value
            .get("category")
            .and_then(|v| v.as_str())
            .map(RelationshipCategory::from_str)
            .unwrap_or_else(|| relationship_type.category());

        // Optional: domain (string, defaults to General)
        let domain = value
            .get("domain")
            .and_then(|v| v.as_str())
            .map(ContentDomain::from_str)
            .unwrap_or(ContentDomain::General);

        // Required: confidence (number)
        let confidence = value
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'confidence' number in LLM response: {}",
                    raw_response
                ),
            })?;

        // Optional: description (string, defaults to empty)
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(GraphAnalysisResult {
            has_connection,
            direction,
            relationship_type,
            category,
            domain,
            confidence,
            description,
            raw_response: Some(raw_response.to_string()),
        })
    }

    /// Parse validation response from LLM.
    fn parse_validation_response(&self, response: &str) -> GraphAgentResult<(bool, f32, String)> {
        // Parse as JSON - grammar guarantees valid structure
        let value: Value = serde_json::from_str(response.trim()).map_err(|e| {
            GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Validation JSON parse failed (unexpected with grammar): {}. Raw response: {}",
                    e, response
                ),
            }
        })?;

        // Required: valid (boolean)
        let valid = value
            .get("valid")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'valid' boolean in validation response: {}",
                    response
                ),
            })?;

        // Required: confidence (number)
        let confidence = value
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: format!(
                    "Missing or invalid 'confidence' number in validation response: {}",
                    response
                ),
            })?;

        // Optional: explanation (string, defaults to empty)
        let explanation = value
            .get("explanation")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok((valid, confidence, explanation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to test JSON parsing without LLM
    fn parse_json_response(response: &str) -> GraphAgentResult<GraphAnalysisResult> {
        let value: Value = serde_json::from_str(response.trim()).map_err(|e| {
            GraphAgentError::LlmResponseParseError {
                message: format!("Parse error: {}", e),
            }
        })?;

        // Required: has_connection
        let has_connection = value
            .get("has_connection")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: "Missing has_connection".to_string(),
            })?;

        // Required: direction
        let direction_str = value
            .get("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: "Missing direction".to_string(),
            })?;
        let direction = GraphLinkDirection::from_str(direction_str);

        // Required: relationship_type
        let type_str = value
            .get("relationship_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: "Missing relationship_type".to_string(),
            })?;
        let relationship_type = RelationshipType::from_str(type_str);

        // Optional: category (defaults to relationship_type's category)
        let category = value
            .get("category")
            .and_then(|v| v.as_str())
            .map(RelationshipCategory::from_str)
            .unwrap_or_else(|| relationship_type.category());

        // Optional: domain (defaults to General)
        let domain = value
            .get("domain")
            .and_then(|v| v.as_str())
            .map(ContentDomain::from_str)
            .unwrap_or(ContentDomain::General);

        // Required: confidence
        let confidence = value
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| (v as f32).clamp(0.0, 1.0))
            .ok_or_else(|| GraphAgentError::LlmResponseParseError {
                message: "Missing confidence".to_string(),
            })?;

        // Optional: description
        let description = value
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(GraphAnalysisResult {
            has_connection,
            direction,
            relationship_type,
            category,
            domain,
            confidence,
            description,
            raw_response: Some(response.to_string()),
        })
    }

    #[test]
    fn test_parse_valid_json_with_connection() {
        let response = r#"{"has_connection": true, "direction": "a_to_b", "relationship_type": "imports", "confidence": 0.85, "description": "A imports B"}"#;

        let result = parse_json_response(response).unwrap();

        assert!(result.has_connection);
        assert_eq!(result.direction, GraphLinkDirection::AConnectsB);
        assert_eq!(result.relationship_type, RelationshipType::Imports);
        assert!((result.confidence - 0.85).abs() < 0.01);
        assert_eq!(result.description, "A imports B");
    }

    #[test]
    fn test_parse_valid_json_no_connection() {
        let response = r#"{"has_connection": false, "direction": "none", "relationship_type": "none", "confidence": 0.1, "description": "No relationship"}"#;

        let result = parse_json_response(response).unwrap();

        assert!(!result.has_connection);
        assert_eq!(result.direction, GraphLinkDirection::NoConnection);
        assert_eq!(result.relationship_type, RelationshipType::None);
        assert!((result.confidence - 0.1).abs() < 0.01);
    }

    #[test]
    fn test_parse_malformed_json_fails() {
        let response = r#"{"has_connection": true, direction: "a_to_b"}"#; // Missing quotes

        let result = parse_json_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_required_field_fails() {
        // Missing confidence field
        let response = r#"{"has_connection": true, "direction": "a_to_b", "relationship_type": "imports", "description": "A imports B"}"#;

        let result = parse_json_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_implements_relationship() {
        let response = r#"{"has_connection": true, "direction": "a_to_b", "relationship_type": "implements", "confidence": 0.92, "description": "A implements trait B"}"#;

        let result = parse_json_response(response).unwrap();

        assert!(result.has_connection);
        assert_eq!(result.relationship_type, RelationshipType::Implements);
        assert!((result.confidence - 0.92).abs() < 0.01);
    }

    #[test]
    fn test_parse_bidirectional_relationship() {
        let response = r#"{"has_connection": true, "direction": "bidirectional", "relationship_type": "references", "confidence": 0.75, "description": "Mutual reference"}"#;

        let result = parse_json_response(response).unwrap();

        assert!(result.has_connection);
        assert_eq!(result.direction, GraphLinkDirection::Bidirectional);
    }
}
