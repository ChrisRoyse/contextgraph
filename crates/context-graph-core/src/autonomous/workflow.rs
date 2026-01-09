//! Autonomous workflow orchestration types
//!
//! This module defines the complete configuration and status types for the
//! autonomous North Star system, including scheduling, event handling, and
//! health monitoring.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::bootstrap::{BootstrapConfig, GoalId};
use super::curation::{ConsolidationConfig, MemoryId, PruningConfig};
use super::drift::{DriftConfig, DriftState};
use super::evolution::GoalEvolutionConfig;
use super::thresholds::{AdaptiveThresholdConfig, AdaptiveThresholdState};

/// Complete autonomous mode configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutonomousConfig {
    /// Enable autonomous mode
    pub enabled: bool,
    /// Bootstrap configuration for initial North Star setup
    pub bootstrap: BootstrapConfig,
    /// Adaptive threshold learning configuration
    pub thresholds: AdaptiveThresholdConfig,
    /// Memory pruning configuration
    pub pruning: PruningConfig,
    /// Memory consolidation configuration
    pub consolidation: ConsolidationConfig,
    /// Drift detection configuration
    pub drift: DriftConfig,
    /// Goal evolution configuration
    pub goals: GoalEvolutionConfig,
}

impl Default for AutonomousConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bootstrap: BootstrapConfig::default(),
            thresholds: AdaptiveThresholdConfig::default(),
            pruning: PruningConfig::default(),
            consolidation: ConsolidationConfig::default(),
            drift: DriftConfig::default(),
            goals: GoalEvolutionConfig::default(),
        }
    }
}

impl AutonomousConfig {
    /// Create a disabled configuration (all features off)
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            bootstrap: BootstrapConfig {
                auto_init: false,
                ..Default::default()
            },
            thresholds: AdaptiveThresholdConfig {
                enabled: false,
                ..Default::default()
            },
            pruning: PruningConfig {
                enabled: false,
                ..Default::default()
            },
            consolidation: ConsolidationConfig {
                enabled: false,
                ..Default::default()
            },
            drift: DriftConfig {
                auto_correct: false,
                ..Default::default()
            },
            goals: GoalEvolutionConfig {
                auto_discover: false,
                ..Default::default()
            },
        }
    }

    /// Check if any autonomous feature is enabled
    pub fn has_any_enabled(&self) -> bool {
        self.enabled
            || self.bootstrap.auto_init
            || self.thresholds.enabled
            || self.pruning.enabled
            || self.consolidation.enabled
            || self.drift.auto_correct
            || self.goals.auto_discover
    }
}

/// Daily optimization schedule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailySchedule {
    /// Low activity window for heavy operations (start_hour, end_hour)
    pub consolidation_window: (u32, u32), // default: (0, 2) = midnight to 2am

    /// Morning drift check hour (0-23)
    pub drift_check_hour: u32, // default: 6

    /// Mid-day statistics collection hour (0-23)
    pub stats_hour: u32, // default: 12

    /// Evening preparation hour (0-23)
    pub prep_hour: u32, // default: 18
}

impl Default for DailySchedule {
    fn default() -> Self {
        Self {
            consolidation_window: (0, 2),
            drift_check_hour: 6,
            stats_hour: 12,
            prep_hour: 18,
        }
    }
}

impl DailySchedule {
    /// Check if a given hour falls within the consolidation window
    pub fn is_consolidation_hour(&self, hour: u32) -> bool {
        let (start, end) = self.consolidation_window;
        if start <= end {
            hour >= start && hour < end
        } else {
            // Handles wrap-around (e.g., 22 to 2)
            hour >= start || hour < end
        }
    }

    /// Get the next scheduled check type for a given hour
    pub fn next_check_for_hour(&self, hour: u32) -> Option<ScheduledCheckType> {
        if self.is_consolidation_hour(hour) {
            Some(ScheduledCheckType::ConsolidationWindow)
        } else if hour == self.drift_check_hour {
            Some(ScheduledCheckType::DriftCheck)
        } else if hour == self.stats_hour {
            Some(ScheduledCheckType::StatisticsCollection)
        } else if hour == self.prep_hour {
            Some(ScheduledCheckType::IndexOptimization)
        } else {
            None
        }
    }

    /// Validate that the schedule is consistent
    pub fn validate(&self) -> Result<(), ScheduleValidationError> {
        if self.drift_check_hour > 23 {
            return Err(ScheduleValidationError::InvalidHour {
                field: "drift_check_hour".into(),
                value: self.drift_check_hour,
            });
        }
        if self.stats_hour > 23 {
            return Err(ScheduleValidationError::InvalidHour {
                field: "stats_hour".into(),
                value: self.stats_hour,
            });
        }
        if self.prep_hour > 23 {
            return Err(ScheduleValidationError::InvalidHour {
                field: "prep_hour".into(),
                value: self.prep_hour,
            });
        }
        if self.consolidation_window.0 > 23 || self.consolidation_window.1 > 23 {
            return Err(ScheduleValidationError::InvalidConsolidationWindow {
                start: self.consolidation_window.0,
                end: self.consolidation_window.1,
            });
        }
        Ok(())
    }
}

/// Schedule validation error
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleValidationError {
    InvalidHour { field: String, value: u32 },
    InvalidConsolidationWindow { start: u32, end: u32 },
}

impl std::fmt::Display for ScheduleValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHour { field, value } => {
                write!(
                    f,
                    "Invalid hour {} for field '{}' (must be 0-23)",
                    value, field
                )
            }
            Self::InvalidConsolidationWindow { start, end } => {
                write!(
                    f,
                    "Invalid consolidation window ({}, {}) - hours must be 0-23",
                    start, end
                )
            }
        }
    }
}

impl std::error::Error for ScheduleValidationError {}

/// Scheduled check type for optimization events
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScheduledCheckType {
    /// Daily drift check
    DriftCheck,
    /// Consolidation window for heavy operations
    ConsolidationWindow,
    /// Statistics collection and reporting
    StatisticsCollection,
    /// Index optimization and maintenance
    IndexOptimization,
}

/// Optimization event trigger
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OptimizationEvent {
    /// A new memory was stored
    MemoryStored { memory_id: MemoryId },
    /// A memory was retrieved
    MemoryRetrieved { memory_id: MemoryId, query: String },
    /// The North Star was updated
    NorthStarUpdated,
    /// A new goal was added
    GoalAdded { goal_id: GoalId },
    /// Consciousness level dropped below threshold
    ConsciousnessDropped { level: f32 },
    /// A scheduled check is due
    ScheduledCheck { check_type: ScheduledCheckType },
}

impl OptimizationEvent {
    /// Get a descriptive name for this event type
    pub fn event_type_name(&self) -> &'static str {
        match self {
            Self::MemoryStored { .. } => "memory_stored",
            Self::MemoryRetrieved { .. } => "memory_retrieved",
            Self::NorthStarUpdated => "north_star_updated",
            Self::GoalAdded { .. } => "goal_added",
            Self::ConsciousnessDropped { .. } => "consciousness_dropped",
            Self::ScheduledCheck { .. } => "scheduled_check",
        }
    }

    /// Check if this event requires immediate processing
    pub fn is_urgent(&self) -> bool {
        matches!(
            self,
            Self::ConsciousnessDropped { .. } | Self::NorthStarUpdated
        )
    }
}

/// Autonomous system health status
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum AutonomousHealth {
    /// System is operating normally
    #[default]
    Healthy,
    /// System has warnings but is operational
    Warning { message: String },
    /// System has errors
    Error { message: String, recoverable: bool },
}

impl AutonomousHealth {
    /// Create a warning status
    pub fn warning(message: impl Into<String>) -> Self {
        Self::Warning {
            message: message.into(),
        }
    }

    /// Create a recoverable error status
    pub fn recoverable_error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            recoverable: true,
        }
    }

    /// Create a fatal error status
    pub fn fatal_error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            recoverable: false,
        }
    }

    /// Check if the system is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if the system can continue operating
    pub fn can_continue(&self) -> bool {
        match self {
            Self::Healthy | Self::Warning { .. } => true,
            Self::Error { recoverable, .. } => *recoverable,
        }
    }

    /// Get the message if any
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Healthy => None,
            Self::Warning { message } | Self::Error { message, .. } => Some(message),
        }
    }
}

/// Autonomous system status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutonomousStatus {
    /// Whether autonomous mode is enabled
    pub enabled: bool,
    /// Whether bootstrap has completed
    pub bootstrap_complete: bool,
    /// Whether a North Star is configured
    pub north_star_configured: bool,
    /// Current drift detection state
    pub drift_state: DriftState,
    /// Current adaptive threshold state
    pub threshold_state: AdaptiveThresholdState,
    /// Number of memories pending pruning review
    pub pending_prune_count: u32,
    /// Number of memories in consolidation queue
    pub consolidation_queue_size: u32,
    /// Last optimization timestamp
    pub last_optimization: DateTime<Utc>,
    /// Next scheduled operation
    pub next_scheduled: Option<DateTime<Utc>>,
    /// System health status
    pub health: AutonomousHealth,
}

impl Default for AutonomousStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            bootstrap_complete: false,
            north_star_configured: false,
            drift_state: DriftState::default(),
            threshold_state: AdaptiveThresholdState::default(),
            pending_prune_count: 0,
            consolidation_queue_size: 0,
            last_optimization: Utc::now(),
            next_scheduled: None,
            health: AutonomousHealth::default(),
        }
    }
}

impl AutonomousStatus {
    /// Create a status for a fully initialized system
    pub fn initialized(north_star_configured: bool) -> Self {
        Self {
            enabled: true,
            bootstrap_complete: true,
            north_star_configured,
            drift_state: DriftState::default(),
            threshold_state: AdaptiveThresholdState::default(),
            pending_prune_count: 0,
            consolidation_queue_size: 0,
            last_optimization: Utc::now(),
            next_scheduled: None,
            health: AutonomousHealth::Healthy,
        }
    }

    /// Check if the system is ready for autonomous operations
    pub fn is_ready(&self) -> bool {
        self.enabled && self.bootstrap_complete && self.health.can_continue()
    }

    /// Check if there is pending work
    pub fn has_pending_work(&self) -> bool {
        self.pending_prune_count > 0
            || self.consolidation_queue_size > 0
            || self.drift_state.requires_attention()
    }

    /// Get a summary of the current status
    pub fn summary(&self) -> String {
        let health_str = match &self.health {
            AutonomousHealth::Healthy => "healthy",
            AutonomousHealth::Warning { .. } => "warning",
            AutonomousHealth::Error { recoverable, .. } => {
                if *recoverable {
                    "error (recoverable)"
                } else {
                    "error (fatal)"
                }
            }
        };

        format!(
            "Autonomous[enabled={}, ready={}, health={}, prune={}, consolidate={}]",
            self.enabled,
            self.is_ready(),
            health_str,
            self.pending_prune_count,
            self.consolidation_queue_size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // AutonomousConfig tests
    #[test]
    fn test_autonomous_config_default() {
        let config = AutonomousConfig::default();
        assert!(config.enabled);
        assert!(config.bootstrap.auto_init);
        assert!(config.thresholds.enabled);
        assert!(config.pruning.enabled);
        assert!(config.consolidation.enabled);
        assert!(config.drift.auto_correct);
        assert!(config.goals.auto_discover);
    }

    #[test]
    fn test_autonomous_config_disabled() {
        let config = AutonomousConfig::disabled();
        assert!(!config.enabled);
        assert!(!config.bootstrap.auto_init);
        assert!(!config.thresholds.enabled);
        assert!(!config.pruning.enabled);
        assert!(!config.consolidation.enabled);
        assert!(!config.drift.auto_correct);
        assert!(!config.goals.auto_discover);
    }

    #[test]
    fn test_autonomous_config_has_any_enabled() {
        let config = AutonomousConfig::default();
        assert!(config.has_any_enabled());

        let disabled = AutonomousConfig::disabled();
        assert!(!disabled.has_any_enabled());

        // Test with just one feature enabled
        let partial = AutonomousConfig {
            enabled: false,
            bootstrap: BootstrapConfig {
                auto_init: true,
                ..Default::default()
            },
            thresholds: AdaptiveThresholdConfig {
                enabled: false,
                ..Default::default()
            },
            pruning: PruningConfig {
                enabled: false,
                ..Default::default()
            },
            consolidation: ConsolidationConfig {
                enabled: false,
                ..Default::default()
            },
            drift: DriftConfig {
                auto_correct: false,
                ..Default::default()
            },
            goals: GoalEvolutionConfig {
                auto_discover: false,
                ..Default::default()
            },
        };
        assert!(partial.has_any_enabled());
    }

    #[test]
    fn test_autonomous_config_serialization() {
        let config = AutonomousConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: AutonomousConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.bootstrap.auto_init, config.bootstrap.auto_init);
        assert_eq!(deserialized.thresholds.enabled, config.thresholds.enabled);
        assert_eq!(deserialized.pruning.enabled, config.pruning.enabled);
        assert_eq!(
            deserialized.consolidation.enabled,
            config.consolidation.enabled
        );
        assert_eq!(deserialized.drift.auto_correct, config.drift.auto_correct);
        assert_eq!(deserialized.goals.auto_discover, config.goals.auto_discover);
    }

    #[test]
    fn test_autonomous_config_nested_values() {
        let config = AutonomousConfig::default();

        // Verify nested default values are correct
        assert_eq!(config.pruning.min_age_days, 30);
        assert!((config.consolidation.similarity_threshold - 0.92).abs() < f32::EPSILON);
        assert!((config.drift.alert_threshold - 0.05).abs() < f32::EPSILON);
        assert_eq!(config.goals.min_cluster_size, 10);
        assert!((config.thresholds.learning_rate - 0.01).abs() < f32::EPSILON);
    }

    // DailySchedule tests
    #[test]
    fn test_daily_schedule_default() {
        let schedule = DailySchedule::default();
        assert_eq!(schedule.consolidation_window, (0, 2));
        assert_eq!(schedule.drift_check_hour, 6);
        assert_eq!(schedule.stats_hour, 12);
        assert_eq!(schedule.prep_hour, 18);
    }

    #[test]
    fn test_daily_schedule_is_consolidation_hour() {
        let schedule = DailySchedule::default(); // Window: 0 to 2

        assert!(schedule.is_consolidation_hour(0));
        assert!(schedule.is_consolidation_hour(1));
        assert!(!schedule.is_consolidation_hour(2));
        assert!(!schedule.is_consolidation_hour(12));
        assert!(!schedule.is_consolidation_hour(23));
    }

    #[test]
    fn test_daily_schedule_is_consolidation_hour_wraparound() {
        let schedule = DailySchedule {
            consolidation_window: (22, 2), // 10pm to 2am
            ..Default::default()
        };

        assert!(schedule.is_consolidation_hour(22));
        assert!(schedule.is_consolidation_hour(23));
        assert!(schedule.is_consolidation_hour(0));
        assert!(schedule.is_consolidation_hour(1));
        assert!(!schedule.is_consolidation_hour(2));
        assert!(!schedule.is_consolidation_hour(12));
        assert!(!schedule.is_consolidation_hour(21));
    }

    #[test]
    fn test_daily_schedule_next_check_for_hour() {
        let schedule = DailySchedule::default();

        // Consolidation window
        assert_eq!(
            schedule.next_check_for_hour(0),
            Some(ScheduledCheckType::ConsolidationWindow)
        );
        assert_eq!(
            schedule.next_check_for_hour(1),
            Some(ScheduledCheckType::ConsolidationWindow)
        );

        // Drift check hour
        assert_eq!(
            schedule.next_check_for_hour(6),
            Some(ScheduledCheckType::DriftCheck)
        );

        // Stats hour
        assert_eq!(
            schedule.next_check_for_hour(12),
            Some(ScheduledCheckType::StatisticsCollection)
        );

        // Prep hour
        assert_eq!(
            schedule.next_check_for_hour(18),
            Some(ScheduledCheckType::IndexOptimization)
        );

        // No scheduled check
        assert_eq!(schedule.next_check_for_hour(10), None);
        assert_eq!(schedule.next_check_for_hour(15), None);
    }

    #[test]
    fn test_daily_schedule_validate_valid() {
        let schedule = DailySchedule::default();
        assert!(schedule.validate().is_ok());

        let custom = DailySchedule {
            consolidation_window: (22, 4),
            drift_check_hour: 23,
            stats_hour: 0,
            prep_hour: 23,
        };
        assert!(custom.validate().is_ok());
    }

    #[test]
    fn test_daily_schedule_validate_invalid_hours() {
        let invalid_drift = DailySchedule {
            drift_check_hour: 24,
            ..Default::default()
        };
        assert!(matches!(
            invalid_drift.validate(),
            Err(ScheduleValidationError::InvalidHour { field, value }) if field == "drift_check_hour" && value == 24
        ));

        let invalid_stats = DailySchedule {
            stats_hour: 25,
            ..Default::default()
        };
        assert!(matches!(
            invalid_stats.validate(),
            Err(ScheduleValidationError::InvalidHour { field, value }) if field == "stats_hour" && value == 25
        ));

        let invalid_prep = DailySchedule {
            prep_hour: 100,
            ..Default::default()
        };
        assert!(matches!(
            invalid_prep.validate(),
            Err(ScheduleValidationError::InvalidHour { field, value }) if field == "prep_hour" && value == 100
        ));
    }

    #[test]
    fn test_daily_schedule_validate_invalid_window() {
        let invalid_window = DailySchedule {
            consolidation_window: (24, 2),
            ..Default::default()
        };
        assert!(matches!(
            invalid_window.validate(),
            Err(ScheduleValidationError::InvalidConsolidationWindow { .. })
        ));

        let invalid_window2 = DailySchedule {
            consolidation_window: (0, 25),
            ..Default::default()
        };
        assert!(matches!(
            invalid_window2.validate(),
            Err(ScheduleValidationError::InvalidConsolidationWindow { .. })
        ));
    }

    #[test]
    fn test_daily_schedule_serialization() {
        let schedule = DailySchedule::default();
        let json = serde_json::to_string(&schedule).expect("serialize");
        let deserialized: DailySchedule = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(
            deserialized.consolidation_window,
            schedule.consolidation_window
        );
        assert_eq!(deserialized.drift_check_hour, schedule.drift_check_hour);
        assert_eq!(deserialized.stats_hour, schedule.stats_hour);
        assert_eq!(deserialized.prep_hour, schedule.prep_hour);
    }

    // ScheduleValidationError tests
    #[test]
    fn test_schedule_validation_error_display() {
        let error = ScheduleValidationError::InvalidHour {
            field: "test_field".into(),
            value: 25,
        };
        assert!(error.to_string().contains("25"));
        assert!(error.to_string().contains("test_field"));

        let error = ScheduleValidationError::InvalidConsolidationWindow { start: 24, end: 25 };
        assert!(error.to_string().contains("24"));
        assert!(error.to_string().contains("25"));
    }

    // ScheduledCheckType tests
    #[test]
    fn test_scheduled_check_type_equality() {
        assert_eq!(
            ScheduledCheckType::DriftCheck,
            ScheduledCheckType::DriftCheck
        );
        assert_ne!(
            ScheduledCheckType::DriftCheck,
            ScheduledCheckType::ConsolidationWindow
        );
        assert_ne!(
            ScheduledCheckType::ConsolidationWindow,
            ScheduledCheckType::StatisticsCollection
        );
        assert_ne!(
            ScheduledCheckType::StatisticsCollection,
            ScheduledCheckType::IndexOptimization
        );
    }

    #[test]
    fn test_scheduled_check_type_serialization() {
        let types = [
            ScheduledCheckType::DriftCheck,
            ScheduledCheckType::ConsolidationWindow,
            ScheduledCheckType::StatisticsCollection,
            ScheduledCheckType::IndexOptimization,
        ];

        for check_type in types {
            let json = serde_json::to_string(&check_type).expect("serialize");
            let deserialized: ScheduledCheckType =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, check_type);
        }
    }

    // OptimizationEvent tests
    #[test]
    fn test_optimization_event_memory_stored() {
        let memory_id = MemoryId::new();
        let event = OptimizationEvent::MemoryStored {
            memory_id: memory_id.clone(),
        };

        assert_eq!(event.event_type_name(), "memory_stored");
        assert!(!event.is_urgent());
    }

    #[test]
    fn test_optimization_event_memory_retrieved() {
        let memory_id = MemoryId::new();
        let event = OptimizationEvent::MemoryRetrieved {
            memory_id: memory_id.clone(),
            query: "test query".into(),
        };

        assert_eq!(event.event_type_name(), "memory_retrieved");
        assert!(!event.is_urgent());
    }

    #[test]
    fn test_optimization_event_north_star_updated() {
        let event = OptimizationEvent::NorthStarUpdated;

        assert_eq!(event.event_type_name(), "north_star_updated");
        assert!(event.is_urgent());
    }

    #[test]
    fn test_optimization_event_goal_added() {
        let goal_id = GoalId::new();
        let event = OptimizationEvent::GoalAdded {
            goal_id: goal_id.clone(),
        };

        assert_eq!(event.event_type_name(), "goal_added");
        assert!(!event.is_urgent());
    }

    #[test]
    fn test_optimization_event_consciousness_dropped() {
        let event = OptimizationEvent::ConsciousnessDropped { level: 0.3 };

        assert_eq!(event.event_type_name(), "consciousness_dropped");
        assert!(event.is_urgent());
    }

    #[test]
    fn test_optimization_event_scheduled_check() {
        let event = OptimizationEvent::ScheduledCheck {
            check_type: ScheduledCheckType::DriftCheck,
        };

        assert_eq!(event.event_type_name(), "scheduled_check");
        assert!(!event.is_urgent());
    }

    #[test]
    fn test_optimization_event_serialization() {
        let events = [
            OptimizationEvent::MemoryStored {
                memory_id: MemoryId::new(),
            },
            OptimizationEvent::MemoryRetrieved {
                memory_id: MemoryId::new(),
                query: "test".into(),
            },
            OptimizationEvent::NorthStarUpdated,
            OptimizationEvent::GoalAdded {
                goal_id: GoalId::new(),
            },
            OptimizationEvent::ConsciousnessDropped { level: 0.5 },
            OptimizationEvent::ScheduledCheck {
                check_type: ScheduledCheckType::DriftCheck,
            },
        ];

        for event in events {
            let json = serde_json::to_string(&event).expect("serialize");
            let _deserialized: OptimizationEvent =
                serde_json::from_str(&json).expect("deserialize");
            // Can't compare events directly due to random IDs, but serialization works
        }
    }

    // AutonomousHealth tests
    #[test]
    fn test_autonomous_health_default() {
        let health = AutonomousHealth::default();
        assert_eq!(health, AutonomousHealth::Healthy);
        assert!(health.is_healthy());
        assert!(health.can_continue());
        assert!(health.message().is_none());
    }

    #[test]
    fn test_autonomous_health_warning() {
        let health = AutonomousHealth::warning("Test warning");
        assert!(!health.is_healthy());
        assert!(health.can_continue());
        assert_eq!(health.message(), Some("Test warning"));
    }

    #[test]
    fn test_autonomous_health_recoverable_error() {
        let health = AutonomousHealth::recoverable_error("Recoverable error");
        assert!(!health.is_healthy());
        assert!(health.can_continue());
        assert_eq!(health.message(), Some("Recoverable error"));
    }

    #[test]
    fn test_autonomous_health_fatal_error() {
        let health = AutonomousHealth::fatal_error("Fatal error");
        assert!(!health.is_healthy());
        assert!(!health.can_continue());
        assert_eq!(health.message(), Some("Fatal error"));
    }

    #[test]
    fn test_autonomous_health_equality() {
        assert_eq!(AutonomousHealth::Healthy, AutonomousHealth::Healthy);
        assert_ne!(
            AutonomousHealth::Healthy,
            AutonomousHealth::Warning {
                message: "test".into()
            }
        );
        assert_eq!(
            AutonomousHealth::Warning {
                message: "test".into()
            },
            AutonomousHealth::Warning {
                message: "test".into()
            }
        );
        assert_ne!(
            AutonomousHealth::Warning {
                message: "test1".into()
            },
            AutonomousHealth::Warning {
                message: "test2".into()
            }
        );
    }

    #[test]
    fn test_autonomous_health_serialization() {
        let healths = [
            AutonomousHealth::Healthy,
            AutonomousHealth::Warning {
                message: "test warning".into(),
            },
            AutonomousHealth::Error {
                message: "recoverable".into(),
                recoverable: true,
            },
            AutonomousHealth::Error {
                message: "fatal".into(),
                recoverable: false,
            },
        ];

        for health in healths {
            let json = serde_json::to_string(&health).expect("serialize");
            let deserialized: AutonomousHealth = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, health);
        }
    }

    // AutonomousStatus tests
    #[test]
    fn test_autonomous_status_default() {
        let status = AutonomousStatus::default();
        assert!(!status.enabled);
        assert!(!status.bootstrap_complete);
        assert!(!status.north_star_configured);
        assert_eq!(status.pending_prune_count, 0);
        assert_eq!(status.consolidation_queue_size, 0);
        assert!(status.next_scheduled.is_none());
        assert_eq!(status.health, AutonomousHealth::Healthy);
    }

    #[test]
    fn test_autonomous_status_initialized() {
        let status = AutonomousStatus::initialized(true);
        assert!(status.enabled);
        assert!(status.bootstrap_complete);
        assert!(status.north_star_configured);
        assert!(status.health.is_healthy());
    }

    #[test]
    fn test_autonomous_status_initialized_without_north_star() {
        let status = AutonomousStatus::initialized(false);
        assert!(status.enabled);
        assert!(status.bootstrap_complete);
        assert!(!status.north_star_configured);
    }

    #[test]
    fn test_autonomous_status_is_ready() {
        // Not ready - disabled
        let status = AutonomousStatus::default();
        assert!(!status.is_ready());

        // Ready - enabled and bootstrapped
        let status = AutonomousStatus::initialized(true);
        assert!(status.is_ready());

        // Not ready - has fatal error
        let mut status = AutonomousStatus::initialized(true);
        status.health = AutonomousHealth::fatal_error("Fatal");
        assert!(!status.is_ready());

        // Ready - has warning
        let mut status = AutonomousStatus::initialized(true);
        status.health = AutonomousHealth::warning("Warning");
        assert!(status.is_ready());

        // Ready - has recoverable error
        let mut status = AutonomousStatus::initialized(true);
        status.health = AutonomousHealth::recoverable_error("Recoverable");
        assert!(status.is_ready());
    }

    #[test]
    fn test_autonomous_status_has_pending_work() {
        let status = AutonomousStatus::default();
        assert!(!status.has_pending_work());

        // With pending prune
        let mut status = AutonomousStatus::default();
        status.pending_prune_count = 5;
        assert!(status.has_pending_work());

        // With consolidation queue
        let mut status = AutonomousStatus::default();
        status.consolidation_queue_size = 10;
        assert!(status.has_pending_work());

        // With drift requiring attention
        let mut status = AutonomousStatus::default();
        let drift_config = DriftConfig::default();
        status.drift_state.add_data_point(0.65, 5, &drift_config); // Creates moderate drift
        status.drift_state.baseline = 0.75;
        status.drift_state.add_data_point(0.65, 5, &drift_config);
        assert!(status.has_pending_work());
    }

    #[test]
    fn test_autonomous_status_summary() {
        let status = AutonomousStatus::default();
        let summary = status.summary();
        assert!(summary.contains("enabled=false"));
        assert!(summary.contains("ready=false"));
        assert!(summary.contains("healthy"));

        let status = AutonomousStatus::initialized(true);
        let summary = status.summary();
        assert!(summary.contains("enabled=true"));
        assert!(summary.contains("ready=true"));

        let mut status = AutonomousStatus::default();
        status.pending_prune_count = 5;
        status.consolidation_queue_size = 10;
        let summary = status.summary();
        assert!(summary.contains("prune=5"));
        assert!(summary.contains("consolidate=10"));
    }

    #[test]
    fn test_autonomous_status_summary_with_errors() {
        let mut status = AutonomousStatus::default();
        status.health = AutonomousHealth::warning("Warning");
        let summary = status.summary();
        assert!(summary.contains("warning"));

        let mut status = AutonomousStatus::default();
        status.health = AutonomousHealth::recoverable_error("Error");
        let summary = status.summary();
        assert!(summary.contains("recoverable"));

        let mut status = AutonomousStatus::default();
        status.health = AutonomousHealth::fatal_error("Fatal");
        let summary = status.summary();
        assert!(summary.contains("fatal"));
    }

    #[test]
    fn test_autonomous_status_serialization() {
        let status = AutonomousStatus::initialized(true);
        let json = serde_json::to_string(&status).expect("serialize");
        let deserialized: AutonomousStatus = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.enabled, status.enabled);
        assert_eq!(deserialized.bootstrap_complete, status.bootstrap_complete);
        assert_eq!(
            deserialized.north_star_configured,
            status.north_star_configured
        );
        assert_eq!(deserialized.pending_prune_count, status.pending_prune_count);
        assert_eq!(
            deserialized.consolidation_queue_size,
            status.consolidation_queue_size
        );
        assert_eq!(deserialized.health, status.health);
    }

    #[test]
    fn test_autonomous_status_with_all_fields_populated() {
        let now = Utc::now();
        let next = now + chrono::Duration::hours(1);

        let status = AutonomousStatus {
            enabled: true,
            bootstrap_complete: true,
            north_star_configured: true,
            drift_state: DriftState::with_baseline(0.80),
            threshold_state: AdaptiveThresholdState::default(),
            pending_prune_count: 15,
            consolidation_queue_size: 25,
            last_optimization: now,
            next_scheduled: Some(next),
            health: AutonomousHealth::warning("Test warning"),
        };

        // Verify all fields
        assert!(status.enabled);
        assert!(status.bootstrap_complete);
        assert!(status.north_star_configured);
        assert!((status.drift_state.baseline - 0.80).abs() < f32::EPSILON);
        assert_eq!(status.pending_prune_count, 15);
        assert_eq!(status.consolidation_queue_size, 25);
        assert!(status.next_scheduled.is_some());
        assert!(!status.health.is_healthy());

        // Test serialization roundtrip
        let json = serde_json::to_string(&status).expect("serialize");
        let deserialized: AutonomousStatus = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(deserialized.enabled, status.enabled);
        assert_eq!(deserialized.pending_prune_count, status.pending_prune_count);
        assert!(deserialized.next_scheduled.is_some());
    }

    // Integration tests
    #[test]
    fn test_config_and_status_interaction() {
        let config = AutonomousConfig::default();
        let mut status = AutonomousStatus::default();

        // Simulate initialization based on config
        status.enabled = config.enabled;
        status.bootstrap_complete = config.bootstrap.auto_init;

        assert!(status.enabled);
        assert!(status.bootstrap_complete);
    }

    #[test]
    fn test_schedule_generates_events() {
        let schedule = DailySchedule::default();

        // Check that each scheduled hour produces an event
        for hour in 0..24 {
            if let Some(check_type) = schedule.next_check_for_hour(hour) {
                let event = OptimizationEvent::ScheduledCheck { check_type };
                assert_eq!(event.event_type_name(), "scheduled_check");
            }
        }
    }

    #[test]
    fn test_health_transitions() {
        let mut health = AutonomousHealth::Healthy;
        assert!(health.is_healthy());
        assert!(health.can_continue());

        health = AutonomousHealth::warning("Minor issue");
        assert!(!health.is_healthy());
        assert!(health.can_continue());

        health = AutonomousHealth::recoverable_error("Temporary failure");
        assert!(!health.is_healthy());
        assert!(health.can_continue());

        health = AutonomousHealth::fatal_error("Critical failure");
        assert!(!health.is_healthy());
        assert!(!health.can_continue());
    }
}
