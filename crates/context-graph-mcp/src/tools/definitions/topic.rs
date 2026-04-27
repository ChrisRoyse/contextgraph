//! Topic tool definitions per PRD v6 Section 10.2.
//!
//! Tools:
//! - get_topic_portfolio: Get all discovered topics with profiles
//! - get_topic_stability: Get portfolio-level stability metrics
//! - detect_topics: Force topic detection recalculation
//! - get_divergence_alerts: Check for divergence from recent activity
//!
//! Constitution Compliance:
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) weight = 0.0 in topic detection

use crate::tools::types::ToolDefinition;
use serde_json::json;

/// Returns topic tool definitions (4 tools per PRD).
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        // get_topic_portfolio
        ToolDefinition::new(
            "get_topic_portfolio",
            "Get all discovered topics with profiles, stability metrics, and tier info. \
             Topics emerge from weighted multi-space clustering (threshold >= 2.5). \
             Temporal embedders (E2-E4) are excluded from topic detection.",
            json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["brief", "standard", "verbose"],
                        "default": "standard",
                        "description": "Output format: brief (names only), standard (with spaces), verbose (full profiles)"
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_topic_stability
        ToolDefinition::new(
            "get_topic_stability",
            "Get portfolio-level stability metrics including churn rate, entropy, and phase breakdown.",
            json!({
                "type": "object",
                "properties": {
                    "hours": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 168,
                        "default": 6,
                        "description": "Lookback period in hours for computing averages"
                    }
                },
                "additionalProperties": false
            }),
        ),
        // detect_topics
        ToolDefinition::new(
            "detect_topics",
            "Force topic detection recalculation using HDBSCAN clustering. \
             Requires minimum 3 memories (per clustering.parameters.min_cluster_size). \
             Topics require weighted_agreement >= 2.5 to be recognized.",
            json!({
                "type": "object",
                "properties": {
                    "force": {
                        "type": "boolean",
                        "default": false,
                        "description": "Force detection even if recently computed"
                    },
                    "max_memories": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 50000,
                        "default": 10000,
                        "description": "Maximum fingerprints to load for clustering (default: 10000, max: 50000)"
                    }
                },
                "additionalProperties": false
            }),
        ),
        // get_divergence_alerts
        ToolDefinition::new(
            "get_divergence_alerts",
            "Check for divergence from recent activity using SEMANTIC embedders only \
             (E1, E6, E7, E10, E12, E13 per AP-62). E5 (Causal) is excluded per AP-77 \
             (returns 0.0 without CausalDirection). Temporal embedders (E2-E4) excluded per AP-63.",
            json!({
                "type": "object",
                "properties": {
                    "lookback_hours": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 48,
                        "default": 2,
                        "description": "Hours to look back for recent activity comparison"
                    }
                },
                "additionalProperties": false
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_exist_with_required_fields() {
        let tools = definitions();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"get_topic_portfolio"));
        assert!(names.contains(&"get_topic_stability"));
        assert!(names.contains(&"detect_topics"));
        assert!(names.contains(&"get_divergence_alerts"));
        // Key schema checks
        let portfolio = tools
            .iter()
            .find(|t| t.name == "get_topic_portfolio")
            .unwrap();
        assert!(portfolio.description.contains("2.5"));
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        assert!(alerts.description.contains("AP-62"));
    }

    #[test]
    fn test_synthetic_valid_input() {
        let tools = definitions();
        // Portfolio format enum
        let portfolio = tools
            .iter()
            .find(|t| t.name == "get_topic_portfolio")
            .unwrap();
        let formats: Vec<&str> = portfolio.input_schema["properties"]["format"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(formats.len(), 3);
        assert!(formats.contains(&"verbose"));
        // Stability hours range
        let stability = tools
            .iter()
            .find(|t| t.name == "get_topic_stability")
            .unwrap();
        let hours = &stability.input_schema["properties"]["hours"];
        assert_eq!(hours["minimum"], 1);
        assert_eq!(hours["maximum"], 168);
        // detect_topics force default
        let detect = tools.iter().find(|t| t.name == "detect_topics").unwrap();
        assert!(!detect.input_schema["properties"]["force"]["default"]
            .as_bool()
            .unwrap());
    }
}
