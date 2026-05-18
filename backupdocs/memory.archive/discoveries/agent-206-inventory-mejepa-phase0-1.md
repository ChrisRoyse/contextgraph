---
namespace: discoveries
agent_id: agent-206-inventory-mejepa-phase0-1
timestamp: 2026-05-10T00:00:00Z
status: active
tags: inventory, mejepa, phase0, phase1, corpus, instruments, embedders, ast-chunking
---

# ME-JEPA Phase 0 / Phase 1 inventory (read-only)

## 1. context-graph-mejepa-corpus

**Public API (lib.rs):** `Language` (11 variants; only Python `is_supported`), `MutationCategory` (8 variants, `all()`, `slug`, `from_slug`), `MutationConfig`, `MutationOutcome`, `MutationSite`, `apply_mutation()`, `MutationError`, `CorpusProvenance`, `CorpusProvenanceRow`, `compute_corpus_sha256`, `build_corpus_provenance`, `finalize_provenance`, `verify_provenance_consistent`, `parse_languages`, `SplitMix64`, `MEJEPA_CORPUS_VERSION_V1` / `MEJEPA_EMBEDDER_VERSION_V1` constants. Re-exports `OracleVerdict / PerTestOutcome / TestOutcome / ExceptionClass` from instruments. Public submodules: `generate`, `oracle`, `patch_mutation`, `prng`, `python_lex`, `source_patch`, `split`, `swebench`. Private operator modules: `compile_error`, `delete_test_call`, `known_good`, `off_by_one`, `over_engineer`, `subtle_flip`, `swap_variable`, `wrong_file`. CLI bin `mejepa-corpus` at `src/bin/mejepa-corpus.rs`.

**Shipped:** All 8 mutation operators (Python only), source-backed patch builder, RocksDB-backed `OracleStore` with corpus + verdict + provenance CFs, `swebench::run_swebench_lite_oracle()` (single-instance Docker bridge), `split::SplitMode { InstanceAtomic (default), RepoAtomic }` with leakage-prevention checks, finalize/verify provenance with sha256 chain.
**Stubs / pending:** Languages other than Python — `Language::is_supported` returns `false` for 10 of 11 (REQ-CORPUS-13 deferred). Full 300×8 batch corpus generation is partial (one-instance × two-category FSV only). No `unimplemented!()` / `todo!()` / `panic!("not implemented")` markers in src/.
**Tests:** 103 `#[test]` / `#[tokio::test]` annotations under `src/`.
**FSV examples:** `examples/{mejepa_corpus_fsv, oracle_fsv, oracle_store_fsv, split_fsv, swebench_oracle_fsv}.rs` — five FSVs.
**Fail-closed taxonomy:** Yes — `MEJEPA_CORPUS_INVALID_INPUT`, `MEJEPA_CORPUS_NO_MUTATION_SITE`, `MEJEPA_CORPUS_OPERATOR_FAILED`, `MEJEPA_CORPUS_LEAKAGE_DETECTED`.

## 2. context-graph-mejepa-instruments

**Public API:** `Instrument` trait, `InstrumentSlot` (15 variants — `EAst`, `ECfg`, `EDataFlow`, `ETypeGraph`, `ETest`, `ETrace`, `EDiff`, `EWitness`, `EOracle`, `EProblem`, `ECommitMsg`, `EStaticAnalysis`, `ERuntime`, `EReasoning`, `Scalars`), `Panel` / `PanelBuilder` (5,120-d, dim+finite+double-fill validated, JSON via `try_from`), `InstrumentError`, `PANEL_DIM=5_120`. Concrete instruments + inputs: `EOracleInstrument`/`OracleVerdict`, `EWitnessInstrument`/`WitnessChainInput`, `EStaticAnalysisInstrument`, `EAstInstrument`, `ECfgInstrument`, `EDataFlowInstrument`, `ETypeGraphInstrument`, `EDiffInstrument`, `ETestInstrument`/`EProblemInstrument`/`ECommitMsgInstrument`, `ETraceInstrument`/`TraceEvent`/`TraceEventKind`, `ERuntimeInstrument`, `EReasoningInstrument`, `ScalarsInstrument`. Also: `materialize::{materialize_panel, vectors_from_panel, MaterializedPanel, TimeStep}`, `panel_store::{PanelStore, PanelKey, PanelMeta, CF_MEJEPA_PANELS, CF_MEJEPA_PANEL_META, MEJEPA_PANEL_CFS}`, `panel_json::{PanelEnvelope, PanelProvenance, PANEL_JSON_SCHEMA_VERSION}`, `frozen_hook::{FrozenGuard, FrozenSnapshot, hash_f32s}`, `linear_probe::{train_linear_probe, ProbeReport, LINEAR_PROBE_MIN_ACCURACY, assert_probe_passes}`.

**Shipped:** All 15 slots filled with deterministic typed encoders. `E_Oracle`, `E_Witness`, `E_StaticAnalysis`, `Panel`/`PanelBuilder` are spec-complete. Frozen-target hooks, RocksDB panel store, JSON envelope with provenance, linear-probe primitive shipped.
**Stubs / pending:** REQ-INSTR-04..14 are "deterministic encoder shipped, embedder/analyzer-backed replacement deferred" — 11 of 14 instruments have placeholder typed encoders awaiting Qodo/E1/E8/E11/E17/Pyright wiring. No `unimplemented!()` / `todo!()` / `panic!("not implemented")` in src/.
**Tests:** 55 `#[test]` annotations under `src/`.
**FSV examples:** `examples/{instruments_fsv, phase1_extended_fsv, ast_embedder_smoke_fsv}.rs`.
**Fail-closed taxonomy:** Yes — `MEJEPA_INSTRUMENTS_INVALID_INPUT`, `MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT`, `MEJEPA_INSTR_FROZEN_VIOLATION`, `MEJEPA_INSTRUMENTS_STORE_ERROR`.

## 3. context-graph-mejepa-embedders

**Public API:** `EmbedderId` (E1..E21, E5 retired/intentionally absent from `required_registrations()`), `EmbedderKind`, `Ensemble`/`EnsembleStatus`/`LoadedEmbedder`, `ModelsConfig`/`EmbedderRegistration`, `route_for_entity_type` + `routing_coverage` (delegates to `context_graph_core::memory::ast::route_for_entity_type`, refuses retired E5), `digest_file_sha256` / `verify_registration_digest` / `digest_manifest_for_embedder`, `verify_calibration_certificate` / `agent_state_score` / `CalibrationCertificate`, `embedder_coherence` / `predictor_collapse_score` (`CoherenceReport`, `CollapseReport`), `query_vram_budget` / `VramBudget`, types `EmbedderInput` / `EmbedderOutput` / `LoraAdapterRegistration` / `RoutingResult` / `VramBudgetReport`, `EmbedError` / `EmbedResult`.

**Shipped:** 21-embedder ID enum + 13 content + learner-state partition; SHA-256 manifest digest verification, `models_config.toml` schema-versioned loader, VRAM budget query, calibration-certificate gate (E17 fails closed without cert), routing wrapper over core AST table covering 11 languages × 13 entity types, diagnostics.
**Stubs / pending:** No actual model-load / inference path — this crate is a validation/control surface only ("does not synthesize vectors or silently switch to CPU"). Per-embedder Instrument-trait wrappers (REQ-EMBED-02..08), per-chunk-type routing table (REQ-EMBED-09), and lazy-load policy (REQ-EMBED-10) are not yet shipped. No `unimplemented!()` / `todo!()` / `panic!("not implemented")` in src/.
**Tests:** 9 `#[test]` annotations in `src/` (additional in dev-deps / examples).
**FSV examples:** `examples/{embedders_fsv, materialize_models_config}.rs`.
**Fail-closed taxonomy:** Yes — 10 codes: `MEJEPA_EMBED_INVALID_INPUT`, `_CONFIG_READ`, `_CONFIG_PARSE`, `_REGISTRATION_MISSING`, `_WEIGHT_MISSING`, `_DIGEST_MISMATCH`, `_ROUTING_MISSING`, `_GPU_UNAVAILABLE`, `_VRAM_EXCEEDED`, `_E17_UNCALIBRATED`.

## 4. context-graph-core/src/memory/ast_chunker.rs

**Rust-only.** Sole tree-sitter language load is `tree_sitter_rust::LANGUAGE` at line 235; no `tree_sitter_python/javascript/typescript/go/java/c/cpp/c_sharp/ruby/php` references. Error variant `ParseFailed` is hardcoded "Failed to parse Rust source". `UnsupportedExtension` exists, but no language dispatch table. Phase 1a multi-language extension (REQ-AST-02..09) is **not yet wired here** — the multi-language semantic extractor still lives in `crates/context-graph-cli/src/commands/dynamicjepa.rs` per CLAUDE.md §1.

## REQ-* shipping status

- **REQ-CORPUS-01..08, 09, 10, 12, 14:** shipped. **CORPUS-11:** partial (CLI + 1×2 FSV; full 300×8 pending). **CORPUS-13:** pending.
- **REQ-AST-01..13:** all "new" (pending). ast_chunker.rs is Rust-only; multi-language extension not started in this crate.
- **REQ-EMBED-01:** shipped (slot enum + Panel scaffolding lives in instruments crate). **EMBED-02..11:** pending.
- **REQ-INSTR-01, 02, 03, 15:** shipped. **INSTR-04..14, 16:** partial (deterministic typed encoder placeholders shipped; neural/analyzer replacements pending).

## Report

Wrote `/home/cabdru/contextgraph/memory/discoveries/agent-206-inventory-mejepa-phase0-1.md` (this file). One write, no code mutations.
