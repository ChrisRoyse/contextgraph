# Phase 1 Scaffolding + E_Oracle + E_Witness + E_StaticAnalysis

> **Shipped:** 2026-05-09
> **Crate:** `crates/context-graph-mejepa-instruments/`
> **Tests:** 40 passing
> **FSV evidence:** `/tmp/contextgraph-mejepa-instruments-fsv/`

## What this primitive does

The Phase 1 trait surface + the first three concrete instruments:

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

6. **`EWitnessInstrument`** (2 of 14) — 256-d output over canonical
   73-byte witness-chain entries from `context-graph-witness`.
   It verifies linked chain bytes before encoding and rejects empty,
   truncated, oversized, tampered, or wrong-version chains.

7. **`EStaticAnalysisInstrument`** (3 of 14) — 256-d output over source
   text plus linter/typechecker diagnostic rows. It rejects blank
   source, bad diagnostic identifiers, excessive diagnostic volume, and
   diagnostic locations outside the analyzed source.

## Why it matters

The trait + Panel scaffold is the keystone of Phase 1. Once it shipped,
all 14 instruments can be developed independently against a stable
contract. The 5,120-d panel layout per doc 09 §3.5.5 is now byte-precise
and tested.

E_Oracle was the easiest instrument to implement. E_Witness and
E_StaticAnalysis harden the scaffold with two additional real signal
types: provenance bytes and analyzer findings. Together they prove the
panel can carry multiple independently verified slots instead of only
the oracle label.

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

### E_Witness safety
- Uses the existing witness source of truth: 73-byte entries from
  `context-graph-witness`.
- Calls `verify_chain_bytes(...)` before feature aggregation.
- Rejects empty/truncated/tampered chains and unsupported format
  versions with `MEJEPA_INSTRUMENTS_INVALID_INPUT`.

### E_StaticAnalysis safety
- Source text must be non-empty and parser-facing UTF-8 without NULs.
- Diagnostic tool/category fields must be non-empty single-line strings.
- Diagnostic lines are one-based and must fit the supplied source text.
- Clean analyzer runs are represented explicitly rather than as missing
  evidence.

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
- 40 unit tests including malformed panel readback, witness-chain
  tamper/version/empty checks, and static-analysis bad-source/bad-line
  checks.
- E_Oracle scenarios: all-pass, mixed pass/fail/skip/error,
  AssertionError crash, TypeError crash.
- E_Witness scenario: raw witness chain bytes at
  `/tmp/contextgraph-mejepa-instruments-fsv/witness_chain.bin` plus
  encoded 256-d vector readback.
- E_StaticAnalysis scenario: analyzer input JSON plus encoded 256-d
  vector readback.
- Panel scenario: E_Oracle + E_Witness + E_StaticAnalysis are filled in
  one 5,120-d panel, read back from JSON, and all unfilled slots are
  checked as zero.

## What this enables downstream

- **All 11 remaining instruments** in Phase 1 — implement against the
  trait surface. Implementation order detailed in `phase1.md`:
  E_Test/E_Problem/E_CommitMsg (E1 reuse) → E_Diff →
  E_AST/E_CFG/E_DataFlow →
  E_TypeGraph → E_Trace/E_Runtime/E_Reasoning → Scalars.
- **Phase 2 panel materializer** (Sub-phase 2.A) — calls
  `PanelBuilder::set_from_instrument` for each shipped instrument when
  building `panel[t=0..2]`.
- **Phase 3 training** — consumes panels.

## See also

- Spec: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2,
  §3.5, §7`
- Phase 1 deferred (11 instruments): `03_deferred/phases/phase1.md`
- Source: `crates/context-graph-mejepa-instruments/src/{lib,e_oracle,e_witness,e_static_analysis,tests}.rs`
- Memory entry: `memory/reports/session-2026-05-09d--phase1-instruments-scaffolding.md`
