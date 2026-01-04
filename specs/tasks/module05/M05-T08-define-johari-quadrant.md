---
id: "M05-T08"
title: "Define JohariQuadrant Enum and SuggestedAction"
description: |
  Implement JohariQuadrant enum for memory classification.
  Variants: Open (low entropy, high coherence), Blind (high entropy, low coherence),
  Hidden (low entropy, low coherence), Unknown (high entropy, high coherence).
  SuggestedAction enum: DirectRecall, TriggerDream, GetNeighborhood, EpistemicAction,
  CritiqueContext, Curate.
  Include methods: name(), is_well_understood(), requires_exploration().
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 2
sequence: 8
depends_on: []
spec_refs:
  - "TECH-UTL-005 Section 4.1"
  - "SPEC-UTL-005 Section 7"
files_to_create:
  - path: "crates/context-graph-utl/src/johari/quadrant.rs"
    description: "JohariQuadrant and SuggestedAction enums"
files_to_modify:
  - path: "crates/context-graph-utl/src/johari/mod.rs"
    description: "Add quadrant module and re-export JohariQuadrant, SuggestedAction"
  - path: "crates/context-graph-utl/src/lib.rs"
    description: "Re-export JohariQuadrant, SuggestedAction at crate root"
test_file: "crates/context-graph-utl/tests/johari_tests.rs"
---

## Overview

The Johari Window is a cognitive framework adapted for memory classification. Memories are classified into four quadrants based on their entropy (surprise) and coherence levels. Each quadrant has distinct retrieval strategies and suggested actions.

## Johari Window Quadrants

```
                    ENTROPY (Surprise)
                    Low         High
             ┌─────────────┬─────────────┐
        High │    OPEN     │   UNKNOWN   │
COHERENCE    │ (Confident) │ (Exploring) │
             ├─────────────┼─────────────┤
        Low  │   HIDDEN    │    BLIND    │
             │ (Isolated)  │ (Confused)  │
             └─────────────┴─────────────┘
```

| Quadrant | Entropy | Coherence | Meaning |
|----------|---------|-----------|---------|
| Open     | Low     | High      | Well-understood, confidently retrievable |
| Blind    | High    | Low       | Surprising and disconnected - confusion |
| Hidden   | Low     | High      | Known but not well connected to context |
| Unknown  | High    | High      | Novel but fits - exploration territory |

## Implementation Requirements

### File: `crates/context-graph-utl/src/johari/quadrant.rs`

```rust
//! Johari Window quadrants for memory classification.
//!
//! # Johari Window Model
//!
//! The Johari Window classifies memories based on:
//! - **Entropy**: Surprise/novelty level (low = expected, high = surprising)
//! - **Coherence**: How well content fits with existing knowledge
//!
//! # Quadrants
//!
//! - **Open**: Low entropy, high coherence - well understood
//! - **Blind**: High entropy, low coherence - confusing
//! - **Hidden**: Low entropy, low coherence - isolated knowledge
//! - **Unknown**: High entropy, high coherence - exploration territory
//!
//! # Constitution Reference
//!
//! - SPEC-UTL-005 Section 7: Johari quadrant definitions
//! - TECH-UTL-005 Section 4.1: Classification algorithm

use serde::{Deserialize, Serialize};

/// Johari Window quadrant for memory classification.
///
/// Each quadrant represents a different relationship between
/// entropy (surprise) and coherence (context fit).
///
/// # Classification Matrix
///
/// | Quadrant | Entropy | Coherence |
/// |----------|---------|-----------|
/// | Open     | Low     | High      |
/// | Blind    | High    | Low       |
/// | Hidden   | Low     | Low       |
/// | Unknown  | High    | High      |
///
/// # Example
///
/// ```
/// use context_graph_utl::johari::JohariQuadrant;
///
/// let quadrant = JohariQuadrant::Open;
/// assert!(quadrant.is_well_understood());
/// assert!(!quadrant.requires_exploration());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum JohariQuadrant {
    /// Low entropy, high coherence.
    /// Well-understood content that fits naturally with existing knowledge.
    /// Retrieval: Direct, confident, minimal exploration needed.
    #[default]
    Open = 0,

    /// High entropy, low coherence.
    /// Surprising content that doesn't fit with context.
    /// Indicates confusion or misunderstanding.
    /// Retrieval: Requires clarification, context building.
    Blind = 1,

    /// Low entropy, low coherence.
    /// Expected content but not well connected to current context.
    /// Isolated knowledge that may need integration.
    /// Retrieval: May benefit from neighborhood exploration.
    Hidden = 2,

    /// High entropy, high coherence.
    /// Novel content that surprisingly fits well.
    /// Territory for productive exploration.
    /// Retrieval: Deep exploration, pattern discovery.
    Unknown = 3,
}

impl JohariQuadrant {
    /// Get the string name of this quadrant.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert_eq!(JohariQuadrant::Open.name(), "Open");
    /// assert_eq!(JohariQuadrant::Blind.name(), "Blind");
    /// ```
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Blind => "Blind",
            Self::Hidden => "Hidden",
            Self::Unknown => "Unknown",
        }
    }

    /// Check if this quadrant represents well-understood content.
    ///
    /// Returns true for Open quadrant (low entropy, high coherence).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert!(JohariQuadrant::Open.is_well_understood());
    /// assert!(!JohariQuadrant::Blind.is_well_understood());
    /// ```
    #[inline]
    pub fn is_well_understood(&self) -> bool {
        matches!(self, Self::Open)
    }

    /// Check if this quadrant requires exploration.
    ///
    /// Returns true for Unknown and Blind quadrants.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert!(JohariQuadrant::Unknown.requires_exploration());
    /// assert!(JohariQuadrant::Blind.requires_exploration());
    /// assert!(!JohariQuadrant::Open.requires_exploration());
    /// ```
    #[inline]
    pub fn requires_exploration(&self) -> bool {
        matches!(self, Self::Unknown | Self::Blind)
    }

    /// Check if this quadrant has high entropy (surprise).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert!(JohariQuadrant::Unknown.is_high_entropy());
    /// assert!(JohariQuadrant::Blind.is_high_entropy());
    /// assert!(!JohariQuadrant::Open.is_high_entropy());
    /// ```
    #[inline]
    pub fn is_high_entropy(&self) -> bool {
        matches!(self, Self::Unknown | Self::Blind)
    }

    /// Check if this quadrant has high coherence.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert!(JohariQuadrant::Open.is_high_coherence());
    /// assert!(JohariQuadrant::Unknown.is_high_coherence());
    /// assert!(!JohariQuadrant::Blind.is_high_coherence());
    /// ```
    #[inline]
    pub fn is_high_coherence(&self) -> bool {
        matches!(self, Self::Open | Self::Unknown)
    }

    /// Get the suggested action for this quadrant.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::{JohariQuadrant, SuggestedAction};
    ///
    /// assert_eq!(JohariQuadrant::Open.suggested_action(), SuggestedAction::DirectRecall);
    /// assert_eq!(JohariQuadrant::Unknown.suggested_action(), SuggestedAction::EpistemicAction);
    /// ```
    #[inline]
    pub fn suggested_action(&self) -> SuggestedAction {
        match self {
            Self::Open => SuggestedAction::DirectRecall,
            Self::Blind => SuggestedAction::CritiqueContext,
            Self::Hidden => SuggestedAction::GetNeighborhood,
            Self::Unknown => SuggestedAction::EpistemicAction,
        }
    }

    /// Get the confidence level for this quadrant.
    ///
    /// Returns a value in [0, 1] where:
    /// - Open: 0.9 (high confidence)
    /// - Hidden: 0.6 (moderate confidence)
    /// - Unknown: 0.4 (low confidence, exploring)
    /// - Blind: 0.2 (very low confidence, confused)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert!(JohariQuadrant::Open.confidence_level() > JohariQuadrant::Blind.confidence_level());
    /// ```
    #[inline]
    pub fn confidence_level(&self) -> f32 {
        match self {
            Self::Open => 0.9,
            Self::Hidden => 0.6,
            Self::Unknown => 0.4,
            Self::Blind => 0.2,
        }
    }

    /// Classify based on entropy and coherence values.
    ///
    /// Uses default thresholds (0.5 for both).
    /// For custom thresholds, use JohariClassifier.
    ///
    /// # Arguments
    ///
    /// * `entropy` - Entropy/surprise level [0, 1]
    /// * `coherence` - Coherence level [0, 1]
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// assert_eq!(JohariQuadrant::classify(0.2, 0.8), JohariQuadrant::Open);
    /// assert_eq!(JohariQuadrant::classify(0.8, 0.2), JohariQuadrant::Blind);
    /// assert_eq!(JohariQuadrant::classify(0.2, 0.2), JohariQuadrant::Hidden);
    /// assert_eq!(JohariQuadrant::classify(0.8, 0.8), JohariQuadrant::Unknown);
    /// ```
    #[inline]
    pub fn classify(entropy: f32, coherence: f32) -> Self {
        Self::classify_with_thresholds(entropy, coherence, 0.5, 0.5)
    }

    /// Classify with custom thresholds.
    ///
    /// # Arguments
    ///
    /// * `entropy` - Entropy/surprise level [0, 1]
    /// * `coherence` - Coherence level [0, 1]
    /// * `entropy_threshold` - Threshold for high entropy
    /// * `coherence_threshold` - Threshold for high coherence
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::JohariQuadrant;
    ///
    /// // More lenient thresholds
    /// let quadrant = JohariQuadrant::classify_with_thresholds(0.6, 0.4, 0.7, 0.3);
    /// assert_eq!(quadrant, JohariQuadrant::Open);
    /// ```
    #[inline]
    pub fn classify_with_thresholds(
        entropy: f32,
        coherence: f32,
        entropy_threshold: f32,
        coherence_threshold: f32,
    ) -> Self {
        let high_entropy = entropy >= entropy_threshold;
        let high_coherence = coherence >= coherence_threshold;

        match (high_entropy, high_coherence) {
            (false, true) => Self::Open,
            (true, false) => Self::Blind,
            (false, false) => Self::Hidden,
            (true, true) => Self::Unknown,
        }
    }
}

impl std::fmt::Display for JohariQuadrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Suggested action based on Johari quadrant classification.
///
/// Each action represents a different strategy for handling the memory
/// or query based on its entropy/coherence characteristics.
///
/// # Actions
///
/// - `DirectRecall`: Direct retrieval, high confidence
/// - `TriggerDream`: Trigger consolidation/integration
/// - `GetNeighborhood`: Explore connected memories
/// - `EpistemicAction`: Active exploration, hypothesis testing
/// - `CritiqueContext`: Question assumptions, verify context
/// - `Curate`: Organize and prune knowledge
///
/// # Example
///
/// ```
/// use context_graph_utl::johari::SuggestedAction;
///
/// let action = SuggestedAction::DirectRecall;
/// assert!(action.is_retrieval_focused());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum SuggestedAction {
    /// Direct memory retrieval with high confidence.
    /// Used for Open quadrant (low entropy, high coherence).
    /// Minimal exploration needed.
    #[default]
    DirectRecall = 0,

    /// Trigger memory consolidation or integration.
    /// Used when memories need to be linked or organized.
    TriggerDream = 1,

    /// Explore neighboring memories in the knowledge graph.
    /// Used for Hidden quadrant to build connections.
    GetNeighborhood = 2,

    /// Active exploration with hypothesis testing.
    /// Used for Unknown quadrant to probe new territory.
    EpistemicAction = 3,

    /// Question assumptions and verify context.
    /// Used for Blind quadrant when confusion is detected.
    CritiqueContext = 4,

    /// Organize and prune knowledge base.
    /// Used for maintenance and optimization.
    Curate = 5,
}

impl SuggestedAction {
    /// Get the string name of this action.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::SuggestedAction;
    ///
    /// assert_eq!(SuggestedAction::DirectRecall.name(), "DirectRecall");
    /// ```
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::DirectRecall => "DirectRecall",
            Self::TriggerDream => "TriggerDream",
            Self::GetNeighborhood => "GetNeighborhood",
            Self::EpistemicAction => "EpistemicAction",
            Self::CritiqueContext => "CritiqueContext",
            Self::Curate => "Curate",
        }
    }

    /// Check if this action is focused on retrieval.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::SuggestedAction;
    ///
    /// assert!(SuggestedAction::DirectRecall.is_retrieval_focused());
    /// assert!(SuggestedAction::GetNeighborhood.is_retrieval_focused());
    /// assert!(!SuggestedAction::Curate.is_retrieval_focused());
    /// ```
    #[inline]
    pub fn is_retrieval_focused(&self) -> bool {
        matches!(self, Self::DirectRecall | Self::GetNeighborhood)
    }

    /// Check if this action involves exploration.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::SuggestedAction;
    ///
    /// assert!(SuggestedAction::EpistemicAction.is_exploratory());
    /// assert!(SuggestedAction::GetNeighborhood.is_exploratory());
    /// assert!(!SuggestedAction::DirectRecall.is_exploratory());
    /// ```
    #[inline]
    pub fn is_exploratory(&self) -> bool {
        matches!(self, Self::EpistemicAction | Self::GetNeighborhood)
    }

    /// Check if this action requires context modification.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::SuggestedAction;
    ///
    /// assert!(SuggestedAction::CritiqueContext.modifies_context());
    /// assert!(SuggestedAction::TriggerDream.modifies_context());
    /// ```
    #[inline]
    pub fn modifies_context(&self) -> bool {
        matches!(self, Self::CritiqueContext | Self::TriggerDream | Self::Curate)
    }

    /// Get the computational cost estimate (1-5 scale).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::johari::SuggestedAction;
    ///
    /// assert_eq!(SuggestedAction::DirectRecall.cost_estimate(), 1);
    /// assert_eq!(SuggestedAction::EpistemicAction.cost_estimate(), 4);
    /// ```
    #[inline]
    pub fn cost_estimate(&self) -> u8 {
        match self {
            Self::DirectRecall => 1,
            Self::GetNeighborhood => 2,
            Self::CritiqueContext => 3,
            Self::TriggerDream => 3,
            Self::EpistemicAction => 4,
            Self::Curate => 5,
        }
    }
}

impl std::fmt::Display for SuggestedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// TESTS - REAL DATA ONLY, NO MOCKS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== JOHARI QUADRANT TESTS ==========

    #[test]
    fn test_default_is_open() {
        let quadrant = JohariQuadrant::default();
        assert_eq!(quadrant, JohariQuadrant::Open);
    }

    #[test]
    fn test_repr_values() {
        assert_eq!(JohariQuadrant::Open as u8, 0);
        assert_eq!(JohariQuadrant::Blind as u8, 1);
        assert_eq!(JohariQuadrant::Hidden as u8, 2);
        assert_eq!(JohariQuadrant::Unknown as u8, 3);
    }

    #[test]
    fn test_name() {
        assert_eq!(JohariQuadrant::Open.name(), "Open");
        assert_eq!(JohariQuadrant::Blind.name(), "Blind");
        assert_eq!(JohariQuadrant::Hidden.name(), "Hidden");
        assert_eq!(JohariQuadrant::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", JohariQuadrant::Open), "Open");
        assert_eq!(format!("{}", JohariQuadrant::Unknown), "Unknown");
    }

    #[test]
    fn test_is_well_understood() {
        assert!(JohariQuadrant::Open.is_well_understood());
        assert!(!JohariQuadrant::Blind.is_well_understood());
        assert!(!JohariQuadrant::Hidden.is_well_understood());
        assert!(!JohariQuadrant::Unknown.is_well_understood());
    }

    #[test]
    fn test_requires_exploration() {
        assert!(!JohariQuadrant::Open.requires_exploration());
        assert!(JohariQuadrant::Blind.requires_exploration());
        assert!(!JohariQuadrant::Hidden.requires_exploration());
        assert!(JohariQuadrant::Unknown.requires_exploration());
    }

    #[test]
    fn test_is_high_entropy() {
        assert!(!JohariQuadrant::Open.is_high_entropy());
        assert!(JohariQuadrant::Blind.is_high_entropy());
        assert!(!JohariQuadrant::Hidden.is_high_entropy());
        assert!(JohariQuadrant::Unknown.is_high_entropy());
    }

    #[test]
    fn test_is_high_coherence() {
        assert!(JohariQuadrant::Open.is_high_coherence());
        assert!(!JohariQuadrant::Blind.is_high_coherence());
        assert!(!JohariQuadrant::Hidden.is_high_coherence());
        assert!(JohariQuadrant::Unknown.is_high_coherence());
    }

    #[test]
    fn test_suggested_action() {
        assert_eq!(JohariQuadrant::Open.suggested_action(), SuggestedAction::DirectRecall);
        assert_eq!(JohariQuadrant::Blind.suggested_action(), SuggestedAction::CritiqueContext);
        assert_eq!(JohariQuadrant::Hidden.suggested_action(), SuggestedAction::GetNeighborhood);
        assert_eq!(JohariQuadrant::Unknown.suggested_action(), SuggestedAction::EpistemicAction);
    }

    #[test]
    fn test_confidence_level() {
        assert_eq!(JohariQuadrant::Open.confidence_level(), 0.9);
        assert_eq!(JohariQuadrant::Hidden.confidence_level(), 0.6);
        assert_eq!(JohariQuadrant::Unknown.confidence_level(), 0.4);
        assert_eq!(JohariQuadrant::Blind.confidence_level(), 0.2);
    }

    #[test]
    fn test_confidence_ordering() {
        assert!(JohariQuadrant::Open.confidence_level() > JohariQuadrant::Hidden.confidence_level());
        assert!(JohariQuadrant::Hidden.confidence_level() > JohariQuadrant::Unknown.confidence_level());
        assert!(JohariQuadrant::Unknown.confidence_level() > JohariQuadrant::Blind.confidence_level());
    }

    // ========== CLASSIFICATION TESTS ==========

    #[test]
    fn test_classify_open() {
        // Low entropy, high coherence
        assert_eq!(JohariQuadrant::classify(0.2, 0.8), JohariQuadrant::Open);
        assert_eq!(JohariQuadrant::classify(0.0, 1.0), JohariQuadrant::Open);
        assert_eq!(JohariQuadrant::classify(0.4, 0.6), JohariQuadrant::Open);
    }

    #[test]
    fn test_classify_blind() {
        // High entropy, low coherence
        assert_eq!(JohariQuadrant::classify(0.8, 0.2), JohariQuadrant::Blind);
        assert_eq!(JohariQuadrant::classify(1.0, 0.0), JohariQuadrant::Blind);
        assert_eq!(JohariQuadrant::classify(0.6, 0.4), JohariQuadrant::Blind);
    }

    #[test]
    fn test_classify_hidden() {
        // Low entropy, low coherence
        assert_eq!(JohariQuadrant::classify(0.2, 0.2), JohariQuadrant::Hidden);
        assert_eq!(JohariQuadrant::classify(0.0, 0.0), JohariQuadrant::Hidden);
        assert_eq!(JohariQuadrant::classify(0.4, 0.4), JohariQuadrant::Hidden);
    }

    #[test]
    fn test_classify_unknown() {
        // High entropy, high coherence
        assert_eq!(JohariQuadrant::classify(0.8, 0.8), JohariQuadrant::Unknown);
        assert_eq!(JohariQuadrant::classify(1.0, 1.0), JohariQuadrant::Unknown);
        assert_eq!(JohariQuadrant::classify(0.6, 0.6), JohariQuadrant::Unknown);
    }

    #[test]
    fn test_classify_boundary_cases() {
        // At exactly threshold (0.5), considered high
        assert_eq!(JohariQuadrant::classify(0.5, 0.5), JohariQuadrant::Unknown);
        assert_eq!(JohariQuadrant::classify(0.5, 0.4), JohariQuadrant::Blind);
        assert_eq!(JohariQuadrant::classify(0.4, 0.5), JohariQuadrant::Open);
    }

    #[test]
    fn test_classify_with_custom_thresholds() {
        // Higher thresholds
        assert_eq!(
            JohariQuadrant::classify_with_thresholds(0.6, 0.6, 0.7, 0.7),
            JohariQuadrant::Hidden
        );

        // Lower thresholds
        assert_eq!(
            JohariQuadrant::classify_with_thresholds(0.4, 0.4, 0.3, 0.3),
            JohariQuadrant::Unknown
        );
    }

    // ========== SUGGESTED ACTION TESTS ==========

    #[test]
    fn test_action_default_is_direct_recall() {
        let action = SuggestedAction::default();
        assert_eq!(action, SuggestedAction::DirectRecall);
    }

    #[test]
    fn test_action_repr_values() {
        assert_eq!(SuggestedAction::DirectRecall as u8, 0);
        assert_eq!(SuggestedAction::TriggerDream as u8, 1);
        assert_eq!(SuggestedAction::GetNeighborhood as u8, 2);
        assert_eq!(SuggestedAction::EpistemicAction as u8, 3);
        assert_eq!(SuggestedAction::CritiqueContext as u8, 4);
        assert_eq!(SuggestedAction::Curate as u8, 5);
    }

    #[test]
    fn test_action_name() {
        assert_eq!(SuggestedAction::DirectRecall.name(), "DirectRecall");
        assert_eq!(SuggestedAction::TriggerDream.name(), "TriggerDream");
        assert_eq!(SuggestedAction::GetNeighborhood.name(), "GetNeighborhood");
        assert_eq!(SuggestedAction::EpistemicAction.name(), "EpistemicAction");
        assert_eq!(SuggestedAction::CritiqueContext.name(), "CritiqueContext");
        assert_eq!(SuggestedAction::Curate.name(), "Curate");
    }

    #[test]
    fn test_action_is_retrieval_focused() {
        assert!(SuggestedAction::DirectRecall.is_retrieval_focused());
        assert!(SuggestedAction::GetNeighborhood.is_retrieval_focused());
        assert!(!SuggestedAction::TriggerDream.is_retrieval_focused());
        assert!(!SuggestedAction::EpistemicAction.is_retrieval_focused());
        assert!(!SuggestedAction::CritiqueContext.is_retrieval_focused());
        assert!(!SuggestedAction::Curate.is_retrieval_focused());
    }

    #[test]
    fn test_action_is_exploratory() {
        assert!(!SuggestedAction::DirectRecall.is_exploratory());
        assert!(SuggestedAction::GetNeighborhood.is_exploratory());
        assert!(SuggestedAction::EpistemicAction.is_exploratory());
        assert!(!SuggestedAction::TriggerDream.is_exploratory());
        assert!(!SuggestedAction::CritiqueContext.is_exploratory());
        assert!(!SuggestedAction::Curate.is_exploratory());
    }

    #[test]
    fn test_action_modifies_context() {
        assert!(!SuggestedAction::DirectRecall.modifies_context());
        assert!(SuggestedAction::TriggerDream.modifies_context());
        assert!(!SuggestedAction::GetNeighborhood.modifies_context());
        assert!(!SuggestedAction::EpistemicAction.modifies_context());
        assert!(SuggestedAction::CritiqueContext.modifies_context());
        assert!(SuggestedAction::Curate.modifies_context());
    }

    #[test]
    fn test_action_cost_estimate() {
        assert_eq!(SuggestedAction::DirectRecall.cost_estimate(), 1);
        assert_eq!(SuggestedAction::GetNeighborhood.cost_estimate(), 2);
        assert_eq!(SuggestedAction::CritiqueContext.cost_estimate(), 3);
        assert_eq!(SuggestedAction::TriggerDream.cost_estimate(), 3);
        assert_eq!(SuggestedAction::EpistemicAction.cost_estimate(), 4);
        assert_eq!(SuggestedAction::Curate.cost_estimate(), 5);
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_quadrant_serde_roundtrip() {
        for quadrant in [
            JohariQuadrant::Open,
            JohariQuadrant::Blind,
            JohariQuadrant::Hidden,
            JohariQuadrant::Unknown,
        ] {
            let json = serde_json::to_string(&quadrant).expect("Serialize failed");
            let recovered: JohariQuadrant = serde_json::from_str(&json).expect("Deserialize failed");
            assert_eq!(quadrant, recovered);
        }
    }

    #[test]
    fn test_action_serde_roundtrip() {
        for action in [
            SuggestedAction::DirectRecall,
            SuggestedAction::TriggerDream,
            SuggestedAction::GetNeighborhood,
            SuggestedAction::EpistemicAction,
            SuggestedAction::CritiqueContext,
            SuggestedAction::Curate,
        ] {
            let json = serde_json::to_string(&action).expect("Serialize failed");
            let recovered: SuggestedAction = serde_json::from_str(&json).expect("Deserialize failed");
            assert_eq!(action, recovered);
        }
    }

    // ========== CLONE/COPY TESTS ==========

    #[test]
    fn test_quadrant_clone_copy() {
        let quadrant = JohariQuadrant::Unknown;
        let cloned = quadrant.clone();
        let copied = quadrant;
        assert_eq!(quadrant, cloned);
        assert_eq!(quadrant, copied);
    }

    #[test]
    fn test_action_clone_copy() {
        let action = SuggestedAction::EpistemicAction;
        let cloned = action.clone();
        let copied = action;
        assert_eq!(action, cloned);
        assert_eq!(action, copied);
    }

    // ========== HASH TESTS ==========

    #[test]
    fn test_quadrant_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(JohariQuadrant::Open);
        set.insert(JohariQuadrant::Blind);
        set.insert(JohariQuadrant::Hidden);
        set.insert(JohariQuadrant::Unknown);
        assert_eq!(set.len(), 4);
    }
}
```

## Acceptance Criteria

### JohariQuadrant Signatures

- [ ] `JohariQuadrant::Open`, `JohariQuadrant::Blind`, `JohariQuadrant::Hidden`, `JohariQuadrant::Unknown` variants
- [ ] `JohariQuadrant::name(&self) -> &'static str`
- [ ] `JohariQuadrant::is_well_understood(&self) -> bool`
- [ ] `JohariQuadrant::requires_exploration(&self) -> bool`
- [ ] `JohariQuadrant::is_high_entropy(&self) -> bool`
- [ ] `JohariQuadrant::is_high_coherence(&self) -> bool`
- [ ] `JohariQuadrant::suggested_action(&self) -> SuggestedAction`
- [ ] `JohariQuadrant::confidence_level(&self) -> f32`
- [ ] `JohariQuadrant::classify(entropy: f32, coherence: f32) -> Self`
- [ ] `JohariQuadrant::classify_with_thresholds(...) -> Self`

### SuggestedAction Signatures

- [ ] All 6 variants: `DirectRecall`, `TriggerDream`, `GetNeighborhood`, `EpistemicAction`, `CritiqueContext`, `Curate`
- [ ] `SuggestedAction::name(&self) -> &'static str`
- [ ] `SuggestedAction::is_retrieval_focused(&self) -> bool`
- [ ] `SuggestedAction::is_exploratory(&self) -> bool`
- [ ] `SuggestedAction::modifies_context(&self) -> bool`
- [ ] `SuggestedAction::cost_estimate(&self) -> u8`

### Trait Implementations

- [ ] Both enums: `Default`, `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`
- [ ] Both enums: `Serialize`, `Deserialize`, `Display`
- [ ] Both enums: `#[repr(u8)]` for efficient storage

### Classification Verification

- [ ] Open: low entropy, high coherence
- [ ] Blind: high entropy, low coherence
- [ ] Hidden: low entropy, low coherence
- [ ] Unknown: high entropy, high coherence

## Verification Commands

```bash
# 1. Build the crate
cargo build -p context-graph-utl

# 2. Run johari tests
cargo test -p context-graph-utl johari -- --nocapture

# 3. Run specific tests
cargo test -p context-graph-utl test_classify_open
cargo test -p context-graph-utl test_suggested_action

# 4. Run clippy
cargo clippy -p context-graph-utl -- -D warnings

# 5. Run doc tests
cargo test -p context-graph-utl --doc
```

## Dependencies

This task has no dependencies.

## Notes for Implementer

1. Classification uses default threshold of 0.5 for both entropy and coherence
2. JohariClassifier (M05-T18) provides more sophisticated classification with config
3. Tests are co-located in `#[cfg(test)]` module per constitution
4. Confidence levels match the quadrant semantics (Open=high, Blind=low)

---

*Task Version: 1.0.0*
*Created: 2026-01-04*
*Module: 05 - UTL Integration*
