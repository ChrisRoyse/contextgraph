---
namespace: reports
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:08:00Z
status: active
tags: mejepa, phase-0, code-simplifier, review, final-report
---

# Phase-0 ME-JEPA mutation corpus review — final report

## Scope

Read-only review of the 12 source files in
`crates/context-graph-mejepa-corpus/` per the handoff at
`memory/handoff/agent-204-coordinator-to-code-simplifier--mejepa-corpus-review.md`.

Verification status (independent of this review): 29 tests green; FSV
evidence at `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
shows all 8 categories pass + 5 boundary cases reject with the right
codes.

## Verdict

**PASS overall.** The crate is well-structured: clear module boundaries
(one operator per file), clean public API, fail-closed error taxonomy
with three stable codes, deterministic seeding via SplitMix64, and
serde round-trip support. Two real bugs caught during dev have explicit
regression tests. No BLOCKERs.

## Counts

- PASS: 1 (compile_error append-only strategy)
- REFINEMENT-RECOMMENDED: 7
- BLOCKER: 0

## Discoveries written

1. `memory/discoveries/agent-205-code-simplifier--mejepa-subtle-flip-vestigial-params.md`
   — `match_keyword` has 4 unused params + duplicate `note` literals at
   call sites.
2. `memory/discoveries/agent-205-code-simplifier--mejepa-off-by-one-redundant-checks.md`
   — `is_float_or_complex` is fully subsumed by `is_word_byte` except
   for `b'.'`.
3. `memory/discoveries/agent-205-code-simplifier--mejepa-swap-fstring-blindspot.md`
   — variable used ONLY inside f-strings is invisible to the state
   machine. Recommend deferral with a documenting test, not a fix.
4. `memory/discoveries/agent-205-code-simplifier--mejepa-overengineer-suffix-width.md`
   — 24-bit hex tag has measurable birthday-collision risk past Phase
   1 corpus scale; widen to 48 bits.
5. `memory/discoveries/agent-205-code-simplifier--mejepa-trailing-offset-duplication.md`
   — `trailing_newline_offset` / `trailing_offset` duplicated between
   over_engineer and compile_error.
6. `memory/discoveries/agent-205-code-simplifier--mejepa-rng-pick-error-boilerplate.md`
   — `rng.pick_index(...).ok_or_else(... op_failed("rng", ...))`
   repeated 5x verbatim.
7. `memory/discoveries/agent-205-code-simplifier--mejepa-compile-error-strategy-pass.md`
   — defends append-only as the right Phase 0 strategy. PASS.
8. `memory/discoveries/agent-205-code-simplifier--mejepa-delete-test-call-dead-arg.md`
   — `is_multiline_continuation`'s `_next_chunk` is dead; the call
   site builds a 200-byte slice for nothing.

## Top-3 actionable refinements

### 1. `subtle_flip.rs:252` — drop `match_keyword`'s 4 vestigial params

The function takes 6 params, uses 3. The other 4 are leading-underscore
ignored. The `_note` parameter passes a string literal that's already
duplicated by the `note:` field on the resulting `Candidate` three lines
later (see `:201`). `#[allow(clippy::too_many_arguments)]` and the
"legacy from earlier API" doc-comment are both admissions.

**Concrete fix:** Reduce signature to
`fn match_keyword(bytes: &[u8], i: usize, keyword: &[u8]) -> Option<usize>`
returning just the offset; caller knows `keyword.len()`. Each call site
shrinks by ~3 LOC. Net: ~32 LOC removed, no behavior change. Drop the
`#[allow]`.

**Rationale:** Misleading public-shape API where unused params imply
behavior they don't have. The alternative (actually using the
`_ctx_*` strings) is undefined — the state machine already does the
word-boundary check, so the strings would be redundant.

### 2. `off_by_one.rs:127-131` — collapse two parallel reject reasons

```rust
let next_is_word_or_dot = next_byte
    .map(|b| is_word_byte(b) || b == b'.')
    .unwrap_or(false);
if prev_ok && !next_is_word_or_dot { /* candidate */ }
```

drops `is_float_or_complex` entirely. The current code's
`Some(b'e') | Some(b'E') | Some(b'j') | Some(b'_')` set is fully
covered by `is_word_byte`; only `Some(b'.')` is unique, and it's the
exact float case the comment says we're guarding against. The
regression tests
(`off_by_one_skips_fractional_part_of_floats`,
`off_by_one_rejects_source_with_only_floats`,
`off_by_one_rejects_digits_inside_identifiers`) all keep passing under
the simpler form because the simpler form is logically equivalent.

**Rationale:** Two parallel checks doing 80% of the same work invite
drift on future edits.

### 3. `over_engineer.rs:20` — widen helper-id from 24 → 48 bits

```rust
let suffix_id = rng.next_u64() & 0xFFFF_FFFF_FFFF;
let helper = format!("...def _unused_helper_{suffix_id:012x}() ...");
```

24 bits hits 50% birthday-collision at ~4,820 helpers; 48 bits
postpones it to ~19.7M. Doc 09 plans a 2,400-entry corpus, but
re-rolls and Phase-1 multi-helper-per-task scenarios push the
population above the safe threshold. 6 extra hex chars in the helper
name is the entire cost.

**Rationale:** Free future-proofing; existing tests still pass (the
inequality assertion in
`over_engineer_helper_id_is_seed_dependent` doesn't care about width).

## Other findings (full detail in discovery files)

- `swap_variable.rs:184-194` — f-string interpolation invisible to
  state machine. **Recommend defer + documenting test for Phase 0**;
  full fix needs a brace-counting f-string-aware tokenizer (Phase 1+).
- `over_engineer.rs:40` + `compile_error.rs:45` — duplicated
  `trailing_offset` helper. Hoist to a shared `util.rs` or inline.
- 5x `rng.pick_index(...).ok_or_else(... "rng", "pick_index returned
  None despite non-empty candidates" ...)` repetition — extract to a
  crate-internal helper.
- `delete_test_call.rs:126` — `is_multiline_continuation(line, _next_chunk)`
  has a dead second param; call site at `:83` builds a 200-byte slice
  for nothing. Drop both.
- `compile_error.rs` whole-file — append-only strategy is **PASS** for
  Phase 0; doc 09 §3.1 row 8 ("FAIL at parse") is satisfied. In-place
  corruption is Phase 1+ (needs a tokenizer).

## Out-of-scope per handoff (not reviewed)

Docker oracle wrapper, `CF_MEJEPA_*` RocksDB CFs, 2,400-entry corpus
generation. All deferred per
`memory/context/mejepa-implementation-initiative.md`.

## Recommendation

Land all 7 refinements (top-3 are mechanical and trivially safe; the
others are pure DRY). The f-string blindspot can be deferred with a
documenting test as the operator boundary. No code changes are
required to ship Phase 0 — every refinement is quality-of-life — but
the top-3 reduce noise enough that I'd take them before the next
agent extends this crate.
