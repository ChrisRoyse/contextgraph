---
namespace: reports
agent_id: agent-204-coordinator
created: 2026-05-09T08:00:00Z
updated: 2026-05-09T08:00:00Z
timestamp: 2026-05-09T08:00:00Z
status: active
tags: report, session, mejepa, phase-1, instruments, e-oracle
---

# Session Report: 2026-05-09d — Phase 1 Instruments Scaffolding + E_Oracle

## Task
Per the user's "fully implement next phase" directive — advance from Phase 0
(mostly shipped: mutation operators, split wrapper, tier compression) to
Phase 1 of doc 09's master plan: instruments. Full Phase 1 is ~10-14 days
across 14 instruments + tree-sitter integration. This turn ships the
**scaffolding + the simplest concrete instrument (E_Oracle)** — the
strong, bounded slice that unblocks the rest.

## Outcome: complete (for this primitive)

## Summary
New crate `crates/context-graph-mejepa-instruments/` (~880 lines across
lib.rs + e_oracle.rs + tests.rs):

- **`Instrument` trait** — every E_* instrument's contract:
  `slot()`, `output_dim()`, `encode(input) -> Result<Vec<f32>>`.
- **`InstrumentSlot` enum (15 variants)** — every instrument from
  doc 09 §2.2 (E_AST → E_CommitMsg) plus the 3 added in §3.5
  (E_StaticAnalysis, E_Runtime, E_Reasoning) plus the 128-d Scalars
  bucket. `InstrumentSlot::layout()` enumerates `(slot, offset, dim)`
  triples that sum to exactly `PANEL_DIM = 5_120`.
- **`Panel` and `PanelBuilder`** — fail-closed 5,120-d joint embedding.
  `set_slot(slot, vector)` validates dim, rejects non-finite, rejects
  double-fill. `set_from_instrument(inst, input)` pipes through the
  trait. `require_slots(&[…])` asserts a curated subset is filled
  (used by `panel[t=2]` per doc 09 §2.3).
- **`EOracleInstrument` (128-d)** — first concrete instrument. Encodes
  an `OracleVerdict` (per-test outcomes + optional top-level
  exception) into a fixed-layout 128-d vector:
  - dims 0..4: pass/fail/skip/error fractions (sum to 1 if any tests)
  - dim 4: total test count (capped at 1e6)
  - dim 5: has_exception flag
  - dims 6..16: one-hot exception class (10 classes from doc 09's
    SWE-bench Lite frequency analysis)
  - dims 16..128: zero-padding reserved for future stack-trace
    embedding extension per doc 09 §3.5.

22 unit tests pass. FSV writes 4 real synthetic scenarios
(all-pass, mixed, crash-AssertionError, crash-TypeError) + panel.json
+ 4 boundary cases to disk; all invariants hold.

## Files Created or Modified in the Repo
- `crates/context-graph-mejepa-instruments/Cargo.toml` (NEW)
- `crates/context-graph-mejepa-instruments/src/lib.rs` (~370 lines)
- `crates/context-graph-mejepa-instruments/src/e_oracle.rs` (~185 lines)
- `crates/context-graph-mejepa-instruments/src/tests.rs` (~330 lines)
- `crates/context-graph-mejepa-instruments/examples/instruments_fsv.rs` (~295 lines)
- `Cargo.toml` (workspace root) — added new crate to members + default-members

## Files Written or Updated in ./memory/
- `reports/session-2026-05-09d--phase1-instruments-scaffolding.md` (this file)

## Decisions Made
- **15 slots, not 14** — doc 09 §3.5.5 ships 11 base instruments + 3
  added (E_StaticAnalysis, E_Runtime, E_Reasoning) + 128-d "Scalar
  feature aggregates" bucket = 15 named slots. Keeping the Scalars
  bucket as a slot lets future scalar-feature aggregators consume it
  via the same `set_slot` machinery, instead of introducing a parallel
  side-channel.
- **`PanelBuilder::build` is infallible** — zero-fills unset slots and
  records a `filled_mask` bitmask. This matches doc 09 §2.3 where
  `panel[t=0]` legitimately has E_Diff/E_Trace/E_Witness/E_Oracle/
  E_CommitMsg slots zero-filled. Callers that need stricter semantics
  use `require_slots(&[…])` before `build`.
- **`EOracleInstrument` is unit-struct (`Default`)** — has no
  configuration. The 128-d layout is hard-coded to match the doc's
  spec; future config (e.g. stack-trace embedding hookup) becomes a
  separate `EOracleConfig` field.

## Discoveries
- **`Panel.data` is a `Vec<f32>`, not `[f32; 5120]`** — the array form
  is 20KB on the stack, which Rust will copy/move freely; passing it
  through generic code (e.g. `set_from_instrument`) results in
  pessimistic copies. `Vec` lives on the heap, single-owner-by-default,
  Clone is explicit. Trade-off: slight allocator pressure vs. moving
  20KB stack frames around. Vec wins.
- **Slot layout is computed at runtime via `layout()`**, not const. Rust
  doesn't allow `match slot { ... }` to produce a const array of
  length 15 without macro gymnastics. Runtime computation is fine —
  `layout()` runs once per call (or never, when monomorphized) and
  produces deterministic results. A `#[test]` asserts the offsets
  sum to 5120 so corruption is caught at build-time-via-test.

## Failures
None this turn. Tests passed on the very first build. No bugs surfaced
during testing.

## Successes
- 22 mejepa-instruments tests pass; the new crate compiles clean.
- FSV evidence on disk:
  - `/tmp/contextgraph-mejepa-instruments-fsv/all_pass_input.json` +
    `all_pass_encoded.json` — 10-test all-pass verdict + 128-d encoded
    vector with `[1.0, 0.0, 0.0, 0.0]` first dims.
  - `mixed_*.json` — 2 pass / 1 fail / 1 skip / 1 error → encodes to
    `[0.4, 0.2, 0.2, 0.2]` first dims; sums to exactly 1.0.
  - `crash_assertion_error_*.json` and `crash_type_error_*.json` —
    no tests, exception set; encodes with `dim[5] = 1.0` and the
    correct one-hot bit at `dims[6..16]`.
  - `panel.json` (46121 bytes) — full 5,120-d panel with EOracle slot
    filled, every other slot zero. Read-back from disk equals
    in-memory.
  - 4 boundary cases (empty test_id, wrong dim, NaN, double-fill) all
    reject with the expected error code.
- Total test count across 6 ME-JEPA-foundation crates: 202
  (mejepa-corpus=50 + solver=58 + witness=30 + mincut=18 +
  tier-compression=24 + mejepa-instruments=22).
- No subagent review this turn — the crate is tightly scoped (1
  trait + 1 enum + 1 struct + 1 builder + 1 instrument); review can
  defer to next turn after E_Witness / E_StaticAnalysis ship.

## Open Blockers
None.

## Tests / Build
- Tests added: 22.
- Tests passing: yes.
- Build status: passing.

## Resume Here Next Session

**Doc 09 implementation state after this turn**:
- Phase 0:
  - 8 mutation operators ✓ (turn 2026-05-09)
  - corpus split wrapper ✓ (turn 2026-05-09b)
  - Docker oracle wrapper — DEFERRED
  - RocksDB CFs — DEFERRED
  - 300-task corpus generation — DEFERRED (operational)
- Phase 1:
  - Trait + Panel + PanelBuilder ✓ (this turn)
  - E_Oracle ✓ (this turn)
  - E_AST, E_CFG, E_DataFlow, E_TypeGraph (new encoders, need
    tree-sitter / Pyright) — DEFERRED
  - E_Test, E_Problem, E_CommitMsg (reuse E1) — DEFERRED
  - E_Trace (sequence transformer) — DEFERRED
  - E_Diff (AST diff encoder) — DEFERRED
  - E_Witness (witness chain sequence encoder) — DEFERRED
  - E_StaticAnalysis (linter aggregates) — DEFERRED
  - E_Runtime (runtime metric aggregates) — DEFERRED
  - E_Reasoning (Qwen CoT encoder) — DEFERRED
  - Scalars (BFS depth, blame age, churn, etc.) — DEFERRED
- Doc 05 Tier 1.3 (8/7/5/3-bit tier compression) ✓
- Doc 05 Tier 1.4 (MMR) ✓ (Tier-1 turn)
- Doc 05 Tier 1.6 (conformal prediction) ✓ (Tier-1 turn)

**Highest-leverage next primitives (recommended)**:
1. **E_Witness instrument (256-d)** — sequence encoder over the
   existing 73-byte witness chain entries from `context-graph-witness`.
   Pure-Rust, no external deps. Bounded ~300 LOC. Unblocks Phase 0
   corpus → panel → witness-chain provenance.
2. **E_StaticAnalysis instrument (256-d)** — operates on linter /
   typecheck / complexity / coverage aggregates that doc 09 §3.5.1
   enumerates. Pure-Rust math; the inputs are scalar features the
   harness can extract from existing tools (mypy, ruff). Bounded ~250
   LOC.
3. **E_AST instrument (384-d)** — first instrument requiring
   tree-sitter. ~500-700 LOC + new `tree-sitter-python` dep. The first
   instrument that needs a real neural network or hashed-feature
   approximation. Significant new complexity; defer until at least
   E_Witness + E_StaticAnalysis are done so the trait surface is
   battle-tested.

**Watch out for**:
- The `Panel.data: Vec<f32>` carries 5,120 floats per panel. With 3
  panels per attempt × 5 attempts × 300 tasks × 8 mutations = 36,000
  panels × 20KB = ~720MB of raw f32. The tier-compression crate
  shipped this turn handles this: every panel that reaches cold
  storage gets quantized to 5-bit (~6.4×) or 3-bit (~10×). Wire
  tier-compression into RocksDB before serializing panels at scale.

**Open question**: does doc 09 §2.2's "E_Witness encodes 73-byte chain
entries as integer features" mean a fixed concatenation of the 73
bytes interpreted as 73 separate 0-255 features, or a learned encoder
on the bytes? The doc is ambiguous; a fixed encoder is faster +
deterministic + needs no training; a learned encoder gets better
representations. Default: ship the fixed encoder first; revisit if
the linear probe accuracy is below the 70% threshold doc 09 §7
Phase 1 DoD specifies.

**Priority**: E_Witness is the next instrument to ship — it's
pure-Rust, has zero external deps, operates on data we already
produce, and uses the trait surface we shipped this turn end-to-end.

## Confidence: high
22 tests pass on first build, no warnings, FSV evidence on disk shows
all 4 scenarios + 4 boundary cases working correctly. Panel JSON
round-trips cleanly. Slot layout sums to exactly 5,120 (asserted by
test). The trait + Panel + PanelBuilder API is now ready for the
remaining 13 instruments to plug into.
