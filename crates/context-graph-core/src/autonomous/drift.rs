//! Drift detection types for alignment monitoring
//!
//! This module defines types for detecting and responding to drift in
//! alignment scores over time, enabling automatic correction and alerting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Drift detection configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftConfig {
    /// Enable continuous monitoring
    pub monitoring: DriftMonitoring,

    /// Alert threshold (alignment drop to trigger alert)
    pub alert_threshold: f32, // default: 0.05

    /// Enable auto-correction
    pub auto_correct: bool,

    /// Severe drift threshold (requires user intervention)
    pub severe_threshold: f32, // default: 0.10

    /// Rolling window size in days
    pub window_days: u32, // default: 7
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            monitoring: DriftMonitoring::Continuous,
            alert_threshold: 0.05,
            auto_correct: true,
            severe_threshold: 0.10,
            window_days: 7,
        }
    }
}

/// Monitoring mode for drift detection
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DriftMonitoring {
    /// Continuous real-time monitoring
    Continuous,
    /// Periodic checks at specified interval
    Periodic { interval_hours: u32 },
    /// Manual checks only
    Manual,
}

/// Drift severity levels
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DriftSeverity {
    /// No significant drift detected
    None,
    /// Mild drift (< alert_threshold)
    Mild,
    /// Moderate drift (>= alert_threshold, < severe_threshold)
    Moderate,
    /// Severe drift (>= severe_threshold)
    Severe,
}

/// Drift trend direction
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DriftTrend {
    /// Alignment is stable
    Stable,
    /// Alignment is improving
    Improving,
    /// Alignment is declining
    Declining,
}

/// A single drift data point for history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftDataPoint {
    /// Mean alignment score at this point
    pub alignment_mean: f32,
    /// Number of new memories added
    pub new_memories_count: u32,
    /// Timestamp of this data point
    pub timestamp: DateTime<Utc>,
}

/// Current drift state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftState {
    /// Rolling mean alignment (window-based)
    pub rolling_mean: f32,

    /// Baseline alignment (established mean)
    pub baseline: f32,

    /// Current drift magnitude (baseline - rolling_mean)
    pub drift: f32,

    /// Drift severity
    pub severity: DriftSeverity,

    /// Trend direction
    pub trend: DriftTrend,

    /// Last check timestamp
    pub checked_at: DateTime<Utc>,

    /// Historical data points
    pub history: VecDeque<DriftDataPoint>,
}

impl Default for DriftState {
    fn default() -> Self {
        Self {
            rolling_mean: 0.75,
            baseline: 0.75,
            drift: 0.0,
            severity: DriftSeverity::None,
            trend: DriftTrend::Stable,
            checked_at: Utc::now(),
            history: VecDeque::with_capacity(168), // 7 days * 24 hours
        }
    }
}

impl DriftState {
    /// Create a new DriftState with a specific baseline
    pub fn with_baseline(baseline: f32) -> Self {
        Self {
            rolling_mean: baseline,
            baseline,
            drift: 0.0,
            severity: DriftSeverity::None,
            trend: DriftTrend::Stable,
            checked_at: Utc::now(),
            history: VecDeque::with_capacity(168),
        }
    }

    /// Add a new data point and update rolling statistics
    pub fn add_data_point(&mut self, mean_alignment: f32, new_memories: u32, config: &DriftConfig) {
        let point = DriftDataPoint {
            alignment_mean: mean_alignment,
            new_memories_count: new_memories,
            timestamp: Utc::now(),
        };

        self.history.push_back(point);

        // Keep only window_days worth of data (assuming hourly points)
        let max_points = (config.window_days * 24) as usize;
        while self.history.len() > max_points {
            self.history.pop_front();
        }

        self.update_rolling_mean();
        self.update_severity(config);
        self.checked_at = Utc::now();
    }

    /// Update the rolling mean from history
    fn update_rolling_mean(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let sum: f32 = self.history.iter().map(|p| p.alignment_mean).sum();
        let old_mean = self.rolling_mean;
        self.rolling_mean = sum / self.history.len() as f32;
        self.drift = self.baseline - self.rolling_mean;

        // Determine trend based on change in rolling mean
        let delta = self.rolling_mean - old_mean;
        self.trend = if delta.abs() < 0.01 {
            DriftTrend::Stable
        } else if delta > 0.0 {
            DriftTrend::Improving
        } else {
            DriftTrend::Declining
        };
    }

    /// Update severity classification based on drift magnitude
    fn update_severity(&mut self, config: &DriftConfig) {
        self.severity = if self.drift.abs() >= config.severe_threshold {
            DriftSeverity::Severe
        } else if self.drift.abs() >= config.alert_threshold {
            DriftSeverity::Moderate
        } else if self.drift.abs() > 0.01 {
            DriftSeverity::Mild
        } else {
            DriftSeverity::None
        };
    }

    /// Reset baseline to current rolling mean
    pub fn reset_baseline(&mut self) {
        self.baseline = self.rolling_mean;
        self.drift = 0.0;
        self.severity = DriftSeverity::None;
    }

    /// Check if drift requires attention (moderate or severe)
    pub fn requires_attention(&self) -> bool {
        matches!(
            self.severity,
            DriftSeverity::Moderate | DriftSeverity::Severe
        )
    }

    /// Check if drift is severe enough to require user intervention
    pub fn requires_intervention(&self) -> bool {
        matches!(self.severity, DriftSeverity::Severe)
    }

    /// Get the number of data points in history
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Get total new memories across all data points in history
    pub fn total_new_memories(&self) -> u64 {
        self.history
            .iter()
            .map(|p| p.new_memories_count as u64)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_config_default() {
        let config = DriftConfig::default();
        assert_eq!(config.monitoring, DriftMonitoring::Continuous);
        assert!((config.alert_threshold - 0.05).abs() < f32::EPSILON);
        assert!(config.auto_correct);
        assert!((config.severe_threshold - 0.10).abs() < f32::EPSILON);
        assert_eq!(config.window_days, 7);
    }

    #[test]
    fn test_drift_config_serialization() {
        let config = DriftConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: DriftConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.monitoring, config.monitoring);
        assert!((deserialized.alert_threshold - config.alert_threshold).abs() < f32::EPSILON);
        assert_eq!(deserialized.auto_correct, config.auto_correct);
    }

    #[test]
    fn test_drift_monitoring_variants() {
        let continuous = DriftMonitoring::Continuous;
        let periodic = DriftMonitoring::Periodic { interval_hours: 6 };
        let manual = DriftMonitoring::Manual;

        assert_eq!(continuous, DriftMonitoring::Continuous);
        assert_ne!(continuous, periodic);
        assert_ne!(continuous, manual);

        // Serialization
        let json = serde_json::to_string(&periodic).unwrap();
        let deserialized: DriftMonitoring = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, periodic);
    }

    #[test]
    fn test_drift_severity_equality() {
        assert_eq!(DriftSeverity::None, DriftSeverity::None);
        assert_ne!(DriftSeverity::None, DriftSeverity::Mild);
        assert_ne!(DriftSeverity::Mild, DriftSeverity::Moderate);
        assert_ne!(DriftSeverity::Moderate, DriftSeverity::Severe);
    }

    #[test]
    fn test_drift_trend_equality() {
        assert_eq!(DriftTrend::Stable, DriftTrend::Stable);
        assert_ne!(DriftTrend::Stable, DriftTrend::Improving);
        assert_ne!(DriftTrend::Improving, DriftTrend::Declining);
    }

    #[test]
    fn test_drift_state_default() {
        let state = DriftState::default();
        assert!((state.rolling_mean - 0.75).abs() < f32::EPSILON);
        assert!((state.baseline - 0.75).abs() < f32::EPSILON);
        assert!((state.drift - 0.0).abs() < f32::EPSILON);
        assert_eq!(state.severity, DriftSeverity::None);
        assert_eq!(state.trend, DriftTrend::Stable);
        assert!(state.history.is_empty());
    }

    #[test]
    fn test_drift_state_with_baseline() {
        let state = DriftState::with_baseline(0.80);
        assert!((state.rolling_mean - 0.80).abs() < f32::EPSILON);
        assert!((state.baseline - 0.80).abs() < f32::EPSILON);
        assert!((state.drift - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_drift_state_add_data_point() {
        let mut state = DriftState::default();
        let config = DriftConfig::default();

        // Add a data point
        state.add_data_point(0.70, 10, &config);

        assert_eq!(state.history.len(), 1);
        assert!((state.rolling_mean - 0.70).abs() < f32::EPSILON);
        // Drift = baseline (0.75) - rolling_mean (0.70) = 0.05
        assert!((state.drift - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn test_drift_state_rolling_mean_calculation() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();

        // Add multiple data points
        state.add_data_point(0.80, 5, &config);
        state.add_data_point(0.70, 5, &config);
        state.add_data_point(0.60, 5, &config);

        // Rolling mean should be (0.80 + 0.70 + 0.60) / 3 = 0.70
        assert!((state.rolling_mean - 0.70).abs() < f32::EPSILON);
        // Drift = 0.80 - 0.70 = 0.10
        assert!((state.drift - 0.10).abs() < f32::EPSILON);
    }

    #[test]
    fn test_drift_state_window_trimming() {
        let mut state = DriftState::default();
        let config = DriftConfig {
            window_days: 1, // Only 24 points max
            ..Default::default()
        };

        // Add 30 data points
        for i in 0..30 {
            state.add_data_point(0.70 + (i as f32 * 0.001), 1, &config);
        }

        // Should only keep 24 points
        assert_eq!(state.history.len(), 24);
    }

    #[test]
    fn test_drift_severity_classification() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();

        // No drift - alignment matches baseline
        state.add_data_point(0.80, 5, &config);
        assert_eq!(state.severity, DriftSeverity::None);

        // Mild drift (< 0.05)
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.78, 5, &config);
        assert_eq!(state.severity, DriftSeverity::Mild);

        // Moderate drift (>= 0.05, < 0.10)
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.73, 5, &config);
        assert_eq!(state.severity, DriftSeverity::Moderate);

        // Severe drift (>= 0.10)
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.68, 5, &config);
        assert_eq!(state.severity, DriftSeverity::Severe);
    }

    #[test]
    fn test_drift_trend_detection() {
        let mut state = DriftState::with_baseline(0.70);
        let config = DriftConfig::default();

        // Start with a point - from 0.70 to 0.70, no change = stable
        state.add_data_point(0.70, 5, &config);
        assert_eq!(state.trend, DriftTrend::Stable);

        // Improving trend (mean goes up from 0.70 to 0.775)
        state.add_data_point(0.85, 5, &config);
        assert_eq!(state.trend, DriftTrend::Improving);

        // Declining trend (mean goes down)
        state.add_data_point(0.50, 5, &config);
        assert_eq!(state.trend, DriftTrend::Declining);

        // Stable (small change) - need to find a value that keeps mean change < 0.01
        // Current mean = (0.70 + 0.85 + 0.50) / 3 = 0.6833
        // We want new mean ~= old mean, so 4th point should give ~same mean
        // (0.70 + 0.85 + 0.50 + x) / 4 ~= 0.6833 => x ~= 0.6833
        state.add_data_point(0.69, 5, &config);
        assert_eq!(state.trend, DriftTrend::Stable);
    }

    #[test]
    fn test_drift_state_reset_baseline() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();

        // Create drift
        state.add_data_point(0.65, 5, &config);
        assert!(state.drift.abs() > 0.1);
        assert_eq!(state.severity, DriftSeverity::Severe);

        // Reset baseline
        state.reset_baseline();
        assert!((state.baseline - state.rolling_mean).abs() < f32::EPSILON);
        assert!((state.drift - 0.0).abs() < f32::EPSILON);
        assert_eq!(state.severity, DriftSeverity::None);
    }

    #[test]
    fn test_drift_state_requires_attention() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();

        // No attention needed
        state.add_data_point(0.80, 5, &config);
        assert!(!state.requires_attention());

        // Moderate - needs attention
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.73, 5, &config);
        assert!(state.requires_attention());

        // Severe - needs attention
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.65, 5, &config);
        assert!(state.requires_attention());
    }

    #[test]
    fn test_drift_state_requires_intervention() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();

        // Moderate - no intervention
        state.add_data_point(0.73, 5, &config);
        assert!(!state.requires_intervention());

        // Severe - requires intervention
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.65, 5, &config);
        assert!(state.requires_intervention());
    }

    #[test]
    fn test_drift_state_history_helpers() {
        let mut state = DriftState::default();
        let config = DriftConfig::default();

        assert_eq!(state.history_len(), 0);
        assert_eq!(state.total_new_memories(), 0);

        state.add_data_point(0.70, 10, &config);
        state.add_data_point(0.72, 15, &config);
        state.add_data_point(0.74, 20, &config);

        assert_eq!(state.history_len(), 3);
        assert_eq!(state.total_new_memories(), 45);
    }

    #[test]
    fn test_drift_data_point_serialization() {
        let point = DriftDataPoint {
            alignment_mean: 0.75,
            new_memories_count: 42,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&point).expect("serialize");
        let deserialized: DriftDataPoint = serde_json::from_str(&json).expect("deserialize");

        assert!((deserialized.alignment_mean - 0.75).abs() < f32::EPSILON);
        assert_eq!(deserialized.new_memories_count, 42);
    }

    #[test]
    fn test_drift_state_serialization() {
        let mut state = DriftState::with_baseline(0.80);
        let config = DriftConfig::default();
        state.add_data_point(0.75, 10, &config);

        let json = serde_json::to_string(&state).expect("serialize");
        let deserialized: DriftState = serde_json::from_str(&json).expect("deserialize");

        assert!((deserialized.baseline - 0.80).abs() < f32::EPSILON);
        assert_eq!(deserialized.history.len(), 1);
        assert_eq!(deserialized.severity, state.severity);
    }

    #[test]
    fn test_negative_drift_detection() {
        // Test when alignment improves beyond baseline (negative drift)
        let mut state = DriftState::with_baseline(0.70);
        let config = DriftConfig::default();

        state.add_data_point(0.85, 5, &config);

        // Drift should be negative (baseline - rolling_mean = 0.70 - 0.85 = -0.15)
        assert!(state.drift < 0.0);
        // But severity is based on absolute value, so it's still severe
        assert_eq!(state.severity, DriftSeverity::Severe);
    }

    #[test]
    fn test_custom_thresholds() {
        let config = DriftConfig {
            monitoring: DriftMonitoring::Periodic { interval_hours: 12 },
            alert_threshold: 0.03,
            auto_correct: false,
            severe_threshold: 0.08,
            window_days: 14,
        };

        let mut state = DriftState::with_baseline(0.80);

        // With custom thresholds, 0.04 drift should be moderate (>= 0.03, < 0.08)
        state.add_data_point(0.76, 5, &config);
        assert_eq!(state.severity, DriftSeverity::Moderate);

        // 0.09 drift should be severe (>= 0.08)
        let mut state = DriftState::with_baseline(0.80);
        state.add_data_point(0.71, 5, &config);
        assert_eq!(state.severity, DriftSeverity::Severe);
    }
}
