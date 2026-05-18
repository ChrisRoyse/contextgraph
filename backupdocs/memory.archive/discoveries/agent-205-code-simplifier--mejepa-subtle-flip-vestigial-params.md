---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:00:00Z
status: active
tags: mejepa, phase-0, subtle_flip, vestigial-code, refinement
---

# subtle_flip::match_keyword has 4 vestigial parameters

## Where

`crates/context-graph-mejepa-corpus/src/subtle_flip.rs:252-276` (the
`match_keyword` definition) and call sites at lines 195, 206, 217, 228.

## What

`match_keyword` takes six parameters but uses only three (`bytes`, `i`,
`keyword`). The other four — `_ctx_with_spaces`, `_ctx_in_parens`,
`_ctx_at_eol`, `_note` — are leading-underscore ignored. The doc-comment
already admits "extra `_*` parameters are ignored (legacy from earlier
API)".

The four call sites pass `" and "`, `"( and "`, etc. as string literals
that are never consulted. Each call site is six positional arguments
where two of them are duplicates (`" and "` appears twice in the `and`
call), which makes the call signature actively misleading — a reader
naturally assumes those strings drive matching behavior.

## Why it matters

1. Misleading API: the `note` param at the call site is duplicated by the
   `note:` field on the `Candidate` struct three lines later (see line
   201's `note: "subtle_flip: and -> or"` — exactly the same string the
   sixth arg supplies).
2. Suppressed via `#[allow(clippy::too_many_arguments)]` and
   leading-underscore — both are admissions of vestigial state.
3. LOC cost: 4 useless params × 4 call sites × 2 LOC each ≈ ~32 lines of
   noise.

## Recommended refinement

Drop `match_keyword` to three params:

```rust
fn match_keyword(bytes: &[u8], i: usize, keyword: &[u8]) -> Option<usize>
```

Return just the byte offset (length is `keyword.len()`, the caller
already knows it). Drop `#[allow(clippy::too_many_arguments)]`. Each
call site shrinks to:

```rust
if let Some(off) = match_keyword(bytes, i, b"and") {
    candidates.push(Candidate { byte_offset: off, byte_length: 3,
        original: "and", replacement: "or",
        note: "subtle_flip: and -> or" });
    i = off + 3;
    continue;
}
```

That eats the `(offset, length)` tuple destructuring noise too.

## Severity

REFINEMENT-RECOMMENDED. Not a bug; pure readability.
