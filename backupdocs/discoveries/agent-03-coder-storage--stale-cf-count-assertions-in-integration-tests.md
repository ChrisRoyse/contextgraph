---
namespace: discoveries
agent_id: agent-03-coder-storage
timestamp: 2026-04-15T05:10:00Z
status: fixed-inline
severity: medium
tags: storage, tests, cf-count, column-families, regression-hazard
---

# Stale CF-count assertions in four integration-test sites

## Context
Bumping `TELEOLOGICAL_CF_COUNT` 26 → 28 and `TOTAL_COLUMN_FAMILIES` 57 → 59
broke four existing integration-test assertions that the Phase-2 author
missed because they only ran `cargo check -p context-graph-core`, not the
full workspace test suite.

## Affected sites (all fixed)
1. `crates/context-graph-storage/src/teleological/tests/column_family.rs`
   - `test_teleological_cf_names_count`: literal `26` → `28`.
   - `test_get_all_teleological_cf_descriptors_returns_39` (name + body):
     literal `39` → `41` (28 teleo + 13 quantized).
2. `crates/context-graph-storage/src/column_families.rs`
   - `test_total_column_families_constant`: docstring + literal `57` →
     `59`.
3. `crates/context-graph-storage/tests/teleological_integration.rs`
   - `test_rocksdb_open_with_26_teleological_column_families` → 28 variant;
     literals `37` → `39` throughout.
   - `test_total_column_families_is_26_teleological` → 28 variant; literals
     `26`/`37` → `28`/`39`.

## Pattern for future agents
Every time `TELEOLOGICAL_CF_COUNT` or `TOTAL_COLUMN_FAMILIES` changes:

1. Grep the workspace for the OLD literal values (`TELEOLOGICAL_CF_COUNT`
   via `Grep`, and all literal instances of the old number in tests).
2. Run `env CUDA_HOME=/usr/local/cuda cargo test -p context-graph-storage`
   (NOT `cargo check`) to surface assertion-level drift that compiles fine
   but panics at runtime.

Per-crate `cargo check` will NOT catch this; `cargo test --all-targets` at
the workspace level does, but most reliably by actually running the tests.

## Lesson
Tests that hard-code "magic numbers" for CF counts are brittle. Prefer
`assert_eq!(cfs.len(), TELEOLOGICAL_CF_COUNT)` over `assert_eq!(cfs.len(),
26)`. I left the assertions that use the constant alone and fixed only the
genuine literal-number assertions.
