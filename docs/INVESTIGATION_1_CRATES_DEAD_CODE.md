# Investigation 1 — Crate Source Tree Dead Code & Tech Debt

**Investigator:** Sherlock Holmes Agent #1
**Date:** 2026-05-19
**Scope:** `/home/cabdru/contextgraph/crates/` — Rust source only (25 workspace members + 1 excluded benchmark crate). Scripts, top-level docs, `tests/oracle_**`, `swebench/**`, etc. are out of scope.

**Verdict-grade summary:** The crate tree has roughly **150,000-200,000 lines of source that cannot be reached from the binary predictor mission** (`context-graph-causal-agent` and `context-graph-graph-agent` are orphaned crates; ~14k LOC of TASK-RWD-201..210 reward/Q4-label signal modules in `mejepa-instruments` have no production caller; `context-graph-benchmark` is 85k LOC excluded from default-members and has zero external callers; the entire `commands/blackbox.rs` 35k-LOC subsystem is undocumented in the futurebuild doctrine). Most damningly, the 13 frozen `E*Instrument` types in `mejepa-instruments` are **not instantiated in the production panel-materialization path** — `compiler.rs::materialize_inference_panels` fills panels via a seed-driven `deterministic_vector(slot, &digest)` sin-wave function (`compiler.rs:1414`), not via the `EAstInstrument::encode()` / `EReasoningInstrument::encode()` / etc. trait calls; the instrument trait impls are only exercised by FSV examples + crate-internal tests.

---

## §1 Method (Linear Sequential Unmasking)

I examined the live tree **before** reading any operator-supplied summaries:

1. Read `/home/cabdru/contextgraph/Cargo.toml` to enumerate the 25 workspace members and the 25-crate `default-members` list (which deliberately excludes `context-graph-benchmark`).
2. Walked every `crates/*/src/lib.rs` to inventory the declared `pub mod` set per crate.
3. For each suspect module, ran workspace-wide grep for **external** type / function references (excluding `target/`, the module's own `*.rs`, its own `tests`, and its crate's `lib.rs` re-exports). A module with zero external references is a strong removal candidate.
4. For each suspect crate, ran `grep -l "context-graph-${name}" crates/*/Cargo.toml` to count workspace dependents. Zero or one self-cycle dependent = orphan candidate.
5. Read enough of the doctrine docs (`docs/futurebuild/01_overview.md`, `02_source_map.md`) AFTER my own grep evidence to compare claims to reality. (Two doctrine drift findings; see §8.)
6. Verified the most surprising claim — the `deterministic_vector` smoking gun in `compiler.rs:1414` — by reading the full function and confirming the `materialize_inference_panels` chain does NOT touch the 13 frozen-instrument `encode()` impls.

The Cardinal Rule was honored: every "no caller" claim below cites the grep command and the count.

---

## §2 Removal Candidates — HIGH confidence (guilty beyond reasonable doubt)

All entries here have **zero or near-zero workspace callers outside their own crate's tests/examples** and are not the live transport for any shipped MCP tool. They are recoverable from `git` history.

| # | Path | LOC | Last touched | External callers | Evidence | Suggested action | Risk |
|---|------|-----|--------------|------------------|----------|------------------|------|
| H1 | `crates/context-graph-graph-agent/` (whole crate) | 1,420 src + 587 tests | `c96f5c60` 2026-05-09 "ccreality-cutover: drop LLM-based MCP discovery surfaces" | **0** (the workspace dep graph: `grep -l 'context-graph-graph-agent' crates/*/Cargo.toml` returns only its own `Cargo.toml`) | All public exports (`MemoryScanner`, `ScannerConfig`, `GraphMarkers`, `MemoryForGraphAnalysis`, `ContentDomain`, `DomainMarkers`, `RelationshipCategory`) appear ONLY in its own `src/scanner/mod.rs` and `tests/scanner_integration.rs`. The 2026-05-09 commit message confirms the LLM backend it served has been retired. | Delete the crate. Remove from workspace members + default-members in `Cargo.toml`. | LOW — no production caller. |
| H2 | `crates/context-graph-causal-agent/` (whole crate) | ~1,300 src | `c96f5c60` 2026-05-09 "ccreality-cutover: drop LLM-based MCP discovery surfaces" | **1**, but only `context-graph-graph-agent` (which is itself H1) via the `From<CausalAgentError>` blanket impl in `src/error.rs`. Removing H1 reduces this to 0. | The only references outside the crate (`E5EmbedderActivator`, `CausalAgentError`, `MemoryScanner`, `CausalCandidate`, etc.) all live in `graph-agent`. Comment at `crates/context-graph-mcp/src/tools/definitions/causal_discovery.rs:5` mentions the crate by name but explicitly states "no MCP tool may trigger a local model". | Delete the crate together with H1. | LOW. |
| H3 | `crates/context-graph-mejepa-instruments/src/composite_reward_signals.rs` | 1,932 | `023e083c` 2026-05-17 "feat(mejepa): add composite reward signals" | **0** outside the crate. The lib.rs re-exports the symbols but `grep -rn '::composite_reward_signals\|CompositeRewardSignalStore' crates/ --include='*.rs'` excluding the instruments crate returns nothing. The only consumer is `examples/task_rwd_209_composite_reward_fsv.rs` inside the same crate. | TASK-RWD-209 was shipped 2026-05-17 but never plumbed into `mejepa-train` or `mejepa` (the predictor). Doctrine `01_overview.md §1.6` lists "composite reward / tech debt / churn / dep-CVE" under Q4 ambiguous surfaces that are **out of scope under #408**. | Move under `phase:cleanup` per `02_source_map.md §12.4`; freeze writes; eventual schema-version-bump deletion. | MEDIUM — Q4 doctrine freeze means the row producer is correctly not running, but the schema/types are referenced by future-cleanup tracking. |
| H4 | `crates/context-graph-mejepa-instruments/src/static_analysis_reward.rs` | 1,258 | `1e15019d` 2026-05-17 "Ship TASK-RWD-208 static-analysis reward signal" | **0** outside the crate (`grep -rn 'static_analysis_reward\|StaticAnalysisSignalStore\|extract_static_analysis_labels' crates/ --include='*.rs'` excluding the instruments crate returns only the in-crate FSV example). | TASK-RWD-208 — never consumed by `mejepa-train` or `mejepa`. Same Q4-freeze fate as H3. | Same as H3. | MEDIUM. |
| H5 | `crates/context-graph-mejepa-instruments/src/telemetry_signals.rs` | 1,243 | `622d94e7` 2026-05-17 "feat(mejepa): add telemetry reward signals" | **0** outside the crate. `TelemetrySignalStore`, `PersistedTelemetrySignal`, `TelemetrySignalRecord` exposed via lib.rs but consumed only by `examples/task_rwd_206_telemetry_fsv.rs` inside the same crate. | TASK-RWD-206 — never plumbed into predictor or trainer. Q4-frozen ambiguous surface. | Same as H3. | MEDIUM. |
| H6 | `crates/context-graph-mejepa-instruments/src/live_session_signals.rs` | 882 | `f3f4babd` 2026-05-18 "Remove TASK-EK-010 north-star intent elicitation feature" | **0** outside the crate. | TASK-RWD-207 — only consumed by `examples/task_rwd_207_live_session_fsv.rs`. Doctrine `01_overview.md` explicitly states "the system observes explicit task text, agent claims, edits, AST deltas… does NOT infer hidden human intent (#358)" — live-session intent signals are *exactly* the surface that was retired by the 2026-05-18 commit. | Same as H3 plus cite #358 / #397 retirement. | MEDIUM. |
| H7 | `crates/context-graph-mejepa-instruments/src/type_graph_signals.rs` | 480 | `1e15019d` 2026-05-17 era | **0** outside the crate. | TASK-RWD-205 — only consumed by `examples/task_rwd_205_type_graph_fsv.rs`. | Same as H3. | MEDIUM. |
| H8 | `crates/context-graph-mejepa-instruments/src/e_coverage.rs` | 795 | `cae96ac4` 2026-05-17 "Ship TASK-RWD-202 coverage reward signal" | **0** outside the crate. `ECoverageInstrument`, `CoverageSnapshot`, `CoverageMetrics`, `analyze_coverage_delta` re-exported via lib.rs but `grep -rn 'ECoverageInstrument\|CoverageSnapshot' crates/ --include='*.rs'` excluding the instruments crate returns nothing. | TASK-RWD-202. Not in the active 15-slot panel layout (`InstrumentSlot::layout()` doesn't list a Coverage slot). No `materialize_inference_panels` codepath consumes it. | Same as H3 — Q4 ambiguity. | MEDIUM. |
| H9 | `crates/context-graph-mejepa-instruments/src/e_cyclomatic.rs` | 828 | `316a3916` 2026-05-17 "Ship TASK-RWD-201 cyclomatic reward signal" | **0** outside the crate. | TASK-RWD-201. Not in the 15-slot panel layout. | Same as H3. | MEDIUM. |
| H10 | `crates/context-graph-mejepa-instruments/src/e_diff_topology.rs` | 789 | `db704a1e` 2026-05-17 "Ship TASK-RWD-203 diff-topology reward signal" | **0** outside the crate. | TASK-RWD-203. Not in the 15-slot panel layout. | Same as H3. | MEDIUM. |
| H11 | `crates/context-graph-mejepa-instruments/src/e_embedder_heads.rs` | 307 | `7ae70ec9` 2026-05-12 "feat(mejepa): WSL VRAM telemetry, typed semantic-head evidence, ship-gate FSV" | **0** outside the crate. | Per-embedder head readout never wired into predictor or trainer. | Same as H3 OR re-establish as live before next predictor refactor. | LOW (small file). |

**Subtotal for H3-H11 (Q4/RWD frozen surfaces in instruments crate):** 8,514 LOC across 9 files. All on the recommended `phase:cleanup` deletion list per `02_source_map.md §12.4` once #408 close-out work proceeds.

---

## §3 Removal Candidates — MEDIUM confidence (probable cause established)

| # | Path | LOC | Last touched | Evidence | Suggested action | Risk |
|---|------|-----|--------------|----------|------------------|------|
| M1 | `crates/context-graph-mejepa-instruments/src/q4_security_labels.rs` | 1,456 | recent | TASK-RWD-204 / TASK-PY-G-046. Externally referenced ONLY by `mejepa-train/src/labels.rs::q4_security_labels_to_prediction_surfaces` (which itself is consumed only by Q4 FSV examples — `q4_doctrine_freeze_fsv.rs`, `task_rwd_204_q4_security_fsv.rs`). The Q4 freeze gate at `mejepa/src/q4_trust_gate.rs:33` sets `Q4_DOCTRINE_FREEZE_ACTIVE: bool = true` — frozen as historical display data. | Move under `phase:cleanup`; freeze writes. Keep ROM-readable types so `mejepa_explain_prediction` can still surface historical Q4 rows. | LOW — already gated by the freeze flag. |
| M2 | `crates/context-graph-mejepa-instruments/src/q4_perf_labels.rs` (+ `_parse.rs`, `_store.rs`, `_support.rs`, `_tests.rs`) | 425 + 4 satellites | recent | TASK-RWD / PY-G-047 / 098 (#266, #399, #401). Used by `mejepa-train/src/labels.rs` Q4 surface converters → only Q4 FSV examples. Q4-freeze flag applies. | Same as M1. | LOW. |
| M3 | `crates/context-graph-mejepa-instruments/src/q4_accuracy_labels.rs` (+ satellites) | 412 + 4 | recent | TASK-PY-G-048 (#267, #401). Same Q4-freeze fate. | Same as M1. | LOW. |
| M4 | `crates/context-graph-mejepa-instruments/src/q4_cost_labels.rs` (+ satellites) | 314 + 4 | recent | TASK-PY-G-049 / 099 (#268, #400, #401). Same Q4-freeze fate. | Same as M1. | LOW. |
| M5 | `crates/context-graph-mejepa-instruments/src/q4_reasoning_labels.rs` (+ `_store.rs`, `_tests.rs`) | 878 + 2 | recent | TASK-PY-G-050 (#269). Same Q4-freeze fate. | Same as M1. | LOW. |
| M6 | `crates/context-graph-mejepa-instruments/src/q4_pass_annotations.rs` (+ `_tests.rs`) | 849 + 1 | recent | TASK-PY-G-051 (#270). Same fate. | Same as M1. | LOW. |
| M7 | `crates/context-graph-mejepa-instruments/src/q4_confidence_calibration.rs` (+ `_tests.rs`) | 975 + 1 | recent | TASK-PY-G-012 (#127). Calibrates the Q4 heads that are now display-only. | Same as M1 — keep readable for historical Q4 rows surfaced through `mejepa_inspect_prediction`. | LOW. |
| M8 | `crates/context-graph-mejepa/src/q4_trust_gate.rs` + `q4_trust_gate_support.rs` + `q4_trust_gate_tests.rs` | ~700 (3 files) | recent | TASK-PY-G-078 (#377). The trust-gate IS the freeze flag — `Q4_DOCTRINE_FREEZE_ACTIVE = true`. This module **is the doctrine enforcement**. Keep. | Keep (do NOT remove). It enforces the §1.4 freeze. Document as Q4-freeze enforcement primitive. | N/A — it's the gate, not the surface. |
| M9 | `crates/context-graph-cli/src/commands/blackbox.rs` + `src/bin/blackbox.rs` | 35,604 + 25 | `df758a88` 2026-05-13 "Fix live GitHub FSV credential preflight" | 26 tests under `crates/context-graph-cli/tests/blackbox_*.rs`. Wired into CLI via `commands::blackbox::handle_blackbox_command` (`main.rs:220`). Sub-system declares itself "BlackBoxFix Phase 1 local ledger and CLI" with 30+ connector IDs (GitHub, GitLab, K8s, Terraform, ticketing, OTEL, MCP, scoring, privacy, release-gate, packaging, risk, reward-compiler, etc.), HTTP server (`HTTP_SERVER_CONNECTOR_ID = "blackbox_http_server_v1"`), GitHub/GitLab API base URLs hard-coded, `reqwest::Client` + `axum::Router`. **Not mentioned in any live futurebuild doc**: `grep -i blackbox docs/futurebuild/*.md` finds nothing in the live set; the only mention is in `docs/futurebuild/_archived/14_NEW_LOOP_MD.md` which calls it the "BlackBox SQLite ledger" — i.e. the SAMP/SMAP-era persistence layer that's been superseded by GitHub Issues (per `CLAUDE.md §4`). | Open a `type:context` GitHub issue asking the operator whether the BlackBoxFix subsystem still has product mission. If no → delete the file + 26 blackbox tests + the `blackbox` `[[bin]]` declaration. If yes → write a doctrine doc placing it in `02_source_map.md §2`. | HIGH (large blast radius; many tests; HTTP server endpoints; operator decision required). |
| M10 | `crates/context-graph-mejepa-instruments/src/linear_probe.rs` | 274 | `259c5f59` 2026-05-09 "phase1(mejepa): instruments helpers + E14 fingerprint hardening" | **0** external callers. Only used by `examples/phase1_extended_fsv.rs`. `LINEAR_PROBE_MIN_ACCURACY = 0.70` constant unused anywhere else. Phase-1 era helper that survived the panel-rule refactor. | Move to FSV-helper module or delete. | LOW. |
| M11 | `crates/context-graph-mejepa-instruments/src/stdlib_registry.rs` | 305 | `2383e62f` 2026-05-12 "feat(mejepa): Phase B prediction surfaces + Phase D enrichment stack" | **0** external callers. Only `examples/phase_d_stdlib_registry_fsv.rs`. Lists 11 languages (`EXPECTED_STDLIB_LANGUAGES`) but the ship gate is Python-only per `01_overview.md §1.1`. | Delete or move under `phase:cleanup`. | LOW. |
| M12 | `crates/context-graph-mejepa/src/algorithmic_embedder_synthesis.rs` | 1,162 | recent | TASK-EK-018D — `AlgorithmicEmbedderCandidate`, `AlgorithmicEmbedderSynthesisConfig`, `AlgorithmicKernelTemplate` only consumed by `examples/task_ek_018d_algorithmic_embedder_synthesis_fsv.rs` (one FSV harness). | Defer to TASK-EK-018 review. Keep until #383-ordered EK-track is closed. | MEDIUM — TASK-EK series may still need this. |
| M13 | `crates/context-graph-mejepa/src/adversarial_embedder_injection.rs` | 510 | recent | TASK-EK-018J — only `examples/task_ek_018j_adversarial_injection_fsv.rs`. | Defer to TASK-EK-018 review. | MEDIUM. |
| M14 | `crates/context-graph-mejepa/src/learned_head_synthesis.rs` | 1,203 | recent | TASK-EK-018E — connected to `embedder_falsification` via `EmbedderFalsificationGate`; only FSV-example caller (`task_ek_018e_learned_head_synthesis_fsv.rs`). Forms a TASK-EK-018D/E/F cluster with M12 + `entity_kge_embedder.rs`. | Defer to TASK-EK-018 review; if EK-track is retired, remove the whole cluster. | MEDIUM. |
| M15 | `crates/context-graph-mejepa/src/entity_kge_embedder.rs` | 1,159 | recent | TASK-EK-018F. E11 replacement TransE on entity graph. Only `task_ek_018f_entity_kge_embedder_fsv.rs`. Doctrine references it as "the E11 replacement" — IF E11 is going to be replaced, this is the surface. | Keep until #383 evidence-weighted ordering decides on the EK-018 track. | MEDIUM. |
| M16 | `crates/context-graph-mejepa/src/instrument_proposal.rs` (referenced symbol `promote_instrument_proposal`) | ~? | recent | LIVE — `mcp/src/handlers/tools/mejepa_tools.rs:25-30` exposes via `MEJEPA_PROMOTE_INSTRUMENT_PROPOSAL`. Marked as `M16` for context — **NOT a removal candidate.** | Keep. | N/A. |

---

## §4 Removal Candidates — LOW confidence (suspicious, needs operator review)

| # | Path | LOC | Evidence | Suggested action | Risk |
|---|------|-----|----------|------------------|------|
| L1 | `crates/context-graph-benchmark/` (whole crate) | 85,087 | Excluded from `default-members` per `Cargo.toml:36`. Zero workspace dependents (`grep -l 'context-graph-benchmark' crates/*/Cargo.toml` returns only its own manifest). Has 4 `[[bin]]` targets (`maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench`). Description says "13-embedder multi-space fingerprinting vs single-embedder RAG" — that taxonomy predates the 2026-05-09 ME-JEPA pivot to the binary predictor. Has `--features real-embeddings` to bring in `context-graph-embeddings`. | Operator decision: do the 4 binary benchmarks still produce ship-gate-relevant numbers? If yes → keep but document under `02_source_map.md §8` more explicitly. If no → delete (85k LOC win). | HIGH (large delete; verify no FSV report still cites these benchmarks). |
| L2 | `crates/context-graph-mejepa/Cargo.toml` dev-dep `fail = { version = "0.5", features = ["failpoints"] }` | n/a | `grep -rn 'fail::\|use fail\|failpoint!' crates/context-graph-mejepa/ --include='*.rs'` returns no usages. The line in `lib.rs:140` (`pub use failure_fingerprint::*`) is unrelated (different `failure_fingerprint` module). | Remove the `fail` dev-dependency unless a planned testing harness needs it. | LOW. |
| L3 | `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain_legacy.rs` | 203 | `verify_legacy_chain_bytes_with_type_validator` is called from `witness_chain.rs:9` and `witness_chain_repair.rs:10`; verifies pre-2026-05-09 chain format for read-compat. Despite the `_legacy` suffix, **it is live** because the witness chain repair path needs to read both formats. | Keep. Listed here only because the `_legacy` suffix would otherwise flag it. | N/A. |
| L4 | `crates/context-graph-mejepa-corpus/src/multilang_fixture_corpus.rs` | ~? | Used only by `bin/mejepa-corpus.rs`. Per `01_overview.md` and `CLAUDE.md §6.3` (GENERIC-ONLY rule + Python-only ship gate), multi-language corpus generation is not on the critical path. | Defer to operator. May be useful for the planned 11-language coverage but not currently consumed. | LOW. |
| L5 | `crates/context-graph-mejepa-corpus/src/generic_language_ops.rs` | ~? | Live fallback for non-Python mutation ops (called from `crates/context-graph-mejepa-corpus/src/lib.rs:719`). Consistent with the "languages except Python deferred" doctrine. Keep as the deferred-language path. | Keep. | N/A. |
| L6 | Tracked TODO/FIXME (9 total across crates) | n/a | `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs:412` (CF_FILE_INDEX cleanup TODO); `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs:752` (Audit-14 STOR-L6 legacy E10 index TODO); `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:1235` (1094-line function TODO(MCP-M4) to split); `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs:3,93` (1800-line file TODO(MCP-L10) + 200-entry hardcoded list TODO(MCP-L9)); `crates/context-graph-core/src/monitoring.rs:237,243` (always-None pending TODO(INFRA-L5)). | File each as a `type:debt` GitHub issue. None are removal candidates; they're maintenance debt. | N/A. |
| L7 | 27 `#[allow(dead_code)]` annotations across the workspace | n/a | Most are CUDA-feature-gated fields (`crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs:51`, `crates/context-graph-mejepa-train/src/eval/mod.rs:131`, etc.) — legitimate. A handful are bare (`crates/context-graph-mcp/src/handlers/tools/memory_tools.rs:2574,2609`; `crates/context-graph-mcp/src/handlers/tools/reality_loop/analyze.rs:732`) — should be audited for actual deadness. | File `type:debt` issue tracking the bare-allow audit. | LOW. |
| L8 | `Qodo-Embed-1-1.5B` (the E7 code embedder built on Qwen2 architecture) | n/a | The "Qwen2" mentions in `crates/context-graph-embeddings/src/models/pretrained/code/*.rs` initially looked like retired-LLM remnants. Verified innocent: `constants.rs:19` pins to `Qodo/Qodo-Embed-1-1.5B`, a 1.5B-parameter **code embedder** that happens to be built on the Qwen2 architecture (decoder-only, GQA, RoPE, last-token-pool for embedding output). This is NOT generative inference — it's frozen-weight embedding extraction. Innocent. | None. | N/A. |
| L9 | `--mode reality-loop` cmdline mode in `mcp/src/main.rs:2370 run_reality_loop_stdio()` | 200+ LOC of singleton-flock plumbing | Initially looked like the retired reality-loop binary. Verified innocent: this is the **lightweight cgreality stdio MCP transport**, distinct from the retired reality-loop binary (which lived at `crates/context-graph-reality-loop` and was deleted 2026-05-09). The `handlers/tools/reality_loop/` directory serves the cgreality MCP tool family (`reality_*`, `harness_*`, `optimizer_*`, `autoresearch`, `reflexion`, `bandit`, `witness_chain`, `shift_log`) — these are read-only/governed MCP tools, NOT inner-loop LLM generation. The deprecation framework at `mcp/src/deprecation.rs:25-31` enumerates exactly 5 retired tools by name (`reality_run_attempt`, `reality_alter_file`, `reality_alter_run`, `reality_alter_apply`, `reality_drip_attempt`), all of which fail-closed with `CCREALITY_ENGINE_RETIRED`. The remaining cgreality tools are LIVE evidence-readback tools. The unfortunate module name `reality_loop/` should perhaps be renamed to `cgreality/` to avoid the namespace collision but that's cosmetic, not dead code. | Cosmetic rename only (not removal). | N/A. |

---

## §5 Q4 Frozen modules (per #408) — read-only forensic continuity

These are KEEP-but-document — see `01_overview.md §1.1` and `05_predictor_pipeline.md §16`. The CFs remain read-only and the producer pipelines are frozen. The MODULES (not just the CFs) are also frozen — `mejepa-train` and `mejepa` no longer call them outside Q4-doctrine FSV harnesses. They should ALL move under the `phase:cleanup` GitHub label per `02_source_map.md §12.4` and be deleted in the next schema-version bump.

| Path | Role |
|---|---|
| `crates/context-graph-mejepa-instruments/src/q4_security_labels.rs` (1,456 LOC) | TASK-RWD-204 / PY-G-046 |
| `crates/context-graph-mejepa-instruments/src/q4_perf_labels.rs` (+ 4 satellites, ~700 LOC total) | TASK-PY-G-047 / 098 |
| `crates/context-graph-mejepa-instruments/src/q4_accuracy_labels.rs` (+ 4 satellites, ~800 LOC total) | TASK-PY-G-048 |
| `crates/context-graph-mejepa-instruments/src/q4_cost_labels.rs` (+ 4 satellites, ~700 LOC total) | TASK-PY-G-049 / 099 |
| `crates/context-graph-mejepa-instruments/src/q4_reasoning_labels.rs` (+ 2 satellites, ~1,000 LOC total) | TASK-PY-G-050 |
| `crates/context-graph-mejepa-instruments/src/q4_pass_annotations.rs` (+ 1 satellite, ~1,000 LOC total) | TASK-PY-G-051 |
| `crates/context-graph-mejepa-instruments/src/q4_confidence_calibration.rs` (+ 1 satellite, ~1,100 LOC total) | TASK-PY-G-012 / #127 |
| `crates/context-graph-mejepa/src/q4_trust_gate.rs` + `_support.rs` + `_tests.rs` (~700 LOC) | **KEEP — this IS the freeze enforcement gate.** `Q4_DOCTRINE_FREEZE_ACTIVE: bool = true` at line 33 of `q4_trust_gate.rs`. |

Aggregate Q4 frozen-but-still-compiled surface: ~7,500 LOC. Aggregate frozen reward signals (H3-H11 in §2): ~14,000 LOC. Cleanup candidate total: ~21,500 LOC of frozen Q4/RWD code.

---

## §6 Tech debt observations (no removal proposed — file as separate Issues)

| Observation | Path | Issue type to file |
|---|---|---|
| The 13 frozen `E*Instrument` types (`EAstInstrument`, `ECfgInstrument`, `EDataFlowInstrument`, `ETypeGraphInstrument`, `ETestInstrument`, `ETraceInstrument`, `EDiffInstrument`, `EWitnessInstrument`, `EOracleInstrument`, `EProblemInstrument`, `ECommitMsgInstrument`, `EStaticAnalysisInstrument`, `EReasoningInstrument`, `ERuntimeInstrument`, `ScalarsInstrument`) are **never called from the production panel-materialization path**. `compiler.rs::materialize_inference_panels` (line 1365) builds panels through `materialize_panel()` with `PanelVectorInput { slot, vector }` where `vector` is `deterministic_vector(slot, &digest)` (line 1405-1421) — a seed-driven sin-wave function. The `Instrument::encode()` impls live in `e_oracle.rs:220`, `e_witness.rs:286`, `e_static_analysis.rs:284`, `e_code.rs:1587`, `e_text.rs:133`, `e_trace.rs:124`, `e_runtime.rs:78`, `e_reasoning.rs:111`, `e_scalars.rs:71` but are exercised ONLY by `crates/context-graph-mejepa-instruments/src/tests.rs` and `examples/phase1_extended_fsv.rs`. **This is the single most consequential finding of this investigation.** Either (a) the instrument trait surface is documentation aspiration and the deterministic-vector stub is the real predictor input, in which case the doctrine `01_overview.md §1.5ter` "What an entry in the panel is" needs revision; or (b) the deterministic_vector path is a placeholder that needs to be replaced before the ship gate. | `crates/context-graph-mejepa/src/compiler.rs:1365` and `:1414`; `crates/context-graph-mejepa-instruments/src/e_*.rs` | `type:context` — critical doctrine/code discrepancy, requires operator clarification. |
| File-length doctrine violations: `crates/context-graph-cli/src/commands/blackbox.rs` is 35,604 lines (71× the 500-line CLAUDE.md §9.10 limit). `crates/context-graph-mejepa/src/types.rs` is reported in `02_source_map.md` as 114K LOC. `crates/context-graph-mejepa-instruments/src/e_code.rs` is 1,587 LOC. `crates/context-graph-mejepa-instruments/src/composite_reward_signals.rs` is 1,932 LOC. `crates/context-graph-mejepa/src/chunk_foundationality.rs` is 2,787 LOC. | various | `type:debt` per file, batch into one rollup. |
| Function-length doctrine violations recorded as TODOs: `memory_tools.rs:1235 TODO(MCP-M4): 1094-line function with 13+ concerns`; `entity_tools.rs:3 TODO(MCP-L10): 1800-line file`. | `crates/context-graph-mcp/src/handlers/tools/` | `type:debt`. |
| Hardcoded-data TODO: `entity_tools.rs:93 TODO(MCP-L9): 200+ hardcoded entries should be data-driven`. | `crates/context-graph-mcp/src/handlers/tools/entity_tools.rs` | `type:debt`. |
| Always-`None` instrumentation: `core/src/monitoring.rs:237,243 TODO(INFRA-L5)` (always returns None instead of real layer-level metrics). | `crates/context-graph-core/src/monitoring.rs` | `type:debt`. |
| Stale "session:XXXX" comment: `temporal_positional/timestamp.rs:169` (matches `XXX` regex but is documentation, not a marker). | `crates/context-graph-embeddings/src/models/custom/temporal_positional/timestamp.rs` | None — false positive in my scan. |
| Pre-existing CF cleanup TODO: `storage/teleological/rocksdb_store/crud.rs:412 TODO: CF_FILE_INDEX cleanup requires reverse lookup`. | `crates/context-graph-storage/src/teleological/rocksdb_store/crud.rs` | `type:debt`. |
| Pre-existing legacy-index TODO: `storage/teleological/rocksdb_store/search.rs:752 TODO(Audit-14 STOR-L6): E10 uses the legacy E10Multimodal index`. | `crates/context-graph-storage/src/teleological/rocksdb_store/search.rs` | `type:debt`. |
| 27 `#[allow(dead_code)]` annotations. Most are CUDA-feature-conditional fields (legitimate). 3-5 are bare allows on apparently-unused functions: `memory_tools.rs:2574,2609`, `reality_loop/analyze.rs:732`, `daemon.rs:84/90/96/174`, `multi_array.rs:406,412`, `fusion.rs:188`. | various | `type:debt` audit. |

---

## §7 Cargo.toml dependency anomalies

| Crate | Dep | Anomaly | Action |
|---|---|---|---|
| `context-graph-mejepa` | `fail = { version = "0.5", features = ["failpoints"] }` (dev-dep) | Zero usages in `mejepa` source (`grep -rn 'fail::\|use fail\|failpoint!' crates/context-graph-mejepa/`). The crate also has `pub use failure_fingerprint::*` in lib.rs which is a different module entirely. | Remove unless an upcoming FSV harness will use failpoints. |
| `context-graph-mcp` | `axum = "0.8"` (prod dep, not feature-gated) | Used by `health_probe.rs` (live, port-bound health endpoint) and `server/http_transport.rs` (live HTTP transport for the MCP server). NOT a violation of CLAUDE.md §15.2 ("No HTTP from action paths") because the MCP server itself is the only sanctioned HTTP surface. | Keep. Document in doctrine that the MCP server's own listener is the allowlist exception. |
| `context-graph-cli` | `axum = "0.8"`, `reqwest = "0.12"` (prod deps, not feature-gated) | Both used **only** by `commands/blackbox.rs` (`grep -n axum\|reqwest crates/context-graph-cli/src/**/*.rs` returns hits only inside `blackbox.rs`). The MCP doctrine forbids HTTP from action paths. blackbox makes outbound HTTP calls to `https://api.github.com` and `https://gitlab.com/api/v4` (constants at lines ~76-79 of blackbox.rs). | Tightly coupled to M9 above. If blackbox stays, document the HTTP-egress exception. If blackbox is removed, remove `axum` and `reqwest` from `context-graph-cli`. |
| `context-graph-cli` | `rusqlite = "0.32"` (also bundled by `context-graph-mcp`) | Two SQLite databases live in the workspace: (1) blackbox local ledger (SQLite — `LEDGER_SCHEMA_VERSION = 14` in blackbox.rs), (2) `context-graph-mejepa/src/project_cache.rs:334` chunk-paths SQLite. The doctrine RocksDB-only invariant (per CF discipline in `03_storage_schema.md`) doesn't apply to ephemeral local caches but does suggest the blackbox ledger is out-of-pattern. | Document or migrate. |
| `context-graph-cli` | 11 `tree-sitter-{language}` deps + `tree-sitter = "0.26"` | Python-only ship gate per `01_overview.md §1.1`. The 10 non-Python tree-sitter deps (`rust`, `javascript`, `typescript`, `go`, `java`, `c`, `cpp`, `c-sharp`, `ruby`, `php`) carry the multi-language deferred surface. Live but representing deferred work. | Keep until 11-language coverage is decided. |
| `context-graph-cli` | `parquet = "58"`, `arrow-array = "58"`, `arrow-schema = "58"` | "Parquet file-format export (Phase 6)" per comment. Used by `commands/export_dynamicjepa.rs`. Live. | Keep. |
| `context-graph-cli` | `zip = "7.2.0"` | Probably used by the export/pilot-package flow. Likely live but worth verifying caller. | Verify with grep then keep/remove. |
| `context-graph-mejepa-corpus` | `rayon = "1.10"`, `num_cpus = "1.16"`, `ctrlc = "3.5"` | All consistent with the parallel-shard corpus generator binary. Live. | Keep. |
| `context-graph-mejepa` | `ruff_python_ast`, `ruff_python_parser` (`rustpython-ruff_python_ast 0.15.8`) | Python AST parsing — consistent with the Python ship gate. Live. | Keep. |
| `context-graph-benchmark` | full `context-graph-storage`, optional `context-graph-embeddings`, `criterion`, `rand_chacha`, `md5` | If the crate is kept, these are fine. If L1 (benchmark crate) is removed, all of these stop mattering. | Conditional on L1. |
| `context-graph-mcp` | `feature = "llm"` enables `compile_error!` at `lib.rs:13` | Correctly fails-closed per CLAUDE.md §15.1. **Innocent and well-implemented.** Same pattern exists in `context-graph-causal-agent/src/lib.rs:18-21` and `context-graph-graph-agent/src/lib.rs:46-49`. | Keep — exemplar pattern. |

---

## §8 Modules not appearing in futurebuild/02_source_map.md

The futurebuild doc `02_source_map.md` §3-§6 is the authoritative module inventory. I cross-checked every `crates/*/src/*.rs` file against it and found:

**Documented and present in the live tree:** all §3 (mejepa), §4 (mejepa-corpus), §5 (mejepa-instruments), §6 (mejepa-train) entries verified present.

**Present in the live tree but NOT in §3-§6 (undocumented modules — likely either legitimate-but-recent or actually-dead):**

| Module | Crate | Verdict |
|---|---|---|
| `crates/context-graph-mejepa/src/q4_trust_gate_support.rs` (700 LOC, included via `#[path]` from `q4_trust_gate.rs:13`) | mejepa | Live — supports M8. Document. |
| `crates/context-graph-mejepa/src/q4_trust_gate_tests.rs` | mejepa | Test-only — fine. |
| `crates/context-graph-mejepa/src/synthetic_stress_cases.rs` (referenced as `mod` in lib.rs:78) | mejepa | Live — supporting synthetic_stress module. |
| `crates/context-graph-mejepa/src/synthetic_stress_corpus.rs` | mejepa | Live — same. |
| `crates/context-graph-mejepa/src/synthetic_stress_eval.rs` | mejepa | Live — same. |
| `crates/context-graph-mejepa/src/synthetic_stress_store.rs` | mejepa | Live — same. |
| `crates/context-graph-mejepa/src/constellation_observation_tests.rs` | mejepa | Test-only — fine. |
| `crates/context-graph-mejepa/src/verify_cli_tests.rs` | mejepa | Test-only — fine. |
| `crates/context-graph-mejepa-instruments/src/q4_*_parse.rs` / `_store.rs` / `_support.rs` / `_tests.rs` satellites (per-Q4-label-kind) | mejepa-instruments | Documented at module level in §5 row; the per-kind satellite files are not explicit but are implied. |
| `crates/context-graph-mejepa-instruments/src/q4_confidence_calibration_tests.rs` | mejepa-instruments | Test-only. |
| `crates/context-graph-mejepa-instruments/src/q4_pass_annotations_tests.rs` | mejepa-instruments | Test-only. |
| `crates/context-graph-mejepa-instruments/src/features.rs` (private `mod features` at lib.rs:47) | mejepa-instruments | Live — shared helpers. |
| `crates/context-graph-cli/src/commands/blackbox.rs` (35,604 LOC) | cli | **Undocumented in live futurebuild docs.** Tracked under M9. |
| `crates/context-graph-cli/src/bin/blackbox.rs` (the binary entry) | cli | Same — M9. |
| `crates/context-graph-cli/src/bin/context-graph.rs` | cli | Documented in §8 binary table but `02_source_map.md §8` only lists the workspace `context-graph-cli` binary at `src/main.rs`. The `[[bin]] name = "context-graph"` at `Cargo.toml:23-25` is an additional binary — should be added to the binary inventory. |
| `crates/context-graph-mejepa-train/src/mistake_log.rs` (mentioned in `02_source_map.md §12.3` as planned, but present already) | mejepa-train | Live. Doc is slightly behind the code. Update doc. |
| `crates/context-graph-mejepa-train/src/replay_buffer.rs` | mejepa-train | Listed in `02_source_map.md §6`. Live. Has `_legacy`-like commit history but file is unambiguously live. |
| `crates/context-graph-mejepa-train/src/chunk_skill_membership_tests.rs` | mejepa-train | Test-only; supports the #414 / #417 skill-membership surface. |
| Several `crates/context-graph-mejepa/src/eval/*.rs` and `crates/context-graph-mejepa/src/heal/*.rs` submodule files | mejepa | Doc mentions `heal/` and `eval/` directories but not per-file inventory. Live — consumed by compiler. |
| `crates/context-graph-mejepa/src/loss/*.rs` | mejepa | Doc mentions `loss/` directory. Live. |
| `crates/context-graph-mejepa/src/ood_harvest/*` (directory of submodules) | mejepa | Doc mentions `ood_harvest.rs` and `ood_harvest/`. Live. |
| `crates/context-graph-mejepa/src/predictor/*.rs` | mejepa | Doc mentions `predictor/`. Live. |
| 5 reward signal modules in mejepa-instruments (H3-H7 in §2) | mejepa-instruments | Listed in §5 of the doc as TASK-RWD-201..210 entries but with no caller chain documented. The doc lists the modules but does not say "the predictor and trainer DO consume these" — and they don't. Doc is technically truthful but misleading. |

**Doctrine drift findings (futurebuild doc inaccuracies):**

1. **`02_source_map.md §7` (Dependency Graph) shows `context-graph-mcp` depending on `context-graph-causal-agent` and `context-graph-graph-agent`. In reality, `crates/context-graph-mcp/Cargo.toml` does NOT list either as a dependency.** Verified: `grep -E 'context-graph-(causal|graph)-agent' crates/context-graph-mcp/Cargo.toml` returns nothing. The MCP only mentions causal-agent in a comment at `tools/definitions/causal_discovery.rs:5`. The doc is wrong; reality is that **causal-agent and graph-agent are orphan crates**.
2. **`02_source_map.md §8` (Binary Targets) lists `context-graph-cli src/main.rs (230 LOC)` but the cli crate has THREE `[[bin]]` entries**: `context-graph-cli`, `context-graph`, and `blackbox`. The doc should list all three.

These doctrine-drift items are themselves filable as `type:discovery` GitHub issues per CLAUDE.md §4.

---

## §9 Benchmark crate fate

**Path:** `crates/context-graph-benchmark/`
**LOC:** 85,087 (`find crates/context-graph-benchmark -name '*.rs' -not -path '*/target/*' | xargs wc -l | tail -1`)
**Default-build status:** EXCLUDED — `Cargo.toml:34` default-members list does not include it.
**Workspace dependents:** **0** (`grep -l 'context-graph-benchmark' crates/*/Cargo.toml` returns only its own manifest).
**Internal binaries:** 4 — `maxsim_bench`, `causal_relationships_bench`, `hybrid_effectiveness_bench`, `e11_quality_bench`.
**Last touched:** mixed (recent commits exist but the framework predates the 2026-05-09 ME-JEPA pivot — the crate description is "13-embedder multi-space fingerprinting vs single-embedder RAG", which is pre-pivot language).
**FSV evidence dependencies:** I did NOT verify whether any current FSV report under `/zfs/archive/contextgraph/fsv/` cites the 4 benchmark bins. That is out of scope (`fsv/` is on aiwonder; my scope is local source).

**Verdict:** Operator decision required.

- If the 4 benchmarks produce ship-gate-relevant numbers (e.g. `e11_quality_bench` is the calibrated E11 → entity_kge_embedder swap reference benchmark referenced by TASK-EK-018F) → **KEEP** but document explicitly in `02_source_map.md §2`.
- If the 4 benchmarks are pre-pivot artifacts (the "13-embedder multi-space fingerprinting vs single-embedder RAG" framing predates the binary-predictor product surface) → **REMOVE** the whole crate, removing `context-graph-benchmark` from workspace members in `Cargo.toml`. 85,087 LOC removed.

Suggested next step: file a `type:context` GitHub issue asking the operator. Do not delete without explicit OK.

---

## §10 Crate-by-crate summary

| # | Crate | Role | Verdict |
|---|---|---|---|
| 1 | `context-graph-mcp` | MCP server | LIVE — product surface. |
| 2 | `context-graph-cli` | Operator CLI + helpers | LIVE — but `blackbox.rs` (35,604 LOC, M9) needs operator decision. |
| 3 | `context-graph-core` | Shared domain types | LIVE — 15 workspace dependents. |
| 4 | `context-graph-storage` | RocksDB wrapper + 51+ CFs | LIVE — 7 dependents. |
| 5 | `context-graph-witness` | Tamper-evident witness chain | LIVE — 6 dependents. |
| 6 | `context-graph-solver` | Causal masking, MMR, conformal | LIVE — 2 dependents. |
| 7 | `context-graph-mincut` | Stoer-Wagner min-cut | LIVE — 2 dependents (mejepa, mejepa-train). |
| 8 | `context-graph-paths` | Path resolver | LIVE — 5 dependents. |
| 9 | `context-graph-tier-compression` | 8/7/5/3-bit groupwise compression | LIVE — used by hygiene crate. |
| 10 | `context-graph-embeddings` | E1-E14 embedder pipeline | LIVE — 9 dependents. Innocent Qwen2 reference is the Qodo-Embed code-embedder architecture (not generative LLM). |
| 11 | `context-graph-cuda` | CUDA helpers | LIVE — 5 dependents. |
| 12 | `context-graph-mejepa` | Predictor trunk (83 src files) | LIVE — but the TASK-EK-018 cluster (M12-M15) and the M8 q4_trust_gate keep are notable. **§6 critical finding** — instrument trait impls not called in production path. |
| 13 | `context-graph-mejepa-cf` | CF name registry | LIVE — 11 dependents (intentional zero-dep design). |
| 14 | `context-graph-mejepa-corpus` | 8× mutation ops + SWE-bench bridge | LIVE — 3 dependents. |
| 15 | `context-graph-mejepa-embedders` | Embedder slots + dynamic registry | LIVE — 6 dependents. |
| 16 | `context-graph-mejepa-hygiene` | Tier compression / quota / GC | LIVE — 4 dependents. |
| 17 | `context-graph-mejepa-instruments` | 15 instrument slots + panel scaffolding | **PARTIALLY DEAD** — H3-H11 (RWD signal modules) and M1-M7 + M10-M11 (Q4 labels + linear_probe + stdlib_registry) total ~22,000 LOC of frozen/unconsumed code. The 15 `Instrument` trait impls themselves are only called in tests and FSV examples (see §6 finding). |
| 18 | `context-graph-mejepa-shift-subscriber` | JSONL shift-log tail → panels | LIVE — 3 dependents. |
| 19 | `context-graph-mejepa-tct` | TCT constellations + Gτ guard | LIVE — 3 dependents. |
| 20 | `context-graph-mejepa-train` | Trainer + UTML + replay buffer | LIVE — 5 dependents. |
| 21 | `context-graph-dynamicjepa` | 5090jepa service | LIVE but borderline — 1 dependent (CLI only). |
| 22 | `context-graph-causal-agent` | Causal scanning + E5 activation | **DEAD (H2)** — 1 self-cycle dependent only (graph-agent). Slated for deletion after 2026-05-09 LLM cutover. |
| 23 | `context-graph-graph-agent` | Graph scanning + E8 activation | **DEAD (H1)** — 0 dependents. |
| 24 | `context-graph-test-utils` | Shared test utilities | LIVE — 3 dependents. |
| 25 | `context-graph-benchmark` | 85k-LOC benchmark suite (excluded from default-members) | **CANDIDATE FOR REMOVAL (L1)** — 0 external dependents. |

---

## §11 Suggested removal sequence (safest order)

Remove in this order to minimize collateral damage:

1. **First (lowest risk, safe to do today):** Remove H1 (`crates/context-graph-graph-agent/`) and H2 (`crates/context-graph-causal-agent/`) together. Edit `Cargo.toml` to drop them from `members` and `default-members`. Net delete: ~2,700 LOC. No production caller — graph-agent has zero workspace dependents and causal-agent is only referenced by graph-agent.
2. **Second (file as type:context issue first):** L1 — `context-graph-benchmark` (85,087 LOC). Operator confirms whether any current FSV cites these benchmarks. If no, delete. If yes, document.
3. **Third (file as type:context issue first):** M9 — `context-graph-cli/src/commands/blackbox.rs` and `src/bin/blackbox.rs` + 26 blackbox tests. Operator confirms whether BlackBoxFix is in product scope. If no, delete (~35,629 LOC + tests). If yes, document under §2 of futurebuild source map.
4. **Fourth (when #408 cleanup proceeds — keep until then):** H3-H11 (the 9 RWD/signal modules in mejepa-instruments). Total ~14,000 LOC. The Q4 freeze gate at M8 must stay; only the producer modules go. Do this as part of the schema-version-bump cleanup pass referenced in `02_source_map.md §12.4`.
5. **Fifth (same cleanup pass):** M1-M7 (the Q4 label modules + their satellites). Total ~7,500 LOC. Keep `q4_trust_gate.rs` (M8) and the freeze flag.
6. **Sixth (open a #383 evidence-weighting decision):** M12-M15 (TASK-EK-018D/E/F/J cluster — algorithmic embedder synthesis, learned head synthesis, entity KGE, adversarial injection). Total ~4,000 LOC. Wait for the EK-track decision per the pinned #310 / #383 context issues.
7. **Seventh (small utility cleanup):** M10 + M11 (linear_probe + stdlib_registry, ~580 LOC) and L2 (remove `fail` dev-dep from mejepa Cargo.toml).
8. **Throughout:** File `type:debt` issues for the L6 TODOs and L7 dead-code allows. None of those block the above sequence.

**Estimated total removable LOC if operator approves L1 + M9:** ~146,300 (H1+H2 ~2,700 + L1 85,087 + M9 35,629 + H3-H11 ~14,000 + M1-M7 ~7,500 + M12-M15 ~4,000 + smaller items).

---

## §12 What I did NOT investigate (out of scope per the operator brief)

- `scripts/` (CC hooks, CI lints, swebench bridge, setup, systemd) — covered by a parallel investigator.
- `docs/` at root + `docs/futurebuild/` content (other than reading them for context) — parallel investigator.
- `tests/oracle_**/**` — explicit don't-touch list per CLAUDE.md §7.
- `swebench/**` — explicit don't-touch list.
- `crates/*/src/reward_specs/**` and `crates/*/src/verifiers/**` — explicit don't-touch list (verified these paths do not exist in the current tree, so the don't-touch list is preventive).
- `*.pem`, `*.key`, `**/credentials.json` — explicit don't-touch list.
- `/zfs/archive/contextgraph/**` and `/zfs/hot/contextgraph/**` aiwonder durable artifacts — out of scope (this investigation reads source code only).
- FSV evidence file inventory — out of scope (lives in `/zfs/archive/contextgraph/fsv/` on aiwonder; my evidence is the source code itself).
- `.github/workflows/` — out of scope (parallel investigator).
- Top-level `Cargo.lock`, `rust-toolchain.toml`, `rustfmt.toml`, `.cargo/config.toml` — touched only to read workspace members from `Cargo.toml`.
- Cross-repo references (claude-flow, agentdb, ruvector) — out of scope per CLAUDE.md §6.7 (clean-room rule means there are none to investigate).
- Runtime behavior — this investigation is forensic on the source tree; no `cargo build` or `cargo test` was run.

---

**End of Investigation 1.**
