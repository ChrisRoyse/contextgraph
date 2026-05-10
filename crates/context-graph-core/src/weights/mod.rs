//! Weight profile configuration for multi-embedding search.
//!
//! This module provides predefined weight profiles for 14 embedding spaces.
//! Moved from MCP crate to Core crate to allow Storage layer access.
//!
//! # 14 Embedding Spaces
//!
//! | Index | Name | Purpose |
//! |-------|------|---------|
//! | 0 | E1_Semantic | General semantic similarity |
//! | 1 | E2_Temporal_Recent | Recent time proximity |
//! | 2 | E3_Temporal_Periodic | Recurring patterns |
//! | 3 | E4_Temporal_Positional | Document position encoding |
//! | 4 | E5_Causal | Cause-effect relationships |
//! | 5 | E6_Sparse | Keyword-level matching |
//! | 6 | E7_Code | Source code similarity |
//! | 7 | E8_Graph | Node2Vec structural |
//! | 8 | E9_HDC | Hyperdimensional computing |
//! | 9 | E10_Multimodal | Cross-modal alignment |
//! | 10 | E11_Entity | Named entity matching |
//! | 11 | E12_Late_Interaction | ColBERT-style token matching |
//! | 12 | E13_SPLADE | Sparse learned expansion (Stage 1) |
//! | 13 | E14_BgeM3Dense | Multilingual dense (BGE-M3 / XLM-RoBERTa-Large) |
//!
//! # Error Handling
//!
//! FAIL FAST: Invalid weights or unknown profiles return detailed errors immediately.

use crate::learner::LearnerStateComponents;
use crate::types::fingerprint::NUM_EMBEDDERS;

/// Whether E2/E3/E4 temporal embedders generate vectors during store_memory.
///
/// When `true`, E2/E3/E4 produce real temporal vectors:
/// - E2: Exponential decay encoding of creation timestamp (recency)
/// - E3: Fourier-basis periodic pattern encoding (time-of-day, day-of-week)
/// - E4: Sinusoidal positional encoding (session ordering)
///
/// E2/E3/E4 participate in fusion when the weight profile assigns non-zero weight
/// (e.g., temporal_navigation profile). Default semantic profiles keep them at 0.0.
/// Post-retrieval temporal boosting (search_recent, search_periodic) is complementary.
pub const TEMPORAL_EMBEDDERS_ENABLED: bool = true;

/// Whether E11 Entity (KEPLER) participates in model loading, search/fusion, and ME-JEPA routing.
///
/// E11 is disabled because the available KEPLER assets are fairseq `.pt`
/// checkpoints (`KEPLERforKE.pt`, `KEPLERforNLP.pt`, `dict.txt`) while the
/// runtime loader requires a self-contained Hugging Face-style directory
/// (`config.json`, `tokenizer.json`, supported transformer weights). Existing
/// audits also showed low-discrimination vectors (0.96-0.98 cosine). Keep the
/// storage slot addressable for old fingerprints, but do not load, score, or
/// route new ME-JEPA work through it.
pub const E11_ENTITY_ENABLED: bool = false;

/// Whether E5 Causal participates in model loading, search, and ME-JEPA routing.
///
/// E5 is intentionally retired: the implementation was never finished, the real
/// Nomic checkpoint does not match the hard-coded loader schema, and existing
/// audits showed near-degenerate similarity. Keep the storage slot addressable
/// for old fingerprints, but do not load, score, or route new work through it.
pub const E5_CAUSAL_ENABLED: bool = false;

/// Number of embedding slots that actively participate in current scoring,
/// clustering, and ME-JEPA routing.
pub fn active_embedder_count() -> usize {
    NUM_EMBEDDERS - disabled_embedder_names().len()
}

/// Storage slots that remain addressable for backwards-compatible persisted
/// fingerprints but must not be loaded, scored, clustered, or routed.
pub fn disabled_embedder_names() -> Vec<&'static str> {
    let mut disabled = Vec::new();
    if !E5_CAUSAL_ENABLED {
        disabled.push("E5");
    }
    if !E11_ENTITY_ENABLED {
        disabled.push("E11");
    }
    disabled
}

/// Weight profile error types.
///
/// Provides detailed context for FAIL FAST error handling.
#[derive(Debug, Clone)]
pub enum WeightProfileError {
    /// Unknown profile name requested.
    UnknownProfile {
        /// The requested profile name.
        name: String,
        /// List of available profile names.
        available: Vec<&'static str>,
    },
    /// A weight is outside the valid range [0.0, 1.0].
    OutOfRange {
        /// Index of the problematic weight.
        index: usize,
        /// Name of the embedding space.
        space_name: &'static str,
        /// The invalid value.
        value: f32,
    },
    /// Weights do not sum to ~1.0.
    InvalidSum {
        /// The actual sum.
        actual: f32,
    },
}

impl std::fmt::Display for WeightProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownProfile { name, available } => {
                write!(
                    f,
                    "Unknown weight profile '{}'. Available profiles: {}",
                    name,
                    available.join(", ")
                )
            }
            Self::OutOfRange {
                index,
                space_name,
                value,
            } => {
                write!(
                    f,
                    "Weight for space {} ({}) is out of range [0.0, 1.0]: {}",
                    index, space_name, value
                )
            }
            Self::InvalidSum { actual } => {
                write!(f, "Weights must sum to ~1.0, got {}", actual)
            }
        }
    }
}

impl std::error::Error for WeightProfileError {}

/// Errors from learner-state-conditioned weight profile routing.
#[derive(Debug, Clone)]
pub enum StateConditionedProfileError {
    /// The supplied learner state is malformed.
    InvalidLearnerState(String),
    /// The caller supplied a base profile that does not exist.
    InvalidBaseProfile(WeightProfileError),
}

impl std::fmt::Display for StateConditionedProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLearnerState(message) => {
                write!(f, "Invalid learner state for retrieval policy: {message}")
            }
            Self::InvalidBaseProfile(error) => write!(f, "Invalid base profile: {error}"),
        }
    }
}

impl std::error::Error for StateConditionedProfileError {}

impl From<WeightProfileError> for StateConditionedProfileError {
    fn from(value: WeightProfileError) -> Self {
        Self::InvalidBaseProfile(value)
    }
}

/// Deterministic routing decision for state-conditioned retrieval.
#[derive(Debug, Clone, PartialEq)]
pub struct StateConditionedProfileSelection {
    pub base_profile: String,
    pub selected_profile: String,
    pub reason: &'static str,
}

/// Predefined weight profiles per query type.
///
/// Each profile has 14 weights corresponding to:
/// [E1, E2, E3, E4, E5, E6, E7, E8, E9, E10, E11, E12, E13, E14]
/// (E14 BGE-M3 Dense is currently 0.0 in legacy profiles; Phase B will
/// reweight category profiles to include it.)
///
/// # Temporal Embedders (E2-E4)
///
/// E2-E4 have weight 0.0 in semantic search profiles (semantic_search, code_search, etc.)
/// because temporal proximity != topical similarity. They participate in fusion only
/// when the AI explicitly selects a temporal-aware profile (temporal_navigation,
/// sequence_navigation, conversation_history) with non-zero E2/E3/E4 weights.
///
/// # Profile Categories
///
/// - **Semantic Profiles** (E2-E4 = 0.0): semantic_search, code_search, causal_reasoning, fact_checking
/// - **Special Profiles**: temporal_navigation (for explicit time-based queries)
/// - **Category-Weighted**: category_weighted (constitution-compliant)
///
/// # IMPORTANT: Pipeline-Stage Embedders (E12, E13) - per ARCH-13
///
/// E12 (Late Interaction) and E13 (SPLADE) have weight 0.0 in ALL semantic scoring profiles
/// because they're used in specific pipeline stages, NOT for similarity scoring:
/// - E13: Stage 1 recall ONLY (inverted index) - per AP-74
/// - E12: Stage 3 re-ranking ONLY (MaxSim) - per AP-73
pub const WEIGHT_PROFILES: &[(&str, [f32; NUM_EMBEDDERS])] = &[
    // =========================================================================
    // SEMANTIC PROFILES - Temporal (E2-E4) = 0.0 per AP-71
    // =========================================================================

    // Semantic Search: General queries - E1 primary, E5/E7/E10 supporting
    (
        "semantic_search",
        [
            0.33, // E1_Semantic (primary)
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.15, // E5_Causal
            0.05, // E6_Sparse (keyword backup)
            0.20, // E7_Code
            0.05, // E8_Graph (relational)
            0.02, // E9_HDC (noise-robust backup for typo tolerance)
            0.15, // E10_Multimodal
            0.05, // E11_Entity (relational)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only)
            0.0,  // E13_SPLADE (Stage 1 recall only)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Causal Reasoning: "Why" questions - E1 primary, E5 binary gate signal only
    // E5 demoted from 0.45→0.10: produces degenerate embeddings (0.93-0.98 for all text)
    // E1 promoted to 0.40: proven 3/3 correct top-1, 17x better discrimination than E5
    (
        "causal_reasoning",
        [
            0.40, // E1_Semantic (primary — proven 3/3 top-1 correct)
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.10, // E5_Causal (demoted — binary structure signal only)
            0.05, // E6_Sparse
            0.15, // E7_Code (handles technical/scientific causal text)
            0.10, // E8_Graph (causal chains)
            0.0,  // E9_HDC
            0.10, // E10_Multimodal (paraphrase matching for same-concept causes)
            0.10, // E11_Entity (entity-aware discrimination)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Code Search: Programming queries - E7 primary
    (
        "code_search",
        [
            0.20, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional
            0.10, // E5_Causal
            0.10, // E6_Sparse (keywords)
            0.40, // E7_Code (primary)
            0.0,  // E8_Graph
            0.0,  // E9_HDC
            0.10, // E10_Multimodal
            0.10, // E11_Entity (function names, etc.)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Fact Checking: Entity/fact queries - E11 primary, E6 for keywords
    (
        "fact_checking",
        [
            0.15, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.15, // E5_Causal
            0.15, // E6_Sparse (keyword match)
            0.05, // E7_Code
            0.05, // E8_Graph
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.40, // E11_Entity (primary - named entities)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // =========================================================================
    // GRAPH REASONING PROFILE - E8 primary for structural queries
    // =========================================================================

    // Graph Reasoning: Structural/connectivity queries - E8 primary
    // Use for: "what imports X?", "what uses X?", "what connects to X?"
    (
        "graph_reasoning",
        [
            0.15, // E1_Semantic
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.10, // E5_Causal
            0.10, // E6_Sparse
            0.0,  // E7_Code
            0.40, // E8_Graph (primary)
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.20, // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // =========================================================================
    // SPECIAL PROFILES
    // =========================================================================

    // Temporal Navigation: EXPLICIT time-based queries only
    // CORE-L3 FIX: E12/E13 set to 0.0 per ARCH-13 (pipeline-stage only, not for scoring)
    // Redistributed 0.04 to E2/E3/E4 (primary temporal embedders: +0.01 each, +0.01 to E1)
    (
        "temporal_navigation",
        [
            0.13, // E1_Semantic
            0.23, // E2_Temporal_Recent (primary)
            0.23, // E3_Temporal_Periodic (primary)
            0.23, // E4_Temporal_Positional (primary)
            0.03, // E5_Causal
            0.02, // E6_Sparse
            0.03, // E7_Code
            0.02, // E8_Graph
            0.03, // E9_HDC
            0.03, // E10_Multimodal
            0.02, // E11_Entity
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per ARCH-13)
            0.0,  // E13_SPLADE (Stage 1 recall only per ARCH-13)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // =========================================================================
    // SEQUENCE NAVIGATION PROFILES - E4 focused
    // =========================================================================

    // Sequence Navigation: For explicit sequence traversal queries
    (
        "sequence_navigation",
        [
            0.20, // E1_Semantic (semantic backup)
            0.05, // E2_Temporal_Recent (mild recency signal)
            0.0,  // E3_Temporal_Periodic (no periodic patterns for sequence)
            0.55, // E4_Temporal_Positional (PRIMARY - sequence ordering)
            0.03, // E5_Causal
            0.02, // E6_Sparse
            0.03, // E7_Code
            0.02, // E8_Graph
            0.03, // E9_HDC
            0.03, // E10_Multimodal
            0.02, // E11_Entity
            0.0,  // E12_Late_Interaction (pipeline stage only)
            0.02, // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Conversation History: Balanced E4 + E1 for contextual recall
    (
        "conversation_history",
        [
            0.30, // E1_Semantic (topic matching)
            0.05, // E2_Temporal_Recent (recent context helps)
            0.0,  // E3_Temporal_Periodic
            0.35, // E4_Temporal_Positional (conversation ordering)
            0.10, // E5_Causal (causal chains in conversation)
            0.03, // E6_Sparse
            0.05, // E7_Code
            0.02, // E8_Graph
            0.0,  // E9_HDC
            0.05, // E10_Multimodal
            0.03, // E11_Entity
            0.0,  // E12_Late_Interaction (pipeline stage only)
            0.02, // E13_SPLADE
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Category-Weighted: Constitution-compliant weights per CLAUDE.md and ARCH-13
    // L8 FIX: Profile normalizer = 6.5 (5 SEMANTIC×1.0 + 2 RELATIONAL×0.5 + 1 STRUCTURAL×0.5)
    // Note: Topic max_weighted_agreement = 8.5 per constitution (includes E12, E13)
    (
        "category_weighted",
        [
            1.0 / 6.5, // E1_Semantic (SEMANTIC)
            0.0,       // E2_Temporal_Recent (TEMPORAL - excluded per AP-60)
            0.0,       // E3_Temporal_Periodic (TEMPORAL - excluded per AP-60)
            0.0,       // E4_Temporal_Positional (TEMPORAL - excluded per AP-60)
            1.0 / 6.5, // E5_Causal (SEMANTIC)
            1.0 / 6.5, // E6_Sparse (SEMANTIC)
            1.0 / 6.5, // E7_Code (SEMANTIC)
            0.5 / 6.5, // E8_Graph (RELATIONAL)
            0.5 / 6.5, // E9_HDC (STRUCTURAL)
            1.0 / 6.5, // E10_Multimodal (SEMANTIC)
            0.5 / 6.5, // E11_Entity (RELATIONAL)
            0.0,       // E12_Late_Interaction (PIPELINE-STAGE)
            0.0,       // E13_SPLADE (PIPELINE-STAGE)
            0.0,       // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // =========================================================================
    // TYPO-TOLERANT PROFILE - E9 primary for noisy queries
    // =========================================================================

    // Typo Tolerant: For queries with potential spelling errors
    // CORE-L3 FIX: E12/E13 set to 0.0 per ARCH-13 (pipeline-stage only, not for scoring)
    // Redistributed 0.05 to E9 (+0.03, primary for typo tolerance) and E6 (+0.02, keyword backup)
    (
        "typo_tolerant",
        [
            0.30, // E1_Semantic (reduced - query might be noisy)
            0.0,  // E2_Temporal_Recent - NOT for semantic search
            0.0,  // E3_Temporal_Periodic - NOT for semantic search
            0.0,  // E4_Temporal_Positional - NOT for semantic search
            0.10, // E5_Causal
            0.07, // E6_Sparse (keyword backup for exact matches)
            0.15, // E7_Code (reduced to make room for E9)
            0.03, // E8_Graph (relational)
            0.18, // E9_HDC (PRIMARY for typo tolerance)
            0.12, // E10_Multimodal
            0.05, // E11_Entity (relational)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per ARCH-13)
            0.0,  // E13_SPLADE (Stage 1 recall only per ARCH-13)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // =========================================================================
    // PIPELINE-AWARE PROFILES - Phase 5 E12/E13 Integration
    // =========================================================================

    // Pipeline Stage 1 Recall: E13-heavy for sparse retrieval
    (
        "pipeline_stage1_recall",
        [
            0.20, // E1_Semantic (backup for semantic overlap)
            0.0,  // E2_Temporal_Recent - NOT for recall stage
            0.0,  // E3_Temporal_Periodic - NOT for recall stage
            0.0,  // E4_Temporal_Positional - NOT for recall stage
            0.05, // E5_Causal (minimal)
            0.25, // E6_Sparse (keyword matching, supports E13)
            0.10, // E7_Code (for code queries)
            0.0,  // E8_Graph
            0.05, // E9_HDC (typo tolerance helps recall)
            0.05, // E10_Multimodal
            0.05, // E11_Entity (entity names)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.25, // E13_SPLADE (PRIMARY - term expansion for recall)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Pipeline Stage 2 Scoring: E1-heavy for dense candidate scoring
    (
        "pipeline_stage2_scoring",
        [
            0.50, // E1_Semantic (PRIMARY - semantic foundation per ARCH-12)
            0.0,  // E2_Temporal_Recent - NOT for scoring stage
            0.0,  // E3_Temporal_Periodic - NOT for scoring stage
            0.0,  // E4_Temporal_Positional - NOT for scoring stage
            0.12, // E5_Causal (causal relationships)
            0.05, // E6_Sparse (keyword precision)
            0.15, // E7_Code (code understanding)
            0.03, // E8_Graph (relational)
            0.02, // E9_HDC (noise tolerance)
            0.08, // E10_Multimodal (paraphrase detection via boost)
            0.05, // E11_Entity (entity matching)
            0.0,  // E12_Late_Interaction (Stage 3 rerank only per AP-73)
            0.0,  // E13_SPLADE (Stage 1 recall only per AP-74)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Pipeline Full: Combined profile for complete pipeline execution
    (
        "pipeline_full",
        [
            0.40, // E1_Semantic (strong foundation)
            0.0,  // E2_Temporal_Recent - NOT for pipeline
            0.0,  // E3_Temporal_Periodic - NOT for pipeline
            0.0,  // E4_Temporal_Positional - NOT for pipeline
            0.10, // E5_Causal
            0.10, // E6_Sparse (keyword precision)
            0.15, // E7_Code
            0.03, // E8_Graph
            0.02, // E9_HDC
            0.08, // E10_Multimodal
            0.05, // E11_Entity
            0.0,  // E12_Late_Interaction (applied via MaxSim, not fusion)
            0.07, // E13_SPLADE (mild weight for fusion awareness)
            0.0,  // E14_BgeM3Dense (E14 excluded from legacy profiles)
        ],
    ),
    // Balanced: Equal weights across all 14 spaces (for testing/comparison)
    (
        "balanced",
        [
            0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715, 0.0715,
            0.0715, 0.0715, 0.071, // 13×0.0715 + 0.071 ≈ 1.0 (sum = 0.9995)
        ],
    ),
    // =========================================================================
    // E14-OPTIMISED PROFILES — multilingual / long-context / translation
    // =========================================================================
    //
    // These profiles explicitly exploit BGE-M3 Dense's unique strengths:
    // cross-lingual alignment, 8192-token context, register-invariant meaning.
    // They are OPT-IN — existing profiles retain E14=0.0 so default behaviour
    // is unchanged until an operator selects one of these explicitly via
    // the `profile` argument to `search_graph` / `search_by_embedder`.

    // Multilingual Search: cross-lingual retrieval across the whole corpus.
    // Use when queries may target documents in a language other than the
    // query's own, or when the corpus is known to be multilingual.
    //
    // E14 primary (0.40), E1 secondary (0.20), E10 paraphrase (0.15),
    // rest spread thinly to preserve keyword/entity/code backup.
    (
        "multilingual_search",
        [
            0.20, // E1_Semantic (English semantic backup)
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.05, // E5_Causal
            0.05, // E6_Sparse (keyword backup — still useful cross-lingually)
            0.05, // E7_Code
            0.03, // E8_Graph
            0.02, // E9_HDC
            0.15, // E10_Multimodal (paraphrase)
            0.05, // E11_Entity
            0.0,  // E12_Late_Interaction (Stage 3 only)
            0.0,  // E13_SPLADE (Stage 1 only)
            0.40, // E14_BgeM3Dense (PRIMARY — multilingual semantic manifold)
        ],
    ),
    // Long Context: for documents/queries that blow past 512 tokens.
    // E14 sees 8192 tokens; all other dense embedders truncate at 512.
    // This profile gives E14 the plurality so long-doc content actually
    // surfaces; E1/E10 remain as local-window semantic backup.
    (
        "long_context",
        [
            0.15, // E1_Semantic (first-512-token backup)
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.05, // E5_Causal
            0.05, // E6_Sparse
            0.10, // E7_Code (long code blocks)
            0.05, // E8_Graph
            0.0,  // E9_HDC
            0.10, // E10_Multimodal
            0.05, // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.45, // E14_BgeM3Dense (PRIMARY — full-document context)
        ],
    ),
    // Translation Finder: explicitly find cross-lingual paraphrases /
    // translation pairs / near-duplicates across language boundaries.
    // E14 dominant (0.70), E10 paraphrase backup (0.20), E11 entity
    // anchor (0.05), E1 English fallback (0.05).
    (
        "translation_finder",
        [
            0.05, // E1_Semantic
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.0,  // E5_Causal
            0.0,  // E6_Sparse (translations typically have NO lexical overlap)
            0.0,  // E7_Code
            0.0,  // E8_Graph
            0.0,  // E9_HDC
            0.20, // E10_Multimodal (paraphrase detection — same-lang backup)
            0.05, // E11_Entity (named entities survive translation)
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.70, // E14_BgeM3Dense (DOMINANT — translation equivalence)
        ],
    ),
    // =========================================================================
    // LEARNER-STATE-AWARE PROFILES — selected from persisted E15-E21 state
    // =========================================================================

    // Affect Repair: broad, analogy-friendly retrieval under elevated stress.
    // Keeps semantic/paraphrase/multilingual channels high, retains a small
    // robust/noisy-text channel, and avoids over-indexing on causal/code edges.
    (
        "affect_repair",
        [
            0.30, // E1_Semantic
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.05, // E5_Causal
            0.05, // E6_Sparse
            0.05, // E7_Code
            0.05, // E8_Graph
            0.10, // E9_HDC
            0.20, // E10_Multimodal / paraphrase
            0.0,  // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.20, // E14_BgeM3Dense
        ],
    ),
    // Affect Priming: when valence is low but stress is not high, bias toward
    // semantically close paraphrases and multilingual/style-safe material.
    (
        "affect_priming",
        [
            0.25, // E1_Semantic
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.05, // E5_Causal
            0.05, // E6_Sparse
            0.05, // E7_Code
            0.05, // E8_Graph
            0.05, // E9_HDC
            0.25, // E10_Multimodal / paraphrase
            0.0,  // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.25, // E14_BgeM3Dense
        ],
    ),
    // Affect Neutral: high stress + high arousal. Prefer literal/factual
    // retrieval with less associative drift while preserving E14 context.
    (
        "affect_neutral",
        [
            0.25, // E1_Semantic
            0.0,  // E2_Temporal_Recent
            0.0,  // E3_Temporal_Periodic
            0.0,  // E4_Temporal_Positional
            0.15, // E5_Causal
            0.20, // E6_Sparse
            0.05, // E7_Code
            0.05, // E8_Graph
            0.05, // E9_HDC
            0.10, // E10_Multimodal
            0.0,  // E11_Entity
            0.0,  // E12_Late_Interaction
            0.0,  // E13_SPLADE
            0.15, // E14_BgeM3Dense
        ],
    ),
];

/// Get weight profile by name.
///
/// # Arguments
/// * `name` - Profile name (e.g., "semantic_search", "code_search")
///
/// # Returns
/// The 14-element weight array if found.
///
/// # Errors
/// Returns `WeightProfileError::UnknownProfile` if the profile name is not found.
/// This is a FAIL FAST behavior - invalid profile names are rejected immediately.
pub fn get_weight_profile(name: &str) -> Result<[f32; NUM_EMBEDDERS], WeightProfileError> {
    WEIGHT_PROFILES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, w)| *w)
        .ok_or_else(|| WeightProfileError::UnknownProfile {
            name: name.to_string(),
            available: get_profile_names(),
        })
}

/// Get all available profile names.
pub fn get_profile_names() -> Vec<&'static str> {
    WEIGHT_PROFILES.iter().map(|(n, _)| *n).collect()
}

/// Select a content retrieval weight profile from persisted E15-E21 learner state.
///
/// This does not invent learner embedders inside content fusion. Instead, it
/// uses the learner-state scalar components as a routing signal for the existing
/// E1-E14 retrieval profiles. The function is deliberately deterministic and
/// fail-fast so MCP callers can verify the same state always produces the same
/// profile.
pub fn select_state_conditioned_weight_profile(
    base_profile: Option<&str>,
    components: &LearnerStateComponents,
) -> Result<StateConditionedProfileSelection, StateConditionedProfileError> {
    components
        .validate()
        .map_err(|e| StateConditionedProfileError::InvalidLearnerState(e.to_string()))?;

    let base_profile = base_profile.unwrap_or("semantic_search");
    get_weight_profile(base_profile)?;

    let (selected_profile, reason) = if components.stress_floor >= 0.80
        || (components.stress_floor >= 0.60 && components.arousal >= 0.65)
    {
        (
            "affect_neutral",
            "stress_floor/arousal above neutral-routing threshold",
        )
    } else if components.stress_floor >= 0.60 {
        (
            "affect_repair",
            "stress_floor above repair-routing threshold",
        )
    } else if components.valence <= -0.30 {
        ("affect_priming", "valence below priming-routing threshold")
    } else {
        (base_profile, "learner state within base-profile band")
    };

    get_weight_profile(selected_profile)?;

    Ok(StateConditionedProfileSelection {
        base_profile: base_profile.to_string(),
        selected_profile: selected_profile.to_string(),
        reason,
    })
}

/// Get weight profile with disabled slots removed from active scoring.
///
/// This is the preferred entry point for production search code. It returns the
/// same base profile as `get_weight_profile` but with retired/disabled slots
/// redistributed to remaining active embedders.
pub fn get_effective_weight_profile(
    name: &str,
) -> Result<[f32; NUM_EMBEDDERS], WeightProfileError> {
    let mut weights = get_weight_profile(name)?;
    if !E5_CAUSAL_ENABLED {
        apply_e5_retirement(&mut weights);
    }
    if !E11_ENTITY_ENABLED {
        apply_e11_disable(&mut weights);
    }
    Ok(weights)
}

/// Zero out retired E5 weight and redistribute proportionally to active embedders.
///
/// After redistribution, the weights still sum to ~1.0. This keeps legacy profile
/// definitions readable while ensuring production search/ME-JEPA paths do not
/// depend on the retired causal embedder.
pub fn apply_e5_retirement(weights: &mut [f32; NUM_EMBEDDERS]) {
    const E5_IDX: usize = 4;
    let e5_weight = weights[E5_IDX];
    if e5_weight <= 0.0 {
        return;
    }
    weights[E5_IDX] = 0.0;
    let remaining_sum: f32 = weights.iter().sum();
    if remaining_sum > 0.0 {
        let scale = (remaining_sum + e5_weight) / remaining_sum;
        for w in weights.iter_mut() {
            if *w > 0.0 {
                *w *= scale;
            }
        }
    }
}

/// Zero out E11 weight and redistribute proportionally to remaining active embedders.
///
/// After redistribution, the weights still sum to ~1.0.
pub fn apply_e11_disable(weights: &mut [f32; NUM_EMBEDDERS]) {
    const E11_IDX: usize = 10;
    let e11_weight = weights[E11_IDX];
    if e11_weight <= 0.0 {
        return;
    }
    weights[E11_IDX] = 0.0;
    let remaining_sum: f32 = weights.iter().sum();
    if remaining_sum > 0.0 {
        let scale = (remaining_sum + e11_weight) / remaining_sum;
        for w in weights.iter_mut() {
            if *w > 0.0 {
                *w *= scale;
            }
        }
    }
}

/// Validate that weights sum to ~1.0 and all are in [0.0, 1.0].
///
/// # FAIL FAST
/// Returns detailed error on validation failure.
pub fn validate_weights(weights: &[f32; NUM_EMBEDDERS]) -> Result<(), WeightProfileError> {
    // Check each weight is in range
    for (i, &w) in weights.iter().enumerate() {
        if !(0.0..=1.0).contains(&w) {
            return Err(WeightProfileError::OutOfRange {
                index: i,
                space_name: space_name(i),
                value: w,
            });
        }
    }

    // Check sum is ~1.0
    let sum: f32 = weights.iter().sum();
    if (sum - 1.0).abs() > 0.01 {
        return Err(WeightProfileError::InvalidSum { actual: sum });
    }

    Ok(())
}

/// Get space name by index.
pub fn space_name(idx: usize) -> &'static str {
    match idx {
        0 => "E1_Semantic",
        1 => "E2_Temporal_Recent",
        2 => "E3_Temporal_Periodic",
        3 => "E4_Temporal_Positional",
        4 => "E5_Causal",
        5 => "E6_Sparse",
        6 => "E7_Code",
        7 => "E8_Graph",
        8 => "E9_HDC",
        9 => "E10_Multimodal",
        10 => "E11_Entity",
        11 => "E12_Late_Interaction",
        12 => "E13_SPLADE",
        13 => "E14_BgeM3Dense",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_profile_fails_fast() {
        let result = get_weight_profile("nonexistent");
        assert!(matches!(
            result,
            Err(WeightProfileError::UnknownProfile { .. })
        ));

        if let Err(WeightProfileError::UnknownProfile { name, available }) = result {
            assert_eq!(name, "nonexistent");
            assert!(!available.is_empty());
            println!("[VERIFIED] Unknown profile fails fast with available profiles list");
        }
    }

    #[test]
    fn test_code_search_profile_weights() {
        let weights = get_weight_profile("code_search").unwrap();
        assert!((weights[6] - 0.40).abs() < 0.001, "E7 Code should be 0.40");
        assert!(weights[6] > weights[0], "E7 > E1 for code search");
        println!("[VERIFIED] code_search has E7={:.2} as primary", weights[6]);
    }

    #[test]
    fn test_all_profiles_sum_to_one() {
        for (name, weights) in WEIGHT_PROFILES {
            let sum: f32 = weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 0.01,
                "Profile '{}' sums to {} (expected ~1.0)",
                name,
                sum
            );
            println!("[VERIFIED] Profile '{}' sums to {:.4}", name, sum);
        }
    }

    #[test]
    fn test_all_profiles_have_14_weights() {
        for (name, weights) in WEIGHT_PROFILES {
            assert_eq!(
                weights.len(),
                NUM_EMBEDDERS,
                "Profile '{}' should have {} weights",
                name,
                NUM_EMBEDDERS
            );
        }
        println!(
            "[VERIFIED] All profiles have exactly {} weights",
            NUM_EMBEDDERS
        );
    }

    #[test]
    fn test_graph_reasoning_profile_exists() {
        let weights = get_weight_profile("graph_reasoning");
        assert!(weights.is_ok(), "graph_reasoning profile should exist");

        let weights = weights.unwrap();
        assert!((weights[7] - 0.40).abs() < 0.001, "E8 Graph should be 0.40");
        assert!(weights[7] > weights[0], "E8 > E1 for graph reasoning");
        println!(
            "[VERIFIED] graph_reasoning has E8={:.2} as primary",
            weights[7]
        );
    }

    #[test]
    fn test_temporal_embedders_excluded_from_semantic_profiles() {
        let semantic_profiles = [
            "semantic_search",
            "causal_reasoning",
            "code_search",
            "fact_checking",
            "graph_reasoning",
        ];

        for profile_name in semantic_profiles {
            let weights = get_weight_profile(profile_name)
                .unwrap_or_else(|_| panic!("Profile '{}' should exist", profile_name));

            assert_eq!(
                weights[1], 0.0,
                "E2 should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[2], 0.0,
                "E3 should be 0.0 in '{}' profile per AP-71",
                profile_name
            );
            assert_eq!(
                weights[3], 0.0,
                "E4 should be 0.0 in '{}' profile per AP-71",
                profile_name
            );

            println!(
                "[VERIFIED] Profile '{}' has temporal embedders (E2-E4) = 0.0",
                profile_name
            );
        }
    }

    #[test]
    fn test_validate_weights_valid() {
        let valid = get_weight_profile("semantic_search").unwrap();
        assert!(
            validate_weights(&valid).is_ok(),
            "Valid profile should pass validation"
        );
        println!("[VERIFIED] Valid weights pass validation");
    }

    #[test]
    fn test_validate_weights_out_of_range() {
        let mut weights = [0.077f32; NUM_EMBEDDERS];
        weights[0] = 1.5; // Out of range

        let result = validate_weights(&weights);
        assert!(result.is_err());

        match result.unwrap_err() {
            WeightProfileError::OutOfRange { index, .. } => {
                assert_eq!(index, 0);
            }
            _ => panic!("Expected OutOfRange error"),
        }
        println!("[VERIFIED] Out-of-range weight fails fast");
    }

    #[test]
    fn test_validate_weights_invalid_sum() {
        let weights = [0.5f32; NUM_EMBEDDERS]; // Sum = 7.0 (14 * 0.5)

        let result = validate_weights(&weights);
        assert!(result.is_err());

        match result.unwrap_err() {
            WeightProfileError::InvalidSum { actual } => {
                assert!((actual - 7.0).abs() < 0.01);
            }
            _ => panic!("Expected InvalidSum error"),
        }
        println!("[VERIFIED] Invalid sum fails fast");
    }

    #[test]
    fn test_space_names() {
        assert_eq!(space_name(0), "E1_Semantic");
        assert_eq!(space_name(6), "E7_Code");
        assert_eq!(space_name(7), "E8_Graph");
        assert_eq!(space_name(12), "E13_SPLADE");
        assert_eq!(space_name(13), "E14_BgeM3Dense");
        assert_eq!(space_name(14), "Unknown");
        println!("[VERIFIED] space_name returns correct names");
    }

    #[test]
    fn test_get_profile_names() {
        let names = get_profile_names();
        assert!(names.contains(&"semantic_search"));
        assert!(names.contains(&"code_search"));
        assert!(names.contains(&"causal_reasoning"));
        assert!(names.contains(&"graph_reasoning"));
        assert!(names.contains(&"affect_repair"));
        assert!(names.contains(&"affect_priming"));
        assert!(names.contains(&"affect_neutral"));
        println!(
            "[VERIFIED] get_profile_names returns {} profiles",
            names.len()
        );
    }

    fn components(
        plasticity_window: f32,
        hrv_coherence: f32,
        valence: f32,
        arousal: f32,
        stress_floor: f32,
    ) -> LearnerStateComponents {
        LearnerStateComponents {
            plasticity_window,
            hrv_coherence,
            valence,
            arousal,
            stress_floor,
            k_sleep: 1.0,
        }
    }

    #[test]
    fn test_state_conditioned_profile_routes_high_stress_to_repair() {
        let selection = select_state_conditioned_weight_profile(
            Some("semantic_search"),
            &components(0.5, 0.4, 0.0, 0.2, 0.70),
        )
        .unwrap();
        assert_eq!(selection.selected_profile, "affect_repair");
        println!(
            "[VERIFIED] high stress routes {} -> {} ({})",
            selection.base_profile, selection.selected_profile, selection.reason
        );
    }

    #[test]
    fn test_state_conditioned_profile_routes_stress_arousal_to_neutral() {
        let selection = select_state_conditioned_weight_profile(
            Some("semantic_search"),
            &components(0.4, 0.3, -0.1, 0.80, 0.85),
        )
        .unwrap();
        assert_eq!(selection.selected_profile, "affect_neutral");
        println!(
            "[VERIFIED] high stress/arousal routes {} -> {} ({})",
            selection.base_profile, selection.selected_profile, selection.reason
        );
    }

    #[test]
    fn test_state_conditioned_profile_routes_low_valence_to_priming() {
        let selection = select_state_conditioned_weight_profile(
            Some("multilingual_search"),
            &components(0.6, 0.7, -0.5, 0.1, 0.3),
        )
        .unwrap();
        assert_eq!(selection.base_profile, "multilingual_search");
        assert_eq!(selection.selected_profile, "affect_priming");
        println!(
            "[VERIFIED] low valence routes {} -> {} ({})",
            selection.base_profile, selection.selected_profile, selection.reason
        );
    }

    #[test]
    fn test_state_conditioned_profile_fails_fast_on_invalid_state() {
        let result = select_state_conditioned_weight_profile(
            Some("semantic_search"),
            &components(0.6, 0.7, 0.0, 0.1, 1.2),
        );
        assert!(matches!(
            result,
            Err(StateConditionedProfileError::InvalidLearnerState(_))
        ));
        println!("[VERIFIED] invalid learner state fails fast");
    }

    #[test]
    fn test_state_conditioned_profile_fails_fast_on_unknown_base() {
        let result = select_state_conditioned_weight_profile(
            Some("missing_profile"),
            &components(0.6, 0.7, 0.0, 0.1, 0.2),
        );
        assert!(matches!(
            result,
            Err(StateConditionedProfileError::InvalidBaseProfile(_))
        ));
        println!("[VERIFIED] invalid base profile fails fast");
    }

    #[test]
    fn test_e11_disabled_profiles_sum_to_one() {
        for (name, _) in WEIGHT_PROFILES {
            let weights = get_effective_weight_profile(name).unwrap();
            let sum: f32 = weights.iter().sum();
            assert!(
                (sum - 1.0).abs() < 0.02,
                "Effective profile '{}' sums to {} (expected ~1.0)",
                name,
                sum
            );
            if !E11_ENTITY_ENABLED {
                assert!(
                    weights[10] == 0.0,
                    "E11 weight should be 0.0 when disabled, got {} for '{}'",
                    weights[10],
                    name
                );
            }
        }
    }

    #[test]
    fn test_apply_e11_disable_redistributes() {
        let mut weights = [0.0f32; NUM_EMBEDDERS];
        weights[0] = 0.5; // E1
        weights[6] = 0.3; // E7
        weights[10] = 0.2; // E11
        let original_sum: f32 = weights.iter().sum();

        apply_e11_disable(&mut weights);

        assert_eq!(weights[10], 0.0, "E11 should be zeroed");
        let new_sum: f32 = weights.iter().sum();
        assert!(
            (new_sum - original_sum).abs() < 0.01,
            "Sum should be preserved: was {}, now {}",
            original_sum,
            new_sum
        );
        assert!(weights[0] > 0.5, "E1 should receive redistributed weight");
        assert!(weights[6] > 0.3, "E7 should receive redistributed weight");
    }

    #[test]
    fn test_apply_e11_disable_noop_when_zero() {
        let mut weights = [0.0f32; NUM_EMBEDDERS];
        weights[0] = 0.6;
        weights[6] = 0.4;
        // E11 is already 0.0
        let original = weights;

        apply_e11_disable(&mut weights);

        assert_eq!(weights, original, "Should be no-op when E11 is already 0.0");
    }
}
