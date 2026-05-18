---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:07:00Z
status: active
tags: mejepa, phase-0, delete_test_call, dead-code, refinement
---

# delete_test_call::is_multiline_continuation has a dead `_next_chunk` argument

## Where

`crates/context-graph-mejepa-corpus/src/delete_test_call.rs:126`
(definition) and the call site at line 83.

## What

```rust
fn is_multiline_continuation(line: &str, _next_chunk: &str) -> bool { ... }
```

The `_next_chunk` parameter is never read. The call site builds a
`&source[line_with_nl_end..n.min(line_with_nl_end + 200)]` slice
specifically to pass it in:

```rust
if !is_multiline_continuation(line, &source[line_with_nl_end..n.min(line_with_nl_end + 200)]) {
```

This creates a 200-byte slice on every assertion-line iteration, then
discards it. It's a wasted slice computation (cheap, but useless) and
the call-site signature pretends behavior depends on the next chunk
when it doesn't.

## Why it matters

Both `next_chunk` allocation AND the parameter declaration are dead
weight. The function name "is_multiline_continuation" implies
look-ahead behavior, so a future reader will naturally wonder why
`_next_chunk` is unused — wasting their time.

## Recommended refinement

Drop the parameter and the slice:

```rust
fn is_multiline_continuation(line: &str) -> bool { ... }
// ...
if !is_multiline_continuation(line) {
```

If a Phase-1 enhancement needs to look at the next line (e.g. to count
matching close-parens across line boundaries), reintroduce the
parameter THEN with a clear caller-side construction. YAGNI applies.

## Severity

REFINEMENT-RECOMMENDED. Pure dead-code cleanup.
