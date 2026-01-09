//! Lifecycle stage enumeration and classification.
//!
//! Defines the lifecycle stages for knowledge base evolution according to
//! the Marblestone model. Each stage has different learning dynamics
//! characterized by lambda weights for surprise vs. coherence.
//!
//! # Constitution Reference
//!
//! ```text
//! Infancy (n=0-50):   lambda_s=0.7, lambda_c=0.3, stance="capture-novelty"
//! Growth (n=50-500):  lambda_s=0.5, lambda_c=0.5, stance="balanced"
//! Maturity (n=500+):  lambda_s=0.3, lambda_c=0.7, stance="curation-coherence"
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Lifecycle stages for knowledge base evolution.
///
/// Knowledge bases progress through these stages as they accumulate
/// interactions, each stage emphasizing different aspects of learning.
///
/// # Stage Transitions
///
/// - `Infancy -> Growth`: After 50 interactions
/// - `Growth -> Maturity`: After 500 interactions
///
/// # Example
///
/// ```
/// use context_graph_utl::lifecycle::LifecycleStage;
///
/// let stage = LifecycleStage::from_interaction_count(75);
/// assert_eq!(stage, LifecycleStage::Growth);
///
/// let (min, max) = stage.interaction_range();
/// assert_eq!(min, 50);
/// assert_eq!(max, 500);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleStage {
    /// Early stage (0-50 interactions).
    ///
    /// Characterized by high novelty capture:
    /// - `lambda_s = 0.7` (surprise weight)
    /// - `lambda_c = 0.3` (coherence weight)
    /// - stance: "capture-novelty"
    Infancy,

    /// Intermediate stage (50-500 interactions).
    ///
    /// Balanced learning between novelty and consolidation:
    /// - `lambda_s = 0.5` (surprise weight)
    /// - `lambda_c = 0.5` (coherence weight)
    /// - stance: "balanced"
    Growth,

    /// Mature stage (500+ interactions).
    ///
    /// Focus on coherence and knowledge curation:
    /// - `lambda_s = 0.3` (surprise weight)
    /// - `lambda_c = 0.7` (coherence weight)
    /// - stance: "curation-coherence"
    Maturity,
}

impl LifecycleStage {
    /// Threshold for transitioning from Infancy to Growth.
    pub const INFANCY_THRESHOLD: u64 = 50;

    /// Threshold for transitioning from Growth to Maturity.
    pub const GROWTH_THRESHOLD: u64 = 500;

    /// Determine lifecycle stage from interaction count.
    ///
    /// # Arguments
    ///
    /// * `count` - Number of interactions in the knowledge base
    ///
    /// # Returns
    ///
    /// The appropriate lifecycle stage for the given interaction count.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::from_interaction_count(0), LifecycleStage::Infancy);
    /// assert_eq!(LifecycleStage::from_interaction_count(49), LifecycleStage::Infancy);
    /// assert_eq!(LifecycleStage::from_interaction_count(50), LifecycleStage::Growth);
    /// assert_eq!(LifecycleStage::from_interaction_count(499), LifecycleStage::Growth);
    /// assert_eq!(LifecycleStage::from_interaction_count(500), LifecycleStage::Maturity);
    /// assert_eq!(LifecycleStage::from_interaction_count(10000), LifecycleStage::Maturity);
    /// ```
    #[inline]
    pub fn from_interaction_count(count: u64) -> Self {
        if count < Self::INFANCY_THRESHOLD {
            LifecycleStage::Infancy
        } else if count < Self::GROWTH_THRESHOLD {
            LifecycleStage::Growth
        } else {
            LifecycleStage::Maturity
        }
    }

    /// Get the interaction count range for this stage.
    ///
    /// Returns a tuple of (min_inclusive, max_exclusive) interaction counts.
    /// For Maturity stage, max is `u64::MAX` to indicate unbounded.
    ///
    /// # Returns
    ///
    /// `(min, max)` where:
    /// - `min`: Minimum interaction count (inclusive) for this stage
    /// - `max`: Maximum interaction count (exclusive) for this stage
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// let (min, max) = LifecycleStage::Infancy.interaction_range();
    /// assert_eq!(min, 0);
    /// assert_eq!(max, 50);
    ///
    /// let (min, max) = LifecycleStage::Growth.interaction_range();
    /// assert_eq!(min, 50);
    /// assert_eq!(max, 500);
    ///
    /// let (min, max) = LifecycleStage::Maturity.interaction_range();
    /// assert_eq!(min, 500);
    /// assert_eq!(max, u64::MAX);
    /// ```
    #[inline]
    pub fn interaction_range(&self) -> (u64, u64) {
        match self {
            LifecycleStage::Infancy => (0, Self::INFANCY_THRESHOLD),
            LifecycleStage::Growth => (Self::INFANCY_THRESHOLD, Self::GROWTH_THRESHOLD),
            LifecycleStage::Maturity => (Self::GROWTH_THRESHOLD, u64::MAX),
        }
    }

    /// Get a human-readable description of this lifecycle stage.
    ///
    /// # Returns
    ///
    /// A static string describing the stage's learning focus and characteristics.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// let desc = LifecycleStage::Infancy.description();
    /// assert!(desc.contains("novelty"));
    /// ```
    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            LifecycleStage::Infancy => {
                "Infancy stage (0-50 interactions): High novelty capture with lambda_s=0.7, lambda_c=0.3. Stance: capture-novelty"
            }
            LifecycleStage::Growth => {
                "Growth stage (50-500 interactions): Balanced learning with lambda_s=0.5, lambda_c=0.5. Stance: balanced"
            }
            LifecycleStage::Maturity => {
                "Maturity stage (500+ interactions): Coherence focus with lambda_s=0.3, lambda_c=0.7. Stance: curation-coherence"
            }
        }
    }

    /// Get the stance name for this lifecycle stage.
    ///
    /// The stance describes the learning strategy employed at this stage.
    ///
    /// # Returns
    ///
    /// - `"capture-novelty"` for Infancy
    /// - `"balanced"` for Growth
    /// - `"curation-coherence"` for Maturity
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::Infancy.stance(), "capture-novelty");
    /// assert_eq!(LifecycleStage::Growth.stance(), "balanced");
    /// assert_eq!(LifecycleStage::Maturity.stance(), "curation-coherence");
    /// ```
    #[inline]
    pub fn stance(&self) -> &'static str {
        match self {
            LifecycleStage::Infancy => "capture-novelty",
            LifecycleStage::Growth => "balanced",
            LifecycleStage::Maturity => "curation-coherence",
        }
    }

    /// Get the next stage in the lifecycle progression.
    ///
    /// # Returns
    ///
    /// - `Some(Growth)` for Infancy
    /// - `Some(Maturity)` for Growth
    /// - `None` for Maturity (already at final stage)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::Infancy.next_stage(), Some(LifecycleStage::Growth));
    /// assert_eq!(LifecycleStage::Growth.next_stage(), Some(LifecycleStage::Maturity));
    /// assert_eq!(LifecycleStage::Maturity.next_stage(), None);
    /// ```
    #[inline]
    pub fn next_stage(&self) -> Option<LifecycleStage> {
        match self {
            LifecycleStage::Infancy => Some(LifecycleStage::Growth),
            LifecycleStage::Growth => Some(LifecycleStage::Maturity),
            LifecycleStage::Maturity => None,
        }
    }

    /// Get the previous stage in the lifecycle progression.
    ///
    /// # Returns
    ///
    /// - `None` for Infancy (already at first stage)
    /// - `Some(Infancy)` for Growth
    /// - `Some(Growth)` for Maturity
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::Infancy.previous_stage(), None);
    /// assert_eq!(LifecycleStage::Growth.previous_stage(), Some(LifecycleStage::Infancy));
    /// assert_eq!(LifecycleStage::Maturity.previous_stage(), Some(LifecycleStage::Growth));
    /// ```
    #[inline]
    pub fn previous_stage(&self) -> Option<LifecycleStage> {
        match self {
            LifecycleStage::Infancy => None,
            LifecycleStage::Growth => Some(LifecycleStage::Infancy),
            LifecycleStage::Maturity => Some(LifecycleStage::Growth),
        }
    }

    /// Check if a forward transition from this stage to another is valid.
    ///
    /// Forward transitions must follow the natural progression:
    /// `Infancy -> Growth -> Maturity`.
    ///
    /// # Arguments
    ///
    /// * `target` - The target lifecycle stage
    ///
    /// # Returns
    ///
    /// `true` if the transition is valid (target is the same or later stage),
    /// `false` otherwise.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Growth));
    /// assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Maturity));
    /// assert!(LifecycleStage::Growth.can_transition_to(LifecycleStage::Maturity));
    /// assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Infancy));
    /// assert!(!LifecycleStage::Growth.can_transition_to(LifecycleStage::Infancy));
    /// ```
    #[inline]
    pub fn can_transition_to(&self, target: LifecycleStage) -> bool {
        matches!(
            (self, target),
            (LifecycleStage::Infancy, _)
                | (LifecycleStage::Growth, LifecycleStage::Growth)
                | (LifecycleStage::Growth, LifecycleStage::Maturity)
                | (LifecycleStage::Maturity, LifecycleStage::Maturity)
        )
    }

    /// Get the numeric index of this stage (for ordering and comparison).
    ///
    /// # Returns
    ///
    /// - `0` for Infancy
    /// - `1` for Growth
    /// - `2` for Maturity
    #[inline]
    pub fn index(&self) -> usize {
        match self {
            LifecycleStage::Infancy => 0,
            LifecycleStage::Growth => 1,
            LifecycleStage::Maturity => 2,
        }
    }

    /// Get all lifecycle stages in order.
    ///
    /// # Returns
    ///
    /// An array containing all stages in progression order.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// let stages = LifecycleStage::all();
    /// assert_eq!(stages.len(), 3);
    /// assert_eq!(stages[0], LifecycleStage::Infancy);
    /// assert_eq!(stages[1], LifecycleStage::Growth);
    /// assert_eq!(stages[2], LifecycleStage::Maturity);
    /// ```
    #[inline]
    pub fn all() -> [LifecycleStage; 3] {
        [
            LifecycleStage::Infancy,
            LifecycleStage::Growth,
            LifecycleStage::Maturity,
        ]
    }
}

impl Default for LifecycleStage {
    /// Returns `LifecycleStage::Infancy` as the default stage.
    fn default() -> Self {
        LifecycleStage::Infancy
    }
}

impl fmt::Display for LifecycleStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleStage::Infancy => write!(f, "Infancy"),
            LifecycleStage::Growth => write!(f, "Growth"),
            LifecycleStage::Maturity => write!(f, "Maturity"),
        }
    }
}

impl PartialOrd for LifecycleStage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LifecycleStage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index().cmp(&other.index())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_interaction_count() {
        // Infancy range: 0-49
        assert_eq!(
            LifecycleStage::from_interaction_count(0),
            LifecycleStage::Infancy
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(25),
            LifecycleStage::Infancy
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(49),
            LifecycleStage::Infancy
        );

        // Growth range: 50-499
        assert_eq!(
            LifecycleStage::from_interaction_count(50),
            LifecycleStage::Growth
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(100),
            LifecycleStage::Growth
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(499),
            LifecycleStage::Growth
        );

        // Maturity range: 500+
        assert_eq!(
            LifecycleStage::from_interaction_count(500),
            LifecycleStage::Maturity
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(1000),
            LifecycleStage::Maturity
        );
        assert_eq!(
            LifecycleStage::from_interaction_count(u64::MAX),
            LifecycleStage::Maturity
        );
    }

    #[test]
    fn test_interaction_range() {
        let (min, max) = LifecycleStage::Infancy.interaction_range();
        assert_eq!(min, 0);
        assert_eq!(max, 50);

        let (min, max) = LifecycleStage::Growth.interaction_range();
        assert_eq!(min, 50);
        assert_eq!(max, 500);

        let (min, max) = LifecycleStage::Maturity.interaction_range();
        assert_eq!(min, 500);
        assert_eq!(max, u64::MAX);
    }

    #[test]
    fn test_description() {
        let desc = LifecycleStage::Infancy.description();
        assert!(desc.contains("Infancy"));
        assert!(desc.contains("0-50"));
        assert!(desc.contains("novelty"));

        let desc = LifecycleStage::Growth.description();
        assert!(desc.contains("Growth"));
        assert!(desc.contains("50-500"));
        assert!(desc.contains("balanced"));

        let desc = LifecycleStage::Maturity.description();
        assert!(desc.contains("Maturity"));
        assert!(desc.contains("500+"));
        assert!(desc.contains("coherence"));
    }

    #[test]
    fn test_stance() {
        assert_eq!(LifecycleStage::Infancy.stance(), "capture-novelty");
        assert_eq!(LifecycleStage::Growth.stance(), "balanced");
        assert_eq!(LifecycleStage::Maturity.stance(), "curation-coherence");
    }

    #[test]
    fn test_next_stage() {
        assert_eq!(
            LifecycleStage::Infancy.next_stage(),
            Some(LifecycleStage::Growth)
        );
        assert_eq!(
            LifecycleStage::Growth.next_stage(),
            Some(LifecycleStage::Maturity)
        );
        assert_eq!(LifecycleStage::Maturity.next_stage(), None);
    }

    #[test]
    fn test_previous_stage() {
        assert_eq!(LifecycleStage::Infancy.previous_stage(), None);
        assert_eq!(
            LifecycleStage::Growth.previous_stage(),
            Some(LifecycleStage::Infancy)
        );
        assert_eq!(
            LifecycleStage::Maturity.previous_stage(),
            Some(LifecycleStage::Growth)
        );
    }

    #[test]
    fn test_can_transition_to() {
        // Valid forward transitions
        assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Infancy));
        assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Growth));
        assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Maturity));
        assert!(LifecycleStage::Growth.can_transition_to(LifecycleStage::Growth));
        assert!(LifecycleStage::Growth.can_transition_to(LifecycleStage::Maturity));
        assert!(LifecycleStage::Maturity.can_transition_to(LifecycleStage::Maturity));

        // Invalid backward transitions
        assert!(!LifecycleStage::Growth.can_transition_to(LifecycleStage::Infancy));
        assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Infancy));
        assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Growth));
    }

    #[test]
    fn test_index() {
        assert_eq!(LifecycleStage::Infancy.index(), 0);
        assert_eq!(LifecycleStage::Growth.index(), 1);
        assert_eq!(LifecycleStage::Maturity.index(), 2);
    }

    #[test]
    fn test_all() {
        let stages = LifecycleStage::all();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0], LifecycleStage::Infancy);
        assert_eq!(stages[1], LifecycleStage::Growth);
        assert_eq!(stages[2], LifecycleStage::Maturity);
    }

    #[test]
    fn test_default() {
        assert_eq!(LifecycleStage::default(), LifecycleStage::Infancy);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", LifecycleStage::Infancy), "Infancy");
        assert_eq!(format!("{}", LifecycleStage::Growth), "Growth");
        assert_eq!(format!("{}", LifecycleStage::Maturity), "Maturity");
    }

    #[test]
    fn test_ordering() {
        assert!(LifecycleStage::Infancy < LifecycleStage::Growth);
        assert!(LifecycleStage::Growth < LifecycleStage::Maturity);
        assert!(LifecycleStage::Infancy < LifecycleStage::Maturity);

        let mut stages = [
            LifecycleStage::Maturity,
            LifecycleStage::Infancy,
            LifecycleStage::Growth,
        ];
        stages.sort();
        assert_eq!(stages[0], LifecycleStage::Infancy);
        assert_eq!(stages[1], LifecycleStage::Growth);
        assert_eq!(stages[2], LifecycleStage::Maturity);
    }

    #[test]
    fn test_serialization() {
        let stage = LifecycleStage::Growth;
        let json = serde_json::to_string(&stage).unwrap();
        assert_eq!(json, "\"growth\"");

        let deserialized: LifecycleStage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, LifecycleStage::Growth);
    }

    #[test]
    fn test_deserialization_all_stages() {
        let infancy: LifecycleStage = serde_json::from_str("\"infancy\"").unwrap();
        assert_eq!(infancy, LifecycleStage::Infancy);

        let growth: LifecycleStage = serde_json::from_str("\"growth\"").unwrap();
        assert_eq!(growth, LifecycleStage::Growth);

        let maturity: LifecycleStage = serde_json::from_str("\"maturity\"").unwrap();
        assert_eq!(maturity, LifecycleStage::Maturity);
    }

    #[test]
    fn test_thresholds() {
        assert_eq!(LifecycleStage::INFANCY_THRESHOLD, 50);
        assert_eq!(LifecycleStage::GROWTH_THRESHOLD, 500);
    }

    #[test]
    fn test_clone_and_copy() {
        let stage = LifecycleStage::Growth;
        let cloned = stage;
        let copied = stage;

        assert_eq!(stage, cloned);
        assert_eq!(stage, copied);
    }

    #[test]
    fn test_debug() {
        let stage = LifecycleStage::Maturity;
        let debug = format!("{:?}", stage);
        assert_eq!(debug, "Maturity");
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(LifecycleStage::Infancy);
        set.insert(LifecycleStage::Growth);
        set.insert(LifecycleStage::Maturity);

        assert_eq!(set.len(), 3);
        assert!(set.contains(&LifecycleStage::Infancy));
        assert!(set.contains(&LifecycleStage::Growth));
        assert!(set.contains(&LifecycleStage::Maturity));
    }
}
