//! Directed relation enum for asymmetric edges.
//!
//! # Architecture Reference
//!
//! - ARCH-18: E5 (causal) and E8 (graph/emotional) use asymmetric similarity
//! - AP-77: E5 MUST NOT use symmetric cosine - FAIL FAST
//!
//! For E5 causal edges:
//! - Forward = cause → effect (1.2x boost per spec)
//! - Backward = effect → cause (0.8x dampening per spec)
//!
//! For E8 graph edges:
//! - Forward = source → target (what this imports/uses)
//! - Backward = target → source (what imports/uses this)

use serde::{Deserialize, Serialize};
use std::fmt;

/// Direction of a relationship for asymmetric embedders (E5, E8).
///
/// Symmetric embedders (E1, E6, E7, E9, E10, E11, E12, E13) use `Symmetric`.
/// Temporal embedders (E2, E3, E4) are excluded from edge detection entirely.
///
/// # Examples
///
/// ```
/// use context_graph_core::graph_linking::DirectedRelation;
///
/// let causal = DirectedRelation::Forward;
/// assert!(!causal.is_symmetric());
/// assert_eq!(causal.similarity_modifier(), 1.2);
///
/// let backward = DirectedRelation::Backward;
/// assert_eq!(backward.similarity_modifier(), 0.8);
///
/// let symmetric = DirectedRelation::Symmetric;
/// assert!(symmetric.is_symmetric());
/// assert_eq!(symmetric.similarity_modifier(), 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum DirectedRelation {
    /// Symmetric relationship (most embedders).
    /// Similarity(A, B) = Similarity(B, A)
    #[default]
    Symmetric = 0,

    /// Forward direction: source → target
    /// - E5 causal: cause → effect (1.2x boost)
    /// - E8 graph: importer → imported
    Forward = 1,

    /// Backward direction: target → source
    /// - E5 causal: effect → cause (0.8x dampening)
    /// - E8 graph: imported → importer
    Backward = 2,
}

impl DirectedRelation {
    /// Check if this is a symmetric (undirected) relation.
    #[inline]
    pub fn is_symmetric(&self) -> bool {
        matches!(self, Self::Symmetric)
    }

    /// Check if this is a forward (cause→effect, source→target) relation.
    #[inline]
    pub fn is_forward(&self) -> bool {
        matches!(self, Self::Forward)
    }

    /// Check if this is a backward (effect→cause, target→source) relation.
    #[inline]
    pub fn is_backward(&self) -> bool {
        matches!(self, Self::Backward)
    }

    /// Get the similarity modifier for this direction.
    ///
    /// Per spec:
    /// - Forward (cause→effect): 1.2x boost
    /// - Backward (effect→cause): 0.8x dampening
    /// - Symmetric: 1.0x (no modification)
    ///
    /// # Returns
    ///
    /// Multiplicative modifier for similarity scores.
    #[inline]
    pub fn similarity_modifier(&self) -> f32 {
        match self {
            Self::Symmetric => 1.0,
            Self::Forward => 1.2,
            Self::Backward => 0.8,
        }
    }

    /// Get the opposite direction.
    ///
    /// - Symmetric → Symmetric (unchanged)
    /// - Forward → Backward
    /// - Backward → Forward
    #[inline]
    pub fn reverse(&self) -> Self {
        match self {
            Self::Symmetric => Self::Symmetric,
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }

    /// Convert to u8 for storage.
    #[inline]
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Create from u8 value.
    ///
    /// # Returns
    ///
    /// - `Some(DirectedRelation)` if value is 0, 1, or 2
    /// - `None` if value is out of range
    #[inline]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Symmetric),
            1 => Some(Self::Forward),
            2 => Some(Self::Backward),
            _ => None,
        }
    }

    /// Get all variants.
    pub fn all() -> [Self; 3] {
        [Self::Symmetric, Self::Forward, Self::Backward]
    }
}

impl fmt::Display for DirectedRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Symmetric => write!(f, "symmetric"),
            Self::Forward => write!(f, "forward"),
            Self::Backward => write!(f, "backward"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_symmetric() {
        assert_eq!(DirectedRelation::default(), DirectedRelation::Symmetric);
    }

    #[test]
    fn test_is_symmetric() {
        assert!(DirectedRelation::Symmetric.is_symmetric());
        assert!(!DirectedRelation::Forward.is_symmetric());
        assert!(!DirectedRelation::Backward.is_symmetric());
    }

    #[test]
    fn test_is_forward() {
        assert!(!DirectedRelation::Symmetric.is_forward());
        assert!(DirectedRelation::Forward.is_forward());
        assert!(!DirectedRelation::Backward.is_forward());
    }

    #[test]
    fn test_is_backward() {
        assert!(!DirectedRelation::Symmetric.is_backward());
        assert!(!DirectedRelation::Forward.is_backward());
        assert!(DirectedRelation::Backward.is_backward());
    }

    #[test]
    fn test_similarity_modifiers() {
        assert_eq!(DirectedRelation::Symmetric.similarity_modifier(), 1.0);
        assert_eq!(DirectedRelation::Forward.similarity_modifier(), 1.2);
        assert_eq!(DirectedRelation::Backward.similarity_modifier(), 0.8);
    }

    #[test]
    fn test_reverse() {
        assert_eq!(
            DirectedRelation::Symmetric.reverse(),
            DirectedRelation::Symmetric
        );
        assert_eq!(
            DirectedRelation::Forward.reverse(),
            DirectedRelation::Backward
        );
        assert_eq!(
            DirectedRelation::Backward.reverse(),
            DirectedRelation::Forward
        );
    }

    #[test]
    fn test_u8_roundtrip() {
        for dir in DirectedRelation::all() {
            let u8_val = dir.as_u8();
            let recovered = DirectedRelation::from_u8(u8_val);
            assert_eq!(recovered, Some(dir), "Roundtrip failed for {:?}", dir);
        }
    }

    #[test]
    fn test_from_u8_invalid() {
        assert!(DirectedRelation::from_u8(3).is_none());
        assert!(DirectedRelation::from_u8(255).is_none());
    }

    #[test]
    fn test_display() {
        assert_eq!(DirectedRelation::Symmetric.to_string(), "symmetric");
        assert_eq!(DirectedRelation::Forward.to_string(), "forward");
        assert_eq!(DirectedRelation::Backward.to_string(), "backward");
    }

    #[test]
    fn test_serde_roundtrip() {
        for dir in DirectedRelation::all() {
            let json = serde_json::to_string(&dir).unwrap();
            let recovered: DirectedRelation = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, dir, "Serde roundtrip failed for {:?}", dir);
        }
    }

    #[test]
    fn test_all_variants() {
        let all = DirectedRelation::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&DirectedRelation::Symmetric));
        assert!(all.contains(&DirectedRelation::Forward));
        assert!(all.contains(&DirectedRelation::Backward));
    }
}
