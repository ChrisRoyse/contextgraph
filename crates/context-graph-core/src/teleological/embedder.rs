//! Canonical Embedder enumeration for the 13-embedder teleological system.
//!
//! This module defines the SINGLE source of truth for embedder types.
//! All other code MUST use this enum - no duplicate definitions allowed.
//!
//! # Architecture Reference
//!
//! From constitution.yaml (ARCH-02): "Compare Only Compatible Embedding Types"
//! From constitution.yaml (ARCH-05): "All 13 Embedders Must Be Present"

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// Import dimension constants from the fingerprint module (via public re-export)
use crate::types::fingerprint::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM,
    E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};

/// Error when parsing embedder name.
#[derive(Debug, Error, Clone)]
pub enum EmbedderNameError {
    /// Unknown embedder name.
    #[error("E_EMB_NAME_001: Unknown embedder '{name}'. Valid names: {valid_names:?}")]
    Unknown {
        name: String,
        valid_names: Vec<&'static str>,
    },
}

/// The 13 embedding models in the teleological array system.
///
/// Each embedder captures a different semantic dimension:
/// - Dense: Standard float vectors (cosine similarity)
/// - Sparse: SPLADE/lexical vectors (~30K vocab, sparse dot product)
/// - TokenLevel: ColBERT-style per-token embeddings (MaxSim)
/// - Binary: HDC hyperdimensional (Hamming distance) - stored as projected dense
///
/// # Ordering
///
/// Variant values (0-12) are STABLE and used for array indexing.
/// DO NOT reorder or renumber variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Embedder {
    /// E1: Semantic understanding via e5-large-v2 (1024D dense)
    Semantic = 0,
    /// E2: Recent temporal context via exponential decay (512D dense)
    TemporalRecent = 1,
    /// E3: Periodic/cyclical patterns via Fourier (512D dense)
    TemporalPeriodic = 2,
    /// E4: Temporal-positional sinusoidal PE (512D dense)
    TemporalPositional = 3,
    /// E5: Causal reasoning via Longformer SCM, ASYMMETRIC (768D dense)
    Causal = 4,
    /// E6: SPLADE sparse lexical expansion (~30K sparse)
    Sparse = 5,
    /// E7: Code via Qodo-Embed (1536D dense)
    Code = 6,
    /// E8: Graph/connectivity patterns via e5-large-v2 (1024D dense, ASYMMETRIC source/target)
    ///
    /// Constitution: teleological_purpose = V_connectivity
    Graph = 7,
    /// E9: HDC projected from 10K-bit hyperdimensional (1024D dense)
    Hdc = 8,
    /// E10: Paraphrase via e5-base-v2 (768D dense, text-only — historically labeled "Multimodal")
    Multimodal = 9,
    /// E11: Entity via KEPLER (768D dense)
    Entity = 10,
    /// E12: Late interaction ColBERT MaxSim (128D per token)
    LateInteraction = 11,
    /// E13: SPLADE v3 for keyword expansion (~30K sparse)
    KeywordSplade = 12,
}

impl Embedder {
    /// Total number of embedders in the system.
    pub const COUNT: usize = 13;

    /// Get the array index for this embedder (0-12).
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Create an embedder from an array index.
    ///
    /// Returns `None` if index >= 13.
    #[inline]
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Semantic),
            1 => Some(Self::TemporalRecent),
            2 => Some(Self::TemporalPeriodic),
            3 => Some(Self::TemporalPositional),
            4 => Some(Self::Causal),
            5 => Some(Self::Sparse),
            6 => Some(Self::Code),
            7 => Some(Self::Graph),
            8 => Some(Self::Hdc),
            9 => Some(Self::Multimodal),
            10 => Some(Self::Entity),
            11 => Some(Self::LateInteraction),
            12 => Some(Self::KeywordSplade),
            _ => None,
        }
    }

    /// Get the expected dimensions for this embedder.
    ///
    /// Returns an `EmbedderDims` variant describing the embedding shape.
    pub fn expected_dims(self) -> EmbedderDims {
        match self {
            Self::Semantic => EmbedderDims::Dense(E1_DIM), // 1024
            Self::TemporalRecent => EmbedderDims::Dense(E2_DIM), // 512
            Self::TemporalPeriodic => EmbedderDims::Dense(E3_DIM), // 512
            Self::TemporalPositional => EmbedderDims::Dense(E4_DIM), // 512
            Self::Causal => EmbedderDims::Dense(E5_DIM),   // 768
            Self::Sparse => EmbedderDims::Sparse {
                vocab_size: E6_SPARSE_VOCAB,
            }, // 30522
            Self::Code => EmbedderDims::Dense(E7_DIM),     // 1536
            Self::Graph => EmbedderDims::Dense(E8_DIM), // 1024 (e5-large-v2)
            Self::Hdc => EmbedderDims::Dense(E9_DIM),      // 1024 (projected)
            Self::Multimodal => EmbedderDims::Dense(E10_DIM), // 768
            Self::Entity => EmbedderDims::Dense(E11_DIM),  // 768 (KEPLER)
            Self::LateInteraction => EmbedderDims::TokenLevel {
                per_token: E12_TOKEN_DIM,
            }, // 128
            Self::KeywordSplade => EmbedderDims::Sparse {
                vocab_size: E13_SPLADE_VOCAB,
            }, // 30522
        }
    }

    /// Iterate over all embedders in order (E1 through E13).
    pub fn all() -> impl ExactSizeIterator<Item = Embedder> {
        (0..Self::COUNT).map(|i| Self::from_index(i).expect("index in range"))
    }

    /// Get the full display name for this embedder.
    ///
    /// Example: "E1_Semantic", "E6_Sparse_Lexical"
    pub fn name(self) -> &'static str {
        match self {
            Self::Semantic => "E1_Semantic",
            Self::TemporalRecent => "E2_Temporal_Recent",
            Self::TemporalPeriodic => "E3_Temporal_Periodic",
            Self::TemporalPositional => "E4_Temporal_Positional",
            Self::Causal => "E5_Causal",
            Self::Sparse => "E6_Sparse_Lexical",
            Self::Code => "E7_Code",
            Self::Graph => "E8_Graph",
            Self::Hdc => "E9_HDC",
            Self::Multimodal => "E10_Multimodal",
            Self::Entity => "E11_Entity",
            Self::LateInteraction => "E12_Late_Interaction",
            Self::KeywordSplade => "E13_SPLADE",
        }
    }

    /// Get a short identifier for this embedder.
    ///
    /// Example: "E1", "E6", "E13"
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Semantic => "E1",
            Self::TemporalRecent => "E2",
            Self::TemporalPeriodic => "E3",
            Self::TemporalPositional => "E4",
            Self::Causal => "E5",
            Self::Sparse => "E6",
            Self::Code => "E7",
            Self::Graph => "E8",
            Self::Hdc => "E9",
            Self::Multimodal => "E10",
            Self::Entity => "E11",
            Self::LateInteraction => "E12",
            Self::KeywordSplade => "E13",
        }
    }

    /// Check if this embedder uses dense (fixed-length float) vectors.
    #[inline]
    pub fn is_dense(self) -> bool {
        matches!(self.expected_dims(), EmbedderDims::Dense(_))
    }

    /// Check if this embedder uses sparse vectors.
    #[inline]
    pub fn is_sparse(self) -> bool {
        matches!(self.expected_dims(), EmbedderDims::Sparse { .. })
    }

    /// Check if this embedder uses token-level embeddings.
    #[inline]
    pub fn is_token_level(self) -> bool {
        matches!(self.expected_dims(), EmbedderDims::TokenLevel { .. })
    }

    /// Get default temperature for confidence calibration (from constitution.yaml ATC).
    pub fn default_temperature(self) -> f32 {
        match self {
            Self::Semantic => 1.0,
            Self::Causal => 1.2,        // Overconfident
            Self::Code => 0.9,          // Needs precision
            Self::Hdc => 1.5,           // Noisy
            Self::KeywordSplade => 1.1, // Sparse = variable
            _ => 1.0,                   // Baseline
        }
    }

    /// Get the teleological purpose for this embedder.
    pub fn purpose(self) -> &'static str {
        match self {
            Self::Semantic => "V_semantic",
            Self::TemporalRecent => "V_temporal_recent",
            Self::TemporalPeriodic => "V_temporal_periodic",
            Self::TemporalPositional => "V_temporal_positional",
            Self::Causal => "V_causality",
            Self::Sparse => "V_sparse",
            Self::Code => "V_code",
            Self::Graph => "V_connectivity", // Per constitution
            Self::Hdc => "V_hdc",
            Self::Multimodal => "V_multimodal",
            Self::Entity => "V_entity",
            Self::LateInteraction => "V_late_interaction",
            Self::KeywordSplade => "V_keyword",
        }
    }

    /// Create embedder from string name.
    ///
    /// Accepts both canonical and deprecated names.
    /// Deprecated names emit a tracing warning.
    ///
    /// # Arguments
    ///
    /// * `name` - Embedder name (case-insensitive)
    ///
    /// # Returns
    ///
    /// * `Ok(Embedder)` - Valid embedder
    /// * `Err(EmbedderNameError)` - Invalid or ambiguous name
    pub fn from_name(name: &str) -> Result<Self, EmbedderNameError> {
        let normalized = name.to_lowercase().replace('-', "_");

        match normalized.as_str() {
            // E1
            "e1" | "e1_semantic" | "semantic" => Ok(Self::Semantic),
            // E2
            "e2" | "e2_temporal_recent" | "temporal_recent" => Ok(Self::TemporalRecent),
            // E3
            "e3" | "e3_temporal_periodic" | "temporal_periodic" => Ok(Self::TemporalPeriodic),
            // E4
            "e4" | "e4_temporal_positional" | "temporal_positional" => Ok(Self::TemporalPositional),
            // E5
            "e5" | "e5_causal" | "causal" => Ok(Self::Causal),
            // E6
            "e6" | "e6_sparse" | "e6_sparse_lexical" | "sparse" | "sparse_lexical" => {
                Ok(Self::Sparse)
            }
            // E7
            "e7" | "e7_code" | "code" => Ok(Self::Code),
            // E8
            "e8" | "e8_graph" | "graph" => Ok(Self::Graph),
            // E9
            "e9" | "e9_hdc" | "hdc" => Ok(Self::Hdc),
            // E10
            "e10" | "e10_multimodal" | "multimodal" => Ok(Self::Multimodal),
            // E11
            "e11" | "e11_entity" | "entity" => Ok(Self::Entity),
            // E12
            "e12" | "e12_late_interaction" | "late_interaction" | "lateinteraction" => {
                Ok(Self::LateInteraction)
            }
            // E13
            "e13" | "e13_splade" | "e13_keyword_splade" | "keyword_splade" | "keywordsplade" => {
                Ok(Self::KeywordSplade)
            }
            // Unknown
            _ => Err(EmbedderNameError::Unknown {
                name: name.to_string(),
                valid_names: Self::all_names(),
            }),
        }
    }

    /// Get all valid embedder names.
    pub fn all_names() -> Vec<&'static str> {
        Self::all().map(|e| e.name()).collect()
    }
}

impl fmt::Display for Embedder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Describes the dimensional structure of an embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedderDims {
    /// Fixed-length dense float vector.
    Dense(usize),
    /// Sparse vector with vocabulary size (active indices << vocab_size).
    Sparse { vocab_size: usize },
    /// Variable-length sequence of per-token embeddings.
    TokenLevel { per_token: usize },
}

impl EmbedderDims {
    /// Get the primary dimension value.
    ///
    /// For Dense: the vector length
    /// For Sparse: the vocabulary size
    /// For TokenLevel: the per-token dimension
    pub fn primary_dim(&self) -> usize {
        match self {
            Self::Dense(d) => *d,
            Self::Sparse { vocab_size } => *vocab_size,
            Self::TokenLevel { per_token } => *per_token,
        }
    }
}

/// Bitmask for selecting a subset of embedders.
///
/// Uses a u16 internally (bits 0-12 for E1-E13).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbedderMask(u16);

impl EmbedderMask {
    /// Create an empty mask (no embedders selected).
    #[inline]
    pub fn new() -> Self {
        Self(0)
    }

    /// Create a mask with all 13 embedders selected.
    #[inline]
    pub fn all() -> Self {
        Self((1 << Embedder::COUNT) - 1)
    }

    /// Create a mask from a slice of embedders.
    pub fn from_slice(embedders: &[Embedder]) -> Self {
        let mut mask = Self::new();
        for &e in embedders {
            mask.set(e);
        }
        mask
    }

    /// Set (enable) an embedder in the mask.
    #[inline]
    pub fn set(&mut self, embedder: Embedder) {
        self.0 |= 1 << embedder.index();
    }

    /// Unset (disable) an embedder in the mask.
    #[inline]
    pub fn unset(&mut self, embedder: Embedder) {
        self.0 &= !(1 << embedder.index());
    }

    /// Check if an embedder is set in the mask.
    #[inline]
    pub fn contains(self, embedder: Embedder) -> bool {
        (self.0 & (1 << embedder.index())) != 0
    }

    /// Iterate over all embedders that are set in this mask.
    pub fn iter(self) -> impl Iterator<Item = Embedder> {
        Embedder::all().filter(move |&e| self.contains(e))
    }

    /// Count the number of embedders set in this mask.
    #[inline]
    pub fn count(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Check if the mask is empty.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Get the raw bitmask value.
    #[inline]
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

/// Predefined groups of embedders for common operations.
///
/// From constitution.yaml teleological.embedder_purposes and teleoplan.md groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbedderGroup {
    /// Temporal embedders: E2 (Recent), E3 (Periodic), E4 (Positional)
    Temporal,
    /// Relational embedders: E4 (Positional), E5 (Causal), E11 (Entity)
    Relational,
    /// Lexical/sparse embedders: E6 (Sparse), E12 (LateInteraction), E13 (KeywordSplade)
    Lexical,
    /// All dense embedders (excludes E6, E12, E13)
    Dense,
    /// Factual embedders: E1 (Semantic), E12 (LateInteraction), E13 (KeywordSplade)
    Factual,
    /// Code-focused: E7 (Code)
    Implementation,
    /// All 13 embedders
    All,
}

impl EmbedderGroup {
    /// Get the embedder mask for this group.
    pub fn embedders(self) -> EmbedderMask {
        match self {
            Self::Temporal => EmbedderMask::from_slice(&[
                Embedder::TemporalRecent,
                Embedder::TemporalPeriodic,
                Embedder::TemporalPositional,
            ]),
            Self::Relational => EmbedderMask::from_slice(&[
                Embedder::TemporalPositional,
                Embedder::Causal,
                Embedder::Entity,
            ]),
            Self::Lexical => EmbedderMask::from_slice(&[
                Embedder::Sparse,
                Embedder::LateInteraction,
                Embedder::KeywordSplade,
            ]),
            Self::Dense => {
                let mut mask = EmbedderMask::all();
                mask.unset(Embedder::Sparse);
                mask.unset(Embedder::LateInteraction);
                mask.unset(Embedder::KeywordSplade);
                mask
            }
            Self::Factual => EmbedderMask::from_slice(&[
                Embedder::Semantic,
                Embedder::LateInteraction,
                Embedder::KeywordSplade,
            ]),
            Self::Implementation => EmbedderMask::from_slice(&[Embedder::Code]),
            Self::All => EmbedderMask::all(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_count() {
        assert_eq!(Embedder::COUNT, 13);
        assert_eq!(Embedder::all().count(), 13);
        println!("[PASS] Embedder::COUNT equals 13");
    }

    #[test]
    fn test_index_roundtrip() {
        for e in Embedder::all() {
            let idx = e.index();
            let recovered = Embedder::from_index(idx);
            assert_eq!(
                recovered,
                Some(e),
                "Roundtrip failed for {:?} at index {}",
                e,
                idx
            );
        }
        println!("[PASS] All embedders roundtrip through index()");
    }

    #[test]
    fn test_index_bounds() {
        assert!(Embedder::from_index(12).is_some());
        assert!(Embedder::from_index(13).is_none());
        assert!(Embedder::from_index(100).is_none());
        println!("[PASS] from_index returns None for out-of-bounds");
    }

    #[test]
    fn test_expected_dims_match_constants() {
        assert_eq!(
            Embedder::Semantic.expected_dims(),
            EmbedderDims::Dense(1024)
        );
        assert_eq!(
            Embedder::TemporalRecent.expected_dims(),
            EmbedderDims::Dense(512)
        );
        assert_eq!(Embedder::Causal.expected_dims(), EmbedderDims::Dense(768));
        assert_eq!(
            Embedder::Sparse.expected_dims(),
            EmbedderDims::Sparse { vocab_size: 30522 }
        );
        assert_eq!(Embedder::Code.expected_dims(), EmbedderDims::Dense(1536));
        assert_eq!(
            Embedder::LateInteraction.expected_dims(),
            EmbedderDims::TokenLevel { per_token: 128 }
        );
        println!("[PASS] expected_dims() matches constants.rs values");
    }

    #[test]
    fn test_names() {
        assert_eq!(Embedder::Semantic.name(), "E1_Semantic");
        assert_eq!(Embedder::Sparse.name(), "E6_Sparse_Lexical");
        assert_eq!(Embedder::KeywordSplade.name(), "E13_SPLADE");
        assert_eq!(Embedder::Semantic.short_name(), "E1");
        assert_eq!(Embedder::KeywordSplade.short_name(), "E13");
        println!("[PASS] name() and short_name() return correct values");
    }

    #[test]
    fn test_embedder_mask_operations() {
        let mut mask = EmbedderMask::new();
        assert!(mask.is_empty());
        assert_eq!(mask.count(), 0);

        mask.set(Embedder::Semantic);
        mask.set(Embedder::Causal);
        assert!(!mask.is_empty());
        assert_eq!(mask.count(), 2);
        assert!(mask.contains(Embedder::Semantic));
        assert!(mask.contains(Embedder::Causal));
        assert!(!mask.contains(Embedder::Code));

        mask.unset(Embedder::Semantic);
        assert_eq!(mask.count(), 1);
        assert!(!mask.contains(Embedder::Semantic));

        println!("[PASS] EmbedderMask operations work correctly");
    }

    #[test]
    fn test_embedder_mask_all() {
        let all = EmbedderMask::all();
        assert_eq!(all.count(), 13);
        for e in Embedder::all() {
            assert!(all.contains(e), "{:?} not in all() mask", e);
        }
        println!("[PASS] EmbedderMask::all() contains all 13 embedders");
    }

    #[test]
    fn test_embedder_mask_iter() {
        let mask =
            EmbedderMask::from_slice(&[Embedder::Semantic, Embedder::Causal, Embedder::Code]);
        let collected: Vec<_> = mask.iter().collect();
        assert_eq!(collected.len(), 3);
        assert!(collected.contains(&Embedder::Semantic));
        assert!(collected.contains(&Embedder::Causal));
        assert!(collected.contains(&Embedder::Code));
        println!("[PASS] EmbedderMask::iter() yields correct embedders");
    }

    #[test]
    fn test_embedder_group_temporal() {
        let temporal = EmbedderGroup::Temporal.embedders();
        assert_eq!(temporal.count(), 3);
        assert!(temporal.contains(Embedder::TemporalRecent));
        assert!(temporal.contains(Embedder::TemporalPeriodic));
        assert!(temporal.contains(Embedder::TemporalPositional));
        assert!(!temporal.contains(Embedder::Semantic));
        println!("[PASS] EmbedderGroup::Temporal includes E2, E3, E4");
    }

    #[test]
    fn test_embedder_group_dense() {
        let dense = EmbedderGroup::Dense.embedders();
        assert_eq!(dense.count(), 10); // 13 - 3 (E6, E12, E13)
        assert!(dense.contains(Embedder::Semantic));
        assert!(dense.contains(Embedder::Code));
        assert!(!dense.contains(Embedder::Sparse));
        assert!(!dense.contains(Embedder::LateInteraction));
        assert!(!dense.contains(Embedder::KeywordSplade));
        println!("[PASS] EmbedderGroup::Dense excludes sparse/token-level");
    }

    #[test]
    fn test_embedder_serde() {
        let e = Embedder::Causal;
        let json = serde_json::to_string(&e).expect("serialize");
        let recovered: Embedder = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(e, recovered);
        println!("[PASS] Embedder serialization roundtrip works");
    }

    #[test]
    fn test_embedder_mask_serde() {
        let mask = EmbedderMask::from_slice(&[Embedder::Semantic, Embedder::Code]);
        let json = serde_json::to_string(&mask).expect("serialize");
        let recovered: EmbedderMask = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mask, recovered);
        println!("[PASS] EmbedderMask serialization roundtrip works");
    }

    #[test]
    fn test_type_classification() {
        assert!(Embedder::Semantic.is_dense());
        assert!(!Embedder::Semantic.is_sparse());
        assert!(!Embedder::Semantic.is_token_level());

        assert!(!Embedder::Sparse.is_dense());
        assert!(Embedder::Sparse.is_sparse());
        assert!(!Embedder::Sparse.is_token_level());

        assert!(!Embedder::LateInteraction.is_dense());
        assert!(!Embedder::LateInteraction.is_sparse());
        assert!(Embedder::LateInteraction.is_token_level());

        println!("[PASS] Type classification methods work correctly");
    }

    #[test]
    fn test_default_temperature() {
        assert_eq!(Embedder::Semantic.default_temperature(), 1.0);
        assert_eq!(Embedder::Causal.default_temperature(), 1.2);
        assert_eq!(Embedder::Code.default_temperature(), 0.9);
        assert_eq!(Embedder::Hdc.default_temperature(), 1.5);
        assert_eq!(Embedder::KeywordSplade.default_temperature(), 1.1);
        println!("[PASS] default_temperature() returns correct values");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Embedder::Semantic), "E1_Semantic");
        assert_eq!(format!("{}", Embedder::KeywordSplade), "E13_SPLADE");
        println!("[PASS] Display trait works correctly");
    }

    #[test]
    fn test_embedder_dims_primary_dim() {
        assert_eq!(EmbedderDims::Dense(1024).primary_dim(), 1024);
        assert_eq!(
            EmbedderDims::Sparse { vocab_size: 30522 }.primary_dim(),
            30522
        );
        assert_eq!(
            EmbedderDims::TokenLevel { per_token: 128 }.primary_dim(),
            128
        );
        println!("[PASS] EmbedderDims::primary_dim() works correctly");
    }

    // ========================================
    // E8 Naming Tests
    // ========================================

    #[test]
    fn test_e8_canonical_name() {
        assert_eq!(Embedder::Graph.name(), "E8_Graph");
        assert_eq!(Embedder::Graph.short_name(), "E8");
        println!("[PASS] E8 canonical name is E8_Graph");
    }

    #[test]
    fn test_e8_purpose() {
        assert_eq!(Embedder::Graph.purpose(), "V_connectivity");
        println!("[PASS] E8 purpose is V_connectivity");
    }

    #[test]
    fn test_e8_from_name_canonical() {
        // All accepted name forms
        assert_eq!(Embedder::from_name("E8").unwrap(), Embedder::Graph);
        assert_eq!(Embedder::from_name("E8_Graph").unwrap(), Embedder::Graph);
        assert_eq!(Embedder::from_name("graph").unwrap(), Embedder::Graph);
        assert_eq!(Embedder::from_name("Graph").unwrap(), Embedder::Graph);
        assert_eq!(Embedder::from_name("GRAPH").unwrap(), Embedder::Graph);
        println!("[PASS] E8 canonical names parse correctly");
    }

    #[test]
    fn test_e8_from_name_old_emotional_rejected() {
        // Old "Emotional" name must fail fast (no backwards compat)
        let result = Embedder::from_name("Emotional");
        assert!(result.is_err(), "Emotional should be rejected");
        let result = Embedder::from_name("E8_Emotional");
        assert!(result.is_err(), "E8_Emotional should be rejected");
        println!("[PASS] Old Emotional names are rejected");
    }

    #[test]
    fn test_e8_from_name_unknown() {
        let result = Embedder::from_name("nonexistent");
        assert!(result.is_err());
        match result {
            Err(EmbedderNameError::Unknown { name, valid_names }) => {
                assert_eq!(name, "nonexistent");
                assert!(!valid_names.is_empty());
                println!("[PASS] Unknown names error correctly");
            }
            Ok(_) => unreachable!("should have been an error"),
        }
    }

    #[test]
    fn test_e8_all_names() {
        let names = Embedder::all_names();
        assert!(names.contains(&"E8_Graph"));
        assert_eq!(names.len(), 13);
        assert!(!names.contains(&"E8_Emotional"));
        println!("[PASS] all_names() returns 13 canonical names including E8_Graph");
    }

    #[test]
    fn test_e8_name_error_code() {
        let unknown = EmbedderNameError::Unknown {
            name: "foo".to_string(),
            valid_names: vec!["E1_Semantic"],
        };
        assert!(format!("{}", unknown).contains("E_EMB_NAME_001"));
        println!("[PASS] EmbedderNameError code is correct");
    }

    #[test]
    fn test_e8_index_unchanged() {
        // E8 (Graph) must remain at index 7
        assert_eq!(Embedder::Graph.index(), 7);
        assert_eq!(Embedder::from_index(7), Some(Embedder::Graph));
        println!("[PASS] E8 index is still 7");
    }
}
