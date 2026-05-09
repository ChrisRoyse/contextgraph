# Plan: E_StaticAnalysis Instrument (256-d)

> **Priority:** HIGH (after E_Witness).
> **Effort:** ~2 days.
> **Crate / module:** `crates/context-graph-mejepa-instruments/src/e_static_analysis.rs`
> **Output dim:** 256 (slot 12 of 15).

## Goal

Encode static-analysis signals (mypy/Pyright/ruff/clippy diagnostics
+ complexity metrics + blame age + churn) into a 256-d latent vector.
Pure-Rust scalar-feature aggregates — no neural network in the
shipped placeholder.

Why this is **HIGH priority**:

1. **Pure-Rust math, no external deps**: tools (mypy, ruff, etc.) ARE
   external, but their JSON output is parsed by a small wrapper that
   shells out to already-installed binaries.
2. **No neural network in placeholder**: feature aggregates are the
   final form, not a placeholder. Phase 3 training will not need to
   replace this.
3. **Doubles linear-probe gate evidence**: a third concrete instrument
   (after E_Oracle and E_Witness) further validates the trait surface.
4. **Per-chunk routing via doc 14 multi-language `ast_chunker`**: each
   tool's diagnostic output (mypy errors at line N, ruff errors at
   line N, clippy errors at line N, javac errors at line N) is mapped
   to the **owning AST-perfect chunk** by line-range intersection.
   E_StaticAnalysis aggregates per-chunk severity histograms instead
   of per-file totals, so a 3-error type-check failure in one method
   chunk no longer pollutes the encoding of the 12 other clean
   methods in the same file. This requires the multi-language
   `ast_chunker` extension (doc 14 §6 items 1-5) to ship first;
   until then, E_StaticAnalysis aggregates per-file as a placeholder.

## Deliverables

1. `EStaticAnalysisInstrument` struct implementing `Instrument`:
   ```rust
   pub struct EStaticAnalysisInstrument { ... }
   impl Instrument for EStaticAnalysisInstrument {
       type Input = StaticAnalysisInput;  // (source_text, optional_pyright_json,
                                          //  optional_ruff_json, optional_blame)
       fn slot(&self) -> InstrumentSlot { InstrumentSlot::StaticAnalysis }
       fn output_dim(&self) -> usize { 256 }
       fn encode(&self, input: &StaticAnalysisInput) -> InstrumentResult<Vec<f32>>;
   }
   ```

2. `StaticAnalysisInput` struct:
   ```rust
   pub struct StaticAnalysisInput {
       pub source_text: String,
       pub pyright_diagnostics: Vec<Diagnostic>,    // possibly empty
       pub ruff_diagnostics: Vec<Diagnostic>,
       pub blame_lines: Option<Vec<BlameLine>>,
       pub churn_30d: Option<u32>,
   }

   pub struct Diagnostic {
       pub severity: Severity,        // error | warning | info
       pub category: String,           // "TypeError", "F401", etc.
       pub line: u32,
       pub col: u32,
   }

   pub struct BlameLine {
       pub commit_sha: [u8; 20],
       pub commit_age_days: u32,
       pub author_id_hash: [u8; 8],
   }
   ```

3. Encoding scheme:
   - Dims 0..32: Pyright severity histogram + top-K category counts.
   - Dims 32..64: ruff severity histogram + top-K category counts.
   - Dims 64..96: complexity features (LOC, n_funcs, max_func_LOC,
     n_classes, max_class_methods, cyclomatic_complexity_estimate,
     max_nesting_depth, n_imports, n_TODO_comments, ...).
   - Dims 96..160: blame-age histogram (10 buckets: <1d, 1-7d, 7-30d,
     ..., >5y).
   - Dims 160..192: author-diversity features (top-K author hashes).
   - Dims 192..224: churn features (line additions/deletions over
     30d, normalized).
   - Dims 224..256: error-class one-hot one-bit-per-common-error.

4. Pure-Rust subprocess wrapper for Pyright/ruff (no required hard
   dependency — the input struct accepts pre-parsed diagnostics so the
   instrument itself doesn't shell out unless a helper is invoked).

5. 20+ unit tests:
   - Empty input (zero diagnostics) → zero-padded sensible output.
   - High-severity-error-heavy input → distinct latent.
   - Same source, different mutation → distinguishable latents.
   - Determinism.

6. FSV at `/tmp/contextgraph-mejepa-instruments-fsv/e-static-analysis/`:
   - 100 synthetic source files across 8 mutation categories.
   - Linear probe accuracy ≥0.70 top-1.

## Definition of Done

- All unit tests pass.
- `cargo build --release` clean; `cargo clippy` clean.
- FSV linear probe ≥0.70.
- One-line entry in `01_status_overview.md`.

## Design notes

### Why no neural network in placeholder

E_StaticAnalysis is the rare instrument where hand-engineered scalar
features are PROBABLY good enough — diagnostics counts and complexity
are highly informative for distinguishing CompileError (lots of
errors) from KnownGood (zero errors) from SubtleFlip (zero errors,
identical complexity).

Phase 3 training MAY add a small projection layer if the linear-probe
accuracy is low, but the placeholder is shipped as-is.

### Python AST parsing

For complexity metrics, parse Python source via the lightweight
tree-sitter-python integration (Phase 1 deferred per `phase1.md`).
For the placeholder, fall back to:
- Simple regex line-counts (LOC, n_funcs via `^def `, n_classes via
  `^class `).
- Indentation-depth-based nesting estimate.

These are crude but deterministic.

### Diagnostic parsing

Pyright JSON output format (`pyright --outputjson <file>`):
```json
{
  "generalDiagnostics": [
    { "file": "...", "severity": "error",
      "message": "Cannot access ...", "range": { "start": {"line": 10, "character": 5}, ... } }
  ]
}
```

Parse via `serde_json` into the `Diagnostic` struct. Severity
canonicalized to `error|warning|information|hint`.

### Test inputs

Real synthetic sources via the 8 shipped mutation operators (Phase 0).
For each, run Pyright + ruff (subprocess) and record the JSON. Cache
the per-source JSON in the FSV directory so subsequent test runs don't
re-shell-out.

## Risks

1. **Pyright/ruff not installed in CI**: gate the FSV's
   subprocess-shelling stage on tool availability; instrument unit
   tests use pre-parsed `Diagnostic` fixtures.
2. **Complexity-metric drift across Python versions**: pin the
   tree-sitter-python version (when integrated) to a specific tag.
3. **Linear-probe accuracy <0.70**: investigate which mutation
   categories collide. CompileError vs OverEngineer should be most
   easily separated; SubtleFlip vs KnownGood may be hardest (no
   diagnostic difference).

## Fresh-agent recipe

1. Read `02_shipped/phase1_scaffolding_and_e_oracle.md`.
2. Implement `e_static_analysis.rs` per design above.
3. Add to `lib.rs` modules.
4. Subprocess wrapper for Pyright/ruff (optional helper at module
   level, not part of the trait impl).
5. 20+ unit tests with pre-parsed diagnostic fixtures.
6. FSV harness building real synthetic sources + running pyright/ruff
   + linear probe.
7. End-of-turn: SMAP report.

## See also

- Phase 1 phase doc: `../03_deferred/phases/phase1.md`
- Trait surface: `02_shipped/phase1_scaffolding_and_e_oracle.md`
- Companion plan: `plan_e_witness.md`
