# Phase 1 Scaffolding + E_Oracle Instrument

> **Shipped:** 2026-05-09
> **Crate:** `crates/context-graph-mejepa-instruments/`
> **Tests:** 27 passing
> **FSV evidence:** `/tmp/contextgraph-mejepa-instruments-fsv/`

## What this primitive does

The Phase 1 trait surface + the first concrete instrument:

1. **`Instrument` trait** — uniform interface every E_* instrument
   implements:
   ```rust
   pub trait Instrument {
       type Input;
       fn slot(&self) -> InstrumentSlot;
       fn output_dim(&self) -> usize;
       fn encode(&self, input: &Self::Input) -> InstrumentResult<Vec<f32>>;
   }
   ```

2. **`InstrumentSlot` enum** — 15 named slots with byte-precise offsets.
   `InstrumentSlot::layout()` returns `(slot, offset, dim)` triples that
   sum to exactly `PANEL_DIM = 5_120` per doc 09 §3.5.5.

3. **`Panel` struct** — 5,120-d `Vec<f32>` + `filled_mask: u16`.
   Created via `PanelBuilder` or `Panel::try_new(data, mask)` (the
   latter enforces invariants for serde-deserialized panels via
   `serde(try_from = "PanelSerde")`).

4. **`PanelBuilder`** — fail-closed `set_slot(slot, vector)` validates
   dim, rejects non-finite values, rejects double-fill;
   `set_from_instrument(inst, input)` pipes through the trait;
   `require_slots(&[…])` asserts a curated subset is filled (used by
   `panel[t=2]`).

5. **`EOracleInstrument`** (1 of 14) — 128-d output mapping
   `OracleVerdict` to:
   - dims 0..4: pass / fail / skip / error fractions (sum to 1 if any tests)
   - dim 4: total test count (capped 1e6)
   - dim 5: has_exception flag
   - dims 6..16: 10-class one-hot exception (`AssertionError`,
     `TypeError`, `ValueError`, `ImportError`, `AttributeError`,
     `KeyError`, `IndexError`, `NameError`, `RuntimeError`, `Other`)
   - dims 16..128: zero padding reserved for future stack-trace
     embedding extension per §3.5.

## Why it matters

The trait + Panel scaffold is the keystone of Phase 1. Once it shipped,
all 14 instruments can be developed independently against a stable
contract. The 5,120-d panel layout per doc 09 §3.5.5 is now byte-precise
and tested.

E_Oracle is the easiest of the 14 instruments to implement (no neural
network, no external integration) and serves as a worked example for
the next 13.

## Implementation notes

### Panel invariants
- `data.len() == PANEL_DIM == 5_120` (exactly).
- `filled_mask != 0` (at least one slot filled).
- All `data[i]` finite (no NaN, no ±Inf).
- `filled_mask` only sets bits for known `InstrumentSlot` variants
  (no future-version bits set without a known slot).

### Linter-added safety
- `serde(deny_unknown_fields)` on `OracleVerdict`.
- `MAX_TEST_COUNT = 1_000_000` cap to prevent f32 precision loss.
- Control-character rejection in exception class names.
- `runtime_ms < -1.0` rejection.

### serde fail-closed
`Panel` uses `#[serde(try_from = "PanelSerde", into = "PanelSerde")]`.
The `PanelSerde` struct accepts `(data, filled_mask)` from disk; the
`try_from` impl runs the same validation as `Panel::try_new`. So a
malformed-source-of-truth blob fails at deserialize time, not at first
use.

### JSON-can't-carry-Inf gotcha
A test asserted "non-finite rejected" by deserializing JSON with
`f32::INFINITY`. JSON spec emits `null` for non-finite, so the test
was actually testing null-rejection. Fix: switch that subtest to
direct `Panel::try_new(...)` call with infinity values, which exercises
the actual non-finite path.

### FSV at /tmp/contextgraph-mejepa-instruments-fsv/
- 27 unit tests including `panel_deserialization_rejects_malformed_source_of_truth`.
- E_Oracle round-trip: 100 synthetic `OracleVerdict` samples produce
  128-d vectors; recomputed-on-read matches initial encoding bit-for-bit.

## What this enables downstream

- **All 13 remaining instruments** in Phase 1 — implement against the
  trait surface. Implementation order detailed in `phase1.md`:
  E_Witness (highest priority) → E_StaticAnalysis → E_Test/E_Problem/
  E_CommitMsg (E1 reuse) → E_Diff → E_AST/E_CFG/E_DataFlow →
  E_TypeGraph → E_Trace/E_Runtime/E_Reasoning → Scalars.
- **Phase 2 panel materializer** (Sub-phase 2.A) — calls
  `PanelBuilder::set_from_instrument` for each shipped instrument when
  building `panel[t=0..2]`.
- **Phase 3 training** — consumes panels.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2,
  §3.5, §7`
- Phase 1 deferred (13 instruments): `03_deferred/phases/phase1.md`
- Highest-priority next: `04_implementation_plans/plan_e_witness.md`
- Source: `crates/context-graph-mejepa-instruments/src/{lib,e_oracle,tests}.rs`
- Memory entry: `memory/reports/session-2026-05-09d--phase1-instruments-scaffolding.md`
