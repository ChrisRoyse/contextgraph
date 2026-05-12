# Enrichment domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

## TASK-ENRICH-001 — Neural per-embedder forward wrappers

- **Phase:** D · **Status:** complete 2026-05-12 · **Blocked by:** TASK-FLYWHEEL-001 · **Spec:** TECH-ENRICHMENT §3 + 04_DDA_TCT_CODE_ENRICHMENT.md §2
- **Goal:** Implement `EmbedderForward` trait for the live 12 active content embedders: pretrained E1/E6/E7/E8/E10/E12/E13/E14 and algorithmic E2/E3/E4/E9. E17 remains a learner-state/affect embedder in the live registry, not a content embedder.
- **Outputs:** `crates/context-graph-mejepa-embedders/src/forward.rs` with `PretrainedEmbedderForward`, `AlgorithmicEmbedderForward`, and explicit supported-embedder sets.
- **Done 2026-05-12:** all 12 active content slots map to a real forward implementation; temporal slots fail closed unless an explicit timestamp is provided; E9 HDC emits a real algorithmic vector; pretrained wrappers retain digest/runtime artifact checks; full forward FSV reads back outputs for 8 pretrained slots, 4 algorithmic slots, all 11 E7 language inputs, and all 143 routing rows.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/forward-wrapper-evidence.json`, `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/routing-forward-resolution.json`, plus unit tests and `phase_d_embedder_cache_fsv`.

## TASK-ENRICH-002 — DDA signal computation per chunk per shift

- **Phase:** D · **Status:** complete 2026-05-12 · **Blocked by:** TASK-ENRICH-001, TASK-FLYWHEEL-006 · **Spec:** TECH-ENRICHMENT §4
- **Goal:** Implement `compute_dda_signals` per the live `DdaSignals` schema. For each chunk: compute per-embedder cosine to centroid, strict upper-triangle pairwise cosine, cached pairwise MI, and blind-spot z-scores.
- **Outputs:** `crates/context-graph-mejepa-train/src/dda.rs`.
- **DoD:** synthetic 30-chunk patch persists 30 `DdaSignals` rows to `CF_MEJEPA_DDA_SIGNALS`, drops/reopens the RocksDB handle, and reads back first/last rows with correct shapes.
- **FSV:** `/mnt/d/contextgraph/fsv/phase-d-fsv/dda-signal-compute.json`

## TASK-ENRICH-003 — Populate new CFs with real signals

- **Phase:** D · **Status:** complete 2026-05-12 · **Blocked by:** TASK-ENRICH-002
- **Goal:** Wire DDA computation into the shift subscriber pipeline; ensure CF writes are atomic per-shift.
- **Done 2026-05-12:** root-cause fix landed: `context_graph_mejepa_cf::INFER_CFS` now includes `CF_MEJEPA_DDA_SIGNALS` and `CF_MEJEPA_FAILURE_EXEMPLARS`, so inference runtimes open the CFs they need. `persist_dda_signals` writes synchronously and immediately reads back the stored value. The live shift subscriber now computes real algorithmic embedder outputs for each code hunk and persists DDA rows before prediction/watermark side effects.
- **Still open:** trained-head DDA feature consumption is tracked under `TASK-PREDICT-005/006`, not this CF population task.
- **DoD:** running 100 real shifts populates `CF_MEJEPA_DDA_SIGNALS` with 100 × chunk-count signal blocks.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/`

## TASK-ENRICH-004 — Embedder cache layer

- **Phase:** D · **Status:** partial/code_complete 2026-05-12 · **Blocked by:** TASK-ENRICH-001
- **Goal:** Implement `EmbedderCache` per `TECH-ENRICHMENT §5`.
- **Outputs:** `crates/context-graph-mejepa-embedders/src/cache.rs`.
- **Done 2026-05-12:** persistent content-addressed JSON cache keyed by `(schema_version, embedder_id, model_version, chunk_sha256)`; atomic write + fsync + readback; corrupt cache entries fail closed and are not treated as misses.
- **Still open:** production LRU pruning/size-bound enforcement and daemon hit-rate telemetry.
- **FSV:** `/mnt/d/contextgraph/fsv/phase-d-fsv/embedder-cache-fsv.json`

## TASK-ENRICH-005 — VRAM budget gate at startup

- **Phase:** D · **Status:** existing gate present; daemon-load FSV open · **Blocked by:** TASK-ENRICH-001
- **Goal:** Per `TECH-ENRICHMENT §7`. Daemon refuses to start over budget.
- **Done before this pass:** `query_vram_budget` and content-set/full Phase 1 VRAM budget checks exist in `crates/context-graph-mejepa-embedders/src/vram.rs`.
- **Still open:** daemon startup evidence under the full active model load and WSL telemetry hardening.
- **DoD:** configured budget 1 GB + active set requires >1 GB → daemon refuses with clear error and logs free/required/total VRAM.

## TASK-ENRICH-006 — Per-embedder FSV

- **Phase:** D · **Status:** complete 2026-05-12 · **Blocked by:** TASK-ENRICH-001
- **Goal:** One FSV example per embedder asserting deterministic input → expected output vector (bytes) when the model is the SHA-pinned artifact.
- **Outputs:** `crates/context-graph-mejepa-embedders/examples/forward_<embedder>_fsv.rs` (12 files).
- **Done 2026-05-12:** full forward FSV proves E1/E6/E7/E8/E10/E12/E13/E14 pretrained outputs, E2/E3/E4/E9 algorithmic outputs, E7 language-specific code inputs for 11 languages, and 143 Language × EntityType routing rows resolving to read-back output files.
- **DoD:** each FSV passes; vectors have expected dimensions, finite nonzero norms, SHA-pinned model versions, and source-of-truth JSON readback.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-mejepa-forward-fsv/`

## TASK-ENRICH-007 — Per-language stdlib registry

- **Phase:** D · **Status:** complete 2026-05-12 · **Blocked by:** none
- **Goal:** Per `TECH-ENRICHMENT §8`. Curated YAML registry per language for `ReinventedWheel` detection.
- **Outputs:** `crates/context-graph-mejepa-instruments/assets/stdlib_registry/{python,rust,javascript,typescript,go,java,c,cpp,csharp,ruby,php}.yml` (11 files) and `src/stdlib_registry.rs`.
- **DoD:** all 11 YAMLs load + parse; pattern-matcher correctly identifies hand-rolled retry-with-backoff in synthetic Python code; unknown language and missing registry file fail closed.
- **FSV:** `/mnt/d/contextgraph/fsv/phase-d-fsv/stdlib-registry-fsv.json`
