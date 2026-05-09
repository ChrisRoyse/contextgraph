# Deferred Items — Detail Index

Per-item deep-dives for everything that has NOT yet shipped on
`main` (or the active branch) as of 2026-05-09.

## Files

### Phases (doc 09)

- `phases/phase0.md` — Phase 0 remaining (official SWE-bench Docker
  image batch, full corpus generation, optional shared-storage wiring).
- `phases/phase1.md` — Phase 1 remaining (11 of 14 instruments,
  tree-sitter / Pyright integration).
- `phases/phase2.md` — Phase 2 (panel materializer, predictor, frozen
  target readback, VICReg loss, oracle head).
- `phases/phase3.md` — Phase 3 (AdamW training loop, per-step certs,
  holdout-agreement gate ≥0.75).
- `phases/phase4.md` — Phase 4 (verify() API, conformal calibration,
  OOD detection).
- `phases/phase5.md` — Phase 5 (drift detector, EWC++ online fine-tune,
  rollback path).
- `phases/phase6.md` — Phase 6 (panel storage tiering, checkpoint
  deduplication, Merkle-DAG, GC).
- `phases/phase7.md` — Phase 7 (MCP tools, trigger-decision integration,
  witness extension).
- `phases/phase8.md` — Phase 8 (held-out evaluation, ≥0.95 correlation
  gate, ship-or-iterate decision).

### Tiers (doc 05)

- `tier_remaining.md` — every doc 05 Tier item that hasn't shipped:
  Tier 1.5 (LCM), Tier 1.2 (BMSSP V-cycle), Tier 2.1 (SONA), Tier 2.2
  (Verified Training), Tier 2.5 (Matryoshka + Graph RAG / Leiden),
  Tier 3.1 (homomorphic-secure attention), Tier 3.3-3.6 (advanced
  retrieval / agentic tooling).

## Cross-cutting dependencies

Phases and tiers are not independent:

```
Phase 0 corpus ──┬──> Phase 1 instruments ──> Phase 2 panels ──> Phase 3 train
                 │                                   │              │
                 │                                   │              v
                 └─── Phase 8 eval set <─────────────┴───── Phase 4 verify() ──> Phase 7 MCP
                                                                  │              │
                                                                  v              v
                                                            Phase 5 self-heal  reality loop
                                                            (Phase 5 also feeds back into Phase 4)

Tier 1.4/2.4 split ─> Phase 0 split (✅ shipped)
Tier 1.6 ed25519   ─> Tier 2.6 cert chain (✅ shipped)
Tier 2.6 cert      ─> Phase 3 per-step certs (extension)
Tier 1.3 compress  ─> Phase 6 storage tiering (wiring deferred)
Tier 3.2 mask      ─> Phase 2 predictor attention (✅ shipped)
Tier 2.4 mincut    ─> Phase 0 split leakage proof (✅ shipped)
```

## Priority order for next-N turns

The `04_implementation_plans/` directory holds ready-to-execute plans
for the 5 highest-priority deferred items. Order:

1. **Phase 0 full SWE-bench corpus batch** (`plan_phase0_docker_oracle.md`)
   — one official task image, gold/custom harness bridge, manifest
   metadata preflight, parser, and store are shipped; now run the
   remaining generation breadth and persist real labels.
2. **E_Test + E_Problem + E_CommitMsg** — reuse the existing E1 embedder
   path for the next three Phase 1 slots.
3. **E_Diff / E_AST scaffold** — tree-sitter-derived deterministic
   encoders before trained replacements.
4. **Phase 2 predictor** (`plan_phase2_predictor.md`) — the biggest
   single item; needs candle CUDA.
5. **Phase 6 storage wiring** (`plan_phase6_storage_wiring.md`) —
   wires shipped tier-compression into RocksDB + checkpoint paths.
