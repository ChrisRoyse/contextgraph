//! Purpose evolution tracking for teleological fingerprints.
//!
//! Tracks how a memory's alignment with North Star goals changes over time.
//! From constitution.yaml: delta_A < -0.15 predicts failure 72 hours ahead.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::johari::JohariFingerprint;
use super::purpose::PurposeVector;

/// Events that trigger a purpose evolution snapshot.
///
/// Each variant captures context about why the alignment was recalculated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvolutionTrigger {
    /// Memory was just created
    Created,

    /// Memory was accessed in a query
    Accessed {
        /// Context/query that accessed this memory
        query_context: String,
    },

    /// The North Star goal itself changed
    GoalChanged {
        /// Previous goal UUID
        old_goal: Uuid,
        /// New goal UUID
        new_goal: Uuid,
    },

    /// Periodic recalibration (scheduled maintenance)
    Recalibration,

    /// Misalignment warning was detected
    MisalignmentDetected {
        /// The alignment delta that triggered warning (< -0.15)
        delta_a: f32,
    },
}

impl EvolutionTrigger {
    /// Check if this trigger indicates a potential problem.
    pub fn is_warning(&self) -> bool {
        matches!(self, Self::MisalignmentDetected { .. })
    }

    /// Get a human-readable description of the trigger.
    pub fn description(&self) -> String {
        match self {
            Self::Created => "Memory created".to_string(),
            Self::Accessed { query_context } => format!("Accessed via: {}", query_context),
            Self::GoalChanged { old_goal, new_goal } => {
                format!("Goal changed: {} -> {}", old_goal, new_goal)
            }
            Self::Recalibration => "Scheduled recalibration".to_string(),
            Self::MisalignmentDetected { delta_a } => {
                format!("Misalignment detected: delta_A = {:.4}", delta_a)
            }
        }
    }
}

impl std::fmt::Display for EvolutionTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// A snapshot of purpose state at a point in time.
///
/// Used to track how a memory's alignment evolves over its lifetime.
/// The system maintains up to MAX_EVOLUTION_SNAPSHOTS (100) snapshots;
/// older snapshots are archived to TimescaleDB in production.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurposeSnapshot {
    /// When this snapshot was taken (UTC)
    pub timestamp: DateTime<Utc>,

    /// The purpose vector at this point in time
    pub purpose: PurposeVector,

    /// The Johari classification at this point in time
    pub johari: JohariFingerprint,

    /// What triggered this snapshot
    pub trigger: EvolutionTrigger,
}

impl PurposeSnapshot {
    /// Create a new snapshot at the current time.
    pub fn new(
        purpose: PurposeVector,
        johari: JohariFingerprint,
        trigger: EvolutionTrigger,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            purpose,
            johari,
            trigger,
        }
    }

    /// Create a snapshot with a specific timestamp (for testing/import).
    pub fn with_timestamp(
        timestamp: DateTime<Utc>,
        purpose: PurposeVector,
        johari: JohariFingerprint,
        trigger: EvolutionTrigger,
    ) -> Self {
        Self {
            timestamp,
            purpose,
            johari,
            trigger,
        }
    }

    /// Get the aggregate alignment at this snapshot.
    pub fn aggregate_alignment(&self) -> f32 {
        self.purpose.aggregate_alignment()
    }
}

#[cfg(test)]
mod tests {
    use super::super::purpose::NUM_EMBEDDERS;
    use super::*;
    use chrono::Duration;

    fn make_test_purpose() -> PurposeVector {
        PurposeVector::new([0.75; NUM_EMBEDDERS])
    }

    fn make_test_johari() -> JohariFingerprint {
        JohariFingerprint::default()
    }

    #[test]
    fn test_evolution_trigger_created() {
        let trigger = EvolutionTrigger::Created;
        assert!(!trigger.is_warning());
        assert_eq!(trigger.description(), "Memory created");

        println!("[PASS] EvolutionTrigger::Created works correctly");
    }

    #[test]
    fn test_evolution_trigger_accessed() {
        let trigger = EvolutionTrigger::Accessed {
            query_context: "Find related documents".to_string(),
        };
        assert!(!trigger.is_warning());
        assert!(trigger.description().contains("Find related documents"));

        println!("[PASS] EvolutionTrigger::Accessed captures query context");
    }

    #[test]
    fn test_evolution_trigger_goal_changed() {
        let old_goal = Uuid::new_v4();
        let new_goal = Uuid::new_v4();
        let trigger = EvolutionTrigger::GoalChanged { old_goal, new_goal };
        assert!(!trigger.is_warning());

        let desc = trigger.description();
        assert!(desc.contains(&old_goal.to_string()));
        assert!(desc.contains(&new_goal.to_string()));

        println!("[PASS] EvolutionTrigger::GoalChanged captures both UUIDs");
    }

    #[test]
    fn test_evolution_trigger_recalibration() {
        let trigger = EvolutionTrigger::Recalibration;
        assert!(!trigger.is_warning());
        assert_eq!(trigger.description(), "Scheduled recalibration");

        println!("[PASS] EvolutionTrigger::Recalibration works correctly");
    }

    #[test]
    fn test_evolution_trigger_misalignment_detected() {
        let trigger = EvolutionTrigger::MisalignmentDetected { delta_a: -0.20 };
        assert!(trigger.is_warning());
        assert!(trigger.description().contains("-0.20"));

        println!("[PASS] EvolutionTrigger::MisalignmentDetected is a warning");
    }

    #[test]
    fn test_purpose_snapshot_new() {
        let purpose = make_test_purpose();
        let johari = make_test_johari();

        let before = Utc::now();
        let snapshot =
            PurposeSnapshot::new(purpose.clone(), johari.clone(), EvolutionTrigger::Created);
        let after = Utc::now();

        // Timestamp should be between before and after
        assert!(snapshot.timestamp >= before);
        assert!(snapshot.timestamp <= after);

        // Data should match
        assert_eq!(snapshot.purpose.alignments, purpose.alignments);
        assert!(matches!(snapshot.trigger, EvolutionTrigger::Created));

        println!("[PASS] PurposeSnapshot::new captures current time");
        println!("  - Timestamp: {}", snapshot.timestamp);
    }

    #[test]
    fn test_purpose_snapshot_with_timestamp() {
        let purpose = make_test_purpose();
        let johari = make_test_johari();
        let custom_time = Utc::now() - Duration::hours(24);

        let snapshot = PurposeSnapshot::with_timestamp(
            custom_time,
            purpose,
            johari,
            EvolutionTrigger::Recalibration,
        );

        assert_eq!(snapshot.timestamp, custom_time);

        println!("[PASS] PurposeSnapshot::with_timestamp accepts custom timestamp");
    }

    #[test]
    fn test_purpose_snapshot_aggregate_alignment() {
        let purpose = PurposeVector::new([0.80; NUM_EMBEDDERS]);
        let johari = make_test_johari();
        let snapshot = PurposeSnapshot::new(purpose, johari, EvolutionTrigger::Created);

        assert!((snapshot.aggregate_alignment() - 0.80).abs() < 1e-6);

        println!("[PASS] PurposeSnapshot::aggregate_alignment returns correct value");
    }

    #[test]
    fn test_evolution_trigger_display() {
        let triggers = vec![
            EvolutionTrigger::Created,
            EvolutionTrigger::Accessed {
                query_context: "test".to_string(),
            },
            EvolutionTrigger::Recalibration,
            EvolutionTrigger::MisalignmentDetected { delta_a: -0.18 },
        ];

        for trigger in triggers {
            let display = format!("{}", trigger);
            assert!(!display.is_empty());
            println!(
                "  - {}: {}",
                std::any::type_name::<EvolutionTrigger>(),
                display
            );
        }

        println!("[PASS] EvolutionTrigger Display trait works for all variants");
    }
}
