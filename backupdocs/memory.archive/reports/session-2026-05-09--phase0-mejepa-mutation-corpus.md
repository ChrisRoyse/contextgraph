---
namespace: reports
agent_id: agent-204-coordinator
created: 2026-05-09T03:50:00Z
updated: 2026-05-09T03:50:00Z
timestamp: 2026-05-09T03:50:00Z
status: active
tags: report, session, mejepa, doc-09, phase-0, mutation-corpus
---

# Session Report: 2026-05-09 — Phase 0 ME-JEPA Mutation Corpus

## Task
Implement doc 09 §3.1's eight mutation operators as a clean Rust crate
(`context-graph-mejepa-corpus`), per the user's directive: "make sure
everything in [doc 09] is implemented."

## Outcome: partial
The full doc 09 plan is ~43-62 days (8 phases). This turn shipped the
in-scope Phase 0 mutation operators with synthetic-data FSV proof.
Docker oracle wrapper, RocksDB CFs, and full 300-task corpus
generation are deferred to follow-up turns; the Phases 1-8 (instruments,
predictor, training, inference, self-healing, hygiene, integration,
evaluation) are scoped for subsequent turns.

## Summary
Built a new clean-room Rust crate with eight mutation categories
(KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall,
WrongFile, OverEngineer, CompileError) per
`docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.1`. Each
operator is deterministic (SplitMix64-seeded), fail-closed
(`MEJEPA_CORPUS_INVALID_INPUT` / `_NO_MUTATION_SITE` /
`_OPERATOR_FAILED`), and operates on Python source text without
external tree-sitter / Python-runtime dependencies (state-machine
scanner that skips comments + same-line strings). The synthetic
real-Python tests during dev surfaced two real bugs that I fixed at
their root cause (off_by_one picking fractional parts of floats;
subtle_flip picking the `>` of Python type-annotation arrows). Both
are covered by explicit regression tests now.

Code-simplifier subagent (synchronous, read-only, agent-205) reviewed
the crate end-to-end. 0 BLOCKER, 7 quality-of-life refinements, 1 PASS
(compile_error's append-only strategy is the right Phase-0 call). I
applied the top 4 actionable refinements (vestigial-params cleanup,
collapsed redundant float check, widened helper-id mask 24→48 bit, dead
arg removal). The remaining 3 refinements are deferred (DRY trailing-
offset helper, RNG-error boilerplate factoring, f-string blindspot —
the latter requires Phase 1's tokenizer).

## Files Created or Modified in the Repo
- `crates/context-graph-mejepa-corpus/Cargo.toml` — new crate manifest.
- `crates/context-graph-mejepa-corpus/src/lib.rs` — public types,
  dispatcher, error taxonomy.
- `crates/context-graph-mejepa-corpus/src/prng.rs` — SplitMix64.
- `crates/context-graph-mejepa-corpus/src/known_good.rs`,
  `subtle_flip.rs`, `off_by_one.rs`, `swap_variable.rs`,
  `delete_test_call.rs`, `wrong_file.rs`, `over_engineer.rs`,
  `compile_error.rs` — one operator per file.
- `crates/context-graph-mejepa-corpus/src/tests.rs` — 29 tests.
- `crates/context-graph-mejepa-corpus/examples/mejepa_corpus_fsv.rs` —
  FSV evidence-producer.
- `Cargo.toml` (workspace root) — added new crate to members +
  default-members.

## Files Written or Updated in ./memory/
- `context/mejepa-implementation-initiative.md` — gap audit + scope split.
- `handoff/agent-204-coordinator-to-code-simplifier--mejepa-corpus-review.md` — review brief.
- `discoveries/agent-205-code-simplifier--mejepa-*.md` (8 files) — subagent findings.
- `reports/agent-205-code-simplifier--phase0-mejepa-corpus-review--final-report.md` — subagent's report.
- `reports/session-2026-05-09--phase0-mejepa-mutation-corpus.md` — this file.

## Decisions Made
- **Append-only `CompileError` strategy** — robust across any Python
  source without tree-sitter. Subagent reviewed and PASS'd this. File:
  `compile_error.rs:32-41`.
- **24→48-bit helper-id widening for `OverEngineer`** — birthday
  collision pushed from ~4,820 to ~19.7M helpers. File:
  `over_engineer.rs:20-24`.
- **F-string blindspot in `swap_variable`** — DEFERRED with note;
  proper fix needs an f-string-aware tokenizer that belongs in Phase 1.
  See `discoveries/agent-205-code-simplifier--mejepa-swap-fstring-blindspot.md`.

## Discoveries
- **off_by_one accepts the fractional part of floats** when only the
  byte AFTER the digit run is checked for `.`. Fix: also reject when
  byte BEFORE is `.` or any word byte. Impact: false-positive
  candidates inside floats. File: `off_by_one.rs:80-92`.
- **subtle_flip accepts `>` of Python type-annotation arrows
  (`-> bool`)** when only `<<`/`>>` are filtered. Fix: also skip `>`
  preceded by `-`. Impact: corrupts function signatures on every
  type-annotated function. File: `subtle_flip.rs:166-189`.

Both bugs surfaced from real Python test inputs (real `def f() -> bool`
signatures + real `PI = 3.14`). The synthetic-real-Python testing
strategy did exactly what the user asked: it found real bugs, not
fake-test-passes.

## Failures
None this turn. Two real bugs surfaced during dev and were fixed at
root cause with explicit regression tests; no test was modified to
"cover up" the broken behavior — the original assertion checked the
right thing (rejection on no-mutation-site for floats-only source) and
flagged the bug.

## Successes
- **29 tests passing**, all green; no warnings, no clippy issues.
- **FSV evidence on disk** at
  `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`
  (3,350 bytes; readback verified). All 8 categories have
  `passes: true`; all 5 boundary cases reject with the right error
  codes; subtle_flip on real Python correctly picks `False` (not the
  `->` arrow's `>`); off_by_one on real Python with floats correctly
  picks the integer `5` (not the fractional `14`).
- **No regressions** in the 4 prior crates: solver (58), witness (30),
  mincut (18), mejepa-corpus (29) → 135 tests total green.
- **Subagent review** synchronous, single-shot, 0 BLOCKER.

## Open Blockers
None.

## Tests / Build
- Tests added: 29 (mejepa-corpus crate, including 3 explicit
  bug-fix-regression tests).
- Tests passing: yes — 135 total across the 4 ME-JEPA-foundation
  crates.
- Build status: passing.

## Resume Here Next Session

**Context to read first**:
- `memory/context/mejepa-implementation-initiative.md` — the gap audit
  + scope split. Has the table of what's shipped vs. what's missing
  per doc 09's 8 phases.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §7` — the
  Phase Plan with deliverables/DoD/verification per phase.

**The next blocking step is still doc 09 Phase 0 completion**:
1. **Docker oracle wrapper** — invokes the SWE-bench Docker evaluator
   on each `(task_id, mutated_patch)` pair, parses the per-test pass/
   fail JSON, persists the verdict. New module
   `crates/context-graph-mejepa-corpus/src/oracle.rs` (~200 LOC).
   Depends on the operator's local Docker daemon being usable; the
   `agent-72-coordinator--docker-permission-denied-blocks-official-eval.md`
   blocker is RESOLVED per the runbook.
2. **RocksDB column families** `CF_MEJEPA_MUTATION_CORPUS` and
   `CF_MEJEPA_ORACLE_VERDICTS` — wire into
   `crates/context-graph-storage`.
3. **Full 300-task corpus generation** — operational batch run, not
   code; will take ~hours.

**After Phase 0 is complete, Phase 1 (instruments, ~10-14 days) is the
next deliverable per doc 09 §7.** Start with `E_AST` since it's
needed by every panel.

**Watch out for**:
- The f-string blindspot in `swap_variable` will produce false
  `NoMutationSite` rejections for variables only used in `f"{...}"`
  contexts. Add a Phase-1 tokenizer-aware version when the AST
  instrument lands.
- The append-only `compile_error` strategy is intentional for Phase 0
  but will need a tree-sitter-aware in-place corruption variant once
  Phase 1's E_AST instrument lands (it can pick a node and corrupt it
  surgically rather than appending at EOF).

**Priority**: Phase 0's Docker oracle wrapper. Without per-task oracle
verdicts, the corpus is just labeled mutations — useful for sanity
checks but not for training the predictor. Phase 1 instruments cannot
run their `latent_separation_score` evaluation without oracle labels
either.

**Open question**: The doc 09 §7 Phase 0 DoD requires
`leakage_check: PASS` from the Stoer-Wagner mincut splits. Our mincut
crate is shipped; need a wrapper that consumes the corpus's
(task_id, repo) bipartite graph and produces train/calibration/holdout
splits. ~50 LOC.

## Confidence: high
Crate compiles clean, 29 tests pass, FSV evidence on disk shows
byte-exact correctness for all 8 categories. The two bugs surfaced
during testing were fixed at root cause and are now regression-locked.
Subagent review confirmed 0 BLOCKER. Per the user's "fail fast / no
workarounds" directive, every operator returns an explicit error code
when it cannot apply — no silent echo, no fallback.
