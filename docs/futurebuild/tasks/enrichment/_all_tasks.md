# Enrichment domain — all atomic tasks

## TASK-ENRICH-001 — Neural per-embedder forward wrappers

- **Phase:** D · **Effort:** 4-5d · **Blocked by:** TASK-FLYWHEEL-001 · **Spec:** TECH-ENRICHMENT §3 + 04_DDA_TCT_CODE_ENRICHMENT.md §2
- **Goal:** Implement `EmbedderForward` trait + Candle/CUDA implementations for 12 active content embedders (E1, E2, E3, E4, E6, E7, E8, E10, E12, E13, E14, E17). Each loads its SHA-pinned model artifact from `/mnt/d/cabdru/contextgraph/models/` and runs real neural forward.
- **Outputs:** `crates/context-graph-mejepa-embedders/src/forward/{e1_e5_large,e7_qodo,e8_graph_attention,e10_paraphrase,e12_colbert,e14_codet5,e17_affect,...}.rs`.
- **DoD:** each embedder returns a non-deterministic vector matching expected shape + dim; per-embedder FSV passes (deterministic input → expected output bytes).
- **FSV:** per-embedder FSV files (12 total).

## TASK-ENRICH-002 — DDA signal computation per chunk per shift

- **Phase:** D · **Effort:** 1d · **Blocked by:** TASK-ENRICH-001, TASK-FLYWHEEL-006 · **Spec:** TECH-ENRICHMENT §4
- **Goal:** Implement `compute_dda_signals` per `TECH-ENRICHMENT §4`. For each chunk × embedder pair: compute cosine to centroid + pairwise cosine + cached pairwise MI. Persist to `CF_MEJEPA_DDA_SIGNALS`.
- **Outputs:** `crates/context-graph-mejepa-train/src/dda/mod.rs` (NEW).
- **DoD:** synthetic 30-chunk patch produces 30 × (12 + 144) = 4,680 signals; round-trip from CF.
- **FSV:** `/mnt/d/contextgraph/fsv/dda-fsv/signal_compute.json`

## TASK-ENRICH-003 — Populate new CFs with real signals

- **Phase:** D · **Effort:** 0.25d · **Blocked by:** TASK-ENRICH-002
- **Goal:** Wire DDA computation into the shift subscriber pipeline; ensure CF writes are atomic per-shift.
- **Outputs:** `shift_subscriber/src/lib.rs` extension.
- **DoD:** running 100 shifts populates `CF_MEJEPA_DDA_SIGNALS` with 100 × N signal blocks.

## TASK-ENRICH-004 — Embedder cache layer

- **Phase:** D · **Effort:** 1d · **Blocked by:** TASK-ENRICH-001
- **Goal:** Implement `EmbedderCache` per `TECH-ENRICHMENT §5`. LRU bounded; persists to `/mnt/d/contextgraph/storage/embedder-cache/`.
- **Outputs:** `crates/context-graph-mejepa-embedders/src/cache.rs` (NEW).
- **DoD:** re-Edit on same chunk → cache hit; no neural forward; verified via call counter.

## TASK-ENRICH-005 — VRAM budget gate at startup

- **Phase:** D · **Effort:** 0.5d · **Blocked by:** TASK-ENRICH-001
- **Goal:** Per `TECH-ENRICHMENT §7`. Daemon refuses to start over budget; `MEJEPA_VRAM_BUDGET_EXCEEDED`.
- **Outputs:** `vram_budget.rs::enforce_startup_budget`.
- **DoD:** configured budget 1 GB + active set requires 6 GB → daemon refuses with clear error.

## TASK-ENRICH-006 — Per-embedder FSV

- **Phase:** D · **Effort:** 2-3d · **Blocked by:** TASK-ENRICH-001
- **Goal:** One FSV example per embedder asserting deterministic input → expected output vector (bytes) when the model is the SHA-pinned artifact.
- **Outputs:** `crates/context-graph-mejepa-embedders/examples/forward_<embedder>_fsv.rs` (12 files).
- **DoD:** each FSV passes; vectors match; cosine similarity to expected = 1.0.

## TASK-ENRICH-007 — Per-language stdlib registry

- **Phase:** D · **Effort:** 1d · **Blocked by:** none
- **Goal:** Per `TECH-ENRICHMENT §8`. Curated YAML registry per language for `ReinventedWheel` detection.
- **Outputs:** `crates/context-graph-mejepa-instruments/data/stdlib_registry/{python,rust,javascript,typescript,go,java,c,cpp,csharp,ruby,php}.yml` (11 files).
- **DoD:** all 11 YAMLs load + parse; pattern-matcher correctly identifies hand-rolled retry-with-backoff in synthetic Python code.
