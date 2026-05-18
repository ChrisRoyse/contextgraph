---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:01:00Z
status: active
tags: mejepa, phase-0, off_by_one, redundant-validation, refinement
---

# off_by_one::find_candidates has overlapping reject conditions

## Where

`crates/context-graph-mejepa-corpus/src/off_by_one.rs:127-131`.

## What

```rust
let is_float_or_complex = matches!(next_byte,
    Some(b'.') | Some(b'e') | Some(b'E') | Some(b'j') | Some(b'_'));
let next_is_word = next_byte.map(is_word_byte).unwrap_or(false);
if prev_ok && !is_float_or_complex && !next_is_word { ... }
```

`is_word_byte(b) == b.is_ascii_alphanumeric() || b == b'_'`. So
`next_is_word` already covers `e`, `E`, `j`, and `_`. The only byte in
`is_float_or_complex` that isn't already in `next_is_word` is `b'.'`
(period). Result: `is_float_or_complex` is fully subsumed by
`next_is_word` plus the dot check.

## Why it matters

Two parallel reject reasons doing 80% of the same work invites future
edits to update one but not the other. Also: `is_float_or_complex`'s name
is misleading — Python's complex literal is `5j`, but `j` already
disqualifies via word-byte check, so the variable is doing nothing
useful for the complex case it claims to cover.

The fact that two of the three reject reasons (the `.` check on
`prev_byte`, the `is_float_or_complex` on `next_byte`) overlap on dot
detection is also worth noting — they're both guarding the same float
shape from opposite sides, which is the right structure but obscured
by the redundant `is_float_or_complex` flag.

## Recommended refinement

Collapse to two clear conditions:

```rust
let next_byte = bytes.get(j).copied();
let next_is_word_or_dot = next_byte
    .map(|b| is_word_byte(b) || b == b'.')
    .unwrap_or(false);
if prev_ok && !next_is_word_or_dot {
    // candidate
}
```

This says exactly what we mean: "rejected if the next byte continues an
identifier OR makes this the integer part of a float". `prev_ok` already
guards the symmetric "fractional part of a float" via the `b != b'.'`
clause at line 117.

## Severity

REFINEMENT-RECOMMENDED. Both forms produce identical behavior on every
test in `tests.rs`; this is purely simplification.
