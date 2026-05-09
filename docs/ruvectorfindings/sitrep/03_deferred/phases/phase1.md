# Doc 09 Phase 1 — Instruments

> **Estimated effort (per doc 09 §7):** 10-14 days.
> **Status (2026-05-09):** PARTIAL — `Instrument` trait + `Panel` +
> `PanelBuilder` + `EOracleInstrument` shipped (1 of 14 instruments).
> 13 instruments deferred.

## Goal

Phase 1 implements the **front-end stages** of the reality compiler:
14 named instruments that each produce a fixed-dim latent vector from
their domain-specific input. Together their outputs concatenate into
the **5,120-d panel** (per doc 09 §3.5.5) — the IR that Phase 2's
predictor consumes.

## Why this phase matters

A panel without instruments is an empty 5,120-d zero vector. Every
later phase (predictor, training, inference, integration) operates on
panels. The instruments ARE the input pipeline.

Per doc 09 §1.1: instruments map directly onto compiler front-end stages
(lexer → parser → semantic analysis → IR). The closer the instruments
match the actual structure of code (AST, CFG, type graph, dataflow),
the better the predictor generalizes.

## Deliverables (per doc 09 §7)

1. **`crates/context-graph-mejepa-instruments/`** with each E_*
   instrument as a module.
2. **Pre-trained encoders** where applicable: E_Test, E_Problem,
   E_CommitMsg reuse `E1` (e5-large-v2) from existing
   `context-graph-embeddings`.
3. **New encoders** (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Trace,
   E_Diff, E_Witness, E_Oracle) as small networks trained against the
   mutation corpus' supervised signals.
4. **Tree-sitter integration** for AST, CFG, DataFlow extraction.
5. **Pyright/mypy integration** for type-graph extraction (Python
   tasks); rust-analyzer for Rust tasks.

## Definition of Done (per doc 09 §7)

- For each instrument, `instrument.encode(input)` returns a fixed-dim
  latent vector.
- **Round-trip test:** encode 100 mutation-corpus examples; the latents
  differentiate categories at >70% top-1 accuracy on a simple linear
  probe.
- All instruments run on RTX 5090 sm_120a CUDA via candle.

## Verification protocol (per doc 09 §7)

```bash
context-graph mejepa-instruments verify --corpus ./data/mejepa-corpus
# expected per-instrument:
#   E_AST: dim=384, latent_separation_score=0.78
#   E_CFG: dim=256, latent_separation_score=0.71
#   ...
```

## What's shipped (✅ this turn series)

### Scaffolding (`crates/context-graph-mejepa-instruments/src/lib.rs`)

- **`Instrument` trait**: `slot()`, `output_dim()`, `encode(input) -> Result<Vec<f32>>`.
- **`InstrumentSlot` enum**: 15 named slots with byte-precise offsets.
  `InstrumentSlot::layout()` returns `(slot, offset, dim)` triples that
  sum to exactly `PANEL_DIM = 5_120`.
- **`Panel` struct**: 5,120-d `Vec<f32>` + `filled_mask: u16`. Created
  via `PanelBuilder` or `Panel::try_new(data, mask)` (the latter
  enforces the same invariants for serde-deserialized panels via
  `serde(try_from = "PanelSerde")`).
- **`PanelBuilder`**: fail-closed `set_slot(slot, vector)` validates
  dim, rejects non-finite, rejects double-fill;
  `set_from_instrument(inst, input)` pipes through the trait;
  `require_slots(&[…])` asserts a curated subset is filled (used by
  `panel[t=2]`).
- **Error taxonomy**: `MEJEPA_INSTRUMENTS_INVALID_INPUT` and
  `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT` codes.

### EOracleInstrument (1 of 14, `src/e_oracle.rs`)

128-d output mapping `OracleVerdict` to:
- dims 0..4: pass/fail/skip/error fractions (sum to 1 if any tests)
- dim 4: total test count (capped 1e6)
- dim 5: has_exception flag
- dims 6..16: 10-class one-hot exception (`AssertionError`, `TypeError`,
  `ValueError`, `ImportError`, `AttributeError`, `KeyError`, `IndexError`,
  `NameError`, `RuntimeError`, `Other`)
- dims 16..128: zero padding reserved for future stack-trace embedding
  extension per §3.5.

27 unit tests + FSV at `/tmp/contextgraph-mejepa-instruments-fsv/`.

## What's deferred (⏳ — 13 instruments + tree-sitter integration)

### Instruments by group

| Slot | Dim | Source | Effort | Priority |
|---|---|---|---|---|
| E_AST | 384 | new — small transformer over tree-sitter AST node-token sequence | ~3 days | medium (after `E_Witness`/`E_StaticAnalysis`) |
| E_CFG | 256 | new — graph attention over basic blocks | ~3 days | medium |
| E_DataFlow | 256 | new — graph attention over def-use edges | ~3 days | medium |
| E_TypeGraph | 256 | new — graph encoder over Pyright/mypy type lattice | ~3 days | medium (needs Pyright/mypy integration) |
| E_Test | 384 | reuse E1 on test-fn natural-language descriptions | ~1 day | low (existing E1 path) |
| E_Trace | 512 | new — sequence transformer over `(fn_id, line_no, val_hash)` events | ~3 days | low (needs runtime trace capture) |
| E_Diff | 256 | new — graph attention over diff-edge events | ~2 days | medium |
| **E_Witness** | 256 | new — sequence transformer over 73-byte chain entries | **~1.5 days** | **HIGHEST** (pure-Rust, no external deps; data already produced) |
| ✅ E_Oracle | 128 | one-hot test outcomes + categorical exception classes | shipped 2026-05-09 | — |
| E_Problem | 1024 | reuse E1 on SWE-bench problem text | ~1 day | low (existing E1) |
| E_CommitMsg | 384 | reuse E1 on commit text | ~1 day | low (existing E1) |
| **E_StaticAnalysis** | 256 | new — scalar feature aggregates (mypy/Pyright/ruff/clippy warnings, complexity, blame age, etc.) | **~2 days** | **HIGH** (pure-Rust math; inputs from already-installed tools) |
| E_Runtime | 256 | new — wall-clock + RSS + coverage + network signal aggregates | ~2 days | medium |
| E_Reasoning | 384 | new — agent-CoT + reflexion-trace encoder (sources: outer-loop conversation transcripts, optimizer recommendation JSONs, reflexion notes; agent-agnostic post-2026-05-09 pivot) | ~2 days | medium |
| Scalars | 128 | aggregates: BFS depth, blame age, churn, coverage Δ, repo-health | ~1 day | low (after Phase-0 Docker oracle ships actual signals) |

### Tree-sitter / Pyright integration

- `tree-sitter-python` crate dependency for AST/CFG/DataFlow.
- Pyright JSON integration (subprocess-based) for E_TypeGraph.
- rust-analyzer LSP integration for Rust SWE-bench tasks (currently
  out-of-scope per Python-heavy SWE-bench Lite).

## Critical path within Phase 1

The trait + Panel scaffold is the keystone — once it shipped, all 14
instruments can be developed independently. The recommended order is:

1. **E_Witness** (highest priority) — pure-Rust, operates on data we
   already produce, validates the trait surface end-to-end with a
   second concrete instrument. ~1.5 days.
2. **E_StaticAnalysis** — pure-Rust scalar aggregates; doesn't need
   neural training. ~2 days.
3. **E_Test, E_Problem, E_CommitMsg** — reuse existing E1 path; bounded
   wrapper code. ~3 days combined.
4. **E_Diff** — bounded scope; needs a tree-sitter dep but the
   encoder is graph attention over a small edge set. ~2 days.
5. **E_AST, E_CFG, E_DataFlow** — biggest tree-sitter-based encoders;
   need a small transformer or graph-attention network. ~9 days
   combined.
6. **E_TypeGraph** — needs Pyright subprocess integration. ~3 days.
7. **E_Trace** — needs runtime trace capture from a real test run.
   Depends on Phase 0 Docker oracle. ~3 days.
8. **E_Runtime, E_Reasoning** — depend on doc 09 §3.5 signal capture
   actually being wired into the harness. ~4 days combined.
9. **Scalars** — last; depends on the per-task aggregate signals being
   present in the corpus. ~1 day.

## Dependencies

- ✅ `Instrument` trait + `InstrumentSlot` + `Panel` + `PanelBuilder`
- ✅ Mutation corpus (Phase 0 partial; the 2,400-entry corpus is needed
  for the linear-probe gate at the end of Phase 1)
- ⏳ Phase 0 Docker oracle (for `E_Trace`, `E_Runtime`, `Scalars`)
- ⏳ Tree-sitter Python crate (for `E_AST`, `E_CFG`, `E_DataFlow`,
  `E_Diff`)
- ⏳ Pyright subprocess integration (for `E_TypeGraph`)

## Implementation order (recommended next 5+ turns)

**Turn 1**: E_Witness (256-d). Pure-Rust sequence encoder over the
73-byte witness chain. ~300 LOC + tests + FSV.

**Turn 2**: E_StaticAnalysis (256-d). Scalar feature aggregates.
~300 LOC + tests + FSV.

**Turn 3**: E_Test + E_Problem + E_CommitMsg. Reuse E1; thin wrappers.
~300 LOC across all three + tests + FSV.

**Turn 4**: E_AST scaffold (without trained weights) — produces
deterministic output via hashed-feature approximation. Allows
downstream phases to compile against real `E_AST::encode` signatures.
Real neural network deferred to Phase 3 training.

**Turn 5**: E_CFG, E_DataFlow, E_Diff scaffolds (same pattern as
E_AST: hashed-feature deterministic outputs).

**Turn 6+**: E_Trace, E_TypeGraph, E_Runtime, E_Reasoning, Scalars
(each requires upstream signal capture in Phase 0 Docker oracle).

## Notes

- Doc 09 §3.5.5 makes the **panel 5,120-d**, not the originally-spec'd
  4,096-d. The shipped scaffold honors the expanded layout (15 slots
  including E_StaticAnalysis, E_Runtime, E_Reasoning, Scalars).
- "Hashed-feature" placeholder encoders make a future-Claude (or a
  human reviewer) able to compile and test the entire panel-builder
  flow without waiting for Phase 3 training. Final neural networks
  replace the placeholders during Phase 3 training; the trait
  signatures don't change.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2,
  §3.5, §7`
- Shipped: `../../02_shipped/phase1_scaffolding_and_e_oracle.md`
- Implementation plans:
  `../../04_implementation_plans/plan_e_witness.md`,
  `plan_e_static_analysis.md`
- Session report:
  `memory/reports/session-2026-05-09d--phase1-instruments-scaffolding.md`
