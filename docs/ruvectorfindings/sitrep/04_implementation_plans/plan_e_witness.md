# Plan: E_Witness Instrument (256-d)

> **Priority:** HIGHEST (next turn).
> **Effort:** ~1.5 days.
> **Crate / module:** `crates/context-graph-mejepa-instruments/src/e_witness.rs`
> **Output dim:** 256 (slot 7 of 15 per `InstrumentSlot::layout()`).

## Goal

Encode an attempt's witness chain — an ordered sequence of 73-byte
SHAKE-256 chain entries — into a 256-d latent vector that linearly
separates the 8 mutation categories at >70% top-1 accuracy on the
linear-probe gate.

Why this is the **highest-priority** next instrument:

1. **Pure Rust, no external deps**: no tree-sitter, no Pyright, no
   Docker. The witness chain crate is already shipped.
2. **Real data already produced**: every attempt the reality loop runs
   produces a witness chain; the corpus generation pipeline will
   produce one per `(task_id, mutation_category)` tuple.
3. **Validates the trait surface**: shipping a second concrete
   instrument (after E_Oracle) is the soonest way to discover trait
   ergonomics issues.
4. **Already AST-aware-equivalent (per doc 14 §7)**: witness chain
   entries are already byte-aligned 73-byte units, each one a
   semantically atomic event (op_class + payload_hash + parent_hash +
   timestamp). E_Witness does NOT need to route through the multi-
   language `ast_chunker` — chain entries ARE the atoms. This makes
   E_Witness the cleanest first instrument-after-E_Oracle to validate
   the per-chunk-type DDA signal expansion (each chain entry contributes
   ~7 cross-embedder signals once the chunked panel materializer
   ships).

## Deliverables

1. `EWitnessInstrument` struct implementing `Instrument`:
   ```rust
   pub struct EWitnessInstrument { ... }
   impl Instrument for EWitnessInstrument {
       type Input = WitnessChain;     // from context-graph-witness
       fn slot(&self) -> InstrumentSlot { InstrumentSlot::Witness }
       fn output_dim(&self) -> usize { 256 }
       fn encode(&self, input: &WitnessChain) -> InstrumentResult<Vec<f32>> { ... }
   }
   ```

2. Encoding scheme:
   - Per chain entry, extract: `(stage_id: u8, op_class: u8, payload_hash: [u8; 32], parent_hash: [u8; 32], timestamp: u32)`.
   - Project each entry to a 32-d vector via fixed hash-features.
   - Aggregate across entries via attention-free pooling (mean +
     position-weighted-sum + first-N-bytes-of-final-hash).
   - Final 256-d output: `[mean_32, weighted_32, last_hash_32, op_histogram_64, n_entries_features_8, ... padding to 256]`.

3. 15+ unit tests:
   - Empty-chain rejection.
   - Single-entry chain.
   - Long-chain (>1k entries) numerical stability.
   - Mutated-chain divergence (changing one byte in mid-chain shifts
     the latent appreciably).
   - Determinism: same chain → same vector.

4. Real-data FSV at
   `/tmp/contextgraph-mejepa-instruments-fsv/e-witness/`:
   - 100 synthetic chains across 8 mutation categories.
   - Linear probe accuracy ≥0.70 top-1.
   - Round-trip JSON serialization byte-equal.

## Definition of Done

- All unit tests pass.
- `cargo build --release -p context-graph-mejepa-instruments` clean.
- `cargo clippy -p context-graph-mejepa-instruments` clean.
- FSV linear probe accuracy ≥0.70.
- One-line entry added to `01_status_overview.md` and
  `02_shipped/phase1_scaffolding_and_e_oracle.md` updated to mention
  E_Witness as second shipped instrument.

## Design notes

### Why hash-features, not learned embedding

Phase 1 instruments come in two flavors:
- **Reuse-E1** instruments (E_Test, E_Problem, E_CommitMsg) wrap an
  existing pre-trained encoder.
- **New encoders** (everyone else) ultimately get small neural
  networks trained against the mutation corpus during Phase 3.

E_Witness is in the second class but ships its **placeholder**
hashed-feature variant in this turn, with the contract that Phase 3
training replaces the projection layer with a trained 32-d-per-entry
encoder. The placeholder must produce deterministic, finite, latent-
separable outputs so the trait surface is exercised end-to-end.

### Hash-feature scheme

For each entry's `payload_hash[i]` byte:
```
proj_i = HASH_BASIS[i % 32] * normalize(payload_hash[i] as f32)
```

Aggregate across entries via mean. The HASH_BASIS is a fixed 32×8
matrix of hand-chosen orthogonal directions (or any fixed seed-derived
random matrix) — its only job is to spread the 256 output bits
distinctively per entry-class.

### Position weighting

```
weighted_pool[k] = Σ_i (i / N) * proj_i[k]
```

This captures "later entries matter more" — which is true for
mutation categories like CompileError that flip the FINAL pass/fail
verdict.

### Op-class histogram

Witness chain entries carry an `op_class` byte (PatchApplied,
TestRun, OracleEvaluated, ChainSpliced, etc.). A 64-bin histogram
over up to 64 op classes (truncating excess) gives the predictor
direct visibility into the structural shape of the chain.

### Test inputs (real, no mocks)

Build 100 synthetic chains by:
1. Take 100 SWE-bench Lite task IDs.
2. For each, construct a chain by applying the 8 mutation operators
   (✅ shipped) and recording each step's witness entry via
   `context-graph-witness::Chain::append`.
3. Serialize the 100 chains to
   `/tmp/contextgraph-mejepa-instruments-fsv/e-witness/chains/<task>-<category>.json`.
4. Encode each chain via `EWitnessInstrument::encode`.
5. Run sklearn-style linear probe (multiclass logistic regression in
   pure Rust via gradient descent on the 256-d latents) and report
   top-1 accuracy.

## Risks

1. **Linear probe accuracy <0.70**: indicates hash-feature scheme too
   lossy. Fix: increase per-entry projection from 32 → 64 dims.
2. **Determinism violation across runs**: tighten hash-basis matrix
   construction (no `rand::random()` calls; seed everything).
3. **Empty-chain edge case**: must error with explicit code, not panic.

## Fresh-agent recipe

If a fresh agent picks this up:

1. Read `02_shipped/phase1_scaffolding_and_e_oracle.md` for the trait
   contract.
2. Read `crates/context-graph-witness/src/lib.rs` for the chain entry
   format.
3. Implement `e_witness.rs` per the design above.
4. Add the module to `crates/context-graph-mejepa-instruments/src/lib.rs`.
5. Write 15+ unit tests.
6. Build the 100-chain FSV harness (real chains via existing
   primitives).
7. Run the linear-probe gate.
8. Update sitrep docs.
9. End-of-turn: write a SMAP report and call
   `mcp__cgreality__optimizer_record_recommendation`.

## See also

- Phase 1 phase doc: `../03_deferred/phases/phase1.md`
- Trait surface: `02_shipped/phase1_scaffolding_and_e_oracle.md`
- Witness chain crate: `crates/context-graph-witness/`
