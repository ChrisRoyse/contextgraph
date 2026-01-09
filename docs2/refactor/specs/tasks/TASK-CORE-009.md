# TASK-CORE-009: Remove Projection Code

```xml
<task_spec id="TASK-CORE-009" version="1.0">
<metadata>
  <title>Remove Broken Projection Code from Alignment Calculator</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>9</sequence>
  <implements>
    <requirement_ref>REQ-NORTHSTAR-REMOVE-03</requirement_ref>
    <requirement_ref>REQ-APPLES-TO-APPLES-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_days>1</estimated_days>
</metadata>

<context>
The alignment calculator contains projection code that attempts to compare single
embeddings to multi-embedder arrays by projecting to a common dimension. This is
fundamentally broken (apples-to-oranges comparison). This task removes all
projection logic.
</context>

<objective>
Remove the project_embedding function and all dimension reduction logic from
the alignment calculator, replacing with a placeholder for teleological array
comparison.
</objective>

<rationale>
Projection is broken because:
1. 13-embedder array has ~8,000+ dimensions total vs 1024D single embedding
2. Dimension reduction loses semantic information
3. Different embedders capture orthogonal concepts
4. Resulting comparison is mathematically meaningless

The correct approach (implemented in TASK-LOGIC-004) compares array-to-array
using per-embedder similarity functions.
</rationale>

<input_context_files>
  <file purpose="alignment_calculator">crates/context-graph-core/src/alignment/calculator.rs</file>
  <file purpose="removal_rationale">docs2/refactor/05-NORTH-STAR-REMOVAL.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-001 complete (protocol constants removed)</check>
  <check>Alignment calculator file exists</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Remove project_embedding function</item>
    <item>Remove all uses of projection in alignment calculation</item>
    <item>Add placeholder for teleological array comparison</item>
    <item>Remove dimension reduction logic</item>
    <item>Update or remove affected tests</item>
  </in_scope>
  <out_of_scope>
    <item>New TeleologicalComparator implementation (TASK-LOGIC-004)</item>
    <item>GoalAlignmentCalculator trait refactor (TASK-LOGIC-010)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/alignment/calculator.rs">
      // These MUST NOT exist:
      // fn project_embedding(...)
      // fn reduce_dimensions(...)
      // fn align_via_projection(...)

      // Placeholder for new implementation:
      /// Calculate alignment between memory and goal.
      /// NOTE: Currently stubbed - will be implemented using TeleologicalComparator.
      /// See TASK-LOGIC-004 for full implementation.
      pub fn calculate_alignment(
          memory_array: &TeleologicalArray,
          goal_array: &TeleologicalArray,
      ) -> AlignmentResult {
          todo!("Implement via TeleologicalComparator in TASK-LOGIC-004")
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>No projection functions in codebase</constraint>
    <constraint>No dimension reduction in alignment</constraint>
    <constraint>Placeholder clearly marked as TODO</constraint>
    <constraint>Code compiles (with placeholder panics)</constraint>
  </constraints>

  <verification>
    <command>rg "project_embedding" --type rust</command>
    <command>rg "reduce_dimensions" --type rust</command>
    <command>rg "dimension.*reduction" --type rust -i</command>
    <command>cargo check -p context-graph-core</command>
  </verification>
</definition_of_done>

<pseudo_code>
// In crates/context-graph-core/src/alignment/calculator.rs

use crate::teleology::array::TeleologicalArray;

/// Result of alignment calculation.
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    pub overall_score: f32,
    pub per_embedder_scores: [Option<f32>; 13],
    pub interpretation: AlignmentInterpretation,
}

#[derive(Debug, Clone, Copy)]
pub enum AlignmentInterpretation {
    HighlyAligned,
    Aligned,
    Neutral,
    Drifted,
    Misaligned,
}

/// Calculate alignment between memory and goal teleological arrays.
///
/// # Panics
/// Currently unimplemented. Will be implemented in TASK-LOGIC-004.
pub fn calculate_alignment(
    _memory_array: &TeleologicalArray,
    _goal_array: &TeleologicalArray,
) -> AlignmentResult {
    // TODO: Implement via TeleologicalComparator in TASK-LOGIC-004
    // This requires:
    // 1. TeleologicalComparator from TASK-LOGIC-004
    // 2. ComparisonType selection logic
    // 3. Per-embedder score aggregation
    todo!("Alignment calculation awaits TeleologicalComparator implementation")
}

// REMOVED CODE (do not include):
// - fn project_embedding(embedding: &[f32], target_dims: usize) -> Vec<f32>
// - fn reduce_dimensions(...)
// - fn cosine_similarity_projected(...)
// - Any PCA, SVD, or other dimensionality reduction

#[cfg(test)]
mod tests {
    // Remove all tests that relied on projection
    // Add placeholder test for new implementation

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn test_alignment_not_yet_implemented() {
        use super::*;
        use uuid::Uuid;

        let memory = TeleologicalArray::new(Uuid::new_v4());
        let goal = TeleologicalArray::new(Uuid::new_v4());
        let _ = calculate_alignment(&memory, &goal);
    }
}
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-core/src/alignment/calculator.rs">
    Remove projection code, add placeholder for array comparison
  </file>
  <file path="crates/context-graph-core/src/alignment/mod.rs">
    Update exports if needed
  </file>
</files_to_modify>

<files_to_create>
  <!-- None -->
</files_to_create>

<validation_criteria>
  <criterion>rg "project_embedding" returns no results</criterion>
  <criterion>rg "reduce_dimensions" returns no results</criterion>
  <criterion>No dimensionality reduction in codebase</criterion>
  <criterion>Placeholder function exists with TODO</criterion>
  <criterion>cargo check passes</criterion>
</validation_criteria>

<test_commands>
  <command>rg "project_embedding" --type rust</command>
  <command>rg "dimension.*reduction" --type rust -i</command>
  <command>cargo check -p context-graph-core</command>
</test_commands>
</task_spec>
```
