//! Graph linking edge types derived from embedder agreement patterns.
//!
//! # Architecture Reference
//!
//! These 8 edge types are distinct from the Marblestone EdgeType (Semantic/Temporal/Causal/Hierarchical/Contradicts).
//! Graph linking edge types are derived from which embedders agree that two nodes are similar.
//!
//! # Edge Types
//!
//! | Type | Primary Embedder | Description |
//! |------|-----------------|-------------|
//! | SemanticSimilar | E1 | High E1 similarity (semantic) |
//! | CodeRelated | E7 | Code/implementation relationship |
//! | EntityShared | E11 | Shared entities (KEPLER) |
//! | CausalChain | E5 | Causal relationship (asymmetric) |
//! | GraphConnected | E8 | Graph connectivity (asymmetric) |
//! | IntentAligned | E10 | Same goal/intent |
//! | KeywordOverlap | E6, E13 | Keyword/lexical similarity |
//! | MultiAgreement | 3+ embedders | Multiple embedders agree |

use serde::{Deserialize, Serialize};
use std::fmt;

/// Edge types derived from embedder agreement patterns.
///
/// Unlike Marblestone EdgeType which describes the semantic relationship type,
/// GraphLinkEdgeType describes which embedder(s) detected the relationship.
///
/// # Asymmetric Types
///
/// `CausalChain` (E5) and `GraphConnected` (E8) are asymmetric per ARCH-18.
/// Use `DirectedRelation` to specify direction for these types.
///
/// # Examples
///
/// ```
/// use context_graph_core::graph_linking::GraphLinkEdgeType;
///
/// let edge_type = GraphLinkEdgeType::SemanticSimilar;
/// assert!(!edge_type.is_asymmetric());
/// assert_eq!(edge_type.primary_embedder_index(), Some(0)); // E1
///
/// let causal = GraphLinkEdgeType::CausalChain;
/// assert!(causal.is_asymmetric());
/// assert_eq!(causal.primary_embedder_index(), Some(4)); // E5
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum GraphLinkEdgeType {
    /// High E1 (semantic) similarity.
    /// The foundation edge type - two memories share similar meaning.
    SemanticSimilar = 0,

    /// Code/implementation relationship via E7 (Qodo-Embed).
    /// Two memories share code patterns, function signatures, or implementations.
    CodeRelated = 1,

    /// Shared entities via E11 (KEPLER).
    /// Two memories reference the same named entities.
    EntityShared = 2,

    /// Causal relationship via E5 (Longformer SCM).
    /// ASYMMETRIC: direction matters (cause → effect).
    /// Per AP-77: MUST NOT use symmetric cosine.
    CausalChain = 3,

    /// Graph connectivity via E8 (MiniLM emotional/connectivity).
    /// ASYMMETRIC: direction matters (source → target).
    GraphConnected = 4,

    /// Same goal/intent via E10 (CLIP multimodal).
    /// Two memories share the same underlying purpose or goal.
    IntentAligned = 5,

    /// Keyword/lexical overlap via E6 (SPLADE) or E13 (SPLADE v3).
    /// Two memories share exact keywords or expanded terms.
    KeywordOverlap = 6,

    /// Multiple embedders (3+) agree on similarity.
    /// The strongest signal - multiple perspectives confirm the relationship.
    MultiAgreement = 7,
}

impl GraphLinkEdgeType {
    /// Total number of edge types.
    pub const COUNT: usize = 8;

    /// Check if this edge type requires asymmetric similarity handling.
    ///
    /// Per ARCH-18 and AP-77:
    /// - CausalChain (E5): cause→effect direction matters
    /// - GraphConnected (E8): source→target direction matters
    #[inline]
    pub fn is_asymmetric(&self) -> bool {
        matches!(self, Self::CausalChain | Self::GraphConnected)
    }

    /// Get the primary embedder index for this edge type.
    ///
    /// Returns `None` for `MultiAgreement` since it requires 3+ embedders.
    ///
    /// # Returns
    ///
    /// - `Some(0)` = E1 Semantic
    /// - `Some(4)` = E5 Causal
    /// - `Some(5)` = E6 Sparse (keyword)
    /// - `Some(6)` = E7 Code
    /// - `Some(7)` = E8 Emotional/Graph
    /// - `Some(9)` = E10 Multimodal
    /// - `Some(10)` = E11 Entity
    /// - `None` = MultiAgreement (no single primary)
    pub fn primary_embedder_index(&self) -> Option<usize> {
        match self {
            Self::SemanticSimilar => Some(0),  // E1
            Self::CodeRelated => Some(6),      // E7
            Self::EntityShared => Some(10),    // E11
            Self::CausalChain => Some(4),      // E5
            Self::GraphConnected => Some(7),   // E8
            Self::IntentAligned => Some(9),    // E10
            Self::KeywordOverlap => Some(5),   // E6 (or E13=12)
            Self::MultiAgreement => None,      // No single primary
        }
    }

    /// Get the default similarity threshold for this edge type.
    ///
    /// Higher thresholds for specialized embedders, lower for broad semantic.
    pub fn default_threshold(&self) -> f32 {
        match self {
            Self::SemanticSimilar => 0.75, // E1 is broad, need higher threshold
            Self::CodeRelated => 0.70,     // E7 code is specific
            Self::EntityShared => 0.65,    // E11 entity matching
            Self::CausalChain => 0.60,     // E5 causal chains
            Self::GraphConnected => 0.60,  // E8 graph structure
            Self::IntentAligned => 0.70,   // E10 intent matching
            Self::KeywordOverlap => 0.50,  // Sparse similarity scores differently
            Self::MultiAgreement => 0.60,  // Multiple agree = strong signal
        }
    }

    /// Get a human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::SemanticSimilar => "Semantically similar (E1)",
            Self::CodeRelated => "Code/implementation related (E7)",
            Self::EntityShared => "Shared entities (E11)",
            Self::CausalChain => "Causal relationship (E5, asymmetric)",
            Self::GraphConnected => "Graph connectivity (E8, asymmetric)",
            Self::IntentAligned => "Intent/goal aligned (E10)",
            Self::KeywordOverlap => "Keyword overlap (E6/E13)",
            Self::MultiAgreement => "Multi-embedder agreement (3+)",
        }
    }

    /// Convert to u8 for storage.
    #[inline]
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Create from u8 value.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::SemanticSimilar),
            1 => Some(Self::CodeRelated),
            2 => Some(Self::EntityShared),
            3 => Some(Self::CausalChain),
            4 => Some(Self::GraphConnected),
            5 => Some(Self::IntentAligned),
            6 => Some(Self::KeywordOverlap),
            7 => Some(Self::MultiAgreement),
            _ => None,
        }
    }

    /// Get all variants.
    pub fn all() -> [Self; 8] {
        [
            Self::SemanticSimilar,
            Self::CodeRelated,
            Self::EntityShared,
            Self::CausalChain,
            Self::GraphConnected,
            Self::IntentAligned,
            Self::KeywordOverlap,
            Self::MultiAgreement,
        ]
    }

    /// Get asymmetric variants only.
    pub fn asymmetric_variants() -> [Self; 2] {
        [Self::CausalChain, Self::GraphConnected]
    }

    /// Get symmetric variants only.
    pub fn symmetric_variants() -> [Self; 6] {
        [
            Self::SemanticSimilar,
            Self::CodeRelated,
            Self::EntityShared,
            Self::IntentAligned,
            Self::KeywordOverlap,
            Self::MultiAgreement,
        ]
    }
}

impl Default for GraphLinkEdgeType {
    /// Default to SemanticSimilar (E1-based).
    fn default() -> Self {
        Self::SemanticSimilar
    }
}

impl fmt::Display for GraphLinkEdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::SemanticSimilar => "semantic_similar",
            Self::CodeRelated => "code_related",
            Self::EntityShared => "entity_shared",
            Self::CausalChain => "causal_chain",
            Self::GraphConnected => "graph_connected",
            Self::IntentAligned => "intent_aligned",
            Self::KeywordOverlap => "keyword_overlap",
            Self::MultiAgreement => "multi_agreement",
        };
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count() {
        assert_eq!(GraphLinkEdgeType::COUNT, 8);
        assert_eq!(GraphLinkEdgeType::all().len(), 8);
    }

    #[test]
    fn test_default() {
        assert_eq!(GraphLinkEdgeType::default(), GraphLinkEdgeType::SemanticSimilar);
    }

    #[test]
    fn test_is_asymmetric() {
        // Asymmetric: E5 (causal) and E8 (graph)
        assert!(GraphLinkEdgeType::CausalChain.is_asymmetric());
        assert!(GraphLinkEdgeType::GraphConnected.is_asymmetric());

        // Symmetric: all others
        assert!(!GraphLinkEdgeType::SemanticSimilar.is_asymmetric());
        assert!(!GraphLinkEdgeType::CodeRelated.is_asymmetric());
        assert!(!GraphLinkEdgeType::EntityShared.is_asymmetric());
        assert!(!GraphLinkEdgeType::IntentAligned.is_asymmetric());
        assert!(!GraphLinkEdgeType::KeywordOverlap.is_asymmetric());
        assert!(!GraphLinkEdgeType::MultiAgreement.is_asymmetric());
    }

    #[test]
    fn test_asymmetric_variants() {
        let asymmetric = GraphLinkEdgeType::asymmetric_variants();
        assert_eq!(asymmetric.len(), 2);
        assert!(asymmetric.contains(&GraphLinkEdgeType::CausalChain));
        assert!(asymmetric.contains(&GraphLinkEdgeType::GraphConnected));
    }

    #[test]
    fn test_symmetric_variants() {
        let symmetric = GraphLinkEdgeType::symmetric_variants();
        assert_eq!(symmetric.len(), 6);
        for variant in symmetric {
            assert!(!variant.is_asymmetric());
        }
    }

    #[test]
    fn test_primary_embedder_index() {
        assert_eq!(GraphLinkEdgeType::SemanticSimilar.primary_embedder_index(), Some(0));
        assert_eq!(GraphLinkEdgeType::CausalChain.primary_embedder_index(), Some(4));
        assert_eq!(GraphLinkEdgeType::KeywordOverlap.primary_embedder_index(), Some(5));
        assert_eq!(GraphLinkEdgeType::CodeRelated.primary_embedder_index(), Some(6));
        assert_eq!(GraphLinkEdgeType::GraphConnected.primary_embedder_index(), Some(7));
        assert_eq!(GraphLinkEdgeType::IntentAligned.primary_embedder_index(), Some(9));
        assert_eq!(GraphLinkEdgeType::EntityShared.primary_embedder_index(), Some(10));
        assert_eq!(GraphLinkEdgeType::MultiAgreement.primary_embedder_index(), None);
    }

    #[test]
    fn test_default_thresholds_valid() {
        for edge_type in GraphLinkEdgeType::all() {
            let threshold = edge_type.default_threshold();
            assert!(threshold >= 0.0 && threshold <= 1.0,
                    "{:?} has invalid threshold: {}", edge_type, threshold);
        }
    }

    #[test]
    fn test_u8_roundtrip() {
        for edge_type in GraphLinkEdgeType::all() {
            let u8_val = edge_type.as_u8();
            let recovered = GraphLinkEdgeType::from_u8(u8_val);
            assert_eq!(recovered, Some(edge_type), "Roundtrip failed for {:?}", edge_type);
        }
    }

    #[test]
    fn test_from_u8_invalid() {
        assert!(GraphLinkEdgeType::from_u8(8).is_none());
        assert!(GraphLinkEdgeType::from_u8(255).is_none());
    }

    #[test]
    fn test_display() {
        assert_eq!(GraphLinkEdgeType::SemanticSimilar.to_string(), "semantic_similar");
        assert_eq!(GraphLinkEdgeType::CausalChain.to_string(), "causal_chain");
        assert_eq!(GraphLinkEdgeType::MultiAgreement.to_string(), "multi_agreement");
    }

    #[test]
    fn test_serde_roundtrip() {
        for edge_type in GraphLinkEdgeType::all() {
            let json = serde_json::to_string(&edge_type).unwrap();
            let recovered: GraphLinkEdgeType = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, edge_type, "Serde roundtrip failed for {:?}", edge_type);
        }
    }

    #[test]
    fn test_serde_snake_case() {
        let json = serde_json::to_string(&GraphLinkEdgeType::SemanticSimilar).unwrap();
        assert_eq!(json, "\"semantic_similar\"");

        let json = serde_json::to_string(&GraphLinkEdgeType::MultiAgreement).unwrap();
        assert_eq!(json, "\"multi_agreement\"");
    }

    #[test]
    fn test_description_non_empty() {
        for edge_type in GraphLinkEdgeType::all() {
            let desc = edge_type.description();
            assert!(!desc.is_empty(), "{:?} has empty description", edge_type);
        }
    }

    #[test]
    fn test_u8_values_sequential() {
        for (i, edge_type) in GraphLinkEdgeType::all().iter().enumerate() {
            assert_eq!(edge_type.as_u8() as usize, i,
                       "{:?} should have u8 value {}", edge_type, i);
        }
    }
}
