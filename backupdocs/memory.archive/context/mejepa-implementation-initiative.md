---
namespace: context
agent_id: agent-204-coordinator
created: 2026-05-09T03:00:00Z
updated: 2026-05-09T03:00:00Z
timestamp: 2026-05-09T03:00:00Z
status: active
tags: mejepa, doc-09, phase-0, mutation-corpus, audit
---

# ME-JEPA-Code Implementation Initiative — Phase 0 (2026-05-09)

## What this initiative is

Per `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`, ME-JEPA-Code
is the master deliverable: a reality compiler whose 8 implementation phases
total ~43-62 days. Phase 0 is the **bottleneck** per the doc itself
(§7 line 842): *"These foundations do not replace Phase 0. The next
blocking ME-JEPA-Code step is still the mutation corpus."*

This turn ships **Phase 0's mutation operators** as a clean Rust crate.
The full Phase 0 deliverable also includes a Docker oracle wrapper +
RocksDB column families + 2,400-entry corpus generation across 300 SWE-bench
Lite tasks; those require the full SWE-bench environment + multi-hour
oracle runs and are deferred to follow-up turns.

## Doc 09 implementation gap audit (2026-05-09)

### Shipped (foundations covered by Tiers 1-3)

| Doc 09 ref | Status | Crate |
|-|-|-|
| §2.5 conformal calibration set | partial (Tier-1) | `context-graph-solver/conformal.rs` |
| §3.4 Stoer-Wagner mincut for holdout split | shipped (Tier-2) | `context-graph-mincut` |
| §5.6 witness-chain integrity check | shipped (pre-existing) | `context-graph-witness` |
| §5.6 ed25519 segment signing | shipped (Tier-1) | `context-graph-witness/sign.rs` |
| §6.4 Merkle compression primitives | shipped (Tier-2) | `context-graph-witness/cert.rs` |
| Forward-Push PPR + CG | shipped (pre-existing) | `context-graph-solver` |
| MMR diversity rerank | shipped (Tier-1) | `context-graph-solver/mmr.rs` |
| §3.7.1 Granger F-test | shipped (Tier-2) | `context-graph-solver/granger.rs` |
| §2.4 Causal masking + retrocausal safety | shipped (Tier-3) | `context-graph-solver/causal_mask.rs` |

### Phase 0 implementation state (updated 2026-05-09)

| Item | Status | Plan |
|-|-|-|
| 8 mutation operators (subtle_flip, off_by_one, swap_variable, delete_test_call, wrong_file, over_engineer, compile_error, known_good) | SHIPPED for Python source | `crates/context-graph-mejepa-corpus`; parser-backed input validation via `tree-sitter-python`; shared Python lexical mask skips comments, strings, triple-quoted docstrings, and f-string literal text while exposing f-string replacement fields |
| Mutation-site application verification | SHIPPED | `apply_site` verifies byte bounds, UTF-8 boundaries, and `original_text` readback before output; invalid operator state fails with `MEJEPA_CORPUS_OPERATOR_FAILED` |
| Manual FSV evidence | SHIPPED | `cargo run --release -p context-graph-mejepa-corpus --example mejepa_corpus_fsv`; source of truth `/tmp/contextgraph-mejepa-corpus-fsv/mejepa-corpus-evidence.json`; generated `.py` files and `py_compile` outcomes are read back from disk |
| Repo-atomic train/calibration/holdout split helper | SHIPPED | `split.rs` with fail-closed fraction validation and zero cross-cut leakage evidence |
| Rust AST parser harness hardening | SHIPPED | `context-graph-core` Rust AST chunker rejects tree-sitter `ERROR`/`MISSING` recovery nodes with structured diagnostics before emitting chunks; impl functions split from an `impl` block are classified as `Method`; manual FSV source of truth `/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json` |
| C AST parser harness hardening | SHIPPED | C semantic extraction and action gating are declarator-aware: nested `function_declarator` nodes under `function_definition` are not duplicate symbols, prototypes are not dataflow edges, compound type specifiers require bodies to become type declarations, macro definitions are indexed, and compiler-backed C semantics require structured `compile_commands.json` arguments |
| Java AST parser harness hardening | SHIPPED | Java semantic extraction and action gating recognize Maven Surefire/Failsafe test class patterns, `src/test/java`, `src/it/java`, and JUnit Jupiter test-method annotations; live Java action semantics refuse parser-only fallback when `javac` is missing and report `RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING` with a snapshot source-of-truth path |
| C# AST parser harness hardening | SHIPPED | C# semantic extraction and action gating preserve operator overload symbol identity (`operator +`, `operator int`), accept exactly one `.sln`, `.slnx`, or `.csproj` build target, run real `dotnet build <target> --nologo` compiler evidence in the semantic snapshot, and refuse parser-only fallback when `dotnet` is missing with durable source-of-truth readback paths |
| JavaScript/TypeScript AST parser harness hardening | SHIPPED | JavaScript/TypeScript assigned functions resolve symbols from binding sites (`variable_declarator`, `assignment_expression`, object `pair`, named functions) instead of parameter identifiers; TypeScript `.tsx` uses the TSX grammar while `.ts`/`.mts`/`.cts` use the TypeScript grammar; `--language typescript` covers both detected dialects; TypeScript interfaces/types become first-class type graph symbols; nested function-expression grammar nodes do not duplicate symbols; export visibility is ancestor-aware; live TS action semantics climb from inner expressions to the nearest TypeScript owner; parser recovery emits structured diagnostics; TypeScript/TSX compiler evidence requires project-local `node_modules/.bin/tsc` and refuses parser-only fallback |
| Ruby AST parser harness hardening | SHIPPED | Ruby semantic extraction and action gating recognize RSpec DSL examples and Minitest `test_` methods; Ruby spec/test paths feed action attribution and test-assertion patch deltas; DSL symbols use call `method` fields so `RSpec.describe` records `describe`, not receiver `RSpec`; live Ruby action semantics refuse parser-only fallback when `ruby -c` is unavailable |
| Docker oracle wrapper | NOT SHIPPED | DEFERRED — requires full SWE-bench env |
| RocksDB CF_MEJEPA_MUTATION_CORPUS + CF_MEJEPA_ORACLE_VERDICTS | NOT SHIPPED | DEFERRED — depends on Docker oracle |
| 2,400-entry corpus generation | NOT SHIPPED | DEFERRED — operational step, not code |

### Not shipped — Phases 1-8 (deferred to subsequent turns)

| Phase | Item | Effort |
|-|-|-|
| Phase 1 | 11 + 3 instruments (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning) | ~10-14 days |
| Phase 2 | 5,120-d panel + 6-layer transformer predictor + EMA target encoder + VICReg loss | ~7-10 days |
| Phase 3 | Training loop + AdamW + per-step training certs + holdout eval | ~5-7 days |
| Phase 4 | `MeJepaCompiler::compile()` + `::verify()` + conformal calibration + OOD detection + MCP `mejepa_predict` | ~3-5 days |
| Phase 5 | observe() + EWC++ + drift detection + A/B/C promotion + periodic full retrain | ~5-7 days |
| Phase 6 | 8/7/5/3-bit tier compression + nightly GC + Merkle compression + quota enforcement | ~3-5 days |
| Phase 7 | mejepa-shift-subscriber + reality-loop integration + MCP tools | ~5-7 days |
| Phase 8 | Evaluation against end-goal correlation metric (≥0.95) | ongoing |

## Scope of this turn

### IN SCOPE — `crates/context-graph-mejepa-corpus/`

Pure-Rust crate exposing one function per mutation category. Each operator:

- Accepts an input source (the official fix's text or a unified-diff string).
- Returns the mutated source plus structured metadata (`MutationKind`,
  `mutation_site` byte-range, optional `mutation_seed` for determinism).
- Fails closed with `MEJEPA_CORPUS_*` codes when the input is malformed
  or the operator cannot apply (e.g. no boolean operators present for
  `SubtleFlip`).
- Is deterministic: same input + same seed → same mutated output.
- For Python source, validates parseability before mutation, masks non-code
  lexical regions, and verifies every reported mutation site against the
  original source bytes before producing the mutated output.

The 8 operators (per doc 09 §3.1 line 191):

| # | Category | Strategy |
|---|---|---|
| 1 | KnownGood | echo input unchanged |
| 2 | SubtleFlip | invert one boolean operator: `==` ↔ `!=`, `<` ↔ `>=`, `and` ↔ `or`, `True` ↔ `False`, `not X` → `X` |
| 3 | OffByOne | add ±1 to one numeric literal |
| 4 | SwapVariable | rename one local variable to a different in-scope name |
| 5 | DeleteTestCall | delete one assertion call (`assert`, `assertEqual`, `pytest.raises`, etc.) |
| 6 | WrongFile | apply patch text to a different file with similar structure |
| 7 | OverEngineer | append unrelated dead code (e.g. unused helper function) |
| 8 | CompileError | introduce a Python syntax error (unclosed paren, bad indent, etc.) |

### OUT OF SCOPE this turn

- Docker oracle wrapper (deferred to next turn — requires SWE-bench env).
- RocksDB column-family wiring (depends on Docker output schema).
- Full 300-task generation (operational batch, not code).
- Phase 1+ instruments (separate crates, separate scope).

## Subagent coordination

Implementation is coordinator-direct (per
`feedback_no_delegate_research_audit`). The `code-simplifier` subagent
reviews the new crate end-of-turn.

## Cross-references

- Master plan: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §3.1, §7 (Phase 0)
- Tier-1 init: `memory/context/tier1-foundation-gaps-initiative.md`
- Tier-2 init: `memory/context/tier2-foundation-gaps-initiative.md`
- Tier-3 init: `memory/context/tier3-foundation-gaps-initiative.md`
- Upstream policy: `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`
- Solver primitives this consumes: `crates/context-graph-solver/`,
  `crates/context-graph-mincut/`, `crates/context-graph-witness/`
