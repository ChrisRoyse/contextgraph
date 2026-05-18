---
namespace: handoff
agent_id: agent-204-coordinator
created: 2026-05-09T03:30:00Z
updated: 2026-05-09T03:30:00Z
timestamp: 2026-05-09T03:30:00Z
status: active
tags: mejepa, doc-09, phase-0, code-simplifier, review-input
---

# Handoff to code-simplifier — Phase 0 ME-JEPA mutation corpus review

## What you are reviewing

A new crate `crates/context-graph-mejepa-corpus/` implementing doc 09 §3.1's
eight Phase-0 mutation operators. Per
`memory/context/mejepa-implementation-initiative.md`, this is the
in-scope slice of doc 09's Phase 0; Docker oracle wrapper + RocksDB CFs
+ 2,400-entry corpus generation are deferred.

## File list

- `crates/context-graph-mejepa-corpus/Cargo.toml` — workspace member, deps
  serde/serde_json/thiserror.
- `crates/context-graph-mejepa-corpus/src/lib.rs` — public types
  (`MutationCategory`, `MutationConfig`, `MutationOutcome`,
  `MutationSite`), `MutationError` taxonomy with three error codes,
  `apply_mutation` dispatcher, `apply_site` private helper.
- `crates/context-graph-mejepa-corpus/src/prng.rs` — SplitMix64 PRNG.
- `crates/context-graph-mejepa-corpus/src/known_good.rs` — echo unchanged.
- `crates/context-graph-mejepa-corpus/src/subtle_flip.rs` — boolean-op
  candidate scanning + selection. Skips comments + same-line strings,
  bitshifts (`<<` / `>>`), and Python type-annotation arrow (`->`).
- `crates/context-graph-mejepa-corpus/src/off_by_one.rs` — integer-literal
  candidate scanning. Skips floats (preceded OR followed by `.`),
  identifiers (preceded by word byte), and complex/exponent literals
  (`e`, `E`, `j` suffix).
- `crates/context-graph-mejepa-corpus/src/swap_variable.rs` — local-var
  def collection + single-usage rename.
- `crates/context-graph-mejepa-corpus/src/delete_test_call.rs` —
  single-line assertion removal. Skips multi-line / continuation lines.
- `crates/context-graph-mejepa-corpus/src/wrong_file.rs` — replace
  primary with alternate.
- `crates/context-graph-mejepa-corpus/src/over_engineer.rs` — append
  unused helper function with seed-derived suffix.
- `crates/context-graph-mejepa-corpus/src/compile_error.rs` — append
  one of three syntax-error variants (seed-driven).
- `crates/context-graph-mejepa-corpus/src/tests.rs` — 29 tests including
  bug-fix regression cases.
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs` —
  FSV evidence-producer (writes
  `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`).
- `Cargo.toml` (workspace root) — added new crate to members + default-members.

## Verification status (all green)

- `cargo build --release -p context-graph-mejepa-corpus` → clean.
- `cargo test --release -p context-graph-mejepa-corpus` → 29 passed.
- FSV evidence on disk at
  `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`:
  all 8 categories pass; 5 boundary cases reject with the right error
  codes; subtle_flip on real Python with type-annotation arrow correctly
  picks `False` (not the arrow's `>`); off_by_one on real Python with
  floats correctly picks `5` (not `14` from `3.14`).

## Two real bugs the synthetic tests caught (and how I fixed them)

1. **off_by_one on `3.14` was extracting `14` as an integer literal.** Root
   cause: I checked the byte AFTER the digit run for `.` (rejecting
   `3.14` from picking `3`) but didn't check the byte BEFORE for `.`
   (which would reject `14` because it follows `.`). Fix at
   `off_by_one.rs:80-87`: also reject when `prev_byte` is `.` or any
   word byte.

2. **subtle_flip on `def f(x) -> bool` was picking the arrow's `>` as a
   comparison candidate.** Root cause: I only skipped `>>` (bitshift)
   not `->` (type arrow). Fix at `subtle_flip.rs:153-178`: skip `>`
   when preceded by `-`. Also added skip for `>` preceded by `>` (and
   `<` preceded by `<`) to defend against the second char of `<<`/`>>`.

Both bugs have explicit regression tests at `tests.rs:330-360`.

## What I want you to look at, in priority order

1. **subtle_flip's state machine** — the `match_keyword` helper has 6
   parameters but only 3 are used. Vestigial from an earlier shape;
   safe to clean up but the `_*` prefix already silences warnings.
   Check whether the simplification produces fewer LOC without
   sacrificing readability.

2. **swap_variable's downstream-usage logic** — see
   `swap_variable.rs:166-217`. The function searches forward from the
   END of the def line for the first word-boundaried occurrence. Edge
   cases I might have missed: variable used inside a string template
   (`f"{a}"`) — currently skipped because of the in_string flag, which
   means a target with usage ONLY in f-strings would be falsely
   rejected. Is that the right call? Argue.

3. **compile_error's append-only strategy** — the operator only
   APPENDS at EOF; never corrupts an existing line. That's robust but
   limited; the doc 09 §3.1 description suggests "introduce a syntax
   error in the fix" — does append-only count? Argue.

4. **over_engineer's helper-id seed-derivation** — currently a
   24-bit hex tag (`& 0xFFFFFF`). Two helpers from different seeds may
   collide if a generator uses thousands of seeds. Should we widen to
   48-bit?

5. **Anything else** — refactor opportunities, dead code, edge cases,
   redundant validation. Cite file:line in every finding.

## Subagent contract

- READ this handoff plus the crate source.
- WORK: write `memory/discoveries/agent-{N}-code-simplifier--*.md` per
  finding; final report at
  `memory/reports/agent-{N}-code-simplifier--phase0-mejepa-corpus-review--final-report.md`.
- Do NOT mutate code. Recommend only.
- Cite file:line on every finding.

In your tool-result message: list memory files written; PASS/REFINEMENT/BLOCKER counts; top-3 actionable refinements with file:line.

Tight timebox: under 800 words.
