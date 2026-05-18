---
namespace: journal
created: 2026-05-11T00:00:00Z
updated: 2026-05-11T00:00:00Z
status: active
tags: utml, hardening, cargo-check, formula-semantics
---

# Failure: UTML Component Move After Formula Hardening

## What Happened

After replacing the simplified `Copy` UTML component structs with richer formula-aligned structs containing `Vec` and `String` fields, `cargo check -p context-graph-mejepa-train` failed in `compute_l_step`.

The immediate failure was Rust move semantics: `compute_delta_p(p)`, `compute_delta_k(k)`, and `compute_delta_omega(o)` consumed the component values before `LearningSignal` stored them as source-of-truth component evidence.

## Why It Matters

The persisted training certificate must contain the exact components used to compute the four UTML factors. Dropping those components or reconstructing them later would weaken FSV and certificate replay. The fix is to clone the components at the `compute_l_step` assembly boundary before computing factors, preserving the original values in the final `LearningSignal`.

## Related

- Source: `crates/context-graph-mejepa-train/src/learning_signal.rs`
- Command: `cargo check -p context-graph-mejepa-train`

## Follow-up Failure

`cargo test -p context-graph-mejepa-train learning_signal --lib` then failed because the old `l_step_known_values` assertion encoded the obsolete assumption that default ΔK/ΔΩ components were zero. With the hardened model, defaults are explicit neutral/healthy source values; the invariant to test is that any zero UTML factor zeros `L_step`. The test was updated to use `delta_p_real=0.0` and assert `L_step=0.0`.
