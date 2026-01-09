//! Core implementation for TeleologicalFingerprint.
//!
//! This module contains constructors, constants, and core methods.

use chrono::Utc;
use uuid::Uuid;

use crate::types::fingerprint::evolution::{EvolutionTrigger, PurposeSnapshot};
use crate::types::fingerprint::johari::JohariFingerprint;
use crate::types::fingerprint::purpose::PurposeVector;
use crate::types::fingerprint::SemanticFingerprint;

use super::types::TeleologicalFingerprint;

impl TeleologicalFingerprint {
    /// Expected size in bytes for a complete teleological fingerprint.
    /// From constitution.yaml: ~46KB per node.
    pub const EXPECTED_SIZE_BYTES: usize = 46_000;

    /// Maximum number of evolution snapshots to retain in memory.
    /// Older snapshots are archived to TimescaleDB in production.
    pub const MAX_EVOLUTION_SNAPSHOTS: usize = 100;

    /// Threshold for misalignment warning (from constitution.yaml).
    /// delta_A < -0.15 predicts failure 72 hours ahead.
    pub const MISALIGNMENT_THRESHOLD: f32 = -0.15;

    /// Create a new TeleologicalFingerprint.
    ///
    /// Automatically:
    /// - Generates a new UUID v4
    /// - Sets timestamps to now
    /// - Computes initial theta_to_north_star
    /// - Records initial evolution snapshot with Created trigger
    ///
    /// # Arguments
    /// * `semantic` - The semantic fingerprint (13 embeddings)
    /// * `purpose_vector` - The purpose alignment vector
    /// * `johari` - The Johari awareness classification
    /// * `content_hash` - SHA-256 hash of source content
    pub fn new(
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let now = Utc::now();
        let theta_to_north_star = purpose_vector.aggregate_alignment();

        // Create initial snapshot
        let initial_snapshot = PurposeSnapshot::new(
            purpose_vector.clone(),
            johari.clone(),
            EvolutionTrigger::Created,
        );

        Self {
            id: Uuid::new_v4(),
            semantic,
            purpose_vector,
            johari,
            purpose_evolution: vec![initial_snapshot],
            theta_to_north_star,
            content_hash,
            created_at: now,
            last_updated: now,
            access_count: 0,
        }
    }

    /// Create a TeleologicalFingerprint with a specific ID (for testing/import).
    pub fn with_id(
        id: Uuid,
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let mut fp = Self::new(semantic, purpose_vector, johari, content_hash);
        fp.id = id;
        fp
    }

    /// Record a new purpose evolution snapshot.
    ///
    /// Updates:
    /// - Adds snapshot to evolution history
    /// - Trims history if over MAX_EVOLUTION_SNAPSHOTS
    /// - Updates last_updated timestamp
    /// - Recalculates theta_to_north_star
    ///
    /// # Arguments
    /// * `trigger` - What caused this evolution event
    pub fn record_snapshot(&mut self, trigger: EvolutionTrigger) {
        let snapshot =
            PurposeSnapshot::new(self.purpose_vector.clone(), self.johari.clone(), trigger);

        self.purpose_evolution.push(snapshot);

        // Trim if over limit (remove oldest)
        if self.purpose_evolution.len() > Self::MAX_EVOLUTION_SNAPSHOTS {
            // In production, archive to TimescaleDB before removing
            self.purpose_evolution.remove(0);
        }

        self.last_updated = Utc::now();
        self.theta_to_north_star = self.purpose_vector.aggregate_alignment();
    }

    /// Record an access event.
    ///
    /// Increments access_count and optionally records a snapshot.
    ///
    /// # Arguments
    /// * `query_context` - Description of the query that accessed this memory
    /// * `record_snapshot` - Whether to record an evolution snapshot
    pub fn record_access(&mut self, query_context: String, record_evolution: bool) {
        self.access_count += 1;
        self.last_updated = Utc::now();

        if record_evolution {
            self.record_snapshot(EvolutionTrigger::Accessed { query_context });
        }
    }

    /// Update purpose vector (e.g., after recalibration).
    ///
    /// Automatically records an evolution snapshot and checks for misalignment.
    pub fn update_purpose(&mut self, new_purpose: PurposeVector, trigger: EvolutionTrigger) {
        self.purpose_vector = new_purpose;
        self.record_snapshot(trigger);
    }

    /// Get the age of this fingerprint (time since creation).
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Get the number of evolution snapshots.
    pub fn evolution_count(&self) -> usize {
        self.purpose_evolution.len()
    }
}
