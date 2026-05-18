# Session Report: Phase G Targeted Mutation Expansion

Date: 2026-05-13
Issue: #99 / TASK-G-003
Commit: 1f2525c7 feat(corpus): expand phase g targeted mutations

## Summary

Implemented targeted training-data expansion for the Phase G weak Python cells:

- off_by_one
- subtle_flip
- swap_variable

The corpus generators now produce broader Python mutation coverage for expression boundaries, membership and identity flips, any/all flips, function parameters, tuple assignments, loop locals, zip arguments, and f-string replacements.

## Files Changed

- crates/context-graph-mejepa-corpus/src/off_by_one.rs
- crates/context-graph-mejepa-corpus/src/subtle_flip.rs
- crates/context-graph-mejepa-corpus/src/swap_variable.rs
- crates/context-graph-mejepa-corpus/src/tests.rs
- crates/context-graph-mejepa-corpus/examples/phase_g_targeted_mutation_expansion_fsv.rs
- scripts/ci/phase_g_targeted_mutation_expansion_fsv.sh
- futurebuild/context_graph_futurebuild/00_INDEX.md
- futurebuild/context_graph_futurebuild/02_CURRENT_STATE_HONEST.md
- futurebuild/context_graph_futurebuild/12_IMPLEMENTATION_PHASES.md
- futurebuild/context_graph_futurebuild/13_TASK_INDEX.md

## Verification

- cargo test -p context-graph-mejepa-corpus --lib
- cargo check -p context-graph-mejepa-corpus --examples
- cargo clippy -p context-graph-mejepa-corpus --all-targets -- -D warnings
- scripts/ci/phase_g_targeted_mutation_expansion_fsv.sh

## FSV Evidence

Artifact:

- /mnt/d/contextgraph/fsv/phase-g-targeted-mutation-expansion-fsv/targeted_mutation_expansion_fsv.json

Post-commit FSV:

- all_passed: true
- build_release_sha: 1f2525c7
- artifact_sha256: a1a533a13ea23e258c9715450b21171f0c14d0b7b85bd2c3e65b5bd6214be966
- rows: 1800
- category_counts:
  - off_by_one: 600
  - subtle_flip: 600
  - swap_variable: 600
- pattern_count: 15
- pytest: 3600 passed in 95.81s

Assertions:

- pattern_diversity: true
- py_compile_success: true
- py_compile_validated_every_generated_module: true
- pytest_exercised_original_and_mutated_rows: true
- pytest_success: true
- rows_per_category_600: true
- target_gap_reduction_fixture: true
- total_rows_1800: true

Note: the target gap reduction evidence is a synthetic correlation remeasurement fixture in the FSV harness. Full rolling holdout model convergence remains tracked by the broader Phase G evaluation workflow.
