---
namespace: discoveries
agent_id: agent-205-code-simplifier
timestamp: 2026-05-09T05:02:00Z
status: active
tags: mejepa, phase-0, swap_variable, false-negative, edge-case, fstring
---

# swap_variable's string state machine treats f-string interpolations as opaque strings

## Where

`crates/context-graph-mejepa-corpus/src/swap_variable.rs:167-215`
(`find_first_usage_after`), specifically the `in_string` state machine
at lines 184-194 and 200-204.

## What

Python f-strings (`f"hello {name}"`) put real variable references INSIDE
quoted regions. The state machine here, copied from `subtle_flip` /
`off_by_one`, treats every `b'"'` / `b'\''` as opaque-string-start
regardless of an `f` / `F` / `rf` / `fr` prefix. Consequence: a target
variable whose ONLY downstream usage is inside an f-string will yield
`NoMutationSite` even though Python would 100% reference that name at
runtime.

Concrete failing example (synthetic; not in current tests):

```python
name = "Alice"
greeting = "Hello"
print(f"{greeting}, {name}!")
```

Both `name` and `greeting` are used at runtime, but `find_first_usage_after`
sees `f"{greeting}, {name}!"` as one unbroken string and reports no
usage, so swap_variable returns `MEJEPA_CORPUS_NO_MUTATION_SITE`.

## Why it matters

Real SWE-bench Lite source contains f-strings constantly (Django ORM
log lines, requests error formatting, etc.). For a Phase-0 corpus
generator that's about to be fed real fix-patches, this false-negative
shape will prune legitimate mutation candidates from the corpus —
possibly a non-trivial fraction.

The handoff explicitly asks me to argue this case. The handoff already
notes "currently skipped because of the in_string flag" and asks "is
that the right call?" — the answer is **no, but the cost of fixing it
is real complexity**, so the question is whether to fix or to document.

## Recommended call

Two reasonable options:

1. **Defer + document (cheap).** Add a comment at line 173 stating that
   f-string interpolation is intentionally treated as opaque for Phase
   0 because parsing nested braces in f-strings without a real Python
   tokenizer is risky. Add a regression test that EXPECTS
   `NoMutationSite` for the f-string-only case so future agents don't
   silently change behavior. Phase 1+ can introduce a real tokenizer.

2. **Fix narrowly (~30 LOC).** When entering a string from a `f` / `F`
   prefix, switch to "f-string mode" where `{` opens a brace-counted
   sub-region in which identifiers ARE visible. Track brace depth.
   This is the right thing eventually but it's outside the doc 09 §3.1
   minimum and pulls in tokenizer-shaped problems (escaped braces
   `{{`, nested f-strings since 3.12).

I'd recommend **option 1** for Phase 0 — the corpus is best-effort, the
boundary is documented, and the 2,400-entry generator can simply skip
sources where no candidate is found. Add the regression test.

## Severity

REFINEMENT-RECOMMENDED — strict deferral with documented test.
Acceptable as a known boundary for Phase 0; promote to BLOCKER if Phase
1 corpus generation reveals f-string-only cases are common enough to
distort the corpus distribution.
