# 05 — The Frozen Instrument Panel

This document explains the most important architectural decision in ME-JEPA-Code: the **target representation has zero trainable parameters**. It exists because of a problem JEPA-class systems have struggled with for years, and because that problem has a clean structural fix.

## The collapse problem

In any joint-embedding predictive architecture, you have two paths:
- A **predictor** that maps input → some representation
- A **target** that maps a related input → another representation
- A loss that pulls them together

The risk: the predictor and target can both collapse to a constant function. `f_predictor(anything) = [0, 0, 0, ...]` and `f_target(anything) = [0, 0, 0, ...]` makes the loss zero and learns nothing. This is the **representation collapse** problem.

The JEPA literature has tried multiple fixes:
- **EMA target** — keep the target as a moving average of the predictor; partial fix, can still collapse under bad init or weak augmentations.
- **Stop-gradient + asymmetry** — block gradient through the target path; helps but doesn't eliminate collapse.
- **VICReg / Barlow Twins** — regularizers on the embedding variance / off-diagonal covariance; works but is sensitive to hyperparameters.
- **BYOL-style predictor head** — asymmetric predictor MLP; helps, but still tunable into collapse.

ME-JEPA-Code's approach is **structural**: the target representation is produced by **frozen, externally-defined instruments** — deterministic functions of typed events that have no trainable parameters and that exist independently of training.

By definition:
- A frozen instrument cannot collapse because it isn't being trained.
- The target panel is a concatenation of frozen instruments → also can't collapse.
- The collapse term `ΔΞ.target_collapse = 0` by construction.

Formal derivation in [`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2`](../ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md). The summary: any partial derivative of the collapse term with respect to a trainable parameter is zero because none of the trainable parameters appear in the target computation graph.

## What "frozen instrument" means concretely

An **instrument** is a deterministic function `f: TypedEvent → Vec<f32>` such that:
1. `f` has no trainable parameters. It is code defined outside the training loop.
2. `f` is purely a function of the input — no randomness, no global state, no fine-tuning.
3. `f` runs identically on the training corpus and at inference time.
4. `f`'s output dimensionality is fixed in advance and registered in the `InstrumentSlot` enum.

Examples of valid instruments:
- "Number of `Function` AST nodes touched by this edit" (1-d, an integer)
- "One-hot of the exception type observed by the Docker oracle" (10-d, deterministic)
- "Cosine similarity between this chunk and a project-level reference chunk" (1-d, computed by a frozen pretrained embedder)
- "Cyclomatic complexity delta of the touched function" (1-d)
- "Depth in the AST" (1-d)
- "Number of import statements added" (1-d)

Examples of *invalid* instruments (would re-introduce collapse):
- "Output of an EMA of the predictor encoder" — has trainable parameters in its history
- "An MLP trained from the panel" — trainable
- "An embedding from a model whose weights we're fine-tuning" — trainable

## The 15 slots

The current target panel has 15 slots summing to 5,120 dimensions. Each slot is one frozen instrument. Implementation: `context-graph-mejepa-instruments` crate, `InstrumentSlot` enum.

| Slot | Name | Dim | What it captures |
|---|---|---|---|
| 1 | E_Oracle | 128 | Docker SWE-bench oracle outcomes: pass/fail/skip/error fractions + total + has_exception + 10-class exception one-hot |
| 2 | E_AST | 384 | AST chunk metadata: node_kind one-hot, depth, parent chain hash, byte span normalized, has-decorators |
| 3 | E_DataFlow | 384 | Static data-flow features per edited symbol: in-degree, out-degree, reaching-def count, kill-def count, taint flags |
| 4 | E_TypeGraph | 384 | Python type info: declared types at call sites, observed types at call sites, type-violation flags |
| 5 | E_Imports | 256 | Import-graph delta: added imports one-hot, removed imports one-hot, transitive-import-shift score |
| 6 | E_TestIntent | 384 | Test files that validate the touched source paths: count, per-test pass/fail history, test-to-source coupling score |
| 7 | E_DDA | 768 | DDA (Data-Driven Augmentation) signals per chunk: per-embedder cosine to cell centroid, pairwise cosine, blind-spot z-scores |
| 8 | E_Witness | 256 | Merkle root of the chunk's content hash chain + parent commit hash + author |
| 9 | E_Cyclomatic | 64 | Static complexity metrics: cyclomatic, cognitive, halstead, depth-of-nesting |
| 10 | E_Coverage | 256 | Lines covered / not covered by existing tests for the touched files |
| 11 | E_DiffTopology | 384 | Diff structure: hunks per file, hunk size distribution, intra-file diff entropy |
| 12 | E_CallGraph | 384 | Call-graph reachability from touched symbols: callers within K hops, callees within K hops |
| 13 | E_SecuritySignals | 256 | Security-relevant patterns: I/O-on-unsanitized-input flag, auth-check-removed flag, secret-exposure flag |
| 14 | E_Telemetry | 192 | Runtime telemetry projected: predicted latency delta, predicted VRAM delta, predicted cost delta |
| 15 | E_LiveSession | 256 | Session-level state: prior verdicts in this session, prior operator overrides, session goal embedding |

Sum: **5,120-d**. The dimensionalities are choices, not constraints. They were picked to balance expressive power per channel against the size of the final panel.

## How a panel is materialized

The capture surface (PreToolUse / PostToolUse hooks, oracle event capture, test capture) writes typed events to RocksDB CFs. The `PanelBuilder` consumes those events and produces a `Panel` for a given `(session_id, file_path, post_edit_sha)` tuple:

```rust
let panel = PanelBuilder::new()
    .with_instrument(EOracleInstrument::new(&oracle_events_for_task))
    .with_instrument(EAstInstrument::new(&ast_diff))
    .with_instrument(EDataFlowInstrument::new(&dataflow_graph))
    // ... 12 more
    .build()?;  // fail-closed: validate dim + finite + double-fill
```

Validation guards:
- **Dim check** — total dim must equal the registered slot sum (5,120). Off-by-one is rejected with `MEJEPA_PANEL_DIM_MISMATCH`.
- **Finite check** — every entry must be `is_finite()`. NaN or infinity is rejected with `MEJEPA_PANEL_NON_FINITE`.
- **Double-fill check** — the same slot can't be set twice. Catches typos and replay bugs. Rejected with `MEJEPA_PANEL_DOUBLE_FILL`.
- **Order independence** — slots can be added in any order; the final panel is sorted by slot id. Helps replay determinism.

JSON serialization (for FSV evidence and storage): `try_from` round-trips a JSON object back to a `Panel` only if every invariant holds. The round-trip is the format authority.

## Why this is a *truth* system

When you cut all the trainable knobs out of the target path, the target representation becomes a measurement of the world. The Docker oracle's exit code is a fact. The AST diff's typed-delta is a fact. The cyclomatic complexity delta is a fact. The Merkle root of the content hash chain is a fact.

The predictor's job is to learn the mapping from (its trainable input view of the code) to (these facts). It can be wrong; the facts can't.

This is the philosophical inversion that makes ME-JEPA-Code not-a-chatbot. The system is not consulting an LLM that consulted an LLM that consulted an LLM. The system is reading the bytes, parsing them deterministically, running them in a deterministic Docker container, and learning to predict the deterministic outputs of those deterministic processes.

When the predictor disagrees with the target, the target is right by construction. That's the source of truth.

## The "is this a real instrument?" test

Five criteria a candidate instrument must satisfy to be added to the panel:

1. **Deterministic.** Given the same input event, it produces the same output bytes — bit-for-bit. No randomness, no time-dependence, no thread-id dependence.
2. **Reproducible.** Anyone with the same input can compute the same output without consulting our system. (A reviewer or auditor can verify the panel was constructed correctly.)
3. **No trainable params.** Not derived from a model whose weights are still moving.
4. **Bounded.** Output dimension is fixed and registered. Not "grows with input length."
5. **Cheap.** Computable in O(K) where K is the size of the touched code — not in O(history). Replay-able on a checkpoint.

Pretrained embeddings (like e5-large-v2) can be used inside an instrument **if and only if** their weights are frozen — they're treated as deterministic functions from text to fixed-dim vectors. (The 8 GPU-eligible embedders in the predictor path are NOT frozen; they're trainable. The 1-2 embedders used inside frozen instruments — e.g., E_LiveSession — ARE frozen.)

## What's shipped, what's pending

| Slot | Status |
|---|---|
| E_Oracle | ✅ Shipped. Implementation in `context-graph-mejepa-instruments/src/oracle.rs`. 27 tests, FSV evidence at `/mnt/d/contextgraph/fsv/contextgraph-mejepa-instruments-fsv/`. |
| E_AST | 🟡 Design done, implementation pending Phase G. |
| E_DataFlow | 🟡 Pending. |
| E_TypeGraph | 🟡 Pending. |
| E_Imports | 🟡 Pending. |
| E_TestIntent | 🟡 Pending. |
| E_DDA | ✅ Shipped (signals already computed). Wraps `python_pairwise_dda_mi.py` outputs. |
| E_Witness | ✅ Shipped (Merkle witness chain crate exists). |
| E_Cyclomatic | 🟡 Pending. |
| E_Coverage | 🟡 Pending. |
| E_DiffTopology | 🟡 Pending. |
| E_CallGraph | 🟡 Pending. |
| E_SecuritySignals | 🟡 Pending. |
| E_Telemetry | 🟡 Pending. |
| E_LiveSession | 🟡 Pending. |

Phase G's first big task is to land the rest. Each is a self-contained instrument with its own FSV evidence and its own dim contract.

## The collapse-immunity claim, formalized

Given the panel construction:

```
Panel = concat(I_1(event), I_2(event), ..., I_15(event))
```

where each `I_k` is a frozen deterministic instrument:

```
∂Panel / ∂θ_predictor = 0  (for any predictor parameter θ_predictor)
```

Therefore:

```
∂L_collapse / ∂θ_predictor = 0  (where L_collapse measures how degenerate the target panel is)
```

The predictor cannot push the panel toward collapse because it has no access to the panel's computation. The instruments are external. The system is collapse-immune by structure, not by tuning.

That's the architectural innovation. Everything else in this whitepaper is either consequence or implementation detail.
