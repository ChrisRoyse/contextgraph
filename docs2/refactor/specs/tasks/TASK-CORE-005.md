# TASK-CORE-005: Update GoalNode Structure

```xml
<task_spec id="TASK-CORE-005" version="1.0">
<metadata>
  <title>Update GoalNode to Use TeleologicalArray</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>5</sequence>
  <implements>
    <requirement_ref>REQ-GOAL-REFACTOR-01</requirement_ref>
    <requirement_ref>REQ-NORTHSTAR-REMOVE-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-001</task_ref>
    <task_ref>TASK-CORE-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>1</estimated_days>
</metadata>

<context>
The existing GoalNode uses a single Vec<f32> embedding field, which cannot be
meaningfully compared to 13-embedder teleological arrays. This task updates
GoalNode to use TeleologicalArray, enabling apples-to-apples comparison.
Depends on TASK-CORE-001 (protocol constants removed) and TASK-CORE-003 (TeleologicalArray defined).
</context>

<objective>
Refactor GoalNode to replace the single embedding field with a TeleologicalArray,
deprecate the north_star() constructor, and add autonomous_goal() constructor.
</objective>

<rationale>
Goals must be represented as teleological arrays because:
1. Memories are teleological arrays - comparison must be apples-to-apples
2. Goals emerge from clustering arrays - centroids are naturally arrays
3. Per-embedder goal tracking enables richer drift detection
4. Autonomous discovery produces array-based goals

The old single-embedding approach is fundamentally broken and must be removed.
</rationale>

<input_context_files>
  <file purpose="goal_struct">crates/context-graph-core/src/purpose.rs</file>
  <file purpose="teleological_array">crates/context-graph-core/src/teleology/array.rs</file>
  <file purpose="removal_rationale">docs2/refactor/05-NORTH-STAR-REMOVAL.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-001 complete (protocol constants removed)</check>
  <check>TASK-CORE-003 complete (TeleologicalArray defined)</check>
  <check>GoalNode struct exists in purpose.rs</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Remove embedding: Vec<f32> field from GoalNode</item>
    <item>Add teleological_array: TeleologicalArray field</item>
    <item>Update GoalNode constructors</item>
    <item>Deprecate north_star() constructor</item>
    <item>Add autonomous_goal() constructor</item>
    <item>Update serialization/deserialization</item>
    <item>Update GoalLevel enum if needed</item>
  </in_scope>
  <out_of_scope>
    <item>Goal discovery pipeline (TASK-LOGIC-009)</item>
    <item>Drift detection (TASK-LOGIC-010)</item>
    <item>MCP handlers (TASK-INTEG-004)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/purpose.rs">
      use crate::teleology::array::TeleologicalArray;
      use uuid::Uuid;
      use chrono::{DateTime, Utc};

      /// Hierarchical goal level in the autonomous purpose system.
      #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
      pub enum GoalLevel {
          /// Top-level strategic goal (discovered, not manual)
          NorthStar,
          /// High-level objectives
          Strategic,
          /// Mid-level tactical goals
          Tactical,
          /// Immediate actionable tasks
          Immediate,
      }

      /// A goal node in the purpose hierarchy.
      /// Goals are discovered autonomously from memory patterns.
      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct GoalNode {
          /// Unique identifier
          pub id: Uuid,
          /// Human-readable description
          pub description: String,
          /// Hierarchical level
          pub level: GoalLevel,
          /// The teleological array representing this goal
          pub teleological_array: TeleologicalArray,
          /// Parent goal (None for NorthStar)
          pub parent_id: Option<Uuid>,
          /// Child goal IDs
          pub child_ids: Vec<Uuid>,
          /// Discovery metadata
          pub discovery: GoalDiscoveryMetadata,
          /// Creation timestamp
          pub created_at: DateTime<Utc>,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct GoalDiscoveryMetadata {
          /// How this goal was discovered
          pub method: DiscoveryMethod,
          /// Confidence score (0.0 - 1.0)
          pub confidence: f32,
          /// Number of memories in cluster
          pub cluster_size: usize,
          /// Coherence score of cluster
          pub coherence: f32,
      }

      #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
      pub enum DiscoveryMethod {
          /// Discovered via clustering
          Clustering,
          /// Discovered via pattern recognition
          PatternRecognition,
          /// Inherited from parent decomposition
          Decomposition,
      }

      impl GoalNode {
          /// Create a new autonomously discovered goal.
          pub fn autonomous_goal(
              description: String,
              level: GoalLevel,
              teleological_array: TeleologicalArray,
              discovery: GoalDiscoveryMetadata,
          ) -> Self;

          /// Check if this goal has the given ancestor.
          pub fn has_ancestor(&self, ancestor_id: Uuid) -> bool;

          /// Get the teleological array for comparison.
          pub fn array(&self) -> &TeleologicalArray;
      }

      // REMOVED: pub fn north_star(...) - deprecated, do not use
    </signature>
  </signatures>

  <constraints>
    <constraint>No single Vec<f32> embedding field exists</constraint>
    <constraint>All goals must have TeleologicalArray</constraint>
    <constraint>north_star() constructor must not exist</constraint>
    <constraint>Serialization backward-compatible where possible</constraint>
    <constraint>GoalLevel::NorthStar now means "autonomously discovered top goal"</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-core</command>
    <command>rg "embedding: Vec" crates/context-graph-core/src/purpose.rs</command>
    <command>rg "fn north_star" crates/context-graph-core/src/purpose.rs</command>
    <command>cargo test -p context-graph-core purpose</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/purpose.rs

use crate::teleology::array::TeleologicalArray;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalLevel {
    NorthStar,    // Top discovered goal
    Strategic,    // High-level objectives
    Tactical,     // Mid-level goals
    Immediate,    // Actionable tasks
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DiscoveryMethod {
    Clustering,
    PatternRecognition,
    Decomposition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalDiscoveryMetadata {
    pub method: DiscoveryMethod,
    pub confidence: f32,
    pub cluster_size: usize,
    pub coherence: f32,
}

impl Default for GoalDiscoveryMetadata {
    fn default() -> Self {
        Self {
            method: DiscoveryMethod::Clustering,
            confidence: 0.0,
            cluster_size: 0,
            coherence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalNode {
    pub id: Uuid,
    pub description: String,
    pub level: GoalLevel,
    pub teleological_array: TeleologicalArray,
    pub parent_id: Option<Uuid>,
    pub child_ids: Vec<Uuid>,
    pub discovery: GoalDiscoveryMetadata,
    pub created_at: DateTime<Utc>,
}

impl GoalNode {
    /// Create a new autonomously discovered goal.
    pub fn autonomous_goal(
        description: String,
        level: GoalLevel,
        teleological_array: TeleologicalArray,
        discovery: GoalDiscoveryMetadata,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            description,
            level,
            teleological_array,
            parent_id: None,
            child_ids: Vec::new(),
            discovery,
            created_at: Utc::now(),
        }
    }

    pub fn array(&self) -> &TeleologicalArray {
        &self.teleological_array
    }

    // No north_star() constructor - goals are discovered, not manually created
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autonomous_goal_creation() {
        let array = TeleologicalArray::new(Uuid::new_v4());
        let discovery = GoalDiscoveryMetadata {
            method: DiscoveryMethod::Clustering,
            confidence: 0.85,
            cluster_size: 42,
            coherence: 0.78,
        };

        let goal = GoalNode::autonomous_goal(
            "Test goal".to_string(),
            GoalLevel::Strategic,
            array,
            discovery,
        );

        assert_eq!(goal.level, GoalLevel::Strategic);
        assert_eq!(goal.discovery.confidence, 0.85);
    }
}
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-core/src/purpose.rs">
    Complete rewrite of GoalNode to use TeleologicalArray
  </file>
</files_to_modify>

<files_to_create>
  <!-- None - modifying existing file -->
</files_to_create>

<validation_criteria>
  <criterion>No "embedding: Vec" in GoalNode</criterion>
  <criterion>No "north_star" function exists</criterion>
  <criterion>GoalNode has teleological_array field</criterion>
  <criterion>autonomous_goal() constructor works</criterion>
  <criterion>GoalDiscoveryMetadata captures discovery context</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core purpose -- --nocapture</command>
  <command>rg "embedding: Vec" crates/context-graph-core/src/purpose.rs</command>
</test_commands>
</task_spec>
```
