---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:06:00Z
status: active
tags: mejepa, phase-0, compile_error, design-review, pass
---

# compile_error append-only strategy: PASS for Phase 0

## Where

`crates/context-graph-mejepa-corpus/src/compile_error.rs` (whole file).

## What

The handoff asks me to argue whether append-only is the right call for
Phase 0. The doc 09 §3.1 row 8 description is "introduce a Python
syntax error". The current implementation appends one of three syntax-
error snippets at EOF.

## Argument for PASS (current strategy is right for Phase 0)

1. **Specification fidelity**: doc 09 §3.1 says "FAIL at parse" is the
   expected oracle. Appending a syntax error at EOF makes the FILE fail
   to parse — Python's parser is single-pass, so a syntax error at byte
   N still causes `python3 -c "open('x.py').read()"` to fail. That's
   the contract.

2. **Robustness across input shapes**: corrupting an existing line
   requires understanding the line's role (you can't put `if True\n`
   in the middle of a function definition without potentially breaking
   the surrounding indentation). Append at EOF is invariant: any
   non-empty Python source + appended garbage = a syntactically broken
   Python source.

3. **No tree-sitter dependency**: in-place corruption needs an AST or
   at minimum a tokenizer to know where a statement ends. Pulling in
   `rustpython-parser` or a tree-sitter Python grammar is real cost —
   doc 09's Phase 0 minimum doesn't justify it.

4. **Variant diversity is preserved**: the operator picks from three
   variant shapes via `seed % 3`. The
   `compile_error_variant_distribution_covers_three_bins` test
   confirms all three bins are reached over 30 seeds. So even though
   the location is constant, the FAILURE MODE varies.

5. **Doesn't perturb other categories' invariants**: a corpus consumer
   reading both KnownGood and CompileError outputs sees the original
   source as a strict prefix of the CompileError output. That's a
   useful property for downstream diffing.

## Counter-argument considered + rejected

"In-place corruption looks more like a real bug than appended garbage."
True for the corpus's REALISM dimension, but doc 09 §3.1 row 8 is
explicit about Phase 0 — the predictor only needs to learn "syntax
error → FAIL". Realism is a Phase 1+ concern.

## Severity

PASS — no change recommended for Phase 0.

## Future work flag (Phase 1)

When ME-JEPA moves past Phase 0, replace with a tokenizer-driven
in-place corruption that picks a real statement and breaks it (drop
trailing colon on `if`/`def`/`class` headers, swap `:` for `;` in
function signatures, etc.). Track in
`memory/blockers/` if/when corpus-realism becomes a measurable problem.
