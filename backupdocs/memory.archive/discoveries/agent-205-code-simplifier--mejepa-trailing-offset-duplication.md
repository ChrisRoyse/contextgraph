---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:04:00Z
status: active
tags: mejepa, phase-0, dry, helpers, refinement
---

# `trailing_newline_offset` / `trailing_offset` duplicated across two operators

## Where

- `crates/context-graph-mejepa-corpus/src/over_engineer.rs:40-46`
  (`fn trailing_newline_offset(source: &str) -> usize`)
- `crates/context-graph-mejepa-corpus/src/compile_error.rs:45-51`
  (`fn trailing_offset(source: &str) -> usize`)

## What

Both are identical:

```rust
if source.ends_with('\n') {
    source.len() - 1
} else {
    source.len()
}
```

Different names, same behavior. Both are private to their module so
clippy doesn't flag them.

## Why it matters

If we ever need to widen the trailing-handling rule (e.g. handle CRLF
line endings, or skip multiple trailing newlines), this rule must be
kept in two places in lockstep. Drift risk is small but measurable.

## Recommended refinement

Two equivalent options:

1. **Hoist to `prng.rs`'s sibling utility module**: add a
   `crates/context-graph-mejepa-corpus/src/util.rs` with
   `pub(crate) fn append_offset(source: &str) -> usize`, declare
   `mod util;` in `lib.rs`, and use `use crate::util::append_offset` in
   both operators.

2. **Inline at the call sites**: the function bodies are 4 lines each;
   inline + a comment is also defensible since both files have only one
   call.

I lean toward (1): the third Phase-0 operator with an "append at EOF"
shape is plausible (any "add a logging statement" operator would want
this), so a single helper is cheaper than three duplications.

## Severity

REFINEMENT-RECOMMENDED. Pure DRY refactor.
