//! Temporal dataset generation for benchmarking E2/E3/E4 embedders.
//!
//! This module generates synthetic datasets with known temporal ground truth for
//! evaluating recency (E2), periodic (E3), and sequence (E4) embedder effectiveness.
//!
//! ## Dataset Types
//!
//! - **Recency Dataset**: Memories distributed across time with known freshness labels
//! - **Periodic Dataset**: Memories with hour-of-day and day-of-week patterns
//! - **Sequence Dataset**: Ordered conversation chains with known before/after relationships
//!
//! ## Ground Truth
//!
//! Each dataset includes ground truth for:
//! - Recency queries: Which memories should rank higher based on freshness
//! - Periodic queries: Which memories from the same hour/day should be retrieved
//! - Sequence queries: Before/after relationships for anchor memories

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Configuration for temporal dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDatasetConfig {
    /// Total number of memories to generate.
    pub num_memories: usize,

    /// Number of temporal queries to generate.
    pub num_queries: usize,

    /// Time span in days for memory distribution.
    pub time_span_days: u32,

    /// Base timestamp for dataset (default: now).
    pub base_timestamp: Option<DateTime<Utc>>,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Configuration for recency distribution.
    pub recency_config: RecencyDistributionConfig,

    /// Configuration for periodic patterns.
    pub periodic_config: PeriodicPatternConfig,

    /// Configuration for sequence chains.
    pub sequence_config: SequenceChainConfig,
}

impl Default for TemporalDatasetConfig {
    fn default() -> Self {
        Self {
            num_memories: 1000,
            num_queries: 100,
            time_span_days: 30,
            base_timestamp: None,
            seed: 42,
            recency_config: RecencyDistributionConfig::default(),
            periodic_config: PeriodicPatternConfig::default(),
            sequence_config: SequenceChainConfig::default(),
        }
    }
}

/// Configuration for recency distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecencyDistributionConfig {
    /// Distribution type: "uniform", "exponential", "clustered".
    pub distribution: String,

    /// For exponential: decay rate (higher = more recent items).
    pub decay_rate: f64,

    /// For clustered: number of time clusters.
    pub num_clusters: usize,

    /// Fraction of memories considered "fresh" for ground truth.
    pub fresh_fraction: f64,

    /// Fresh threshold in hours.
    pub fresh_threshold_hours: u32,
}

impl Default for RecencyDistributionConfig {
    fn default() -> Self {
        Self {
            distribution: "exponential".to_string(),
            decay_rate: 2.0,
            num_clusters: 5,
            fresh_fraction: 0.2,
            fresh_threshold_hours: 24,
        }
    }
}

/// Configuration for periodic patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodicPatternConfig {
    /// Peak hours for activity (0-23).
    pub peak_hours: Vec<u8>,

    /// Peak days for activity (0-6, 0=Sunday).
    pub peak_days: Vec<u8>,

    /// Concentration factor (higher = more concentrated in peaks).
    pub concentration: f64,

    /// Enable weekly patterns.
    pub enable_weekly: bool,

    /// Enable daily patterns.
    pub enable_daily: bool,
}

impl Default for PeriodicPatternConfig {
    fn default() -> Self {
        Self {
            peak_hours: vec![9, 10, 11, 14, 15, 16], // Work hours
            peak_days: vec![1, 2, 3, 4, 5],          // Weekdays
            concentration: 3.0,
            enable_weekly: true,
            enable_daily: true,
        }
    }
}

/// Configuration for sequence chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceChainConfig {
    /// Number of conversation chains.
    pub num_chains: usize,

    /// Average chain length.
    pub avg_chain_length: usize,

    /// Variance in chain length.
    pub length_variance: usize,

    /// Gap between chain items in minutes.
    pub avg_gap_minutes: u32,

    /// Session boundary gap in hours.
    pub session_gap_hours: u32,
}

impl Default for SequenceChainConfig {
    fn default() -> Self {
        Self {
            num_chains: 50,
            avg_chain_length: 10,
            length_variance: 5,
            avg_gap_minutes: 5,
            session_gap_hours: 4,
        }
    }
}

/// Complete temporal benchmark dataset.
#[derive(Debug)]
pub struct TemporalBenchmarkDataset {
    /// All generated memories with timestamps.
    pub memories: Vec<TemporalMemory>,

    /// Recency-specific queries with ground truth.
    pub recency_queries: Vec<RecencyQuery>,

    /// Periodic-specific queries with ground truth.
    pub periodic_queries: Vec<PeriodicQuery>,

    /// Sequence-specific queries with ground truth.
    pub sequence_queries: Vec<SequenceQuery>,

    /// Session/episode boundaries.
    pub episode_boundaries: Vec<usize>,

    /// Configuration used.
    pub config: TemporalDatasetConfig,
}

/// A memory with temporal information.
#[derive(Debug, Clone)]
pub struct TemporalMemory {
    /// Unique ID.
    pub id: Uuid,

    /// Content text.
    pub content: String,

    /// Creation timestamp.
    pub timestamp: DateTime<Utc>,

    /// Hour of day (0-23).
    pub hour: u8,

    /// Day of week (0-6).
    pub day_of_week: u8,

    /// Chain ID (for sequence queries).
    pub chain_id: Option<usize>,

    /// Position in chain.
    pub chain_position: Option<usize>,

    /// Session ID.
    pub session_id: String,

    /// Is this a session boundary marker?
    pub is_boundary: bool,
}

/// Recency query with ground truth.
#[derive(Debug, Clone)]
pub struct RecencyQuery {
    /// Query ID.
    pub id: Uuid,

    /// Query timestamp (when the query is made).
    pub query_timestamp: DateTime<Utc>,

    /// Ground truth: IDs of fresh memories (should rank higher).
    pub fresh_memory_ids: HashSet<Uuid>,

    /// Ground truth: IDs ranked by recency (most recent first).
    pub recency_ranked_ids: Vec<Uuid>,

    /// Fresh threshold in milliseconds.
    pub fresh_threshold_ms: i64,
}

/// Periodic query with ground truth.
#[derive(Debug, Clone)]
pub struct PeriodicQuery {
    /// Query ID.
    pub id: Uuid,

    /// Target hour (0-23).
    pub target_hour: u8,

    /// Target day (0-6).
    pub target_day: Option<u8>,

    /// Ground truth: IDs of memories from same hour.
    pub same_hour_ids: HashSet<Uuid>,

    /// Ground truth: IDs of memories from same day.
    pub same_day_ids: HashSet<Uuid>,
}

/// Sequence query with ground truth.
#[derive(Debug, Clone)]
pub struct SequenceQuery {
    /// Query ID.
    pub id: Uuid,

    /// Anchor memory ID.
    pub anchor_id: Uuid,

    /// Anchor timestamp.
    pub anchor_timestamp: DateTime<Utc>,

    /// Anchor's position in the chain (session sequence).
    pub anchor_sequence: Option<usize>,

    /// Direction: "before", "after", "both".
    pub direction: String,

    /// Ground truth: IDs that should be retrieved (in temporal order).
    pub expected_ids: Vec<Uuid>,

    /// Chain this belongs to.
    pub chain_id: usize,
}

/// Generator for temporal benchmark datasets.
pub struct TemporalDatasetGenerator {
    config: TemporalDatasetConfig,
    rng: ChaCha8Rng,
}

impl TemporalDatasetGenerator {
    /// Create a new generator with the given configuration.
    pub fn new(config: TemporalDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate a complete temporal benchmark dataset.
    pub fn generate(&mut self) -> TemporalBenchmarkDataset {
        let base_ts = self.config.base_timestamp.unwrap_or_else(Utc::now);

        // Generate memories with temporal distribution
        let mut memories = self.generate_memories(base_ts);

        // Generate sequence chains and update memories
        let chains = self.generate_chains(&mut memories);

        // Identify episode boundaries
        let episode_boundaries = self.identify_boundaries(&memories);

        // Generate queries
        let recency_queries = self.generate_recency_queries(&memories, base_ts);
        let periodic_queries = self.generate_periodic_queries(&memories);
        let sequence_queries = self.generate_sequence_queries(&memories, &chains);

        TemporalBenchmarkDataset {
            memories,
            recency_queries,
            periodic_queries,
            sequence_queries,
            episode_boundaries,
            config: self.config.clone(),
        }
    }

    fn generate_memories(&mut self, base_ts: DateTime<Utc>) -> Vec<TemporalMemory> {
        let mut memories = Vec::with_capacity(self.config.num_memories);
        let time_span_ms = self.config.time_span_days as i64 * 24 * 60 * 60 * 1000;

        for i in 0..self.config.num_memories {
            // Generate timestamp based on distribution
            let age_ms = match self.config.recency_config.distribution.as_str() {
                "uniform" => self.rng.gen_range(0..time_span_ms),
                "exponential" => {
                    let rate = self.config.recency_config.decay_rate;
                    let u: f64 = self.rng.gen_range(0.0..1.0);
                    let exp_sample = -u.ln() / rate;
                    ((exp_sample * time_span_ms as f64 / 5.0) as i64).min(time_span_ms)
                }
                "clustered" => {
                    let cluster = self
                        .rng
                        .gen_range(0..self.config.recency_config.num_clusters);
                    let cluster_center = time_span_ms * cluster as i64
                        / self.config.recency_config.num_clusters as i64;
                    let noise = self
                        .rng
                        .gen_range(-(time_span_ms / 20)..(time_span_ms / 20));
                    (cluster_center + noise).clamp(0, time_span_ms)
                }
                _ => self.rng.gen_range(0..time_span_ms),
            };

            // Apply periodic patterns
            let timestamp = self.apply_periodic_pattern(base_ts - Duration::milliseconds(age_ms));

            let hour = timestamp.hour() as u8;
            let day_of_week = timestamp.weekday().num_days_from_sunday() as u8;

            let session_id = format!("session-{}", i / 20); // ~20 memories per session

            memories.push(TemporalMemory {
                id: Uuid::new_v4(),
                content: format!("Memory content {} created at {}", i, timestamp),
                timestamp,
                hour,
                day_of_week,
                chain_id: None,
                chain_position: None,
                session_id,
                is_boundary: false,
            });
        }

        // Sort by timestamp
        memories.sort_by_key(|m| m.timestamp);

        memories
    }

    fn apply_periodic_pattern(&mut self, base_ts: DateTime<Utc>) -> DateTime<Utc> {
        if !self.config.periodic_config.enable_daily && !self.config.periodic_config.enable_weekly {
            return base_ts;
        }

        let concentration = self.config.periodic_config.concentration;

        // Apply hourly concentration
        let hour = if self.config.periodic_config.enable_daily
            && !self.config.periodic_config.peak_hours.is_empty()
        {
            let random_val: f64 = self.rng.gen_range(0.0..1.0);
            if random_val < concentration / (concentration + 1.0) {
                // Pick a peak hour
                let idx = self
                    .rng
                    .gen_range(0..self.config.periodic_config.peak_hours.len());
                self.config.periodic_config.peak_hours[idx]
            } else {
                // Random hour
                self.rng.gen_range(0..24)
            }
        } else {
            base_ts.hour() as u8
        };

        // Apply day-of-week concentration
        let target_day = if self.config.periodic_config.enable_weekly
            && !self.config.periodic_config.peak_days.is_empty()
        {
            let random_day_val: f64 = self.rng.gen_range(0.0..1.0);
            if random_day_val < concentration / (concentration + 1.0) {
                let idx = self
                    .rng
                    .gen_range(0..self.config.periodic_config.peak_days.len());
                self.config.periodic_config.peak_days[idx]
            } else {
                self.rng.gen_range(0..7)
            }
        } else {
            base_ts.weekday().num_days_from_sunday() as u8
        };

        // Adjust timestamp to match target hour and day
        let current_day = base_ts.weekday().num_days_from_sunday() as i64;
        let day_diff = target_day as i64 - current_day;
        let hour_diff = hour as i64 - base_ts.hour() as i64;

        base_ts
            + Duration::days(day_diff)
            + Duration::hours(hour_diff)
            + Duration::minutes(self.rng.gen_range(0..60))
    }

    fn generate_chains(&mut self, memories: &mut [TemporalMemory]) -> Vec<Vec<usize>> {
        let mut chains: Vec<Vec<usize>> = Vec::new();
        let mut used_indices: HashSet<usize> = HashSet::new();

        for chain_idx in 0..self.config.sequence_config.num_chains {
            let chain_length = (self.config.sequence_config.avg_chain_length as i32
                + self.rng.gen_range(
                    -(self.config.sequence_config.length_variance as i32)
                        ..=(self.config.sequence_config.length_variance as i32),
                ))
            .max(3) as usize;

            // Find contiguous memories for this chain
            let start_idx = self
                .rng
                .gen_range(0..memories.len().saturating_sub(chain_length));
            let mut chain: Vec<usize> = Vec::new();

            for idx in start_idx..(start_idx + chain_length).min(memories.len()) {
                if !used_indices.contains(&idx) {
                    chain.push(idx);
                    used_indices.insert(idx);
                }
            }

            if chain.len() >= 3 {
                // Update memories with chain info
                for (pos, &idx) in chain.iter().enumerate() {
                    memories[idx].chain_id = Some(chain_idx);
                    memories[idx].chain_position = Some(pos);

                    // Mark session boundaries
                    if pos == 0 {
                        memories[idx].is_boundary = true;
                    }
                }

                chains.push(chain);
            }
        }

        chains
    }

    fn identify_boundaries(&self, memories: &[TemporalMemory]) -> Vec<usize> {
        let mut boundaries = Vec::new();
        let gap_threshold = Duration::hours(self.config.sequence_config.session_gap_hours as i64);

        for i in 1..memories.len() {
            let gap = memories[i].timestamp - memories[i - 1].timestamp;
            if gap > gap_threshold || memories[i].is_boundary {
                boundaries.push(i);
            }
        }

        boundaries
    }

    fn generate_recency_queries(
        &mut self,
        memories: &[TemporalMemory],
        base_ts: DateTime<Utc>,
    ) -> Vec<RecencyQuery> {
        let num_recency_queries = self.config.num_queries / 3;
        let fresh_threshold =
            Duration::hours(self.config.recency_config.fresh_threshold_hours as i64);

        (0..num_recency_queries)
            .map(|_| {
                let query_ts = base_ts - Duration::hours(self.rng.gen_range(0..24));
                let threshold_ts = query_ts - fresh_threshold;

                // Find fresh memories
                let fresh_ids: HashSet<Uuid> = memories
                    .iter()
                    .filter(|m| m.timestamp >= threshold_ts && m.timestamp <= query_ts)
                    .map(|m| m.id)
                    .collect();

                // Rank by recency
                let mut ranked: Vec<_> = memories
                    .iter()
                    .filter(|m| m.timestamp <= query_ts)
                    .collect();
                ranked.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                let recency_ranked_ids: Vec<Uuid> = ranked.iter().map(|m| m.id).collect();

                RecencyQuery {
                    id: Uuid::new_v4(),
                    query_timestamp: query_ts,
                    fresh_memory_ids: fresh_ids,
                    recency_ranked_ids,
                    fresh_threshold_ms: fresh_threshold.num_milliseconds(),
                }
            })
            .collect()
    }

    fn generate_periodic_queries(&mut self, memories: &[TemporalMemory]) -> Vec<PeriodicQuery> {
        let num_periodic_queries = self.config.num_queries / 3;

        (0..num_periodic_queries)
            .map(|_| {
                let target_hour = self.rng.gen_range(0..24);
                let target_day = if self.rng.gen_bool(0.5) {
                    Some(self.rng.gen_range(0..7))
                } else {
                    None
                };

                let same_hour_ids: HashSet<Uuid> = memories
                    .iter()
                    .filter(|m| m.hour == target_hour)
                    .map(|m| m.id)
                    .collect();

                let same_day_ids: HashSet<Uuid> = if let Some(day) = target_day {
                    memories
                        .iter()
                        .filter(|m| m.day_of_week == day)
                        .map(|m| m.id)
                        .collect()
                } else {
                    HashSet::new()
                };

                PeriodicQuery {
                    id: Uuid::new_v4(),
                    target_hour,
                    target_day,
                    same_hour_ids,
                    same_day_ids,
                }
            })
            .collect()
    }

    fn generate_sequence_queries(
        &mut self,
        memories: &[TemporalMemory],
        chains: &[Vec<usize>],
    ) -> Vec<SequenceQuery> {
        let num_sequence_queries = self.config.num_queries / 3;
        let mut queries = Vec::new();

        for _ in 0..num_sequence_queries {
            if chains.is_empty() {
                continue;
            }

            let chain_idx = self.rng.gen_range(0..chains.len());
            let chain = &chains[chain_idx];

            if chain.len() < 3 {
                continue;
            }

            // Pick an anchor position (not first or last)
            let anchor_pos = self.rng.gen_range(1..chain.len() - 1);
            let anchor_mem_idx = chain[anchor_pos];
            let anchor = &memories[anchor_mem_idx];

            let direction = match self.rng.gen_range(0..3) {
                0 => "before",
                1 => "after",
                _ => "both",
            };

            let expected_ids: Vec<Uuid> = match direction {
                "before" => chain[..anchor_pos]
                    .iter()
                    .map(|&idx| memories[idx].id)
                    .collect(),
                "after" => chain[anchor_pos + 1..]
                    .iter()
                    .map(|&idx| memories[idx].id)
                    .collect(),
                _ => chain
                    .iter()
                    .filter(|&&idx| idx != anchor_mem_idx)
                    .map(|&idx| memories[idx].id)
                    .collect(),
            };

            queries.push(SequenceQuery {
                id: Uuid::new_v4(),
                anchor_id: anchor.id,
                anchor_timestamp: anchor.timestamp,
                anchor_sequence: anchor.chain_position,
                direction: direction.to_string(),
                expected_ids,
                chain_id: chain_idx,
            });
        }

        queries
    }
}

impl TemporalBenchmarkDataset {
    /// Get memory by ID.
    pub fn get_memory(&self, id: &Uuid) -> Option<&TemporalMemory> {
        self.memories.iter().find(|m| &m.id == id)
    }

    /// Get memories for a specific chain.
    pub fn get_chain_memories(&self, chain_id: usize) -> Vec<&TemporalMemory> {
        self.memories
            .iter()
            .filter(|m| m.chain_id == Some(chain_id))
            .collect()
    }

    /// Get timestamp in milliseconds for a memory.
    pub fn timestamp_ms(&self, id: &Uuid) -> Option<i64> {
        self.get_memory(id).map(|m| m.timestamp.timestamp_millis())
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check all chain positions are valid
        for memory in &self.memories {
            if let (Some(chain_id), Some(pos)) = (memory.chain_id, memory.chain_position) {
                let chain_size = self
                    .memories
                    .iter()
                    .filter(|m| m.chain_id == Some(chain_id))
                    .count();
                if pos >= chain_size {
                    return Err(format!(
                        "Memory {} has invalid chain position {} (chain size {})",
                        memory.id, pos, chain_size
                    ));
                }
            }
        }

        // Check recency queries have fresh memories
        for query in &self.recency_queries {
            if query.fresh_memory_ids.is_empty() {
                return Err(format!("Recency query {} has no fresh memories", query.id));
            }
        }

        // Check periodic queries have same-hour memories
        for query in &self.periodic_queries {
            if query.same_hour_ids.is_empty() {
                return Err(format!(
                    "Periodic query {} for hour {} has no matching memories",
                    query.id, query.target_hour
                ));
            }
        }

        // Check sequence queries have expected IDs
        for query in &self.sequence_queries {
            if query.expected_ids.is_empty() {
                return Err(format!("Sequence query {} has no expected IDs", query.id));
            }
        }

        Ok(())
    }

    /// Get statistics about the dataset.
    pub fn stats(&self) -> TemporalDatasetStats {
        let hours: Vec<u8> = self.memories.iter().map(|m| m.hour).collect();
        let days: Vec<u8> = self.memories.iter().map(|m| m.day_of_week).collect();

        let mut hour_counts = [0usize; 24];
        for h in &hours {
            hour_counts[*h as usize] += 1;
        }

        let mut day_counts = [0usize; 7];
        for d in &days {
            day_counts[*d as usize] += 1;
        }

        let chain_count = self
            .memories
            .iter()
            .filter_map(|m| m.chain_id)
            .collect::<HashSet<_>>()
            .len();

        TemporalDatasetStats {
            total_memories: self.memories.len(),
            recency_queries: self.recency_queries.len(),
            periodic_queries: self.periodic_queries.len(),
            sequence_queries: self.sequence_queries.len(),
            chain_count,
            episode_boundaries: self.episode_boundaries.len(),
            hour_distribution: hour_counts,
            day_distribution: day_counts,
        }
    }
}

/// Statistics about a temporal dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDatasetStats {
    pub total_memories: usize,
    pub recency_queries: usize,
    pub periodic_queries: usize,
    pub sequence_queries: usize,
    pub chain_count: usize,
    pub episode_boundaries: usize,
    pub hour_distribution: [usize; 24],
    pub day_distribution: [usize; 7],
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_generation() {
        let config = TemporalDatasetConfig {
            num_memories: 100,
            num_queries: 30,
            seed: 42,
            ..Default::default()
        };

        let mut generator = TemporalDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert_eq!(dataset.memories.len(), 100);
        assert!(dataset.recency_queries.len() > 0);
        assert!(dataset.periodic_queries.len() > 0);
        assert!(dataset.sequence_queries.len() > 0);
    }

    #[test]
    fn test_dataset_validation() {
        // Use 500 memories to ensure all hourly buckets have coverage
        // With 100 memories spread across 24 hours, some hours may be empty
        let config = TemporalDatasetConfig {
            num_memories: 500,
            num_queries: 30,
            seed: 42,
            ..Default::default()
        };

        let mut generator = TemporalDatasetGenerator::new(config);
        let dataset = generator.generate();

        let result = dataset.validate();
        if let Err(e) = &result {
            eprintln!("Validation error: {}", e);
        }
        assert!(result.is_ok(), "validation failed: {:?}", result);
    }

    #[test]
    fn test_periodic_distribution() {
        let config = TemporalDatasetConfig {
            num_memories: 500,
            num_queries: 10,
            seed: 42,
            periodic_config: PeriodicPatternConfig {
                peak_hours: vec![9, 10, 11],
                concentration: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut generator = TemporalDatasetGenerator::new(config);
        let dataset = generator.generate();
        let stats = dataset.stats();

        // Peak hours should have more memories
        let peak_count: usize = stats.hour_distribution[9..12].iter().copied().sum();
        let non_peak_count: usize = stats.hour_distribution[0..9].iter().copied().sum::<usize>()
            + stats.hour_distribution[12..24]
                .iter()
                .copied()
                .sum::<usize>();

        // With concentration=5.0, peak hours should have significantly more
        assert!(peak_count as f64 / non_peak_count as f64 > 0.5);
    }

    #[test]
    fn test_chain_generation() {
        let config = TemporalDatasetConfig {
            num_memories: 200,
            num_queries: 30,
            seed: 42,
            sequence_config: SequenceChainConfig {
                num_chains: 10,
                avg_chain_length: 5,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut generator = TemporalDatasetGenerator::new(config);
        let dataset = generator.generate();

        // Check chains were created
        let stats = dataset.stats();
        assert!(stats.chain_count > 0);

        // Check chain memories have correct positions
        for memory in &dataset.memories {
            if let Some(chain_id) = memory.chain_id {
                let chain_mems = dataset.get_chain_memories(chain_id);
                assert!(chain_mems.len() >= 3);
            }
        }
    }
}
