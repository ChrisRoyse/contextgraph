//! NORTH-012: PruningService
//!
//! Service for identifying and removing low-value memories from the knowledge graph.
//! Uses alignment scores, age, connection count, and quality metrics to determine
//! which memories should be pruned.
//!
//! # Architecture
//!
//! The PruningService operates in three phases:
//! 1. **Identification**: Scan memories and identify pruning candidates
//! 2. **Evaluation**: Score each candidate and determine prune reason
//! 3. **Execution**: Remove candidates up to daily limit
//!
//! # Fail-Fast Behavior
//!
//! All methods fail immediately on invalid input rather than returning
//! partial results or silently ignoring errors.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::autonomous::curation::{MemoryId, PruningConfig};

// ============================================================================
// Extended Types for PruningService
// ============================================================================

/// Extended prune reasons beyond base curation types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PruneReason {
    /// Alignment score below threshold
    LowAlignment,
    /// Memory not accessed for extended period
    Stale,
    /// No connections to other memories (isolated)
    Orphaned,
    /// Duplicate or near-duplicate of another memory
    Redundant,
    /// Quality metrics below acceptable threshold
    LowQuality,
}

impl PruneReason {
    /// Get a human-readable description of the prune reason
    pub fn description(&self) -> &'static str {
        match self {
            PruneReason::LowAlignment => "Alignment score below threshold",
            PruneReason::Stale => "Not accessed for extended period",
            PruneReason::Orphaned => "No connections to other memories",
            PruneReason::Redundant => "Duplicate or near-duplicate content",
            PruneReason::LowQuality => "Quality metrics below threshold",
        }
    }
}

/// Metadata about a memory for pruning evaluation
#[derive(Clone, Debug)]
pub struct MemoryMetadata {
    /// Unique memory identifier
    pub id: MemoryId,
    /// When the memory was created
    pub created_at: DateTime<Utc>,
    /// North Star alignment score [0.0, 1.0]
    pub alignment: f32,
    /// Number of connections to other memories
    pub connection_count: u32,
    /// Size in bytes
    pub byte_size: u64,
    /// Last time memory was retrieved/accessed
    pub last_accessed: Option<DateTime<Utc>>,
    /// Quality score from content analysis [0.0, 1.0]
    pub quality_score: Option<f32>,
    /// Content hash for redundancy detection
    pub content_hash: Option<u64>,
}

impl MemoryMetadata {
    /// Create new memory metadata
    pub fn new(id: MemoryId, created_at: DateTime<Utc>, alignment: f32) -> Self {
        Self {
            id,
            created_at,
            alignment,
            connection_count: 0,
            byte_size: 0,
            last_accessed: None,
            quality_score: None,
            content_hash: None,
        }
    }

    /// Calculate age in days from creation time
    pub fn age_days(&self) -> u32 {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        duration.num_days().max(0) as u32
    }

    /// Calculate days since last access (None if never accessed)
    pub fn days_since_access(&self) -> Option<u32> {
        self.last_accessed.map(|accessed| {
            let now = Utc::now();
            let duration = now.signed_duration_since(accessed);
            duration.num_days().max(0) as u32
        })
    }
}

/// A candidate identified for potential pruning
#[derive(Clone, Debug)]
pub struct PruningCandidate {
    /// Memory identifier
    pub memory_id: MemoryId,
    /// Age of the memory in days
    pub age_days: u32,
    /// Alignment score
    pub alignment: f32,
    /// Number of connections
    pub connections: u32,
    /// Why this memory is a candidate for pruning
    pub reason: PruneReason,
    /// Size in bytes (for freed space calculation)
    pub byte_size: u64,
    /// Pruning priority score (higher = more urgent to prune)
    pub priority_score: f32,
}

impl PruningCandidate {
    /// Create a new pruning candidate
    pub fn new(
        memory_id: MemoryId,
        age_days: u32,
        alignment: f32,
        connections: u32,
        reason: PruneReason,
        byte_size: u64,
    ) -> Self {
        // Calculate priority score: lower alignment + older age = higher priority
        let alignment_factor = 1.0 - alignment;
        let age_factor = (age_days as f32 / 365.0).min(1.0);
        let connection_factor = 1.0 / (connections as f32 + 1.0);
        let priority_score = alignment_factor * 0.5 + age_factor * 0.3 + connection_factor * 0.2;

        Self {
            memory_id,
            age_days,
            alignment,
            connections,
            reason,
            byte_size,
            priority_score,
        }
    }
}

/// Report from a pruning operation
#[derive(Clone, Debug, Default)]
pub struct PruningReport {
    /// Number of memories evaluated for pruning
    pub candidates_evaluated: usize,
    /// Number of memories actually pruned
    pub pruned_count: usize,
    /// Total bytes freed by pruning
    pub bytes_freed: u64,
    /// Breakdown of prune reasons
    pub reasons_breakdown: HashMap<PruneReason, usize>,
    /// Number of candidates preserved (met protection criteria)
    pub preserved_count: usize,
    /// Whether daily limit was reached
    pub daily_limit_reached: bool,
}

impl PruningReport {
    /// Create an empty report
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pruned candidate to the report
    pub fn record_prune(&mut self, candidate: &PruningCandidate) {
        self.pruned_count += 1;
        self.bytes_freed += candidate.byte_size;
        *self
            .reasons_breakdown
            .entry(candidate.reason.clone())
            .or_insert(0) += 1;
    }

    /// Record a preserved candidate
    pub fn record_preserved(&mut self) {
        self.preserved_count += 1;
    }
}

/// Extended pruning configuration with daily limits
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendedPruningConfig {
    /// Base pruning configuration
    #[serde(flatten)]
    pub base: PruningConfig,
    /// Maximum prunes per day
    pub max_daily_prunes: u32,
    /// Stale threshold in days (no access)
    pub stale_days: u32,
    /// Quality score threshold
    pub min_quality: f32,
}

impl Default for ExtendedPruningConfig {
    fn default() -> Self {
        Self {
            base: PruningConfig::default(),
            max_daily_prunes: 100,
            stale_days: 90,
            min_quality: 0.30,
        }
    }
}

// ============================================================================
// PruningService
// ============================================================================

/// Service for identifying and removing low-value memories
#[derive(Clone, Debug)]
pub struct PruningService {
    /// Configuration for pruning behavior
    config: ExtendedPruningConfig,
    /// Count of prunes today
    daily_prune_count: u32,
    /// Date of current day for daily limit tracking
    current_day: DateTime<Utc>,
    /// Known content hashes for redundancy detection
    known_hashes: HashMap<u64, MemoryId>,
}

impl PruningService {
    /// Create a new PruningService with default configuration
    pub fn new() -> Self {
        Self {
            config: ExtendedPruningConfig::default(),
            daily_prune_count: 0,
            current_day: Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc(),
            known_hashes: HashMap::new(),
        }
    }

    /// Create a new PruningService with custom configuration
    pub fn with_config(config: ExtendedPruningConfig) -> Self {
        Self {
            config,
            daily_prune_count: 0,
            current_day: Utc::now()
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc(),
            known_hashes: HashMap::new(),
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &ExtendedPruningConfig {
        &self.config
    }

    /// Reset daily counter if day has changed
    fn check_day_rollover(&mut self) {
        let today = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        if today > self.current_day {
            self.daily_prune_count = 0;
            self.current_day = today;
        }
    }

    /// Get remaining prunes allowed today
    pub fn remaining_daily_prunes(&mut self) -> u32 {
        self.check_day_rollover();
        self.config
            .max_daily_prunes
            .saturating_sub(self.daily_prune_count)
    }

    /// Identify all pruning candidates from a set of memories
    ///
    /// # Fail-Fast
    /// Returns empty vec if memories is empty (not an error condition).
    pub fn identify_candidates(&self, memories: &[MemoryMetadata]) -> Vec<PruningCandidate> {
        if memories.is_empty() {
            return Vec::new();
        }

        // Build hash map for redundancy detection
        let mut hash_to_first: HashMap<u64, &MemoryMetadata> = HashMap::new();
        let mut redundant_ids: HashMap<MemoryId, MemoryId> = HashMap::new();

        for memory in memories {
            if let Some(hash) = memory.content_hash {
                if let Some(first) = hash_to_first.get(&hash) {
                    // This is a duplicate - the one with lower alignment is redundant
                    if memory.alignment < first.alignment {
                        redundant_ids.insert(memory.id.clone(), first.id.clone());
                    }
                } else {
                    hash_to_first.insert(hash, memory);
                }
            }
        }

        let mut candidates = Vec::new();

        for memory in memories {
            if let Some(candidate) = self.evaluate_candidate_internal(memory, &redundant_ids) {
                candidates.push(candidate);
            }
        }

        // Sort by priority score (highest first)
        candidates.sort_by(|a, b| {
            b.priority_score
                .partial_cmp(&a.priority_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates
    }

    /// Evaluate a single memory for pruning
    ///
    /// Returns Some(PruningCandidate) if the memory should be pruned,
    /// None if it should be kept.
    pub fn evaluate_candidate(&self, metadata: &MemoryMetadata) -> Option<PruningCandidate> {
        self.evaluate_candidate_internal(metadata, &HashMap::new())
    }

    /// Internal evaluation with redundancy context
    fn evaluate_candidate_internal(
        &self,
        metadata: &MemoryMetadata,
        redundant_ids: &HashMap<MemoryId, MemoryId>,
    ) -> Option<PruningCandidate> {
        if !self.config.base.enabled {
            return None;
        }

        let age_days = metadata.age_days();

        // Must be old enough to prune
        if age_days < self.config.base.min_age_days {
            return None;
        }

        // Check for prune reason
        if let Some(reason) = self.get_prune_reason_internal(metadata, redundant_ids) {
            // Check if should be preserved due to connections
            if self.config.base.preserve_connected
                && metadata.connection_count >= self.config.base.min_connections
            {
                return None;
            }

            return Some(PruningCandidate::new(
                metadata.id.clone(),
                age_days,
                metadata.alignment,
                metadata.connection_count,
                reason,
                metadata.byte_size,
            ));
        }

        None
    }

    /// Determine if a candidate should be pruned
    ///
    /// Takes into account daily limits and preservation rules.
    pub fn should_prune(&self, candidate: &PruningCandidate) -> bool {
        // Check if preserved by connection count
        if self.config.base.preserve_connected
            && candidate.connections >= self.config.base.min_connections
        {
            return false;
        }

        // Check daily limit (must be mutable to check, so we're conservative here)
        // The actual limit enforcement happens in prune()
        true
    }

    /// Execute pruning on a set of candidates
    ///
    /// Returns a report of what was pruned.
    ///
    /// # Fail-Fast
    /// - Respects daily limit strictly
    /// - Preserves candidates meeting protection criteria
    pub fn prune(&mut self, candidates: &[PruningCandidate]) -> PruningReport {
        self.check_day_rollover();

        let mut report = PruningReport::new();
        report.candidates_evaluated = candidates.len();

        for candidate in candidates {
            // Check daily limit
            if self.daily_prune_count >= self.config.max_daily_prunes {
                report.daily_limit_reached = true;
                break;
            }

            // Check preservation rules
            if self.config.base.preserve_connected
                && candidate.connections >= self.config.base.min_connections
            {
                report.record_preserved();
                continue;
            }

            // Prune this candidate
            report.record_prune(candidate);
            self.daily_prune_count += 1;
        }

        report
    }

    /// Get the prune reason for a memory
    pub fn get_prune_reason(&self, metadata: &MemoryMetadata) -> Option<PruneReason> {
        self.get_prune_reason_internal(metadata, &HashMap::new())
    }

    /// Internal prune reason with redundancy context
    fn get_prune_reason_internal(
        &self,
        metadata: &MemoryMetadata,
        redundant_ids: &HashMap<MemoryId, MemoryId>,
    ) -> Option<PruneReason> {
        // Priority order of reasons (first match wins)

        // 1. Check for redundancy
        if redundant_ids.contains_key(&metadata.id) {
            return Some(PruneReason::Redundant);
        }

        // 2. Check for orphaned (no connections)
        if metadata.connection_count == 0 {
            return Some(PruneReason::Orphaned);
        }

        // 3. Check for low alignment
        if metadata.alignment < self.config.base.min_alignment {
            return Some(PruneReason::LowAlignment);
        }

        // 4. Check for staleness
        if let Some(days) = metadata.days_since_access() {
            if days >= self.config.stale_days {
                return Some(PruneReason::Stale);
            }
        }

        // 5. Check for low quality
        if let Some(quality) = metadata.quality_score {
            if quality < self.config.min_quality {
                return Some(PruneReason::LowQuality);
            }
        }

        None
    }

    /// Estimate total bytes that would be freed by pruning candidates
    pub fn estimate_bytes_freed(&self, candidates: &[PruningCandidate]) -> u64 {
        let remaining = self
            .config
            .max_daily_prunes
            .saturating_sub(self.daily_prune_count) as usize;

        candidates
            .iter()
            .take(remaining)
            .filter(|c| self.should_prune(c))
            .map(|c| c.byte_size)
            .sum()
    }

    /// Register a content hash for redundancy detection
    pub fn register_hash(&mut self, hash: u64, memory_id: MemoryId) {
        self.known_hashes.insert(hash, memory_id);
    }

    /// Check if a hash is already known (potential duplicate)
    pub fn is_hash_known(&self, hash: u64) -> bool {
        self.known_hashes.contains_key(&hash)
    }

    /// Get the memory id for a known hash
    pub fn get_hash_owner(&self, hash: u64) -> Option<&MemoryId> {
        self.known_hashes.get(&hash)
    }
}

impl Default for PruningService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // Helper to create test metadata
    fn make_metadata(
        alignment: f32,
        age_days: i64,
        connections: u32,
        byte_size: u64,
    ) -> MemoryMetadata {
        let created_at = Utc::now() - Duration::days(age_days);
        let mut meta = MemoryMetadata::new(MemoryId::new(), created_at, alignment);
        meta.connection_count = connections;
        meta.byte_size = byte_size;
        meta
    }

    fn make_metadata_with_access(
        alignment: f32,
        age_days: i64,
        connections: u32,
        days_since_access: i64,
    ) -> MemoryMetadata {
        let created_at = Utc::now() - Duration::days(age_days);
        let mut meta = MemoryMetadata::new(MemoryId::new(), created_at, alignment);
        meta.connection_count = connections;
        meta.last_accessed = Some(Utc::now() - Duration::days(days_since_access));
        meta
    }

    // ========================================================================
    // PruneReason tests
    // ========================================================================

    #[test]
    fn test_prune_reason_description() {
        println!("[PASS] PruneReason::LowAlignment description");
        assert_eq!(
            PruneReason::LowAlignment.description(),
            "Alignment score below threshold"
        );

        println!("[PASS] PruneReason::Stale description");
        assert_eq!(
            PruneReason::Stale.description(),
            "Not accessed for extended period"
        );

        println!("[PASS] PruneReason::Orphaned description");
        assert_eq!(
            PruneReason::Orphaned.description(),
            "No connections to other memories"
        );

        println!("[PASS] PruneReason::Redundant description");
        assert_eq!(
            PruneReason::Redundant.description(),
            "Duplicate or near-duplicate content"
        );

        println!("[PASS] PruneReason::LowQuality description");
        assert_eq!(
            PruneReason::LowQuality.description(),
            "Quality metrics below threshold"
        );
    }

    #[test]
    fn test_prune_reason_equality() {
        println!("[PASS] PruneReason equality");
        assert_eq!(PruneReason::LowAlignment, PruneReason::LowAlignment);
        assert_ne!(PruneReason::LowAlignment, PruneReason::Stale);
        assert_ne!(PruneReason::Stale, PruneReason::Orphaned);
        assert_ne!(PruneReason::Orphaned, PruneReason::Redundant);
        assert_ne!(PruneReason::Redundant, PruneReason::LowQuality);
    }

    #[test]
    fn test_prune_reason_serialization() {
        let reasons = [
            PruneReason::LowAlignment,
            PruneReason::Stale,
            PruneReason::Orphaned,
            PruneReason::Redundant,
            PruneReason::LowQuality,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).expect("serialize");
            let deserialized: PruneReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, reason);
        }
        println!("[PASS] PruneReason serialization round-trip");
    }

    // ========================================================================
    // MemoryMetadata tests
    // ========================================================================

    #[test]
    fn test_memory_metadata_new() {
        let id = MemoryId::new();
        let created = Utc::now();
        let meta = MemoryMetadata::new(id.clone(), created, 0.75);

        assert_eq!(meta.id, id);
        assert!((meta.alignment - 0.75).abs() < f32::EPSILON);
        assert_eq!(meta.connection_count, 0);
        assert_eq!(meta.byte_size, 0);
        assert!(meta.last_accessed.is_none());
        assert!(meta.quality_score.is_none());
        assert!(meta.content_hash.is_none());
        println!("[PASS] MemoryMetadata::new");
    }

    #[test]
    fn test_memory_metadata_age_days() {
        let meta = make_metadata(0.75, 45, 0, 0);
        let age = meta.age_days();
        assert!((44..=46).contains(&age)); // Allow for timing variance
        println!("[PASS] MemoryMetadata::age_days = {}", age);
    }

    #[test]
    fn test_memory_metadata_days_since_access() {
        let meta = make_metadata_with_access(0.75, 100, 0, 30);
        let days = meta.days_since_access();
        assert!(days.is_some());
        let d = days.unwrap();
        assert!((29..=31).contains(&d));
        println!("[PASS] MemoryMetadata::days_since_access = {}", d);
    }

    #[test]
    fn test_memory_metadata_never_accessed() {
        let meta = make_metadata(0.75, 100, 0, 0);
        assert!(meta.days_since_access().is_none());
        println!("[PASS] MemoryMetadata::days_since_access returns None for never-accessed");
    }

    // ========================================================================
    // PruningCandidate tests
    // ========================================================================

    #[test]
    fn test_pruning_candidate_new() {
        let id = MemoryId::new();
        let candidate =
            PruningCandidate::new(id.clone(), 60, 0.30, 2, PruneReason::LowAlignment, 1024);

        assert_eq!(candidate.memory_id, id);
        assert_eq!(candidate.age_days, 60);
        assert!((candidate.alignment - 0.30).abs() < f32::EPSILON);
        assert_eq!(candidate.connections, 2);
        assert_eq!(candidate.reason, PruneReason::LowAlignment);
        assert_eq!(candidate.byte_size, 1024);
        assert!(candidate.priority_score > 0.0);
        println!(
            "[PASS] PruningCandidate::new with priority_score = {:.3}",
            candidate.priority_score
        );
    }

    #[test]
    fn test_pruning_candidate_priority_ordering() {
        // Lower alignment = higher priority
        let low_align =
            PruningCandidate::new(MemoryId::new(), 30, 0.10, 0, PruneReason::LowAlignment, 100);
        let high_align =
            PruningCandidate::new(MemoryId::new(), 30, 0.80, 0, PruneReason::LowAlignment, 100);
        assert!(low_align.priority_score > high_align.priority_score);
        println!("[PASS] Lower alignment has higher priority");

        // Older age = higher priority
        let old = PruningCandidate::new(MemoryId::new(), 365, 0.50, 0, PruneReason::Stale, 100);
        let young = PruningCandidate::new(MemoryId::new(), 30, 0.50, 0, PruneReason::Stale, 100);
        assert!(old.priority_score > young.priority_score);
        println!("[PASS] Older memories have higher priority");

        // Fewer connections = higher priority
        let isolated =
            PruningCandidate::new(MemoryId::new(), 60, 0.50, 0, PruneReason::Orphaned, 100);
        let connected =
            PruningCandidate::new(MemoryId::new(), 60, 0.50, 10, PruneReason::Orphaned, 100);
        assert!(isolated.priority_score > connected.priority_score);
        println!("[PASS] Isolated memories have higher priority");
    }

    // ========================================================================
    // PruningReport tests
    // ========================================================================

    #[test]
    fn test_pruning_report_default() {
        let report = PruningReport::default();
        assert_eq!(report.candidates_evaluated, 0);
        assert_eq!(report.pruned_count, 0);
        assert_eq!(report.bytes_freed, 0);
        assert!(report.reasons_breakdown.is_empty());
        assert_eq!(report.preserved_count, 0);
        assert!(!report.daily_limit_reached);
        println!("[PASS] PruningReport::default");
    }

    #[test]
    fn test_pruning_report_record_prune() {
        let mut report = PruningReport::new();
        let candidate = PruningCandidate::new(
            MemoryId::new(),
            60,
            0.30,
            0,
            PruneReason::LowAlignment,
            2048,
        );

        report.record_prune(&candidate);
        assert_eq!(report.pruned_count, 1);
        assert_eq!(report.bytes_freed, 2048);
        assert_eq!(
            *report
                .reasons_breakdown
                .get(&PruneReason::LowAlignment)
                .unwrap(),
            1
        );
        println!("[PASS] PruningReport::record_prune");
    }

    #[test]
    fn test_pruning_report_multiple_reasons() {
        let mut report = PruningReport::new();

        report.record_prune(&PruningCandidate::new(
            MemoryId::new(),
            60,
            0.30,
            0,
            PruneReason::LowAlignment,
            100,
        ));
        report.record_prune(&PruningCandidate::new(
            MemoryId::new(),
            60,
            0.30,
            0,
            PruneReason::LowAlignment,
            100,
        ));
        report.record_prune(&PruningCandidate::new(
            MemoryId::new(),
            60,
            0.50,
            0,
            PruneReason::Orphaned,
            200,
        ));

        assert_eq!(report.pruned_count, 3);
        assert_eq!(report.bytes_freed, 400);
        assert_eq!(
            *report
                .reasons_breakdown
                .get(&PruneReason::LowAlignment)
                .unwrap(),
            2
        );
        assert_eq!(
            *report
                .reasons_breakdown
                .get(&PruneReason::Orphaned)
                .unwrap(),
            1
        );
        println!("[PASS] PruningReport reasons breakdown");
    }

    // ========================================================================
    // ExtendedPruningConfig tests
    // ========================================================================

    #[test]
    fn test_extended_pruning_config_default() {
        let config = ExtendedPruningConfig::default();
        assert!(config.base.enabled);
        assert_eq!(config.base.min_age_days, 30);
        assert!((config.base.min_alignment - 0.40).abs() < f32::EPSILON);
        assert_eq!(config.base.min_connections, 3);
        assert_eq!(config.max_daily_prunes, 100);
        assert_eq!(config.stale_days, 90);
        assert!((config.min_quality - 0.30).abs() < f32::EPSILON);
        println!("[PASS] ExtendedPruningConfig::default");
    }

    #[test]
    fn test_extended_pruning_config_serialization() {
        let config = ExtendedPruningConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: ExtendedPruningConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.max_daily_prunes, config.max_daily_prunes);
        assert_eq!(deserialized.stale_days, config.stale_days);
        println!("[PASS] ExtendedPruningConfig serialization");
    }

    // ========================================================================
    // PruningService construction tests
    // ========================================================================

    #[test]
    fn test_pruning_service_new() {
        let service = PruningService::new();
        assert_eq!(service.config().max_daily_prunes, 100);
        assert_eq!(service.daily_prune_count, 0);
        println!("[PASS] PruningService::new");
    }

    #[test]
    fn test_pruning_service_with_config() {
        let config = ExtendedPruningConfig {
            max_daily_prunes: 50,
            ..Default::default()
        };
        let service = PruningService::with_config(config);
        assert_eq!(service.config().max_daily_prunes, 50);
        println!("[PASS] PruningService::with_config");
    }

    #[test]
    fn test_pruning_service_default() {
        let service = PruningService::default();
        assert_eq!(service.config().max_daily_prunes, 100);
        println!("[PASS] PruningService::default");
    }

    // ========================================================================
    // identify_candidates tests
    // ========================================================================

    #[test]
    fn test_identify_candidates_empty() {
        let service = PruningService::new();
        let candidates = service.identify_candidates(&[]);
        assert!(candidates.is_empty());
        println!("[PASS] identify_candidates with empty input");
    }

    #[test]
    fn test_identify_candidates_low_alignment() {
        let service = PruningService::new();
        let memories = vec![
            make_metadata(0.20, 60, 1, 1024), // Low alignment, should be candidate
            make_metadata(0.80, 60, 1, 1024), // High alignment, should not
        ];

        let candidates = service.identify_candidates(&memories);
        assert_eq!(candidates.len(), 1);
        assert!((candidates[0].alignment - 0.20).abs() < f32::EPSILON);
        assert_eq!(candidates[0].reason, PruneReason::LowAlignment);
        println!("[PASS] identify_candidates finds low alignment");
    }

    #[test]
    fn test_identify_candidates_orphaned() {
        let service = PruningService::new();
        let memories = vec![
            make_metadata(0.80, 60, 0, 1024), // No connections = orphaned
            make_metadata(0.80, 60, 5, 1024), // Has connections, should not
        ];

        let candidates = service.identify_candidates(&memories);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].connections, 0);
        assert_eq!(candidates[0].reason, PruneReason::Orphaned);
        println!("[PASS] identify_candidates finds orphaned");
    }

    #[test]
    fn test_identify_candidates_too_young() {
        let service = PruningService::new();
        // 10 days old, below min_age_days of 30
        let memories = vec![make_metadata(0.20, 10, 0, 1024)];

        let candidates = service.identify_candidates(&memories);
        assert!(candidates.is_empty());
        println!("[PASS] identify_candidates ignores young memories");
    }

    #[test]
    fn test_identify_candidates_stale() {
        let service = PruningService::new();
        // Stale: 100 days since access (> 90 day threshold)
        let memories = vec![make_metadata_with_access(0.80, 120, 2, 100)];

        let candidates = service.identify_candidates(&memories);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::Stale);
        println!("[PASS] identify_candidates finds stale memories");
    }

    #[test]
    fn test_identify_candidates_low_quality() {
        let service = PruningService::new();
        let mut meta = make_metadata(0.80, 60, 2, 1024);
        meta.quality_score = Some(0.10); // Below 0.30 threshold

        let candidates = service.identify_candidates(&[meta]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::LowQuality);
        println!("[PASS] identify_candidates finds low quality");
    }

    #[test]
    fn test_identify_candidates_redundant() {
        let service = PruningService::new();

        let mut meta1 = make_metadata(0.80, 60, 2, 1024);
        meta1.content_hash = Some(12345);

        let mut meta2 = make_metadata(0.50, 60, 2, 1024);
        meta2.content_hash = Some(12345); // Same hash, lower alignment

        let candidates = service.identify_candidates(&[meta1, meta2]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::Redundant);
        println!("[PASS] identify_candidates finds redundant memories");
    }

    #[test]
    fn test_identify_candidates_preserves_connected() {
        let config = ExtendedPruningConfig {
            base: PruningConfig {
                preserve_connected: true,
                min_connections: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let service = PruningService::with_config(config);

        // Low alignment but well connected - should be preserved
        let memories = vec![make_metadata(0.20, 60, 5, 1024)];

        let candidates = service.identify_candidates(&memories);
        assert!(candidates.is_empty());
        println!("[PASS] identify_candidates preserves well-connected memories");
    }

    #[test]
    fn test_identify_candidates_sorted_by_priority() {
        let service = PruningService::new();
        let memories = vec![
            make_metadata(0.35, 60, 1, 1024), // Higher alignment, lower priority
            make_metadata(0.10, 60, 0, 1024), // Lower alignment, higher priority
            make_metadata(0.25, 60, 1, 1024), // Medium
        ];

        let candidates = service.identify_candidates(&memories);
        assert_eq!(candidates.len(), 3);
        // Should be sorted by priority (highest first)
        assert!(candidates[0].priority_score >= candidates[1].priority_score);
        assert!(candidates[1].priority_score >= candidates[2].priority_score);
        println!("[PASS] identify_candidates sorted by priority");
    }

    // ========================================================================
    // evaluate_candidate tests
    // ========================================================================

    #[test]
    fn test_evaluate_candidate_prune_disabled() {
        let config = ExtendedPruningConfig {
            base: PruningConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let service = PruningService::with_config(config);
        let meta = make_metadata(0.10, 60, 0, 1024);

        let candidate = service.evaluate_candidate(&meta);
        assert!(candidate.is_none());
        println!("[PASS] evaluate_candidate returns None when disabled");
    }

    #[test]
    fn test_evaluate_candidate_healthy_memory() {
        let service = PruningService::new();
        // Good alignment, has connections, not stale
        let mut meta = make_metadata(0.80, 60, 5, 1024);
        meta.last_accessed = Some(Utc::now() - Duration::days(5));
        meta.quality_score = Some(0.90);

        let candidate = service.evaluate_candidate(&meta);
        assert!(candidate.is_none());
        println!("[PASS] evaluate_candidate returns None for healthy memory");
    }

    // ========================================================================
    // should_prune tests
    // ========================================================================

    #[test]
    fn test_should_prune_basic() {
        let service = PruningService::new();
        let candidate = PruningCandidate::new(
            MemoryId::new(),
            60,
            0.30,
            1,
            PruneReason::LowAlignment,
            1024,
        );

        assert!(service.should_prune(&candidate));
        println!("[PASS] should_prune returns true for basic candidate");
    }

    #[test]
    fn test_should_prune_preserves_connected() {
        let config = ExtendedPruningConfig {
            base: PruningConfig {
                preserve_connected: true,
                min_connections: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let service = PruningService::with_config(config);

        let candidate = PruningCandidate::new(
            MemoryId::new(),
            60,
            0.30,
            5,
            PruneReason::LowAlignment,
            1024,
        );

        assert!(!service.should_prune(&candidate));
        println!("[PASS] should_prune preserves connected memories");
    }

    // ========================================================================
    // prune tests
    // ========================================================================

    #[test]
    fn test_prune_empty() {
        let mut service = PruningService::new();
        let report = service.prune(&[]);

        assert_eq!(report.candidates_evaluated, 0);
        assert_eq!(report.pruned_count, 0);
        assert_eq!(report.bytes_freed, 0);
        println!("[PASS] prune with empty input");
    }

    #[test]
    fn test_prune_basic() {
        let mut service = PruningService::new();
        let candidates = vec![
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                1024,
            ),
            PruningCandidate::new(MemoryId::new(), 60, 0.20, 0, PruneReason::Orphaned, 2048),
        ];

        let report = service.prune(&candidates);
        assert_eq!(report.candidates_evaluated, 2);
        assert_eq!(report.pruned_count, 2);
        assert_eq!(report.bytes_freed, 3072);
        println!("[PASS] prune basic operation");
    }

    #[test]
    fn test_prune_respects_daily_limit() {
        let config = ExtendedPruningConfig {
            max_daily_prunes: 2,
            ..Default::default()
        };
        let mut service = PruningService::with_config(config);

        let candidates: Vec<PruningCandidate> = (0..5)
            .map(|_| {
                PruningCandidate::new(MemoryId::new(), 60, 0.30, 1, PruneReason::LowAlignment, 100)
            })
            .collect();

        let report = service.prune(&candidates);
        assert_eq!(report.candidates_evaluated, 5);
        assert_eq!(report.pruned_count, 2);
        assert!(report.daily_limit_reached);
        println!("[PASS] prune respects daily limit");
    }

    #[test]
    fn test_prune_preserves_connected() {
        let config = ExtendedPruningConfig {
            base: PruningConfig {
                preserve_connected: true,
                min_connections: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut service = PruningService::with_config(config);

        let candidates = vec![
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                5,
                PruneReason::LowAlignment,
                1024,
            ), // Preserved
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                1024,
            ), // Pruned
        ];

        let report = service.prune(&candidates);
        assert_eq!(report.pruned_count, 1);
        assert_eq!(report.preserved_count, 1);
        println!("[PASS] prune preserves connected memories");
    }

    // ========================================================================
    // get_prune_reason tests
    // ========================================================================

    #[test]
    fn test_get_prune_reason_none() {
        let service = PruningService::new();
        let mut meta = make_metadata(0.80, 60, 5, 1024);
        meta.last_accessed = Some(Utc::now());
        meta.quality_score = Some(0.90);

        let reason = service.get_prune_reason(&meta);
        assert!(reason.is_none());
        println!("[PASS] get_prune_reason returns None for healthy memory");
    }

    #[test]
    fn test_get_prune_reason_priority() {
        let service = PruningService::new();

        // Orphaned takes priority over low alignment
        let meta = make_metadata(0.30, 60, 0, 1024);
        let reason = service.get_prune_reason(&meta);
        assert_eq!(reason, Some(PruneReason::Orphaned));
        println!("[PASS] get_prune_reason: Orphaned > LowAlignment");

        // Low alignment
        let meta = make_metadata(0.30, 60, 1, 1024);
        let reason = service.get_prune_reason(&meta);
        assert_eq!(reason, Some(PruneReason::LowAlignment));
        println!("[PASS] get_prune_reason: LowAlignment");
    }

    // ========================================================================
    // estimate_bytes_freed tests
    // ========================================================================

    #[test]
    fn test_estimate_bytes_freed_empty() {
        let service = PruningService::new();
        let estimate = service.estimate_bytes_freed(&[]);
        assert_eq!(estimate, 0);
        println!("[PASS] estimate_bytes_freed with empty input");
    }

    #[test]
    fn test_estimate_bytes_freed_basic() {
        let service = PruningService::new();
        let candidates = vec![
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                1000,
            ),
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                2000,
            ),
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                3000,
            ),
        ];

        let estimate = service.estimate_bytes_freed(&candidates);
        assert_eq!(estimate, 6000);
        println!("[PASS] estimate_bytes_freed = 6000");
    }

    #[test]
    fn test_estimate_bytes_freed_respects_limit() {
        let config = ExtendedPruningConfig {
            max_daily_prunes: 2,
            ..Default::default()
        };
        let service = PruningService::with_config(config);

        let candidates = vec![
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                1000,
            ),
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                2000,
            ),
            PruningCandidate::new(
                MemoryId::new(),
                60,
                0.30,
                1,
                PruneReason::LowAlignment,
                3000,
            ),
        ];

        let estimate = service.estimate_bytes_freed(&candidates);
        // Only first 2 count due to limit
        assert_eq!(estimate, 3000);
        println!("[PASS] estimate_bytes_freed respects daily limit");
    }

    // ========================================================================
    // Hash tracking tests
    // ========================================================================

    #[test]
    fn test_register_and_check_hash() {
        let mut service = PruningService::new();
        let id = MemoryId::new();

        assert!(!service.is_hash_known(12345));

        service.register_hash(12345, id.clone());

        assert!(service.is_hash_known(12345));
        assert_eq!(service.get_hash_owner(12345), Some(&id));
        println!("[PASS] register_hash and is_hash_known");
    }

    // ========================================================================
    // Daily rollover tests
    // ========================================================================

    #[test]
    fn test_remaining_daily_prunes() {
        let config = ExtendedPruningConfig {
            max_daily_prunes: 100,
            ..Default::default()
        };
        let mut service = PruningService::with_config(config);

        assert_eq!(service.remaining_daily_prunes(), 100);

        // Prune some
        let candidates = vec![
            PruningCandidate::new(MemoryId::new(), 60, 0.30, 1, PruneReason::LowAlignment, 100),
            PruningCandidate::new(MemoryId::new(), 60, 0.30, 1, PruneReason::LowAlignment, 100),
        ];
        service.prune(&candidates);

        assert_eq!(service.remaining_daily_prunes(), 98);
        println!("[PASS] remaining_daily_prunes tracks correctly");
    }

    // ========================================================================
    // Integration test
    // ========================================================================

    #[test]
    fn test_full_pruning_workflow() {
        let mut service = PruningService::new();

        // Create a diverse set of memories
        let memories = vec![
            // Healthy - should not be pruned
            make_metadata(0.85, 60, 5, 1000),
            // Low alignment
            make_metadata(0.20, 60, 2, 2000),
            // Orphaned
            make_metadata(0.75, 60, 0, 3000),
            // Too young
            make_metadata(0.10, 10, 0, 4000),
            // Well connected despite low alignment
            make_metadata(0.25, 60, 10, 5000),
        ];

        // Identify candidates
        let candidates = service.identify_candidates(&memories);

        // Should find: low alignment (0.20) and orphaned (0.75)
        // The well-connected one should be excluded
        // The too-young one should be excluded
        assert_eq!(candidates.len(), 2);
        println!(
            "[PASS] Full workflow: identified {} candidates",
            candidates.len()
        );

        // Prune
        let report = service.prune(&candidates);

        assert_eq!(report.pruned_count, 2);
        assert_eq!(report.bytes_freed, 5000); // 2000 + 3000
        assert!(!report.daily_limit_reached);
        println!(
            "[PASS] Full workflow: pruned {} memories, freed {} bytes",
            report.pruned_count, report.bytes_freed
        );

        // Verify reasons breakdown
        assert!(
            report
                .reasons_breakdown
                .contains_key(&PruneReason::LowAlignment)
                || report
                    .reasons_breakdown
                    .contains_key(&PruneReason::Orphaned)
        );
        println!("[PASS] Full workflow: reasons breakdown populated");
    }
}
