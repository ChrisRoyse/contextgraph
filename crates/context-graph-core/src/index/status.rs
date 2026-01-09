//! Index status and health types.
//!
//! # Health States
//!
//! - `Healthy`: Normal operation
//! - `Failed`: Index corrupted, must rebuild
//! - `Rebuilding`: Index being reconstructed
//!
//! **NO DEGRADED MODE.** If an index fails, it fails completely.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use super::config::EmbedderIndex;

/// Index health state - NO DEGRADED MODE, fail fast.
///
/// # States
///
/// - `Healthy`: Index operating normally, all operations available
/// - `Failed`: Index corrupted or inconsistent, must rebuild
/// - `Rebuilding`: Index being reconstructed, read operations may return stale data
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexHealth {
    /// Index is operating normally
    #[default]
    Healthy,
    /// Index has failed and must be rebuilt
    Failed,
    /// Index is being rebuilt
    Rebuilding,
}

impl IndexHealth {
    /// Check if index is operational (Healthy or Rebuilding).
    #[inline]
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Rebuilding)
    }

    /// Check if index can accept writes.
    #[inline]
    pub fn can_write(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// Status of a single embedder index.
///
/// # Fields
///
/// - `embedder`: Which embedder this index serves
/// - `is_loaded`: Whether the index is in memory
/// - `element_count`: Number of vectors in the index
/// - `memory_usage_bytes`: Approximate memory footprint
/// - `last_updated`: When the index was last modified
/// - `health`: Current health state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexStatus {
    /// Which embedder this index serves
    pub embedder: EmbedderIndex,
    /// Whether the index is loaded in memory
    pub is_loaded: bool,
    /// Number of elements in the index
    pub element_count: usize,
    /// Approximate memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
    /// Health state
    pub health: IndexHealth,
}

impl IndexStatus {
    /// Create a new status for an initialized but empty index.
    pub fn new_empty(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            is_loaded: true,
            element_count: 0,
            memory_usage_bytes: 0,
            last_updated: Utc::now(),
            health: IndexHealth::Healthy,
        }
    }

    /// Create a status for an uninitialized index.
    pub fn uninitialized(embedder: EmbedderIndex) -> Self {
        Self {
            embedder,
            is_loaded: false,
            element_count: 0,
            memory_usage_bytes: 0,
            last_updated: Utc::now(),
            health: IndexHealth::Healthy,
        }
    }

    /// Update element count and recalculate memory estimate.
    ///
    /// # Arguments
    ///
    /// - `count`: New element count
    /// - `bytes_per_element`: Memory per element (dimension * 4 + overhead)
    pub fn update_count(&mut self, count: usize, bytes_per_element: usize) {
        self.element_count = count;
        self.memory_usage_bytes = count * bytes_per_element;
        self.last_updated = Utc::now();
    }

    /// Mark index as failed.
    pub fn mark_failed(&mut self) {
        self.health = IndexHealth::Failed;
        self.last_updated = Utc::now();
    }

    /// Mark index as rebuilding.
    pub fn mark_rebuilding(&mut self) {
        self.health = IndexHealth::Rebuilding;
        self.last_updated = Utc::now();
    }

    /// Mark index as healthy.
    pub fn mark_healthy(&mut self) {
        self.health = IndexHealth::Healthy;
        self.last_updated = Utc::now();
    }
}

impl PartialEq for IndexStatus {
    fn eq(&self, other: &Self) -> bool {
        self.embedder == other.embedder
            && self.is_loaded == other.is_loaded
            && self.element_count == other.element_count
            && self.health == other.health
    }
}

/// Aggregated health status for all indexes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiIndexHealth {
    /// Count of healthy indexes
    pub healthy_count: usize,
    /// Count of failed indexes
    pub failed_count: usize,
    /// Count of rebuilding indexes
    pub rebuilding_count: usize,
    /// Total element count across all indexes
    pub total_elements: usize,
    /// Total memory usage across all indexes
    pub total_memory_bytes: usize,
    /// List of failed embedders
    pub failed_embedders: Vec<EmbedderIndex>,
}

impl MultiIndexHealth {
    /// Create health summary from a list of statuses.
    pub fn from_statuses(statuses: &[IndexStatus]) -> Self {
        let mut healthy = 0;
        let mut failed = 0;
        let mut rebuilding = 0;
        let mut total_elements = 0;
        let mut total_memory = 0;
        let mut failed_embedders = Vec::new();

        for status in statuses {
            total_elements += status.element_count;
            total_memory += status.memory_usage_bytes;

            match status.health {
                IndexHealth::Healthy => healthy += 1,
                IndexHealth::Failed => {
                    failed += 1;
                    failed_embedders.push(status.embedder);
                }
                IndexHealth::Rebuilding => rebuilding += 1,
            }
        }

        Self {
            healthy_count: healthy,
            failed_count: failed,
            rebuilding_count: rebuilding,
            total_elements,
            total_memory_bytes: total_memory,
            failed_embedders,
        }
    }

    /// Check if all indexes are healthy.
    #[inline]
    pub fn all_healthy(&self) -> bool {
        self.failed_count == 0 && self.rebuilding_count == 0
    }

    /// Check if any index has failed.
    #[inline]
    pub fn has_failures(&self) -> bool {
        self.failed_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_health_defaults_to_healthy() {
        let health = IndexHealth::default();
        assert_eq!(health, IndexHealth::Healthy);
        println!("[VERIFIED] IndexHealth defaults to Healthy");
    }

    #[test]
    fn test_index_health_operational_states() {
        assert!(IndexHealth::Healthy.is_operational());
        assert!(IndexHealth::Rebuilding.is_operational());
        assert!(!IndexHealth::Failed.is_operational());
        println!("[VERIFIED] Health operational state checks");
    }

    #[test]
    fn test_index_health_write_capability() {
        assert!(IndexHealth::Healthy.can_write());
        assert!(!IndexHealth::Rebuilding.can_write());
        assert!(!IndexHealth::Failed.can_write());
        println!("[VERIFIED] Health write capability checks");
    }

    #[test]
    fn test_index_status_new_empty() {
        let status = IndexStatus::new_empty(EmbedderIndex::E1Semantic);
        assert_eq!(status.embedder, EmbedderIndex::E1Semantic);
        assert!(status.is_loaded);
        assert_eq!(status.element_count, 0);
        assert_eq!(status.health, IndexHealth::Healthy);
        println!("[VERIFIED] IndexStatus::new_empty creates healthy empty index");
    }

    #[test]
    fn test_index_status_update_count() {
        let mut status = IndexStatus::new_empty(EmbedderIndex::E1Semantic);
        println!("[BEFORE] element_count: {}", status.element_count);

        status.update_count(100, 1024 * 4);
        println!("[AFTER] element_count: {}", status.element_count);

        assert_eq!(status.element_count, 100);
        assert_eq!(status.memory_usage_bytes, 100 * 4096);
        println!("[VERIFIED] update_count calculates memory correctly");
    }

    #[test]
    fn test_index_status_health_transitions() {
        let mut status = IndexStatus::new_empty(EmbedderIndex::E1Semantic);

        status.mark_failed();
        assert_eq!(status.health, IndexHealth::Failed);
        println!("[VERIFIED] mark_failed sets Failed state");

        status.mark_rebuilding();
        assert_eq!(status.health, IndexHealth::Rebuilding);
        println!("[VERIFIED] mark_rebuilding sets Rebuilding state");

        status.mark_healthy();
        assert_eq!(status.health, IndexHealth::Healthy);
        println!("[VERIFIED] mark_healthy sets Healthy state");
    }

    #[test]
    fn test_multi_index_health_aggregation() {
        let statuses = vec![
            IndexStatus::new_empty(EmbedderIndex::E1Semantic),
            IndexStatus::new_empty(EmbedderIndex::E2TemporalRecent),
            {
                let mut s = IndexStatus::new_empty(EmbedderIndex::E7Code);
                s.mark_failed();
                s
            },
        ];

        let health = MultiIndexHealth::from_statuses(&statuses);

        assert_eq!(health.healthy_count, 2);
        assert_eq!(health.failed_count, 1);
        assert!(!health.all_healthy());
        assert!(health.has_failures());
        assert_eq!(health.failed_embedders, vec![EmbedderIndex::E7Code]);
        println!("[VERIFIED] MultiIndexHealth aggregation correct");
    }
}
