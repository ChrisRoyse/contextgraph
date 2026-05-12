# 07 — Instruments + Panel(5,120-d)

Source files covered: `crates/context-graph-mejepa-instruments/src/{lib,e_oracle,e_witness,e_static_analysis,e_code,e_text,e_trace,e_scalars,e_runtime,e_reasoning,e_embedder_heads,features,frozen_hook,linear_probe,materialize,panel_json,panel_store,tests}.rs`, `crates/context-graph-mejepa-cf/src/lib.rs`, `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §3`.

---

## 1. Purpose

The 15 frozen instruments are the **mathematical foundation** of ME-JEPA's structural collapse immunity. Each is a deterministic function from a typed event → fixed-dim feature subvector with **zero trainable parameters**. Their concatenation forms the 5,120-d **Panel** — the target representation `f_target(panel_t)` that the predictor learns to map to in latent space.

Because they're not trainable, **`ΔΞ.target_collapse ≡ 0` by construction** (UTML invariant per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`). Any code path that produces non-zero target_collapse is a critical bug — an instrument has accidentally become trainable. Enforced via `MEJEPA_INSTRUMENT_GRADIENT_LEAK`.

---

## 2. The 15 instrument slots (Panel composition)

| # | Slot | Dim | Builder | Inputs | Status |
|---|---|---|---|---|---|
| 1 | `E_AST` | 384 | Qodo + AST-token transformer over chunks | `Vec<Chunk>` from `memory::ast` | partial (deterministic shipped; Qodo/router replacement pending) |
| 2 | `E_CFG` | 256 | E8 + graph-attention over basic blocks | CFG from chunker | partial (deterministic source-shape encoder shipped; E8 head pending) |
| 3 | `E_DataFlow` | 256 | E11 + graph-attention over def-use chains | def-use graph | partial (E11 retired; replacement deferred) |
| 4 | `E_TypeGraph` | 256 | E11 + Pyright/mypy/rust-analyzer/javac/dotnet lattice | type info | partial (analyzer-backed pending) |
| 5 | `E_Test` | 384 | E1 on test-fn descriptions | test functions | partial (E1 replacement pending) |
| 6 | `E_Trace` | 512 | E2/E3/E4 + sequence transformer over `(fn_id, line_no, val_hash)` events | runtime traces | partial (live harness capture pending) |
| 7 | `E_Diff` | 256 | Qodo + diff-edge encoder | before/after source | partial (Qodo head pending) |
| 8 | `E_Witness` | 256 | Sequence transformer over 73-byte chain entries | witness chain | **shipped** |
| 9 | `E_Oracle` | 128 | pass/fail/skip/error fractions + total + has_exception + 10-class exception one-hot | test outcomes | **shipped** |
| 10 | `E_Problem` | 1024 | E1 on SWE-bench problem text | problem statement | partial (E1 replacement pending) |
| 11 | `E_CommitMsg` | 384 | E1 on commit text | commit message | partial |
| 12 | `E_StaticAnalysis` | 256 | scalar aggregates over clippy/pylint/eslint/bandit/cargo-audit | analyzer outputs | **shipped** |
| 13 | `E_Runtime` | 256 | scalar walltime + RSS + coverage Δ | runtime aggregates | partial (live capture pending) |
| 14 | `E_Reasoning` | 384 | E17 AffectText on agent CoT | agent prose | partial (E17 replacement pending) |
| 15 | `Scalars` | 128 | BFS depth + blame age + churn + complexity + coverage Δ + repo health | scalar features | partial (production plumbing pending) |
| **Total** | | **5,120** | | | |

---

## 3. `Instrument` trait

```rust
pub trait Instrument: Send + Sync {
    fn slot(&self) -> InstrumentSlot;
    fn output_dim(&self) -> usize;
    fn encode(&self, event: &TypedEvent) -> Result<Vec<f32>, InstrumentError>;
}

pub enum InstrumentSlot {
    EAst, ECfg, EDataFlow, ETypeGraph,
    ETest, ETrace, EDiff,
    EWitness, EOracle,
    EProblem, ECommitMsg,
    EStaticAnalysis, ERuntime,
    EReasoning, Scalars,
}
```

Each `encode()` is:
- **Deterministic**: same input + same constitution → byte-equal output.
- **Fail-closed**: rejects non-finite floats, dim mismatches, malformed inputs with namespace-specific errors.
- **Stateless**: no internal mutable state across calls.
- **Bounded**: returns exactly `output_dim()` floats; never longer, never shorter.

---

## 4. The Panel

```rust
pub struct Panel {
    pub data: [f32; 5120],
    pub provenance: PanelProvenance,
}

pub struct PanelProvenance {
    pub corpus_sha: Sha256,
    pub embedder_versions: HashMap<EmbedderId, String>,
    pub frozen_at: DateTime<Utc>,
    pub code_version: String,
    pub witness_hash: WitnessHash,
}

pub struct PanelBuilder {
    slots: HashMap<InstrumentSlot, Vec<f32>>,
}

impl PanelBuilder {
    pub fn add_slot(&mut self, slot: InstrumentSlot, data: Vec<f32>) -> Result<(), PanelError>;
    pub fn build(self) -> Result<Panel, PanelError>;
}
```

### 4.1 Invariants (enforced by `panel_json.rs::try_from`)

| Invariant | Error |
|---|---|
| Total dim = exactly 5,120 | `MEJEPA_INFER_DIM_MISMATCH` |
| All 15 slots populated (no missing) | `MEJEPA_INFER_INSTRUMENT_MISSING` |
| No slot filled twice (double-fill) | `MEJEPA_PANEL_DOUBLE_FILL` |
| Every float finite (no NaN, no Inf) | `MEJEPA_PANEL_NON_FINITE` |
| JSON round-trip succeeds (try_from re-validates) | `MEJEPA_PANEL_INVALID_JSON` |
| Provenance metadata present | `MEJEPA_PANEL_MISSING_PROVENANCE` |

### 4.2 PanelStore

Source: `panel_store.rs`. Persists to `CF_MEJEPA_PANELS` keyed by `(session_id, panel_id)`. Per-panel tier policy:

```rust
fn tier_for_panel(meta: &PanelMeta) -> Tier {
    match meta.access_count_30d {
        0..=5     => Tier::Cold,    // 3-bit
        6..=30    => Tier::Cool,    // 5-bit
        31..=100  => Tier::Warm,    // 7-bit
        _         => Tier::Hot,     // 8-bit
    }
}
```

`CF_MEJEPA_PANEL_META` keyed by `panel_id` holds `{ tier, access_count_30d, created_at, last_accessed }`.

---

## 5. The 4 shipped slots in detail

### 5.1 `E_Oracle` (128-d)

Source: `crates/context-graph-mejepa-instruments/src/e_oracle.rs`.

| Sub-slot | Dim | Computation |
|---|---|---|
| pass_fraction | 1 | `count(test == Pass) / total_tests` |
| fail_fraction | 1 | `count(test == Fail) / total_tests` |
| skip_fraction | 1 | `count(test == Skip) / total_tests` |
| error_fraction | 1 | `count(test == Error) / total_tests` |
| total_test_count | 1 | normalized log(1 + total_tests) |
| has_exception | 1 | 0.0 / 1.0 |
| exception_class_onehot | 10 | one-hot over `{AssertionError, AttributeError, KeyError, TypeError, ValueError, IndexError, NameError, ZeroDivisionError, FileNotFoundError, Other}` |
| reserved padding | 112 | zeros (for future test taxonomy expansion) |

**Failure → all zeros except `has_exception=1, exception_class_onehot[exception_class]=1`**. FSV at `/tmp/contextgraph-mejepa-instruments-fsv/e_oracle_*.json`.

### 5.2 `E_Witness` (256-d)

Source: `e_witness.rs`. Sequence-transformer over the last K 73-byte witness-chain entries. Outputs 256-d aggregation. Frozen weights; no training.

### 5.3 `E_StaticAnalysis` (256-d)

Source: `e_static_analysis.rs`. Scalar aggregates over analyzer output deltas (clippy / pylint / eslint / bandit / cargo-audit). Per warning class: `(count_added, count_removed, severity_max, severity_mean)`. Aggregated to 256-d via fixed bucketing.

### 5.4 `Scalars` (128-d)

Source: `e_scalars.rs`. Six scalar features × 21 bins each = 126-d + 2 reserved:

| Scalar | Source |
|---|---|
| BFS depth | distance from package root |
| Blame age | days since last meaningful blame |
| Churn | edits per month (rolling) |
| Complexity | cyclomatic / cognitive (radon / lizard / clippy) |
| Coverage Δ | line + branch coverage diff vs baseline |
| Repo health | per-repo curated yaml (stars, recent activity, test ratio) |

---

## 6. `EmbedderBackedInstrumentHead`

Source: `e_embedder_heads.rs`. Maps validated E7/E1/E14/E10 outputs into code/text/trace panel slots with:

- Source-ID checks (linking the embedder output to the chunk it came from)
- Fail-closed analyzer-feature requirements
- Frozen linear projection per slot (no gradient flow back to the embedder)

```rust
pub struct EmbedderBackedInstrumentHead {
    base: Box<dyn EmbedderForward>,
    projection: LinearProjection,         // FROZEN parameters
    target_slot: InstrumentSlot,
}
```

The projection's parameters are part of the **frozen constitution** — they ship with the constitution version and are SHA-pinned, just like the embedder weights.

---

## 7. Linear-probe gate

Source: `linear_probe.rs`. Validates panel composition by running a small frozen linear probe over the panel and checking the output sits within the expected manifold. Used to:

1. Detect panel-corruption bugs at build time (e.g., slot wired into the wrong index).
2. Prove `frozen_hook` is intact (no accidental gradient flow).
3. FSV evidence at `/tmp/contextgraph-mejepa-instruments-fsv/linear_probe_evidence.json`.

---

## 8. `frozen_hook.rs` — instrument gradient gate

Source: `frozen_hook.rs` + `compute_delta_xi` in `crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs`.

```
ΔΞ_h(t) = (1 − target_collapse_h(t))^0.5
        · (1 − predictor_redundancy_h(t))^0.3
        · (1 − constellation_violation_rate_h(t))^0.2
```

`target_collapse_h ≡ 0` for every head, by construction. If `compute_delta_xi` ever observes `target_collapse > 0` or non-finite, it fails closed with `MEJEPA_INSTRUMENT_GRADIENT_LEAK`. **An instrument has accidentally become trainable. Fix at the trait level, not at the loss.**

---

## 9. PanelSource trait (in `mejepa::panel_source`)

```rust
pub trait PanelSource: Send + Sync {
    fn build_panel(&self, event: &TypedEvent) -> Result<Panel, PanelError>;
    fn build_panel_with_provenance(&self, event: &TypedEvent, prov: PanelProvenance) -> Result<Panel, PanelError>;
}
```

Implementations: `ChunkPanelSource` (from `Vec<Chunk>`), `ShiftPanelSource` (from a shift-log entry — used by the shift subscriber).

---

## 10. `FrozenTargetAdapter`

Source: `crates/context-graph-mejepa/src/frozen_target.rs`. Reads `panel[t=2]` from `CF_MEJEPA_PANELS` and exposes it as the **target** for the predictor's VICReg loss.

The adapter is **read-only**; it never writes back through the instrument trait. Gradients from the predictor flow into the predictor's parameters only; the panel is held fixed.

---

## 11. Panel materialization end-to-end

Source: `materialize.rs`. Called by the shift subscriber per `docs/futurebuild/07_TRAINING_DATA_FLYWHEEL.md §5`:

```rust
fn materialize_shift(shift: &Shift) -> Result<PanelMaterializationOutcome, PanelError> {
    // 1. Read file at after_sha
    let source = storage.read_file(&shift.file, &shift.after_sha)?;
    
    // 2. Chunk
    let chunks = memory::ast::chunk_with_options(&source, shift.language, default_opts())?;
    
    // 3. For each chunk, route + forward + DDA signals
    for chunk in &chunks {
        let embedders = routing::lookup(chunk.language, chunk.kind);
        for emb in embedders {
            let vec = EmbedderForward::forward(emb, chunk)?;
            cache.put((emb, chunk.sha), vec);
        }
        let dda = compute_dda_signals(chunk, &cache);
        persist(CF_MEJEPA_DDA_SIGNALS, (panel_id, chunk.id), dda);
    }
    
    // 4. Build panel
    let mut builder = PanelBuilder::new();
    for slot in InstrumentSlot::all() {
        let instrument = get_instrument(slot);
        let event = TypedEvent::from(&chunks, slot);
        let data = instrument.encode(&event)?;
        builder.add_slot(slot, data)?;
    }
    let panel = builder.build()?;
    
    // 5. Persist
    PanelStore::persist(&panel, session_id, panel_id)?;
    
    Ok(PanelMaterializationOutcome { panel, witness_hash })
}
```

---

## 12. FSV evidence

| FSV | Output |
|---|---|
| All 15 slots | `/tmp/contextgraph-mejepa-instruments-fsv/panel.json` (46 KB) + per-slot evidence |
| E_Oracle 4 cases | `{all_pass, mixed, crash_assertion_error, crash_type_error}_{input,encoded}.json` |
| Boundary cases (empty, max, malformed) | per-slot |
| Linear probe gate | `linear_probe_evidence.json` |

---

## 13. What this subsystem commits to

1. 15 frozen instrument slots = 5,120-d Panel.
2. `ΔΞ.target_collapse ≡ 0` by construction; gradient leak is a critical bug.
3. Per-(session, panel_id) RocksDB persistence with tier policy.
4. Source-of-truth provenance metadata (`corpus_sha + embedder_versions + frozen_at + code_version + witness_hash`).
5. Fail-closed on dim mismatch, non-finite floats, double-fill, missing provenance.
6. Cross-references: [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) (DDA signals from panels), [`09_predictor_multi_head.md`](09_predictor_multi_head.md) (predictor consumes panels), [`10_training_loop_utml.md`](10_training_loop_utml.md) (frozen target via panel[t=2]).
