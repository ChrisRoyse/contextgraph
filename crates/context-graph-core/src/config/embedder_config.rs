//! Embedder configuration with name-based deserialization.
//!
//! This module provides custom serde deserializers that resolve embedder names
//! to `Embedder` enum variants via `Embedder::from_name()`.

use crate::teleological::Embedder;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Custom deserializer that resolves embedder names.
///
/// # Behavior
///
/// - Valid names (E8_Graph, E8, graph): deserialized to the correct variant
/// - Unknown names: returns error
///
/// # Example Config
///
/// ```toml
/// embedder = "E8_Graph"
/// ```
pub fn deserialize_embedder<'de, D>(deserializer: D) -> Result<Embedder, D::Error>
where
    D: Deserializer<'de>,
{
    let name = String::deserialize(deserializer)?;
    Embedder::from_name(&name).map_err(serde::de::Error::custom)
}

/// Custom deserializer for optional embedder.
pub fn deserialize_embedder_opt<'de, D>(deserializer: D) -> Result<Option<Embedder>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt_name: Option<String> = Option::deserialize(deserializer)?;
    match opt_name {
        Some(name) => Embedder::from_name(&name)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Configuration for a single embedder.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbedderConfig {
    /// Embedder type.
    #[serde(deserialize_with = "deserialize_embedder")]
    pub embedder: Embedder,

    /// Weight for this embedder in fusion [0.0, 1.0].
    #[serde(default = "default_weight")]
    pub weight: f32,

    /// Whether this embedder is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Model path override (optional).
    #[serde(default)]
    pub model_path: Option<String>,
}

fn default_weight() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl Default for EmbedderConfig {
    fn default() -> Self {
        Self {
            embedder: Embedder::Semantic,
            weight: 1.0,
            enabled: true,
            model_path: None,
        }
    }
}

/// Configuration for all embedder weights.
///
/// Supports both map and list formats:
///
/// ```toml
/// # Map format
/// [embedders.weights]
/// E1_Semantic = 1.0
/// E8_Graph = 0.5
///
/// # List format
/// [[embedders.configs]]
/// embedder = "E1_Semantic"
/// weight = 1.0
///
/// [[embedders.configs]]
/// embedder = "E8_Graph"
/// weight = 0.5
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EmbedderWeightsConfig {
    /// Weight map by embedder name.
    #[serde(default, deserialize_with = "deserialize_weight_map")]
    pub weights: HashMap<Embedder, f32>,

    /// Detailed embedder configurations.
    #[serde(default)]
    pub configs: Vec<EmbedderConfig>,
}

/// Custom deserializer for weight map.
fn deserialize_weight_map<'de, D>(deserializer: D) -> Result<HashMap<Embedder, f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let string_map: HashMap<String, f32> = HashMap::deserialize(deserializer)?;

    let mut result = HashMap::new();
    for (name, weight) in string_map {
        let embedder = Embedder::from_name(&name).map_err(serde::de::Error::custom)?;
        result.insert(embedder, weight);
    }

    Ok(result)
}

impl EmbedderWeightsConfig {
    /// Get weight for an embedder.
    ///
    /// Priority:
    /// 1. Detailed config (if exists)
    /// 2. Weight map
    /// 3. Default: 0.0 (unknown embedders should NOT participate in scoring)
    ///
    /// ERR-6 fix: Previously defaulted to 1.0 which amplified unconfigured embedders
    /// to maximum weight, corrupting fusion scores. Now returns 0.0 so unknown
    /// embedders are excluded from scoring.
    pub fn get_weight(&self, embedder: Embedder) -> f32 {
        // Check detailed configs first
        if let Some(config) = self.configs.iter().find(|c| c.embedder == embedder) {
            return config.weight;
        }

        // Check weight map — unknown embedders get 0.0 (excluded from scoring)
        self.weights.get(&embedder).copied().unwrap_or(0.0)
    }

    /// Check if an embedder is enabled.
    pub fn is_enabled(&self, embedder: Embedder) -> bool {
        if let Some(config) = self.configs.iter().find(|c| c.embedder == embedder) {
            return config.enabled;
        }
        true // Default enabled
    }

    /// Get all configured embedders.
    pub fn configured_embedders(&self) -> Vec<Embedder> {
        let mut embedders: Vec<Embedder> = self.weights.keys().copied().collect();
        for config in &self.configs {
            if !embedders.contains(&config.embedder) {
                embedders.push(config.embedder);
            }
        }
        embedders
    }
}

/// Check config file for deprecated embedder names.
///
/// Returns list of deprecated names found for migration guidance.
pub fn check_deprecated_names(config_content: &str) -> Vec<DeprecatedNameUsage> {
    let mut usages = Vec::new();

    // Check for E8_Emotional usage (old name, now renamed to E8_Graph)
    for (line_num, line) in config_content.lines().enumerate() {
        if line.contains("E8_Emotional") || line.contains("e8_emotional") {
            usages.push(DeprecatedNameUsage {
                line: line_num + 1,
                deprecated: "E8_Emotional".to_string(),
                canonical: "E8_Graph".to_string(),
                context: line.trim().to_string(),
            });
        }
    }

    usages
}

/// Record of deprecated name usage for migration.
#[derive(Debug, Clone)]
pub struct DeprecatedNameUsage {
    /// Line number (1-indexed).
    pub line: usize,
    /// Deprecated name found.
    pub deprecated: String,
    /// Canonical replacement.
    pub canonical: String,
    /// Line content for context.
    pub context: String,
}

impl std::fmt::Display for DeprecatedNameUsage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Line {}: Replace '{}' with '{}' (context: {})",
            self.line, self.deprecated, self.canonical, self.context
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_canonical_name() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(deserialize_with = "deserialize_embedder")]
            embedder: Embedder,
        }

        let json = r#"{"embedder": "E8_Graph"}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.embedder, Embedder::Graph);
        println!("[PASS] Canonical E8_Graph deserializes correctly");
    }

    #[test]
    fn test_deserialize_old_emotional_fails() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(deserialize_with = "deserialize_embedder")]
            #[allow(dead_code)]
            embedder: Embedder,
        }

        // E8_Emotional is no longer valid -- fail fast
        let json = r#"{"embedder": "E8_Emotional"}"#;
        let result: Result<TestConfig, _> = serde_json::from_str(json);
        assert!(result.is_err(), "E8_Emotional should fail deserialization");
        println!("[PASS] Old E8_Emotional name fails deserialization (no backwards compat)");
    }

    #[test]
    fn test_deserialize_short_name() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(deserialize_with = "deserialize_embedder")]
            embedder: Embedder,
        }

        let json = r#"{"embedder": "E8"}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.embedder, Embedder::Graph);
        println!("[PASS] Short name E8 deserializes to Graph");
    }

    #[test]
    fn test_deserialize_invalid_name() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(deserialize_with = "deserialize_embedder")]
            #[allow(dead_code)]
            embedder: Embedder,
        }

        let json = r#"{"embedder": "InvalidEmbedder"}"#;
        let result: Result<TestConfig, _> = serde_json::from_str(json);
        assert!(result.is_err());
        println!("[PASS] Invalid embedder name fails deserialization");
    }

    #[test]
    fn test_deserialize_all_embedders() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(deserialize_with = "deserialize_embedder")]
            embedder: Embedder,
        }

        let test_cases = [
            ("E1_Semantic", Embedder::Semantic),
            ("E2_Temporal_Recent", Embedder::TemporalRecent),
            ("E3_Temporal_Periodic", Embedder::TemporalPeriodic),
            ("E4_Temporal_Positional", Embedder::TemporalPositional),
            ("E5_Causal", Embedder::Causal),
            ("E6_Sparse_Lexical", Embedder::Sparse),
            ("E7_Code", Embedder::Code),
            ("E8_Graph", Embedder::Graph),
            ("E9_HDC", Embedder::Hdc),
            ("E10_Multimodal", Embedder::Multimodal),
            ("E11_Entity", Embedder::Entity),
            ("E12_Late_Interaction", Embedder::LateInteraction),
            ("E13_SPLADE", Embedder::KeywordSplade),
        ];

        for (name, expected) in test_cases {
            let json = format!(r#"{{"embedder": "{}"}}"#, name);
            let config: TestConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(config.embedder, expected, "Failed for {}", name);
        }
        println!("[PASS] All 13 embedder names deserialize correctly");
    }

    #[test]
    fn test_embedder_config_default() {
        let config = EmbedderConfig::default();
        assert_eq!(config.embedder, Embedder::Semantic);
        assert!((config.weight - 1.0).abs() < f32::EPSILON);
        assert!(config.enabled);
        assert!(config.model_path.is_none());
        println!("[PASS] EmbedderConfig default values correct");
    }

    #[test]
    fn test_embedder_weights_config_get_weight() {
        let mut config = EmbedderWeightsConfig::default();
        config.weights.insert(Embedder::Graph, 0.5);

        assert!((config.get_weight(Embedder::Graph) - 0.5).abs() < f32::EPSILON);
        // ERR-6: Unknown embedders default to 0.0 (excluded from scoring), not 1.0
        assert!((config.get_weight(Embedder::Semantic) - 0.0).abs() < f32::EPSILON);
        println!("[PASS] get_weight returns configured weight and 0.0 for unconfigured");
    }

    #[test]
    fn test_embedder_weights_config_from_json() {
        let json = r#"{
            "weights": {
                "E1_Semantic": 1.0,
                "E8_Graph": 0.5
            }
        }"#;

        let config: EmbedderWeightsConfig = serde_json::from_str(json).unwrap();
        assert!((config.get_weight(Embedder::Semantic) - 1.0).abs() < f32::EPSILON);
        assert!((config.get_weight(Embedder::Graph) - 0.5).abs() < f32::EPSILON);
        println!("[PASS] EmbedderWeightsConfig deserializes from JSON");
    }

    #[test]
    fn test_embedder_weights_config_with_configs() {
        let json = r#"{
            "weights": {},
            "configs": [
                {"embedder": "E1_Semantic", "weight": 0.8, "enabled": true},
                {"embedder": "E8_Graph", "weight": 0.3, "enabled": false}
            ]
        }"#;

        let config: EmbedderWeightsConfig = serde_json::from_str(json).unwrap();
        assert!((config.get_weight(Embedder::Semantic) - 0.8).abs() < f32::EPSILON);
        assert!((config.get_weight(Embedder::Graph) - 0.3).abs() < f32::EPSILON);
        assert!(config.is_enabled(Embedder::Semantic));
        assert!(!config.is_enabled(Embedder::Graph));
        println!("[PASS] Detailed configs override weight map");
    }

    #[test]
    fn test_check_deprecated_names() {
        let config = r#"
            [embedders]
            E1_Semantic = 1.0
            E8_Emotional = 0.5
            E8_Graph = 0.3
        "#;

        let usages = check_deprecated_names(config);
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].deprecated, "E8_Emotional");
        assert_eq!(usages[0].canonical, "E8_Graph");
        println!("[PASS] check_deprecated_names finds E8_Emotional usage");
    }

    #[test]
    fn test_check_deprecated_names_none() {
        let config = r#"
            [embedders]
            E1_Semantic = 1.0
            E8_Graph = 0.5
        "#;

        let usages = check_deprecated_names(config);
        assert!(usages.is_empty());
        println!("[PASS] check_deprecated_names returns empty for clean config");
    }

    #[test]
    fn test_check_deprecated_names_lowercase() {
        let config = r#"
            embedder = "e8_emotional"
        "#;

        let usages = check_deprecated_names(config);
        assert_eq!(usages.len(), 1);
        println!("[PASS] check_deprecated_names finds lowercase e8_emotional");
    }

    #[test]
    fn test_configured_embedders() {
        let mut config = EmbedderWeightsConfig::default();
        config.weights.insert(Embedder::Semantic, 1.0);
        config.weights.insert(Embedder::Graph, 0.5);

        let embedders = config.configured_embedders();
        assert!(embedders.contains(&Embedder::Semantic));
        assert!(embedders.contains(&Embedder::Graph));
        assert_eq!(embedders.len(), 2);
        println!("[PASS] configured_embedders returns correct list");
    }

    #[test]
    fn test_configured_embedders_includes_configs() {
        let json = r#"{
            "weights": {"E1_Semantic": 1.0},
            "configs": [{"embedder": "E8_Graph", "weight": 0.5}]
        }"#;

        let config: EmbedderWeightsConfig = serde_json::from_str(json).unwrap();
        let embedders = config.configured_embedders();
        assert!(embedders.contains(&Embedder::Semantic));
        assert!(embedders.contains(&Embedder::Graph));
        println!("[PASS] configured_embedders includes both weights and configs");
    }

    #[test]
    fn test_deprecated_name_usage_display() {
        let usage = DeprecatedNameUsage {
            line: 5,
            deprecated: "E8_Emotional".to_string(),
            canonical: "E8_Graph".to_string(),
            context: "embedder = \"E8_Emotional\"".to_string(),
        };

        let display = format!("{}", usage);
        assert!(display.contains("Line 5"));
        assert!(display.contains("E8_Emotional"));
        assert!(display.contains("E8_Graph"));
        println!("[PASS] DeprecatedNameUsage Display trait works");
    }

    #[test]
    fn test_deserialize_embedder_opt_some() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(default, deserialize_with = "deserialize_embedder_opt")]
            embedder: Option<Embedder>,
        }

        let json = r#"{"embedder": "E8_Graph"}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.embedder, Some(Embedder::Graph));
        println!("[PASS] Optional embedder deserializes Some value");
    }

    #[test]
    fn test_deserialize_embedder_opt_none() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(default, deserialize_with = "deserialize_embedder_opt")]
            embedder: Option<Embedder>,
        }

        let json = r#"{"embedder": null}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.embedder, None);
        println!("[PASS] Optional embedder deserializes None value");
    }
}
