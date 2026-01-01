//! Cognitive Pulse types for meta-cognitive state tracking.
//!
//! Every MCP tool response includes a Cognitive Pulse header to convey
//! the current system state and suggest next actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::nervous::LayerId;
use super::utl::{EmotionalState, UtlMetrics};

/// Cognitive Pulse header included in all tool responses.
///
/// Provides meta-cognitive state information to help agents
/// understand system state and decide on next actions.
///
/// The 7-field structure captures:
/// - Core metrics: entropy, coherence, coherence_delta
/// - Emotional context: emotional_weight (from EmotionalState)
/// - Action guidance: suggested_action
/// - Source tracking: source_layer
/// - Temporal context: timestamp
///
/// # Example Response
///
/// ```json
/// {
///   "entropy": 0.45,
///   "coherence": 0.72,
///   "coherence_delta": 0.05,
///   "emotional_weight": 1.2,
///   "suggested_action": "continue",
///   "source_layer": "learning",
///   "timestamp": "2025-01-01T12:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitivePulse {
    /// Current entropy level [0.0, 1.0]
    /// Higher values indicate more uncertainty/novelty
    pub entropy: f32,

    /// Current coherence level [0.0, 1.0]
    /// Higher values indicate better integration/understanding
    pub coherence: f32,

    /// Change in coherence from previous measurement [−1.0, 1.0]
    /// Positive values indicate improving understanding
    pub coherence_delta: f32,

    /// Emotional weight modifier from EmotionalState [0.0, 2.0]
    /// Derived from EmotionalState::weight_modifier()
    pub emotional_weight: f32,

    /// Suggested action based on current state
    pub suggested_action: SuggestedAction,

    /// Source layer that generated this pulse (None if computed globally)
    pub source_layer: Option<LayerId>,

    /// UTC timestamp when this pulse was created
    pub timestamp: DateTime<Utc>,
}

impl Default for CognitivePulse {
    fn default() -> Self {
        Self {
            entropy: 0.5,
            coherence: 0.5,
            coherence_delta: 0.0,
            emotional_weight: 1.0,
            suggested_action: SuggestedAction::Continue,
            source_layer: None,
            timestamp: Utc::now(),
        }
    }
}

impl CognitivePulse {
    /// Create a new pulse with explicit values for all 7 fields.
    ///
    /// Values are clamped to valid ranges:
    /// - entropy: [0.0, 1.0]
    /// - coherence: [0.0, 1.0]
    /// - coherence_delta: [-1.0, 1.0]
    /// - emotional_weight: [0.0, 2.0]
    pub fn new(
        entropy: f32,
        coherence: f32,
        coherence_delta: f32,
        emotional_weight: f32,
        suggested_action: SuggestedAction,
        source_layer: Option<LayerId>,
    ) -> Self {
        let entropy = entropy.clamp(0.0, 1.0);
        let coherence = coherence.clamp(0.0, 1.0);
        let coherence_delta = coherence_delta.clamp(-1.0, 1.0);
        let emotional_weight = emotional_weight.clamp(0.0, 2.0);

        Self {
            entropy,
            coherence,
            coherence_delta,
            emotional_weight,
            suggested_action,
            source_layer,
            timestamp: Utc::now(),
        }
    }

    /// Create a new pulse computed from UTL metrics and source layer.
    ///
    /// Derives all 7 fields from the provided metrics:
    /// - entropy: from metrics.entropy
    /// - coherence: from metrics.coherence
    /// - coherence_delta: from metrics.coherence_change
    /// - emotional_weight: from metrics.emotional_weight
    /// - suggested_action: computed from entropy/coherence
    /// - source_layer: from the provided layer parameter
    /// - timestamp: current UTC time
    pub fn computed(metrics: &UtlMetrics, source_layer: Option<LayerId>) -> Self {
        let entropy = metrics.entropy.clamp(0.0, 1.0);
        let coherence = metrics.coherence.clamp(0.0, 1.0);
        let coherence_delta = metrics.coherence_change.clamp(-1.0, 1.0);
        let emotional_weight = metrics.emotional_weight.clamp(0.0, 2.0);
        let suggested_action = Self::compute_action(entropy, coherence);

        Self {
            entropy,
            coherence,
            coherence_delta,
            emotional_weight,
            suggested_action,
            source_layer,
            timestamp: Utc::now(),
        }
    }

    /// Create a simple pulse from entropy and coherence values.
    ///
    /// Uses default values for other fields:
    /// - coherence_delta: 0.0
    /// - emotional_weight: 1.0
    /// - source_layer: None
    /// - suggested_action: computed from entropy/coherence
    pub fn from_values(entropy: f32, coherence: f32) -> Self {
        let entropy = entropy.clamp(0.0, 1.0);
        let coherence = coherence.clamp(0.0, 1.0);
        let suggested_action = Self::compute_action(entropy, coherence);

        Self {
            entropy,
            coherence,
            coherence_delta: 0.0,
            emotional_weight: 1.0,
            suggested_action,
            source_layer: None,
            timestamp: Utc::now(),
        }
    }

    /// Compute the suggested action based on entropy and coherence.
    fn compute_action(entropy: f32, coherence: f32) -> SuggestedAction {
        match (entropy, coherence) {
            // High entropy, low coherence - needs stabilization
            (e, c) if e > 0.7 && c < 0.4 => SuggestedAction::Stabilize,
            // High entropy, high coherence - exploration frontier
            (e, c) if e > 0.6 && c > 0.5 => SuggestedAction::Explore,
            // Low entropy, high coherence - well understood, ready
            (e, c) if e < 0.4 && c > 0.6 => SuggestedAction::Ready,
            // Low coherence - needs consolidation
            (_, c) if c < 0.4 => SuggestedAction::Consolidate,
            // High entropy - consider pruning
            (e, _) if e > 0.8 => SuggestedAction::Prune,
            // Review needed
            (e, c) if e > 0.5 && c < 0.5 => SuggestedAction::Review,
            // Default: continue
            _ => SuggestedAction::Continue,
        }
    }

    /// Returns true if the system is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        self.entropy < 0.8 && self.coherence > 0.3
    }

    /// Updates entropy and coherence by applying deltas.
    ///
    /// Recomputes suggested_action based on new values.
    /// All values are clamped to valid ranges.
    ///
    /// # Arguments
    /// * `delta_entropy` - Change to apply to entropy
    /// * `delta_coherence` - Change to apply to coherence
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::types::CognitivePulse;
    ///
    /// let mut pulse = CognitivePulse::default();
    /// assert_eq!(pulse.entropy, 0.5);
    /// assert_eq!(pulse.coherence, 0.5);
    ///
    /// pulse.update(0.2, -0.1);
    /// assert_eq!(pulse.entropy, 0.7);
    /// assert_eq!(pulse.coherence, 0.4);
    /// ```
    pub fn update(&mut self, delta_entropy: f32, delta_coherence: f32) {
        // Store old coherence for delta calculation
        let old_coherence = self.coherence;

        // Apply deltas with clamping
        self.entropy = (self.entropy + delta_entropy).clamp(0.0, 1.0);
        self.coherence = (self.coherence + delta_coherence).clamp(0.0, 1.0);

        // Update coherence_delta to reflect this change
        self.coherence_delta = (self.coherence - old_coherence).clamp(-1.0, 1.0);

        // Recompute suggested action
        self.suggested_action = Self::compute_action(self.entropy, self.coherence);

        // Update timestamp to now
        self.timestamp = Utc::now();
    }

    /// Linearly interpolates between two pulses.
    ///
    /// Creates a new pulse that is a blend of `self` and `other`.
    /// The blend factor `t` determines the weight:
    /// - t = 0.0 → result equals self
    /// - t = 1.0 → result equals other
    /// - t = 0.5 → result is midpoint
    ///
    /// # Arguments
    /// * `other` - The other pulse to blend with
    /// * `t` - Blend factor clamped to [0.0, 1.0]
    ///
    /// # Returns
    /// A new CognitivePulse with interpolated values.
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::types::CognitivePulse;
    ///
    /// let pulse1 = CognitivePulse::from_values(0.2, 0.8);
    /// let pulse2 = CognitivePulse::from_values(0.8, 0.2);
    ///
    /// let blended = pulse1.blend(&pulse2, 0.5);
    /// assert_eq!(blended.entropy, 0.5);  // (0.2 + 0.8) / 2
    /// assert_eq!(blended.coherence, 0.5); // (0.8 + 0.2) / 2
    /// ```
    pub fn blend(&self, other: &CognitivePulse, t: f32) -> CognitivePulse {
        let t = t.clamp(0.0, 1.0);

        // Linear interpolation helper
        let lerp = |a: f32, b: f32| a + t * (b - a);

        // Interpolate numeric fields
        let entropy = lerp(self.entropy, other.entropy);
        let coherence = lerp(self.coherence, other.coherence);
        let coherence_delta = lerp(self.coherence_delta, other.coherence_delta);
        let emotional_weight = lerp(self.emotional_weight, other.emotional_weight);

        // For non-interpolatable fields, use threshold-based selection
        // t < 0.5 → use self, t >= 0.5 → use other
        let source_layer = if t < 0.5 {
            self.source_layer
        } else {
            other.source_layer
        };

        // Compute new action from blended values
        let suggested_action = Self::compute_action(entropy, coherence);

        CognitivePulse {
            entropy,
            coherence,
            coherence_delta,
            emotional_weight,
            suggested_action,
            source_layer,
            timestamp: Utc::now(),
        }
    }

    /// Create a pulse with a specific emotional state.
    ///
    /// Derives emotional_weight from the provided EmotionalState.
    pub fn with_emotion(
        entropy: f32,
        coherence: f32,
        emotional_state: EmotionalState,
        source_layer: Option<LayerId>,
    ) -> Self {
        let entropy = entropy.clamp(0.0, 1.0);
        let coherence = coherence.clamp(0.0, 1.0);
        let emotional_weight = emotional_state.weight_modifier();
        let suggested_action = Self::compute_action(entropy, coherence);

        Self {
            entropy,
            coherence,
            coherence_delta: 0.0,
            emotional_weight,
            suggested_action,
            source_layer,
            timestamp: Utc::now(),
        }
    }
}

/// Action suggestions based on cognitive state.
///
/// These suggest what the agent should consider doing next
/// based on the current entropy/coherence balance.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    /// System ready for new input - low entropy, high coherence.
    Ready,
    /// Continue current activity - balanced state (DEFAULT).
    #[default]
    Continue,
    /// Explore new knowledge - use epistemic_action or trigger_dream(rem).
    Explore,
    /// Consolidate knowledge - use trigger_dream(nrem) or merge_concepts.
    Consolidate,
    /// Prune redundant information - review curation_tasks.
    Prune,
    /// Stabilize context - use trigger_dream or critique_context.
    Stabilize,
    /// Review context - use critique_context or reflect_on_memory.
    Review,
}

impl SuggestedAction {
    /// Returns a human-readable description with MCP tool guidance.
    ///
    /// Each description includes actionable guidance for which MCP tools
    /// to use based on the current cognitive state.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Ready => "System ready for new input - low entropy, high coherence",
            Self::Continue => "Continue current activity - balanced state",
            Self::Explore => "Explore new knowledge - use epistemic_action or trigger_dream(rem)",
            Self::Consolidate => {
                "Consolidate knowledge - use trigger_dream(nrem) or merge_concepts"
            }
            Self::Prune => "Prune redundant information - review curation_tasks",
            Self::Stabilize => "Stabilize context - use trigger_dream or critique_context",
            Self::Review => "Review context - use critique_context or reflect_on_memory",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pulse_default() {
        let pulse = CognitivePulse::default();
        assert_eq!(pulse.entropy, 0.5);
        assert_eq!(pulse.coherence, 0.5);
        assert_eq!(pulse.coherence_delta, 0.0);
        assert_eq!(pulse.emotional_weight, 1.0);
        assert_eq!(pulse.suggested_action, SuggestedAction::Continue);
        assert!(pulse.source_layer.is_none());
        // Timestamp should be recent (within last second)
        let now = Utc::now();
        let diff = now.signed_duration_since(pulse.timestamp);
        assert!(diff.num_seconds().abs() < 2);
    }

    #[test]
    fn test_pulse_new_with_all_fields() {
        let pulse = CognitivePulse::new(
            0.5,
            0.7,
            0.1,
            1.2,
            SuggestedAction::Explore,
            Some(LayerId::Learning),
        );
        assert_eq!(pulse.entropy, 0.5);
        assert_eq!(pulse.coherence, 0.7);
        assert_eq!(pulse.coherence_delta, 0.1);
        assert_eq!(pulse.emotional_weight, 1.2);
        assert_eq!(pulse.suggested_action, SuggestedAction::Explore);
        assert_eq!(pulse.source_layer, Some(LayerId::Learning));
    }

    #[test]
    fn test_pulse_computed_from_metrics() {
        let metrics = UtlMetrics {
            entropy: 0.3,
            coherence: 0.8,
            learning_score: 0.5,
            surprise: 0.4,
            coherence_change: 0.15,
            emotional_weight: 1.3,
            alignment: 0.9,
        };

        let pulse = CognitivePulse::computed(&metrics, Some(LayerId::Coherence));

        assert_eq!(pulse.entropy, 0.3);
        assert_eq!(pulse.coherence, 0.8);
        assert_eq!(pulse.coherence_delta, 0.15);
        assert_eq!(pulse.emotional_weight, 1.3);
        assert_eq!(pulse.suggested_action, SuggestedAction::Ready); // low entropy, high coherence
        assert_eq!(pulse.source_layer, Some(LayerId::Coherence));
    }

    #[test]
    fn test_pulse_from_values() {
        let pulse = CognitivePulse::from_values(0.9, 0.2);
        assert_eq!(pulse.entropy, 0.9);
        assert_eq!(pulse.coherence, 0.2);
        assert_eq!(pulse.coherence_delta, 0.0);
        assert_eq!(pulse.emotional_weight, 1.0);
        assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
        assert!(pulse.source_layer.is_none());
    }

    #[test]
    fn test_pulse_with_emotion() {
        let pulse = CognitivePulse::with_emotion(
            0.5,
            0.6,
            EmotionalState::Focused,
            Some(LayerId::Memory),
        );
        assert_eq!(pulse.entropy, 0.5);
        assert_eq!(pulse.coherence, 0.6);
        assert_eq!(pulse.coherence_delta, 0.0);
        assert_eq!(pulse.emotional_weight, 1.3); // Focused weight
        assert_eq!(pulse.source_layer, Some(LayerId::Memory));
    }

    #[test]
    fn test_pulse_computed_stabilize() {
        let pulse = CognitivePulse::from_values(0.9, 0.2);
        assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
    }

    #[test]
    fn test_pulse_computed_ready() {
        let pulse = CognitivePulse::from_values(0.3, 0.8);
        assert_eq!(pulse.suggested_action, SuggestedAction::Ready);
    }

    #[test]
    fn test_is_healthy() {
        let healthy = CognitivePulse::from_values(0.5, 0.6);
        assert!(healthy.is_healthy());

        let unhealthy = CognitivePulse::from_values(0.9, 0.2);
        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn test_pulse_clamps_values() {
        let pulse = CognitivePulse::new(
            1.5,   // should clamp to 1.0
            -0.5,  // should clamp to 0.0
            2.0,   // should clamp to 1.0
            3.0,   // should clamp to 2.0
            SuggestedAction::Continue,
            None,
        );
        assert_eq!(pulse.entropy, 1.0);
        assert_eq!(pulse.coherence, 0.0);
        assert_eq!(pulse.coherence_delta, 1.0);
        assert_eq!(pulse.emotional_weight, 2.0);
    }

    #[test]
    fn test_pulse_clamps_negative_coherence_delta() {
        let pulse = CognitivePulse::new(
            0.5,
            0.5,
            -2.0, // should clamp to -1.0
            1.0,
            SuggestedAction::Continue,
            None,
        );
        assert_eq!(pulse.coherence_delta, -1.0);
    }

    #[test]
    fn test_pulse_timestamp_is_current() {
        let before = Utc::now();
        let pulse = CognitivePulse::default();
        let after = Utc::now();

        assert!(pulse.timestamp >= before);
        assert!(pulse.timestamp <= after);
    }

    #[test]
    fn test_pulse_serde_roundtrip() {
        let pulse = CognitivePulse::new(
            0.5,
            0.7,
            0.1,
            1.2,
            SuggestedAction::Explore,
            Some(LayerId::Learning),
        );

        let json = serde_json::to_string(&pulse).unwrap();
        let parsed: CognitivePulse = serde_json::from_str(&json).unwrap();

        assert_eq!(pulse.entropy, parsed.entropy);
        assert_eq!(pulse.coherence, parsed.coherence);
        assert_eq!(pulse.coherence_delta, parsed.coherence_delta);
        assert_eq!(pulse.emotional_weight, parsed.emotional_weight);
        assert_eq!(pulse.suggested_action, parsed.suggested_action);
        assert_eq!(pulse.source_layer, parsed.source_layer);
        assert_eq!(pulse.timestamp, parsed.timestamp);
    }

    #[test]
    fn test_pulse_all_seven_fields_present() {
        let pulse = CognitivePulse::default();

        // Verify all 7 fields exist and have valid values
        let _entropy: f32 = pulse.entropy;
        let _coherence: f32 = pulse.coherence;
        let _coherence_delta: f32 = pulse.coherence_delta;
        let _emotional_weight: f32 = pulse.emotional_weight;
        let _suggested_action: SuggestedAction = pulse.suggested_action;
        let _source_layer: Option<LayerId> = pulse.source_layer;
        let _timestamp: DateTime<Utc> = pulse.timestamp;

        // All fields are valid
        assert!(pulse.entropy >= 0.0 && pulse.entropy <= 1.0);
        assert!(pulse.coherence >= 0.0 && pulse.coherence <= 1.0);
        assert!(pulse.coherence_delta >= -1.0 && pulse.coherence_delta <= 1.0);
        assert!(pulse.emotional_weight >= 0.0 && pulse.emotional_weight <= 2.0);
    }

    #[test]
    fn test_computed_derives_all_fields_from_metrics() {
        let metrics = UtlMetrics {
            entropy: 0.45,
            coherence: 0.75,
            learning_score: 0.6,
            surprise: 0.5,
            coherence_change: -0.1,
            emotional_weight: 0.8,
            alignment: 0.95,
        };

        let pulse = CognitivePulse::computed(&metrics, Some(LayerId::Sensing));

        // Verify derived values
        assert_eq!(pulse.entropy, metrics.entropy);
        assert_eq!(pulse.coherence, metrics.coherence);
        assert_eq!(pulse.coherence_delta, metrics.coherence_change);
        assert_eq!(pulse.emotional_weight, metrics.emotional_weight);
        assert_eq!(pulse.source_layer, Some(LayerId::Sensing));
    }

    // =======================================================================
    // SuggestedAction Tests (TASK-M02-020)
    // =======================================================================

    #[test]
    fn test_suggested_action_default_is_continue() {
        let action = SuggestedAction::default();
        assert_eq!(action, SuggestedAction::Continue);
    }

    #[test]
    fn test_suggested_action_serde_roundtrip() {
        let actions = [
            SuggestedAction::Ready,
            SuggestedAction::Continue,
            SuggestedAction::Explore,
            SuggestedAction::Consolidate,
            SuggestedAction::Prune,
            SuggestedAction::Stabilize,
            SuggestedAction::Review,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: SuggestedAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, parsed);
        }
    }

    #[test]
    fn test_suggested_action_serde_snake_case() {
        // Verify snake_case serialization
        let json = serde_json::to_string(&SuggestedAction::Ready).unwrap();
        assert_eq!(json, "\"ready\"");

        let json = serde_json::to_string(&SuggestedAction::Continue).unwrap();
        assert_eq!(json, "\"continue\"");
    }

    #[test]
    fn test_suggested_action_descriptions_not_empty() {
        let actions = [
            SuggestedAction::Ready,
            SuggestedAction::Continue,
            SuggestedAction::Explore,
            SuggestedAction::Consolidate,
            SuggestedAction::Prune,
            SuggestedAction::Stabilize,
            SuggestedAction::Review,
        ];
        for action in actions {
            let desc = action.description();
            assert!(!desc.is_empty(), "{:?} has empty description", action);
            assert!(
                desc.len() > 20,
                "{:?} description too short: {}",
                action,
                desc
            );
        }
    }

    #[test]
    fn test_suggested_action_descriptions_unique() {
        use std::collections::HashSet;
        let actions = [
            SuggestedAction::Ready,
            SuggestedAction::Continue,
            SuggestedAction::Explore,
            SuggestedAction::Consolidate,
            SuggestedAction::Prune,
            SuggestedAction::Stabilize,
            SuggestedAction::Review,
        ];
        let descriptions: HashSet<_> = actions.iter().map(|a| a.description()).collect();
        assert_eq!(
            descriptions.len(),
            actions.len(),
            "Descriptions must be unique"
        );
    }

    #[test]
    fn test_suggested_action_copy_semantics() {
        let action = SuggestedAction::Explore;
        let copied = action; // Copy, not move
        assert_eq!(action, copied);
        assert_eq!(action.description(), copied.description());
    }

    #[test]
    fn test_suggested_action_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SuggestedAction::Ready);
        set.insert(SuggestedAction::Continue);
        set.insert(SuggestedAction::Ready); // duplicate
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_suggested_action_invalid_serde_rejected() {
        // Verify invalid variant is correctly rejected
        let json = "\"unknown_action\"";
        let result: Result<SuggestedAction, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Invalid variant should be rejected");
    }

    #[test]
    fn test_suggested_action_descriptions_contain_mcp_tools() {
        // Verify key actions have MCP tool guidance
        assert!(
            SuggestedAction::Explore
                .description()
                .contains("epistemic_action")
        );
        assert!(SuggestedAction::Explore.description().contains("trigger_dream"));
        assert!(
            SuggestedAction::Consolidate
                .description()
                .contains("trigger_dream")
        );
        assert!(
            SuggestedAction::Consolidate
                .description()
                .contains("merge_concepts")
        );
        assert!(
            SuggestedAction::Stabilize
                .description()
                .contains("critique_context")
        );
        assert!(SuggestedAction::Review.description().contains("reflect_on_memory"));
    }

    // =======================================================================
    // CognitivePulse::update() Tests (TASK-M02-022)
    // =======================================================================

    #[test]
    fn test_update_modifies_entropy_and_coherence() {
        let mut pulse = CognitivePulse::default();
        assert_eq!(pulse.entropy, 0.5);
        assert_eq!(pulse.coherence, 0.5);

        pulse.update(0.2, -0.1);

        assert_eq!(pulse.entropy, 0.7);
        assert_eq!(pulse.coherence, 0.4);
    }

    #[test]
    fn test_update_clamps_values() {
        let mut pulse = CognitivePulse::default();

        // Try to exceed bounds
        pulse.update(1.0, 1.0);
        assert_eq!(pulse.entropy, 1.0);
        assert_eq!(pulse.coherence, 1.0);

        // Try to go below zero
        pulse.update(-2.0, -2.0);
        assert_eq!(pulse.entropy, 0.0);
        assert_eq!(pulse.coherence, 0.0);
    }

    #[test]
    fn test_update_recomputes_action() {
        let mut pulse = CognitivePulse::from_values(0.5, 0.5);
        assert_eq!(pulse.suggested_action, SuggestedAction::Continue);

        // Push to high entropy, low coherence → Stabilize
        pulse.update(0.3, -0.2);
        assert_eq!(pulse.entropy, 0.8);
        assert_eq!(pulse.coherence, 0.3);
        assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
    }

    #[test]
    fn test_update_updates_coherence_delta() {
        let mut pulse = CognitivePulse::default();
        assert_eq!(pulse.coherence_delta, 0.0);

        pulse.update(0.0, 0.2);

        // coherence went from 0.5 to 0.7, so delta = 0.2
        assert!((pulse.coherence_delta - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_update_updates_timestamp() {
        let mut pulse = CognitivePulse::default();
        let original_timestamp = pulse.timestamp;

        std::thread::sleep(std::time::Duration::from_millis(10));
        pulse.update(0.1, 0.1);

        assert!(pulse.timestamp > original_timestamp);
    }

    // =======================================================================
    // CognitivePulse::blend() Tests (TASK-M02-022)
    // =======================================================================

    #[test]
    fn test_blend_at_zero_equals_self() {
        let pulse1 = CognitivePulse::from_values(0.2, 0.8);
        let pulse2 = CognitivePulse::from_values(0.8, 0.2);

        let blended = pulse1.blend(&pulse2, 0.0);

        assert_eq!(blended.entropy, 0.2);
        assert_eq!(blended.coherence, 0.8);
    }

    #[test]
    fn test_blend_at_one_equals_other() {
        let pulse1 = CognitivePulse::from_values(0.2, 0.8);
        let pulse2 = CognitivePulse::from_values(0.8, 0.2);

        let blended = pulse1.blend(&pulse2, 1.0);

        // Use approximate comparison for floating-point values
        assert!((blended.entropy - 0.8).abs() < 0.0001);
        assert!((blended.coherence - 0.2).abs() < 0.0001);
    }

    #[test]
    fn test_blend_at_midpoint() {
        let pulse1 = CognitivePulse::from_values(0.2, 0.8);
        let pulse2 = CognitivePulse::from_values(0.8, 0.2);

        let blended = pulse1.blend(&pulse2, 0.5);

        assert_eq!(blended.entropy, 0.5);
        assert_eq!(blended.coherence, 0.5);
    }

    #[test]
    fn test_blend_clamps_t() {
        let pulse1 = CognitivePulse::from_values(0.2, 0.8);
        let pulse2 = CognitivePulse::from_values(0.8, 0.2);

        // t > 1.0 should clamp to 1.0
        let blended = pulse1.blend(&pulse2, 2.0);
        assert_eq!(blended.entropy, 0.8);

        // t < 0.0 should clamp to 0.0
        let blended = pulse1.blend(&pulse2, -1.0);
        assert_eq!(blended.entropy, 0.2);
    }

    #[test]
    fn test_blend_interpolates_all_numeric_fields() {
        let pulse1 = CognitivePulse::new(
            0.2,
            0.8,
            0.1,
            1.0,
            SuggestedAction::Ready,
            Some(LayerId::Sensing),
        );
        let pulse2 = CognitivePulse::new(
            0.8,
            0.2,
            -0.1,
            1.4,
            SuggestedAction::Explore,
            Some(LayerId::Coherence),
        );

        let blended = pulse1.blend(&pulse2, 0.5);

        assert_eq!(blended.entropy, 0.5);
        assert_eq!(blended.coherence, 0.5);
        assert_eq!(blended.coherence_delta, 0.0); // (0.1 + -0.1) / 2
        assert_eq!(blended.emotional_weight, 1.2); // (1.0 + 1.4) / 2
    }

    #[test]
    fn test_blend_source_layer_threshold() {
        let pulse1 = CognitivePulse::new(
            0.5,
            0.5,
            0.0,
            1.0,
            SuggestedAction::Continue,
            Some(LayerId::Sensing),
        );
        let pulse2 = CognitivePulse::new(
            0.5,
            0.5,
            0.0,
            1.0,
            SuggestedAction::Continue,
            Some(LayerId::Coherence),
        );

        // t < 0.5 → use self's source_layer
        let blended = pulse1.blend(&pulse2, 0.49);
        assert_eq!(blended.source_layer, Some(LayerId::Sensing));

        // t >= 0.5 → use other's source_layer
        let blended = pulse1.blend(&pulse2, 0.5);
        assert_eq!(blended.source_layer, Some(LayerId::Coherence));
    }

    #[test]
    fn test_blend_recomputes_action() {
        let pulse1 = CognitivePulse::from_values(0.2, 0.9); // Ready
        let pulse2 = CognitivePulse::from_values(0.9, 0.2); // Stabilize

        // Midpoint should compute a new action
        let blended = pulse1.blend(&pulse2, 0.5);

        // entropy=0.55, coherence=0.55 → should compute appropriate action
        // Not testing specific action, just that it computes something
        assert!(matches!(
            blended.suggested_action,
            SuggestedAction::Continue | SuggestedAction::Explore | SuggestedAction::Review
        ));
    }

    #[test]
    fn test_blend_creates_new_timestamp() {
        let pulse1 = CognitivePulse::default();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let pulse2 = CognitivePulse::default();
        std::thread::sleep(std::time::Duration::from_millis(10));

        let blended = pulse1.blend(&pulse2, 0.5);

        // Blended timestamp should be newest
        assert!(blended.timestamp >= pulse2.timestamp);
    }

    // =======================================================================
    // Edge Case Tests (TASK-M02-022)
    // =======================================================================

    #[test]
    fn edge_case_update_extreme_deltas() {
        let mut pulse = CognitivePulse::from_values(0.5, 0.5);

        println!(
            "BEFORE: entropy={}, coherence={}",
            pulse.entropy, pulse.coherence
        );

        pulse.update(100.0, -100.0);

        println!(
            "AFTER: entropy={}, coherence={}",
            pulse.entropy, pulse.coherence
        );

        assert_eq!(
            pulse.entropy, 1.0,
            "Extreme positive delta should clamp to 1.0"
        );
        assert_eq!(
            pulse.coherence, 0.0,
            "Extreme negative delta should clamp to 0.0"
        );
    }

    #[test]
    fn edge_case_blend_identical_pulses() {
        let pulse = CognitivePulse::from_values(0.6, 0.7);

        println!(
            "ORIGINAL: entropy={}, coherence={}",
            pulse.entropy, pulse.coherence
        );

        let blended = pulse.blend(&pulse, 0.5);

        println!(
            "BLENDED: entropy={}, coherence={}",
            blended.entropy, blended.coherence
        );

        assert_eq!(blended.entropy, pulse.entropy);
        assert_eq!(blended.coherence, pulse.coherence);
    }

    #[test]
    fn edge_case_update_action_transition() {
        let mut pulse = CognitivePulse::from_values(0.35, 0.75);

        println!(
            "STATE 1: entropy={}, coherence={}, action={:?}",
            pulse.entropy, pulse.coherence, pulse.suggested_action
        );

        // Low entropy + high coherence = Ready
        assert_eq!(pulse.suggested_action, SuggestedAction::Ready);

        pulse.update(0.5, -0.5);

        println!(
            "STATE 2: entropy={}, coherence={}, action={:?}",
            pulse.entropy, pulse.coherence, pulse.suggested_action
        );

        // High entropy + low coherence = Stabilize
        assert_eq!(pulse.suggested_action, SuggestedAction::Stabilize);
    }
}
