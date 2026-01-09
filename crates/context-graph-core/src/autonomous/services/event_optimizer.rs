//! NORTH-019: Event-driven optimization service
//!
//! This module provides the EventOptimizer service that performs event-driven
//! optimization based on system events such as high drift, low performance,
//! memory pressure, and scheduled maintenance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Configuration for the EventOptimizer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventOptimizerConfig {
    /// Maximum number of events to retain in history
    pub max_history_size: usize,
    /// High drift severity threshold
    pub high_drift_threshold: f32,
    /// Low performance threshold
    pub low_performance_threshold: f32,
    /// Memory pressure threshold (percentage)
    pub memory_pressure_threshold: f32,
    /// Enable automatic optimization on events
    pub auto_optimize: bool,
}

impl Default for EventOptimizerConfig {
    fn default() -> Self {
        Self {
            max_history_size: 1000,
            high_drift_threshold: 0.10,
            low_performance_threshold: 0.50,
            memory_pressure_threshold: 85.0,
            auto_optimize: true,
        }
    }
}

/// Trigger for optimization operations
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum OptimizationTrigger {
    /// High drift detected in alignment
    HighDrift { severity: f32 },
    /// Performance metric fell below threshold
    LowPerformance { metric: String, value: f32 },
    /// Memory usage exceeded threshold
    MemoryPressure { usage_percent: f32 },
    /// Scheduled maintenance window
    ScheduledMaintenance,
    /// User-initiated optimization
    UserTriggered { reason: String },
}

impl OptimizationTrigger {
    /// Get a descriptive name for the trigger type
    pub fn trigger_type_name(&self) -> &'static str {
        match self {
            Self::HighDrift { .. } => "high_drift",
            Self::LowPerformance { .. } => "low_performance",
            Self::MemoryPressure { .. } => "memory_pressure",
            Self::ScheduledMaintenance => "scheduled_maintenance",
            Self::UserTriggered { .. } => "user_triggered",
        }
    }

    /// Check if this trigger is critical and requires immediate action
    pub fn is_critical(&self) -> bool {
        match self {
            Self::HighDrift { severity } => *severity >= 0.20,
            Self::MemoryPressure { usage_percent } => *usage_percent >= 95.0,
            _ => false,
        }
    }

    /// Get the base priority for this trigger type
    pub fn base_priority(&self) -> u8 {
        match self {
            Self::HighDrift { severity } if *severity >= 0.20 => 10,
            Self::HighDrift { severity } if *severity >= 0.10 => 8,
            Self::HighDrift { .. } => 5,
            Self::MemoryPressure { usage_percent } if *usage_percent >= 95.0 => 10,
            Self::MemoryPressure { usage_percent } if *usage_percent >= 90.0 => 8,
            Self::MemoryPressure { .. } => 6,
            Self::LowPerformance { value, .. } if *value < 0.30 => 7,
            Self::LowPerformance { .. } => 5,
            Self::UserTriggered { .. } => 9,
            Self::ScheduledMaintenance => 3,
        }
    }
}

/// Actions that can be performed during optimization
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum OptimizationAction {
    /// Rebuild memory indices for faster retrieval
    ReindexMemories,
    /// Recompute alignment scores for all memories
    RecomputeAlignments,
    /// Remove stale or orphaned data
    PruneStaleData,
    /// Adjust weights based on usage patterns
    RebalanceWeights,
    /// Compact storage to reclaim space
    CompactStorage,
}

impl OptimizationAction {
    /// Get the estimated cost of this action (0.0 to 1.0)
    pub fn estimated_cost(&self) -> f32 {
        match self {
            Self::ReindexMemories => 0.8,
            Self::RecomputeAlignments => 0.9,
            Self::PruneStaleData => 0.4,
            Self::RebalanceWeights => 0.3,
            Self::CompactStorage => 0.7,
        }
    }

    /// Get the estimated benefit of this action (0.0 to 1.0)
    pub fn estimated_benefit(&self) -> f32 {
        match self {
            Self::ReindexMemories => 0.7,
            Self::RecomputeAlignments => 0.8,
            Self::PruneStaleData => 0.5,
            Self::RebalanceWeights => 0.6,
            Self::CompactStorage => 0.4,
        }
    }

    /// Get a human-readable description of this action
    pub fn description(&self) -> &'static str {
        match self {
            Self::ReindexMemories => "Rebuild memory indices for faster retrieval",
            Self::RecomputeAlignments => "Recompute alignment scores for all memories",
            Self::PruneStaleData => "Remove stale or orphaned data",
            Self::RebalanceWeights => "Adjust weights based on usage patterns",
            Self::CompactStorage => "Compact storage to reclaim space",
        }
    }

    /// Get actions recommended for a given trigger
    pub fn recommended_for_trigger(trigger: &OptimizationTrigger) -> Vec<Self> {
        match trigger {
            OptimizationTrigger::HighDrift { severity } => {
                if *severity >= 0.20 {
                    vec![
                        Self::RecomputeAlignments,
                        Self::RebalanceWeights,
                        Self::ReindexMemories,
                    ]
                } else {
                    vec![Self::RecomputeAlignments, Self::RebalanceWeights]
                }
            }
            OptimizationTrigger::LowPerformance { metric, .. } => {
                if metric.contains("retrieval") || metric.contains("search") {
                    vec![Self::ReindexMemories, Self::CompactStorage]
                } else {
                    vec![Self::RebalanceWeights, Self::PruneStaleData]
                }
            }
            OptimizationTrigger::MemoryPressure { usage_percent } => {
                if *usage_percent >= 95.0 {
                    vec![
                        Self::PruneStaleData,
                        Self::CompactStorage,
                        Self::ReindexMemories,
                    ]
                } else {
                    vec![Self::PruneStaleData, Self::CompactStorage]
                }
            }
            OptimizationTrigger::ScheduledMaintenance => {
                vec![
                    Self::PruneStaleData,
                    Self::CompactStorage,
                    Self::ReindexMemories,
                    Self::RebalanceWeights,
                ]
            }
            OptimizationTrigger::UserTriggered { .. } => {
                vec![
                    Self::RecomputeAlignments,
                    Self::ReindexMemories,
                    Self::RebalanceWeights,
                    Self::PruneStaleData,
                    Self::CompactStorage,
                ]
            }
        }
    }
}

/// System metrics snapshot
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Average alignment score
    pub avg_alignment: f32,
    /// Average retrieval latency in milliseconds
    pub avg_latency_ms: f32,
    /// Memory usage percentage
    pub memory_usage_percent: f32,
    /// Total memory count
    pub memory_count: u64,
    /// Index health score (0.0 to 1.0)
    pub index_health: f32,
}

impl SystemMetrics {
    /// Create a new metrics snapshot
    pub fn new(
        avg_alignment: f32,
        avg_latency_ms: f32,
        memory_usage_percent: f32,
        memory_count: u64,
        index_health: f32,
    ) -> Self {
        Self {
            avg_alignment,
            avg_latency_ms,
            memory_usage_percent,
            memory_count,
            index_health,
        }
    }

    /// Calculate improvement from before to after
    pub fn calculate_improvement(&self, after: &Self) -> f32 {
        let alignment_improvement = after.avg_alignment - self.avg_alignment;
        let latency_improvement = if self.avg_latency_ms > 0.0 {
            (self.avg_latency_ms - after.avg_latency_ms) / self.avg_latency_ms
        } else {
            0.0
        };
        let memory_improvement = self.memory_usage_percent - after.memory_usage_percent;
        let index_improvement = after.index_health - self.index_health;

        // Weighted average of improvements
        (alignment_improvement * 0.4
            + latency_improvement * 0.3
            + memory_improvement * 0.01
            + index_improvement * 0.29)
            .clamp(-1.0, 1.0)
    }
}

/// Record of an optimization event
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationEventRecord {
    /// Type of event that triggered optimization
    pub event_type: String,
    /// When the optimization occurred
    pub timestamp: DateTime<Utc>,
    /// System metrics before optimization
    pub metrics_before: SystemMetrics,
    /// System metrics after optimization
    pub metrics_after: SystemMetrics,
    /// Whether the optimization was successful
    pub success: bool,
    /// Actions that were executed
    pub actions_executed: Vec<OptimizationAction>,
    /// Duration of the optimization in milliseconds
    pub duration_ms: u64,
    /// Error message if optimization failed
    pub error: Option<String>,
}

impl OptimizationEventRecord {
    /// Calculate the improvement achieved by this optimization
    pub fn improvement(&self) -> f32 {
        if self.success {
            self.metrics_before
                .calculate_improvement(&self.metrics_after)
        } else {
            0.0
        }
    }
}

/// Plan for an optimization operation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizationPlan {
    /// The trigger that initiated this plan
    pub trigger: OptimizationTrigger,
    /// Actions to be executed
    pub actions: Vec<OptimizationAction>,
    /// Priority level (0-10, higher is more urgent)
    pub priority: u8,
    /// Estimated impact (0.0 to 1.0)
    pub estimated_impact: f32,
    /// When the plan was created
    pub created_at: DateTime<Utc>,
}

impl OptimizationPlan {
    /// Create a new optimization plan
    pub fn new(trigger: OptimizationTrigger, actions: Vec<OptimizationAction>) -> Self {
        let priority = trigger.base_priority();
        let estimated_impact = Self::calculate_estimated_impact(&actions);
        Self {
            trigger,
            actions,
            priority,
            estimated_impact,
            created_at: Utc::now(),
        }
    }

    /// Create a plan with custom priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority.min(10);
        self
    }

    /// Calculate estimated impact from actions
    fn calculate_estimated_impact(actions: &[OptimizationAction]) -> f32 {
        if actions.is_empty() {
            return 0.0;
        }

        let total_benefit: f32 = actions.iter().map(|a| a.estimated_benefit()).sum();
        let total_cost: f32 = actions.iter().map(|a| a.estimated_cost()).sum();

        if total_cost > 0.0 {
            (total_benefit / actions.len() as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Check if this plan is empty
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Get total estimated cost
    pub fn total_cost(&self) -> f32 {
        self.actions.iter().map(|a| a.estimated_cost()).sum()
    }
}

/// Event-driven optimizer service
#[derive(Debug)]
pub struct EventOptimizer {
    config: EventOptimizerConfig,
    event_history: VecDeque<OptimizationEventRecord>,
    current_metrics: SystemMetrics,
    optimization_count: u64,
    total_improvement: f32,
}

impl Default for EventOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl EventOptimizer {
    /// Create a new EventOptimizer with default configuration
    pub fn new() -> Self {
        Self {
            config: EventOptimizerConfig::default(),
            event_history: VecDeque::with_capacity(1000),
            current_metrics: SystemMetrics::default(),
            optimization_count: 0,
            total_improvement: 0.0,
        }
    }

    /// Create an EventOptimizer with custom configuration
    pub fn with_config(config: EventOptimizerConfig) -> Self {
        let capacity = config.max_history_size;
        Self {
            config,
            event_history: VecDeque::with_capacity(capacity),
            current_metrics: SystemMetrics::default(),
            optimization_count: 0,
            total_improvement: 0.0,
        }
    }

    /// Handle an optimization trigger event and return a plan
    pub fn on_event(&mut self, trigger: OptimizationTrigger) -> OptimizationPlan {
        self.plan_optimization(&trigger)
    }

    /// Create an optimization plan for a given trigger
    pub fn plan_optimization(&self, trigger: &OptimizationTrigger) -> OptimizationPlan {
        let mut actions = OptimizationAction::recommended_for_trigger(trigger);
        self.prioritize_actions(&mut actions);
        OptimizationPlan::new(trigger.clone(), actions)
    }

    /// Execute an optimization plan and return the event record
    pub fn execute_plan(&mut self, plan: &OptimizationPlan) -> OptimizationEventRecord {
        let start_time = std::time::Instant::now();
        let metrics_before = self.current_metrics.clone();

        // Simulate execution of actions
        let (success, error) = self.execute_actions(&plan.actions);

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let metrics_after = self.current_metrics.clone();

        let record = OptimizationEventRecord {
            event_type: plan.trigger.trigger_type_name().to_string(),
            timestamp: Utc::now(),
            metrics_before,
            metrics_after,
            success,
            actions_executed: plan.actions.clone(),
            duration_ms,
            error,
        };

        self.record_event(record.clone());
        record
    }

    /// Execute a list of optimization actions
    fn execute_actions(&mut self, actions: &[OptimizationAction]) -> (bool, Option<String>) {
        if actions.is_empty() {
            return (true, None);
        }

        for action in actions {
            // Apply simulated improvements based on action type
            match action {
                OptimizationAction::ReindexMemories => {
                    self.current_metrics.index_health =
                        (self.current_metrics.index_health + 0.1).min(1.0);
                    self.current_metrics.avg_latency_ms =
                        (self.current_metrics.avg_latency_ms * 0.9).max(1.0);
                }
                OptimizationAction::RecomputeAlignments => {
                    self.current_metrics.avg_alignment =
                        (self.current_metrics.avg_alignment + 0.05).min(1.0);
                }
                OptimizationAction::PruneStaleData => {
                    self.current_metrics.memory_usage_percent =
                        (self.current_metrics.memory_usage_percent - 5.0).max(0.0);
                }
                OptimizationAction::RebalanceWeights => {
                    self.current_metrics.avg_alignment =
                        (self.current_metrics.avg_alignment + 0.02).min(1.0);
                }
                OptimizationAction::CompactStorage => {
                    self.current_metrics.memory_usage_percent =
                        (self.current_metrics.memory_usage_percent - 3.0).max(0.0);
                }
            }
        }

        (true, None)
    }

    /// Estimate the impact of a set of actions
    pub fn estimate_impact(&self, actions: &[OptimizationAction]) -> f32 {
        if actions.is_empty() {
            return 0.0;
        }

        let total_benefit: f32 = actions.iter().map(|a| a.estimated_benefit()).sum();
        (total_benefit / actions.len() as f32).clamp(0.0, 1.0)
    }

    /// Prioritize actions based on current system state
    pub fn prioritize_actions(&self, actions: &mut [OptimizationAction]) {
        // Sort by benefit-to-cost ratio, descending
        actions.sort_by(|a, b| {
            let ratio_a = if a.estimated_cost() > 0.0 {
                a.estimated_benefit() / a.estimated_cost()
            } else {
                f32::MAX
            };
            let ratio_b = if b.estimated_cost() > 0.0 {
                b.estimated_benefit() / b.estimated_cost()
            } else {
                f32::MAX
            };
            ratio_b
                .partial_cmp(&ratio_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Additional prioritization based on current metrics
        let needs_memory_relief = self.current_metrics.memory_usage_percent > 80.0;
        let needs_index_repair = self.current_metrics.index_health < 0.7;

        if needs_memory_relief {
            // Move PruneStaleData and CompactStorage to the front
            actions.sort_by(|a, b| {
                let priority_a = matches!(
                    a,
                    OptimizationAction::PruneStaleData | OptimizationAction::CompactStorage
                );
                let priority_b = matches!(
                    b,
                    OptimizationAction::PruneStaleData | OptimizationAction::CompactStorage
                );
                priority_b.cmp(&priority_a)
            });
        } else if needs_index_repair {
            // Move ReindexMemories to the front
            actions.sort_by(|a, b| {
                let priority_a = matches!(a, OptimizationAction::ReindexMemories);
                let priority_b = matches!(b, OptimizationAction::ReindexMemories);
                priority_b.cmp(&priority_a)
            });
        }
    }

    /// Record an optimization event in history
    pub fn record_event(&mut self, event: OptimizationEventRecord) {
        if event.success {
            self.optimization_count += 1;
            self.total_improvement += event.improvement();
        }

        self.event_history.push_back(event);

        // Trim history if needed
        while self.event_history.len() > self.config.max_history_size {
            self.event_history.pop_front();
        }
    }

    /// Get the event history
    pub fn event_history(&self) -> &VecDeque<OptimizationEventRecord> {
        &self.event_history
    }

    /// Get the current metrics
    pub fn current_metrics(&self) -> &SystemMetrics {
        &self.current_metrics
    }

    /// Set the current metrics (for testing or external updates)
    pub fn set_current_metrics(&mut self, metrics: SystemMetrics) {
        self.current_metrics = metrics;
    }

    /// Get total optimization count
    pub fn optimization_count(&self) -> u64 {
        self.optimization_count
    }

    /// Get average improvement across all optimizations
    pub fn average_improvement(&self) -> f32 {
        if self.optimization_count > 0 {
            self.total_improvement / self.optimization_count as f32
        } else {
            0.0
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &EventOptimizerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // EventOptimizerConfig tests
    #[test]
    fn test_config_default() {
        let config = EventOptimizerConfig::default();
        assert_eq!(config.max_history_size, 1000);
        assert!((config.high_drift_threshold - 0.10).abs() < f32::EPSILON);
        assert!((config.low_performance_threshold - 0.50).abs() < f32::EPSILON);
        assert!((config.memory_pressure_threshold - 85.0).abs() < f32::EPSILON);
        assert!(config.auto_optimize);
        println!("[PASS] test_config_default");
    }

    #[test]
    fn test_config_serialization() {
        let config = EventOptimizerConfig::default();
        let json = serde_json::to_string(&config).expect("serialize failed");
        let deserialized: EventOptimizerConfig =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.max_history_size, config.max_history_size);
        assert!(
            (deserialized.high_drift_threshold - config.high_drift_threshold).abs() < f32::EPSILON
        );
        println!("[PASS] test_config_serialization");
    }

    // OptimizationTrigger tests
    #[test]
    fn test_trigger_high_drift() {
        let trigger = OptimizationTrigger::HighDrift { severity: 0.15 };
        assert_eq!(trigger.trigger_type_name(), "high_drift");
        assert!(!trigger.is_critical());
        assert_eq!(trigger.base_priority(), 8);
        println!("[PASS] test_trigger_high_drift");
    }

    #[test]
    fn test_trigger_critical_high_drift() {
        let trigger = OptimizationTrigger::HighDrift { severity: 0.25 };
        assert!(trigger.is_critical());
        assert_eq!(trigger.base_priority(), 10);
        println!("[PASS] test_trigger_critical_high_drift");
    }

    #[test]
    fn test_trigger_low_performance() {
        let trigger = OptimizationTrigger::LowPerformance {
            metric: "retrieval_latency".to_string(),
            value: 0.35,
        };
        assert_eq!(trigger.trigger_type_name(), "low_performance");
        assert!(!trigger.is_critical());
        assert_eq!(trigger.base_priority(), 5);
        println!("[PASS] test_trigger_low_performance");
    }

    #[test]
    fn test_trigger_memory_pressure() {
        let trigger = OptimizationTrigger::MemoryPressure {
            usage_percent: 90.0,
        };
        assert_eq!(trigger.trigger_type_name(), "memory_pressure");
        assert!(!trigger.is_critical());
        assert_eq!(trigger.base_priority(), 8);
        println!("[PASS] test_trigger_memory_pressure");
    }

    #[test]
    fn test_trigger_critical_memory_pressure() {
        let trigger = OptimizationTrigger::MemoryPressure {
            usage_percent: 96.0,
        };
        assert!(trigger.is_critical());
        assert_eq!(trigger.base_priority(), 10);
        println!("[PASS] test_trigger_critical_memory_pressure");
    }

    #[test]
    fn test_trigger_scheduled_maintenance() {
        let trigger = OptimizationTrigger::ScheduledMaintenance;
        assert_eq!(trigger.trigger_type_name(), "scheduled_maintenance");
        assert!(!trigger.is_critical());
        assert_eq!(trigger.base_priority(), 3);
        println!("[PASS] test_trigger_scheduled_maintenance");
    }

    #[test]
    fn test_trigger_user_triggered() {
        let trigger = OptimizationTrigger::UserTriggered {
            reason: "Manual cleanup".to_string(),
        };
        assert_eq!(trigger.trigger_type_name(), "user_triggered");
        assert!(!trigger.is_critical());
        assert_eq!(trigger.base_priority(), 9);
        println!("[PASS] test_trigger_user_triggered");
    }

    #[test]
    fn test_trigger_serialization() {
        let triggers = [
            OptimizationTrigger::HighDrift { severity: 0.15 },
            OptimizationTrigger::LowPerformance {
                metric: "test".to_string(),
                value: 0.4,
            },
            OptimizationTrigger::MemoryPressure {
                usage_percent: 88.0,
            },
            OptimizationTrigger::ScheduledMaintenance,
            OptimizationTrigger::UserTriggered {
                reason: "test".to_string(),
            },
        ];

        for trigger in triggers {
            let json = serde_json::to_string(&trigger).expect("serialize failed");
            let deserialized: OptimizationTrigger =
                serde_json::from_str(&json).expect("deserialize failed");
            assert_eq!(deserialized, trigger);
        }
        println!("[PASS] test_trigger_serialization");
    }

    // OptimizationAction tests
    #[test]
    fn test_action_estimated_cost() {
        assert!((OptimizationAction::ReindexMemories.estimated_cost() - 0.8).abs() < f32::EPSILON);
        assert!(
            (OptimizationAction::RecomputeAlignments.estimated_cost() - 0.9).abs() < f32::EPSILON
        );
        assert!((OptimizationAction::PruneStaleData.estimated_cost() - 0.4).abs() < f32::EPSILON);
        assert!((OptimizationAction::RebalanceWeights.estimated_cost() - 0.3).abs() < f32::EPSILON);
        assert!((OptimizationAction::CompactStorage.estimated_cost() - 0.7).abs() < f32::EPSILON);
        println!("[PASS] test_action_estimated_cost");
    }

    #[test]
    fn test_action_estimated_benefit() {
        assert!(
            (OptimizationAction::ReindexMemories.estimated_benefit() - 0.7).abs() < f32::EPSILON
        );
        assert!(
            (OptimizationAction::RecomputeAlignments.estimated_benefit() - 0.8).abs()
                < f32::EPSILON
        );
        assert!(
            (OptimizationAction::PruneStaleData.estimated_benefit() - 0.5).abs() < f32::EPSILON
        );
        assert!(
            (OptimizationAction::RebalanceWeights.estimated_benefit() - 0.6).abs() < f32::EPSILON
        );
        assert!(
            (OptimizationAction::CompactStorage.estimated_benefit() - 0.4).abs() < f32::EPSILON
        );
        println!("[PASS] test_action_estimated_benefit");
    }

    #[test]
    fn test_action_description() {
        assert!(!OptimizationAction::ReindexMemories.description().is_empty());
        assert!(!OptimizationAction::RecomputeAlignments
            .description()
            .is_empty());
        assert!(!OptimizationAction::PruneStaleData.description().is_empty());
        assert!(!OptimizationAction::RebalanceWeights
            .description()
            .is_empty());
        assert!(!OptimizationAction::CompactStorage.description().is_empty());
        println!("[PASS] test_action_description");
    }

    #[test]
    fn test_action_recommended_for_high_drift() {
        let trigger = OptimizationTrigger::HighDrift { severity: 0.25 };
        let actions = OptimizationAction::recommended_for_trigger(&trigger);
        assert!(actions.contains(&OptimizationAction::RecomputeAlignments));
        assert!(actions.contains(&OptimizationAction::RebalanceWeights));
        assert!(actions.contains(&OptimizationAction::ReindexMemories));
        println!("[PASS] test_action_recommended_for_high_drift");
    }

    #[test]
    fn test_action_recommended_for_memory_pressure() {
        let trigger = OptimizationTrigger::MemoryPressure {
            usage_percent: 96.0,
        };
        let actions = OptimizationAction::recommended_for_trigger(&trigger);
        assert!(actions.contains(&OptimizationAction::PruneStaleData));
        assert!(actions.contains(&OptimizationAction::CompactStorage));
        println!("[PASS] test_action_recommended_for_memory_pressure");
    }

    #[test]
    fn test_action_recommended_for_low_performance_retrieval() {
        let trigger = OptimizationTrigger::LowPerformance {
            metric: "retrieval_latency".to_string(),
            value: 0.3,
        };
        let actions = OptimizationAction::recommended_for_trigger(&trigger);
        assert!(actions.contains(&OptimizationAction::ReindexMemories));
        println!("[PASS] test_action_recommended_for_low_performance_retrieval");
    }

    #[test]
    fn test_action_serialization() {
        let actions = [
            OptimizationAction::ReindexMemories,
            OptimizationAction::RecomputeAlignments,
            OptimizationAction::PruneStaleData,
            OptimizationAction::RebalanceWeights,
            OptimizationAction::CompactStorage,
        ];

        for action in actions {
            let json = serde_json::to_string(&action).expect("serialize failed");
            let deserialized: OptimizationAction =
                serde_json::from_str(&json).expect("deserialize failed");
            assert_eq!(deserialized, action);
        }
        println!("[PASS] test_action_serialization");
    }

    // SystemMetrics tests
    #[test]
    fn test_system_metrics_default() {
        let metrics = SystemMetrics::default();
        assert!((metrics.avg_alignment - 0.0).abs() < f32::EPSILON);
        assert!((metrics.avg_latency_ms - 0.0).abs() < f32::EPSILON);
        assert!((metrics.memory_usage_percent - 0.0).abs() < f32::EPSILON);
        assert_eq!(metrics.memory_count, 0);
        assert!((metrics.index_health - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_system_metrics_default");
    }

    #[test]
    fn test_system_metrics_new() {
        let metrics = SystemMetrics::new(0.75, 50.0, 60.0, 1000, 0.9);
        assert!((metrics.avg_alignment - 0.75).abs() < f32::EPSILON);
        assert!((metrics.avg_latency_ms - 50.0).abs() < f32::EPSILON);
        assert!((metrics.memory_usage_percent - 60.0).abs() < f32::EPSILON);
        assert_eq!(metrics.memory_count, 1000);
        assert!((metrics.index_health - 0.9).abs() < f32::EPSILON);
        println!("[PASS] test_system_metrics_new");
    }

    #[test]
    fn test_system_metrics_calculate_improvement() {
        let before = SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8);
        let after = SystemMetrics::new(0.80, 50.0, 70.0, 1000, 0.9);

        let improvement = before.calculate_improvement(&after);
        assert!(improvement > 0.0); // Should be positive (improvement)
        println!("[PASS] test_system_metrics_calculate_improvement");
    }

    #[test]
    fn test_system_metrics_calculate_degradation() {
        let before = SystemMetrics::new(0.80, 50.0, 60.0, 1000, 0.9);
        let after = SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.7);

        let improvement = before.calculate_improvement(&after);
        assert!(improvement < 0.0); // Should be negative (degradation)
        println!("[PASS] test_system_metrics_calculate_degradation");
    }

    #[test]
    fn test_system_metrics_serialization() {
        let metrics = SystemMetrics::new(0.75, 50.0, 60.0, 1000, 0.9);
        let json = serde_json::to_string(&metrics).expect("serialize failed");
        let deserialized: SystemMetrics = serde_json::from_str(&json).expect("deserialize failed");
        assert!((deserialized.avg_alignment - metrics.avg_alignment).abs() < f32::EPSILON);
        assert!((deserialized.avg_latency_ms - metrics.avg_latency_ms).abs() < f32::EPSILON);
        println!("[PASS] test_system_metrics_serialization");
    }

    // OptimizationPlan tests
    #[test]
    fn test_optimization_plan_new() {
        let trigger = OptimizationTrigger::HighDrift { severity: 0.15 };
        let actions = vec![
            OptimizationAction::RecomputeAlignments,
            OptimizationAction::RebalanceWeights,
        ];
        let plan = OptimizationPlan::new(trigger.clone(), actions.clone());

        assert_eq!(plan.trigger, trigger);
        assert_eq!(plan.actions.len(), 2);
        assert_eq!(plan.priority, 8);
        assert!(plan.estimated_impact > 0.0);
        println!("[PASS] test_optimization_plan_new");
    }

    #[test]
    fn test_optimization_plan_with_priority() {
        let trigger = OptimizationTrigger::ScheduledMaintenance;
        let plan = OptimizationPlan::new(trigger, vec![OptimizationAction::PruneStaleData])
            .with_priority(7);

        assert_eq!(plan.priority, 7);
        println!("[PASS] test_optimization_plan_with_priority");
    }

    #[test]
    fn test_optimization_plan_priority_clamped() {
        let trigger = OptimizationTrigger::ScheduledMaintenance;
        let plan = OptimizationPlan::new(trigger, vec![OptimizationAction::PruneStaleData])
            .with_priority(15); // Should be clamped to 10

        assert_eq!(plan.priority, 10);
        println!("[PASS] test_optimization_plan_priority_clamped");
    }

    #[test]
    fn test_optimization_plan_is_empty() {
        let trigger = OptimizationTrigger::ScheduledMaintenance;
        let empty_plan = OptimizationPlan::new(trigger.clone(), vec![]);
        assert!(empty_plan.is_empty());

        let non_empty_plan =
            OptimizationPlan::new(trigger, vec![OptimizationAction::PruneStaleData]);
        assert!(!non_empty_plan.is_empty());
        println!("[PASS] test_optimization_plan_is_empty");
    }

    #[test]
    fn test_optimization_plan_total_cost() {
        let trigger = OptimizationTrigger::ScheduledMaintenance;
        let plan = OptimizationPlan::new(
            trigger,
            vec![
                OptimizationAction::PruneStaleData,
                OptimizationAction::CompactStorage,
            ],
        );

        let expected_cost = 0.4 + 0.7; // PruneStaleData + CompactStorage
        assert!((plan.total_cost() - expected_cost).abs() < f32::EPSILON);
        println!("[PASS] test_optimization_plan_total_cost");
    }

    #[test]
    fn test_optimization_plan_serialization() {
        let trigger = OptimizationTrigger::HighDrift { severity: 0.15 };
        let plan = OptimizationPlan::new(
            trigger,
            vec![
                OptimizationAction::RecomputeAlignments,
                OptimizationAction::RebalanceWeights,
            ],
        );

        let json = serde_json::to_string(&plan).expect("serialize failed");
        let deserialized: OptimizationPlan =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(deserialized.trigger, plan.trigger);
        assert_eq!(deserialized.actions.len(), plan.actions.len());
        println!("[PASS] test_optimization_plan_serialization");
    }

    // EventOptimizer tests
    #[test]
    fn test_event_optimizer_new() {
        let optimizer = EventOptimizer::new();
        assert_eq!(optimizer.optimization_count(), 0);
        assert!(optimizer.event_history().is_empty());
        println!("[PASS] test_event_optimizer_new");
    }

    #[test]
    fn test_event_optimizer_with_config() {
        let config = EventOptimizerConfig {
            max_history_size: 500,
            high_drift_threshold: 0.15,
            low_performance_threshold: 0.40,
            memory_pressure_threshold: 80.0,
            auto_optimize: false,
        };
        let optimizer = EventOptimizer::with_config(config.clone());
        assert_eq!(optimizer.config().max_history_size, 500);
        assert!(!optimizer.config().auto_optimize);
        println!("[PASS] test_event_optimizer_with_config");
    }

    #[test]
    fn test_event_optimizer_on_event() {
        let mut optimizer = EventOptimizer::new();
        let trigger = OptimizationTrigger::HighDrift { severity: 0.15 };

        let plan = optimizer.on_event(trigger.clone());

        assert_eq!(plan.trigger, trigger);
        assert!(!plan.is_empty());
        println!("[PASS] test_event_optimizer_on_event");
    }

    #[test]
    fn test_event_optimizer_plan_optimization() {
        let optimizer = EventOptimizer::new();
        let trigger = OptimizationTrigger::MemoryPressure {
            usage_percent: 90.0,
        };

        let plan = optimizer.plan_optimization(&trigger);

        assert!(plan.actions.contains(&OptimizationAction::PruneStaleData));
        assert!(plan.actions.contains(&OptimizationAction::CompactStorage));
        println!("[PASS] test_event_optimizer_plan_optimization");
    }

    #[test]
    fn test_event_optimizer_execute_plan() {
        let mut optimizer = EventOptimizer::new();
        optimizer.set_current_metrics(SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8));

        let trigger = OptimizationTrigger::HighDrift { severity: 0.15 };
        let plan = optimizer.plan_optimization(&trigger);
        let event = optimizer.execute_plan(&plan);

        assert!(event.success);
        assert_eq!(event.event_type, "high_drift");
        assert!(!event.actions_executed.is_empty());
        assert_eq!(optimizer.optimization_count(), 1);
        println!("[PASS] test_event_optimizer_execute_plan");
    }

    #[test]
    fn test_event_optimizer_estimate_impact() {
        let optimizer = EventOptimizer::new();
        let actions = vec![
            OptimizationAction::RecomputeAlignments,
            OptimizationAction::RebalanceWeights,
        ];

        let impact = optimizer.estimate_impact(&actions);
        assert!(impact > 0.0);
        assert!(impact <= 1.0);
        println!("[PASS] test_event_optimizer_estimate_impact");
    }

    #[test]
    fn test_event_optimizer_estimate_impact_empty() {
        let optimizer = EventOptimizer::new();
        let impact = optimizer.estimate_impact(&[]);
        assert!((impact - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_event_optimizer_estimate_impact_empty");
    }

    #[test]
    fn test_event_optimizer_prioritize_actions() {
        let mut optimizer = EventOptimizer::new();
        optimizer.set_current_metrics(SystemMetrics::new(0.70, 100.0, 85.0, 1000, 0.8));

        let mut actions = vec![
            OptimizationAction::ReindexMemories,
            OptimizationAction::RecomputeAlignments,
            OptimizationAction::PruneStaleData,
            OptimizationAction::CompactStorage,
        ];

        optimizer.prioritize_actions(&mut actions);

        // With high memory usage, PruneStaleData and CompactStorage should be prioritized
        assert!(
            actions[0] == OptimizationAction::PruneStaleData
                || actions[0] == OptimizationAction::CompactStorage
        );
        println!("[PASS] test_event_optimizer_prioritize_actions");
    }

    #[test]
    fn test_event_optimizer_prioritize_actions_index_repair() {
        let mut optimizer = EventOptimizer::new();
        optimizer.set_current_metrics(SystemMetrics::new(0.70, 100.0, 50.0, 1000, 0.5));

        let mut actions = vec![
            OptimizationAction::PruneStaleData,
            OptimizationAction::ReindexMemories,
            OptimizationAction::RebalanceWeights,
        ];

        optimizer.prioritize_actions(&mut actions);

        // With low index health, ReindexMemories should be prioritized
        assert_eq!(actions[0], OptimizationAction::ReindexMemories);
        println!("[PASS] test_event_optimizer_prioritize_actions_index_repair");
    }

    #[test]
    fn test_event_optimizer_record_event() {
        let mut optimizer = EventOptimizer::new();

        let record = OptimizationEventRecord {
            event_type: "test".to_string(),
            timestamp: Utc::now(),
            metrics_before: SystemMetrics::default(),
            metrics_after: SystemMetrics::default(),
            success: true,
            actions_executed: vec![OptimizationAction::PruneStaleData],
            duration_ms: 100,
            error: None,
        };

        optimizer.record_event(record);

        assert_eq!(optimizer.event_history().len(), 1);
        assert_eq!(optimizer.optimization_count(), 1);
        println!("[PASS] test_event_optimizer_record_event");
    }

    #[test]
    fn test_event_optimizer_record_event_failed() {
        let mut optimizer = EventOptimizer::new();

        let record = OptimizationEventRecord {
            event_type: "test".to_string(),
            timestamp: Utc::now(),
            metrics_before: SystemMetrics::default(),
            metrics_after: SystemMetrics::default(),
            success: false,
            actions_executed: vec![],
            duration_ms: 100,
            error: Some("Test error".to_string()),
        };

        optimizer.record_event(record);

        assert_eq!(optimizer.event_history().len(), 1);
        assert_eq!(optimizer.optimization_count(), 0); // Failed events don't count
        println!("[PASS] test_event_optimizer_record_event_failed");
    }

    #[test]
    fn test_event_optimizer_history_trimming() {
        let config = EventOptimizerConfig {
            max_history_size: 5,
            ..Default::default()
        };
        let mut optimizer = EventOptimizer::with_config(config);

        for i in 0..10 {
            let record = OptimizationEventRecord {
                event_type: format!("test_{}", i),
                timestamp: Utc::now(),
                metrics_before: SystemMetrics::default(),
                metrics_after: SystemMetrics::default(),
                success: true,
                actions_executed: vec![],
                duration_ms: 100,
                error: None,
            };
            optimizer.record_event(record);
        }

        assert_eq!(optimizer.event_history().len(), 5);
        println!("[PASS] test_event_optimizer_history_trimming");
    }

    #[test]
    fn test_event_optimizer_average_improvement() {
        let mut optimizer = EventOptimizer::new();

        // Record some events with improvements
        for _ in 0..5 {
            optimizer.set_current_metrics(SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8));
            let trigger = OptimizationTrigger::HighDrift { severity: 0.12 };
            let plan = optimizer.plan_optimization(&trigger);
            optimizer.execute_plan(&plan);
        }

        assert!(optimizer.average_improvement() >= 0.0);
        println!("[PASS] test_event_optimizer_average_improvement");
    }

    #[test]
    fn test_event_optimizer_average_improvement_no_events() {
        let optimizer = EventOptimizer::new();
        assert!((optimizer.average_improvement() - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_event_optimizer_average_improvement_no_events");
    }

    #[test]
    fn test_event_optimizer_set_current_metrics() {
        let mut optimizer = EventOptimizer::new();
        let metrics = SystemMetrics::new(0.85, 25.0, 45.0, 5000, 0.95);

        optimizer.set_current_metrics(metrics.clone());

        assert!((optimizer.current_metrics().avg_alignment - 0.85).abs() < f32::EPSILON);
        assert!((optimizer.current_metrics().avg_latency_ms - 25.0).abs() < f32::EPSILON);
        println!("[PASS] test_event_optimizer_set_current_metrics");
    }

    // OptimizationEventRecord tests
    #[test]
    fn test_optimization_event_record_improvement() {
        let record = OptimizationEventRecord {
            event_type: "test".to_string(),
            timestamp: Utc::now(),
            metrics_before: SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8),
            metrics_after: SystemMetrics::new(0.80, 50.0, 70.0, 1000, 0.9),
            success: true,
            actions_executed: vec![OptimizationAction::RecomputeAlignments],
            duration_ms: 500,
            error: None,
        };

        assert!(record.improvement() > 0.0);
        println!("[PASS] test_optimization_event_record_improvement");
    }

    #[test]
    fn test_optimization_event_record_improvement_failed() {
        let record = OptimizationEventRecord {
            event_type: "test".to_string(),
            timestamp: Utc::now(),
            metrics_before: SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8),
            metrics_after: SystemMetrics::new(0.80, 50.0, 70.0, 1000, 0.9),
            success: false, // Failed
            actions_executed: vec![],
            duration_ms: 500,
            error: Some("Failed".to_string()),
        };

        assert!((record.improvement() - 0.0).abs() < f32::EPSILON);
        println!("[PASS] test_optimization_event_record_improvement_failed");
    }

    #[test]
    fn test_optimization_event_record_serialization() {
        let record = OptimizationEventRecord {
            event_type: "high_drift".to_string(),
            timestamp: Utc::now(),
            metrics_before: SystemMetrics::new(0.70, 100.0, 80.0, 1000, 0.8),
            metrics_after: SystemMetrics::new(0.80, 50.0, 70.0, 1000, 0.9),
            success: true,
            actions_executed: vec![
                OptimizationAction::RecomputeAlignments,
                OptimizationAction::RebalanceWeights,
            ],
            duration_ms: 500,
            error: None,
        };

        let json = serde_json::to_string(&record).expect("serialize failed");
        let deserialized: OptimizationEventRecord =
            serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(deserialized.event_type, record.event_type);
        assert_eq!(deserialized.success, record.success);
        assert_eq!(
            deserialized.actions_executed.len(),
            record.actions_executed.len()
        );
        println!("[PASS] test_optimization_event_record_serialization");
    }

    // Integration tests
    #[test]
    fn test_full_optimization_cycle() {
        let mut optimizer = EventOptimizer::new();

        // Set initial metrics with some issues
        optimizer.set_current_metrics(SystemMetrics::new(0.65, 150.0, 85.0, 10000, 0.7));

        // Trigger high drift event
        let trigger = OptimizationTrigger::HighDrift { severity: 0.18 };
        let plan = optimizer.on_event(trigger);

        assert!(!plan.is_empty());
        assert!(plan.priority >= 8);

        // Execute the plan
        let event = optimizer.execute_plan(&plan);

        assert!(event.success);
        assert!(!event.actions_executed.is_empty());
        // duration_ms may be 0 in fast tests, so just check it's not negative (which is impossible for u64)
        // The key assertion is that execution completed successfully

        // Verify metrics improved
        assert!(optimizer.current_metrics().avg_alignment > 0.65);
        println!("[PASS] test_full_optimization_cycle");
    }

    #[test]
    fn test_multiple_triggers() {
        let mut optimizer = EventOptimizer::new();
        optimizer.set_current_metrics(SystemMetrics::new(0.60, 200.0, 90.0, 15000, 0.6));

        // Execute multiple optimization cycles
        let triggers = [
            OptimizationTrigger::HighDrift { severity: 0.15 },
            OptimizationTrigger::MemoryPressure {
                usage_percent: 92.0,
            },
            OptimizationTrigger::LowPerformance {
                metric: "search_speed".to_string(),
                value: 0.35,
            },
        ];

        for trigger in triggers {
            let plan = optimizer.on_event(trigger);
            optimizer.execute_plan(&plan);
        }

        assert_eq!(optimizer.optimization_count(), 3);
        assert_eq!(optimizer.event_history().len(), 3);
        println!("[PASS] test_multiple_triggers");
    }

    #[test]
    fn test_trigger_equality() {
        let t1 = OptimizationTrigger::HighDrift { severity: 0.15 };
        let t2 = OptimizationTrigger::HighDrift { severity: 0.15 };
        let t3 = OptimizationTrigger::HighDrift { severity: 0.20 };

        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
        assert_ne!(t1, OptimizationTrigger::ScheduledMaintenance);
        println!("[PASS] test_trigger_equality");
    }

    #[test]
    fn test_action_equality_and_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(OptimizationAction::ReindexMemories);
        set.insert(OptimizationAction::RecomputeAlignments);
        set.insert(OptimizationAction::ReindexMemories); // Duplicate

        assert_eq!(set.len(), 2);
        assert!(set.contains(&OptimizationAction::ReindexMemories));
        assert!(set.contains(&OptimizationAction::RecomputeAlignments));
        assert!(!set.contains(&OptimizationAction::PruneStaleData));
        println!("[PASS] test_action_equality_and_hash");
    }
}
