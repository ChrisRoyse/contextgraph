//! Type definitions for core handlers.
//!
//! TASK-S005: Prediction types for meta-UTL tracking.
//! TASK-METAUTL-P0-001: Domain and meta-learning event types.
//! TASK-METAUTL-P0-004: Added serializable timestamp to MetaLearningEvent.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Prediction type for tracking
/// TASK-S005: Used to distinguish storage vs retrieval predictions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PredictionType {
    Storage,
    Retrieval,
}

/// Domain enum for domain-specific accuracy tracking.
/// TASK-METAUTL-P0-001: Enables per-domain meta-learning optimization.
/// TASK-METAUTL-P0-004: Added Serialize/Deserialize for event log persistence.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Domain {
    /// Source code, programming-related content
    Code,
    /// Medical and healthcare content
    Medical,
    /// Legal documents and regulations
    Legal,
    /// Creative writing, art, design
    Creative,
    /// Research papers, scientific content
    Research,
    /// General purpose, unclassified
    #[default]
    General,
}

/// Per-domain accuracy tracking with rolling window.
/// TASK-METAUTL-P1-001: Enables domain-specific lambda optimization.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DomainAccuracyTracker {
    /// Rolling window of accuracy samples (100 samples)
    accuracy_history: [f32; 100],
    /// Current index in rolling window
    history_index: usize,
    /// Number of samples recorded (up to 100)
    sample_count: usize,
    /// Total predictions in this domain
    pub total_predictions: usize,
    /// Consecutive low accuracy count for this domain
    pub consecutive_low_count: usize,
}

impl Default for DomainAccuracyTracker {
    fn default() -> Self {
        Self {
            accuracy_history: [0.0; 100],
            history_index: 0,
            sample_count: 0,
            total_predictions: 0,
            consecutive_low_count: 0,
        }
    }
}

#[allow(dead_code)]
impl DomainAccuracyTracker {
    /// Record an accuracy value in the rolling window.
    pub fn record(&mut self, accuracy: f32) {
        let clamped = accuracy.clamp(0.0, 1.0);
        self.accuracy_history[self.history_index] = clamped;
        self.history_index = (self.history_index + 1) % 100;
        if self.sample_count < 100 {
            self.sample_count += 1;
        }
        self.total_predictions += 1;
    }

    /// Get average accuracy from recorded samples.
    /// Returns None if no samples have been recorded.
    pub fn average(&self) -> Option<f32> {
        if self.sample_count == 0 {
            return None;
        }
        let sum: f32 = self.accuracy_history[..self.sample_count].iter().sum();
        Some(sum / self.sample_count as f32)
    }

    /// Get number of samples recorded.
    pub fn sample_count(&self) -> usize {
        self.sample_count
    }
}

/// Meta-learning event types for logging and auditing.
/// TASK-METAUTL-P0-001: Used to track significant meta-learning state changes.
/// TASK-METAUTL-P0-004: Added Serialize/Deserialize for event log persistence.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetaLearningEventType {
    /// Lambda weight adjustment occurred
    LambdaAdjustment,
    /// Bayesian optimization escalation triggered
    BayesianEscalation,
    /// Accuracy dropped below threshold
    AccuracyAlert,
    /// Recovery from low accuracy period
    AccuracyRecovery,
    /// Weight clamping applied (exceeded bounds)
    WeightClamped,
}

/// Helper function for Instant default (serde skip requires a function)
fn instant_now() -> Instant {
    Instant::now()
}

/// Meta-learning event for logging significant state changes.
/// TASK-METAUTL-P0-001: Provides audit trail for meta-learning behavior.
/// TASK-METAUTL-P0-004: Added serializable created_at timestamp and domain field.
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaLearningEvent {
    /// Type of event
    pub event_type: MetaLearningEventType,
    /// When the event occurred (monotonic, not serializable).
    /// Uses Instant::now() as default during deserialization since Instant
    /// cannot be meaningfully restored from serialization.
    #[serde(skip, default = "instant_now")]
    pub timestamp: Instant,
    /// Serializable wall-clock timestamp for persistence and queries
    /// TASK-METAUTL-P0-004: Required for time-range queries and JSON serialization
    pub created_at: DateTime<Utc>,
    /// Domain this event belongs to (for domain-specific filtering)
    /// TASK-METAUTL-P0-004: Required for domain-based event queries
    #[serde(default)]
    pub domain: Domain,
    /// Embedder index affected (if applicable)
    pub embedder_index: Option<usize>,
    /// Previous value (if applicable)
    pub previous_value: Option<f32>,
    /// New value (if applicable)
    pub new_value: Option<f32>,
    /// Optional description
    pub description: Option<String>,
    /// Accuracy at the time of the event (for stats computation)
    /// TASK-METAUTL-P0-004: Used for EventLogStats.avg_accuracy
    pub accuracy: Option<f32>,
}

#[allow(dead_code)]
impl MetaLearningEvent {
    /// Create a lambda adjustment event.
    pub fn lambda_adjustment(embedder_idx: usize, previous: f32, new: f32) -> Self {
        Self {
            event_type: MetaLearningEventType::LambdaAdjustment,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain: Domain::default(),
            embedder_index: Some(embedder_idx),
            previous_value: Some(previous),
            new_value: Some(new),
            description: None,
            accuracy: None,
        }
    }

    /// Create a lambda adjustment event with domain.
    pub fn lambda_adjustment_with_domain(
        embedder_idx: usize,
        previous: f32,
        new: f32,
        domain: Domain,
    ) -> Self {
        Self {
            event_type: MetaLearningEventType::LambdaAdjustment,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain,
            embedder_index: Some(embedder_idx),
            previous_value: Some(previous),
            new_value: Some(new),
            description: None,
            accuracy: None,
        }
    }

    /// Create a bayesian escalation event.
    pub fn bayesian_escalation(consecutive_low: usize) -> Self {
        Self {
            event_type: MetaLearningEventType::BayesianEscalation,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain: Domain::default(),
            embedder_index: None,
            previous_value: None,
            new_value: Some(consecutive_low as f32),
            description: Some(format!(
                "Escalation triggered after {} consecutive low accuracy cycles",
                consecutive_low
            )),
            accuracy: None,
        }
    }

    /// Create a weight clamped event.
    pub fn weight_clamped(embedder_idx: usize, original: f32, clamped: f32) -> Self {
        Self {
            event_type: MetaLearningEventType::WeightClamped,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain: Domain::default(),
            embedder_index: Some(embedder_idx),
            previous_value: Some(original),
            new_value: Some(clamped),
            description: None,
            accuracy: None,
        }
    }

    /// Create an accuracy alert event.
    /// TASK-METAUTL-P0-004: For logging when accuracy drops below threshold.
    pub fn accuracy_alert(current_accuracy: f32, threshold: f32) -> Self {
        Self {
            event_type: MetaLearningEventType::AccuracyAlert,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain: Domain::default(),
            embedder_index: None,
            previous_value: Some(threshold),
            new_value: Some(current_accuracy),
            description: Some(format!(
                "Accuracy {} dropped below threshold {}",
                current_accuracy, threshold
            )),
            accuracy: Some(current_accuracy),
        }
    }

    /// Create an accuracy recovery event.
    /// TASK-METAUTL-P0-004: For logging when accuracy recovers above threshold.
    pub fn accuracy_recovery(previous_accuracy: f32, current_accuracy: f32) -> Self {
        Self {
            event_type: MetaLearningEventType::AccuracyRecovery,
            timestamp: Instant::now(),
            created_at: Utc::now(),
            domain: Domain::default(),
            embedder_index: None,
            previous_value: Some(previous_accuracy),
            new_value: Some(current_accuracy),
            description: Some(format!(
                "Accuracy recovered from {} to {}",
                previous_accuracy, current_accuracy
            )),
            accuracy: Some(current_accuracy),
        }
    }

    /// Set the domain for this event.
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = domain;
        self
    }

    /// Set the accuracy for this event.
    pub fn with_accuracy(mut self, accuracy: f32) -> Self {
        self.accuracy = Some(accuracy);
        self
    }
}

/// Configuration for self-correction behavior.
/// TASK-METAUTL-P0-001: Constitution-defined parameters for meta-learning.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SelfCorrectionConfig {
    /// Whether self-correction is enabled
    pub enabled: bool,
    /// Prediction error threshold (constitution: 0.2)
    pub error_threshold: f32,
    /// Maximum consecutive failures before escalation (constitution: 10)
    pub max_consecutive_failures: usize,
    /// Accuracy threshold below which is considered "low" (constitution: 0.7)
    pub low_accuracy_threshold: f32,
    /// Minimum weight bound (constitution NORTH-016: 0.05)
    /// Note: 13 × 0.05 = 0.65 < 1.0, so sum=1.0 is achievable
    pub min_weight: f32,
    /// Maximum weight bound (constitution: 0.9)
    pub max_weight: f32,
    /// Escalation strategy
    pub escalation_strategy: String,
    /// Base learning rate for lambda adjustment (TASK-METAUTL-P0-002)
    /// Modulated by ACh level: alpha = base_alpha * (1.0 + ach_normalized)
    pub base_alpha: f32,
}

/// Record of a single lambda adjustment.
/// TASK-METAUTL-P0-002: Tracks adjustment details for auditing and rollback.
/// TASK-METAUTL-P0-005: Added Serialize/Deserialize for MCP handlers.
#[allow(dead_code)] // Will be used in TASK-METAUTL-P0-005/006
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LambdaAdjustment {
    /// Change in surprise weight (lambda_s)
    pub delta_lambda_s: f32,
    /// Change in coherence weight (lambda_c)
    pub delta_lambda_c: f32,
    /// Learning rate used for this adjustment
    pub alpha: f32,
    /// Prediction error that triggered this adjustment
    pub trigger_error: f32,
}

impl Default for SelfCorrectionConfig {
    /// Creates config with constitution-mandated defaults.
    ///
    /// From docs2/constitution.yaml:
    /// - threshold: 0.2
    /// - max_consecutive_failures: 10
    /// - escalation_strategy: "bayesian_optimization"
    /// - NORTH-016_WeightAdjuster: min=0.05, max_delta=0.10
    /// - base_alpha: 0.05 (SPEC-METAUTL-001)
    fn default() -> Self {
        Self {
            enabled: true,
            error_threshold: 0.2,
            max_consecutive_failures: 10,
            low_accuracy_threshold: 0.7,
            min_weight: 0.05, // NORTH-016: min=0.05 (13×0.05=0.65 < 1.0, sum is achievable)
            max_weight: 0.9,
            escalation_strategy: "bayesian_optimization".to_string(),
            base_alpha: 0.05, // TASK-METAUTL-P0-002: base learning rate for lambda adjustment
        }
    }
}

/// Stored prediction for validation
/// TASK-S005: Stores predicted values for later validation against actual outcomes.
#[derive(Clone, Debug)]
pub struct StoredPrediction {
    pub _created_at: Instant,
    pub prediction_type: PredictionType,
    pub predicted_values: serde_json::Value,
    #[allow(dead_code)]
    pub fingerprint_id: Uuid,
}
