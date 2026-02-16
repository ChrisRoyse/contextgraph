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
                }
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
                }
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
                    }
                }
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
                }
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_definitions_count() {
        let tools = definitions();
        assert_eq!(tools.len(), 4, "Should have 4 topic tools");
    }

    #[test]
    fn test_topic_tools_names() {
        let tools = definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"get_topic_portfolio"));
        assert!(names.contains(&"get_topic_stability"));
        assert!(names.contains(&"detect_topics"));
        assert!(names.contains(&"get_divergence_alerts"));
    }

    #[test]
    fn test_get_topic_portfolio_format_enum() {
        let tools = definitions();
        let portfolio = tools
            .iter()
            .find(|t| t.name == "get_topic_portfolio")
            .unwrap();
        let props = portfolio.input_schema.get("properties").unwrap();
        let format = props.get("format").unwrap();
        let enum_vals = format.get("enum").unwrap().as_array().unwrap();
        assert_eq!(enum_vals.len(), 3);
        let values: Vec<&str> = enum_vals.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(values.contains(&"brief"));
        assert!(values.contains(&"standard"));
        assert!(values.contains(&"verbose"));
    }

    #[test]
    fn test_get_topic_stability_hours_range() {
        let tools = definitions();
        let stability = tools
            .iter()
            .find(|t| t.name == "get_topic_stability")
            .unwrap();
        let props = stability.input_schema.get("properties").unwrap();
        let hours = props.get("hours").unwrap();
        assert_eq!(hours.get("minimum").unwrap().as_u64().unwrap(), 1);
        assert_eq!(hours.get("maximum").unwrap().as_u64().unwrap(), 168);
        assert_eq!(hours.get("default").unwrap().as_u64().unwrap(), 6);
    }

    #[test]
    fn test_detect_topics_force_default() {
        let tools = definitions();
        let detect = tools.iter().find(|t| t.name == "detect_topics").unwrap();
        let props = detect.input_schema.get("properties").unwrap();
        let force = props.get("force").unwrap();
        assert!(!force.get("default").unwrap().as_bool().unwrap());
    }

    #[test]
    fn test_get_divergence_alerts_lookback_range() {
        let tools = definitions();
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        let props = alerts.input_schema.get("properties").unwrap();
        let lookback = props.get("lookback_hours").unwrap();
        assert_eq!(lookback.get("minimum").unwrap().as_u64().unwrap(), 1);
        assert_eq!(lookback.get("maximum").unwrap().as_u64().unwrap(), 48);
        assert_eq!(lookback.get("default").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn test_descriptions_mention_constitution() {
        let tools = definitions();
        // get_topic_portfolio mentions threshold
        let portfolio = tools
            .iter()
            .find(|t| t.name == "get_topic_portfolio")
            .unwrap();
        assert!(
            portfolio.description.contains("2.5"),
            "Should mention threshold"
        );

        // get_topic_stability mentions stability metrics
        let stability = tools
            .iter()
            .find(|t| t.name == "get_topic_stability")
            .unwrap();
        assert!(
            stability.description.contains("entropy"),
            "Should reference entropy metric"
        );

        // get_divergence_alerts mentions AP-62
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        assert!(
            alerts.description.contains("AP-62"),
            "Should reference AP-62"
        );
    }

    // ========== SYNTHETIC DATA VALIDATION TESTS ==========

    #[test]
    fn test_synthetic_get_topic_portfolio_valid() {
        // Synthetic valid input
        let synthetic_input = json!({"format": "verbose"});

        let tools = definitions();
        let portfolio = tools
            .iter()
            .find(|t| t.name == "get_topic_portfolio")
            .unwrap();
        let props = portfolio.input_schema.get("properties").unwrap();

        // Validate format is in enum
        let format = synthetic_input.get("format").unwrap().as_str().unwrap();
        let valid_formats: Vec<&str> = props
            .get("format")
            .unwrap()
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(valid_formats.contains(&format));
        println!("[SYNTHETIC TEST] get_topic_portfolio with format='verbose' is valid");
    }

    #[test]
    fn test_synthetic_get_topic_stability_boundary() {
        // Test at maximum boundary
        let synthetic_input = json!({"hours": 168});

        let tools = definitions();
        let stability = tools
            .iter()
            .find(|t| t.name == "get_topic_stability")
            .unwrap();
        let props = stability.input_schema.get("properties").unwrap();
        let hours_schema = props.get("hours").unwrap();

        let hours = synthetic_input.get("hours").unwrap().as_u64().unwrap();
        let max_hours = hours_schema.get("maximum").unwrap().as_u64().unwrap();
        assert!(hours <= max_hours);
        println!("[SYNTHETIC TEST] get_topic_stability with hours=168 is at max boundary");
    }

    #[test]
    fn test_synthetic_detect_topics_empty_input() {
        // Empty input should use defaults
        let synthetic_input = json!({});

        let tools = definitions();
        let detect = tools.iter().find(|t| t.name == "detect_topics").unwrap();
        let props = detect.input_schema.get("properties").unwrap();
        let force_default = props
            .get("force")
            .unwrap()
            .get("default")
            .unwrap()
            .as_bool()
            .unwrap();

        // When no input provided, force should default to false
        assert!(!force_default);
        assert!(synthetic_input.get("force").is_none());
        println!("[SYNTHETIC TEST] detect_topics with empty input uses force=false default");
    }

    #[test]
    fn test_synthetic_get_divergence_alerts_valid() {
        // Valid lookback_hours
        let synthetic_input = json!({"lookback_hours": 6});

        let tools = definitions();
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        let props = alerts.input_schema.get("properties").unwrap();
        let lookback_schema = props.get("lookback_hours").unwrap();

        let lookback = synthetic_input
            .get("lookback_hours")
            .unwrap()
            .as_u64()
            .unwrap();
        let min = lookback_schema.get("minimum").unwrap().as_u64().unwrap();
        let max = lookback_schema.get("maximum").unwrap().as_u64().unwrap();
        assert!(lookback >= min && lookback <= max);
        println!("[SYNTHETIC TEST] get_divergence_alerts with lookback_hours=6 is valid");
    }

    // ========== EDGE CASE TESTS ==========

    #[test]
    fn test_edge_case_minimum_hours() {
        let tools = definitions();
        let stability = tools
            .iter()
            .find(|t| t.name == "get_topic_stability")
            .unwrap();
        let props = stability.input_schema.get("properties").unwrap();
        let hours = props.get("hours").unwrap();

        assert_eq!(hours.get("minimum").unwrap().as_u64().unwrap(), 1);
        println!("[EDGE CASE] get_topic_stability minimum hours is 1");
    }

    #[test]
    fn test_edge_case_minimum_lookback() {
        let tools = definitions();
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        let props = alerts.input_schema.get("properties").unwrap();
        let lookback = props.get("lookback_hours").unwrap();

        assert_eq!(lookback.get("minimum").unwrap().as_u64().unwrap(), 1);
        println!("[EDGE CASE] get_divergence_alerts minimum lookback_hours is 1");
    }

    #[test]
    fn test_edge_case_maximum_lookback() {
        let tools = definitions();
        let alerts = tools
            .iter()
            .find(|t| t.name == "get_divergence_alerts")
            .unwrap();
        let props = alerts.input_schema.get("properties").unwrap();
        let lookback = props.get("lookback_hours").unwrap();

        assert_eq!(lookback.get("maximum").unwrap().as_u64().unwrap(), 48);
        println!("[EDGE CASE] get_divergence_alerts maximum lookback_hours is 48");
    }

    #[test]
    fn test_all_tools_have_type_object() {
        let tools = definitions();
        for tool in &tools {
            assert_eq!(
                tool.input_schema.get("type").unwrap().as_str().unwrap(),
                "object",
                "Tool {} should have type: object",
                tool.name
            );
        }
        println!("[PASS] All topic tools have type: object schema");
    }

    #[test]
    fn test_all_tools_have_properties() {
        let tools = definitions();
        for tool in &tools {
            assert!(
                tool.input_schema.get("properties").is_some(),
                "Tool {} should have properties",
                tool.name
            );
        }
        println!("[PASS] All topic tools have properties defined");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tools = definitions();
        let json_str = serde_json::to_string(&tools).expect("Serialization failed");
        assert!(json_str.contains("get_topic_portfolio"));
        assert!(json_str.contains("get_topic_stability"));
        assert!(json_str.contains("detect_topics"));
        assert!(json_str.contains("get_divergence_alerts"));
        assert!(json_str.contains("inputSchema"));
        println!("[PASS] Topic tools serialize correctly");
    }
}
