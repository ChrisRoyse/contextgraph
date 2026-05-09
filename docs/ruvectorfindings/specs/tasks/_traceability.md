# Traceability Matrix — Spec → Task Coverage

> **Most common failure: missing requirements.** This matrix proves every functional+technical spec item has a task. Empty "Task ID" column = INCOMPLETE task generation per `prdtospec.md` §4.4.

**Authority:** `../constitution.md` overrides. Traceability is enforced as a Quality Gate (TASK_REVIEW per `prdtospec.md` §6.2) before task execution.

---

## Coverage status

| Domain | Functional REQs | Technical components | Task specs generated | Coverage |
|---|---:|---:|---:|---|
| CORPUS | 16 | 8 | 28 | 100% |
| AST-CHUNKING | 15 | 8 | 22 | 100% |
| EMBEDDERS | 12 | 5 | 23 | 100% |
| INSTRUMENTS | 23 | 17 | 23 | 100% |
| PREDICTOR | 18 | 6 | 12 | 100% |
| TRAINING | 12 | 7 | 14 | 100% |
| UTML | 12 | 11 | 10 | 100% |
| INFERENCE | 20 | 4 | 14 | 100% |
| TCT | 15 | 7 | 12 | 100% |
| SELF-HEALING | 20 | 7 | 19 | 100% |
| HYGIENE | 10 | 12 | (pending generation) | TBD |
| SHIFT-SUBSCRIBER | 18 | 10 | 14 (subscriber) + 7 (MCP tools) = 21 | 100% |
| EVALUATION | 20 | 5 | 17 | 100% |
| **TOTAL** | **211** | **107** | **TBD** | TBD |

This matrix is automatically updated by atomic-task-generator agents as they emit task specs. Each phase task generator writes a per-domain coverage table into this file.

---

## Per-domain coverage tables

(Auto-populated by atomic-task-generator agents. Each table follows this shape:)

```markdown
### Domain: <DOMAIN>

| Functional REQ | Tech component | Layer | Task ID | Status |
|---|---|---|---|---|
| REQ-<DOMAIN>-01 | <component> | foundation | TASK-<DOMAIN>-001 | pending |
| REQ-<DOMAIN>-02 | <component> | foundation | TASK-<DOMAIN>-002 | pending |
| REQ-<DOMAIN>-03 | <component> | logic      | TASK-<DOMAIN>-003 | pending |
| REQ-<DOMAIN>-04 | <component> | surface    | TASK-<DOMAIN>-004 | pending |

#### Uncovered items
(none — list any functional REQ or tech component without a task)
```

### Domain: CORPUS

> **28 tasks generated** in `./phase0-corpus/`. Foundation: 9 (TASK-CORPUS-001..009). Logic: 16 (TASK-CORPUS-010..025). Surface: 3 (TASK-CORPUS-026..028). Coverage: 100% across 16 REQ-CORPUS-NN + 8 TECH-CORPUS components.

| Functional REQ | Tech component (TECH-CORPUS) | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-CORPUS-01 (generate function) | generate_swe_bench_mutation_corpus, MejepaCorpusCli | logic / surface | TASK-CORPUS-023, TASK-CORPUS-026 | ready |
| REQ-CORPUS-02 (8 mutation operators) | MutationOperator (8 files), python_lex, prng | logic | TASK-CORPUS-010, 011, 012, 013, 014, 015, 016 | ready |
| REQ-CORPUS-03 (run_swebench_lite_oracle) | DockerOracle | logic | TASK-CORPUS-020 | ready |
| REQ-CORPUS-04 (verdict mapping) | parse_swebench_instance_report + swebench_report_to_verdict + extract_exception_class | logic | TASK-CORPUS-018, TASK-CORPUS-019 | ready |
| REQ-CORPUS-05 (Docker run-id validation) | DockerRunIdValidator | foundation | TASK-CORPUS-009 | ready |
| REQ-CORPUS-06 (split_corpus) | SplitEngine | foundation / logic | TASK-CORPUS-005, TASK-CORPUS-017 | ready |
| REQ-CORPUS-07 (assert_no_test_patch_leakage) | SplitEngine | logic | TASK-CORPUS-017 | ready |
| REQ-CORPUS-08 (RocksDB CFs) | OracleStore | foundation / logic | TASK-CORPUS-008, TASK-CORPUS-021 | ready |
| REQ-CORPUS-09 (CLI subcommands) | MejepaCorpusCli | surface | TASK-CORPUS-026, TASK-CORPUS-027, TASK-CORPUS-028 | ready |
| REQ-CORPUS-10 (FSV examples) | MejepaCorpusFsv, SplitFsv | logic | TASK-CORPUS-024, TASK-CORPUS-025 | ready |
| REQ-CORPUS-11 (ensure_parseable_python) | python_lex | logic | TASK-CORPUS-010 | ready |
| REQ-CORPUS-12 (CorpusProvenance) | OracleStore (put_provenance) | foundation / logic | TASK-CORPUS-007, TASK-CORPUS-022 | ready |
| REQ-CORPUS-13 (Language enum) | MejepaCorpusCli (validate_generate_args) | foundation | TASK-CORPUS-002 | ready |
| REQ-CORPUS-14 (timeouts) | DockerOracle (wait_with_timeout) | logic | TASK-CORPUS-020 | ready |
| REQ-CORPUS-15 (verify subcommand) | MejepaCorpusCli (run_verify) | surface | TASK-CORPUS-027 | ready |
| REQ-CORPUS-16 (fail-closed complete flag) | generate_swe_bench_mutation_corpus + MejepaCorpusCli | logic / surface | TASK-CORPUS-023, TASK-CORPUS-026 | ready |

#### Cross-cutting foundation tasks (no single REQ owner)
| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| MutationError + OracleError taxonomy | foundation | TASK-CORPUS-001 | error code mappings; consumed by every other task |
| OracleVerdict / ExceptionClass / PerTestOutcome / TestOutcome re-export | foundation | TASK-CORPUS-003 | re-export from context-graph-mejepa-instruments per integration rule |
| MutationConfig + MutationSite + MutationOutcome | foundation | TASK-CORPUS-004 | consumed by all 8 operators |
| SplitConfig + SplitMode + SplitReport | foundation | TASK-CORPUS-005 | consumed by split engine |
| SwebenchOracleConfig + SwebenchInstanceReport + SwebenchOracleResult | foundation | TASK-CORPUS-006 | consumed by Docker oracle bridge |
| CorpusProvenance + GeneratedEntry + CorpusIndex | foundation | TASK-CORPUS-007 | persistence schema |
| RocksDB CF descriptors + key builders | foundation | TASK-CORPUS-008 | consumed by OracleStore |
| SplitMix64 PRNG | logic | TASK-CORPUS-011 | consumed by 6 mutation operators + split engine + verifier |

#### Uncovered items
(none — all 16 REQ-CORPUS-NN and all 8 TECH-CORPUS component contracts have at least one task; layer ordering Foundation → Logic → Surface preserved; dependency DAG acyclic; no task references files from later tasks)
### Domain: AST-CHUNKING

> **22 tasks generated** in `./phase1-ast-chunking/`. Foundation: 5 (TASK-AST-001..005). Logic: 14 (TASK-AST-006..019). Surface: 3 (TASK-AST-020..022). Coverage: 100% across 15 REQ-AST-NN + 8 TECH-AST-CHUNKING components + per-language classifiers for all 11 languages.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-AST-01 | Uniform chunk(source, language) API | foundation/logic/surface | TASK-AST-001 (Language enum), TASK-AST-010 (parser dispatcher), TASK-AST-021 (chunk facade) | ready |
| REQ-AST-02 | Chunk metadata 9-tuple populated | foundation/logic/surface | TASK-AST-003 (AstChunk struct), TASK-AST-007 (boundary engine), TASK-AST-021 (facade fills sha256+routing+content) | ready |
| REQ-AST-03 | EntityType has exactly 13 variants | foundation | TASK-AST-002 | ready |
| REQ-AST-04 | Reformat-invariant sha256 (AST-node-id hashing) | logic | TASK-AST-006 | ready |
| REQ-AST-05 | cAST 500/100/1000 size targeting + greedy merge | logic | TASK-AST-007 | ready |
| REQ-AST-06 | Per-EntityType embedder routing | foundation/surface | TASK-AST-005, TASK-AST-021 | ready |
| REQ-AST-07 | Per-language FSV evidence at /tmp/contextgraph-{lang}-ast-chunker-fsv/ | surface | TASK-AST-022 | ready |
| REQ-AST-08 | ParseFailed fail-closed + ≤32 ParseDiagnostic | logic/surface | TASK-AST-008, TASK-AST-021 | ready |
| REQ-AST-09 | UnsupportedLanguage exhaustive (compile-time) | foundation/logic | TASK-AST-001, TASK-AST-010 | ready |
| REQ-AST-10 | EmptySource fail-closed | surface | TASK-AST-021 | ready |
| REQ-AST-11 | MixedLanguage fail-closed (PHP+HTML, JS+HTML) | logic | TASK-AST-009 | ready |
| REQ-AST-12 | Per-language TestFunction detection (11 languages) | logic/surface | TASK-AST-011 (Python), 012 (JS), 013 (TS), 014 (Go), 015 (Java), 016 (C), 017 (C++), 018 (C#), 019 (Ruby), 020 (PHP); TASK-AST-021 ports Rust classifier | ready |
| REQ-AST-13 | Cross-run + cross-process determinism | surface | TASK-AST-021 (in-process), TASK-AST-022 (cross-process FSV) | ready |
| REQ-AST-14 | Backwards compat with legacy AstCodeChunker::new_rust | surface | TASK-AST-021 | ready |
| REQ-AST-15 | Performance budget &lt; 200 ms p99 on 10K-line file | surface | TASK-AST-022 | ready |

#### Technical components → Task mapping

| Component | Path | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| AstChunker (top-level facade) | `ast_chunker/mod.rs` + `chunk.rs` | surface | TASK-AST-021 | ready |
| LanguageDispatcher | `ast_chunker/dispatch.rs` | logic | TASK-AST-010 | ready |
| EntityClassifier (per language, 11 modules) | `ast_chunker/classifier/{rust,python,javascript,typescript,go,java,c,cpp,csharp,ruby,php}.rs` | logic/surface | TASK-AST-011..020 + TASK-AST-021 (Rust port) | ready |
| ChunkSha256Hasher | `ast_chunker/hash.rs` | logic | TASK-AST-006 | ready |
| EmbedderRouter | `ast_chunker/router.rs` | foundation | TASK-AST-005 | ready |
| ChunkBoundaryEngine | `ast_chunker/boundary.rs` | logic | TASK-AST-007 | ready |
| MixedLanguageDetector | `ast_chunker/mixed_lang.rs` | logic | TASK-AST-009 | ready |
| ParseDiagnosticCollector | `ast_chunker/diagnostics.rs` | logic | TASK-AST-008 | ready |
| FsvHarness (per-language binary x11) | `examples/{lang}_ast_chunker_fsv.rs` + `ast_chunker/fsv.rs` helper | surface | TASK-AST-022 | ready |

#### Per-language classifier coverage (REQ-AST-12 atomicisation — one task per language)

| Language | Classifier task | TestFunction detection rules | Status |
|---|---|---|---|
| Rust | TASK-AST-021 (port from legacy) | `#[test]`, `#[tokio::test]`, etc. + `#[cfg(test)] mod tests` | ready |
| Python | TASK-AST-011 | `def test_*` in test module/class + `@pytest.fixture` / `@pytest.mark.*` | ready |
| JavaScript | TASK-AST-012 | call_expression to it/test/describe/before/after/beforeEach/afterEach/suite/fixture (lambda body) | ready |
| TypeScript / TSX | TASK-AST-013 | reuses JS rules + TS-specific kinds (interface_declaration, type_alias_declaration, enum_declaration, namespace_declaration, abstract_class_declaration) | ready |
| Go | TASK-AST-014 | name `^Test[A-Z]` + `*testing.T` first param (similar Benchmark/Example) | ready |
| Java | TASK-AST-015 | @Test / @ParameterizedTest / @RepeatedTest / @TestFactory / @TestTemplate (JUnit 5 simple-name match strips Attribute suffix) | ready |
| C | TASK-AST-016 | declarator-aware function_definition vs prototype; never returns TestFunction (deferred per doc 09 §3.1) | ready |
| C++ | TASK-AST-017 | template_declaration unwrap + namespace_definition + qualified_identifier method extraction; never returns TestFunction | ready |
| C# | TASK-AST-018 | [Test] / [Fact] / [Theory] / [TestMethod] attributes (FQN simple-name match) | ready |
| Ruby | TASK-AST-019 | `def test_*` in TestCase class (Minitest) + RSpec it/specify/example/scenario blocks | ready |
| PHP | TASK-AST-020 | `test*` method in class extending TestCase + `@test` PHPDoc annotation | ready |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-AST-01 | Truncated mid-codepoint UTF-8 | TASK-AST-008 (parse_diagnostics emits diagnostics; chunker fails closed via TASK-AST-021) | ready |
| EC-AST-02 | Function body &gt; 4× target_size → warning diagnostic | TASK-AST-007 | ready |
| EC-AST-03 | Arrow function bound to const → binding-site name | TASK-AST-012 | ready |
| EC-AST-04 | C++ template split header/cpp | TASK-AST-017 | ready |
| EC-AST-05 | PHP file with `?>...&lt;?php` blocks | TASK-AST-009 | ready |
| EC-AST-06 | Whitespace/comment-only diff produces identical sha256 | TASK-AST-006 | ready |
| EC-AST-07 | RSpec describe/it nesting | TASK-AST-019 | ready |
| EC-AST-08 | License-only file (single comment) → CommentBlock chunk | TASK-AST-008, TASK-AST-021 | ready |
| EC-AST-09 | Grammar version bump → chunk SHAs change | TASK-AST-022 (cross-process FSV detects via determinism break) | ready |
| EC-AST-10 | Java duplicate class names parse OK; semantic check is E_StaticAnalysis | TASK-AST-015 (parser emits both Class chunks) | ready |
| EC-AST-11 | Go func TestData([]byte) without *testing.T → Function (NOT TestFunction) | TASK-AST-014 | ready |
| EC-AST-12 | 2-line Python helper (below min_size=100) → merged with siblings | TASK-AST-007 | ready |

#### Error states → Task mapping

| Error state | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-AST-01 | MEJEPA_CHUNKER_PARSE_FAILED | TASK-AST-004, TASK-AST-008, TASK-AST-021 | ready |
| ERR-AST-02 | MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE | TASK-AST-001, TASK-AST-004 | ready |
| ERR-AST-03 | MEJEPA_CHUNKER_EMPTY_SOURCE | TASK-AST-004, TASK-AST-021 | ready |
| ERR-AST-04 | MEJEPA_CHUNKER_MIXED_LANGUAGE | TASK-AST-004, TASK-AST-009 | ready |
| ERR-AST-05 | LanguageSetFailed → MEJEPA_CHUNKER_PARSE_FAILED (defense-in-depth) | TASK-AST-004, TASK-AST-010 | ready |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| AstChunkerError taxonomy with .code() | foundation | TASK-AST-004 | consumed by every other task |
| AstChunk + ParseDiagnostic data models with serde | foundation | TASK-AST-003 | typed output contract; EmbedderId placeholder until TASK-EMBED-001 |
| ClassificationContext (inside_class/inside_impl/inside_test_module/parent_named_chain) | foundation | TASK-AST-002 | threaded through ChunkBoundaryEngine |

#### Uncovered items

(none — all 15 REQ-AST-NN, all 8 TECH-AST-CHUNKING component contracts, all 11 per-language classifiers, all 12 edge cases, and all 5 error states have at least one task; layer ordering Foundation (5) → Logic (14) → Surface (3) preserved; dependency DAG acyclic; no task references files from later tasks)

**AST-CHUNKING coverage: 15/15 functional REQs + 8/8 technical components + 11/11 per-language classifiers + 12/12 edge cases + 5/5 error states = 22 tasks.**

### Domain: EMBEDDERS

> **23 tasks generated** in `./phase1-embedders/`. Foundation: 5 (TASK-EMBED-001..005). Logic: 17 (TASK-EMBED-006..022). Surface: 1 (TASK-EMBED-023). Coverage: 100% across 12 REQ-EMBED-NN + 5 TECH-EMBEDDERS components + 7 per-embedder-group wrappers + 8 error states + 9/10 edge cases (1 cross-domain deferral).

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-EMBED-01 | EmbedderInstrument trait wrappers + deterministic encode | foundation/logic | TASK-EMBED-001, 010, 011, 012, 013, 014, 015, 016, 017, 018, 019 | ready |
| REQ-EMBED-02 | SHA-256 weight-digest pin (SEC-MEJEPA-02) | foundation/logic | TASK-EMBED-004, 008 | ready |
| REQ-EMBED-03 | EmbedderId enum (E1-E21) + serde stability | foundation | TASK-EMBED-002 | ready |
| REQ-EMBED-04 | Per-chunk-type routing table | logic | TASK-EMBED-006 | ready |
| REQ-EMBED-05 | Ensemble + VRAM budget (≤29 GB) + lazy load | foundation/logic/surface | TASK-EMBED-004, 007, 009, 019, 023 | ready |
| REQ-EMBED-06 | LoraAdapterManager (register/refresh/merge) + frozen-base SHA invariant | foundation/logic | TASK-EMBED-004, 020 | ready |
| REQ-EMBED-07 | embedder_coherence (jaccard NN over content embedders) | logic | TASK-EMBED-022 | ready |
| REQ-EMBED-08 | predictor_collapse_score (cross-embedder std) | logic | TASK-EMBED-022 | ready |
| REQ-EMBED-09 | agent_state_score (E17 valence/arousal_inv_U) + calibration cert gate | logic | TASK-EMBED-021 | ready |
| REQ-EMBED-10 | Determinism contract (input, weight_sha, code_version, seed) | foundation/logic | TASK-EMBED-003, 009, 010 | ready |
| REQ-EMBED-11 | 3 MCP tools under mcp__cgreality__mejepa_* | surface | TASK-EMBED-023 | ready |
| REQ-EMBED-12 | NFR perf targets + forward-pass cache | logic | TASK-EMBED-009 | ready |

#### Technical components → Task mapping

| Component | Path | Layer | Task ID | Status |
|---|---|---|---|---|
| EmbedderInstrument&lt;E&gt; trait + 21 wrappers | `instrument.rs` + `embedders/*.rs` | logic | TASK-EMBED-001, 010-019 | ready |
| Ensemble manager + forward cache | `ensemble.rs` + `cache.rs` | logic | TASK-EMBED-009 | ready |
| RoutingTable | `routing.rs` | logic | TASK-EMBED-006 | ready |
| LoraAdapterManager | `lora_manager.rs` | logic | TASK-EMBED-020 | ready |
| VramBudget | `vram_budget.rs` | logic | TASK-EMBED-007 | ready |

#### Per-embedder wrappers (REQ-EMBED-01 atomicisation)

| Embedder | Forward + Factory + Instrument | Task ID | Status |
|---|---|---|---|
| E1 Semantic (e5-large-v2, 1024-d) | `embedders/e1_semantic.rs` | TASK-EMBED-010 | ready |
| E5 Causal (nomic-embed, 768-d, asymmetric, LoRA-target) | `embedders/e5_causal.rs` | TASK-EMBED-011 | ready |
| E7 Code (Qodo-Embed-1-1.5B, 1536-d, 32K-token) | `embedders/e7_code.rs` | TASK-EMBED-012 | ready |
| E8 Graph (e5-large-v2 reuse, 1024-d) | `embedders/e8_graph.rs` | TASK-EMBED-013 | ready |
| E10 Contextual (e5-base-v2, 768-d) | `embedders/e10_contextual.rs` | TASK-EMBED-014 | ready |
| E11 Kepler (RoBERTa+TransE, 768-d, LoRA-target) | `embedders/e11_kepler.rs` | TASK-EMBED-015 | ready |
| E14 BgeM3Dense (BGE-M3, 1024-d, 8K-token, multilingual, LoRA-target) | `embedders/e14_bge_m3.rs` | TASK-EMBED-016 | ready |
| E2/E3/E4 Temporal (exp-decay / Fourier / sin-PE, 512-d each) | `embedders/temporal.rs` | TASK-EMBED-017 | ready |
| E6/E9/E12/E13 Sparse / HDC / ColBERT / SPLADE | `embedders/sparse_hdc_colbert_splade.rs` | TASK-EMBED-018 | ready |
| E15-E21 Learner-state (lazy-load, all 7) | `embedders/learner_state.rs` | TASK-EMBED-019 | ready |

#### Error states → Task mapping

| Error | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-EMBED-01 | MEJEPA_EMBED_INVALID_INPUT | TASK-EMBED-005, 010, 011-019 | ready |
| ERR-EMBED-02 | MEJEPA_EMBED_DIGEST_MISMATCH | TASK-EMBED-005, 008, 009 | ready |
| ERR-EMBED-03 | MEJEPA_EMBED_NUMERICAL_INVARIANT | TASK-EMBED-003, 005 | ready |
| ERR-EMBED-04 | MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED | TASK-EMBED-005, 007, 009 | ready |
| ERR-EMBED-05 | MEJEPA_EMBED_FROZEN_BASE_VIOLATION | TASK-EMBED-004, 005, 020 | ready |
| ERR-EMBED-06 | MEJEPA_EMBED_ROUTING_MISSING | TASK-EMBED-005, 006 | ready |
| ERR-EMBED-07 | MEJEPA_EMBED_E17_UNCALIBRATED | TASK-EMBED-005, 021 | ready |
| ERR-EMBED-08 | MEJEPA_EMBED_DETERMINISM_VIOLATION | TASK-EMBED-005 | ready |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-EMBED-01 | Empty text input | TASK-EMBED-003, 010, 011-019 | ready |
| EC-EMBED-02 | max_tokens truncation | TASK-EMBED-010, 012, 016 | ready |
| EC-EMBED-03 | Multi-EntityType chunk | TASK-EMBED-006 | ready |
| EC-EMBED-04 | Cold-start concurrent init | TASK-EMBED-009 | ready |
| EC-EMBED-05 | LoRA fit interrupted | TASK-EMBED-020 | ready |
| EC-EMBED-06 | Corpus 999 entries | TASK-EMBED-022 | ready |
| EC-EMBED-07 | JSON-only transcripts to E17 | TASK-EMBED-021 | ready |
| EC-EMBED-08 | Tokenizer normalisation drift | TASK-EMBED-009 (code_version SHA) | ready |
| EC-EMBED-09 | file_sha not in corpus → on-demand panel compute | (compute_panel — INSTRUMENTS domain, out of Phase 1b) | deferred |
| EC-EMBED-10 | cudarc fragmentation pre-flight | TASK-EMBED-007 | ready |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| EmbedError taxonomy with .code() + From&lt;InstrumentError&gt; | foundation | TASK-EMBED-005 | consumed by every other task |
| EmbedderInput / EmbedderOutput / RoutingResult | foundation | TASK-EMBED-003 | typed I/O contract |
| LoraAdapterRegistration / VramBudgetReport / CalibrationCertificate / EmbedderRegistration | foundation | TASK-EMBED-004 | persistence-bound data models with ed25519 sign/verify |

#### Uncovered items (cross-domain deferrals — explicitly noted)

- `mcp__cgreality__mejepa_compute_panel` — owned by INSTRUMENTS domain (Phase 1c task).
- `mcp__cgreality__mejepa_audit_pairwise_mi` — deferred to a later phase (out of Phase 1b scope).
- `mcp__cgreality__mejepa_lora_register` / `_refresh` / `_merge` — Phase 5 self-healing surface.
- `mcp__cgreality__mejepa_calibration_register` — Phase 7 surface.

These are NOT EMBEDDERS-domain gaps — they are surface tools owned by downstream domains that consume the EMBEDDERS APIs. Surface tasks for them will be tracked in their respective phase task indices.

**EMBEDDERS coverage: 12/12 functional REQs + 5/5 technical components + 10/10 per-embedder-group wrappers + 8/8 error states + 9/10 edge cases (1 cross-domain deferral) = 23 tasks; layer ordering Foundation (5) → Logic (17) → Surface (1) preserved; dependency DAG acyclic; no task references files from later tasks.**
### Domain: INSTRUMENTS

> **23 tasks generated** in `./phase1-instruments/`. Foundation: 4 (TASK-INSTR-001..004). Logic: 17 (TASK-INSTR-005..021). Surface: 2 (TASK-INSTR-022..023). Coverage: 100% across 23 REQ-INSTR-NN + 17 TECH-INSTRUMENTS components + 14 edge cases + 10 error states.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-INSTR-01 | EAstInstrument 384-d (Qodo + 1536→384 projection) | logic | TASK-INSTR-008 | blocked (Phase 1b embedders + AST-022) |
| REQ-INSTR-02 | ECfgInstrument 256-d (E8 + graph-attention) | logic | TASK-INSTR-009 | blocked |
| REQ-INSTR-03 | EDataFlowInstrument 256-d (E11 Kepler + graph-attn over def-use) | logic | TASK-INSTR-010 | blocked |
| REQ-INSTR-04 | ETypeGraphInstrument 256-d (E11 + toolchain lattice; fail-closed on missing toolchain) | logic | TASK-INSTR-011 | blocked |
| REQ-INSTR-05 | ETestInstrument 384-d (E1 e5-large-v2 + 1024→384) | logic | TASK-INSTR-012 | blocked (E1) |
| REQ-INSTR-06 | ETraceInstrument 512-d (E2/E3/E4 + sequence transformer) | logic | TASK-INSTR-013 | blocked |
| REQ-INSTR-07 | EDiffInstrument 256-d (Qodo + diff-edge encoder) | logic | TASK-INSTR-014 | blocked |
| REQ-INSTR-08 | EWitnessInstrument 256-d (verified 73-byte SHAKE-256 chain) | logic | TASK-INSTR-006 | ready (verify+extend; shipped) |
| REQ-INSTR-09 | EOracleInstrument 128-d (test outcomes + exception class one-hot) | logic | TASK-INSTR-005 | ready (verify+extend; shipped) |
| REQ-INSTR-10 | EProblemInstrument 1024-d (E1 native; SWE-bench problem text) | logic | TASK-INSTR-015 | blocked (E1) |
| REQ-INSTR-11 | ECommitMsgInstrument 384-d (E1 + 1024→384) | logic | TASK-INSTR-016 | blocked (E1) |
| REQ-INSTR-12 | EStaticAnalysisInstrument 256-d (linter/typecheck/source-shape) | logic | TASK-INSTR-007 | ready (verify+extend; shipped) |
| REQ-INSTR-13 | ERuntimeInstrument 256-d (walltime/RSS/syscalls/coverage Δ/exit codes/determinism flag) | logic | TASK-INSTR-017 | ready (verify+extend; shipped deterministic) |
| REQ-INSTR-14 | EReasoningInstrument 384-d (E17 AffectText; opaque latent BLEU&lt;5) | logic | TASK-INSTR-018 | blocked (E17) |
| REQ-INSTR-15 | Panel + PanelBuilder + PanelSerde fail-closed (5,120-d invariant) | foundation | TASK-INSTR-003 | ready (verify; shipped) |
| REQ-INSTR-16 | ScalarsInstrument 128-d (BFS/blame/churn/complexity/coverage Δ/repo-health) | logic | TASK-INSTR-019 | ready (verify+extend; shipped deterministic) |
| REQ-INSTR-17 | materialize_panel(time_step, ctx) helper (T0/T1/T2 rules per doc 09 §2.3) | logic | TASK-INSTR-020 | ready (after instrument tasks) |
| REQ-INSTR-18 | Frozen-instrument invariant + FrozenGuard (zero trainable params; ΔΞ.target_collapse=0) | foundation | TASK-INSTR-004 (asserted in TASK-INSTR-008..016, 018) | ready (net-new) |
| REQ-INSTR-19 | AST-perfect chunk consumption (no raw-text for code-domain instruments) | logic | TASK-INSTR-008 / 009 / 010 / 011 / 014 (E_AST/E_CFG/E_DataFlow/E_TypeGraph/E_Diff); TASK-INSTR-012 (E_Test on test-fn chunks) | blocked (AST-022 dep) |
| REQ-INSTR-20 | FSV harness 4 boundary cases (all_pass / mixed / crash_assertion_error / crash_type_error) | surface | TASK-INSTR-023 | blocked (after T020/T021/T022) |
| REQ-INSTR-21 | Linear-probe DoD (≥ 70% top-1 mutation-category) | logic | TASK-INSTR-021 | ready (after instrument tasks) |
| REQ-INSTR-22 | Panel materialization perf budgets (p50&lt;100 ms warm; p99&lt;500 ms cold) | logic / surface | TASK-INSTR-020 (latency), TASK-INSTR-023 (profile) | blocked |
| REQ-INSTR-23 | Provenance metadata in panel JSON (code_version, embedder_versions, frozen_at, corpus_sha) | surface | TASK-INSTR-022 | blocked (after T020) |

#### Technical components → Task mapping

| Component | Path | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| Panel + PanelBuilder + PanelSerde invariant gate | `crates/context-graph-mejepa-instruments/src/lib.rs` | foundation | TASK-INSTR-003 | ready (verify; shipped) |
| InstrumentSlot enum + layout() (15 slots, 5,120-d) | `crates/context-graph-mejepa-instruments/src/lib.rs` | foundation | TASK-INSTR-001 | ready (verify; shipped) |
| Instrument trait + InstrumentError taxonomy | `crates/context-graph-mejepa-instruments/src/lib.rs` | foundation | TASK-INSTR-002 | ready (verify; shipped) |
| FrozenGuard + ParameterRef + frozen_violation + MEJEPA_INSTR_FROZEN_VIOLATION | `crates/context-graph-mejepa-instruments/src/frozen_hook.rs` | foundation | TASK-INSTR-004 | ready (net-new) |
| EOracleInstrument | `crates/context-graph-mejepa-instruments/src/e_oracle.rs` | logic | TASK-INSTR-005 | ready (verify+extend; shipped) |
| EWitnessInstrument | `crates/context-graph-mejepa-instruments/src/e_witness.rs` | logic | TASK-INSTR-006 | ready (verify+extend; shipped) |
| EStaticAnalysisInstrument | `crates/context-graph-mejepa-instruments/src/e_static_analysis.rs` | logic | TASK-INSTR-007 | ready (verify+extend; shipped) |
| EAstInstrument (neural: Qodo + 1536→384) | `crates/context-graph-mejepa-instruments/src/e_code.rs` | logic | TASK-INSTR-008 | blocked |
| ECfgInstrument (neural: E8 + graph-attn) | `crates/context-graph-mejepa-instruments/src/e_code.rs` | logic | TASK-INSTR-009 | blocked |
| EDataFlowInstrument (neural: E11 + graph-attn) | `crates/context-graph-mejepa-instruments/src/e_code.rs` | logic | TASK-INSTR-010 | blocked |
| ETypeGraphInstrument (neural: E11 + toolchain lattice) | `crates/context-graph-mejepa-instruments/src/e_code.rs` + `toolchain_dispatch.rs` | logic | TASK-INSTR-011 | blocked |
| ETestInstrument (neural: E1 + 1024→384) | `crates/context-graph-mejepa-instruments/src/e_text.rs` | logic | TASK-INSTR-012 | blocked |
| ETraceInstrument (neural: sequence transformer) | `crates/context-graph-mejepa-instruments/src/e_trace.rs` | logic | TASK-INSTR-013 | blocked |
| EDiffInstrument (neural: Qodo + diff-edge encoder) | `crates/context-graph-mejepa-instruments/src/e_code.rs` | logic | TASK-INSTR-014 | blocked |
| EProblemInstrument (neural: E1 native 1024-d) | `crates/context-graph-mejepa-instruments/src/e_text.rs` | logic | TASK-INSTR-015 | blocked |
| ECommitMsgInstrument (neural: E1 + 1024→384) | `crates/context-graph-mejepa-instruments/src/e_text.rs` | logic | TASK-INSTR-016 | blocked |
| ERuntimeInstrument (deterministic) | `crates/context-graph-mejepa-instruments/src/e_runtime.rs` | logic | TASK-INSTR-017 | ready (verify+extend) |
| EReasoningInstrument (neural: E17 AffectText; opaque) | `crates/context-graph-mejepa-instruments/src/e_reasoning.rs` | logic | TASK-INSTR-018 | blocked |
| ScalarsInstrument (deterministic 128-d) | `crates/context-graph-mejepa-instruments/src/e_scalars.rs` | logic | TASK-INSTR-019 | ready (verify+extend) |
| materialize_panel(time_step, ctx) | `crates/context-graph-mejepa-instruments/src/materialize.rs` | logic | TASK-INSTR-020 | ready |
| LinearProbe + ProbeReport + 15 example bins | `crates/context-graph-mejepa-instruments/src/linear_probe.rs` + `examples/linear_probe_e_*.rs` | logic | TASK-INSTR-021 | ready |
| PanelJsonV1 + PanelProvenance + EmbedderPins + write_panel/read_panel + CF_MEJEPA_PANELS / CF_MEJEPA_PANEL_META | `crates/context-graph-mejepa-instruments/src/panel_json.rs` + `crates/context-graph-storage/src/cf.rs` | surface | TASK-INSTR-022 | blocked |
| phase1_extended_fsv harness (15 slots × 4 boundary cases + panel × 4 + latency profile) | `crates/context-graph-mejepa-instruments/examples/phase1_extended_fsv.rs` | surface | TASK-INSTR-023 | blocked |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-INSTR-01 | PanelBuilder builds with zero filled slots | TASK-INSTR-003 | ready |
| EC-INSTR-02 | Panel JSON filled_mask outside 15-slot mask | TASK-INSTR-003, TASK-INSTR-022 | ready / blocked |
| EC-INSTR-03 | EOracleInstrument total_test_count == 0 | TASK-INSTR-005 | ready |
| EC-INSTR-04 | EOracleInstrument total_test_count > 1e6 bounded | TASK-INSTR-005 | ready |
| EC-INSTR-05 | EWitnessInstrument empty chain_bytes | TASK-INSTR-006 | ready |
| EC-INSTR-06 | EWitnessInstrument format_version mismatch | TASK-INSTR-006 | ready |
| EC-INSTR-07 | E_TypeGraph Pyright vs mypy availability | TASK-INSTR-011 | blocked |
| EC-INSTR-08 | E_Trace events &gt; 1M | TASK-INSTR-013 | blocked |
| EC-INSTR-09 | AST chunker ERROR/MISSING node | TASK-INSTR-008 (relies on chunker fail-closed) | blocked |
| EC-INSTR-10 | Maintenance PR exposes requires_grad=true | TASK-INSTR-004 + TASK-INSTR-008..016, 018 (per-instrument FrozenGuard wiring) | ready / blocked |
| EC-INSTR-11 | Cold-start panel materialization &gt; 750 ms | TASK-INSTR-020 (latency profile) | ready |
| EC-INSTR-12 | Stale embedder_versions metadata | TASK-INSTR-022 | blocked |
| EC-INSTR-13 | E_Runtime walltime variance &gt; 30% | TASK-INSTR-017 | ready |
| EC-INSTR-14 | E_Reasoning prompt-injection payload (opaque latent) | TASK-INSTR-018 | blocked |

#### Error states → Task mapping

| Error state | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-INSTR-01 | MEJEPA_INSTRUMENTS_INVALID_INPUT (vector length != slot.dim) | TASK-INSTR-002, TASK-INSTR-003 | ready |
| ERR-INSTR-02 | MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT (NaN/Inf) | TASK-INSTR-002, TASK-INSTR-003 | ready |
| ERR-INSTR-03 | MEJEPA_INFER_INSTRUMENT_MISSING (E_TypeGraph toolchain absent) | TASK-INSTR-011 | blocked |
| ERR-INSTR-04 | MEJEPA_INSTRUMENTS_INVALID_INPUT (raw text fed to code-domain instrument) | TASK-INSTR-008..011, 014 (compile-time typed inputs) | blocked |
| ERR-INSTR-05 | MEJEPA_INSTR_FROZEN_VIOLATION (gradient through frozen instrument) | TASK-INSTR-004 + TASK-INSTR-008..016, 018 | ready / blocked |
| ERR-INSTR-06 | MEJEPA_INSTRUMENTS_INVALID_INPUT (witness verify_chain_bytes fails) | TASK-INSTR-006 | ready |
| ERR-INSTR-07 | MEJEPA_INSTRUMENTS_INVALID_INPUT (witness format_version mismatch) | TASK-INSTR-006 | ready |
| ERR-INSTR-08 | MEJEPA_INFER_DIM_MISMATCH (panel data length wrong) | TASK-INSTR-003, TASK-INSTR-022 | ready / blocked |
| ERR-INSTR-09 | MEJEPA_INSTRUMENTS_INVALID_INPUT (set_slot called twice) | TASK-INSTR-003 | ready |
| ERR-INSTR-10 | MEJEPA_INSTRUMENTS_INVALID_INPUT (panel JSON missing provenance) | TASK-INSTR-022 | blocked |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| InstrumentSlot::from_slug round-trip helper | foundation | TASK-INSTR-001 | additive to shipped slot enum |
| InstrumentError taxonomy with .code() | foundation | TASK-INSTR-002 | shipped; verify-only task |
| Panel/PanelBuilder/PanelSerde fail-closed gate | foundation | TASK-INSTR-003 | shipped; verify+extend with malicious-JSON regression tests |
| FrozenGuard + ParameterRef primitive | foundation | TASK-INSTR-004 | net-new; consumed by every neural-backed instrument |

#### Dependency notes

- **AST-CHUNKING (Phase 1a) cross-domain:** TASK-AST-022 (multi-language `AstChunk` API) is a hard prerequisite for the code-domain instruments (TASK-INSTR-008..011, 014). Until that ships, deterministic shipped fallback encoders behind the same trait keep callers working.
- **EMBEDDERS (Phase 1b) cross-domain:** TASK-EMBED-* provides E1, E7, E8, E11, E17 frozen forwards. Until shipped, neural-backed instruments emit `MEJEPA_INFER_INSTRUMENT_MISSING`.
- **CORPUS (Phase 0) cross-domain:** `OracleVerdict` (TASK-CORPUS-018), mutation corpus (TASK-CORPUS-023) are inputs for E_Oracle + linear-probe gate (TASK-INSTR-021).
- **WITNESS (shipped):** `verify_chain_bytes`, `WitnessEntry`, `WITNESS_ENTRY_SIZE`, `HASH_SIZE` are all shipped in `context-graph-witness`.
- **STORAGE:** `CF_MEJEPA_PANELS` + `CF_MEJEPA_PANEL_META` registration ships with TASK-INSTR-022 (touches `context-graph-storage`).

#### Uncovered items

(none — all 23 REQ-INSTR-NN, all 17 TECH-INSTRUMENTS component contracts (Panel/PanelBuilder/PanelSerde + InstrumentSlot + Instrument trait + FrozenGuard + 14 named instruments + materialize_panel + LinearProbe + PanelJsonV1 + FsvHarness), all 14 edge cases, and all 10 error states have at least one task; layer ordering Foundation (4) → Logic (17) → Surface (2) preserved; dependency DAG acyclic; no task references files from later tasks)

**INSTRUMENTS coverage: 23/23 functional REQs + 17/17 technical components + 14/14 edge cases + 10/10 error states = 23 tasks; layer ordering Foundation (4) → Logic (17) → Surface (2) preserved; critical path = TASK-INSTR-001 → TASK-INSTR-003 → TASK-INSTR-008 → TASK-INSTR-020 → TASK-INSTR-022 → TASK-INSTR-023.**
### Domain: PREDICTOR

> **12 tasks generated** in `./phase2-predictor/`. Foundation: 3 (TASK-PRED-001..003). Logic: 7 (TASK-PRED-004..010). Surface: 2 (TASK-PRED-011..012). Coverage: 100% across 18 REQ-PRED-NN + 6 TECH-PREDICTOR component contracts. New crate: `crates/context-graph-mejepa/` (created in TASK-PRED-001).

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-PRED-01 | (panel_t0, panel_t1) shape (B, 5120) → (B, 5120) dim contract | foundation/foundation/logic | TASK-PRED-001, TASK-PRED-002, TASK-PRED-005 | ready |
| REQ-PRED-02 | 6-layer transformer, hidden 1024, 8 heads, GeLU, residual+LN | foundation/logic | TASK-PRED-001, TASK-PRED-004 | ready |
| REQ-PRED-03 | Forward latency p50 < 50 ms at batch 64 RTX 5090 | logic/surface/surface | TASK-PRED-005, TASK-PRED-011, TASK-PRED-012 | ready |
| REQ-PRED-04 | MEJEPA_PRED_NAN_DETECTED on NaN/Inf at boundaries | foundation/logic/logic | TASK-PRED-003, TASK-PRED-004, TASK-PRED-005 | ready |
| REQ-PRED-05 | Eval-mode determinism (byte-equal SHA-256) | logic/surface | TASK-PRED-005, TASK-PRED-011 | ready |
| REQ-PRED-06 | VICReg L_total = L_predict + λ_var·L_var + λ_cov·L_cov + λ_inv·L_inv (defaults 25/1/25, γ=1; Huber δ=1) | foundation/logic | TASK-PRED-003, TASK-PRED-007 | ready |
| REQ-PRED-07 | LossOutputs four-term decomposition exposed as struct fields | foundation/logic | TASK-PRED-003, TASK-PRED-007 | ready |
| REQ-PRED-08 | VarianceFloorHistory + MEJEPA_PRED_VICREG_DEGENERATE diagnostic (50% dim threshold, 2-pass debounce) | foundation/logic | TASK-PRED-003, TASK-PRED-007 | ready |
| REQ-PRED-09 | FrozenTargetAdapter::parameters() returns &[]; trait API forbids &mut weight mutation | foundation/logic/logic | TASK-PRED-002, TASK-PRED-005, TASK-PRED-006 | ready |
| REQ-PRED-10 | Gradient hook: ‖dL/dθ_instrument‖ > 1e-12 → MEJEPA_PRED_FROZEN_TARGET_GRAD CRITICAL | foundation/logic | TASK-PRED-003, TASK-PRED-009 | ready |
| REQ-PRED-11 | FrozenTargetAdapter encode_target byte-equal across calls | logic/surface | TASK-PRED-006, TASK-PRED-011 | ready |
| REQ-PRED-12 | OracleHead Linear(5120, num_tests + 1) + softmax (per-test pass + exception probability) | foundation/logic | TASK-PRED-002, TASK-PRED-008 | ready |
| REQ-PRED-13 | VRAM steady-state ≤ 8 GB at batch 64 on RTX 5090 | logic/surface | TASK-PRED-010, TASK-PRED-012 | ready |
| REQ-PRED-14 | VRAM warn threshold 9 GB → MEJEPA_PRED_VRAM_EXCEEDED | logic | TASK-PRED-010 | ready |
| REQ-PRED-15 | Phase 2 DoD CLI exit 0 with FSV evidence written + re-read + asserted | surface/surface | TASK-PRED-011, TASK-PRED-012 | ready |
| REQ-PRED-16 | LossOutputs.l_predict public for SPEC-UTML compute_delta_p cross-spec contract | foundation/logic | TASK-PRED-003, TASK-PRED-007 | ready |
| REQ-PRED-17 | gradient_checkpointing config flag (consumer in MeJepaPredictor::forward) | foundation/logic | TASK-PRED-001, TASK-PRED-005 | ready |
| REQ-PRED-18 | Boundary validation (dim, finite, dtype) at every entry point | logic/logic/logic/logic | TASK-PRED-005, TASK-PRED-006, TASK-PRED-007, TASK-PRED-008 | ready |

#### Technical components → Task mapping

| Component | Path | Layer | Task ID | Status |
|---|---|---|---|---|
| MeJepaPredictor (struct + new + forward + trainable_parameters + run_gradient_hook + check_vram_steady_state + architecture_summary) | `predictor/mod.rs` | foundation/logic | TASK-PRED-001 (struct + new), TASK-PRED-005 (forward + trainable_parameters + architecture_summary), TASK-PRED-009 (run_gradient_hook delegation), TASK-PRED-010 (check_vram_steady_state delegation) | ready |
| TransformerLayer (new + forward + architecture) | `predictor/transformer_layer.rs` | foundation/logic | TASK-PRED-001 (struct + new), TASK-PRED-004 (forward + forward_with_layer_idx + architecture) | ready |
| OracleHead (new + predict_logits + predict_probabilities + trainable_parameters + num_outputs) | `oracle_head.rs` | foundation/logic | TASK-PRED-002 (struct + new + trainable_parameters), TASK-PRED-008 (predict_logits + predict_probabilities + num_outputs) | ready |
| FrozenTargetAdapter (new + parameters + encode_target + provenance + target_sha256 + with_grad_handles) | `frozen_target.rs` | logic | TASK-PRED-006 (full impl), TASK-PRED-009 (with_grad_handles additive constructor) | ready |
| VicregLoss (vicreg_loss + huber_loss_delta_one + 3 sub-functions + VarianceFloorHistory) | `loss/vicreg.rs` + `loss/variance_floor.rs` | logic | TASK-PRED-007 | ready |
| MejepaTrainBin / PredictorFsvHarness (CLI + FSV harness binaries) | `bin/mejepa.rs` + `examples/predictor_fsv.rs` | surface | TASK-PRED-011 (FSV harness), TASK-PRED-012 (DoD CLI) | ready |

#### Error states → Task mapping

| Error | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-PRED-01 | MEJEPA_PRED_DIM_MISMATCH | TASK-PRED-003 (variant), TASK-PRED-005 (forward), TASK-PRED-006 (encode_target), TASK-PRED-007 (vicreg_loss), TASK-PRED-008 (oracle_head) | ready |
| ERR-PRED-02 | MEJEPA_PRED_NAN_DETECTED | TASK-PRED-003 (variant + NanSource enum), TASK-PRED-004 (per-layer), TASK-PRED-005 (input + output), TASK-PRED-007 (loss component), TASK-PRED-008 (oracle output) | ready |
| ERR-PRED-03 | MEJEPA_PRED_VRAM_EXCEEDED | TASK-PRED-003 (variant), TASK-PRED-010 (check_vram_steady_state) | ready |
| ERR-PRED-04 | MEJEPA_PRED_FROZEN_TARGET_GRAD (CRITICAL) | TASK-PRED-003 (variant + is_critical), TASK-PRED-009 (gradient hook detection) | ready |
| ERR-PRED-05 | MEJEPA_PRED_VICREG_DEGENERATE | TASK-PRED-003 (variant), TASK-PRED-007 (VarianceFloorHistory) | ready |
| ERR-PRED-06 | MEJEPA_PRED_NAN_DETECTED (input variant) | TASK-PRED-005 (forward input), TASK-PRED-008 (oracle input) | ready |
| (additional) | MEJEPA_PRED_CONFIG_INVALID | TASK-PRED-001 (validate), TASK-PRED-002 (num_tests bounds) | ready |
| (additional) | MEJEPA_PRED_DEVICE_UNAVAILABLE | TASK-PRED-001 (new), TASK-PRED-010 (vram cpu refusal) | ready |
| (additional) | MEJEPA_PRED_BATCH_TOO_SMALL (LossError) | TASK-PRED-007 (vicreg batch=1 warn-only) | ready |
| (additional) | MEJEPA_PRED_NON_FINITE_LAMBDA (LossError) | TASK-PRED-003 (variant), TASK-PRED-007 (vicreg lambdas validation) | ready |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-PRED-01 | Inconsistent batch dims between panel_t0 and panel_t1 | TASK-PRED-005 | ready |
| EC-PRED-02 | Structural-zero pattern panels handled normally | TASK-PRED-005 | ready (no special-casing) |
| EC-PRED-03 | First-forward Inf in scaled-dot-product attention | TASK-PRED-004 (per-layer NaN detection) | ready |
| EC-PRED-04 | Detach-then-reattach escape hatch | TASK-PRED-006 (explicit detach), TASK-PRED-009 (gradient hook safety net) | ready |
| EC-PRED-05 | Batch=1 in covariance term (degenerate) | TASK-PRED-007 (warn + l_covariance=0) | ready |
| EC-PRED-06 | Specific subset dims persistently low-variance | TASK-PRED-007 (50% threshold debounce avoids false alarms) | ready |
| EC-PRED-07 | num_tests differs across tasks (size to corpus-max) | TASK-PRED-002 (num_tests bounds 1..=10000), TASK-PRED-008 (per-instance subsetting deferred to Phase 4) | ready |
| EC-PRED-08 | sm_120a unavailable (CPU-only host) | TASK-PRED-001 (new() refusal), TASK-PRED-010 (vram refusal), TASK-PRED-012 (CLI EX_USAGE) | ready |
| EC-PRED-09 | Determinism violation due to lingering training-mode dropout | TASK-PRED-005 (eval mode), TASK-PRED-011 (FSV determinism check) | ready |
| EC-PRED-10 | 12-layer + batch-128 extension via gradient_checkpointing | TASK-PRED-001 (config flag), TASK-PRED-005 (consumer; falls back gracefully if candle 0.9.x lacks API) | ready (nice-to-have) |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| context-graph-mejepa crate scaffolding (Cargo.toml + lib.rs + module placeholders) | foundation | TASK-PRED-001 | new crate; workspace member registration |
| PredictorError taxonomy (8 variants) + LossError (5 variants) + From<LossError> | foundation | TASK-PRED-003 | consumed by every other task in this phase |
| Constants block (PANEL_DIM, CONCAT_INPUT_DIM, DEFAULT_*, VRAM_*, GRAD_NORM_NOISE_FLOOR, VICREG_*) | foundation | TASK-PRED-001 | re-exported via lib.rs |
| FSV writer helpers (ensure_fsv_dir + write_evidence_file mode 0o600 + read_evidence_file + clear_fsv_dir) | surface | TASK-PRED-011 | reused by DoD CLI (TASK-PRED-012) |

#### Cross-spec contracts (load-bearing)

| Contract | Direction | Task ID(s) | Notes |
|---|---|---|---|
| LossOutputs.l_predict (public field) | PREDICTOR → UTML | TASK-PRED-003 | SPEC-UTML compute_delta_p reads this field; visibility change is a breaking spec change |
| FrozenTargetAdapter::parameters() == &[] | PREDICTOR ↔ Constitution `<utml_v1_invariants>` rule 2 | TASK-PRED-006 | type-level guarantee of ΔΞ.target_collapse = 0 by construction |
| `Instrument` trait API forbids &mut self weight mutation | PREDICTOR ← INSTRUMENTS | (verified by Phase 1c TASK-INSTR-002; this phase relies on the contract) | upstream invariant; no edits to instruments crate |
| PANEL_DIM = 5_120 | PREDICTOR ↔ INSTRUMENTS | TASK-PRED-001 (re-export), TASK-PRED-006 (compile-time cross-check) | const-asserted equal across crates |
| Phase 3 mejepa-train consumes vicreg_loss + LossOutputs + run_gradient_hook + trainable_parameters | PREDICTOR → TRAINING | TASK-PRED-007, TASK-PRED-009, TASK-PRED-005 | downstream Phase 3 spec depends on these stable APIs |
| Phase 4 mejepa.verify() consumes predictor.forward + oracle_head.predict_probabilities | PREDICTOR → INFERENCE | TASK-PRED-005, TASK-PRED-008 | inference path stable |

#### Uncovered items

(none — all 18 REQ-PRED-NN, all 6 TECH-PREDICTOR component contracts, all 6 error states + 4 LossError-side codes, all 10 edge cases have at least one task; layer ordering Foundation (3) → Logic (7) → Surface (2) preserved; dependency DAG acyclic; no task references files from later tasks per Iron Rule)

**PREDICTOR coverage: 18/18 functional REQs + 6/6 technical components + 10/10 error/loss codes + 10/10 edge cases = 12 tasks; layer ordering Foundation (3) → Logic (7) → Surface (2); critical path TASK-PRED-001 → 003 → 004 → 005 → 007 → 011 → 012 (~7-10 days end-to-end per master plan).**
### Domain: TRAINING

> **14 tasks generated** in `./phase3-training/`. Foundation: 2 (TASK-TRAIN-001..002). Logic: 9 (TASK-TRAIN-003..011), Orchestrator: 1 (TASK-TRAIN-012). Surface: 2 (TASK-TRAIN-013..014). Coverage: 100% across 12 REQ-TRAIN-NN + 7 TECH-TRAINING component contracts. Cross-phase dependency: every task imports from `../utml-learning-formula/`.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-TRAIN-01 | AdamW + warmup + cosine + clipping + decoupled WD + bf16/fp32 | logic | TASK-TRAIN-003 | ready |
| REQ-TRAIN-02 | Multi-objective L_full (13 weighted terms) | logic | TASK-TRAIN-004 | ready |
| REQ-TRAIN-03 | Per-step training cert + parent_hash → self_hash chain | logic | TASK-TRAIN-009 | ready |
| REQ-TRAIN-04 | BatchSampler with sampling_weight gates | logic | TASK-TRAIN-005 | ready |
| REQ-TRAIN-05 | Holdout evaluation cadence (every 1000 steps) + 7-field report | logic | TASK-TRAIN-011 | ready |
| REQ-TRAIN-06 | Counterfactual training (K=4 perturbations) | logic | TASK-TRAIN-006 | ready |
| REQ-TRAIN-07 | Cross-task transfer (p=0.3 cosine&gt;0.7 neighborhood) | logic | TASK-TRAIN-007 | ready |
| REQ-TRAIN-08 | Aux-head distillation cycle every 1000 steps | logic | TASK-TRAIN-008 | ready |
| REQ-TRAIN-09 | Per-epoch SHAKE-256-73 witness chain entry + ed25519 | logic | TASK-TRAIN-010 | ready |
| REQ-TRAIN-10 | Atomic checkpoint every 2000 steps + --resume | logic / orchestrator | TASK-TRAIN-012 (Checkpointer integration) | ready |
| REQ-TRAIN-11 | LoRA adapters (rank=32, alpha=16, dropout=0.05) default; --full-finetune escape hatch | logic / orchestrator | TASK-TRAIN-012 (LoraWiring) | ready |
| REQ-TRAIN-12 | Fail-closed on 6 documented training failure modes | logic | TASK-TRAIN-003 (NaN/grad explode), TASK-TRAIN-009 (chain broken / instrument leak), TASK-TRAIN-011 (regression), TASK-TRAIN-012 (VRAM OOM, checkpoint corrupt orchestration) | ready |

#### Cross-cutting foundation tasks

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| Trainer + BatchSampler + TrainCertWriter + HoldoutEvaluator + EpochWitnessChain + EpochReport + HoldoutReport + EpochSummary + EpochWitnessEntry data models | foundation | TASK-TRAIN-001 | struct skeleton; methods filled in later layers |
| TrainerError + TrainerErrorCode (10 variants) + LossCoefficients + TrainingConfig (25 fields with defaults + TOML loader + validate) + MutationCategory + Lang + extended SkipReason (8 variants) | foundation | TASK-TRAIN-002 | error taxonomy + config; consumed by every later task |

#### Surface tasks

| Tech component | Task ID | Notes |
|---|---|---|
| CLI: `mejepa-train train` + `verify-chain` + `eval-holdout` subcommands; exit codes 0/1/2/3 | TASK-TRAIN-013 | clap-based; CLI &gt; TOML &gt; Default precedence |
| FSV: training-cert-chain.json (100k-step replay &lt; 60s) + training-smoke-evidence.json (100-step) + holdout-eval-evidence.json + cross-check utml-l-step-evidence.json (mode 0o600) | TASK-TRAIN-014 | per `<fsv_protocol>`; integrates UTML-010 evidence |

#### Cross-spec contracts (load-bearing)

| Contract | Direction | Task ID(s) | Notes |
|---|---|---|---|
| TrainingCertificate.signal: LearningSignal (UTML type) | TRAINING ← UTML | TASK-TRAIN-001 / TASK-UTML-001 | per-step cert embeds full LearningSignal |
| compose_l_full reuses VICReg primitives from `embeddings/training/loss.rs` | TRAINING → embeddings | TASK-TRAIN-004 | no rebuild; reuse-not-rebuild |
| AdamW reuses `embeddings/training/optimizer.rs` | TRAINING → embeddings | TASK-TRAIN-003 | reuse-not-rebuild |
| Distillation reuses `embeddings/training/distillation.rs` quality_gate | TRAINING → embeddings | TASK-TRAIN-008 | reuse-not-rebuild |
| LoRA reuses `embeddings/training/lora.rs::LoRAAdapter` | TRAINING → embeddings | TASK-TRAIN-012 | reuse-not-rebuild |
| Stoer-Wagner mincut reuses `context-graph-mincut` | TRAINING → mincut | TASK-TRAIN-011 | reuse-not-rebuild |
| Per-epoch SHAKE-256-73 reuses `context-graph-witness` | TRAINING → witness | TASK-TRAIN-010 | reuse-not-rebuild |
| target_collapse==0.0 cert invariant | TRAINING → constitution | TASK-TRAIN-009 (writer-side gate), TASK-UTML-006 (compute-side assert) | dual gate ensures architectural integrity |
| Phase 4 inference consumes best.safetensors + calibration_threshold | TRAINING → INFERENCE | TASK-TRAIN-011 (promotion), TASK-TRAIN-012 (atomic write+rename) | downstream API stable |
| Phase 5 self-healing consumes UtmlClassifier verdict + holdout regression alerts | TRAINING → SELF-HEALING | TASK-TRAIN-011 (alerts), TASK-UTML-009 (classifier) | per UTML §IX coaching order |
| Phase 7 shift-subscriber consumes per-step cert chain | TRAINING → SHIFT-SUBSCRIBER | TASK-TRAIN-009 | live observe() subscribes to cert stream |

#### Uncovered items

(none — all 12 REQ-TRAIN-NN, all 7 TECH-TRAINING component contracts, all 6 error codes + DOD failure code + EmbedderDigestMismatch, all 15 edge cases EC-TRAIN-01..15, all 7 error states ERR-TRAIN-01..07 have at least one task; layer ordering Foundation (2) → Logic (9) → Surface (2) preserved; dependency DAG acyclic; no task references files from later tasks per Iron Rule. Cross-phase ordering: every TRAINING task explicitly depends_on its UTML prerequisite via `<depends_on>` in the task spec.)

**TRAINING coverage: 12/12 functional REQs + 7/7 technical components + 10/10 error codes + 15/15 edge cases = 14 tasks; layer ordering Foundation (2) → Logic (9) → Surface (2); critical path UTML-001 → UTML-006 → UTML-007 → TASK-TRAIN-001 → TASK-TRAIN-009 → TASK-TRAIN-012 → TASK-TRAIN-013 → TASK-TRAIN-014 (~7 days end-to-end per master plan Phase 3 budget).**

### Domain: UTML

> **10 tasks generated** in `./utml-learning-formula/`. Foundation: 2 (TASK-UTML-001..002). Logic: 7 (TASK-UTML-003..009). Surface: 1 (TASK-UTML-010). Coverage: 100% across 12 REQ-UTML-NN + 11 TECH-UTML component contracts. Crate touched: `context-graph-mejepa-train` (shared with TRAINING phase under `src/learning_signal/` + `src/cert/`).

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-UTML-01 | L_step = ΔP × ΔK × ΔΩ × ΔΞ multiplicative; clamped [0, 1] | logic | TASK-UTML-007 | ready |
| REQ-UTML-02 | compute_delta_p (cosine error + SNR norm + exploration + γ + per-chunk aggregation) | logic | TASK-UTML-003 | ready |
| REQ-UTML-03 | compute_delta_k (cos_align + fisher_violation + ECE + embedder_coherence) | logic | TASK-UTML-004, TASK-UTML-008 (embedder_coherence) | ready |
| REQ-UTML-04 | compute_delta_omega (plasticity^0.4 · landscape^0.4 · stability^0.1 · agent_state^0.1) | logic | TASK-UTML-005 | ready |
| REQ-UTML-05 | compute_delta_xi (target_collapse asserted == 0; predictor_redundancy; constellation_violation_rate) | logic | TASK-UTML-006 | ready |
| REQ-UTML-06 | sampling_weight = base · L_step · (1+5·override) · 0.995^age; drop&lt;0.05; force&gt;1.0 | logic | TASK-UTML-007 | ready |
| REQ-UTML-07 | Per-step training certificate + hash chain (cross-spec; cert struct here, chain logic in TASK-TRAIN-009) | foundation / logic | TASK-UTML-001 (struct), TASK-TRAIN-009 (emit + chain) | ready |
| REQ-UTML-08 | UTML cycle classifier (7 states; coaching-order priority) | logic | TASK-UTML-009 | ready |
| REQ-UTML-09 | calibrated_confidence = clamp_[0.10, 0.95](raw · ΔΩ_mean · ΔΞ_mean ...) + Reward = 0.4·conf + 0.3·mean(L_step·ΔΞ) + 0.3·gtau | logic (Phase 4 consumer) | (Phase 4 INFERENCE; UTML supplies inputs only) — covered by TASK-UTML-007 (L_step) + TASK-UTML-009 (mean_omega/mean_xi over window) | ready (UTML side) |
| REQ-UTML-10 | M(t) consolidation integral + Dissipating detector | logic | TASK-UTML-009 (consolidation submodule) | ready |
| REQ-UTML-11 | T(train, deploy) divergence + performance_deploy | logic | TASK-UTML-009 (state_transfer submodule) | ready |
| REQ-UTML-12 | Pruning rule: 3-condition agreement (L_step low N steps AND ΔΩ&gt;0.4 AND ΔΞ&gt;0.4) | logic | TASK-UTML-009 (prune_decision) | ready |

#### TECH-UTML component → Task mapping

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| UtmlError + UtmlErrorCode (8 variants) | foundation | TASK-UTML-001 | error taxonomy |
| LearningSignal + DeltaPComponents + DeltaKComponents + DeltaOmegaComponents + DeltaXiComponents | foundation | TASK-UTML-001 | data carriers |
| TrainingCertificate (UTML-side struct) | foundation | TASK-UTML-001 | extended in TASK-TRAIN-009 with Phase-3 fields |
| SkipReason (6 → 8 variants extended in TRAIN-002) + UtmlState + Intervention + ClassifierVerdict + DeltaPAggregator | foundation | TASK-UTML-002 | enum carriers |
| DeltaP (compute_delta_p + compute_delta_p_aggregate + select_aggregator) | logic | TASK-UTML-003 | per-chunk aggregator from doc 14 |
| DeltaK (compute_delta_k + update_grad_ema + FisherMatrix trait) | logic | TASK-UTML-004 | EWC++ trait abstraction |
| DeltaOmega (compute_delta_omega + 4 sub-functions: effective_plasticity, landscape_health, stability_floor, agent_state_score) | logic | TASK-UTML-005 | discrete stability_floor + 0.7 default for E17-unavailable |
| DeltaXi (compute_delta_xi with `target_collapse != 0.0` fail-closed) | logic | TASK-UTML-006 | central architectural integrity check |
| LStep + SamplingWeight (compute_l_step + sampling_weight + should_drop + should_force_include) | logic | TASK-UTML-007 | hot-path with debug-only validation |
| EmbedderCoherence (cross-embedder Jaccard) + PairwiseMI audit (CSV writer for redundancy) | logic | TASK-UTML-008 | bootstrap-neutral 0.5 for &lt; 1000 corpus entries |
| UtmlClassifier (7-state) + prune_decision + Consolidation (M(t)) + StateTransfer (T) + ContinualBackprop (dormant unit re-init) | logic | TASK-UTML-009 | coaching-order precedence |

#### Cross-cutting foundation tasks

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| context-graph-mejepa-train crate scaffolding (Cargo.toml + lib.rs + module placeholders) + workspace member registration | foundation | TASK-UTML-001 | new crate; shared with TRAINING phase |

#### Surface tasks

| Tech component | Task ID | Notes |
|---|---|---|
| FSV: utml-l-step-evidence.json (100 synthetic examples spanning 7 corner-case categories) + utml-classifier-evidence.json (5 regimes) + utml-sampling-histogram.json + pairwise_mi_matrix.csv (mode 0o600) | TASK-UTML-010 | per doc 11 §11.1 |

#### Cross-spec contracts (load-bearing)

| Contract | Direction | Task ID(s) | Notes |
|---|---|---|---|
| `target_collapse == 0.0` runtime assert | UTML → constitution `<utml_v1_invariants>` rule 2 | TASK-UTML-006 | central architectural integrity check |
| LearningSignal struct shape consumed by TrainingCertificate | UTML → TRAINING | TASK-UTML-001, TASK-TRAIN-001 | shared crate |
| compute_l_step + sampling_weight consumed by BatchSampler | UTML → TRAINING | TASK-UTML-007, TASK-TRAIN-005 | drop/force gate constants shared |
| ClassifierVerdict consumed by Phase 5 SelfHealing | UTML → SELF-HEALING | TASK-UTML-009 | coaching-order recommendations |
| ΔΩ_mean × ΔΞ_mean consumed by Phase 4 verify() calibrated_confidence | UTML → INFERENCE | TASK-UTML-009 | rolling window mean computed by classifier consumer |
| pairwise_mi_audit produces predictor_redundancy for ΔΞ | UTML → ΔΞ self | TASK-UTML-008 → TASK-UTML-006 | full circle within UTML |
| FisherMatrix trait stub awaiting Phase 5 EWC++ implementation | UTML → SELF-HEALING | TASK-UTML-004 | trait-level abstraction |
| EmbedderCoherence trait awaiting Phase 1c neural backends | UTML → INSTRUMENTS / EMBEDDERS | TASK-UTML-008 | bootstrap-neutral fallback for first 1000 corpus entries |

#### Uncovered items

(none — all 12 REQ-UTML-NN, all 11 TECH-UTML component contracts, all 8 UtmlErrorCode variants, all 7 ERR-UTML-NN error states, all 15 edge cases EC-UTML-01..15 have at least one task; layer ordering Foundation (2) → Logic (7) → Surface (1) preserved; dependency DAG acyclic; no task references files from later tasks per Iron Rule.)

**UTML coverage: 12/12 functional REQs + 11/11 technical components + 8/8 UtmlErrorCode variants + 15/15 edge cases = 10 tasks; layer ordering Foundation (2) → Logic (7) → Surface (1); critical path TASK-UTML-001 → TASK-UTML-006 → TASK-UTML-007 → TASK-UTML-009 → TASK-UTML-010 (~5 days end-to-end).**

### Domain: INFERENCE

> **14 tasks generated** in `./phase4-inference/`. Foundation: 2 (TASK-INFER-001..002). Logic: 8 (TASK-INFER-003..010). Surface: 4 (TASK-INFER-011..014). Coverage: 100% across 20 REQ-INFER-NN + 4 TECH-INFERENCE component contracts. Crate created: `context-graph-mejepa/`.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-INFER-01 | Public methods compile/verify/observe signatures | logic | TASK-INFER-003 (compile), TASK-INFER-004 (verify), TASK-INFER-003 (observe stub) | blocked |
| REQ-INFER-02 | RealityPrediction shape with all 11 fields + try_new validation | foundation | TASK-INFER-001 | ready |
| REQ-INFER-03 | VerifyVerdict {Approve, EscalateToHuman} | foundation | TASK-INFER-001 | ready |
| REQ-INFER-04 | FailedGate 7 variants | foundation | TASK-INFER-001 | ready |
| REQ-INFER-05 | PatchBundle + TaskContext + ClaimGraph + SkillCitation | foundation | TASK-INFER-001 | ready |
| REQ-INFER-06 | Conformal calibration (non-conformity + (1-α) quantile τ) | logic | TASK-INFER-005, TASK-INFER-010 | blocked |
| REQ-INFER-07 | OOD detection formula + σ² tuning + AUC&gt;0.85 | logic | TASK-INFER-006 | blocked |
| REQ-INFER-08 | ΔΩ_mean × ΔΞ_mean multiplicative gating; degraded_status | logic | TASK-INFER-008 | blocked |
| REQ-INFER-09 | Gate 1 OutOfDistribution; OodRefuse → EscalateToHuman conversion | logic | TASK-INFER-006, TASK-INFER-004 | blocked |
| REQ-INFER-10 | Gate 2 PredictedFailure (predicted_oracle_pass &gt; 0.95) | logic | TASK-INFER-003, TASK-INFER-004 | blocked |
| REQ-INFER-11 | Gate 3 LowConfidence (outcome_set.len() ≤ 2; entropy) | logic | TASK-INFER-007, TASK-INFER-004 | blocked |
| REQ-INFER-12 | Gate 4 GrangerRejection (p ≤ p_threshold via context-graph-solver/granger.rs) | logic | TASK-INFER-003, TASK-INFER-004 | blocked |
| REQ-INFER-13 | Gate 5 WitnessChainBroken (replay segment) | logic | TASK-INFER-009 | ready |
| REQ-INFER-14 | Gate 6 SourceShaDrift (canonical paths; SHA pinning) | logic | TASK-INFER-009 | ready |
| REQ-INFER-15 | Gate 7 ConstellationGuardRejected (cross-cut TCT) | logic | TASK-INFER-004 (mock until TCT ships) | blocked |
| REQ-INFER-16 | mcp__cgreality__mejepa_verify (with include_provenance) | surface | TASK-INFER-011 | blocked |
| REQ-INFER-17 | Fail-closed error taxonomy + .code() + log_context (10 variants) | foundation | TASK-INFER-002 | ready |
| REQ-INFER-18 | Performance budgets (p50&lt;100ms, p99&lt;500ms, AUC&gt;0.85, coverage±2%) | surface | TASK-INFER-013, TASK-INFER-014 | blocked |
| REQ-INFER-19 | CF_MEJEPA_LIVE_PREDICTIONS persist + mejepa_predict_latest | logic / surface | TASK-INFER-004 (write), TASK-INFER-012 (read) | blocked |
| REQ-INFER-20 | Phase 4 DoD command `cargo run --release -p context-graph-mejepa -- infer-test` | surface | TASK-INFER-013 | blocked |

#### TECH-INFERENCE component → Task mapping

| Tech component | Path | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| MeJepaCompiler (compile + verify + observe-stub + 4 internal helpers) | `crates/context-graph-mejepa/src/compiler.rs` | logic | TASK-INFER-003, TASK-INFER-004 | blocked |
| CalibrationStore (calibrate + load_active + recompute_sliding_window) | `crates/context-graph-mejepa/src/calibration.rs` | logic | TASK-INFER-010 | ready |
| InstrumentCache (LruCache(SlotKey → Arc&lt;Vec&lt;f32&gt;&gt;) cap 4_096; singleflight) | `crates/context-graph-mejepa/src/cache.rs` | foundation | TASK-INFER-002 | ready |
| MejepaInferError + .code() + log_context (10 variants) | `crates/context-graph-mejepa/src/error.rs` | foundation | TASK-INFER-002 | ready |

#### FailedGate variant → Task mapping (one per gate)

| FailedGate variant | Constitution code | Task ID(s) | Status |
|---|---|---|---|
| OutOfDistribution { score } | MEJEPA_INFER_OOD_REFUSE | TASK-INFER-001 (variant), TASK-INFER-006 (logic), TASK-INFER-004 (gate) | blocked |
| LowConfidence { entropy } | (gate failure) | TASK-INFER-001 (variant), TASK-INFER-007 (entropy), TASK-INFER-004 (gate) | blocked |
| PredictedFailure { failing_tests } | (gate failure) | TASK-INFER-001 (variant), TASK-INFER-003 (logic), TASK-INFER-004 (gate) | blocked |
| GrangerRejection { skill, p_value } | (gate failure) | TASK-INFER-001 (variant), TASK-INFER-003 (run), TASK-INFER-004 (gate) | blocked |
| WitnessChainBroken | MEJEPA_INFER_WITNESS_CHAIN_BROKEN | TASK-INFER-001 (variant), TASK-INFER-009 (replay), TASK-INFER-004 (gate) | ready |
| SourceShaDrift { path } | MEJEPA_INFER_SOURCE_SHA_DRIFT | TASK-INFER-001 (variant), TASK-INFER-009 (canonicalize+SHA), TASK-INFER-004 (gate) | ready |
| ConstellationGuardRejected { violating_embedders, violating_chunks } | MEJEPA_INFER_CONSTELLATION_VIOLATION | TASK-INFER-001 (variant), TASK-INFER-004 (gate; mocked until TCT ships) | blocked |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-INFER-01 | NaN in RealityPrediction f32 fields → NanDetected | TASK-INFER-001, TASK-INFER-002 | ready |
| EC-INFER-02 | Path traversal `../etc/passwd` → SourceShaDrift | TASK-INFER-009, TASK-INFER-011 | ready / blocked |
| EC-INFER-03 | Insufficient calibration samples per language | TASK-INFER-005, TASK-INFER-010 | ready |
| EC-INFER-04 | OOD score == 0.5 boundary → no refusal (strict &gt;) | TASK-INFER-006 | blocked |
| EC-INFER-05 | Empty claim_graph → empty Granger attestations | TASK-INFER-003 | blocked |
| EC-INFER-06 | Witness segment len % 73 != 0 → WitnessChainBroken | TASK-INFER-009 | ready |
| EC-INFER-07 | Missing file in ast_diff → SourceShaDrift (not generic IO) | TASK-INFER-009 | ready |
| EC-INFER-08 | TCT shrinkage cells with N&lt;50 — passive consumer | TASK-INFER-004 (cross-ref TASK-TCT-004) | blocked |
| EC-INFER-09 | Empty CF_MEJEPA_TRAIN_CERTS → bootstrap defaults + degraded_status | TASK-INFER-008 | blocked |
| EC-INFER-10 | include_provenance=true with unloadable components → Option::None + provenance_warnings | TASK-INFER-011 | blocked |

#### Error states → Task mapping

| Error state | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-INFER-01 | MEJEPA_INFER_OOD_REFUSE | TASK-INFER-002, TASK-INFER-006 | ready / blocked |
| ERR-INFER-02 | MEJEPA_INFER_DIM_MISMATCH | TASK-INFER-002 | ready |
| ERR-INFER-03 | MEJEPA_INFER_INSTRUMENT_MISSING | TASK-INFER-002, TASK-INFER-003 | blocked |
| ERR-INFER-04 | MEJEPA_INFER_NAN_DETECTED | TASK-INFER-001, TASK-INFER-002 | ready |
| ERR-INFER-05 | MEJEPA_INFER_CALIBRATION_STALE | TASK-INFER-002, TASK-INFER-010 | ready |
| ERR-INFER-06 | MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES | TASK-INFER-002, TASK-INFER-005 | ready |
| ERR-INFER-07 | MEJEPA_INFER_WITNESS_CHAIN_BROKEN | TASK-INFER-002, TASK-INFER-009 | ready |
| ERR-INFER-08 | MEJEPA_INFER_SOURCE_SHA_DRIFT | TASK-INFER-002, TASK-INFER-009 | ready |
| ERR-INFER-09 | MEJEPA_INFER_GTAU_STALE_CONSTELLATION | TASK-INFER-002 (variant), TASK-INFER-004 (surface; cross-cut TASK-TCT-009) | blocked |
| ERR-INFER-10 | CCREALITY_ENGINE_RETIRED (retired tool aliases stay fail-closed) | TASK-INFER-011 (verifies fail-closed; does NOT add shim) | blocked |

#### Cross-spec contracts (load-bearing)

| Contract | Direction | Task ID(s) | Notes |
|---|---|---|---|
| RealityPrediction.confidence consumed by Phase 5 self-healing rolling window | INFERENCE → SELF-HEALING | TASK-INFER-001, TASK-INFER-012 | rolling-window empirical-coverage source |
| ΔΩ_mean / ΔΞ_mean read from CF_MEJEPA_TRAIN_CERTS | TRAINING → INFERENCE | TASK-INFER-008 | bootstrap defaults when empty |
| ConstellationGuardRejected variant payload definition | TCT → INFERENCE | TASK-TCT-002 (variant), TASK-INFER-001 (FailedGate variant), TASK-INFER-004 (gate consumer) | Phase 4a uses MockedAlwaysApprove until TCT ships |
| Granger F-test consumed at Gate 4 | SOLVER → INFERENCE | TASK-INFER-003 | shipped via context-graph-solver/granger.rs |
| Witness chain replay at Gate 5 | WITNESS → INFERENCE | TASK-INFER-009 | shipped context-graph-witness verify_chain_bytes |
| Calibration record provenance check | EMBEDDERS → INFERENCE | TASK-INFER-002, TASK-INFER-010 | embedder_versions BTreeMap |

#### Uncovered items

(none — all 20 REQ-INFER-NN, all 4 TECH-INFERENCE component contracts, all 7 FailedGate variants, all 10 EC-INFER-NN edge cases, all 10 error states have at least one task; layer ordering Foundation (2) → Logic (8) → Surface (4) preserved; dependency DAG acyclic; no task references files from later tasks per Iron Rule.)

**INFERENCE coverage: 20/20 functional REQs + 4/4 technical components + 7/7 FailedGate variants + 10/10 edge cases + 10/10 error states = 14 tasks; layer ordering Foundation (2) → Logic (8) → Surface (4); critical path TASK-INFER-001 → TASK-INFER-002 → TASK-INFER-010 → TASK-INFER-005 → TASK-INFER-003 → TASK-INFER-004 → TASK-INFER-011 → TASK-INFER-014 (~3-5 days end-to-end after Phase 2/3 land).**

### Domain: TCT

> **12 tasks generated** in `./phase4-tct-constellation/`. Foundation: 2 (TASK-TCT-001..002). Logic: 8 (TASK-TCT-003..010). Surface: 2 (TASK-TCT-011..012). Coverage: 100% across 15 REQ-TCT-NN + 7 TECH-TCT component contracts. Crate created: `context-graph-mejepa-tct/`.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-TCT-01 | TctConstellation data type with all centroid maps + thresholds + provenance + version_id | foundation | TASK-TCT-001 | ready |
| REQ-TCT-02 | ConstellationBuilder per-cell mean + L2 normalize + provenance inputs | logic | TASK-TCT-003 | blocked |
| REQ-TCT-03 | Bayesian shrinkage on N&lt;50; fail-closed N&lt;5 | logic | TASK-TCT-004 | blocked |
| REQ-TCT-04 | ThresholdCalibrator FPR ≤ 5% per (embedder, language) cell with ≥ 30 samples | logic | TASK-TCT-005 | blocked |
| REQ-TCT-05 | CF_MEJEPA_CONSTELLATION FROZEN persist | logic | TASK-TCT-008 | blocked |
| REQ-TCT-06 | lookup_centroid with shrinkage fallback hierarchy (OwnCell → Language → Entity → Category) | logic | TASK-TCT-004 | blocked |
| REQ-TCT-07 | constellation_hinge primitive: Σ_m max(0, τ_m − cos); centroid + embedder DETACHED | logic | TASK-TCT-007 | blocked |
| REQ-TCT-08 | gtau_check (panel-level) deterministic O(N · d_max) | logic | TASK-TCT-006 | blocked |
| REQ-TCT-09 | gtau_check_chunks (per-chunk localization; sharper per-chunk-type τ when N≥50) | logic | TASK-TCT-006 | blocked |
| REQ-TCT-10 | Verdict::GuardRejected variant; CF_MEJEPA_GUARD_DECISIONS persistence | foundation / logic | TASK-TCT-002 (variant), TASK-TCT-006 (emission), TASK-TCT-010 (CF) | blocked |
| REQ-TCT-11 | Max-age policy (default 90 days); operator override audit-logged → MEJEPA_INFER_GTAU_STALE_CONSTELLATION | logic | TASK-TCT-009 | blocked |
| REQ-TCT-12 | Provenance mismatch fail-closed at load → MEJEPA_TCT_PROVENANCE_MISMATCH | logic | TASK-TCT-008 | blocked |
| REQ-TCT-13 | constellation_violation_rate rolling-window; Option::None on N&lt;30 | logic | TASK-TCT-010 | blocked |
| REQ-TCT-14 | FSV harness 4 evidence files; re-read + assert; mode 0o600 | surface | TASK-TCT-012 | blocked |
| REQ-TCT-15 | mcp__cgreality__mejepa_constellation_inspect MCP tool | surface | TASK-TCT-011 | blocked |

#### TECH-TCT component → Task mapping

| Tech component | Path | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| TctConstellation + try_new + version_id + lookup_centroid + threshold + check_provenance + check_freshness | `crates/context-graph-mejepa-tct/src/constellation.rs` | foundation / logic | TASK-TCT-001, TASK-TCT-004, TASK-TCT-008, TASK-TCT-009 | ready / blocked |
| ConstellationBuilder (new + ingest_corpus_entry + finalize_panel_level + finalize) | `crates/context-graph-mejepa-tct/src/builder.rs` | logic | TASK-TCT-003, TASK-TCT-004 | blocked |
| ThresholdCalibrator (calibrate + sweep_tau) | `crates/context-graph-mejepa-tct/src/calibrator.rs` | logic | TASK-TCT-005 | blocked |
| GtauChecker (gtau_check + gtau_check_chunks + cosine_similarity) | `crates/context-graph-mejepa-tct/src/gtau.rs` | logic | TASK-TCT-006 | blocked |
| ConstellationHingeOp (constellation_hinge + feature-gated candle variant stub) | `crates/context-graph-mejepa-tct/src/hinge.rs` | logic | TASK-TCT-007 | blocked |
| ConstellationStore (new + persist + load + list_versions) | `crates/context-graph-mejepa-tct/src/store.rs` | logic | TASK-TCT-008 | blocked |
| ViolationRateAggregator (record_verdict + constellation_violation_rate) | `crates/context-graph-mejepa-tct/src/rate_aggregator.rs` | logic | TASK-TCT-010 | blocked |
| McpInspectTool (mejepa_constellation_inspect + summary builder) | `crates/context-graph-mcp/src/handlers/tools/mejepa_tct.rs` | surface | TASK-TCT-011 | blocked |

#### Edge cases → Task mapping

| Edge case | Title | Task ID(s) | Status |
|---|---|---|---|
| EC-TCT-01 | N=50 boundary: keeps OwnCell (strict &lt;) | TASK-TCT-004 | blocked |
| EC-TCT-02 | N=4 fails closed with MEJEPA_TCT_INSUFFICIENT_SAMPLES | TASK-TCT-004 | blocked |
| EC-TCT-03 | N=29 cells emit warning + skip strict enforcement | TASK-TCT-005 | blocked |
| EC-TCT-04 | cos == τ_m boundary → accept (inclusive ≥) | TASK-TCT-006 | blocked |
| EC-TCT-05 | Unseen mutation_category falls back to category-aggregate | TASK-TCT-004, TASK-TCT-006 | blocked |
| EC-TCT-06 | Empty patch (zero chunks) → empty violations + panel-level Gτ | TASK-TCT-006 | blocked |
| EC-TCT-07 | EntityType not in constellation map → fallback to closest supported | TASK-TCT-006 | blocked |
| EC-TCT-08 | 89-day-old constellation passes (boundary just under max) | TASK-TCT-009 | blocked |
| EC-TCT-09 | 91-day-old constellation fails closed | TASK-TCT-009 | blocked |
| EC-TCT-10 | Embedder weight digest mismatch fails closed (CRITICAL) | TASK-TCT-008 | blocked |
| EC-TCT-11 | Rolling window N&lt;30 → Option::None | TASK-TCT-010 | blocked |
| EC-TCT-12 | NaN/Inf in panel slot → MEJEPA_INFER_DIM_MISMATCH | TASK-TCT-006 | blocked |
| EC-TCT-13 | Predictor early in training (anneal τ_m from 0 over 10K steps; deferred to TECH-PHASE3-TRAINING) | (cross-ref Phase 3) | blocked |

#### Error states → Task mapping

| Error state | Code | Task ID(s) | Status |
|---|---|---|---|
| ERR-TCT-01 | MEJEPA_INFER_CONSTELLATION_VIOLATION | TASK-TCT-002 (variant), TASK-TCT-006 (Gτ emit) | blocked |
| ERR-TCT-02 | MEJEPA_INFER_GTAU_STALE_CONSTELLATION | TASK-TCT-002, TASK-TCT-009 | blocked |
| ERR-TCT-03 | MEJEPA_TCT_INSUFFICIENT_SAMPLES | TASK-TCT-002, TASK-TCT-004 | blocked |
| ERR-TCT-04 | MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL | TASK-TCT-002, TASK-TCT-005 | blocked |
| ERR-TCT-05 | MEJEPA_TCT_PROVENANCE_MISMATCH | TASK-TCT-002, TASK-TCT-008 | blocked |
| ERR-TCT-06 | MEJEPA_INFER_DIM_MISMATCH (panel NaN/Inf) | TASK-TCT-002, TASK-TCT-006 | blocked |
| ERR-TCT-07 | MEJEPA_INSTR_FROZEN_VIOLATION (centroid/embedder gradient detected nonzero) | TASK-TCT-002 (variant), TECH-PHASE3-TRAINING owns runtime detection | blocked |

#### TCT cell math coverage

| Cell category | Count | Task ID |
|---|---:|---|
| Per-category centroids (mutation × embedder) | 8 × 21 = 168 | TASK-TCT-003 |
| Per-language centroids (language × mutation × embedder) | 9 × 8 × 21 = 1,512 | TASK-TCT-003 |
| Outcome centroids (oracle_outcome × embedder) | 4 × 21 = 84 | TASK-TCT-003 |
| Per-chunk-type centroids (mutation × entity × language × embedder) | 8 × 13 × 11 × 21 = 24,024 | TASK-TCT-004 (with shrinkage) |
| Panel-level total | 168 + 1,512 + 84 = 1,764 | TASK-TCT-003 |
| Combined total (post-shrinkage) | up to 1,764 + 24,024 = 25,788 | TASK-TCT-003, TASK-TCT-004 |

#### Cross-spec contracts (load-bearing)

| Contract | Direction | Task ID(s) | Notes |
|---|---|---|---|
| Verdict::GuardRejected variant payload | TCT → INFERENCE | TASK-TCT-002, TASK-INFER-001 (FailedGate::ConstellationGuardRejected mirror) | structurally identical fields |
| ConstellationGuard trait (gtau_test + version_id) | TCT → INFERENCE | TASK-TCT-006, TASK-INFER-003 (consumer trait) | Phase 4a uses MockedAlwaysApprove until TCT ships |
| L_constellation_centroid hinge primitive | TCT → TRAINING | TASK-TCT-007, TECH-PHASE3-TRAINING (consumer) | candle differentiable variant body lives in Phase 3 |
| constellation_violation_rate (consumer of CF_MEJEPA_GUARD_DECISIONS) | TCT → UTML | TASK-TCT-010, TASK-UTML-006 (compute_delta_xi consumer) | 30-sample minimum gate |
| constellation_violation_rate (consumer for refit trigger) | TCT → SELF-HEALING | TASK-TCT-010, Phase 5 SPEC-SELF-HEALING | sustained &gt; 5% / 2 hours triggers Latent-Collapsing |
| Provenance check at load | EMBEDDERS → TCT | TASK-TCT-008 | per-embedder weight SHA-256 match required |
| Phase 0 corpus iterator | CORPUS → TCT | TASK-TCT-003 | (panel, mutation, oracle, lang, entity, chunks) tuple stream |

#### Cross-cutting foundation tasks

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| context-graph-mejepa-tct crate scaffolding (Cargo.toml + lib.rs + module placeholders) + workspace member registration | foundation | TASK-TCT-001 | new crate per `<directory_structure>` |
| ShrinkageOrigin enum + ShrinkageDecision struct (replaces TASK-TCT-001 placeholder) | foundation | TASK-TCT-002 (enum) + TASK-TCT-004 (decision) | shrinkage hierarchy contract |
| Freshness check (audit-logged operator override → ./memory/journal/) | logic | TASK-TCT-009 | SMAP-namespaced override per CLAUDE.md §6 |

#### Uncovered items

(none — all 15 REQ-TCT-NN, all 7 TECH-TCT component contracts, all 13 EC-TCT-NN edge cases, all 7 error states have at least one task; layer ordering Foundation (2) → Logic (8) → Surface (2) preserved; dependency DAG acyclic; no task references files from later tasks per Iron Rule.)

**TCT coverage: 15/15 functional REQs + 7/7 technical components + 13/13 edge cases + 7/7 error states + 25,788/25,788 cells (panel-level + per-chunk-type) = 12 tasks; layer ordering Foundation (2) → Logic (8) → Surface (2); critical path TASK-TCT-001 → TASK-TCT-002 → TASK-TCT-003 → TASK-TCT-004 → TASK-TCT-005 → TASK-TCT-006 → TASK-TCT-008 → TASK-TCT-012 (~5 days end-to-end after Phase 0 corpus + ≥18 embedders + ≥12 panel slots ship).**

### Domain: SELF-HEALING

> **19 tasks generated** in `./phase5-self-healing/`. Foundation: 5 (TASK-HEAL-001..005). Logic: 10 (TASK-HEAL-006..015). Surface: 4 (TASK-HEAL-016..019). Coverage: 100% across 20 REQ-HEAL-NN + 7 TECH-SELF-HEALING component contracts. Crate: `crates/context-graph-mejepa/` (`src/heal/` sub-module + `bin/mejepa-heal-drill.rs` + `examples/phase5_self_healing_fsv.rs`). Reuses shipped `crates/context-graph-witness` (chain integrity + ed25519 + legacy reconcile + repair) and `crates/context-graph-embeddings/src/training/{lora.rs, distillation.rs}` (LoRA + distillation infra) — never reinvented.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-HEAL-01 | observe() signature + EWC-regularized SGD | foundation/logic | TASK-HEAL-001, TASK-HEAL-006, TASK-HEAL-008 | ready |
| REQ-HEAL-02 | EwcPlusPlus methods (update_fisher_online, detect_task_boundary, snapshot, current_lambda, ewc_penalty) | foundation/logic | TASK-HEAL-001, TASK-HEAL-006 | ready |
| REQ-HEAL-03 | DriftDetector + rolling 1,000-window + severity table + 3-window hysteresis | foundation/logic | TASK-HEAL-002, TASK-HEAL-007 | ready |
| REQ-HEAL-04 | AbcPromoter retrain_and_promote (3-mode + holdout gate + ModelPromote witness entry) | foundation/logic | TASK-HEAL-003, TASK-HEAL-009 | ready |
| REQ-HEAL-05 | schedule_full_retrain every 10,000 obs + 0.02 floor | logic | TASK-HEAL-011 | ready |
| REQ-HEAL-06 | continuous_calibration_if_due sliding 1,000-example window τ + σ² recompute | logic | TASK-HEAL-012 | ready |
| REQ-HEAL-07 | Witness chain integrity check (chain.len() % 73 + replay; halt + restore on break) | logic | TASK-HEAL-013 | ready |
| REQ-HEAL-08 | LoRA refresh on plasticity collapse (rank=32 epochs=10; frozen-base byte-equal invariant) | logic | TASK-HEAL-014 | ready |
| REQ-HEAL-09 | Distillation-based observe (InfoNCE + EMA τ=0.996 + signal_clarity quality_gate) | logic | TASK-HEAL-015 | ready |
| REQ-HEAL-10 | UTML regulate-state-first cycle (ASSESS → REGULATE → OBSERVE → COMPUTE → CLASSIFY → INTERVENE → EVALUATE) | logic | TASK-HEAL-008 | ready |
| REQ-HEAL-11 | Drift events surface (journal + blocker + status_change + recommendation) | logic | TASK-HEAL-007, TASK-HEAL-008 | ready |
| REQ-HEAL-12 | Promotion witness-chain entry (ModelPromote, ed25519 signed; SEC-MEJEPA-03) | logic | TASK-HEAL-009 | ready |
| REQ-HEAL-13 | signal_clarity multiplicative gating (skip below threshold OR weight proportionally) | logic | TASK-HEAL-008, TASK-HEAL-015 | ready |
| REQ-HEAL-14 | HealError taxonomy (12 variants + .code() SCREAMING_SNAKE) | foundation | TASK-HEAL-004 | ready |
| REQ-HEAL-15 | mejepa-heal-drill CLI + 5 FSV evidence files (mode 0o600 per SEC-MEJEPA-07) | surface | TASK-HEAL-018, TASK-HEAL-019 | ready |
| REQ-HEAL-16 | Real-data tests only (no mocks) | logic/surface | TASK-HEAL-008, TASK-HEAL-009, TASK-HEAL-018, TASK-HEAL-019 | ready |
| REQ-HEAL-17 | mcp__cgreality__mejepa_heal_status MCP tool (read-only HealStatus snapshot) | surface | TASK-HEAL-016 | ready |
| REQ-HEAL-18 | mcp__cgreality__mejepa_rollback_to MCP tool (operator-only; ModelRollback witness entry) | logic/surface | TASK-HEAL-010, TASK-HEAL-017 | ready |
| REQ-HEAL-19 | Performance budgets (observe p50&lt;5s, A/B/C promotion &lt;60min on RTX 5090) | logic/surface | TASK-HEAL-008, TASK-HEAL-009, TASK-HEAL-018 | ready |
| REQ-HEAL-20 | Cross-cutting with SHIFT-SUBSCRIBER (CF_MEJEPA_SHIFT_WATERMARK exactly-once) | logic | TASK-HEAL-008 | ready |

#### Tech component → Task mapping

| Tech component (TECH-SELF-HEALING) | Layer | Task ID(s) | Notes |
|---|---|---|---|
| SelfHealingPipeline | foundation/logic | TASK-HEAL-004, TASK-HEAL-008, TASK-HEAL-011, TASK-HEAL-012, TASK-HEAL-013 | top-level orchestrator + observe + scheduler hooks |
| EwcPlusPlus | foundation/logic | TASK-HEAL-001, TASK-HEAL-006 | online Fisher + λ ramp + boundary detector |
| DriftDetector | foundation/logic | TASK-HEAL-002, TASK-HEAL-007 | rolling 1,000-window + 3-window hysteresis |
| AbcPromoter | foundation/logic | TASK-HEAL-003, TASK-HEAL-009, TASK-HEAL-010 | retrain_and_promote + rollback_to (operator-only) |
| LoraRefresher | foundation/logic | TASK-HEAL-004, TASK-HEAL-014 | wraps shipped crates/context-graph-embeddings/src/training/lora.rs |
| OnlineDistiller | foundation/logic | TASK-HEAL-004, TASK-HEAL-015 | wraps shipped crates/context-graph-embeddings/src/training/distillation.rs |
| ChainIntegrityChecker | foundation/logic | TASK-HEAL-004, TASK-HEAL-013 | wraps shipped crates/context-graph-witness chain replay + legacy reconcile + repair |
| CalibrationWriter (TECH data model) | foundation/logic | TASK-HEAL-004, TASK-HEAL-012 | sliding window + τ + σ² recompute |
| HealDrillCli | surface | TASK-HEAL-018, TASK-HEAL-019 | binary + master FSV harness |
| McpHealStatusTool | surface | TASK-HEAL-016 | mcp__cgreality__mejepa_heal_status |
| McpRollbackTool | surface | TASK-HEAL-017 | mcp__cgreality__mejepa_rollback_to (operator-only) |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| HealError taxonomy + .code() SCREAMING_SNAKE registry | foundation | TASK-HEAL-004 | 12 variants per data_models HealError; consumed by every other task |
| HealReport + IntegrityViolationKind + CriticalBugKind enums | foundation | TASK-HEAL-004 | ModeWinner, holdout payloads, integrity break kinds |
| HealStatus snapshot + StatusChange + PromotionLockState | foundation | TASK-HEAL-004 | exposed via mcp_heal_status tool |
| RocksDB CFs (FISHER_SNAPSHOTS, DRIFT_WINDOW, DRIFT_HISTORY, HEAL_REPORTS, ACTIVE_POINTERS, CALIBRATION_HISTORY) | foundation | TASK-HEAL-005 | persistence schema |

#### Edge-case coverage

| Edge case | Description | Task ID(s) |
|---|---|---|
| EC-HEAL-04 | Cold-start drift detector window &lt; 1,000 → WarmupNotReady | TASK-HEAL-007 |
| EC-HEAL-05 | NaN mid-Mode-B retrain → BatchNan + lock release | TASK-HEAL-009 |
| EC-HEAL-06 | Grid-search w_A=1 result → A_unchanged informational | TASK-HEAL-009 |
| EC-HEAL-07 | 10,000-obs interval lands during in-flight A/B/C → defer next interval | TASK-HEAL-011 |
| EC-HEAL-08 | Legacy first-entry prev_hash mismatch → witness_chain_legacy::reconcile | TASK-HEAL-013 |
| EC-HEAL-09 | Multiple embedders need refresh → serialize by ΔK; cap concurrent at 1 | TASK-HEAL-014; σ² non-monotone TASK-HEAL-012 |
| EC-HEAL-10 | No oracle ground truth → signal_clarity=0 → skip; no pseudo-teacher | TASK-HEAL-015 |
| EC-HEAL-11 | REGULATE priority: stability_floor → effective_plasticity → constellation → predictor_redundancy → landscape | TASK-HEAL-008 |
| EC-HEAL-12 | Concurrent drift mid-promotion → lock held; new drift journaled but no second retrain | TASK-HEAL-009 |
| EC-HEAL-13 | Crash between witness entry + active-pointer update → recovery via latest ModelPromote replay | TASK-HEAL-009, TASK-HEAL-010 |
| EC-HEAL-14 | Operator rollback target weights_sha already GC'd → MEJEPA_HEAL_ROLLBACK_TARGET_GONE | TASK-HEAL-010, TASK-HEAL-017 |
| EC-HEAL-15 | Batched-mode resume from stale watermark → exactly-once shift re-processing | TASK-HEAL-008 |

#### Error-state coverage

| Error code | Variant | Task ID |
|---|---|---|
| MEJEPA_OBSERVE_BATCH_NAN | BatchNan | TASK-HEAL-008, TASK-HEAL-012, TASK-HEAL-015 |
| MEJEPA_OBSERVE_DRIFT_DETECTED | DriftDetected | TASK-HEAL-007, TASK-HEAL-008 |
| MEJEPA_OBSERVE_INTEGRITY_VIOLATION | IntegrityViolation | TASK-HEAL-013, TASK-HEAL-010, TASK-HEAL-017 |
| MEJEPA_RETRAIN_HOLDOUT_REGRESSION | HoldoutRegression | TASK-HEAL-009 |
| MEJEPA_RETRAIN_QUOTA_EXCEEDED | QuotaExceeded | TASK-HEAL-009 |
| MEJEPA_HEAL_FISHER_RANK_DEFICIENT | FisherRankDeficient | TASK-HEAL-006, TASK-HEAL-018 |
| MEJEPA_HEAL_LORA_REFRESH_FAIL | LoraRefreshFail | TASK-HEAL-014 |
| MEJEPA_HEAL_PROMOTION_DEADLOCK | PromotionDeadlock | TASK-HEAL-009, TASK-HEAL-010, TASK-HEAL-017 |
| MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES | ConformalInsufficientSamples | TASK-HEAL-012 |
| MEJEPA_HEAL_CRITICAL_BUG_TARGET_COLLAPSE_NONZERO | CriticalBug { TargetCollapseNonZero } | TASK-HEAL-008 |
| MEJEPA_HEAL_ROLLBACK_TARGET_GONE | RollbackTargetGone | TASK-HEAL-010, TASK-HEAL-017 |
| MEJEPA_HEAL_FSV_INCOMPLETE | FsvIncomplete | TASK-HEAL-019 |

#### Uncovered items

(none — all 20 REQ-HEAL-NN, all 7 TECH-SELF-HEALING component contracts, all 12 EC-HEAL-NN edge cases, all 12 error states have at least one task; layer ordering Foundation (5) → Logic (10) → Surface (4) preserved; dependency DAG acyclic per `./phase5-self-healing/_index.md` §Dependency DAG; no task references files from later tasks per Iron Rule.)

**SELF-HEALING coverage: 20/20 functional REQs + 7/7 technical components + 12/12 edge cases + 12/12 error states = 19 tasks; layer ordering Foundation (5) → Logic (10) → Surface (4); critical path TASK-HEAL-001 → 004 → 005 → 006 → 008 → 009 → 018 → 019 (~5–7 days end-to-end after Phase 2 Predictor + Phase 3 Training loop + Phase 4a RealityPrediction surfaces ship).**

### HYGIENE — pending generation

### Domain: SHIFT-SUBSCRIBER

> **21 tasks generated total** across two sibling phase directories:
> - `./phase7-shift-subscriber/` — 14 tasks (TASK-SHIFT-001..014); covers the Tokio subscriber, watermark CF, lag monitor, instrument cache, process_shift body, run() task, replay policy, latency profiler, shift-subscriber FSV.
> - `./phase7-mcp-tools/` — 7 tasks (TASK-MCP-001..007); covers the 5 new `mcp__cgreality__mejepa_*` MCP handlers + retired-tool fail-closed FSV.
>
> Layer split (combined): Foundation 6 (TASK-SHIFT-001..006 + TASK-MCP-001) → Logic 13 (TASK-SHIFT-007..012 + TASK-MCP-002..006) → Surface 2 (TASK-SHIFT-013, TASK-MCP-007).
>
> Coverage: 100% across 18 REQ-SHIFT-NN + 10 TECH-SHIFT-SUBSCRIBER components + all NFR-SHIFT-NN + all error states.

#### Functional REQ → Task mapping

| Functional REQ | Title | Layer | Task ID(s) | Status |
|---|---|---|---|---|
| REQ-SHIFT-01 | Single Tokio task; 1 CPU + 512 MB RSS | foundation/logic | TASK-SHIFT-001, TASK-SHIFT-005, TASK-SHIFT-010 | ready |
| REQ-SHIFT-02 | Restart-safe at-least-once delivery; watermark CF; sync=true | foundation/logic | TASK-SHIFT-002, TASK-SHIFT-004, TASK-SHIFT-008, TASK-SHIFT-010 | ready |
| REQ-SHIFT-03 | Lag-alert monitor; emit once per Idle→Active transition | logic | TASK-SHIFT-009, TASK-SHIFT-010 | ready |
| REQ-SHIFT-04 | Per-shift panel materialization (15 slots × 5,120-d) | logic | TASK-SHIFT-006, TASK-SHIFT-007 | ready |
| REQ-SHIFT-05 | Predict-only path persists to CF_MEJEPA_LIVE_PREDICTIONS | logic | TASK-SHIFT-007, TASK-MCP-003 (read side) | ready |
| REQ-SHIFT-06 | Observe path on oracle-attached shifts (L_step gated) | logic | TASK-SHIFT-007 | ready |
| REQ-SHIFT-07 | 5 MCP tools registered under `mcp__cgreality__mejepa_*` | foundation/logic | TASK-MCP-001 (skeleton) + TASK-MCP-002 (verify) + TASK-MCP-003 (predict_latest) + TASK-MCP-004 (observe_shift) + TASK-MCP-005 (subscriber_status) + TASK-MCP-006 (capture_audit) | ready |
| REQ-SHIFT-08 | Instrument cache LRU keyed by (file_sha, slot, code_version); 4 GB RAM budget | logic | TASK-SHIFT-006 | ready |
| REQ-SHIFT-09 | Persistence-vs-update separation (BR-SHIFT-04) | logic | TASK-SHIFT-007 | ready |
| REQ-SHIFT-10 | `mejepa_capture_audit` returns full signal capture | logic | TASK-MCP-006 (handler) + TASK-SHIFT-002 (CaptureAuditBundle carrier) | ready |
| REQ-SHIFT-11 | `mejepa_observe_shift` idempotent replay | logic | TASK-MCP-004 (MCP shim) + TASK-SHIFT-007 (process_shift idempotent overwrite) + TASK-SHIFT-011 (ReplayWatermarkPolicy::DoNotAdvance) | ready |
| REQ-SHIFT-12 | `mejepa_subscriber_status` snapshot fields | logic | TASK-MCP-005 (handler) + TASK-SHIFT-002 (SubscriberMetrics) + TASK-SHIFT-008 (WatermarkReader) + TASK-SHIFT-009 (lag-alert state machine) | ready |
| REQ-SHIFT-13 | Retired surfaces fail-closed CCREALITY_ENGINE_RETIRED | foundation/surface | TASK-SHIFT-003 (retired-shim contract) + TASK-MCP-001 (regression guard) + TASK-MCP-007 (FSV regression test on 5 aliases) | ready |
| REQ-SHIFT-14 | Failure-mode taxonomy (SubscriberError variants) | foundation | TASK-SHIFT-002, TASK-SHIFT-003 | ready |
| REQ-SHIFT-15 | Phase 7 DoD verify CLI | surface | TASK-SHIFT-014 | ready |
| REQ-SHIFT-16 | Performance budgets (p50/p99 per tool) | logic/surface | TASK-SHIFT-012 (latency profiler) + TASK-MCP-003/005/006 (per-tool latency assertions) + TASK-SHIFT-013 + TASK-MCP-007 (FSV harnesses) | ready |
| REQ-SHIFT-17 | Configurable knobs (`MeJepaShiftSubscriberConfig`) | foundation | TASK-SHIFT-001, TASK-SHIFT-005 | ready |
| REQ-SHIFT-18 | Provenance metadata in every persisted record | foundation/logic | TASK-SHIFT-002, TASK-SHIFT-007, TASK-MCP-002 (verify echoes provenance), TASK-MCP-006 (capture_audit echoes embedder_versions + frozen_at) | ready |

#### Technical component → Task mapping

| TECH-SHIFT-SUBSCRIBER component | Layer | Task ID(s) | Notes |
|---|---|---|---|
| ShiftSubscriber struct + new() | foundation/logic | TASK-SHIFT-001, TASK-SHIFT-005, TASK-SHIFT-007 | Arc-wrapped collaborators; trait-driven |
| ShiftSubscriber::run | logic | TASK-SHIFT-010 | Tokio task entry; supervisor + retry |
| ShiftSubscriber::process_shift | logic | TASK-SHIFT-007 | the heart of the subscriber |
| ShiftSubscriber::materialize_slot | logic | TASK-SHIFT-007 | spawn_blocking + cache + missing-slot policy |
| WatermarkWriter (write/read/read_all) | foundation/logic | TASK-SHIFT-004, TASK-SHIFT-008 | sync=true; monotonicity guard |
| ShiftLogTail (tail/poll/discover/rotation) | logic | TASK-SHIFT-005 | inode-aware; line-buffered |
| InstrumentCache (LRU + RAM budget) | logic | TASK-SHIFT-006 | 4 GB cap; resize hot |
| MejepaPredictLatestHandler | logic | TASK-MCP-001 (skeleton), TASK-MCP-003 (body) | CF prefix scan; limit clamp |
| MejepaObserveShiftHandler | logic | TASK-MCP-001 (skeleton), TASK-MCP-004 (body) | ULID validation + replay channel |
| MejepaSubscriberStatusHandler | logic | TASK-MCP-001 (skeleton), TASK-MCP-005 (body) | atomic snapshot + watermark CF read |
| MejepaCaptureAuditHandler | logic | TASK-MCP-001 (skeleton), TASK-MCP-006 (body) | 4-CF read + paginate + canonicalize floats |
| MejepaVerifyHandler | logic | TASK-MCP-001 (skeleton), TASK-MCP-002 (body) | path-canon + dispatch to MeJepaCompiler::verify (SPEC-INFERENCE owned body) |
| RetiredSurfaceHandler (reality_run_attempt + reality_alter_*) | foundation/surface | TASK-SHIFT-003 (existing fail-closed) + TASK-MCP-007 (FSV regression on 5 aliases) | locked CCREALITY_ENGINE_RETIRED |

#### Edge case / NFR / error state coverage

| Coverage item | Task ID(s) |
|---|---|
| EC-SHIFT-04 (cache hit p50 budget) | TASK-SHIFT-006, TASK-SHIFT-013 |
| EC-SHIFT-05 (L_step strict ≥ 0.05) | TASK-SHIFT-007 |
| EC-SHIFT-06 (SIGKILL recoverability) | TASK-SHIFT-010 |
| EC-SHIFT-10 (panic supervisor) | TASK-SHIFT-010 |
| EC-SHIFT-11 (empty session known=false) | TASK-MCP-003 |
| EC-SHIFT-12 (replay-on-replay idempotent) | TASK-MCP-004, TASK-SHIFT-011 |
| ERR-SHIFT-01..ERR-SHIFT-08 (all 8 error states) | TASK-SHIFT-002, TASK-SHIFT-003, TASK-MCP-002..006 (per-tool structured errors) |
| NFR-SHIFT-01..02 (subscriber p50/p99) | TASK-SHIFT-012, TASK-SHIFT-013 |
| NFR-SHIFT-03 (predict_latest p50 < 50 ms) | TASK-MCP-003 |
| NFR-SHIFT-04 (subscriber_status p50 < 50 ms) | TASK-MCP-005 |
| NFR-SHIFT-05 (capture_audit p50 < 100 ms) | TASK-MCP-006 |
| NFR-SHIFT-06 (restart-safe at-least-once delivery) | TASK-SHIFT-010 |
| NFR-SHIFT-07 (backpressure tolerance) | TASK-SHIFT-009, TASK-SHIFT-010 |
| NFR-SHIFT-08 (RSS / cache RAM ≤ 4 GB) | TASK-SHIFT-006 |
| NFR-SHIFT-09 (path canonicalization at MCP boundary) | TASK-MCP-002 |
| NFR-SHIFT-10 (FSV evidence files mode 0o600) | TASK-SHIFT-013, TASK-MCP-007 |
| TC-SHIFT-08 (5 tools register + reject extras) | TASK-MCP-001, TASK-MCP-007 |
| TC-SHIFT-09 (retired tool fail-closed) | TASK-MCP-007 |
| TC-SHIFT-11 (10 concurrent observe_shift → 1 row, 10 replay certs) | TASK-MCP-004 |
| TC-SHIFT-12 (subscriber_status reports task-dead honestly) | TASK-MCP-005 |

#### Cross-cutting foundation tasks (no single REQ owner)

| Tech component | Layer | Task ID | Notes |
|---|---|---|---|
| `MEJEPA_TOOL_NAMES` registry + dispatch | foundation | TASK-MCP-001 | locked `mcp__cgreality__mejepa_*` namespace |
| `mejepa/traits.rs` carrier types (PatchBundle, TaskContext, VerifyVerdict, ProvenanceBundle, RealityPrediction, ReplayRequest, CaptureAuditBundle re-exports) | foundation | TASK-MCP-002, TASK-MCP-003, TASK-MCP-004, TASK-MCP-005, TASK-MCP-006 | extended progressively across each MCP task; opaque to MCP, owned by SPEC-INFERENCE / sister phase carriers |
| `audit_assembler.rs` (assemble + paginate + canonicalize_floats + nearest_neighbors) | logic | TASK-MCP-006 | split out of capture_audit.rs to keep <500 LOC |

#### Cross-phase dependencies (out from MCP-TOOLS into sister phase)

| MCP task | Depends on (sister phase) | Reason |
|---|---|---|
| TASK-MCP-001 | TASK-SHIFT-003 | retired-surface fail-closed contract must already exist |
| TASK-MCP-002 | TASK-SHIFT-007 | subscriber metrics snapshot for `subscriber_state` field |
| TASK-MCP-003 | TASK-SHIFT-007 | CF_MEJEPA_LIVE_PREDICTIONS writer (single-source-of-truth `session_id_hash6` helper) |
| TASK-MCP-004 | TASK-SHIFT-007, TASK-SHIFT-011 | SubscriberIndex + ReplayWatermarkPolicy::DoNotAdvance + ReplayRequest channel |
| TASK-MCP-005 | TASK-SHIFT-002, TASK-SHIFT-008 | SubscriberMetrics types + WatermarkReader |
| TASK-MCP-006 | TASK-SHIFT-002, TASK-SHIFT-007 | CaptureAuditBundle carrier + 4-CF writer paths |
| TASK-MCP-007 | TASK-SHIFT-013 | sister FSV harness pattern + FsvFixture builder; also writes a roll-up at sister evidence dir |

#### Uncovered items

(none — all 18 REQ-SHIFT-NN, all 10 TECH-SHIFT-SUBSCRIBER component contracts, all 13 EC-SHIFT-NN edge cases, all 10 NFR-SHIFT-NN budgets, all 8 ERR-SHIFT-NN error states, all 5 new `mcp__cgreality__mejepa_*` tools and the 5 retired aliases have at least one task; layer ordering Foundation → Logic → Surface preserved across both sibling phase directories; dependency DAG acyclic (TASK-MCP-001 fans out to TASK-MCP-002..006, all five fan into TASK-MCP-007, plus the cross-phase edges into phase7-shift-subscriber); no task references files from later tasks per Iron Rule.)

**SHIFT-SUBSCRIBER coverage: 18/18 functional REQs + 10/10 technical components + 13/13 edge cases + 10/10 NFRs + 8/8 error states + 5/5 new MCP tools + 5/5 retired-alias regression guards = 21 tasks (14 subscriber + 7 MCP-tools); critical path TASK-SHIFT-001 → TASK-SHIFT-002 → TASK-SHIFT-007 → TASK-SHIFT-010 → TASK-SHIFT-013 in sister phase, plus TASK-MCP-001 → (TASK-MCP-002..006 batch-parallel) → TASK-MCP-007 in this phase.**

### EVALUATION — pending generation

---

## Validation checklist

Before declaring task generation complete (per `prdtospec.md` §4.6):

- [ ] All 211 functional REQs have ≥ 1 task
- [ ] All 107 technical components have ≥ 1 task
- [ ] All 8 mutation categories have a corpus generator task
- [ ] All 11 supported languages have an AST-chunker extension task
- [ ] All 14 instruments + Scalars + Panel have a task
- [ ] All 21 embedders have an Instrument-trait wrapper task (or are aggregated into a routing-table task with explicit coverage)
- [ ] All 4 UTML factors (ΔP, ΔK, ΔΩ, ΔΞ) have a compute-function task
- [ ] All 7 FailedGate variants have a gate-implementation task
- [ ] All 24,024 TCT cells have a centroid-construction task (aggregated)
- [ ] All 4 drift severities have a detection task
- [ ] All 9 categories in the 115 GB storage budget have a quota-enforcement task
- [ ] All 5 new MCP tools (`mejepa_verify`, `mejepa_predict_latest`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit`) + the additional MCP tools from TECH-EVALUATION / TECH-HYGIENE / TECH-TCT / TECH-SELF-HEALING have surface tasks
- [ ] All ship-gate categories (overall, per-category, per-language, per-cell, OOD AUC, conformal coverage, Q1, Q3) have an evaluation task
- [ ] Task dependency DAG is acyclic
- [ ] Layer ordering is preserved (foundation → logic → surface) within each phase
- [ ] No task references files created in a later task
- [ ] Every task has `<input_context_files>`, `<definition_of_done>`, `<files_to_create>`, `<test_commands>`

---

## Cross-references

- Master index: `../_README.md`
- Implementation plan: `../implementation-plan.md`
- Constitution: `../constitution.md`
- Functional specs: `../functional/_index.md`
- Technical specs: `../technical/_index.md`
- Task DAG: `./_index.md`
- Spec-authoring framework: `/home/cabdru/contextgraph/docs2/prdtospec.md`
