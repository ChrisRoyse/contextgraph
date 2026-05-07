---
namespace: discoveries
agent_id: agent-02-coder-core
timestamp: 2026-04-15T04:30:00Z
status: active
tags: line-count, tests, training, file-budget
---

# Discovery: training/tests.rs at 518 lines (was 443)

## What
After adding the two required new tests (`edge_type_distribution_sum_matches_outgoing_edges`
and `edge_type_distribution_bincode_roundtrip`) plus the field addition to
the existing sample_record fixture, `training/tests.rs` is now 518 lines.
The 500-line file budget is exceeded by 18 lines.

## Why I did not split

1. The spec explicitly directs to add the new tests to the existing
   `training/tests.rs`, not a new file.
2. All test fixtures (sample_record, empty_semantic_fingerprint, etc.) are
   already in this file and tightly coupled to the test bodies; splitting
   would require duplicating fixtures or extracting them to a module.
3. The added tests (~75 lines) are tightly thematic ("v2 distribution
   field") and live alongside the bincode-roundtrip section.

## Suggested follow-up

Phase 5 / 6 agents may want to split tests.rs into:
- `tests/cross_correlations.rs`
- `tests/group_alignments.rs`
- `tests/topic_profile.rs`
- `tests/temporal.rs`
- `tests/v2_signature.rs`
- shared `tests/fixtures.rs`

But that is a refactor beyond Phase 2's scope.
