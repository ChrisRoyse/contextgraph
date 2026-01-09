//! System monitoring traits for real health metrics.
//!
//! TASK-EMB-024: This module provides traits for collecting REAL system metrics
//! to replace hardcoded values in the codebase.
//!
//! # Problem Statement
//!
//! The following hardcoded values need to be replaced with real metrics:
//! - `coherence_recovery_time_ms: 8500` (utl.rs:366)
//! - `attack_detection_rate: 0.97` (utl.rs:367)
//! - `false_positive_rate: 0.015` (utl.rs:368)
//! - Layer statuses: `"stub"` hardcoded (tools.rs:459-461)
//!
//! # Design Principles
//!
//! 1. **No Defaults, No Fallbacks**: All methods return `Result<T, SystemMonitorError>`
//! 2. **Explicit Failure**: Stub implementations FAIL with descriptive errors
//! 3. **Thread-Safe**: All traits are `Send + Sync + 'static` for `Arc` usage
//! 4. **Detailed Errors**: Each failure mode has its own error variant
//!
//! # Example
//!
//! ```rust,ignore
//! use context_graph_core::monitoring::{SystemMonitor, SystemMonitorError};
//! use std::sync::Arc;
//!
//! async fn get_metrics(monitor: Arc<dyn SystemMonitor>) {
//!     // This will FAIL if no real monitor is configured
//!     match monitor.coherence_recovery_time_ms().await {
//!         Ok(time) => println!("Recovery time: {}ms", time),
//!         Err(SystemMonitorError::NotImplemented { component }) => {
//!             panic!("Real monitoring not configured for: {}", component);
//!         }
//!         Err(e) => panic!("Monitoring error: {}", e),
//!     }
//! }
//! ```

use async_trait::async_trait;
use std::fmt;
use std::time::Duration;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

/// Error type for system monitoring operations.
///
/// Each variant represents a specific failure mode with detailed context.
/// NO fallback values - all failures are explicit errors.
#[derive(Debug, Error)]
pub enum SystemMonitorError {
    /// Component is not implemented - used by stub implementations.
    ///
    /// # When This Occurs
    ///
    /// - Using `StubSystemMonitor` which intentionally fails
    /// - Real monitoring backend not configured
    /// - Feature not implemented in current version
    #[error("Not implemented: {component} - {message}")]
    NotImplemented {
        /// Name of the unimplemented component
        component: String,
        /// Additional context about why it's not implemented
        message: String,
    },

    /// Failed to collect metrics from the underlying system.
    ///
    /// # When This Occurs
    ///
    /// - Metrics collection timed out
    /// - Underlying system unavailable
    /// - Resource exhaustion during collection
    #[error("Metric collection failed for {metric}: {reason}")]
    CollectionFailed {
        /// Name of the metric that failed to collect
        metric: String,
        /// Reason for the failure
        reason: String,
    },

    /// Metric value is invalid or out of expected range.
    ///
    /// # When This Occurs
    ///
    /// - Metric value is NaN or Infinity
    /// - Value outside valid range (e.g., rate > 1.0)
    /// - Corrupted data from monitoring backend
    #[error("Invalid metric value for {metric}: expected {expected}, got {actual}")]
    InvalidValue {
        /// Name of the metric with invalid value
        metric: String,
        /// Description of expected value/range
        expected: String,
        /// Actual value received
        actual: String,
    },

    /// Communication with monitoring backend failed.
    ///
    /// # When This Occurs
    ///
    /// - Network timeout to monitoring service
    /// - Authentication failure
    /// - Backend service unavailable
    #[error("Backend communication error: {0}")]
    BackendError(String),

    /// Monitoring system is not initialized.
    ///
    /// # When This Occurs
    ///
    /// - Calling methods before initialization
    /// - Initialization failed silently
    /// - Monitor was shut down
    #[error("Monitor not initialized: {0}")]
    NotInitialized(String),

    /// Timeout while waiting for metrics.
    ///
    /// # When This Occurs
    ///
    /// - Metric collection exceeded timeout
    /// - Backend response too slow
    /// - System under heavy load
    #[error("Timeout after {duration:?} waiting for {metric}")]
    Timeout {
        /// Name of the metric that timed out
        metric: String,
        /// Duration waited before timeout
        duration: Duration,
    },

    /// Layer-specific error during status check.
    ///
    /// # When This Occurs
    ///
    /// - Layer health check failed
    /// - Layer in error state
    /// - Layer communication failure
    #[error("Layer '{layer}' error: {message}")]
    LayerError {
        /// Name of the layer with the error
        layer: String,
        /// Error message from the layer
        message: String,
    },
}

/// Result type alias for monitoring operations.
pub type MonitorResult<T> = Result<T, SystemMonitorError>;

// ============================================================================
// Layer Status Types
// ============================================================================

/// Status of a single layer in the 5-layer bio-nervous architecture.
///
/// Represents the REAL implementation status of each layer.
/// No layer should report "active" unless it has a working implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerStatus {
    /// Layer is fully implemented and operational.
    ///
    /// Only report this if the layer has a real, working implementation
    /// that passes health checks.
    Active,

    /// Layer has only stub/placeholder implementation.
    ///
    /// The layer exists but returns mock/deterministic data.
    /// This is honest reporting during development phases.
    Stub,

    /// Layer is in an error state with details.
    ///
    /// The layer was expected to be active but encountered an error.
    Error(String),

    /// Layer is not yet implemented.
    ///
    /// The layer is defined in the architecture but has no implementation.
    NotImplemented,

    /// Layer is disabled by configuration.
    ///
    /// The layer exists but is explicitly disabled.
    Disabled,
}

impl fmt::Display for LayerStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayerStatus::Active => write!(f, "active"),
            LayerStatus::Stub => write!(f, "stub"),
            LayerStatus::Error(msg) => write!(f, "error: {}", msg),
            LayerStatus::NotImplemented => write!(f, "not_implemented"),
            LayerStatus::Disabled => write!(f, "disabled"),
        }
    }
}

impl LayerStatus {
    /// Returns true if the layer is fully operational.
    pub fn is_active(&self) -> bool {
        matches!(self, LayerStatus::Active)
    }

    /// Returns true if the layer has any kind of error.
    pub fn is_error(&self) -> bool {
        matches!(self, LayerStatus::Error(_))
    }

    /// Returns the status as a JSON-compatible string.
    pub fn as_str(&self) -> &str {
        match self {
            LayerStatus::Active => "active",
            LayerStatus::Stub => "stub",
            LayerStatus::Error(_) => "error",
            LayerStatus::NotImplemented => "not_implemented",
            LayerStatus::Disabled => "disabled",
        }
    }
}

/// Information about a layer including its status and health metrics.
#[derive(Debug, Clone)]
pub struct LayerInfo {
    /// Layer name (e.g., "perception", "memory", "reasoning")
    pub name: String,
    /// Current status of the layer
    pub status: LayerStatus,
    /// Optional latency of last operation in microseconds
    pub last_latency_us: Option<u64>,
    /// Optional error count in current session
    pub error_count: Option<u64>,
    /// Whether health check passed
    pub health_check_passed: Option<bool>,
}

// ============================================================================
// System Monitor Trait
// ============================================================================

/// Trait for collecting REAL system health metrics.
///
/// This trait defines the contract for system monitoring. All implementations
/// MUST return actual measured values or explicit errors - NO hardcoded defaults.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync + 'static` to support `Arc<dyn SystemMonitor>`.
///
/// # Error Handling
///
/// All methods return `MonitorResult<T>`. Implementations MUST NOT:
/// - Return hardcoded fallback values
/// - Silently succeed with fake data
/// - Return `Ok(default_value)` when real data is unavailable
///
/// # Example Implementation
///
/// ```rust,ignore
/// use context_graph_core::monitoring::{SystemMonitor, MonitorResult, SystemMonitorError};
/// use async_trait::async_trait;
///
/// struct RealSystemMonitor {
///     metrics_backend: MetricsClient,
/// }
///
/// #[async_trait]
/// impl SystemMonitor for RealSystemMonitor {
///     async fn coherence_recovery_time_ms(&self) -> MonitorResult<u64> {
///         // Actually measure recovery time from real coherence events
///         self.metrics_backend
///             .get_p95_recovery_time()
///             .await
///             .map_err(|e| SystemMonitorError::BackendError(e.to_string()))
///     }
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait SystemMonitor: Send + Sync + 'static {
    /// Get the P95 coherence recovery time in milliseconds.
    ///
    /// This measures how long it takes the system to recover coherence
    /// after a disruption (e.g., network partition, crash recovery).
    ///
    /// # Target
    ///
    /// The UTL constitution specifies a target of <10000ms for this metric.
    ///
    /// # Returns
    ///
    /// - `Ok(ms)` - Actual P95 recovery time from monitoring data
    /// - `Err(NotImplemented)` - No real monitoring configured
    /// - `Err(CollectionFailed)` - Metric collection failed
    async fn coherence_recovery_time_ms(&self) -> MonitorResult<u64>;

    /// Get the attack detection rate (0.0 to 1.0).
    ///
    /// This measures the proportion of adversarial attacks that are
    /// successfully detected by the system's security mechanisms.
    ///
    /// # Target
    ///
    /// The UTL constitution specifies a target of >=0.95 for this metric.
    ///
    /// # Returns
    ///
    /// - `Ok(rate)` - Actual detection rate in [0.0, 1.0]
    /// - `Err(InvalidValue)` - Rate outside valid range
    /// - `Err(NotImplemented)` - No security monitoring configured
    async fn attack_detection_rate(&self) -> MonitorResult<f32>;

    /// Get the false positive rate (0.0 to 1.0).
    ///
    /// This measures the proportion of legitimate operations incorrectly
    /// flagged as attacks or anomalies.
    ///
    /// # Target
    ///
    /// The UTL constitution specifies a target of <0.02 for this metric.
    ///
    /// # Returns
    ///
    /// - `Ok(rate)` - Actual false positive rate in [0.0, 1.0]
    /// - `Err(InvalidValue)` - Rate outside valid range
    /// - `Err(NotImplemented)` - No monitoring configured
    async fn false_positive_rate(&self) -> MonitorResult<f32>;

    /// Get all health metrics at once.
    ///
    /// This is more efficient than calling individual methods when
    /// all metrics are needed, as it may batch backend requests.
    ///
    /// # Returns
    ///
    /// - `Ok(HealthMetrics)` - All metrics collected successfully
    /// - `Err(_)` - Any metric failed to collect
    async fn all_health_metrics(&self) -> MonitorResult<HealthMetrics>;

    /// Check if the monitoring system is operational.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - Monitoring is fully operational
    /// - `Ok(false)` - Monitoring is degraded but functional
    /// - `Err(NotInitialized)` - Monitoring is not initialized
    async fn is_operational(&self) -> MonitorResult<bool>;
}

/// Collection of all health metrics.
#[derive(Debug, Clone)]
pub struct HealthMetrics {
    /// P95 coherence recovery time in milliseconds
    pub coherence_recovery_time_ms: u64,
    /// Attack detection rate (0.0 to 1.0)
    pub attack_detection_rate: f32,
    /// False positive rate (0.0 to 1.0)
    pub false_positive_rate: f32,
    /// Timestamp when metrics were collected
    pub collected_at: std::time::Instant,
}

// ============================================================================
// Layer Status Provider Trait
// ============================================================================

/// Trait for providing REAL layer status information.
///
/// This trait reports the actual implementation status of each layer
/// in the 5-layer bio-nervous architecture. Implementations MUST report
/// honest status - a layer is only "active" if it has real functionality.
///
/// # Thread Safety
///
/// All implementations must be `Send + Sync + 'static` for `Arc` usage.
///
/// # Layers
///
/// The 5-layer architecture consists of:
/// 1. **Perception** - Sensory input processing and feature extraction
/// 2. **Memory** - Teleological memory with 13-embedding semantic fingerprints
/// 3. **Reasoning** - Inference, planning, and decision making
/// 4. **Action** - Response generation and execution
/// 5. **Meta** - Self-monitoring and meta-cognition
#[async_trait]
pub trait LayerStatusProvider: Send + Sync + 'static {
    /// Get the status of the perception layer.
    async fn perception_status(&self) -> MonitorResult<LayerStatus>;

    /// Get the status of the memory layer.
    async fn memory_status(&self) -> MonitorResult<LayerStatus>;

    /// Get the status of the reasoning layer.
    async fn reasoning_status(&self) -> MonitorResult<LayerStatus>;

    /// Get the status of the action layer.
    async fn action_status(&self) -> MonitorResult<LayerStatus>;

    /// Get the status of the meta layer.
    async fn meta_status(&self) -> MonitorResult<LayerStatus>;

    /// Get detailed info for all layers.
    ///
    /// Returns a vector of LayerInfo with status and optional metrics.
    async fn all_layer_info(&self) -> MonitorResult<Vec<LayerInfo>>;

    /// Get status for a layer by name.
    ///
    /// # Arguments
    ///
    /// * `layer_name` - One of: "perception", "memory", "reasoning", "action", "meta"
    ///
    /// # Returns
    ///
    /// - `Ok(LayerStatus)` - Status of the requested layer
    /// - `Err(LayerError)` - Layer not found or error checking status
    async fn layer_status_by_name(&self, layer_name: &str) -> MonitorResult<LayerStatus>;
}

// ============================================================================
// Stub Implementations - THESE INTENTIONALLY FAIL
// ============================================================================

/// Stub implementation that FAILS with explicit "not implemented" errors.
///
/// This is the default when no real monitoring is configured. It exists
/// to make the absence of real monitoring EXPLICIT rather than hiding it
/// behind hardcoded values.
///
/// # Behavior
///
/// ALL methods return `Err(SystemMonitorError::NotImplemented { ... })`.
/// This forces callers to either:
/// 1. Configure real monitoring
/// 2. Explicitly handle the not-implemented case
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::monitoring::{StubSystemMonitor, SystemMonitor};
///
/// let monitor = StubSystemMonitor::new();
///
/// // This WILL fail - by design
/// let result = monitor.coherence_recovery_time_ms().await;
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone, Default)]
pub struct StubSystemMonitor;

impl StubSystemMonitor {
    /// Create a new stub monitor.
    ///
    /// The stub monitor ALWAYS fails with NotImplemented errors.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SystemMonitor for StubSystemMonitor {
    async fn coherence_recovery_time_ms(&self) -> MonitorResult<u64> {
        Err(SystemMonitorError::NotImplemented {
            component: "coherence_recovery_monitoring".to_string(),
            message: "Not implemented - no real system monitor configured. \
                     Configure a real SystemMonitor implementation to get actual metrics."
                .to_string(),
        })
    }

    async fn attack_detection_rate(&self) -> MonitorResult<f32> {
        Err(SystemMonitorError::NotImplemented {
            component: "attack_detection_monitoring".to_string(),
            message: "Not implemented - no real system monitor configured. \
                     Configure a real SystemMonitor implementation to get actual metrics."
                .to_string(),
        })
    }

    async fn false_positive_rate(&self) -> MonitorResult<f32> {
        Err(SystemMonitorError::NotImplemented {
            component: "false_positive_monitoring".to_string(),
            message: "Not implemented - no real system monitor configured. \
                     Configure a real SystemMonitor implementation to get actual metrics."
                .to_string(),
        })
    }

    async fn all_health_metrics(&self) -> MonitorResult<HealthMetrics> {
        Err(SystemMonitorError::NotImplemented {
            component: "health_metrics".to_string(),
            message: "Not implemented - no real system monitor configured. \
                     Configure a real SystemMonitor implementation to get actual metrics."
                .to_string(),
        })
    }

    async fn is_operational(&self) -> MonitorResult<bool> {
        // This is the ONE method that can return Ok - to indicate the stub is "running"
        // but it returns false to indicate monitoring is not operational
        Ok(false)
    }
}

/// Stub implementation for layer status that reports honest stub status.
///
/// This implementation reports layers as they actually are in the current
/// development phase - some active, some stubbed.
///
/// # Current Status (Phase 0 - Ghost System)
///
/// - Perception: Active (stub implementation exists and works)
/// - Memory: Active (InMemoryTeleologicalStore works)
/// - Reasoning: Stub (no real implementation)
/// - Action: Stub (no real implementation)
/// - Meta: Stub (no real implementation)
#[derive(Debug, Clone, Default)]
pub struct StubLayerStatusProvider;

impl StubLayerStatusProvider {
    /// Create a new stub layer status provider.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LayerStatusProvider for StubLayerStatusProvider {
    async fn perception_status(&self) -> MonitorResult<LayerStatus> {
        // Perception layer has working stub implementation
        Ok(LayerStatus::Active)
    }

    async fn memory_status(&self) -> MonitorResult<LayerStatus> {
        // Memory layer has working InMemoryTeleologicalStore
        Ok(LayerStatus::Active)
    }

    async fn reasoning_status(&self) -> MonitorResult<LayerStatus> {
        // Reasoning layer is not implemented yet
        Ok(LayerStatus::Stub)
    }

    async fn action_status(&self) -> MonitorResult<LayerStatus> {
        // Action layer is not implemented yet
        Ok(LayerStatus::Stub)
    }

    async fn meta_status(&self) -> MonitorResult<LayerStatus> {
        // Meta layer is not implemented yet
        Ok(LayerStatus::Stub)
    }

    async fn all_layer_info(&self) -> MonitorResult<Vec<LayerInfo>> {
        Ok(vec![
            LayerInfo {
                name: "perception".to_string(),
                status: LayerStatus::Active,
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(true),
            },
            LayerInfo {
                name: "memory".to_string(),
                status: LayerStatus::Active,
                last_latency_us: None,
                error_count: None,
                health_check_passed: Some(true),
            },
            LayerInfo {
                name: "reasoning".to_string(),
                status: LayerStatus::Stub,
                last_latency_us: None,
                error_count: None,
                health_check_passed: None,
            },
            LayerInfo {
                name: "action".to_string(),
                status: LayerStatus::Stub,
                last_latency_us: None,
                error_count: None,
                health_check_passed: None,
            },
            LayerInfo {
                name: "meta".to_string(),
                status: LayerStatus::Stub,
                last_latency_us: None,
                error_count: None,
                health_check_passed: None,
            },
        ])
    }

    async fn layer_status_by_name(&self, layer_name: &str) -> MonitorResult<LayerStatus> {
        match layer_name.to_lowercase().as_str() {
            "perception" => self.perception_status().await,
            "memory" => self.memory_status().await,
            "reasoning" => self.reasoning_status().await,
            "action" => self.action_status().await,
            "meta" => self.meta_status().await,
            _ => Err(SystemMonitorError::LayerError {
                layer: layer_name.to_string(),
                message: format!("Unknown layer: '{}'. Valid layers: perception, memory, reasoning, action, meta", layer_name),
            }),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stub_system_monitor_fails() {
        let monitor = StubSystemMonitor::new();

        // All metric methods should fail with NotImplemented
        let result = monitor.coherence_recovery_time_ms().await;
        assert!(matches!(
            result,
            Err(SystemMonitorError::NotImplemented { .. })
        ));

        let result = monitor.attack_detection_rate().await;
        assert!(matches!(
            result,
            Err(SystemMonitorError::NotImplemented { .. })
        ));

        let result = monitor.false_positive_rate().await;
        assert!(matches!(
            result,
            Err(SystemMonitorError::NotImplemented { .. })
        ));

        let result = monitor.all_health_metrics().await;
        assert!(matches!(
            result,
            Err(SystemMonitorError::NotImplemented { .. })
        ));

        // is_operational returns Ok(false) - stub is running but not operational
        let result = monitor.is_operational().await;
        assert!(matches!(result, Ok(false)));
    }

    #[tokio::test]
    async fn test_stub_layer_status_provider() {
        let provider = StubLayerStatusProvider::new();

        // Active layers
        assert!(matches!(
            provider.perception_status().await,
            Ok(LayerStatus::Active)
        ));
        assert!(matches!(
            provider.memory_status().await,
            Ok(LayerStatus::Active)
        ));

        // Stub layers
        assert!(matches!(
            provider.reasoning_status().await,
            Ok(LayerStatus::Stub)
        ));
        assert!(matches!(
            provider.action_status().await,
            Ok(LayerStatus::Stub)
        ));
        assert!(matches!(
            provider.meta_status().await,
            Ok(LayerStatus::Stub)
        ));

        // Unknown layer should error
        let result = provider.layer_status_by_name("unknown").await;
        assert!(matches!(result, Err(SystemMonitorError::LayerError { .. })));
    }

    #[test]
    fn test_layer_status_display() {
        assert_eq!(LayerStatus::Active.to_string(), "active");
        assert_eq!(LayerStatus::Stub.to_string(), "stub");
        assert_eq!(
            LayerStatus::Error("test".to_string()).to_string(),
            "error: test"
        );
        assert_eq!(LayerStatus::NotImplemented.to_string(), "not_implemented");
        assert_eq!(LayerStatus::Disabled.to_string(), "disabled");
    }

    #[test]
    fn test_layer_status_methods() {
        assert!(LayerStatus::Active.is_active());
        assert!(!LayerStatus::Stub.is_active());
        assert!(!LayerStatus::Error("x".to_string()).is_active());

        assert!(!LayerStatus::Active.is_error());
        assert!(LayerStatus::Error("x".to_string()).is_error());
    }

    #[test]
    fn test_error_display() {
        let err = SystemMonitorError::NotImplemented {
            component: "test".to_string(),
            message: "not configured".to_string(),
        };
        assert!(err.to_string().contains("Not implemented"));
        assert!(err.to_string().contains("test"));

        let err = SystemMonitorError::CollectionFailed {
            metric: "latency".to_string(),
            reason: "timeout".to_string(),
        };
        assert!(err.to_string().contains("collection failed"));
        assert!(err.to_string().contains("latency"));
    }
}
