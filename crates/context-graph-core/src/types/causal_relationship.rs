//! CausalRelationship type for storing LLM-generated causal descriptions with provenance.
//!
//! This module defines the [`CausalRelationship`] struct that stores:
//! - LLM-generated 1-3 paragraph causal explanations
//! - E5 dual embeddings (as_cause, as_effect) for asymmetric search (768D each)
//! - E1 semantic embedding for fallback search (1024D)
//! - Full provenance linking back to source content and fingerprint
//!
//! # Storage
//!
//! CausalRelationships are stored in dedicated RocksDB column families:
//! - `CF_CAUSAL_RELATIONSHIPS`: Primary storage by UUID
//! - `CF_CAUSAL_BY_SOURCE`: Secondary index by source fingerprint ID
//! - `CF_CAUSAL_E5_CAUSE_INDEX`: HNSW index for e5_as_cause vectors
//! - `CF_CAUSAL_E5_EFFECT_INDEX`: HNSW index for e5_as_effect vectors
//!
//! # Search
//!
//! E5 dual embeddings enable asymmetric causal search:
//! - "What caused X?" → Query as effect, search cause index
//! - "What are effects of X?" → Query as cause, search effect index
//!
//! E1 embedding provides fallback semantic search for general queries.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// LLM PROVENANCE (Phase 1.3)
// ============================================================================

/// Provenance metadata for LLM-generated content.
///
/// Tracks which model, version, and parameters produced a given relationship.
/// This enables auditing, reproducibility, and debugging of LLM-generated
/// causal relationships.
///
/// # Example
///
/// ```ignore
/// let provenance = LLMProvenance {
///     model_name: "Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf".to_string(),
///     model_version: "Q5_K_M".to_string(),
///     quantization: "Q5_K_M".to_string(),
///     temperature: 0.0,
///     max_tokens: 512,
///     prompt_template_hash: "a1b2c3...".to_string(),
///     grammar_type: Some("causal_analysis".to_string()),
///     tokens_consumed: Some(147),
///     generation_time_ms: Some(1250),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LLMProvenance {
    /// Model display name (e.g., "Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf")
    pub model_name: String,

    /// Version tag or file hash of the model
    pub model_version: String,

    /// Quantization method (e.g., "Q5_K_M")
    pub quantization: String,

    /// Temperature used for generation
    pub temperature: f32,

    /// Maximum tokens allowed for generation
    pub max_tokens: usize,

    /// SHA-256 hash of the prompt template used
    pub prompt_template_hash: String,

    /// Grammar type used: "causal_analysis", "graph_relationship", etc.
    #[serde(default)]
    pub grammar_type: Option<String>,

    /// Actual tokens consumed during generation
    #[serde(default)]
    pub tokens_consumed: Option<u32>,

    /// LLM inference time in milliseconds
    #[serde(default)]
    pub generation_time_ms: Option<u64>,
}

impl LLMProvenance {
    /// Create a new LLMProvenance with required fields.
    pub fn new(
        model_name: String,
        model_version: String,
        quantization: String,
        temperature: f32,
        max_tokens: usize,
        prompt_template_hash: String,
    ) -> Self {
        Self {
            model_name,
            model_version,
            quantization,
            temperature,
            max_tokens,
            prompt_template_hash,
            grammar_type: None,
            tokens_consumed: None,
            generation_time_ms: None,
        }
    }

    /// Add grammar type.
    pub fn with_grammar_type(mut self, grammar_type: impl Into<String>) -> Self {
        self.grammar_type = Some(grammar_type.into());
        self
    }

    /// Add token consumption metadata.
    pub fn with_tokens_consumed(mut self, tokens: u32) -> Self {
        self.tokens_consumed = Some(tokens);
        self
    }

    /// Add generation timing metadata.
    pub fn with_generation_time_ms(mut self, time_ms: u64) -> Self {
        self.generation_time_ms = Some(time_ms);
        self
    }
}

// ============================================================================
// SOURCE SPAN TYPES (Provenance Tracking)
// ============================================================================

/// A character span in the source text where a causal relationship was found.
///
/// Provides fine-grained provenance tracking by recording exact character offsets
/// into the source content. This enables displaying the exact text that was
/// used to derive a causal relationship.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CausalSourceSpan {
    /// Character offset from start of source content (0-based).
    pub start_char: usize,

    /// Character offset for end of span (exclusive).
    pub end_char: usize,

    /// The exact text excerpt (truncated to 200 chars).
    /// This MUST be copied exactly from the source, not paraphrased.
    pub text_excerpt: String,

    /// What this span represents: "cause", "effect", or "full".
    pub span_type: String,
}

impl CausalSourceSpan {
    /// Create a new causal source span.
    pub fn new(
        start_char: usize,
        end_char: usize,
        text_excerpt: impl Into<String>,
        span_type: impl Into<String>,
    ) -> Self {
        let excerpt = text_excerpt.into();
        // Truncate to 200 chars if needed
        let truncated = if excerpt.chars().count() > 200 {
            excerpt.chars().take(200).collect::<String>() + "..."
        } else {
            excerpt
        };
        Self {
            start_char,
            end_char,
            text_excerpt: truncated,
            span_type: span_type.into(),
        }
    }

    /// Check if the offsets are within bounds of a given text length.
    pub fn is_valid_for(&self, text_len: usize) -> bool {
        self.start_char < self.end_char && self.end_char <= text_len
    }

    /// Validate that text_excerpt matches the source text at the given offsets.
    ///
    /// Returns true if the excerpt matches the actual source text (allowing for
    /// truncation with "...").
    pub fn matches_source(&self, source_text: &str) -> bool {
        if self.end_char > source_text.len() {
            return false;
        }
        let actual = &source_text[self.start_char..self.end_char];
        // Allow for truncation - check if excerpt starts with actual or vice versa
        self.text_excerpt.trim() == actual.trim()
            || actual.starts_with(self.text_excerpt.trim_end_matches("..."))
    }

    /// Check if this is a "cause" span.
    pub fn is_cause_span(&self) -> bool {
        self.span_type == "cause"
    }

    /// Check if this is an "effect" span.
    pub fn is_effect_span(&self) -> bool {
        self.span_type == "effect"
    }

    /// Check if this is a "full" span.
    pub fn is_full_span(&self) -> bool {
        self.span_type == "full"
    }
}

// ============================================================================
// CAUSAL RELATIONSHIP
// ============================================================================

/// A causal relationship identified by LLM analysis.
///
/// Contains the LLM-generated explanation with E5 dual embeddings for
/// asymmetric causal search, plus full provenance linking back to source.
///
/// # Embeddings
///
/// - `e5_as_cause` (768D): Explanation embedded as a cause
/// - `e5_as_effect` (768D): Explanation embedded as an effect
/// - `e1_semantic` (1024D): Fallback semantic embedding
///
/// # Search Strategy
///
/// For "What caused X?":
/// - Query X using `embed_as_effect`
/// - Search `e5_as_cause` index
/// - Returns relationships where explanation acts as a cause of X
///
/// For "What are effects of X?":
/// - Query X using `embed_as_cause`
/// - Search `e5_as_effect` index
/// - Returns relationships where explanation is an effect of X
///
/// # Example
///
/// ```ignore
/// let rel = CausalRelationship::new(
///     "Chronic stress elevates cortisol".to_string(),  // cause
///     "Memory impairment".to_string(),                 // effect
///     "High cortisol causes memory impairment...".to_string(), // explanation
///     e5_cause, e5_effect, e1_semantic,
///     source_id,
///     0.9,
///     "mediated".to_string(),
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalRelationship {
    /// Unique ID for this causal relationship.
    pub id: Uuid,

    // ===== CAUSAL CONTENT =====

    /// Brief statement of the cause (e.g., "Chronic stress elevates cortisol")
    pub cause_statement: String,

    /// Brief statement of the effect (e.g., "Memory impairment")
    pub effect_statement: String,

    /// LLM-generated explanation (1-2 paragraphs).
    ///
    /// Structure:
    /// - Paragraph 1: What is the causal relationship and why
    /// - Paragraph 2: Mechanism/evidence details
    pub explanation: String,

    // ===== E5 DUAL EMBEDDINGS (768D each) =====

    /// E5 embedding of explanation as a CAUSE (768D).
    ///
    /// Used when searching for effects: "What does X cause?"
    pub e5_as_cause: Vec<f32>,

    /// E5 embedding of explanation as an EFFECT (768D).
    ///
    /// Used when searching for causes: "What caused X?"
    pub e5_as_effect: Vec<f32>,

    // ===== SOURCE-ANCHORED E5 EMBEDDINGS (768D each) =====
    //
    // These embeddings anchor to the unique source content rather than the
    // LLM-generated explanation. This prevents clustering of explanations
    // that share similar prompt structure but come from different documents.
    //
    // Format: source_content + cause_statement + effect_statement

    /// E5 embedding of source-anchored text as a CAUSE (768D).
    ///
    /// Source-anchored text = source_content + cause/effect statements.
    /// Used to prevent LLM explanation clustering by anchoring to unique source.
    #[serde(default)]
    pub e5_source_cause: Vec<f32>,

    /// E5 embedding of source-anchored text as an EFFECT (768D).
    ///
    /// Source-anchored text = source_content + cause/effect statements.
    /// Used to prevent LLM explanation clustering by anchoring to unique source.
    #[serde(default)]
    pub e5_source_effect: Vec<f32>,

    // ===== E1 SEMANTIC EMBEDDING (1024D) =====

    /// E1 semantic embedding of the explanation (1024D).
    ///
    /// Provides fallback for general semantic search.
    pub e1_semantic: Vec<f32>,

    // ===== E8 GRAPH EMBEDDINGS (1024D each) =====
    //
    // E8 graph embeddings enable graph structure search for causal relationships.
    // These use the same directional approach as E5 (asymmetric similarity):
    // - e8_graph_source: "What does this cause?" (relationship as source)
    // - e8_graph_target: "What causes this?" (relationship as target)

    /// E8 embedding of causal structure as a SOURCE (1024D).
    ///
    /// Used when searching for targets: "What does X cause?"
    #[serde(default)]
    pub e8_graph_source: Vec<f32>,

    /// E8 embedding of causal structure as a TARGET (1024D).
    ///
    /// Used when searching for sources: "What causes X?"
    #[serde(default)]
    pub e8_graph_target: Vec<f32>,

    // ===== E11 ENTITY EMBEDDING (768D) =====
    //
    // E11 KEPLER embeddings enable entity-based search using TransE knowledge
    // graph operations. KEPLER is RoBERTa-base trained with TransE on Wikidata5M.
    // Unlike E1, KEPLER knows that "Diesel" is a Rust database ORM, not fuel.

    /// E11 KEPLER entity embedding (768D).
    ///
    /// Uses concatenated cause|effect|explanation for entity context.
    /// Enables entity-based search (e.g., "Diesel" → "Rust database ORM").
    #[serde(default)]
    pub e11_entity: Vec<f32>,

    // ===== METADATA =====

    /// LLM confidence score [0.0, 1.0].
    pub confidence: f32,

    /// Type of causal mechanism: "direct", "mediated", "feedback", "temporal"
    pub mechanism_type: String,

    // ===== PROVENANCE =====

    /// Original source content this was derived from.
    pub source_content: String,

    /// Fingerprint ID of the source memory.
    pub source_fingerprint_id: Uuid,

    /// Unix timestamp when relationship was identified.
    pub created_at: i64,

    /// Character spans in source_content where this relationship was extracted.
    /// Provides fine-grained provenance for displaying the exact source text.
    #[serde(default)]
    pub source_spans: Vec<CausalSourceSpan>,

    /// LLM provenance metadata tracking which model generated this relationship (Phase 1.3).
    /// None if relationship was created before Phase 1.3 or not generated by LLM.
    #[serde(default)]
    pub llm_provenance: Option<LLMProvenance>,
}

impl CausalRelationship {
    /// E5 embedding dimension (768D per constitution.yaml).
    pub const E5_DIM: usize = 768;

    /// E1 embedding dimension (1024D per constitution.yaml).
    pub const E1_DIM: usize = 1024;

    /// E8 embedding dimension (1024D per constitution.yaml upgrade).
    pub const E8_DIM: usize = 1024;

    /// E11 entity embedding dimension (768D KEPLER per constitution.yaml).
    pub const E11_DIM: usize = 768;

    /// Create a new causal relationship with all embeddings.
    ///
    /// # Arguments
    ///
    /// * `cause_statement` - Brief statement of the cause
    /// * `effect_statement` - Brief statement of the effect
    /// * `explanation` - LLM-generated 1-2 paragraph explanation
    /// * `e5_as_cause` - E5 768D embedding as cause
    /// * `e5_as_effect` - E5 768D embedding as effect
    /// * `e1_semantic` - E1 1024D semantic embedding
    /// * `source_content` - Original text this was derived from
    /// * `source_fingerprint_id` - UUID of the source memory
    /// * `confidence` - LLM confidence score [0.0, 1.0]
    /// * `mechanism_type` - "direct", "mediated", "feedback", or "temporal"
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cause_statement: String,
        effect_statement: String,
        explanation: String,
        e5_as_cause: Vec<f32>,
        e5_as_effect: Vec<f32>,
        e1_semantic: Vec<f32>,
        source_content: String,
        source_fingerprint_id: Uuid,
        confidence: f32,
        mechanism_type: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            cause_statement,
            effect_statement,
            explanation,
            e5_as_cause,
            e5_as_effect,
            e5_source_cause: Vec::new(),
            e5_source_effect: Vec::new(),
            e1_semantic,
            e8_graph_source: Vec::new(),
            e8_graph_target: Vec::new(),
            e11_entity: Vec::new(),
            confidence: confidence.clamp(0.0, 1.0),
            mechanism_type,
            source_content,
            source_fingerprint_id,
            created_at: chrono::Utc::now().timestamp(),
            source_spans: Vec::new(),
            llm_provenance: None,
        }
    }

    /// Create with source spans for provenance tracking.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_spans(
        cause_statement: String,
        effect_statement: String,
        explanation: String,
        e5_as_cause: Vec<f32>,
        e5_as_effect: Vec<f32>,
        e1_semantic: Vec<f32>,
        source_content: String,
        source_fingerprint_id: Uuid,
        confidence: f32,
        mechanism_type: String,
        source_spans: Vec<CausalSourceSpan>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            cause_statement,
            effect_statement,
            explanation,
            e5_as_cause,
            e5_as_effect,
            e5_source_cause: Vec::new(),
            e5_source_effect: Vec::new(),
            e1_semantic,
            e8_graph_source: Vec::new(),
            e8_graph_target: Vec::new(),
            e11_entity: Vec::new(),
            confidence: confidence.clamp(0.0, 1.0),
            mechanism_type,
            source_content,
            source_fingerprint_id,
            created_at: chrono::Utc::now().timestamp(),
            source_spans,
            llm_provenance: None,
        }
    }

    /// Create with a specific ID (for testing or migration).
    #[allow(clippy::too_many_arguments)]
    pub fn with_id(
        id: Uuid,
        cause_statement: String,
        effect_statement: String,
        explanation: String,
        e5_as_cause: Vec<f32>,
        e5_as_effect: Vec<f32>,
        e1_semantic: Vec<f32>,
        source_content: String,
        source_fingerprint_id: Uuid,
        confidence: f32,
        mechanism_type: String,
    ) -> Self {
        Self {
            id,
            cause_statement,
            effect_statement,
            explanation,
            e5_as_cause,
            e5_as_effect,
            e5_source_cause: Vec::new(),
            e5_source_effect: Vec::new(),
            e1_semantic,
            e8_graph_source: Vec::new(),
            e8_graph_target: Vec::new(),
            e11_entity: Vec::new(),
            confidence: confidence.clamp(0.0, 1.0),
            mechanism_type,
            source_content,
            source_fingerprint_id,
            created_at: chrono::Utc::now().timestamp(),
            source_spans: Vec::new(),
            llm_provenance: None,
        }
    }

    /// Add source spans to an existing relationship.
    pub fn with_source_spans(mut self, spans: Vec<CausalSourceSpan>) -> Self {
        self.source_spans = spans;
        self
    }

    /// Add source-anchored E5 embeddings to an existing relationship.
    ///
    /// Source-anchored embeddings are derived from the original source content
    /// rather than the LLM-generated explanation, preventing clustering of
    /// relationships that share similar explanation structure.
    ///
    /// # Arguments
    /// * `source_cause` - E5 768D embedding of source-anchored text as cause
    /// * `source_effect` - E5 768D embedding of source-anchored text as effect
    pub fn with_source_embeddings(
        mut self,
        source_cause: Vec<f32>,
        source_effect: Vec<f32>,
    ) -> Self {
        self.e5_source_cause = source_cause;
        self.e5_source_effect = source_effect;
        self
    }

    /// Set source-anchored E5 embeddings in place.
    ///
    /// Mutates the relationship to add source-anchored embeddings.
    pub fn set_source_embeddings(&mut self, source_cause: Vec<f32>, source_effect: Vec<f32>) {
        self.e5_source_cause = source_cause;
        self.e5_source_effect = source_effect;
    }

    /// Check if source-anchored embeddings are present and valid.
    ///
    /// Returns true if both e5_source_cause and e5_source_effect have
    /// the expected 768D dimensionality.
    pub fn has_source_embeddings(&self) -> bool {
        self.e5_source_cause.len() == Self::E5_DIM && self.e5_source_effect.len() == Self::E5_DIM
    }

    /// Get the source-anchored E5 cause embedding as a slice.
    ///
    /// Returns an empty slice if source embeddings are not set.
    pub fn e5_source_cause_embedding(&self) -> &[f32] {
        &self.e5_source_cause
    }

    /// Get the source-anchored E5 effect embedding as a slice.
    ///
    /// Returns an empty slice if source embeddings are not set.
    pub fn e5_source_effect_embedding(&self) -> &[f32] {
        &self.e5_source_effect
    }

    /// Build the source-anchored text for embedding.
    ///
    /// Combines source content with cause/effect statements to create a unique
    /// text that anchors to the original document rather than LLM patterns.
    ///
    /// Format: "{source_content} {cause_statement} causes {effect_statement}."
    pub fn build_source_anchored_text(&self) -> String {
        // Truncate source content to ~500 chars to keep embedding focused
        let source_truncated: String = self.source_content.chars().take(500).collect();
        format!(
            "{} {} causes {}.",
            source_truncated, self.cause_statement, self.effect_statement
        )
    }

    /// Add E8 graph embeddings to an existing relationship.
    ///
    /// E8 graph embeddings enable graph structure search using asymmetric similarity
    /// (similar to E5 causal embeddings).
    ///
    /// # Arguments
    /// * `graph_source` - E8 1024D embedding as graph source
    /// * `graph_target` - E8 1024D embedding as graph target
    pub fn with_graph_embeddings(mut self, graph_source: Vec<f32>, graph_target: Vec<f32>) -> Self {
        self.e8_graph_source = graph_source;
        self.e8_graph_target = graph_target;
        self
    }

    /// Set E8 graph embeddings in place.
    ///
    /// Mutates the relationship to add graph embeddings.
    pub fn set_graph_embeddings(&mut self, graph_source: Vec<f32>, graph_target: Vec<f32>) {
        self.e8_graph_source = graph_source;
        self.e8_graph_target = graph_target;
    }

    /// Check if E8 graph embeddings are present and valid.
    ///
    /// Returns true if both e8_graph_source and e8_graph_target have
    /// the expected 1024D dimensionality.
    pub fn has_graph_embeddings(&self) -> bool {
        self.e8_graph_source.len() == Self::E8_DIM && self.e8_graph_target.len() == Self::E8_DIM
    }

    /// Get the E8 graph source embedding as a slice.
    ///
    /// Returns an empty slice if graph embeddings are not set.
    pub fn e8_graph_source_embedding(&self) -> &[f32] {
        &self.e8_graph_source
    }

    /// Get the E8 graph target embedding as a slice.
    ///
    /// Returns an empty slice if graph embeddings are not set.
    pub fn e8_graph_target_embedding(&self) -> &[f32] {
        &self.e8_graph_target
    }

    /// Add E11 KEPLER entity embedding to an existing relationship.
    ///
    /// E11 (KEPLER) enables entity-based search using TransE knowledge graph
    /// operations. KEPLER is trained on Wikidata5M and knows entity relationships
    /// that E1 misses (e.g., "Diesel" = Rust database ORM, not fuel).
    ///
    /// # Arguments
    /// * `e11_entity` - E11 768D entity embedding
    pub fn with_entity_embedding(mut self, e11_entity: Vec<f32>) -> Self {
        self.e11_entity = e11_entity;
        self
    }

    /// Set E11 entity embedding in place.
    ///
    /// Mutates the relationship to add entity embedding.
    pub fn set_entity_embedding(&mut self, e11_entity: Vec<f32>) {
        self.e11_entity = e11_entity;
    }

    /// Check if E11 entity embedding is present and valid.
    ///
    /// Returns true if e11_entity has the expected 768D dimensionality.
    pub fn has_entity_embedding(&self) -> bool {
        self.e11_entity.len() == Self::E11_DIM
    }

    /// Get the E11 entity embedding as a slice.
    ///
    /// Returns an empty slice if entity embedding is not set.
    pub fn e11_embedding(&self) -> &[f32] {
        &self.e11_entity
    }

    /// Check if all embeddings have the expected dimensions.
    ///
    /// - E5 embeddings: 768D
    /// - E1 embedding: 1024D
    pub fn has_valid_embeddings(&self) -> bool {
        self.e5_as_cause.len() == Self::E5_DIM
            && self.e5_as_effect.len() == Self::E5_DIM
            && self.e1_semantic.len() == Self::E1_DIM
    }

    /// Get the E5 cause embedding as a slice.
    pub fn e5_cause_embedding(&self) -> &[f32] {
        &self.e5_as_cause
    }

    /// Get the E5 effect embedding as a slice.
    pub fn e5_effect_embedding(&self) -> &[f32] {
        &self.e5_as_effect
    }

    /// Get the E1 semantic embedding as a slice.
    pub fn e1_embedding(&self) -> &[f32] {
        &self.e1_semantic
    }

    /// Check if this is a high-confidence relationship (>= 0.7).
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.7
    }

    /// Get the mechanism type as a normalized string.
    pub fn normalized_mechanism_type(&self) -> &str {
        match self.mechanism_type.to_lowercase().as_str() {
            "direct" => "direct",
            "mediated" | "indirect" => "mediated",
            "feedback" | "bidirectional" | "loop" => "feedback",
            "temporal" | "sequence" => "temporal",
            _ => "direct",
        }
    }

    // ===== PROVENANCE METHODS =====

    /// Check if this relationship has provenance tracking (source spans).
    pub fn has_provenance(&self) -> bool {
        !self.source_spans.is_empty()
    }

    /// Validate all source spans against the source content.
    ///
    /// Returns true if all spans have valid offsets and their excerpts
    /// match the actual source content.
    pub fn validate_spans(&self) -> bool {
        if self.source_spans.is_empty() {
            return false;
        }
        let source_len = self.source_content.len();
        self.source_spans.iter().all(|span| {
            span.is_valid_for(source_len) && span.matches_source(&self.source_content)
        })
    }

    /// Get spans of a specific type.
    pub fn spans_of_type(&self, span_type: &str) -> Vec<&CausalSourceSpan> {
        self.source_spans
            .iter()
            .filter(|s| s.span_type == span_type)
            .collect()
    }

    /// Get the "cause" span(s).
    pub fn cause_spans(&self) -> Vec<&CausalSourceSpan> {
        self.spans_of_type("cause")
    }

    /// Get the "effect" span(s).
    pub fn effect_spans(&self) -> Vec<&CausalSourceSpan> {
        self.spans_of_type("effect")
    }

    /// Get the "full" span(s).
    pub fn full_spans(&self) -> Vec<&CausalSourceSpan> {
        self.spans_of_type("full")
    }
}

// ============================================================================
// CAUSAL SEARCH RESULT (Multi-Embedder Search Output)
// ============================================================================

/// Configuration for multi-embedder causal search.
///
/// Controls which embedders to use and their relative weights in RRF fusion.
#[derive(Debug, Clone)]
pub struct MultiEmbedderConfig {
    /// Weight for E1 semantic search (default: 0.30)
    pub e1_weight: f32,
    /// Weight for E5 causal search (default: 0.35 - highest for causal queries)
    pub e5_weight: f32,
    /// Weight for E8 graph structure search (default: 0.15)
    pub e8_weight: f32,
    /// Weight for E11 entity search (default: 0.20)
    pub e11_weight: f32,
    /// Enable E12 MaxSim reranking (default: false for speed)
    pub enable_e12_rerank: bool,
    /// Minimum consensus threshold (0.0-1.0) for result inclusion
    pub min_consensus: f32,
}

impl Default for MultiEmbedderConfig {
    fn default() -> Self {
        Self {
            e1_weight: 0.30,
            e5_weight: 0.35,
            e8_weight: 0.15,
            e11_weight: 0.20,
            enable_e12_rerank: false,
            min_consensus: 0.0,
        }
    }
}

impl MultiEmbedderConfig {
    /// Create a new config with default weights optimized for causal search accuracy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create config with custom weights.
    pub fn with_weights(e1: f32, e5: f32, e8: f32, e11: f32) -> Self {
        Self {
            e1_weight: e1,
            e5_weight: e5,
            e8_weight: e8,
            e11_weight: e11,
            ..Self::default()
        }
    }

    /// Enable E12 MaxSim reranking for maximum precision (slower).
    pub fn with_e12_rerank(mut self) -> Self {
        self.enable_e12_rerank = true;
        self
    }

    /// Set minimum consensus threshold.
    pub fn with_min_consensus(mut self, threshold: f32) -> Self {
        self.min_consensus = threshold.clamp(0.0, 1.0);
        self
    }
}

/// Result from multi-embedder causal search.
///
/// Contains the causal relationship with per-embedder scores,
/// fusion scores, and consensus metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalSearchResult {
    /// The causal relationship ID.
    pub id: Uuid,

    /// The full causal relationship (optional, populated on demand).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship: Option<CausalRelationship>,

    // ===== PER-EMBEDDER SCORES =====

    /// E1 semantic similarity score [0.0, 1.0].
    pub e1_score: f32,

    /// E5 causal similarity score [0.0, 1.0] (directional).
    pub e5_score: f32,

    /// E8 graph structure similarity score [0.0, 1.0].
    pub e8_score: f32,

    /// E11 entity similarity score [0.0, 1.0].
    pub e11_score: f32,

    // ===== FUSION SCORES =====

    /// Weighted RRF fusion score (per ARCH-21).
    pub rrf_score: f32,

    /// E12 MaxSim reranking score (if reranking enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxsim_score: Option<f32>,

    // ===== VALIDATION SCORES =====

    /// TransE knowledge graph validation score.
    /// More negative = more valid relationship.
    pub transe_confidence: f32,

    // ===== CONSENSUS METRICS =====

    /// Consensus score: fraction of embedders that agree this is a good match.
    /// Range [0.0, 1.0] where 1.0 = all 4 embedders agree.
    pub consensus_score: f32,

    /// Direction confidence: how confident we are in the cause→effect direction.
    /// Higher = more confident in directionality.
    pub direction_confidence: f32,
}

impl CausalSearchResult {
    /// Create a new search result with per-embedder scores.
    pub fn new(id: Uuid, e1: f32, e5: f32, e8: f32, e11: f32) -> Self {
        Self {
            id,
            relationship: None,
            e1_score: e1,
            e5_score: e5,
            e8_score: e8,
            e11_score: e11,
            rrf_score: 0.0,
            maxsim_score: None,
            transe_confidence: 0.0,
            consensus_score: 0.0,
            direction_confidence: 0.0,
        }
    }

    /// Compute consensus: how many embedders ranked this highly.
    ///
    /// Uses a threshold of 0.5 to determine if an embedder "agrees".
    pub fn compute_consensus(&mut self) {
        let high_threshold = 0.5;
        let embedders_agree = [
            self.e1_score > high_threshold,
            self.e5_score > high_threshold,
            self.e8_score > high_threshold,
            self.e11_score > high_threshold,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        self.consensus_score = embedders_agree as f32 / 4.0;
    }

    /// Compute consensus with a custom threshold.
    pub fn compute_consensus_with_threshold(&mut self, threshold: f32) {
        let embedders_agree = [
            self.e1_score > threshold,
            self.e5_score > threshold,
            self.e8_score > threshold,
            self.e11_score > threshold,
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        self.consensus_score = embedders_agree as f32 / 4.0;
    }

    /// Compute direction confidence from E5 asymmetric scores.
    ///
    /// # Arguments
    /// * `as_cause_score` - Score when treating query as cause
    /// * `as_effect_score` - Score when treating query as effect
    pub fn compute_direction_confidence(&mut self, as_cause_score: f32, as_effect_score: f32) {
        // High confidence when one direction much stronger than other
        self.direction_confidence = (as_cause_score - as_effect_score).abs();
    }

    /// Set the RRF fusion score.
    pub fn with_rrf_score(mut self, score: f32) -> Self {
        self.rrf_score = score;
        self
    }

    /// Set the TransE confidence score.
    pub fn with_transe_confidence(mut self, score: f32) -> Self {
        self.transe_confidence = score;
        self
    }

    /// Set the MaxSim reranking score.
    pub fn with_maxsim_score(mut self, score: f32) -> Self {
        self.maxsim_score = Some(score);
        self
    }

    /// Attach the full relationship data.
    pub fn with_relationship(mut self, rel: CausalRelationship) -> Self {
        self.relationship = Some(rel);
        self
    }

    /// Get all per-embedder scores as a map (for JSON serialization).
    pub fn per_embedder_scores(&self) -> std::collections::HashMap<&'static str, f32> {
        let mut map = std::collections::HashMap::new();
        map.insert("e1", self.e1_score);
        map.insert("e5", self.e5_score);
        map.insert("e8", self.e8_score);
        map.insert("e11", self.e11_score);
        map
    }

    /// Check if this result has high consensus (3+ embedders agree).
    pub fn has_high_consensus(&self) -> bool {
        self.consensus_score >= 0.75
    }

    /// Check if this result has strong direction confidence.
    pub fn has_strong_direction(&self) -> bool {
        self.direction_confidence >= 0.3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_embeddings() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        (
            vec![0.0_f32; CausalRelationship::E5_DIM], // e5_as_cause
            vec![0.0_f32; CausalRelationship::E5_DIM], // e5_as_effect
            vec![0.0_f32; CausalRelationship::E1_DIM], // e1_semantic
        )
    }

    #[test]
    fn test_causal_relationship_new() {
        let (e5_cause, e5_effect, e1) = test_embeddings();
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Chronic stress elevates cortisol".to_string(),
            "Memory impairment".to_string(),
            "High cortisol causes memory impairment...".to_string(),
            e5_cause,
            e5_effect,
            e1,
            "Source content".to_string(),
            source_id,
            0.9,
            "mediated".to_string(),
        );

        assert!(!rel.id.is_nil());
        assert_eq!(rel.cause_statement, "Chronic stress elevates cortisol");
        assert_eq!(rel.effect_statement, "Memory impairment");
        assert_eq!(rel.source_fingerprint_id, source_id);
        assert_eq!(rel.mechanism_type, "mediated");
        assert!((rel.confidence - 0.9).abs() < 0.01);
        assert!(rel.has_valid_embeddings());
    }

    #[test]
    fn test_confidence_clamping() {
        let (e5_cause, e5_effect, e1) = test_embeddings();
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Cause".to_string(),
            "Effect".to_string(),
            "Test".to_string(),
            e5_cause,
            e5_effect,
            e1,
            "Source".to_string(),
            source_id,
            1.5, // Exceeds max
            "direct".to_string(),
        );

        assert_eq!(rel.confidence, 1.0);
    }

    #[test]
    fn test_normalized_mechanism_type() {
        let (e5_cause, e5_effect, e1) = test_embeddings();
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Cause".to_string(),
            "Effect".to_string(),
            "Test".to_string(),
            e5_cause,
            e5_effect,
            e1,
            "Source".to_string(),
            source_id,
            0.8,
            "MEDIATED".to_string(), // Different case
        );

        assert_eq!(rel.normalized_mechanism_type(), "mediated");
    }

    #[test]
    fn test_is_high_confidence() {
        let (e5_cause1, e5_effect1, e1_1) = test_embeddings();
        let (e5_cause2, e5_effect2, e1_2) = test_embeddings();
        let source_id = Uuid::new_v4();

        let high = CausalRelationship::new(
            "Cause".to_string(),
            "Effect".to_string(),
            "Test".to_string(),
            e5_cause1,
            e5_effect1,
            e1_1,
            "Source".to_string(),
            source_id,
            0.8,
            "direct".to_string(),
        );

        let low = CausalRelationship::new(
            "Cause".to_string(),
            "Effect".to_string(),
            "Test".to_string(),
            e5_cause2,
            e5_effect2,
            e1_2,
            "Source".to_string(),
            source_id,
            0.5,
            "direct".to_string(),
        );

        assert!(high.is_high_confidence());
        assert!(!low.is_high_confidence());
    }

    #[test]
    fn test_embedding_accessors() {
        let (e5_cause, e5_effect, e1) = test_embeddings();
        let source_id = Uuid::new_v4();

        let rel = CausalRelationship::new(
            "Cause".to_string(),
            "Effect".to_string(),
            "Test".to_string(),
            e5_cause,
            e5_effect,
            e1,
            "Source".to_string(),
            source_id,
            0.8,
            "direct".to_string(),
        );

        assert_eq!(rel.e5_cause_embedding().len(), CausalRelationship::E5_DIM);
        assert_eq!(rel.e5_effect_embedding().len(), CausalRelationship::E5_DIM);
        assert_eq!(rel.e1_embedding().len(), CausalRelationship::E1_DIM);
    }
}
