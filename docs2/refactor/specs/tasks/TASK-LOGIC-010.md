# TASK-LOGIC-010: Drift Detection

```xml
<task_spec id="TASK-LOGIC-010" version="1.0">
<metadata>
  <title>Implement Teleological Drift Detection</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>20</sequence>
  <implements>
    <requirement_ref>REQ-DRIFT-DETECTION-01</requirement_ref>
    <requirement_ref>REQ-PURPOSE-CHECK-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-004</task_ref>
    <task_ref>TASK-LOGIC-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
Drift detection monitors whether recent work has diverged from established goals using
teleological array comparison. Unlike the broken single-embedding approach, drift is
computed per-embedder, providing granular insight into which semantic dimensions are
drifting.
</context>

<objective>
Implement TeleologicalDriftDetector that analyzes per-embedder drift, classifies drift
severity, tracks trends over time, and generates actionable recommendations.
</objective>

<rationale>
Per-embedder drift detection provides:
1. Granular insight into which dimensions are diverging
2. Early warning for different types of drift (semantic, temporal, causal)
3. Trend analysis for proactive intervention
4. Recommendations based on specific embedder drift patterns
</rationale>

<input_context_files>
  <file purpose="comparator">crates/context-graph-core/src/teleology/comparator.rs</file>
  <file purpose="discovery">crates/context-graph-core/src/autonomous/discovery.rs</file>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#purpose/drift_check</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-004 complete (TeleologicalComparator exists)</check>
  <check>TASK-LOGIC-009 complete (GoalDiscoveryPipeline exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create TeleologicalDriftDetector struct</item>
    <item>Implement per-embedder drift analysis</item>
    <item>Classify drift levels (None, Low, Medium, High, Critical)</item>
    <item>Track drift trends over time</item>
    <item>Generate drift recommendations</item>
    <item>Store drift history for trend analysis</item>
  </in_scope>
  <out_of_scope>
    <item>MCP handler implementation (TASK-INTEG-*)</item>
    <item>Automatic drift correction</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/autonomous/drift.rs">
      use crate::teleology::array::TeleologicalArray;
      use crate::teleology::comparison::ComparisonType;
      use crate::teleology::comparator::TeleologicalComparator;
      use crate::teleology::embedder::Embedder;

      /// Drift severity levels.
      #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
      pub enum DriftLevel {
          None,
          Low,
          Medium,
          High,
          Critical,
      }

      /// Drift trend direction.
      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      pub enum DriftTrend {
          Improving,
          Stable,
          Worsening,
      }

      /// Teleological drift detector using array comparison.
      pub struct TeleologicalDriftDetector {
          comparator: TeleologicalComparator,
          history: DriftHistory,
          thresholds: DriftThresholds,
      }

      impl TeleologicalDriftDetector {
          pub fn new(comparator: TeleologicalComparator) -> Self;
          pub fn with_thresholds(comparator: TeleologicalComparator, thresholds: DriftThresholds) -> Self;

          /// Check drift of memories against a goal.
          pub fn check_drift(
              &self,
              memories: &[TeleologicalArray],
              goal: &TeleologicalArray,
              comparison_type: &ComparisonType,
          ) -> DriftResult;

          /// Check drift and update history for trend analysis.
          pub fn check_drift_with_history(
              &mut self,
              memories: &[TeleologicalArray],
              goal: &TeleologicalArray,
              comparison_type: &ComparisonType,
          ) -> DriftResult;

          /// Get drift trend for a goal.
          pub fn get_trend(&self, goal_id: &str) -> Option<TrendAnalysis>;

          /// Classify similarity score to drift level.
          fn classify_drift(&self, similarity: f32) -> DriftLevel;

          /// Generate recommendations based on drift analysis.
          fn generate_recommendations(&self, result: &DriftResult) -> Vec<DriftRecommendation>;
      }

      /// Configuration for drift thresholds.
      #[derive(Debug, Clone)]
      pub struct DriftThresholds {
          pub none_min: f32,
          pub low_min: f32,
          pub medium_min: f32,
          pub high_min: f32,
          // Below high_min is Critical
      }

      /// Result of drift analysis.
      #[derive(Debug)]
      pub struct DriftResult {
          pub overall_drift: OverallDrift,
          pub per_embedder_drift: PerEmbedderDrift,
          pub most_drifted_embedders: Vec<EmbedderDriftInfo>,
          pub recommendations: Vec<DriftRecommendation>,
          pub trend: Option<TrendAnalysis>,
      }

      /// Overall drift assessment.
      #[derive(Debug)]
      pub struct OverallDrift {
          pub has_drifted: bool,
          pub drift_score: f32,
          pub drift_level: DriftLevel,
      }

      /// Per-embedder drift breakdown.
      #[derive(Debug)]
      pub struct PerEmbedderDrift {
          pub embedder_drift: [(Embedder, EmbedderDriftInfo); 13],
      }

      /// Drift info for a single embedder.
      #[derive(Debug, Clone)]
      pub struct EmbedderDriftInfo {
          pub embedder: Embedder,
          pub similarity: f32,
          pub drift_level: DriftLevel,
      }

      /// Recommendation for addressing drift.
      #[derive(Debug)]
      pub struct DriftRecommendation {
          pub embedder: Embedder,
          pub issue: String,
          pub suggestion: String,
          pub priority: RecommendationPriority,
      }

      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      pub enum RecommendationPriority {
          Low,
          Medium,
          High,
          Critical,
      }

      /// Trend analysis over time.
      #[derive(Debug)]
      pub struct TrendAnalysis {
          pub direction: DriftTrend,
          pub velocity: f32,
          pub samples: usize,
          pub projected_critical_in: Option<String>,
      }

      /// History of drift measurements for trend analysis.
      #[derive(Debug, Default)]
      pub struct DriftHistory {
          entries: Vec<DriftHistoryEntry>,
          max_entries: usize,
      }

      #[derive(Debug)]
      pub struct DriftHistoryEntry {
          pub goal_id: String,
          pub timestamp: chrono::DateTime<chrono::Utc>,
          pub overall_similarity: f32,
          pub per_embedder: [f32; 13],
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Drift classification uses configurable thresholds</constraint>
    <constraint>Per-embedder breakdown always available</constraint>
    <constraint>Trend analysis requires minimum history samples</constraint>
    <constraint>Recommendations specific to embedder type</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-core autonomous::drift</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/autonomous/drift.rs

use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::teleology::array::TeleologicalArray;
use crate::teleology::comparison::ComparisonType;
use crate::teleology::comparator::TeleologicalComparator;
use crate::teleology::embedder::Embedder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DriftLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl DriftLevel {
    pub fn from_similarity(similarity: f32, thresholds: &DriftThresholds) -> Self {
        if similarity >= thresholds.none_min {
            DriftLevel::None
        } else if similarity >= thresholds.low_min {
            DriftLevel::Low
        } else if similarity >= thresholds.medium_min {
            DriftLevel::Medium
        } else if similarity >= thresholds.high_min {
            DriftLevel::High
        } else {
            DriftLevel::Critical
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftTrend {
    Improving,
    Stable,
    Worsening,
}

#[derive(Debug, Clone)]
pub struct DriftThresholds {
    pub none_min: f32,
    pub low_min: f32,
    pub medium_min: f32,
    pub high_min: f32,
}

impl Default for DriftThresholds {
    fn default() -> Self {
        Self {
            none_min: 0.85,
            low_min: 0.70,
            medium_min: 0.55,
            high_min: 0.40,
        }
    }
}

pub struct TeleologicalDriftDetector {
    comparator: TeleologicalComparator,
    history: DriftHistory,
    thresholds: DriftThresholds,
}

impl TeleologicalDriftDetector {
    pub fn new(comparator: TeleologicalComparator) -> Self {
        Self {
            comparator,
            history: DriftHistory::default(),
            thresholds: DriftThresholds::default(),
        }
    }

    pub fn with_thresholds(comparator: TeleologicalComparator, thresholds: DriftThresholds) -> Self {
        Self {
            comparator,
            history: DriftHistory::default(),
            thresholds,
        }
    }

    pub fn check_drift(
        &self,
        memories: &[TeleologicalArray],
        goal: &TeleologicalArray,
        comparison_type: &ComparisonType,
    ) -> DriftResult {
        if memories.is_empty() {
            return DriftResult::no_drift();
        }

        // Compare each memory to goal and aggregate
        let mut overall_sum = 0.0f32;
        let mut per_embedder_sums = [0.0f32; 13];
        let mut per_embedder_counts = [0usize; 13];

        for memory in memories {
            let result = self.comparator.compare(memory, goal, comparison_type);
            overall_sum += result.overall_similarity;

            for (i, score_opt) in result.per_embedder.iter().enumerate() {
                if let Some(score) = score_opt {
                    per_embedder_sums[i] += score;
                    per_embedder_counts[i] += 1;
                }
            }
        }

        let overall_similarity = overall_sum / memories.len() as f32;
        let overall_drift_level = self.classify_drift(overall_similarity);

        // Per-embedder drift
        let mut embedder_drift_infos = Vec::new();
        for embedder in Embedder::all() {
            let idx = embedder.index();
            let similarity = if per_embedder_counts[idx] > 0 {
                per_embedder_sums[idx] / per_embedder_counts[idx] as f32
            } else {
                1.0 // No data means no drift
            };

            let drift_level = self.classify_drift(similarity);
            embedder_drift_infos.push(EmbedderDriftInfo {
                embedder,
                similarity,
                drift_level,
            });
        }

        // Sort by drift level (worst first)
        let mut most_drifted = embedder_drift_infos.clone();
        most_drifted.sort_by(|a, b| b.drift_level.cmp(&a.drift_level));
        most_drifted.truncate(5);

        let overall_drift = OverallDrift {
            has_drifted: overall_drift_level > DriftLevel::None,
            drift_score: 1.0 - overall_similarity,
            drift_level: overall_drift_level,
        };

        let result = DriftResult {
            overall_drift,
            per_embedder_drift: PerEmbedderDrift {
                embedder_drift: embedder_drift_infos.try_into().unwrap(),
            },
            most_drifted_embedders: most_drifted,
            recommendations: Vec::new(),
            trend: None,
        };

        // Generate recommendations
        let mut result = result;
        result.recommendations = self.generate_recommendations(&result);
        result
    }

    pub fn check_drift_with_history(
        &mut self,
        memories: &[TeleologicalArray],
        goal: &TeleologicalArray,
        comparison_type: &ComparisonType,
    ) -> DriftResult {
        let mut result = self.check_drift(memories, goal, comparison_type);

        // Record in history
        let per_embedder: [f32; 13] = result.per_embedder_drift.embedder_drift
            .iter()
            .map(|e| e.similarity)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        self.history.add(DriftHistoryEntry {
            goal_id: goal.id.to_string(),
            timestamp: Utc::now(),
            overall_similarity: 1.0 - result.overall_drift.drift_score,
            per_embedder,
        });

        // Compute trend
        result.trend = self.get_trend(&goal.id.to_string());
        result
    }

    pub fn get_trend(&self, goal_id: &str) -> Option<TrendAnalysis> {
        let entries: Vec<_> = self.history.entries
            .iter()
            .filter(|e| e.goal_id == goal_id)
            .collect();

        if entries.len() < 3 {
            return None;
        }

        // Simple linear regression on recent entries
        let recent: Vec<_> = entries.iter().rev().take(10).collect();
        let n = recent.len() as f32;

        let sum_x: f32 = (0..recent.len()).map(|i| i as f32).sum();
        let sum_y: f32 = recent.iter().map(|e| e.overall_similarity).sum();
        let sum_xy: f32 = recent.iter().enumerate()
            .map(|(i, e)| i as f32 * e.overall_similarity)
            .sum();
        let sum_x2: f32 = (0..recent.len()).map(|i| (i * i) as f32).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);

        let direction = if slope > 0.01 {
            DriftTrend::Improving
        } else if slope < -0.01 {
            DriftTrend::Worsening
        } else {
            DriftTrend::Stable
        };

        // Project when critical if worsening
        let projected = if direction == DriftTrend::Worsening {
            let current = recent[0].overall_similarity;
            let critical_threshold = self.thresholds.high_min;
            if current > critical_threshold && slope < 0.0 {
                let days = (current - critical_threshold) / (-slope);
                Some(format!("{:.1} days at current rate", days))
            } else {
                None
            }
        } else {
            None
        };

        Some(TrendAnalysis {
            direction,
            velocity: slope.abs(),
            samples: recent.len(),
            projected_critical_in: projected,
        })
    }

    fn classify_drift(&self, similarity: f32) -> DriftLevel {
        DriftLevel::from_similarity(similarity, &self.thresholds)
    }

    fn generate_recommendations(&self, result: &DriftResult) -> Vec<DriftRecommendation> {
        let mut recommendations = Vec::new();

        for info in &result.most_drifted_embedders {
            if info.drift_level >= DriftLevel::Medium {
                let (issue, suggestion) = self.get_recommendation_text(info.embedder, info.drift_level);
                recommendations.push(DriftRecommendation {
                    embedder: info.embedder,
                    issue,
                    suggestion,
                    priority: match info.drift_level {
                        DriftLevel::Critical => RecommendationPriority::Critical,
                        DriftLevel::High => RecommendationPriority::High,
                        DriftLevel::Medium => RecommendationPriority::Medium,
                        _ => RecommendationPriority::Low,
                    },
                });
            }
        }

        recommendations
    }

    fn get_recommendation_text(&self, embedder: Embedder, level: DriftLevel) -> (String, String) {
        let severity = match level {
            DriftLevel::Critical => "Critical",
            DriftLevel::High => "Significant",
            DriftLevel::Medium => "Moderate",
            _ => "Minor",
        };

        match embedder {
            Embedder::Semantic => (
                format!("{} semantic drift detected", severity),
                "Review if work aligns with core conceptual goals".to_string(),
            ),
            Embedder::TemporalRecent => (
                format!("{} temporal drift detected", severity),
                "Recent work may have diverged from goal timeline".to_string(),
            ),
            Embedder::TemporalPeriodic => (
                format!("{} periodic pattern drift detected", severity),
                "Cyclical patterns have shifted - check if intentional".to_string(),
            ),
            Embedder::Entity => (
                format!("{} entity drift detected", severity),
                "Key entities have changed - review entity relationships".to_string(),
            ),
            Embedder::Causal => (
                format!("{} causal chain drift detected", severity),
                "Review causal dependencies in implementation".to_string(),
            ),
            Embedder::SpladeExpansion => (
                format!("{} lexical expansion drift detected", severity),
                "Terminology has diverged - align vocabulary".to_string(),
            ),
            Embedder::Code => (
                format!("{} code pattern drift detected", severity),
                "Code structure has diverged - review architecture".to_string(),
            ),
            Embedder::Graph => (
                format!("{} relationship drift detected", severity),
                "Graph structure has changed - check connections".to_string(),
            ),
            Embedder::Hdc => (
                format!("{} holographic pattern drift detected", severity),
                "Abstract patterns have shifted".to_string(),
            ),
            Embedder::Multimodal => (
                format!("{} multimodal drift detected", severity),
                "Cross-modal alignment has shifted".to_string(),
            ),
            Embedder::EntityTransE => (
                format!("{} knowledge base drift detected", severity),
                "Entity relationships have changed".to_string(),
            ),
            Embedder::LateInteraction => (
                format!("{} precision matching drift detected", severity),
                "Fine-grained matching patterns have shifted".to_string(),
            ),
            Embedder::SpladeKeyword => (
                format!("{} keyword drift detected", severity),
                "Key terms have diverged - realign terminology".to_string(),
            ),
        }
    }
}

impl DriftResult {
    fn no_drift() -> Self {
        Self {
            overall_drift: OverallDrift {
                has_drifted: false,
                drift_score: 0.0,
                drift_level: DriftLevel::None,
            },
            per_embedder_drift: PerEmbedderDrift {
                embedder_drift: std::array::from_fn(|i| EmbedderDriftInfo {
                    embedder: Embedder::from_index(i).unwrap(),
                    similarity: 1.0,
                    drift_level: DriftLevel::None,
                }),
            },
            most_drifted_embedders: Vec::new(),
            recommendations: Vec::new(),
            trend: None,
        }
    }
}

#[derive(Debug)]
pub struct DriftResult {
    pub overall_drift: OverallDrift,
    pub per_embedder_drift: PerEmbedderDrift,
    pub most_drifted_embedders: Vec<EmbedderDriftInfo>,
    pub recommendations: Vec<DriftRecommendation>,
    pub trend: Option<TrendAnalysis>,
}

#[derive(Debug)]
pub struct OverallDrift {
    pub has_drifted: bool,
    pub drift_score: f32,
    pub drift_level: DriftLevel,
}

#[derive(Debug)]
pub struct PerEmbedderDrift {
    pub embedder_drift: [EmbedderDriftInfo; 13],
}

#[derive(Debug, Clone)]
pub struct EmbedderDriftInfo {
    pub embedder: Embedder,
    pub similarity: f32,
    pub drift_level: DriftLevel,
}

#[derive(Debug)]
pub struct DriftRecommendation {
    pub embedder: Embedder,
    pub issue: String,
    pub suggestion: String,
    pub priority: RecommendationPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug)]
pub struct TrendAnalysis {
    pub direction: DriftTrend,
    pub velocity: f32,
    pub samples: usize,
    pub projected_critical_in: Option<String>,
}

#[derive(Debug, Default)]
pub struct DriftHistory {
    entries: Vec<DriftHistoryEntry>,
    max_entries: usize,
}

impl DriftHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn add(&mut self, entry: DriftHistoryEntry) {
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }
}

#[derive(Debug)]
pub struct DriftHistoryEntry {
    pub goal_id: String,
    pub timestamp: DateTime<Utc>,
    pub overall_similarity: f32,
    pub per_embedder: [f32; 13],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_level_classification() {
        let thresholds = DriftThresholds::default();
        assert_eq!(DriftLevel::from_similarity(0.90, &thresholds), DriftLevel::None);
        assert_eq!(DriftLevel::from_similarity(0.75, &thresholds), DriftLevel::Low);
        assert_eq!(DriftLevel::from_similarity(0.60, &thresholds), DriftLevel::Medium);
        assert_eq!(DriftLevel::from_similarity(0.45, &thresholds), DriftLevel::High);
        assert_eq!(DriftLevel::from_similarity(0.30, &thresholds), DriftLevel::Critical);
    }

    #[test]
    fn test_trend_detection() {
        // Test trend analysis with synthetic history
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/autonomous/drift.rs">
    Teleological drift detector implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/autonomous/mod.rs">
    Add: pub mod drift;
  </file>
  <file path="crates/context-graph-core/Cargo.toml">
    Add: chrono dependency for timestamps
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Drift levels classified correctly based on thresholds</criterion>
  <criterion>Per-embedder breakdown available for all 13 embedders</criterion>
  <criterion>Trend analysis detects improving/stable/worsening</criterion>
  <criterion>Recommendations generated for high drift embedders</criterion>
  <criterion>History tracking enables trend computation</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core autonomous::drift -- --nocapture</command>
</test_commands>
</task_spec>
```
