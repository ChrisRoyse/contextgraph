---
namespace: discoveries
created: 2026-05-09T17:42:44Z
updated: 2026-05-09T17:42:44Z
status: active
severity: medium
tags: phase0, corpus, swebench, batch-oracle, validation
---

# Phase 0 Batch Oracle Control-Byte Validation

## What Happened
Pre-production review of the new SWE-bench batch oracle found a validation
blindspot: `validate_batch_config` checked every prediction's instance id,
model name, duplicate id, and empty patch state, but the model-patch control
character check was inherited from `validate_config` on only the first
prediction. The original single-prediction predicate also missed ASCII control
bytes because it only entered the check when the patch was not ASCII.

## Why It Matters
The corpus generator writes predictions JSONL for the official SWE-bench
harness. Every row must be validated before Docker execution, otherwise a bad
patch row deep in a 300-instance category batch could fail late in the harness
instead of failing closed at the Rust boundary with a precise error.

## Resolution
`crates/context-graph-mejepa-corpus/src/swebench.rs` now has a shared
`validate_model_patch_text` helper used by both single and batch oracle paths.
It rejects any control character except `\n`, `\r`, and `\t`, while still
allowing printable Unicode source text.

Regression coverage:
- `validate_config_rejects_ascii_control_bytes_in_patch`
- `validate_batch_config_checks_every_patch_for_control_bytes`

Verification: `cargo test -p context-graph-mejepa-corpus` passed 97 tests and
focused clippy passed after the patch.

## Related
- `crates/context-graph-mejepa-corpus/src/swebench.rs`
- `memory/journal/2026-05-09--failure-swebench-parallel-docker-race.md`
