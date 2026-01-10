# TASK-INTEG-002: Purpose and Goal MCP Handlers Integration

```xml
<task_spec id="TASK-INTEG-002" version="5.0">
<metadata>
  <title>Integrate Purpose/Goal MCP Handlers with TASK-LOGIC-009/010</title>
  <status>todo</status>
  <layer>integration</layer>
  <sequence>22</sequence>
  <implements>
    <requirement_ref>REQ-MCP-PURPOSE-01</requirement_ref>
    <requirement_ref>REQ-MCP-GOAL-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref status="DONE">TASK-LOGIC-009</task_ref>
    <task_ref status="DONE">TASK-LOGIC-010</task_ref>
    <task_ref status="TODO">TASK-INTEG-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
## CURRENT STATE (Verified 2026-01-09)

### CRITICAL: Files ALREADY EXIST - Do NOT Create New Ones

| File | Lines | Status | Purpose |
|------|-------|--------|---------|
| `crates/context-graph-mcp/src/handlers/purpose.rs` | 986 | **EXISTS** | Purpose query, goal hierarchy, aligned memories, drift check |
| `crates/context-graph-mcp/src/handlers/autonomous.rs` | 1112 | **EXISTS** | Bootstrap, drift, correction, pruning, consolidation, sub-goals, status |
| `crates/context-graph-core/src/autonomous/discovery.rs` | 840 | **DONE** (TASK-LOGIC-009) | GoalDiscoveryPipeline with K-means clustering |
| `crates/context-graph-core/src/autonomous/drift.rs` | 1647 | **DONE** (TASK-LOGIC-010) | TeleologicalDriftDetector with 5-level DriftLevel |

### EXISTING PURPOSE HANDLERS (purpose.rs:986 lines)

```rust
// Protocol constants (protocol.rs:296-302)
pub const PURPOSE_QUERY: &str = "purpose/query";
pub const GOAL_HIERARCHY_QUERY: &str = "goal/hierarchy_query";
pub const GOAL_ALIGNED_MEMORIES: &str = "goal/aligned_memories";
pub const PURPOSE_DRIFT_CHECK: &str = "purpose/drift_check";

// Existing handler methods (purpose.rs)
impl Handlers {
    pub(super) async fn handle_purpose_query(/* line 54 */)
    pub(super) async fn handle_goal_hierarchy_query(/* line 244 */)
    pub(super) async fn handle_goal_aligned_memories(/* line 519 */)
    pub(super) async fn handle_purpose_drift_check(/* line 710 */)
}
```

### EXISTING AUTONOMOUS HANDLERS (autonomous.rs:1112 lines)

```rust
// Existing handler methods (autonomous.rs)
impl Handlers {
    pub(super) async fn call_auto_bootstrap_north_star(/* line 231 */)
    pub(super) async fn call_get_alignment_drift(/* line 331 */)
    pub(super) async fn call_trigger_drift_correction(/* line 438 */)
    pub(super) async fn call_get_pruning_candidates(/* line 551 */)
    pub(super) async fn call_trigger_consolidation(/* line 627 */)
    pub(super) async fn call_discover_sub_goals(/* line 719 */)
    pub(super) async fn call_get_autonomous_status(/* line 847 */)
}
```

### DEPENDENCIES (VERIFIED COMPLETE)

| Task | Component | Location | Status |
|------|-----------|----------|--------|
| TASK-LOGIC-009 | GoalDiscoveryPipeline | `crates/context-graph-core/src/autonomous/discovery.rs` | **DONE** (840 lines, 10 tests) |
| TASK-LOGIC-010 | TeleologicalDriftDetector | `crates/context-graph-core/src/autonomous/drift.rs` | **DONE** (1647 lines, 30 tests) |
| TASK-INTEG-001 | Memory MCP Handlers | `crates/context-graph-mcp/src/handlers/memory.rs` | **TODO** |

### CORE TYPES FROM TASK-LOGIC-009 (discovery.rs)

```rust
pub struct GoalDiscoveryPipeline { comparator: TeleologicalComparator, config: DiscoveryConfig }
pub struct DiscoveryConfig { sample_size, min_cluster_size, min_coherence, clustering_algorithm, num_clusters, comparison_type }
pub enum ClusteringAlgorithm { KMeans, HDBSCAN { min_samples }, Spectral { n_neighbors } }
pub enum NumClusters { Auto, Fixed(usize) }
pub struct DiscoveredGoal { goal_id, description, level, confidence, member_count, centroid, centroid_strength, dominant_embedders, keywords, coherence_score }
pub struct DiscoveryResult { discovered_goals, total_arrays_analyzed, clusters_found, clusters_above_threshold }
```

### CORE TYPES FROM TASK-LOGIC-010 (drift.rs)

```rust
pub struct TeleologicalDriftDetector { comparator: TeleologicalComparator, history: DriftHistory, thresholds: DriftThresholds }
pub enum DriftLevel { Critical, High, Medium, Low, None }  // 5 levels, ordered worst-to-best
pub enum DriftTrend { Improving, Stable, Worsening, Declining }
pub struct DriftResult { overall_drift, per_embedder_drift, most_drifted_embedders, recommendations, trend, analyzed_count, timestamp }
pub struct PerEmbedderDrift { embedder_drift: [EmbedderDriftInfo; 13] }
pub struct DriftHistoryEntry { timestamp, overall_similarity, per_embedder: [f32; 13], memories_analyzed }
pub enum DriftError { EmptyMemories, InvalidGoal { reason }, ComparisonFailed { embedder, reason }, InvalidThresholds { reason }, ComparisonValidationFailed { reason } }
```

### EXISTING SERVICE TYPES (autonomous/services/)

```rust
// bootstrap_service.rs
pub struct BootstrapService { /* lines 82+ */ }
pub struct BootstrapResult { goal_id, goal_text, confidence, source }

// drift_detector.rs (legacy, used by autonomous.rs)
pub struct DriftDetector { /* line 111 */ }
pub struct DetectorState { rolling_mean, baseline, drift, severity, trend, checked_at, history }

// drift_corrector.rs
pub struct DriftCorrector { /* line 119 */ }
pub enum CorrectionStrategy { Reinforce, Rebalance, Reset }
pub struct CorrectionResult { success, strategy_applied, alignment_before, alignment_after, improvement }

// pruning_service.rs
pub struct PruningService { /* line 227 */ }
pub struct PruningCandidate { memory_id, reason, score, metadata }
pub enum PruneReason { LowUtility, Stale, Redundant, Misaligned, Corrupted }

// consolidation_service.rs
pub struct ConsolidationService { /* line 102 */ }
pub struct ServiceConsolidationCandidate { pair, similarity, strategy }

// subgoal_discovery.rs
pub struct SubGoalDiscovery { /* line 164 */ }
pub struct MemoryCluster { centroid, members, coherence }
pub struct DiscoveryResult { discovered_subgoals, cluster_analysis }
```
</context>

<objective>
**INTEGRATE** existing MCP handlers with new TASK-LOGIC-009/010 implementations:

1. **Wire** `GoalDiscoveryPipeline` (TASK-LOGIC-009) into `call_discover_sub_goals`
2. **Wire** `TeleologicalDriftDetector` (TASK-LOGIC-010) into `handle_purpose_drift_check` and `call_get_alignment_drift`
3. **Replace** legacy `DriftDetector` with `TeleologicalDriftDetector` for per-embedder analysis
4. **Add** new protocol constants if missing for discovery endpoints
5. **Update** response types to include per-embedder breakdown from DriftResult

This is an **INTEGRATION** task, NOT a creation task. The handlers exist. The logic modules exist. Wire them together.
</objective>

<rationale>
1. **GoalDiscoveryPipeline** (TASK-LOGIC-009) provides K-means clustering on TeleologicalArrays
2. **TeleologicalDriftDetector** (TASK-LOGIC-010) provides 5-level per-embedder drift analysis
3. **Existing handlers** use legacy `DriftDetector` which lacks per-embedder granularity
4. **Integration** enables ARCH-02 compliant apples-to-apples comparison in MCP layer
5. **No new files** - modify existing purpose.rs and autonomous.rs
</rationale>

<architecture_constraints>
## From constitution.yaml (MUST NOT VIOLATE)

- **ARCH-01**: TeleologicalArray is atomic - all 13 embeddings stored/retrieved together
- **ARCH-02**: Apples-to-apples comparison - E1 compares with E1, NEVER cross-embedder
- **ARCH-03**: Autonomous operation - goals emerge from data patterns, no manual goal setting
- **ARCH-05**: All 13 embedders must be present in comparisons
- **ARCH-06**: All memory operations through MCP tools
- **ARCH-07**: Hooks control memory lifecycle
- **FAIL FAST**: All errors are fatal. No recovery attempts. No fallbacks. No `unwrap_or_default()`.

## Embedder Enum (13 variants - crates/context-graph-core/src/teleological/embedder.rs)

```rust
pub enum Embedder {
    Semantic = 0,           // E1: Core meaning
    TemporalRecent = 1,     // E2: Recent time
    TemporalPeriodic = 2,   // E3: Cyclical patterns
    TemporalPositional = 3, // E4: Sequence position
    Causal = 4,             // E5: Cause-effect
    Sparse = 5,             // E6: BM25-style lexical
    Code = 6,               // E7: Code structure
    Graph = 7,              // E8: Relationship structure
    Hdc = 8,                // E9: Holographic patterns
    Multimodal = 9,         // E10: Cross-modal
    Entity = 10,            // E11: Named entities
    LateInteraction = 11,   // E12: ColBERT tokens
    KeywordSplade = 12,     // E13: Learned expansion
}
```
</architecture_constraints>

<implementation_requirements>
## 1. INTEGRATION POINTS

### 1.1 Wire GoalDiscoveryPipeline into call_discover_sub_goals

**File**: `crates/context-graph-mcp/src/handlers/autonomous.rs`
**Method**: `call_discover_sub_goals` (line 719)

**Current State**: Uses `SubGoalDiscovery` service
**Required Change**: Also invoke `GoalDiscoveryPipeline::discover()` and return enhanced results

```rust
// Add import
use context_graph_core::autonomous::discovery::{GoalDiscoveryPipeline, DiscoveryConfig, DiscoveryResult};

// In call_discover_sub_goals, after existing SubGoalDiscovery logic:
let discovery_pipeline = GoalDiscoveryPipeline::new(comparator.clone());
let arrays: Vec<SemanticFingerprint> = /* load from teleological store */;
let config = DiscoveryConfig {
    sample_size: params.max_goals.unwrap_or(10) * 10,
    min_cluster_size: 3,
    min_coherence: params.min_confidence.unwrap_or(0.5),
    clustering_algorithm: ClusteringAlgorithm::KMeans,
    num_clusters: NumClusters::Auto,
    comparison_type: SearchStrategy::Balanced,
};
let discovery_result = discovery_pipeline.discover(&arrays, &config)?;
// Merge discovered_goals into response
```

### 1.2 Wire TeleologicalDriftDetector into handle_purpose_drift_check

**File**: `crates/context-graph-mcp/src/handlers/purpose.rs`
**Method**: `handle_purpose_drift_check` (line 710)

**Current State**: Returns basic drift info
**Required Change**: Use `TeleologicalDriftDetector` for 5-level per-embedder analysis

```rust
// Add import
use context_graph_core::autonomous::drift::{
    TeleologicalDriftDetector, DriftLevel, DriftResult, DriftError, DriftThresholds
};

// Replace legacy drift check with TeleologicalDriftDetector
let detector = TeleologicalDriftDetector::new(comparator.clone());
let result: DriftResult = detector.check_drift(&memories, &goal_fingerprint, strategy)?;

// Return per-embedder breakdown
json!({
    "overall_drift": {
        "level": format!("{:?}", result.overall_drift.drift_level),
        "similarity": result.overall_drift.similarity,
        "drift_score": result.overall_drift.drift_score,
        "has_drifted": result.overall_drift.has_drifted
    },
    "per_embedder_drift": result.per_embedder_drift.embedder_drift.iter().map(|e| json!({
        "embedder": format!("{:?}", e.embedder),
        "similarity": e.similarity,
        "drift_score": e.drift_score,
        "drift_level": format!("{:?}", e.drift_level)
    })).collect::<Vec<_>>(),
    "most_drifted_embedders": result.most_drifted_embedders.iter().take(5).map(/*...*/).collect(),
    "recommendations": result.recommendations.iter().map(/*...*/).collect(),
    "analyzed_count": result.analyzed_count,
    "timestamp": result.timestamp.to_rfc3339()
})
```

### 1.3 Wire TeleologicalDriftDetector into call_get_alignment_drift

**File**: `crates/context-graph-mcp/src/handlers/autonomous.rs`
**Method**: `call_get_alignment_drift` (line 331)

**Current State**: Uses legacy `DriftDetector`
**Required Change**: Use `TeleologicalDriftDetector` with history tracking

```rust
// Replace DriftDetector with TeleologicalDriftDetector
let mut detector = TeleologicalDriftDetector::new(comparator.clone());
let result = detector.check_drift_with_history(&memories, &goal_fingerprint, goal_id, strategy)?;

// Include trend analysis if available
if let Some(trend) = result.trend {
    json!({
        "trend": {
            "direction": format!("{:?}", trend.direction),
            "slope": trend.slope,
            "velocity": trend.velocity,
            "projected_critical_in": trend.projected_critical_in
        }
    })
}
```

## 2. ERROR HANDLING (FAIL FAST)

### 2.1 Error Propagation - NO Recovery

```rust
// CORRECT: Propagate errors immediately
let result = detector.check_drift(&memories, &goal, strategy)
    .map_err(|e| match e {
        DriftError::EmptyMemories => HandlerError::new(
            ErrorCode::InvalidParams,
            "Empty memories slice - cannot check drift"
        ),
        DriftError::InvalidGoal { reason } => HandlerError::new(
            ErrorCode::InvalidParams,
            format!("Invalid goal fingerprint: {}", reason)
        ),
        DriftError::ComparisonFailed { embedder, reason } => HandlerError::new(
            ErrorCode::InternalError,
            format!("Comparison failed for {:?}: {}", embedder, reason)
        ),
        DriftError::InvalidThresholds { reason } => HandlerError::new(
            ErrorCode::InternalError,
            format!("Invalid thresholds: {}", reason)
        ),
        DriftError::ComparisonValidationFailed { reason } => HandlerError::new(
            ErrorCode::InternalError,
            format!("Comparison validation failed: {}", reason)
        ),
    })?;

// FORBIDDEN: No recovery, no defaults
// result.unwrap_or_default()  // NEVER
// result.ok()                 // NEVER unless explicitly documented
// result.unwrap_or(fallback)  // NEVER
```

### 2.2 Input Validation - Fail Early

```rust
// Validate inputs BEFORE any processing
if memory_ids.is_empty() {
    return Err(HandlerError::new(
        ErrorCode::InvalidParams,
        "memory_ids array must not be empty"
    ));
}

if goal_id.is_empty() {
    return Err(HandlerError::new(
        ErrorCode::InvalidParams,
        "goal_id must not be empty"
    ));
}

// Validate each memory exists
let mut memories = Vec::with_capacity(memory_ids.len());
for id in &memory_ids {
    let fingerprint = store.retrieve(id).await
        .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
        .ok_or_else(|| HandlerError::new(
            ErrorCode::NotFound,
            format!("Memory {} not found", id)
        ))?;
    memories.push(fingerprint);
}
```

## 3. RESPONSE TYPE UPDATES

### 3.1 Purpose Drift Check Response (Enhanced)

```rust
#[derive(Debug, Serialize)]
pub struct PurposeDriftCheckResponse {
    pub overall_drift: OverallDriftResponse,
    pub per_embedder_drift: Vec<EmbedderDriftResponse>,
    pub most_drifted_embedders: Vec<EmbedderDriftResponse>,
    pub recommendations: Vec<DriftRecommendationResponse>,
    pub trend: Option<TrendResponse>,
    pub analyzed_count: usize,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct OverallDriftResponse {
    pub level: String,           // "Critical" | "High" | "Medium" | "Low" | "None"
    pub similarity: f32,         // [0.0, 1.0]
    pub drift_score: f32,        // 1.0 - similarity
    pub has_drifted: bool,       // level != None
}

#[derive(Debug, Serialize)]
pub struct EmbedderDriftResponse {
    pub embedder: String,        // "Semantic" | "TemporalRecent" | etc.
    pub embedder_index: usize,   // 0-12
    pub similarity: f32,
    pub drift_score: f32,
    pub drift_level: String,
}

#[derive(Debug, Serialize)]
pub struct TrendResponse {
    pub direction: String,       // "Improving" | "Stable" | "Worsening"
    pub slope: f32,
    pub velocity: f32,
    pub projected_critical_in: Option<usize>,  // Days until critical (if worsening)
}
```

### 3.2 Goal Discovery Response (Enhanced)

```rust
#[derive(Debug, Serialize)]
pub struct DiscoverSubGoalsResponse {
    pub discovered_goals: Vec<DiscoveredGoalResponse>,
    pub cluster_analysis: ClusterAnalysisResponse,
    pub discovery_metadata: DiscoveryMetadataResponse,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredGoalResponse {
    pub goal_id: String,
    pub description: String,
    pub level: String,           // "NorthStar" | "Strategic" | "Tactical" | "Immediate"
    pub confidence: f32,
    pub member_count: usize,
    pub centroid_id: String,     // UUID of centroid fingerprint
    pub centroid_strength: HashMap<String, f32>,  // Per-embedder strength
    pub dominant_embedders: Vec<String>,
    pub keywords: Vec<String>,
    pub coherence_score: f32,
}

#[derive(Debug, Serialize)]
pub struct DiscoveryMetadataResponse {
    pub total_arrays_analyzed: usize,
    pub clusters_found: usize,
    pub clusters_above_threshold: usize,
    pub algorithm_used: String,
    pub processing_time_ms: u64,
}
```
</implementation_requirements>

<test_requirements>
## REAL DATA TESTING - NO MOCKS

### Test Setup - Real TeleologicalArrays

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::teleological::array::TeleologicalArray;
    use context_graph_core::teleological::comparator::TeleologicalComparator;
    use context_graph_core::autonomous::drift::{TeleologicalDriftDetector, DriftThresholds};
    use context_graph_core::autonomous::discovery::{GoalDiscoveryPipeline, DiscoveryConfig};

    /// Create a real TeleologicalArray with specified characteristics
    fn create_test_array(semantic_strength: f32, temporal_strength: f32) -> SemanticFingerprint {
        let mut embeddings = [[0.0f32; 384]; 13];
        // Set realistic embedding values (NOT random, NOT zeros)
        for i in 0..384 {
            embeddings[0][i] = (i as f32 * 0.01 * semantic_strength).sin();
            embeddings[1][i] = (i as f32 * 0.02 * temporal_strength).cos();
            // ... set all 13 embedders with meaningful patterns
        }
        SemanticFingerprint::from_embeddings(embeddings)
    }

    /// Create a cluster of related arrays for discovery testing
    fn create_test_cluster(center: &SemanticFingerprint, variance: f32, count: usize) -> Vec<SemanticFingerprint> {
        (0..count).map(|i| {
            let mut modified = center.clone();
            // Add controlled variance to create cluster
            for e in 0..13 {
                for d in 0..384 {
                    modified.embeddings[e][d] += (i as f32 * variance * 0.001).sin();
                }
            }
            modified
        }).collect()
    }
}
```

### Required Test Cases

```rust
// === DRIFT DETECTION TESTS ===

#[tokio::test]
async fn test_purpose_drift_check_returns_per_embedder_breakdown() {
    // Setup: Create goal and drifted memories
    let goal = create_test_array(1.0, 0.8);
    let memories = vec![
        create_test_array(0.9, 0.7),  // Slightly drifted
        create_test_array(0.3, 0.9),  // Semantically drifted
    ];

    // Execute
    let result = handler.handle_purpose_drift_check(params).await?;

    // Verify: Response contains all 13 embedders
    assert_eq!(result["per_embedder_drift"].as_array().unwrap().len(), 13);

    // Verify: Each embedder has required fields
    for drift in result["per_embedder_drift"].as_array().unwrap() {
        assert!(drift["embedder"].is_string());
        assert!(drift["similarity"].is_number());
        assert!(drift["drift_level"].is_string());
    }

    // Verify: Most drifted is sorted (worst first)
    let most_drifted = result["most_drifted_embedders"].as_array().unwrap();
    for window in most_drifted.windows(2) {
        let level_a = parse_drift_level(&window[0]["drift_level"]);
        let level_b = parse_drift_level(&window[1]["drift_level"]);
        assert!(level_a <= level_b, "Most drifted should be sorted worst-first");
    }
}

#[tokio::test]
async fn test_drift_check_fail_fast_empty_memories() {
    let params = DriftCheckParams {
        memory_ids: vec![],  // Empty
        goal_id: "test-goal".to_string(),
    };

    let result = handler.handle_purpose_drift_check(params).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidParams);
    assert!(err.message.contains("empty"));
}

#[tokio::test]
async fn test_drift_check_fail_fast_invalid_goal() {
    let goal = create_test_array_with_nan();  // Contains NaN
    store.store(&goal).await?;

    let params = DriftCheckParams {
        memory_ids: vec![memory_id],
        goal_id: goal.id.to_string(),
    };

    let result = handler.handle_purpose_drift_check(params).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    // Should propagate DriftError::InvalidGoal
}

#[tokio::test]
async fn test_drift_level_thresholds() {
    // Test each of 5 drift levels
    let test_cases = [
        (0.90, "None"),      // >= 0.85
        (0.75, "Low"),       // >= 0.70, < 0.85
        (0.60, "Medium"),    // >= 0.55, < 0.70
        (0.45, "High"),      // >= 0.40, < 0.55
        (0.30, "Critical"),  // < 0.40
    ];

    for (similarity, expected_level) in test_cases {
        let (goal, memories) = create_arrays_with_similarity(similarity);
        let result = handler.handle_purpose_drift_check(params).await?;
        assert_eq!(result["overall_drift"]["level"], expected_level);
    }
}

// === GOAL DISCOVERY TESTS ===

#[tokio::test]
async fn test_discover_sub_goals_uses_clustering() {
    // Create 3 distinct clusters
    let cluster1 = create_test_cluster(&create_test_array(1.0, 0.0), 0.1, 10);
    let cluster2 = create_test_cluster(&create_test_array(0.0, 1.0), 0.1, 10);
    let cluster3 = create_test_cluster(&create_test_array(0.5, 0.5), 0.1, 10);

    // Store all arrays
    for array in cluster1.iter().chain(cluster2.iter()).chain(cluster3.iter()) {
        store.store(array).await?;
    }

    let params = DiscoverSubGoalsParams {
        parent_goal_id: parent_goal.id.to_string(),
        max_goals: Some(5),
        min_confidence: Some(0.5),
    };

    let result = handler.call_discover_sub_goals(params).await?;

    // Verify: Found at least 3 clusters
    assert!(result["discovered_goals"].as_array().unwrap().len() >= 3);

    // Verify: Each goal has coherence_score
    for goal in result["discovered_goals"].as_array().unwrap() {
        assert!(goal["coherence_score"].as_f64().unwrap() > 0.0);
        assert!(goal["member_count"].as_u64().unwrap() >= 3);
    }
}

#[tokio::test]
async fn test_discovery_returns_dominant_embedders() {
    // Create cluster with strong Semantic embedder
    let arrays = create_semantic_dominant_cluster(20);
    for array in &arrays {
        store.store(array).await?;
    }

    let result = handler.call_discover_sub_goals(params).await?;

    // Verify: Semantic should be in dominant_embedders
    let goal = &result["discovered_goals"][0];
    let dominant = goal["dominant_embedders"].as_array().unwrap();
    assert!(dominant.iter().any(|e| e == "Semantic"));
}

// === INTEGRATION TESTS ===

#[tokio::test]
async fn test_drift_and_discovery_use_same_comparator() {
    // Both TeleologicalDriftDetector and GoalDiscoveryPipeline should use
    // TeleologicalComparator for ARCH-02 compliant comparison

    let comparator = TeleologicalComparator::new();
    let detector = TeleologicalDriftDetector::new(comparator.clone());
    let pipeline = GoalDiscoveryPipeline::new(comparator.clone());

    // Verify: Same arrays produce consistent similarity scores
    let a = create_test_array(1.0, 0.5);
    let b = create_test_array(0.9, 0.6);

    let drift_result = detector.check_drift(&[a.clone()], &b, SearchStrategy::Balanced)?;
    let comparison = comparator.compare(&a, &b, &SearchStrategy::Balanced)?;

    // Similarity from drift should match direct comparison
    assert!((drift_result.overall_drift.similarity - comparison.overall_similarity).abs() < 0.001);
}
```
</test_requirements>

<source_of_truth>
## Full State Verification Protocol

### 1. Before ANY Handler Call

```rust
// Print handler state
println!("=== SOURCE OF TRUTH: Handler State ===");
println!("Store connected: {:?}", store.is_connected());
println!("Comparator config: {:?}", comparator.config());
println!("Detector thresholds: {:?}", detector.thresholds);
```

### 2. After Handler Execution

```rust
let result = handler.handle_purpose_drift_check(params).await?;

// VERIFY: Response has all 13 embedders
let per_embedder = result["per_embedder_drift"].as_array().unwrap();
assert_eq!(per_embedder.len(), 13, "Must have exactly 13 embedders");

// VERIFY: Each embedder index is unique
let indices: HashSet<_> = per_embedder.iter()
    .map(|e| e["embedder_index"].as_u64().unwrap())
    .collect();
assert_eq!(indices.len(), 13, "All embedder indices must be unique");

// VERIFY: Similarities in valid range
for drift in per_embedder {
    let sim = drift["similarity"].as_f64().unwrap();
    assert!(sim >= 0.0 && sim <= 1.0, "Similarity must be in [0.0, 1.0]");
    assert!(!sim.is_nan(), "Similarity must not be NaN");
}

// VERIFY: Drift score = 1.0 - similarity
let overall_sim = result["overall_drift"]["similarity"].as_f64().unwrap();
let overall_drift = result["overall_drift"]["drift_score"].as_f64().unwrap();
assert!((overall_drift - (1.0 - overall_sim)).abs() < 0.0001);

// VERIFY: Drift level matches similarity thresholds
let level = result["overall_drift"]["level"].as_str().unwrap();
match level {
    "None" => assert!(overall_sim >= 0.85),
    "Low" => assert!(overall_sim >= 0.70 && overall_sim < 0.85),
    "Medium" => assert!(overall_sim >= 0.55 && overall_sim < 0.70),
    "High" => assert!(overall_sim >= 0.40 && overall_sim < 0.55),
    "Critical" => assert!(overall_sim < 0.40),
    _ => panic!("Invalid drift level: {}", level),
}

// VERIFY: Recommendations only for Medium+ drift
for rec in result["recommendations"].as_array().unwrap() {
    let level = rec["for_drift_level"].as_str().unwrap();
    assert!(level == "Medium" || level == "High" || level == "Critical");
}
```

### 3. Manual Verification Checklist

| Check | How to Verify | Expected |
|-------|---------------|----------|
| Handler returns 13 embedders | `result["per_embedder_drift"].len()` | 13 |
| No NaN in response | Check all f32 fields | All finite |
| Drift levels correct | Match similarity to thresholds | Matches |
| History recorded | Check detector.history after call | Entry added |
| Trend after 3+ calls | Call 3 times, check trend | Some(TrendAnalysis) |
| Fail fast on bad input | Pass empty/invalid params | Err with clear message |
</source_of_truth>

<verification_commands>
## Verification Commands

```bash
# 1. Verify purpose.rs exists with expected line count
wc -l crates/context-graph-mcp/src/handlers/purpose.rs
# Expected: ~986

# 2. Verify autonomous.rs exists with expected line count
wc -l crates/context-graph-mcp/src/handlers/autonomous.rs
# Expected: ~1112

# 3. Verify TeleologicalDriftDetector import in purpose.rs
grep -c "TeleologicalDriftDetector" crates/context-graph-mcp/src/handlers/purpose.rs
# After integration: >= 1

# 4. Verify GoalDiscoveryPipeline import in autonomous.rs
grep -c "GoalDiscoveryPipeline" crates/context-graph-mcp/src/handlers/autonomous.rs
# After integration: >= 1

# 5. Verify no unwrap_or_default (fail-fast)
grep -c "unwrap_or_default" crates/context-graph-mcp/src/handlers/purpose.rs
grep -c "unwrap_or_default" crates/context-graph-mcp/src/handlers/autonomous.rs
# Expected: 0 in both

# 6. Verify per-embedder response structure
grep -c "per_embedder_drift" crates/context-graph-mcp/src/handlers/purpose.rs
# After integration: >= 1

# 7. Compile check
cargo check -p context-graph-mcp
# Expected: success

# 8. Run purpose handler tests
cargo test -p context-graph-mcp handlers::purpose -- --nocapture
# Expected: all tests pass

# 9. Run autonomous handler tests
cargo test -p context-graph-mcp handlers::autonomous -- --nocapture
# Expected: all tests pass

# 10. Clippy validation
cargo clippy -p context-graph-mcp -- -D warnings
# Expected: no errors
```
</verification_commands>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/purpose.rs">
    - Add import: `use context_graph_core::autonomous::drift::{TeleologicalDriftDetector, DriftLevel, DriftResult, DriftError};`
    - Modify `handle_purpose_drift_check` to use TeleologicalDriftDetector
    - Update response to include per_embedder_drift array with 13 entries
    - Add DriftError -> HandlerError mapping
  </file>
  <file path="crates/context-graph-mcp/src/handlers/autonomous.rs">
    - Add import: `use context_graph_core::autonomous::discovery::{GoalDiscoveryPipeline, DiscoveryConfig, DiscoveryResult};`
    - Add import: `use context_graph_core::autonomous::drift::TeleologicalDriftDetector;`
    - Modify `call_discover_sub_goals` to also use GoalDiscoveryPipeline
    - Modify `call_get_alignment_drift` to use TeleologicalDriftDetector with history
    - Update responses to include enhanced discovery/drift metadata
  </file>
</files_to_modify>

<files_to_create>
  <!-- NO NEW FILES - Integration only -->
</files_to_create>

<validation_criteria>
  <criterion>handle_purpose_drift_check returns per_embedder_drift with exactly 13 entries</criterion>
  <criterion>Each embedder in per_embedder_drift has: embedder name, index (0-12), similarity, drift_score, drift_level</criterion>
  <criterion>Drift levels match 5-level enum: Critical, High, Medium, Low, None</criterion>
  <criterion>call_discover_sub_goals returns goals with coherence_score and dominant_embedders</criterion>
  <criterion>Empty input arrays return Err, not empty Ok</criterion>
  <criterion>Invalid goal fingerprint returns Err with clear reason</criterion>
  <criterion>All errors propagate immediately (no recovery attempts)</criterion>
  <criterion>Responses include timestamp in RFC3339 format</criterion>
  <criterion>cargo test passes for both handler modules</criterion>
  <criterion>cargo clippy -D warnings passes</criterion>
</validation_criteria>

<anti_patterns>
## FORBIDDEN Patterns

```rust
// NEVER: Silent failure with default
result.unwrap_or_default()

// NEVER: Ignore errors
let _ = detector.check_drift(...);

// NEVER: Convert error to None
result.ok()  // Unless explicitly documented why

// NEVER: Partial embedder response
json!({ "per_embedder_drift": only_3_embedders })  // Must be 13

// NEVER: Cross-embedder comparison
compare(memory.semantic, goal.temporal)  // ARCH-02 violation

// NEVER: Create new handler files
// purpose.rs and autonomous.rs already exist
```
</anti_patterns>

<test_commands>
  <command>cargo test -p context-graph-mcp handlers::purpose -- --nocapture</command>
  <command>cargo test -p context-graph-mcp handlers::autonomous -- --nocapture</command>
  <command>cargo test -p context-graph-mcp --lib -- --nocapture</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>
```
