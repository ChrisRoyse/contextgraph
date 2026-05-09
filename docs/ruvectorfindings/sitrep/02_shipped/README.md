# Shipped Primitives — Detail Index

This directory holds per-primitive deep-dives for everything that has
landed on `main` (or the active branch) as of 2026-05-09. Each file
covers: scope, location, test count, FSV evidence path, design notes,
and what the primitive enables downstream.

## Files

- `tier1_foundations.md` — generic MMR + split conformal (`context-graph-solver`),
  ed25519 segment signing (`context-graph-witness`).
- `tier2_foundations.md` — Stoer-Wagner mincut (`context-graph-mincut`),
  Granger F-test (`context-graph-solver/granger.rs`), Stage 0 + Stage F +
  Merkle certificates (`context-graph-witness/cert.rs`).
- `tier3_causal_mask.md` — `MaskStrategy` + retrocausal-safety guard
  (`context-graph-causal/mask.rs`); MCP root-cause fix.
- `tier1_3_tier_compression.md` — 8/7/5/3-bit groupwise quantization
  (`context-graph-tier-compression`).
- `phase0_mutation_operators.md` — 8 mutation categories
  (`context-graph-mejepa-corpus`).
- `phase0_corpus_split.md` — exact 240/30/30 instance-atomic split plus
  repo-atomic no-repo-leak diagnostic split
  (`context-graph-mejepa-corpus::split`).
- `phase0_oracle_and_store.md` — command/Docker oracle wrapper + strict
  pytest report parser + RocksDB oracle store (`context-graph-mejepa-corpus::oracle`).
- `phase1_scaffolding_and_e_oracle.md` — `Instrument` trait + `Panel` +
  `EOracleInstrument`, `EWitnessInstrument`, and
  `EStaticAnalysisInstrument` (`context-graph-mejepa-instruments`).

## How to add a new shipped-primitive entry

1. Create `<scope>_<short_name>.md` in this directory.
2. Use this template:

```markdown
# <Tier-X.Y or Phase N> — <Primitive Name>

> **Shipped:** YYYY-MM-DD
> **Crate:** `crates/context-graph-<name>/` (or path to module)
> **Tests:** N passing
> **FSV evidence:** /tmp/contextgraph-<name>-fsv/

## What this primitive does

(1-3 paragraphs)

## Why it matters

(1-2 paragraphs — pointer to which doc 05/09 §)

## Implementation notes

- Key data structure
- Key invariant
- Key gotcha caught during build

## What this enables downstream

(bullet list — phases/tiers that consume this primitive)

## See also

- Spec: doc 05 / doc 09 §<X>
- Implementation plan: 04_implementation_plans/...
- Phase / Tier: 03_deferred/phases/...
```

3. Add a one-line pointer in `01_status_overview.md` and the bullet in
   the parent README.
