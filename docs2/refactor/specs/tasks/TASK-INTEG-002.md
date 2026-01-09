# TASK-INTEG-002: Purpose and Goal MCP Handlers

```xml
<task_spec id="TASK-INTEG-002" version="1.0">
<metadata>
  <title>Implement Purpose and Goal MCP Tool Handlers</title>
  <status>todo</status>
  <layer>integration</layer>
  <sequence>22</sequence>
  <implements>
    <requirement_ref>REQ-MCP-PURPOSE-01</requirement_ref>
    <requirement_ref>REQ-MCP-GOAL-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-009</task_ref>
    <task_ref>TASK-LOGIC-010</task_ref>
    <task_ref>TASK-INTEG-001</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
Purpose and goal handlers implement autonomous goal discovery, alignment checking, and
drift detection. These replace the removed manual North Star system with clustering-based
goal emergence.
</context>

<objective>
Implement MCP handlers for purpose/discover_goals, purpose/goal_alignment, purpose/drift_check,
and goal/cluster_analysis tools.
</objective>

<rationale>
Purpose handlers enable autonomous goal management:
1. purpose/discover_goals uses clustering for goal emergence
2. purpose/goal_alignment computes array-to-array alignment
3. purpose/drift_check provides per-embedder drift analysis
4. goal/cluster_analysis supports goal hierarchy construction
</rationale>

<input_context_files>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#4-autonomous-purpose-tools</file>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#6-goal-clustering-tools</file>
  <file purpose="discovery">crates/context-graph-core/src/autonomous/discovery.rs</file>
  <file purpose="drift">crates/context-graph-core/src/autonomous/drift.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-009 complete (GoalDiscoveryPipeline exists)</check>
  <check>TASK-LOGIC-010 complete (TeleologicalDriftDetector exists)</check>
  <check>TASK-INTEG-001 complete (MemoryHandler exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement purpose/discover_goals handler</item>
    <item>Implement purpose/goal_alignment handler</item>
    <item>Implement purpose/drift_check handler</item>
    <item>Implement goal/cluster_analysis handler</item>
    <item>JSON-RPC request/response serialization</item>
    <item>Rate limiting for expensive operations</item>
  </in_scope>
  <out_of_scope>
    <item>Consciousness handlers (TASK-INTEG-003)</item>
    <item>Hook handlers (TASK-INTEG-004)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/purpose.rs">
      use crate::protocol::{HandlerError, ErrorCode};
      use context_graph_core::autonomous::discovery::{GoalDiscoveryPipeline, DiscoveryConfig};
      use context_graph_core::autonomous::drift::TeleologicalDriftDetector;
      use context_graph_storage::teleological::store::TeleologicalArrayStore;

      /// Purpose MCP handler for goal discovery and alignment.
      pub struct PurposeHandler {
          store: Arc<dyn TeleologicalArrayStore>,
          discovery_pipeline: Arc<GoalDiscoveryPipeline>,
          drift_detector: Arc<RwLock<TeleologicalDriftDetector>>,
          goal_store: Arc<GoalStore>,
      }

      impl PurposeHandler {
          pub fn new(
              store: Arc<dyn TeleologicalArrayStore>,
              discovery_pipeline: Arc<GoalDiscoveryPipeline>,
              drift_detector: Arc<RwLock<TeleologicalDriftDetector>>,
              goal_store: Arc<GoalStore>,
          ) -> Self;

          /// Handle purpose/discover_goals request.
          pub async fn handle_discover_goals(
              &self,
              params: DiscoverGoalsParams,
          ) -> Result<DiscoverGoalsResponse, HandlerError>;

          /// Handle purpose/goal_alignment request.
          pub async fn handle_goal_alignment(
              &self,
              params: GoalAlignmentParams,
          ) -> Result<GoalAlignmentResponse, HandlerError>;

          /// Handle purpose/drift_check request.
          pub async fn handle_drift_check(
              &self,
              params: DriftCheckParams,
          ) -> Result<DriftCheckResponse, HandlerError>;
      }

      /// Goal clustering handler.
      pub struct GoalHandler {
          discovery_pipeline: Arc<GoalDiscoveryPipeline>,
          store: Arc<dyn TeleologicalArrayStore>,
      }

      impl GoalHandler {
          /// Handle goal/cluster_analysis request.
          pub async fn handle_cluster_analysis(
              &self,
              params: ClusterAnalysisParams,
          ) -> Result<ClusterAnalysisResponse, HandlerError>;
      }

      // Request/Response types
      #[derive(Debug, Deserialize)]
      pub struct DiscoverGoalsParams {
          pub namespace: Option<String>,
          pub discovery_config: DiscoveryConfigParams,
          pub comparison_type: Option<serde_json::Value>,
      }

      #[derive(Debug, Serialize)]
      pub struct DiscoverGoalsResponse {
          pub discovered_goals: Vec<DiscoveredGoalItem>,
          pub discovery_info: DiscoveryInfo,
      }

      #[derive(Debug, Deserialize)]
      pub struct GoalAlignmentParams {
          pub memory_id: Uuid,
          pub goal_id: String,
          pub comparison_type: Option<serde_json::Value>,
      }

      #[derive(Debug, Serialize)]
      pub struct GoalAlignmentResponse {
          pub alignment_score: f32,
          pub interpretation: String,
          pub per_embedder_alignment: PerEmbedderAlignment,
          pub goal_info: GoalInfo,
          pub contribution_analysis: ContributionAnalysis,
      }

      #[derive(Debug, Deserialize)]
      pub struct DriftCheckParams {
          pub memory_ids: Vec<Uuid>,
          pub goal_id: String,
          pub comparison_type: Option<serde_json::Value>,
      }

      #[derive(Debug, Serialize)]
      pub struct DriftCheckResponse {
          pub overall_drift: OverallDriftInfo,
          pub per_embedder_drift: PerEmbedderDriftInfo,
          pub most_drifted_embedders: Vec<EmbedderDriftItem>,
          pub recommendations: Vec<DriftRecommendationItem>,
          pub trend: Option<TrendInfo>,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Discover goals rate limited to 10/hour</constraint>
    <constraint>Cluster analysis rate limited to 10/hour</constraint>
    <constraint>Alignment uses teleological array comparison</constraint>
    <constraint>Drift returns per-embedder breakdown</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-mcp handlers::purpose</command>
    <command>cargo test -p context-graph-mcp handlers::goal</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-mcp/src/handlers/purpose.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::protocol::{HandlerError, ErrorCode};
use context_graph_core::autonomous::discovery::{
    GoalDiscoveryPipeline, DiscoveryConfig, DiscoveryResult,
    ClusteringAlgorithm, NumClusters,
};
use context_graph_core::autonomous::drift::{
    TeleologicalDriftDetector, DriftResult, DriftLevel,
};
use context_graph_core::teleology::comparison::ComparisonType;
use context_graph_storage::teleological::store::TeleologicalArrayStore;

pub struct PurposeHandler {
    store: Arc<dyn TeleologicalArrayStore + Send + Sync>,
    discovery_pipeline: Arc<GoalDiscoveryPipeline>,
    drift_detector: Arc<RwLock<TeleologicalDriftDetector>>,
    goal_store: Arc<GoalStore>,
}

impl PurposeHandler {
    pub fn new(
        store: Arc<dyn TeleologicalArrayStore + Send + Sync>,
        discovery_pipeline: Arc<GoalDiscoveryPipeline>,
        drift_detector: Arc<RwLock<TeleologicalDriftDetector>>,
        goal_store: Arc<GoalStore>,
    ) -> Self {
        Self {
            store,
            discovery_pipeline,
            drift_detector,
            goal_store,
        }
    }

    pub async fn handle_discover_goals(
        &self,
        params: DiscoverGoalsParams,
    ) -> Result<DiscoverGoalsResponse, HandlerError> {
        // Load arrays from namespace
        let namespace = params.namespace.as_deref();
        let arrays = self.store
            .list_arrays(namespace, Some(params.discovery_config.sample_size))
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        if arrays.is_empty() {
            return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "No arrays found in namespace",
            ));
        }

        // Convert config
        let config = self.parse_discovery_config(&params.discovery_config)?;

        // Run discovery
        let result = self.discovery_pipeline
            .discover(&arrays, &config)
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Store discovered goals
        for goal in &result.discovered_goals {
            self.goal_store.store_goal(goal).await
                .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;
        }

        // Build response
        let discovered_goals: Vec<_> = result.discovered_goals.iter()
            .map(|g| DiscoveredGoalItem {
                goal_id: g.goal_id.clone(),
                description: g.description.clone(),
                suggested_level: format!("{:?}", g.level),
                confidence: g.confidence,
                member_count: g.member_count,
                teleological_array_id: g.centroid.id.to_string(),
                centroid_strength: self.build_centroid_strength(&g.centroid_strength),
                dominant_embedders: g.dominant_embedders.iter()
                    .map(|e| format!("{:?}", e))
                    .collect(),
                keywords: g.keywords.clone(),
                coherence_score: g.coherence_score,
            })
            .collect();

        Ok(DiscoverGoalsResponse {
            discovered_goals,
            discovery_info: DiscoveryInfo {
                total_arrays_analyzed: result.total_arrays_analyzed,
                clusters_found: result.clusters_found,
                clusters_above_threshold: result.clusters_above_threshold,
                processing_time_ms: 0, // TODO: measure
            },
        })
    }

    pub async fn handle_goal_alignment(
        &self,
        params: GoalAlignmentParams,
    ) -> Result<GoalAlignmentResponse, HandlerError> {
        // Load memory
        let memory = self.store
            .retrieve(&params.memory_id)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Memory not found"))?;

        // Load goal
        let goal = self.goal_store
            .get_goal(&params.goal_id)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Goal not found"))?;

        // Get comparison type
        let comparison_type = if let Some(ct) = params.comparison_type {
            self.parse_comparison_type(&ct)?
        } else {
            ComparisonType::default()
        };

        // Compare memory to goal centroid
        let result = self.discovery_pipeline
            .comparator()
            .compare(&memory, &goal.centroid, &comparison_type);

        // Interpret alignment
        let interpretation = self.interpret_alignment(result.overall_similarity);

        // Per-embedder alignment
        let per_embedder = PerEmbedderAlignment {
            alignments: result.per_embedder.iter().enumerate()
                .map(|(i, score)| {
                    let embedder = Embedder::from_index(i).unwrap();
                    EmbedderAlignmentItem {
                        embedder: format!("{:?}", embedder),
                        score: score.unwrap_or(0.0),
                        interpretation: self.interpret_embedder_alignment(score.unwrap_or(0.0)),
                    }
                })
                .collect(),
        };

        // Contribution analysis
        let contribution = ContributionAnalysis {
            strengthens_goal: result.overall_similarity > 0.7,
            novelty_contribution: self.compute_novelty(&memory, &goal),
            coherence_contribution: result.coherence.unwrap_or(0.0),
        };

        Ok(GoalAlignmentResponse {
            alignment_score: result.overall_similarity,
            interpretation,
            per_embedder_alignment: per_embedder,
            goal_info: GoalInfo {
                goal_id: params.goal_id,
                description: goal.description.clone(),
                level: format!("{:?}", goal.level),
            },
            contribution_analysis: contribution,
        })
    }

    pub async fn handle_drift_check(
        &self,
        params: DriftCheckParams,
    ) -> Result<DriftCheckResponse, HandlerError> {
        // Load memories
        let mut memories = Vec::with_capacity(params.memory_ids.len());
        for id in &params.memory_ids {
            if let Some(memory) = self.store.retrieve(id).await.ok().flatten() {
                memories.push(memory);
            }
        }

        if memories.is_empty() {
            return Err(HandlerError::new(
                ErrorCode::InvalidParams,
                "No valid memories found",
            ));
        }

        // Load goal
        let goal = self.goal_store
            .get_goal(&params.goal_id)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Goal not found"))?;

        // Get comparison type
        let comparison_type = if let Some(ct) = params.comparison_type {
            self.parse_comparison_type(&ct)?
        } else {
            ComparisonType::default()
        };

        // Check drift with history tracking
        let result = {
            let mut detector = self.drift_detector.write().await;
            detector.check_drift_with_history(&memories, &goal.centroid, &comparison_type)
        };

        // Build response
        let overall_drift = OverallDriftInfo {
            has_drifted: result.overall_drift.has_drifted,
            drift_score: result.overall_drift.drift_score,
            drift_level: format!("{:?}", result.overall_drift.drift_level),
        };

        let per_embedder = PerEmbedderDriftInfo {
            drifts: result.per_embedder_drift.embedder_drift.iter()
                .map(|d| EmbedderDriftItem {
                    embedder: format!("{:?}", d.embedder),
                    similarity: d.similarity,
                    drift_level: format!("{:?}", d.drift_level),
                })
                .collect(),
        };

        let most_drifted: Vec<_> = result.most_drifted_embedders.iter()
            .map(|d| EmbedderDriftItem {
                embedder: format!("{:?}", d.embedder),
                similarity: d.similarity,
                drift_level: format!("{:?}", d.drift_level),
            })
            .collect();

        let recommendations: Vec<_> = result.recommendations.iter()
            .map(|r| DriftRecommendationItem {
                embedder: format!("{:?}", r.embedder),
                issue: r.issue.clone(),
                suggestion: r.suggestion.clone(),
            })
            .collect();

        let trend = result.trend.map(|t| TrendInfo {
            direction: format!("{:?}", t.direction),
            velocity: t.velocity,
            projected_critical_in: t.projected_critical_in,
        });

        Ok(DriftCheckResponse {
            overall_drift,
            per_embedder_drift: per_embedder,
            most_drifted_embedders: most_drifted,
            recommendations,
            trend,
        })
    }

    fn parse_discovery_config(&self, params: &DiscoveryConfigParams) -> Result<DiscoveryConfig, HandlerError> {
        let algorithm = match params.clustering_algorithm.as_deref() {
            Some("kmeans") | None => ClusteringAlgorithm::KMeans,
            Some("hdbscan") => ClusteringAlgorithm::HDBSCAN { min_samples: 3 },
            Some("spectral") => ClusteringAlgorithm::Spectral { n_neighbors: 10 },
            _ => return Err(HandlerError::new(ErrorCode::InvalidParams, "Unknown algorithm")),
        };

        let num_clusters = match params.num_clusters.as_deref() {
            Some("auto") | None => NumClusters::Auto,
            Some(n) => NumClusters::Fixed(n.parse().unwrap_or(10)),
        };

        Ok(DiscoveryConfig {
            sample_size: params.sample_size,
            min_cluster_size: params.min_cluster_size,
            min_coherence: params.min_coherence,
            clustering_algorithm: algorithm,
            num_clusters,
            comparison_type: ComparisonType::default(),
        })
    }

    fn interpret_alignment(&self, score: f32) -> String {
        if score >= 0.9 { "strongly_aligned" }
        else if score >= 0.7 { "highly_aligned" }
        else if score >= 0.5 { "moderately_aligned" }
        else if score >= 0.3 { "weakly_aligned" }
        else { "misaligned" }.to_string()
    }

    fn interpret_embedder_alignment(&self, score: f32) -> String {
        if score >= 0.8 { "strong" }
        else if score >= 0.6 { "moderate" }
        else if score >= 0.4 { "weak" }
        else { "poor" }.to_string()
    }

    fn parse_comparison_type(&self, ct: &serde_json::Value) -> Result<ComparisonType, HandlerError> {
        serde_json::from_value(ct.clone())
            .map_err(|e| HandlerError::new(ErrorCode::InvalidParams, e.to_string()))
    }

    fn compute_novelty(&self, _memory: &TeleologicalArray, _goal: &StoredGoal) -> f32 {
        0.5 // TODO: implement novelty computation
    }

    fn build_centroid_strength(&self, strengths: &EmbedderStrengths) -> HashMap<String, f32> {
        Embedder::all()
            .map(|e| (format!("{:?}", e), strengths.strengths[e.index()]))
            .collect()
    }
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct DiscoverGoalsParams {
    pub namespace: Option<String>,
    pub discovery_config: DiscoveryConfigParams,
    pub comparison_type: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct DiscoveryConfigParams {
    pub sample_size: usize,
    pub min_cluster_size: usize,
    pub min_coherence: f32,
    pub clustering_algorithm: Option<String>,
    pub num_clusters: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscoverGoalsResponse {
    pub discovered_goals: Vec<DiscoveredGoalItem>,
    pub discovery_info: DiscoveryInfo,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredGoalItem {
    pub goal_id: String,
    pub description: String,
    pub suggested_level: String,
    pub confidence: f32,
    pub member_count: usize,
    pub teleological_array_id: String,
    pub centroid_strength: HashMap<String, f32>,
    pub dominant_embedders: Vec<String>,
    pub keywords: Vec<String>,
    pub coherence_score: f32,
}

// Additional types...

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_goals() {
        // Test goal discovery
    }

    #[tokio::test]
    async fn test_goal_alignment() {
        // Test alignment checking
    }

    #[tokio::test]
    async fn test_drift_check() {
        // Test drift detection
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/purpose.rs">
    Purpose MCP handler implementation
  </file>
  <file path="crates/context-graph-mcp/src/handlers/goal.rs">
    Goal clustering MCP handler implementation
  </file>
  <file path="crates/context-graph-mcp/src/stores/goal_store.rs">
    Goal storage for discovered goals
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/mod.rs">
    Add: pub mod purpose; pub mod goal;
  </file>
  <file path="crates/context-graph-mcp/src/handlers/core.rs">
    Add dispatch routes for purpose/* and goal/* tools
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>purpose/discover_goals returns valid discovered goals</criterion>
  <criterion>purpose/goal_alignment computes array-to-array similarity</criterion>
  <criterion>purpose/drift_check returns per-embedder breakdown</criterion>
  <criterion>goal/cluster_analysis performs autonomous clustering</criterion>
  <criterion>Rate limits enforced for expensive operations</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp handlers::purpose -- --nocapture</command>
  <command>cargo test -p context-graph-mcp handlers::goal -- --nocapture</command>
</test_commands>
</task_spec>
```
