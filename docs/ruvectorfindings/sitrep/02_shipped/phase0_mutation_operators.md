# Phase 0 Mutation Operators (8 of 8)

> **Shipped:** 2026-05-09
> **Crate:** `crates/context-graph-mejepa-corpus/src/`
> **Tests:** 50 passing (across 8 operators)
> **FSV evidence:** `/tmp/contextgraph-mejepa-corpus-fsv/` (real Python `py_compile` verification)

## What this primitive does

Eight deterministic mutation operators per doc 09 §3.1, each producing
a `(before, after)` source-text pair from a SWE-bench Lite Python file.
The mutations form the supervision signal for ME-JEPA: each pair has a
known oracle outcome category, letting Phase 3 train the predictor on
labeled data.

| # | Category | Strategy | Status |
|---|---|---|---|
| 1 | KnownGood | echo input unchanged (control) | ✅ |
| 2 | SubtleFlip | invert one boolean operator (`==`↔`!=`, `<`↔`>=`, `and`↔`or`, `True`↔`False`) | ✅ |
| 3 | OffByOne | add ±1 to one integer literal | ✅ |
| 4 | SwapVariable | rename one local-var usage to another in-scope var | ✅ |
| 5 | DeleteTestCall | remove one assertion line (`assert`, `self.assertX(...)`, `pytest.raises`) | ✅ |
| 6 | WrongFile | replace primary source with alternate file's content | ✅ |
| 7 | OverEngineer | append `_unused_helper_<48-bit hex>` function at EOF | ✅ |
| 8 | CompileError | append one of 3 syntax errors (unclosed paren / missing colon / invalid def) | ✅ |

## Why it matters

These eight categories span the realistic distribution of bugs the
inner repair actor (Qwen3.6-27b) faces. KnownGood is the negative
control. SubtleFlip and OffByOne capture the most common LLM-emitted
patches that flip semantics. SwapVariable tests scope analysis.
DeleteTestCall tests test-modification awareness. WrongFile tests
file-locality reasoning. OverEngineer tests the "minimal fix" instinct.
CompileError is the trivial-syntax-error class.

Without all 8 categories, Phase 3 training underfits the bug
distribution.

## Implementation notes

### Real-Python FSV gate
Every operator's output is fed through `python3 py_compile <file>`:
- For categories 1-7: must successfully compile.
- For CompileError: must intentionally fail compilation.
- The FSV harness at `/tmp/contextgraph-mejepa-corpus-fsv/` runs
  `py_compile` on 50 mutation outputs per operator and asserts the
  expected outcome. Missing Python interpreter aborts the run.

### Real bugs caught by real Python
Two bugs surfaced only because the FSV used real Python, not a
hand-rolled tokenizer:

1. **off_by_one.rs:80-92** — original logic checked the byte AFTER a
   digit run for "is this part of an identifier suffix" but didn't
   check the byte BEFORE. So `3.14` had `14` extracted as a "literal"
   and incremented to `15`, producing `3.15` which py_compile accepted
   but is semantically wrong. Fix: also reject when prev byte is `.`.

2. **subtle_flip.rs:166-189** — original logic flipped any `>` it
   found, including the `>` of Python's type-annotation arrow `->`.
   This silently produced syntactically-valid-but-meaningless code
   like `def f(x: int) -< int:`. Fix: skip `>` when preceded by `-`.

Both bugs would have been invisible to a regex-based tokenizer; the
real Python interpreter caught them by being strict.

### python_lex.rs DRY refactor
Linter introduced `python_lex.rs` with f-string awareness. Three
operators (subtle_flip, off_by_one, delete_test_call) consume the
shared lexer rather than each rolling its own.

### Determinism
All operators take a `&mut SplitMix64` PRNG. Same seed → same output.
The corpus generation pipeline pins the seed per `(task_id, category)`
tuple so re-running produces bit-identical mutations.

### Whitespace-preserving
Each operator preserves original line endings (LF vs CRLF), trailing
whitespace, BOM marks, and indent style (tabs vs spaces). Mutations
that change indentation are explicitly out-of-scope.

## What this enables downstream

- **Phase 0 corpus generation** — once Docker oracle ships, runs each
  of 300 tasks × 8 operators = 2,400 entries.
- **Phase 1 instrument linear-probe gate** — uses the labeled
  mutations to measure latent-separation accuracy.
- **Phase 3 training** — direct consumer.
- **Phase 8 evaluation** — generates FRESH mutation seeds for the
  held-out eval set (different RNG seed from corpus generation seed).

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1`
- Phase 0 deferred items: `03_deferred/phases/phase0.md` (Docker oracle
  + RocksDB CFs + full corpus gen)
- Phase 0 split partner: `phase0_corpus_split.md`
- Source: `crates/context-graph-mejepa-corpus/src/{known_good,subtle_flip,off_by_one,swap_variable,delete_test_call,wrong_file,over_engineer,compile_error,python_lex,prng}.rs`
