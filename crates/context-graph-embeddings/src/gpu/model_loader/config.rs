//! BERT model configuration parsing from config.json.
//!
//! Supports BERT and MPNet model architectures with sensible defaults
//! for missing configuration fields.

use serde::Deserialize;

/// BERT model configuration parsed from config.json.
#[derive(Debug, Clone, Deserialize)]
pub struct BertConfig {
    /// Vocabulary size (e.g., 30522 for BERT).
    pub vocab_size: usize,
    /// Hidden layer size (e.g., 768 for BERT-base, 1024 for BERT-large).
    pub hidden_size: usize,
    /// Number of hidden layers (e.g., 12 for BERT-base, 24 for BERT-large).
    pub num_hidden_layers: usize,
    /// Number of attention heads (e.g., 12 for BERT-base, 16 for BERT-large).
    pub num_attention_heads: usize,
    /// Intermediate FFN size (usually 4x hidden_size).
    pub intermediate_size: usize,
    /// Hidden activation function (gelu, relu, etc.).
    #[serde(default = "default_hidden_act")]
    pub hidden_act: String,
    /// Dropout probability for hidden layers.
    #[serde(default = "default_dropout")]
    pub hidden_dropout_prob: f64,
    /// Dropout probability for attention.
    #[serde(default = "default_dropout")]
    pub attention_probs_dropout_prob: f64,
    /// Maximum sequence length.
    #[serde(default = "default_max_position")]
    pub max_position_embeddings: usize,
    /// Token type vocabulary size (usually 2).
    #[serde(default = "default_type_vocab")]
    pub type_vocab_size: usize,
    /// Layer normalization epsilon.
    #[serde(default = "default_layer_norm_eps")]
    pub layer_norm_eps: f64,
    /// Padding token ID.
    #[serde(default)]
    pub pad_token_id: usize,
    /// Model type string (bert, mpnet, etc.).
    #[serde(default = "default_model_type")]
    pub model_type: String,
    /// Architecture list.
    #[serde(default)]
    pub architectures: Vec<String>,
}

fn default_hidden_act() -> String {
    "gelu".to_string()
}

fn default_dropout() -> f64 {
    0.1
}

fn default_max_position() -> usize {
    512
}

fn default_type_vocab() -> usize {
    2
}

fn default_layer_norm_eps() -> f64 {
    1e-12
}

fn default_model_type() -> String {
    "bert".to_string()
}

impl BertConfig {
    /// Check if this config represents a BERT-like architecture.
    pub fn is_bert(&self) -> bool {
        self.model_type == "bert"
            || self
                .architectures
                .iter()
                .any(|a| a.contains("Bert") || a.contains("bert"))
    }

    /// Check if this config represents an MPNet architecture.
    pub fn is_mpnet(&self) -> bool {
        self.model_type == "mpnet"
            || self
                .architectures
                .iter()
                .any(|a| a.contains("MPNet") || a.contains("mpnet"))
    }

    /// Check if this config represents an XLM-RoBERTa architecture.
    ///
    /// BGE-M3 (BAAI/bge-m3) ships with `model_type = "xlm-roberta"` and
    /// architectures listing `XLMRobertaModel`. The forward pass is
    /// architecturally identical to BERT — the only load-time differences
    /// are weight-key prefix (`roberta.`) and larger vocab + position ranges.
    pub fn is_xlm_roberta(&self) -> bool {
        self.model_type == "xlm-roberta"
            || self.model_type == "roberta"
            || self
                .architectures
                .iter()
                .any(|a| a.contains("XLMRoberta") || a.contains("RobertaModel"))
    }

    /// Check if this is a supported architecture.
    pub fn is_supported(&self) -> bool {
        self.is_bert() || self.is_mpnet() || self.is_xlm_roberta()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bert_config_defaults() {
        let json = r#"{
            "vocab_size": 30522,
            "hidden_size": 768,
            "num_hidden_layers": 12,
            "num_attention_heads": 12,
            "intermediate_size": 3072
        }"#;

        let config: BertConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hidden_act, "gelu");
        assert_eq!(config.max_position_embeddings, 512);
        assert_eq!(config.type_vocab_size, 2);
        assert!((config.layer_norm_eps - 1e-12).abs() < 1e-15);
    }

    #[test]
    fn test_bert_config_full() {
        let json = r#"{
            "vocab_size": 30522,
            "hidden_size": 1024,
            "num_hidden_layers": 24,
            "num_attention_heads": 16,
            "intermediate_size": 4096,
            "hidden_act": "gelu",
            "hidden_dropout_prob": 0.1,
            "attention_probs_dropout_prob": 0.1,
            "max_position_embeddings": 512,
            "type_vocab_size": 2,
            "layer_norm_eps": 1e-12,
            "pad_token_id": 0,
            "model_type": "bert",
            "architectures": ["BertModel"]
        }"#;

        let config: BertConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.vocab_size, 30522);
        assert_eq!(config.hidden_size, 1024);
        assert_eq!(config.num_hidden_layers, 24);
        assert_eq!(config.num_attention_heads, 16);
        assert_eq!(config.intermediate_size, 4096);
        assert_eq!(config.model_type, "bert");
        assert!(config.architectures.contains(&"BertModel".to_string()));
    }

    #[test]
    fn test_is_bert() {
        let json = r#"{
            "vocab_size": 30522,
            "hidden_size": 768,
            "num_hidden_layers": 12,
            "num_attention_heads": 12,
            "intermediate_size": 3072,
            "model_type": "bert"
        }"#;

        let config: BertConfig = serde_json::from_str(json).unwrap();
        assert!(config.is_bert());
        assert!(!config.is_mpnet());
        assert!(config.is_supported());
    }

    #[test]
    fn test_is_mpnet() {
        let json = r#"{
            "vocab_size": 30522,
            "hidden_size": 768,
            "num_hidden_layers": 12,
            "num_attention_heads": 12,
            "intermediate_size": 3072,
            "model_type": "mpnet"
        }"#;

        let config: BertConfig = serde_json::from_str(json).unwrap();
        assert!(!config.is_bert());
        assert!(config.is_mpnet());
        assert!(config.is_supported());
    }
}
