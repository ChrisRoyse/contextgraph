---
namespace: discoveries
agent_id: agent-08-code-simplifier
timestamp: 2026-04-14T18:25:00Z
status: active
tags: contrastive, pre-filter, numerical, correctness
---

# high_low_bounds returns `(NEG_INFINITY, INFINITY)` which survives the finite-check

## Classification
**NIT** — not a defect (the downstream check catches it), but the
intermediate state is surprising.

## Severity
**Low** — correctness is preserved; the comment could be clearer.

## Location
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:682-698`
  — helper returns sentinel infinities when no embedder clears either
  threshold.
- `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs:292-295`
  — uses the result as `disagreement = max_high - min_low` and guards with
  `if !disagreement.is_finite() || disagreement <= 0.0 { continue; }`.

## Observation

When `high_low_bounds` finds no high or low embedder, `max_high` stays
`f32::NEG_INFINITY` and/or `min_low` stays `f32::INFINITY`. The subtraction
`NEG_INFINITY - INFINITY = NEG_INFINITY` (IEEE-754), which is non-finite,
so `!disagreement.is_finite()` correctly skips the pair.

This works, but the code relies on subtle float semantics and is not
obvious to a reader. A clearer form:

```rust
fn high_low_bounds(
    profile: &[f32; NUM_EMBEDDERS],
    high: f32,
    low: f32,
) -> Option<(f32, f32)> {
    let mut max_high: Option<f32> = None;
    let mut min_low: Option<f32> = None;
    for &v in profile.iter() {
        if v > high { max_high = Some(max_high.map_or(v, |m| m.max(v))); }
        if v < low  { min_low  = Some(min_low.map_or(v, |m| m.min(v))); }
    }
    match (max_high, min_low) {
        (Some(h), Some(l)) => Some((h, l)),
        _ => None,
    }
}
```

Then the call site at line 287-297 becomes:
```rust
let (max_high, min_low) = match high_low_bounds(&profile, cfg.high_threshold, cfg.low_threshold) {
    Some(bounds) => bounds,
    None => continue,  // no high/low signal — skip
};
let disagreement = max_high - min_low;
if disagreement <= 0.0 { continue; }
scored.push((neg_idx, disagreement, profile));
```

## Recommendation

Apply this cleanup together with the contrastive-prefilter fix (see
separate finding). Makes the control-flow legible without relying on
IEEE-754 sentinel arithmetic.

## Rationale

"Correct but relies on how the compiler handles infinities" is a typical
low-priority hazard — not broken today, but easy to break by adding a
third path later that subtracts the sentinels again without the finite
check.
