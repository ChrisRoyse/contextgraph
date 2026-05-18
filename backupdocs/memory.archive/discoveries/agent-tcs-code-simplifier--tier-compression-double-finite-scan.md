---
namespace: discoveries
agent_id: agent-tcs-code-simplifier
timestamp: 2026-05-08T00:00:00Z
status: active
tags: tier-compression, encode, micro-optimization, readability
---

# Encode walks `values` twice (is_finite check, then min/max scan)

`crates/context-graph-tier-compression/src/lib.rs:182-200` does two sequential
linear passes:

1. `for (i, v) in values.iter().enumerate() { if !v.is_finite() { reject } }`
2. `for v in values { if *v < min { ... } if *v > max { ... } }`

Both can fold into one pass:

```rust
let mut min = f32::INFINITY;
let mut max = f32::NEG_INFINITY;
for (i, v) in values.iter().enumerate() {
    if !v.is_finite() {
        return Err(TierCompressionError::invalid(
            "values",
            format!("values[{i}] is non-finite: {v}"),
            "scrub NaN/Inf before encoding",
        ));
    }
    if *v < min { min = *v; }
    if *v > max { max = *v; }
}
```

Halves the cache traffic on the input slice and the resulting code is shorter,
not denser. Severity: LOW (micro). Optional refinement.
