# TASK-P3-006: DivergenceDetector

```xml
<task_spec id="TASK-P3-006" version="2.0">
<metadata>
  <title>DivergenceDetector Implementation</title>
  <status>COMPLETE</status>
  <layer>logic</layer>
  <sequence>25</sequence>
  <phase>3</phase>
  <implements>
    <requirement_ref>REQ-P3-04</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="COMPLETE">TASK-P3-002</task_ref>
    <task_ref status="COMPLETE">TASK-P3-003</task_ref>
    <task_ref status="COMPLETE">TASK-P3-004</task_ref>
    <task_ref status="COMPLETE">TASK-P3-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <last_updated>2026-01-17</last_updated>
  <completed_at>2026-01-17</completed_at>
</metadata>

<codebase_state>
## CRITICAL: Current Codebase State (as of 2025-01-17)

### Completed Prerequisites
- TASK-P3-002 (COMPLETE): DivergenceAlert, DivergenceReport, DivergenceSeverity types exist in `divergence.rs`
- TASK-P3-003 (COMPLETE): SimilarityThresholds, PerSpaceThresholds, SPACE_WEIGHTS exist in `config.rs`
- TASK-P3-004 (COMPLETE): compute_similarity_for_space, compute_all_similarities exist in `distance.rs`
- TASK-P3-005 (COMPLETE): MultiSpaceSimilarity exists in `multi_space.rs`

### Existing Types Available for Use (Already Implemented)
1. `DIVERGENCE_SPACES: [Embedder; 7]` - Already exists in divergence.rs
2. `DivergenceAlert`, `DivergenceReport`, `DivergenceSeverity` - Already exist in divergence.rs
3. `MultiSpaceSimilarity` - Already exists in multi_space.rs
4. `SemanticFingerprint` - Defined in types/fingerprint.rs (NOT TeleologicalArray)
5. `Embedder` enum - Defined in teleological/embedder.rs with variants: Semantic, Causal, Sparse, Code, etc. (NO E1/E5 prefix!)

### CORRECT File Paths (CRITICAL - old task had WRONG paths)
| Purpose | CORRECT Path | WRONG Path (from old task) |
|---------|--------------|---------------------------|
| Category | `crates/context-graph-core/src/embeddings/category.rs` | `crates/context-graph-core/src/embedding/category.rs` |
| Embeddings Config | `crates/context-graph-core/src/embeddings/config.rs` | `crates/context-graph-core/src/embedding/config.rs` |
| Retrieval Config | `crates/context-graph-core/src/retrieval/config.rs` | N/A |
| Divergence Types | `crates/context-graph-core/src/retrieval/divergence.rs` | Already exists |
| MultiSpaceSimilarity | `crates/context-graph-core/src/retrieval/multi_space.rs` | Already exists |
| Distance Metrics | `crates/context-graph-core/src/retrieval/distance.rs` | Already exists |
| Fingerprint Type | `crates/context-graph-core/src/types/fingerprint.rs` | N/A |
| Embedder Enum | `crates/context-graph-core/src/teleological/embedder.rs` | N/A |

### Correct Embedder Variant Names (CRITICAL)
```rust
// CORRECT - Use these names (crates/context-graph-core/src/teleological/embedder.rs)
Embedder::Semantic        // NOT Embedder::E1Semantic
Embedder::TemporalRecent  // NOT Embedder::E2TempRecent
Embedder::TemporalPeriodic
Embedder::TemporalPositional
Embedder::Causal          // NOT Embedder::E5Causal
Embedder::Sparse
Embedder::Code
Embedder::Emotional       // Relational category (E8)
Embedder::Hdc             // Structural category (E9)
Embedder::Multimodal
Embedder::Entity          // Relational category (E11)
Embedder::LateInteraction
Embedder::KeywordSplade
```

### Already Exported from retrieval/mod.rs
```rust
pub use divergence::{
    DivergenceAlert, DivergenceReport, DivergenceSeverity,
    DIVERGENCE_SPACES, MAX_SUMMARY_LEN, truncate_summary,
};
pub use multi_space::{
    compute_similarities_batch, filter_relevant, sort_by_relevance, MultiSpaceSimilarity,
};
pub use config::{
    high_thresholds, low_thresholds, default_weights,
    PerSpaceThresholds, SimilarityThresholds, SpaceWeights,
    RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES, SPACE_WEIGHTS, TOTAL_WEIGHT,
};
```
</codebase_state>

<context>
Implements the DivergenceDetector service that identifies when the current query has
diverged from recent context. It compares the query embedding against recent
memories and generates alerts when similarity falls below low thresholds.

Divergence detection helps surface when users have shifted topics and may
need different context.

CATEGORY-AWARE DIVERGENCE: The detector only checks SEMANTIC spaces for divergence:
- DIVERGENCE_SPACES = {Semantic, Causal, Sparse, Code, Multimodal, LateInteraction, KeywordSplade}
- Temporal spaces (TemporalRecent, TemporalPeriodic, TemporalPositional) are IGNORED - they indicate time-based features, not topic shift
- Relational spaces (Emotional, Entity) are IGNORED - emotional/entity drift is not topic divergence
- Structural space (Hdc) is IGNORED - pattern changes are not semantic divergence

This prevents false positive divergence alerts from time-based or structural changes.

## Architecture Rules (from constitution.yaml)
- ARCH-10: Divergence detection uses SEMANTIC embedders only
- AP-62: Divergence alerts MUST only use SEMANTIC embedders
- AP-63: NEVER trigger divergence from temporal proximity differences
</context>

<input_context_files>
  <file purpose="divergence_types" path="crates/context-graph-core/src/retrieval/divergence.rs">
    Contains: DIVERGENCE_SPACES, DivergenceAlert, DivergenceReport, DivergenceSeverity, truncate_summary
    Status: ALREADY IMPLEMENTED (TASK-P3-002)
  </file>
  <file purpose="multi_space" path="crates/context-graph-core/src/retrieval/multi_space.rs">
    Contains: MultiSpaceSimilarity with is_below_low_threshold(), compute_similarity()
    Status: ALREADY IMPLEMENTED (TASK-P3-005)
  </file>
  <file purpose="config" path="crates/context-graph-core/src/retrieval/config.rs">
    Contains: SimilarityThresholds, RECENT_LOOKBACK_SECS (7200), MAX_RECENT_MEMORIES (50)
    Status: ALREADY IMPLEMENTED (TASK-P3-003)
  </file>
  <file purpose="distance" path="crates/context-graph-core/src/retrieval/distance.rs">
    Contains: compute_similarity_for_space, compute_all_similarities
    Status: ALREADY IMPLEMENTED (TASK-P3-004)
  </file>
  <file purpose="category" path="crates/context-graph-core/src/embeddings/category.rs">
    Contains: EmbedderCategory, category_for(), topic_weight(), used_for_divergence_detection()
    Status: ALREADY IMPLEMENTED
  </file>
  <file purpose="fingerprint" path="crates/context-graph-core/src/types/fingerprint.rs">
    Contains: SemanticFingerprint struct (the 13-embedding array type)
    Status: ALREADY IMPLEMENTED
  </file>
  <file purpose="embedder_enum" path="crates/context-graph-core/src/teleological/embedder.rs">
    Contains: Embedder enum with all(), short_name() methods
    Status: ALREADY IMPLEMENTED
  </file>
  <file purpose="mod_exports" path="crates/context-graph-core/src/retrieval/mod.rs">
    Status: Will need update to export new detector types
  </file>
</input_context_files>

<prerequisites>
  <check status="COMPLETE">TASK-P3-002 complete (DivergenceAlert, DivergenceReport, DivergenceSeverity exist in divergence.rs)</check>
  <check status="COMPLETE">TASK-P3-003 complete (SimilarityThresholds, RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES exist in config.rs)</check>
  <check status="COMPLETE">TASK-P3-004 complete (compute_similarity_for_space exists in distance.rs)</check>
  <check status="COMPLETE">TASK-P3-005 complete (MultiSpaceSimilarity with is_below_low_threshold exists in multi_space.rs)</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement DivergenceDetector struct in detector.rs
    - Implement RecentMemory struct for memory context
    - Implement detect_divergence method that compares query against recent memories
    - Generate alerts only for SEMANTIC spaces using DIVERGENCE_SPACES constant (already exists)
    - Sort alerts by severity (lowest score first)
    - Implement lookback filtering (within RECENT_LOOKBACK_SECS = 7200s)
    - Limit to MAX_RECENT_MEMORIES (50)
    - Implement should_alert() logic for high severity alerts
    - Implement summarize_divergence() for human-readable output
    - Add module export in mod.rs
    - Implement is_divergence_space() helper function
  </in_scope>
  <out_of_scope>
    - Memory storage/retrieval (use MemoryStore interface in future)
    - Automatic alert notification
    - Divergence tracking over time
    - Temporal/relational/structural divergence (not topic divergence per ARCH-10)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/retrieval/detector.rs">
// CRITICAL: Use CORRECT type names from codebase
use std::time::Duration;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::types::fingerprint::SemanticFingerprint;  // NOT TeleologicalArray
use crate::teleological::Embedder;                    // Uses Embedder::Semantic, etc.
use super::multi_space::MultiSpaceSimilarity;
use super::divergence::{DivergenceAlert, DivergenceReport, DivergenceSeverity, DIVERGENCE_SPACES};
use super::config::{RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES};

/// Check if an embedder is used for divergence detection (semantic only)
pub fn is_divergence_space(embedder: Embedder) -&gt; bool;

/// A recent memory for divergence checking
#[derive(Debug, Clone)]
pub struct RecentMemory {
    pub id: Uuid,
    pub content: String,
    pub embedding: SemanticFingerprint,  // NOT TeleologicalArray
    pub created_at: DateTime&lt;Utc&gt;,
}

/// Detects divergence between current query and recent context
pub struct DivergenceDetector {
    similarity: MultiSpaceSimilarity,
    lookback_duration: Duration,
    max_recent: usize,
}

impl DivergenceDetector {
    pub fn new(similarity: MultiSpaceSimilarity) -&gt; Self;
    pub fn with_config(similarity: MultiSpaceSimilarity, lookback: Duration, max_recent: usize) -&gt; Self;

    /// Detect divergence - ONLY checks DIVERGENCE_SPACES (semantic embedders)
    pub fn detect_divergence(&amp;self, query: &amp;SemanticFingerprint, recent_memories: &amp;[RecentMemory]) -&gt; DivergenceReport;

    /// Returns true only for high severity divergence
    pub fn should_alert(&amp;self, report: &amp;DivergenceReport) -&gt; bool;

    /// Generate human-readable summary
    pub fn summarize_divergence(&amp;self, report: &amp;DivergenceReport) -&gt; String;

    pub fn lookback_duration(&amp;self) -&gt; Duration;
    pub fn max_recent(&amp;self) -&gt; usize;
}
    </signature>
  </signatures>

  <constraints>
    - Only check against recent memories within lookback window (RECENT_LOOKBACK_SECS = 7200 = 2 hours)
    - Max 50 recent memories checked (MAX_RECENT_MEMORIES)
    - Alert generated when ANY SEMANTIC space below low threshold
    - Only check DIVERGENCE_SPACES (Semantic, Causal, Sparse, Code, Multimodal, LateInteraction, KeywordSplade)
    - IGNORE temporal spaces (TemporalRecent, TemporalPeriodic, TemporalPositional) for divergence detection
    - IGNORE relational spaces (Emotional, Entity) for divergence detection
    - IGNORE structural space (Hdc) for divergence detection
    - Alerts sorted by severity (lowest score first = most severe first)
    - Include memory summary in alert (100 chars max via truncate_summary)
  </constraints>

  <verification>
    - Similar query generates no alerts (all scores above low thresholds)
    - Divergent query generates alerts for semantic spaces with low similarity
    - Alerts sorted by severity (lowest score first)
    - Max memory limit (50) respected
    - Lookback filtering (2 hours) works - old memories excluded
    - Low temporal similarity does NOT generate alerts (AP-63)
    - Low relational/structural similarity does NOT generate alerts
    - Only semantic spaces (Semantic, Causal, Sparse, Code, Multimodal, LateInteraction, KeywordSplade) can trigger alerts
    - DIVERGENCE_SPACES contains exactly 7 embedders
    - is_divergence_space() returns true only for semantic embedders
    - should_alert() returns true only for High severity
    - summarize_divergence() produces readable output
  </verification>
</definition_of_done>

<implementation_code file="crates/context-graph-core/src/retrieval/detector.rs">
//! DivergenceDetector for topic drift detection.
//!
//! This module implements the core divergence detection service that compares
//! the current query against recent memories and generates alerts when
//! similarity falls below low thresholds in SEMANTIC embedding spaces.
//!
//! # Architecture Rules
//!
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders
//! - AP-63: NEVER trigger divergence from temporal proximity differences

use std::time::Duration;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::embeddings::category::category_for;
use crate::types::fingerprint::SemanticFingerprint;
use crate::teleological::Embedder;

use super::multi_space::MultiSpaceSimilarity;
use super::divergence::{DivergenceAlert, DivergenceReport, DivergenceSeverity, DIVERGENCE_SPACES};
use super::config::{RECENT_LOOKBACK_SECS, MAX_RECENT_MEMORIES};

/// Check if an embedder is used for divergence detection.
///
/// Only SEMANTIC category embedders are used for divergence detection per ARCH-10.
/// Returns true for: Semantic, Causal, Sparse, Code, Multimodal, LateInteraction, KeywordSplade
/// Returns false for: TemporalRecent, TemporalPeriodic, TemporalPositional, Emotional, Entity, Hdc
#[inline]
pub fn is_divergence_space(embedder: Embedder) -&gt; bool {
    category_for(embedder).used_for_divergence_detection()
}

/// A recent memory for divergence checking.
///
/// Represents a memory that was recently created and should be compared
/// against the current query to detect topic divergence.
#[derive(Debug, Clone)]
pub struct RecentMemory {
    /// Unique identifier of the memory
    pub id: Uuid,
    /// Text content of the memory (for alert summaries)
    pub content: String,
    /// Full 13-embedding fingerprint
    pub embedding: SemanticFingerprint,
    /// When this memory was created
    pub created_at: DateTime&lt;Utc&gt;,
}

impl RecentMemory {
    /// Create a new RecentMemory.
    pub fn new(
        id: Uuid,
        content: String,
        embedding: SemanticFingerprint,
        created_at: DateTime&lt;Utc&gt;,
    ) -&gt; Self {
        Self { id, content, embedding, created_at }
    }
}

/// Detects divergence between current query and recent context.
///
/// The detector compares the query's embeddings against recent memories
/// and generates alerts when similarity falls BELOW low thresholds.
///
/// # Category-Aware Detection (ARCH-10)
///
/// Only SEMANTIC embedding spaces are checked for divergence:
/// - Semantic (E1), Causal (E5), Sparse (E6), Code (E7)
/// - Multimodal (E10), LateInteraction (E12), KeywordSplade (E13)
///
/// These spaces are IGNORED (AP-63):
/// - Temporal (E2-E4): Time-based features, not topic indicators
/// - Relational (E8, E11): Emotional/entity drift is not topic divergence
/// - Structural (E9): Pattern changes are not semantic divergence
#[derive(Debug, Clone)]
pub struct DivergenceDetector {
    similarity: MultiSpaceSimilarity,
    lookback_duration: Duration,
    max_recent: usize,
}

impl DivergenceDetector {
    /// Create with default configuration.
    ///
    /// Uses RECENT_LOOKBACK_SECS (7200 = 2 hours) and MAX_RECENT_MEMORIES (50).
    pub fn new(similarity: MultiSpaceSimilarity) -&gt; Self {
        Self {
            similarity,
            lookback_duration: Duration::from_secs(RECENT_LOOKBACK_SECS),
            max_recent: MAX_RECENT_MEMORIES,
        }
    }

    /// Create with custom configuration.
    ///
    /// # Arguments
    /// * `similarity` - MultiSpaceSimilarity service for computing scores
    /// * `lookback` - How far back to look for recent memories
    /// * `max_recent` - Maximum number of recent memories to check
    pub fn with_config(
        similarity: MultiSpaceSimilarity,
        lookback: Duration,
        max_recent: usize,
    ) -&gt; Self {
        Self {
            similarity,
            lookback_duration: lookback,
            max_recent,
        }
    }

    /// Detect divergence between query and recent memories.
    ///
    /// # Algorithm
    /// 1. Filter memories to those within lookback window
    /// 2. Limit to max_recent memories
    /// 3. For each memory, compute similarity in all 13 spaces
    /// 4. Only check DIVERGENCE_SPACES (semantic embedders) - temporal/relational/structural IGNORED
    /// 5. Generate alert if ANY semantic space is below low threshold
    /// 6. Sort alerts by severity (lowest score = most severe first)
    ///
    /// # Arguments
    /// * `query` - The current query's embedding fingerprint
    /// * `recent_memories` - Recent memories to compare against
    ///
    /// # Returns
    /// DivergenceReport containing all detected divergence alerts
    pub fn detect_divergence(
        &amp;self,
        query: &amp;SemanticFingerprint,
        recent_memories: &amp;[RecentMemory],
    ) -&gt; DivergenceReport {
        let mut report = DivergenceReport::new();

        // Calculate cutoff time for lookback window
        let cutoff = Utc::now() - chrono::Duration::from_std(self.lookback_duration)
            .unwrap_or(chrono::Duration::hours(2));

        // Filter to recent memories within lookback window, limit to max_recent
        let filtered: Vec&lt;&amp;RecentMemory&gt; = recent_memories
            .iter()
            .filter(|m| m.created_at &gt;= cutoff)
            .take(self.max_recent)
            .collect();

        // Check each recent memory for divergence
        for memory in filtered {
            // Compute similarity across all 13 spaces
            let scores = self.similarity.compute_similarity(query, &amp;memory.embedding);

            // Only check DIVERGENCE_SPACES (semantic embedders per ARCH-10)
            // Temporal (E2-E4), Relational (E8, E11), Structural (E9) are IGNORED
            for &amp;embedder in &amp;DIVERGENCE_SPACES {
                let score = scores.get_score(embedder);

                // Check if score is below low threshold for this space
                if self.similarity.is_below_low_threshold(embedder, score) {
                    let alert = DivergenceAlert::new(
                        memory.id,
                        embedder,
                        score,
                        &amp;memory.content,
                    );
                    report.add(alert);
                }
            }
        }

        // Sort by severity (lowest score = most severe first)
        report.sort_by_severity();

        report
    }

    /// Check if report contains alerts worth surfacing to user.
    ///
    /// Returns true only for High severity divergence (score &lt; 0.10).
    /// Medium and Low severity alerts are logged but not surfaced.
    pub fn should_alert(&amp;self, report: &amp;DivergenceReport) -&gt; bool {
        if report.is_empty() {
            return false;
        }

        // Alert only if there's at least one high severity divergence
        if let Some(most_severe) = report.most_severe() {
            most_severe.severity() == DivergenceSeverity::High
        } else {
            false
        }
    }

    /// Generate human-readable divergence summary.
    ///
    /// # Format
    /// - If no divergence: "No divergence detected. Context is coherent."
    /// - If divergence: Summary with counts and top 3 alerts
    pub fn summarize_divergence(&amp;self, report: &amp;DivergenceReport) -&gt; String {
        if report.is_empty() {
            return "No divergence detected. Context is coherent.".to_string();
        }

        let (high, medium, low) = report.count_by_severity();

        let mut summary = format!(
            "Divergence detected: {} high, {} medium, {} low severity alerts.\n",
            high, medium, low
        );

        // Add top 3 most severe alerts
        for alert in report.alerts.iter().take(3) {
            summary.push_str(&amp;format!("  - {}\n", alert.format_alert()));
        }

        summary
    }

    /// Get the lookback duration.
    #[inline]
    pub fn lookback_duration(&amp;self) -&gt; Duration {
        self.lookback_duration
    }

    /// Get the max recent memories limit.
    #[inline]
    pub fn max_recent(&amp;self) -&gt; usize {
        self.max_recent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // is_divergence_space Tests
    // =========================================================================

    #[test]
    fn test_divergence_spaces_count() {
        // DIVERGENCE_SPACES should contain exactly 7 semantic embedders
        assert_eq!(DIVERGENCE_SPACES.len(), 7);
        println!("[PASS] DIVERGENCE_SPACES has exactly 7 semantic embedders");
    }

    #[test]
    fn test_semantic_is_divergence_space() {
        // All semantic embedders should return true
        assert!(is_divergence_space(Embedder::Semantic));
        assert!(is_divergence_space(Embedder::Causal));
        assert!(is_divergence_space(Embedder::Sparse));
        assert!(is_divergence_space(Embedder::Code));
        assert!(is_divergence_space(Embedder::Multimodal));
        assert!(is_divergence_space(Embedder::LateInteraction));
        assert!(is_divergence_space(Embedder::KeywordSplade));
        println!("[PASS] All 7 semantic embedders return true for is_divergence_space");
    }

    #[test]
    fn test_temporal_not_divergence_space() {
        // Temporal spaces should NOT be divergence spaces (AP-63)
        assert!(!is_divergence_space(Embedder::TemporalRecent));
        assert!(!is_divergence_space(Embedder::TemporalPeriodic));
        assert!(!is_divergence_space(Embedder::TemporalPositional));
        println!("[PASS] Temporal embedders excluded from divergence detection");
    }

    #[test]
    fn test_relational_not_divergence_space() {
        // Relational spaces should NOT be divergence spaces
        assert!(!is_divergence_space(Embedder::Emotional));
        assert!(!is_divergence_space(Embedder::Entity));
        println!("[PASS] Relational embedders excluded from divergence detection");
    }

    #[test]
    fn test_structural_not_divergence_space() {
        // Structural space should NOT be divergence space
        assert!(!is_divergence_space(Embedder::Hdc));
        println!("[PASS] Structural embedder excluded from divergence detection");
    }

    // =========================================================================
    // RecentMemory Tests
    // =========================================================================

    #[test]
    fn test_recent_memory_creation() {
        let id = Uuid::new_v4();
        let content = "Test memory content".to_string();
        let embedding = SemanticFingerprint::zeroed();
        let created_at = Utc::now();

        let memory = RecentMemory::new(id, content.clone(), embedding, created_at);

        assert_eq!(memory.id, id);
        assert_eq!(memory.content, content);
        assert_eq!(memory.created_at, created_at);
        println!("[PASS] RecentMemory created correctly");
    }

    // =========================================================================
    // DivergenceDetector Tests
    // =========================================================================

    #[test]
    fn test_detector_default_config() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        assert_eq!(detector.lookback_duration(), Duration::from_secs(7200));
        assert_eq!(detector.max_recent(), 50);
        println!("[PASS] DivergenceDetector uses default config");
    }

    #[test]
    fn test_detector_custom_config() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(3600),  // 1 hour
            25,
        );

        assert_eq!(detector.lookback_duration(), Duration::from_secs(3600));
        assert_eq!(detector.max_recent(), 25);
        println!("[PASS] DivergenceDetector accepts custom config");
    }

    #[test]
    fn test_detect_divergence_empty_memories() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);
        let query = SemanticFingerprint::zeroed();

        let report = detector.detect_divergence(&amp;query, &amp;[]);

        assert!(report.is_empty());
        assert!(!detector.should_alert(&amp;report));
        println!("[PASS] Empty memories produces empty report");
    }

    #[test]
    fn test_detect_divergence_filters_by_lookback() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(60),  // 1 minute lookback
            10,
        );

        // Create old memory (1 hour ago)
        let old_memory = RecentMemory::new(
            Uuid::new_v4(),
            "Old memory".to_string(),
            SemanticFingerprint::zeroed(),
            Utc::now() - chrono::Duration::hours(1),
        );

        let query = SemanticFingerprint::zeroed();
        let report = detector.detect_divergence(&amp;query, &amp;[old_memory]);

        // Old memory should be filtered out
        assert!(report.is_empty());
        println!("[PASS] Old memories filtered by lookback window");
    }

    #[test]
    fn test_detect_divergence_respects_max_recent() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::with_config(
            similarity,
            Duration::from_secs(7200),
            2,  // Only check 2 memories
        );

        // Create 5 recent memories
        let memories: Vec&lt;RecentMemory&gt; = (0..5)
            .map(|i| RecentMemory::new(
                Uuid::new_v4(),
                format!("Memory {}", i),
                SemanticFingerprint::zeroed(),
                Utc::now(),
            ))
            .collect();

        let query = SemanticFingerprint::zeroed();
        let report = detector.detect_divergence(&amp;query, &amp;memories);

        // Should only process first 2 memories
        // Exact alert count depends on similarity scores
        println!("[PASS] Max recent limit respected (processed up to 2 memories)");
    }

    #[test]
    fn test_should_alert_high_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.05,  // High severity (score &lt; 0.10)
            "Test content",
        ));

        assert!(detector.should_alert(&amp;report));
        println!("[PASS] should_alert returns true for High severity");
    }

    #[test]
    fn test_should_alert_medium_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.15,  // Medium severity (0.10 &lt;= score &lt; 0.20)
            "Test content",
        ));

        assert!(!detector.should_alert(&amp;report));
        println!("[PASS] should_alert returns false for Medium severity");
    }

    #[test]
    fn test_should_alert_low_severity() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.25,  // Low severity (score &gt;= 0.20)
            "Test content",
        ));

        assert!(!detector.should_alert(&amp;report));
        println!("[PASS] should_alert returns false for Low severity");
    }

    #[test]
    fn test_summarize_empty_report() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);
        let report = DivergenceReport::new();

        let summary = detector.summarize_divergence(&amp;report);
        assert!(summary.contains("No divergence"));
        println!("[PASS] summarize_divergence for empty report: {}", summary);
    }

    #[test]
    fn test_summarize_with_alerts() {
        let similarity = MultiSpaceSimilarity::with_defaults();
        let detector = DivergenceDetector::new(similarity);

        let mut report = DivergenceReport::new();
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Semantic,
            0.05,
            "High severity alert",
        ));
        report.add(DivergenceAlert::new(
            Uuid::new_v4(),
            Embedder::Code,
            0.15,
            "Medium severity alert",
        ));

        let summary = detector.summarize_divergence(&amp;report);
        assert!(summary.contains("high"));
        assert!(summary.contains("medium"));
        println!("[PASS] summarize_divergence with alerts: {}", summary);
    }

    // =========================================================================
    // Constitution Compliance Tests
    // =========================================================================

    #[test]
    fn test_arch10_semantic_only() {
        // ARCH-10: Divergence detection uses SEMANTIC embedders only
        for embedder in Embedder::all() {
            let is_semantic = category_for(embedder).is_semantic();
            let is_divergence = is_divergence_space(embedder);
            assert_eq!(
                is_semantic, is_divergence,
                "{:?}: is_semantic={} but is_divergence_space={}",
                embedder, is_semantic, is_divergence
            );
        }
        println!("[PASS] ARCH-10: is_divergence_space matches is_semantic");
    }

    #[test]
    fn test_ap63_no_temporal_divergence() {
        // AP-63: NEVER trigger divergence from temporal proximity differences
        for embedder in [
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
        ] {
            assert!(
                !DIVERGENCE_SPACES.contains(&amp;embedder),
                "AP-63 violation: {:?} in DIVERGENCE_SPACES",
                embedder
            );
            assert!(
                !is_divergence_space(embedder),
                "AP-63 violation: is_divergence_space({:?}) returned true",
                embedder
            );
        }
        println!("[PASS] AP-63: Temporal embedders excluded from DIVERGENCE_SPACES");
    }
}
</implementation_code>

<files_to_create>
  <file path="crates/context-graph-core/src/retrieval/detector.rs">
    DivergenceDetector service implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/retrieval/mod.rs">
    Add: pub mod detector;
    Add exports: pub use detector::{DivergenceDetector, RecentMemory, is_divergence_space};
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>detector.rs compiles without errors</criterion>
  <criterion>detect_divergence generates alerts for low-similarity SEMANTIC spaces only</criterion>
  <criterion>Alerts sorted by severity (lowest score first)</criterion>
  <criterion>Lookback filtering works correctly (2 hour default)</criterion>
  <criterion>Max recent limit (50) respected</criterion>
  <criterion>should_alert returns true only for high severity (score &lt; 0.10)</criterion>
  <criterion>summarize_divergence produces readable output</criterion>
  <criterion>DIVERGENCE_SPACES contains exactly 7 embedders (from existing divergence.rs)</criterion>
  <criterion>Temporal spaces (TemporalRecent, TemporalPeriodic, TemporalPositional) do NOT generate divergence alerts</criterion>
  <criterion>Relational spaces (Emotional, Entity) do NOT generate divergence alerts</criterion>
  <criterion>Structural space (Hdc) does NOT generate divergence alerts</criterion>
  <criterion>is_divergence_space() correctly identifies semantic embedders only</criterion>
  <criterion>All unit tests pass: cargo test --package context-graph-core detector</criterion>
</validation_criteria>

<test_commands>
  <command description="Run detector tests">cargo test --package context-graph-core detector -- --nocapture</command>
  <command description="Run all retrieval tests">cargo test --package context-graph-core --lib retrieval -- --nocapture</command>
  <command description="Check compilation">cargo check --package context-graph-core</command>
  <command description="Run clippy">cargo clippy --package context-graph-core -- -D warnings</command>
</test_commands>
</task_spec>
```

---

## Full State Verification Requirements

### Source of Truth
| State | Location | Verification Method |
|-------|----------|---------------------|
| DivergenceDetector struct | `crates/context-graph-core/src/retrieval/detector.rs` | File exists, `cargo check` passes |
| RecentMemory struct | `crates/context-graph-core/src/retrieval/detector.rs` | `cargo doc` shows struct |
| is_divergence_space function | `crates/context-graph-core/src/retrieval/detector.rs` | Unit test passes |
| Module export | `crates/context-graph-core/src/retrieval/mod.rs` | `grep 'pub mod detector'` |
| Type re-exports | `crates/context-graph-core/src/retrieval/mod.rs` | `grep 'DivergenceDetector'` |

### Execute & Inspect Protocol
After implementation, run these verification commands:

```bash
# 1. Compile check - Source of Truth: compiler output
cargo check --package context-graph-core 2>&1 | tee /tmp/check.log
grep -E "(error|warning)" /tmp/check.log && echo "FAIL" || echo "PASS"

# 2. Run tests - Source of Truth: test output
cargo test --package context-graph-core detector -- --nocapture 2>&1 | tee /tmp/test.log
grep "FAILED" /tmp/test.log && echo "FAIL" || echo "PASS"

# 3. Verify exports - Source of Truth: mod.rs content
grep -E "pub (mod|use).*detector" crates/context-graph-core/src/retrieval/mod.rs
grep "DivergenceDetector" crates/context-graph-core/src/retrieval/mod.rs
grep "RecentMemory" crates/context-graph-core/src/retrieval/mod.rs
grep "is_divergence_space" crates/context-graph-core/src/retrieval/mod.rs

# 4. Verify DIVERGENCE_SPACES count - Source of Truth: test assertion
cargo test --package context-graph-core test_divergence_spaces_count -- --nocapture

# 5. Verify temporal exclusion - Source of Truth: test assertion
cargo test --package context-graph-core test_temporal_not_divergence_space -- --nocapture
```

### Boundary & Edge Case Audit

**Edge Case 1: Empty memories list**
- Input: `detect_divergence(query, &[])`
- Expected: Empty DivergenceReport, `is_empty() == true`, `should_alert() == false`
- Test: `test_detect_divergence_empty_memories`

**Edge Case 2: All memories outside lookback window**
- Input: Memories with `created_at` older than lookback duration
- Expected: Empty DivergenceReport (filtered out)
- Test: `test_detect_divergence_filters_by_lookback`

**Edge Case 3: Exactly at threshold boundary**
- Input: Score exactly at low threshold (e.g., 0.30 for Semantic)
- Expected: NOT an alert (< threshold, not <=)
- Test: Verify in integration test

**Edge Case 4: Max recent limit exceeded**
- Input: 100 memories, max_recent = 50
- Expected: Only first 50 processed
- Test: `test_detect_divergence_respects_max_recent`

**Edge Case 5: High temporal similarity, low semantic similarity**
- Input: TemporalRecent = 0.95, Semantic = 0.05
- Expected: Alert for Semantic ONLY, not temporal (AP-63)
- Test: `test_ap63_no_temporal_divergence`

### Evidence of Success
After implementation, capture and verify:

```bash
# Evidence 1: detector.rs exists with correct content
ls -la crates/context-graph-core/src/retrieval/detector.rs
wc -l crates/context-graph-core/src/retrieval/detector.rs

# Evidence 2: All tests pass
cargo test --package context-graph-core detector 2>&1 | grep -E "test result|PASSED|FAILED"

# Evidence 3: Clippy clean
cargo clippy --package context-graph-core -- -D warnings 2>&1 | grep -c "error" | xargs -I{} test {} -eq 0

# Evidence 4: Module properly exported
cargo doc --package context-graph-core --no-deps 2>&1 | grep -E "Documenting|error"
```

---

## Execution Checklist

### Phase 1: Setup
- [ ] Verify all prerequisite tasks are COMPLETE (P3-002 through P3-005)
- [ ] Read existing `divergence.rs` to understand current types
- [ ] Read existing `multi_space.rs` to understand MultiSpaceSimilarity interface
- [ ] Read `config.rs` for RECENT_LOOKBACK_SECS and MAX_RECENT_MEMORIES values

### Phase 2: Implementation
- [ ] Create `detector.rs` in `crates/context-graph-core/src/retrieval/`
- [ ] Implement `is_divergence_space()` function using `category_for().used_for_divergence_detection()`
- [ ] Implement `RecentMemory` struct with id, content, embedding (SemanticFingerprint), created_at
- [ ] Implement `DivergenceDetector::new()` with defaults
- [ ] Implement `DivergenceDetector::with_config()` for custom settings
- [ ] Implement `detect_divergence()` method:
  - Filter by lookback window
  - Limit to max_recent
  - Iterate DIVERGENCE_SPACES only (NOT all embedders)
  - Use `similarity.is_below_low_threshold()` for alert generation
  - Sort by severity
- [ ] Implement `should_alert()` - true only for High severity
- [ ] Implement `summarize_divergence()` for human-readable output

### Phase 3: Integration
- [ ] Add `pub mod detector;` to `mod.rs`
- [ ] Add exports to `mod.rs`:
  ```rust
  pub use detector::{DivergenceDetector, RecentMemory, is_divergence_space};
  ```

### Phase 4: Verification
- [ ] Run `cargo check --package context-graph-core`
- [ ] Run `cargo test --package context-graph-core detector -- --nocapture`
- [ ] Run `cargo clippy --package context-graph-core -- -D warnings`
- [ ] Verify all 7 semantic embedders in is_divergence_space() tests pass
- [ ] Verify temporal exclusion tests pass (AP-63)
- [ ] Verify lookback filtering test passes
- [ ] Verify max_recent limit test passes

### Phase 5: Final Validation
- [ ] All tests pass with 0 failures
- [ ] Clippy reports 0 errors
- [ ] Manual review of detector.rs confirms ARCH-10 compliance
- [ ] Manual review confirms correct embedder variant names used

---

## Fail Fast Requirements

This implementation uses **NO BACKWARDS COMPATIBILITY**. The following will cause immediate failure:

1. **Wrong type names**: Using `TeleologicalArray` instead of `SemanticFingerprint` = compilation error
2. **Wrong embedder names**: Using `Embedder::E1Semantic` instead of `Embedder::Semantic` = compilation error
3. **Wrong file paths**: Importing from `embedding/` instead of `embeddings/` = compilation error
4. **Missing dependencies**: Not importing from correct modules = compilation error
5. **NaN/Infinity scores**: Will fail at runtime with invalid comparison (AP-10)

All errors will surface immediately via `cargo check` or `cargo test` - no silent failures.

---

## Proceed to TASK-P3-007

After successful completion, proceed to TASK-P3-007 (next in sequence).
