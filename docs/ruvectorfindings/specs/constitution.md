# ME-JEPA-Code Constitution

> **Level 1 (immutable rules).** This document is the supreme spec authority. When a functional or technical spec conflicts with this constitution, the constitution wins. Constitution changes require explicit operator approval and a new `<spec_version>`.

```xml
<constitution version="1.0.0">

<metadata>
  <project_name>ME-JEPA-Code (the Reality Compiler)</project_name>
  <spec_version>1.0.0</spec_version>
  <established>2026-05-09</established>
  <governing_directives>
    <doc>docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md (master plan)</doc>
    <doc>docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md (post-pivot architecture)</doc>
    <doc>docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md (UTML v1.0 four-factor formula)</doc>
    <doc>docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md (21-embedder ensemble)</doc>
    <doc>docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md (TCT framework)</doc>
    <doc>docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md (multi-language AST chunking)</doc>
    <doc>CLAUDE.md §11 (operator role + reality-loop contract)</doc>
    <doc>memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md</doc>
  </governing_directives>
</metadata>

<mission>
Build ME-JEPA-Code: a Joint Embedding Predictive Architecture that, given (code change + context),
predicts a calibrated reality state — what the SWE-bench Docker oracle would say if the change were
run. When `predicted_oracle_pass` correlation with the actual Docker oracle stably crosses 0.95
across all 8 mutation categories AND all supported languages on a holdout split, human code review
of AI-generated patches becomes statistically unjustified.

ME-JEPA-Code answers three operator questions deterministically for any AI agent's output:
  Q1: Does what the AI claimed it did EXIST?       (claim graph ↔ source-of-truth SHA chain)
  Q2: Does it WORK as intended?                    (predicted oracle outcome ↔ actual outcome)
  Q3: How is it actually IMPACTING REALITY?        (predicted side-effect set ↔ observed shifts)
</mission>

<tech_stack>
  <language version="1.84+">Rust (stable, edition 2021)</language>
  <ml_framework version="0.9.x">candle (pure-Rust, sm_120a CUDA via cudarc)</ml_framework>
  <ast_parser version="0.23.x">tree-sitter (11 grammars: rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php)</ast_parser>
  <storage>RocksDB (column families) + AgentDB hybrid (per CLAUDE.md §4)</storage>
  <protocol>Model Context Protocol (MCP) over JSON-RPC stdio (context-graph-mcp crate)</protocol>
  <oracle>Official SWE-bench Docker harness (swebench.harness.run_evaluation; per-instance grading per swebench/harness/grading.py)</oracle>
  <signing>ed25519 segment signing + SHAKE-256 73-byte witness chain (clean-room reimplementation)</signing>
  <embedder_count>21 (E1-E14 content + E15-E21 learner-state, all clean-room)</embedder_count>
  <required_libraries>
    <library version="0.23.x">tree-sitter (rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php)</library>
    <library version="0.9.x">candle-core, candle-nn, candle-transformers</library>
    <library version="0.21.x">rocksdb</library>
    <library version="1.0.x">serde, serde_json</library>
    <library version="1.x">tokio (with full feature)</library>
    <library version="0.7.x">sha3 (SHAKE-256)</library>
    <library version="2.x">ed25519-dalek</library>
    <library version="0.4.x">tokenizers (HuggingFace)</library>
  </required_libraries>
</tech_stack>

<directory_structure>
<![CDATA[
crates/
  context-graph-mcp/                       # MCP server (JSON-RPC over stdio)
  context-graph-cli/                       # CLI surface (DynamicJEPA, ME-JEPA, harness ops)
  context-graph-core/                      # AST chunker (multi-language) + memory primitives
    src/memory/ast_chunker.rs              # 11-language AST-perfect chunker (Phase 1)
  context-graph-storage/                   # RocksDB CFs + tier compression integration
  context-graph-embeddings/                # 21 embedders (E1-E21) + LoRA + distillation
    src/types/model_id/core.rs             # E1-E14 enum
    src/learner/                           # E15-E21 learner-state
    src/training/                          # LoRA + AdamW + InfoNCE + multitask + distillation
  context-graph-witness/                   # 73-byte SHAKE-256 chain + ed25519 + Stage 0/F/Merkle certs (✅ shipped)
  context-graph-solver/                    # Forward-Push PPR + CG + MMR + conformal predictor (✅ partial)
  context-graph-mincut/                    # Stoer-Wagner mincut for repo-atomic split (✅ shipped)
  context-graph-tier-compression/          # 8/7/5/3-bit groupwise quantization (✅ shipped)
  context-graph-mejepa-corpus/             # Phase 0 mutation corpus + Docker oracle bridge + batch CLI (✅ partial; full 300x8 run pending)
  context-graph-mejepa-instruments/        # Phase 1 instrument trait + all 15 deterministic typed panel-slot encoders (✅ partial; neural/deep integrations pending)
  context-graph-mejepa/                    # Phase 2-4 predictor + frozen target + VICReg + Oracle head + verify() (NEW)
  context-graph-mejepa-train/              # Phase 3 training loop + UTML L_step + sampler (NEW)
  context-graph-mejepa-tct/                # TCT constellation construction + Gτ guard (NEW)
  context-graph-mejepa-shift-subscriber/   # Phase 7 §3.8 Tokio subscriber (NEW)

docs/ruvectorfindings/
  09_END_GOAL_REVIEW_REPLACEMENT.md        # master plan
  10_MEJEPA_PIVOT_2026_05_09.md            # post-pivot architecture
  11_MEJEPA_LEARNING_FORMULA.md            # UTML v1.0
  12_EMBEDDER_INTEGRATION.md   # 21-embedder map
  13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md  # TCT
  14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md       # multi-language AST chunking
  specs/
    constitution.md                         # THIS FILE (Level 1)
    _README.md                              # master index + phase roadmap
    implementation-plan.md                  # phase-ordered execution plan
    functional/                             # Level 2 — what to build
    technical/                              # Level 3 — how to build
    tasks/                                  # Level 4 — atomic work units
      _index.md                             # task dependency DAG
      _traceability.md                      # spec → task coverage matrix
      phase0-corpus/
      phase1-ast-chunking/
      phase1-instruments/
      phase1-embedders/
      phase2-predictor/
      phase3-training/
      phase4-inference/
      phase4-tct-constellation/
      phase5-self-healing/
      phase6-hygiene/
      phase7-shift-subscriber/
      phase7-mcp-tools/
      phase8-evaluation/
      utml-learning-formula/

memory/                                    # SMAP shared mind (per CLAUDE.md §6)
  context/ decisions/ discoveries/ handoff/ journal/ blockers/ patterns/ reports/

tasks/swebench-lite/*.json                 # 300-task corpus manifest (KEEP)
]]>
</directory_structure>

<coding_standards>
  <naming_conventions>
    <files>snake_case Rust files; one logical unit per file</files>
    <crates>kebab-case crate names: context-graph-mejepa-* convention</crates>
    <types>PascalCase types; SCREAMING_SNAKE consts; snake_case functions/vars</types>
    <error_codes>SCREAMING_SNAKE error code tags: MEJEPA_OBSERVE_BATCH_NAN, MEJEPA_INFER_OOD_REFUSE, etc.</error_codes>
    <mcp_tools>snake_case mcp__cgreality__mejepa_*: mejepa_verify, mejepa_predict_latest, mejepa_observe_shift, mejepa_subscriber_status, mejepa_capture_audit</mcp_tools>
    <mcp_tool_namespace_lock>All MCP tools added by ME-JEPA SHALL be prefixed `mcp__cgreality__mejepa_*`. The retired `mcp__cgreality__reality_run_attempt` and friends MUST emit fail-closed `CCREALITY_ENGINE_RETIRED` and MUST NOT be reintroduced as compatibility shims.</mcp_tool_namespace_lock>
    <rocksdb_cfs>SCREAMING_SNAKE column-family names with CF_ prefix: CF_MEJEPA_PANELS, CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_CONSTELLATION, etc.</rocksdb_cfs>
  </naming_conventions>
  <file_organization>
    <rule>Files under 500 lines (per CLAUDE.md §4); split via file-length-manager when exceeded</rule>
    <rule>Tests co-located: `<unit>_tests.rs` or inline `#[cfg(test)] mod tests`</rule>
    <rule>FSV harness binaries under `examples/<name>_fsv.rs` per crate</rule>
    <rule>One Instrument trait impl per file under context-graph-mejepa-instruments/src/instruments/</rule>
    <rule>Source-of-truth file paths in errors and recommendations always start with `file:` (or `sqlite:`/`git:`)</rule>
  </file_organization>
  <error_handling>
    <rule>No silent fallback paths; no `unwrap_or(0.5)` on confidence intervals; no mock data in tests</rule>
    <rule>Every fallible operation returns Result&lt;T, ContextGraphError&gt; with a structured error variant carrying a SCREAMING_SNAKE `.code()` tag</rule>
    <rule>Robust error logging: every failure logs context (input shapes, file paths, SHAs) before returning</rule>
    <rule>Fail-closed at boundaries: panel dim mismatch, NaN/Inf in gradients, witness-chain integrity break, missing language toolchain — all halt the operation, never substitute defaults</rule>
  </error_handling>
  <fsv_protocol>
    <rule>Every shipped primitive ships with a Forensic Source Verification (FSV) example binary that writes evidence to /tmp/contextgraph-&lt;crate&gt;-fsv/ and re-reads + asserts before reporting success</rule>
    <rule>Tests use real data (real Python source, real Docker oracle output, real witness-chain bytes) — synthetic only when explicitly justified</rule>
    <rule>Per-language FSV (Python, Rust, C, C++, Java, C#, JS, TS, Ruby, Go, PHP) verifies parser correctness against language toolchain output (py_compile, rustc, gcc, javac, dotnet build, node, tsc, ruby -c, go build, php -l)</rule>
  </fsv_protocol>
  <provenance>
    <rule>Every clean-room reimplementation cites its inspirational source as `// Inspired by ruvnet/RuVector crates/&lt;crate&gt;/src/&lt;file&gt; at HEAD ef5274c2`. Inspiration only — no code copied.</rule>
    <rule>Every persistent artifact (panel, training cert, constellation, verdict) carries provenance metadata: corpus_sha, embedder_versions, frozen_at, code_version</rule>
  </provenance>
</coding_standards>

<anti_patterns>
  <forbidden>
    <item reason="LockedDecision">No vendored ruvnet/agentdb or ruvnet/ruvector code; no path-deps; no npm-deps. Reimplement clean-room only. (memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md)</item>
    <item reason="LockedDecision">No reintroduction of inner-actor LLM (Qwen3.6, Hermes-2-Pro, Ollama, llama.cpp generative inference) without explicit operator authorization. The pivot is intentional. (doc 10 §1)</item>
    <item reason="LockedDecision">No `mcp__cgreality__reality_run_attempt` revival, no `target/release/reality-loop` resurrection — the retired CLI shim must remain fail-closed with CCREALITY_ENGINE_RETIRED. (doc 10 §3)</item>
    <item reason="Generic-Only">No per-task overfit. Every primitive must generalize across all 300 SWE-bench Lite tasks AND across all supported languages. Mental check before merging: would this same primitive be load-bearing for psf__requests-2317 / django__django-12286 / sympy__sympy-13895 and any future repo? If no, reformulate. (CLAUDE.md §11.4)</item>
    <item reason="Reality-Boundary">No edits to: .env / .env.* (anywhere); swebench/** (upstream evaluator); tests/oracle_** (benchmark fixtures); crates/context-graph-benchmark/datasets/swe_bench_*/**; crates/*/src/reward_specs/**; crates/*/src/verifiers/**; *.pem / *.key / **/credentials.json. (CLAUDE.md §8)</item>
    <item reason="Security">No hardcoded API keys, secrets, credentials, .env content in source files</item>
    <item reason="DataIntegrity">No mock embedders, mock oracle outputs, mock witness chain bytes in tests that pass while project is broken. Real data only. (CLAUDE.md §11.9)</item>
    <item reason="Architecture">No silent OOD fallback. When OOD score &gt; 0.5, ME-JEPA refuses to predict and surfaces with `MEJEPA_INFER_OOD_REFUSE`. No `unwrap_or(0.5)` on confidence intervals. (doc 10 §D)</item>
    <item reason="Architecture">No learnable target encoder. ME-JEPA's target panel is produced by frozen externally-defined instruments (zero trainable parameters) — this is the structural collapse-immunity property. Any PR that introduces gradient flow through an instrument is a CRITICAL bug, not a feature. (doc 11 §5.2)</item>
    <item reason="Architecture">No raw-text chunking for code-domain instruments. E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test consume AST-perfect atomic chunks bounded by syntactic entities (function/class/impl/module-decl). (doc 14 §3.2)</item>
    <item reason="Determinism">No random initialization in instrument outputs. Same input + same frozen weights + same code_version = same output, byte-equal. Stochastic operations must be seeded.</item>
    <item reason="Storage">No unbounded growth. Every persistent surface has a budget (§Section 6.1 of doc 09); every column family has a tier policy.</item>
    <item reason="Network">No HTTP from inside ME-JEPA crates. The MCP server is the only network surface; WebSearch / WebFetch are research tools, not action tools. (CLAUDE.md §11.9)</item>
    <item reason="Git">No destructive git commands (push --force, --force-with-lease, reset --hard, checkout --, clean -f, rebase) without explicit operator request.</item>
    <item reason="BackwardsCompat">No backwards-compat shims unless operator explicitly requests. Pre-production; synthetic data only; wipe DB on schema change. Per memory/feedback_no_backwards_compat.md</item>
  </forbidden>
</anti_patterns>

<security_requirements>
  <rule id="SEC-MEJEPA-01">Validate all inputs at MCP tool boundaries (panel dimensionality, finite floats, file path canonicalization). Reject malformed inputs with structured errors.</rule>
  <rule id="SEC-MEJEPA-02">Embedder weight files load from pinned SHA-256 digests in models_config.toml; refuse to load if digest mismatches.</rule>
  <rule id="SEC-MEJEPA-03">Witness-chain entries signed with ed25519 keypair stored in operator-controlled keyring; private key never leaves operator hardware.</rule>
  <rule id="SEC-MEJEPA-04">Constellation centroids loaded with corpus_sha + frozen_at metadata; refuse to use stale centroids beyond a configurable max-age (default 90 days) without operator override.</rule>
  <rule id="SEC-MEJEPA-05">Docker oracle invocations sanitize task_id + run_id (Docker-name-safe regex; reject `+`, spaces, control chars). (per memory/project_phase0_mejepa_mutation_corpus.md)</rule>
  <rule id="SEC-MEJEPA-06">No prompt injection from training corpus into MCP tool responses; CoT artifacts encoded as opaque embeddings via E17, never echoed verbatim.</rule>
  <rule id="SEC-MEJEPA-07">FSV evidence files in /tmp/ created with mode 0o600; never world-readable.</rule>
</security_requirements>

<performance_budgets>
  <metric name="panel_materialization_p50">&lt; 100 ms per attempt with cached instruments (file-sha keyed cache)</metric>
  <metric name="panel_materialization_p99">&lt; 500 ms cold (full AST + CFG + DataFlow + TypeGraph rebuild)</metric>
  <metric name="predictor_forward_p50">&lt; 50 ms / batch (batch=64, RTX 5090, candle-cuda, bf16)</metric>
  <metric name="single_prediction_latency_p50">&lt; 100 ms end-to-end (instruments cached + predictor + Gτ)</metric>
  <metric name="shift_subscriber_observe_latency_p50">&lt; 5 s per shift (per doc 09 §3.8.6)</metric>
  <metric name="shift_subscriber_observe_latency_p99">&lt; 30 s per shift</metric>
  <metric name="vram_steady_state">&lt;= 29 GB on RTX 5090 (21 embedders + predictor + LoRA + activations + KV-cache; per doc 12 §4)</metric>
  <metric name="rocksdb_disk_total_quota">&lt;= 115 GB hard quota; configurable via CG_MEJEPA_DISK_QUOTA_GB (per doc 09 §6.1)</metric>
  <metric name="phase0_corpus_size">2,400 entries (8 categories × 300 SWE-bench Lite tasks); ~5 GB raw</metric>
  <metric name="phase3_training_walltime">&lt; 12 hours for 10 epochs on base 2,400-example corpus, RTX 5090</metric>
  <metric name="conformal_coverage_target">stated 1-α coverage ± 2% empirical (default α=0.10 → coverage 88-92%)</metric>
  <metric name="ood_separation_target">AUC &gt; 0.85 between in-distribution mutation corpus and out-of-distribution synthetic perturbations</metric>
  <metric name="phase8_ship_gate">prediction-oracle correlation ≥ 0.95 stably across all 8 mutation categories AND all supported languages on rolling-window holdout</metric>
</performance_budgets>

<testing_requirements>
  <coverage_minimum>80% line coverage on shipped crates; 90% for safety-critical paths (verify(), observe(), Gτ, witness chain)</coverage_minimum>
  <required_tests>
    <test_type>Unit tests for every instrument's `encode()`</test_type>
    <test_type>Integration tests for panel materialization across all 11 supported languages</test_type>
    <test_type>FSV harness per shipped primitive (Phase 0 corpus, instruments, predictor, training, inference, self-healing, hygiene, subscriber)</test_type>
    <test_type>Per-language oracle round-trip: real source → AST chunker → instruments → panel → predictor → Gτ; oracle outcome compared against actual toolchain (py_compile, rustc, etc.)</test_type>
    <test_type>UTML L_step computation validation: 100 synthetic examples spanning ΔP/ΔK/ΔΩ/ΔΞ corner cases (per doc 11 §11.1)</test_type>
    <test_type>Constellation Gτ false-positive rate validation: ≤5% on held-out KnownGood examples per language</test_type>
    <test_type>Drift-detection drill: inject 70% empirical conformal coverage; system detects within 1,000 observations and triggers A/B/C retrain (per doc 09 §5.2)</test_type>
    <test_type>EWC++ Fisher non-degeneracy: after 10 task boundaries, weight protection has full rank (rank-deficient = bug)</test_type>
    <test_type>Witness-chain end-to-end replay verification on 100,000-entry chain</test_type>
    <test_type>Shift-subscriber latency profiling: p50 &lt; 5s, p99 &lt; 30s on synthetic shift stream</test_type>
  </required_tests>
  <fsv_evidence_paths>
    <path>/tmp/contextgraph-mejepa-corpus-fsv/{mejepa-corpus-evidence.json,split-evidence.json,split-assignments.json}</path>
    <path>/tmp/contextgraph-mejepa-instruments-fsv/{panel.json,e_oracle_*.json,e_witness_*.json,e_static_analysis_*.json}</path>
    <path>/tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json,vicreg-loss-evidence.json}</path>
    <path>/tmp/contextgraph-mejepa-train-fsv/{training-cert-chain.json,utml-l-step-evidence.json}</path>
    <path>/tmp/contextgraph-mejepa-tct-fsv/{constellation-evidence.json,gtau-evidence.json}</path>
    <path>/tmp/contextgraph-mejepa-shift-subscriber-fsv/{watermark.json,observe-evidence.json}</path>
    <path>/tmp/contextgraph-rust-ast-chunker-fsv/ast-chunker-evidence.json (Rust)</path>
    <path>/tmp/contextgraph-python-ast-chunker-fsv/ast-chunker-evidence.json (Python — Phase 1)</path>
    <path>/tmp/contextgraph-{c,cpp,java,csharp,go,js,ts,ruby,php}-ast-chunker-fsv/ast-chunker-evidence.json (Phase 1 extension)</path>
  </fsv_evidence_paths>
</testing_requirements>

<phase_plan_summary>
The 8-phase ME-JEPA build plan (per doc 09 §7) is the supreme scheduling authority. Each phase has
a deliverable, definition of done, and FSV verification protocol. Functional and technical specs
are stratified by phase. Tasks generated within each phase obey Inside-Out, Bottom-Up layering
(foundation → logic → surface) per prdtospec.md §4.1.

  Phase 0 — Workspace + corpus generation                  (~5-7 days, partial; CLI + one-instance Docker FSV shipped, full 300x8 run pending)
  Phase 1 — Instruments + multi-language AST chunking      (~25 days incl. 21-embedder integration, partial; all deterministic slots shipped, deep integrations pending)
  Phase 2 — Panel + predictor + frozen target + VICReg     (~7-10 days, deferred)
  Phase 3 — Training loop + UTML L_step + per-step certs   (~5-7 days, deferred)
  Phase 4 — Inference + conformal calibration + OOD        (~3-5 days, deferred)
       4b — TCT constellation + Gτ guard (cross-cutting)   (~5 days, deferred)
  Phase 5 — Self-healing + EWC++ + drift + A/B/C           (~5-7 days, deferred)
  Phase 6 — Disk + memory hygiene + tier compression       (~3-5 days, deferred; tier-compression ✅ shipped)
  Phase 7 — Reality-loop integration + MCP tools           (~5-7 days, deferred)
  Phase 8 — Evaluation against ≥0.95 ship gate             (ongoing)
</phase_plan_summary>

<utml_v1_invariants>
  <rule>L_step = ΔP × ΔK × ΔΩ × ΔΞ; all factors clamped to [0, 1]; multiplicative gating non-negotiable (doc 11 §1)</rule>
  <rule>ΔΞ.target_collapse = 0 by construction for ME-JEPA (frozen externally-defined instruments). Any code path that produces ΔΞ.target_collapse &gt; 0 is a critical bug — an instrument has accidentally become trainable. Fix at the trait level. (doc 11 §5.2, §11.2 edge-case audit)</rule>
  <rule>Coaching order: regulate ΔΩ and ΔΞ FIRST, then adjust ΔP or ΔK (doc 11 §9). The training cycle assesses ΔΩ + ΔΞ before computing ΔP / ΔK.</rule>
  <rule>Per-step training certificate (CF_MEJEPA_TRAIN_CERTS) records all four factors + components + sampling_weight + grad_norm + drift_coverage + merkle_root (doc 11 §11)</rule>
  <rule>Pruning rule: prune curriculum item only if L_step &lt; 0.1 for N steps AND ΔΩ &gt; 0.4 throughout AND ΔΞ &gt; 0.4 throughout. All three must agree. (doc 11 §10)</rule>
</utml_v1_invariants>

<tct_invariants>
  <rule>The 21 frozen embedders serve THREE simultaneous roles without parameter update: labeller (Phase 1 centroid construction), loss target (Phase 3 L_constellation_centroid), runtime guard (Phase 4 Gτ). (doc 13 §3.4)</rule>
  <rule>Constellation centroids are FROZEN once constructed. Updates require explicit operator action with provenance (corpus_sha + embedder_versions + frozen_at). (doc 13 §3.1)</rule>
  <rule>Gτ test is deterministic O(N · d_max), no learned model, re-runnable by anyone with the frozen embedders + the constellation. (doc 13 §3.3)</rule>
  <rule>Per-chunk-type localization: GuardRejected verdicts include violating_chunks: Vec&lt;ChunkId&gt; so rejection is localized to specific function/class/impl/module-decl, not just the whole patch. (doc 14 §8.1)</rule>
  <rule>Per-chunk-type cell math: 8 mutation categories × 13 entity types × 11 languages × 21 embedders = 24,024 cells. Bayesian shrinkage (per doc 13 §10 risk #5) collapses cells with N &lt; 50 supporting examples to language-aggregate or entity-aggregate centroid.</rule>
</tct_invariants>

<reality_loop_contract>
  <rule>Per CLAUDE.md §11.2, every Claude Code session in this repo is one outer-loop turn doing: (1) ME-JEPA implementation work, (2) maintenance, or (3) self-validation. Each turn produces durable reality artifacts (shift-log, witness-chain, test outcomes, recommendation JSONs).</rule>
  <rule>Stop-hook contract: every turn must call `mcp__cgreality__optimizer_record_recommendation`. Recommendation contents describe ME-JEPA implementation deltas. SMAP report at memory/reports/agent-{N}-{role}--final-report.md is honor-system but mandatory by team discipline.</rule>
  <rule>The `mejepa-shift-subscriber` (Phase 7 §3.8) consumes shift-log + witness-chain artifacts and feeds mejepa.observe(). Latency budget: &lt; 5s p50, &lt; 30s p99. (doc 09 §3.8.6)</rule>
  <rule>One test at a time: per /loop turn, scope is one SWE-bench Lite task (or one ME-JEPA primitive when not in attempt mode). Partial-progress signals still feed ME-JEPA gold supervision. (doc 10 Section A)</rule>
</reality_loop_contract>

<failure_taxonomy>
  <error_code id="MEJEPA_OBSERVE_BATCH_NAN">Gradient or activation went non-finite during observe(); rollback to prior checkpoint; surface to operator</error_code>
  <error_code id="MEJEPA_OBSERVE_DRIFT_DETECTED">Empirical conformal coverage &lt; 0.85; severity ≥ hard; trigger A/B/C retrain</error_code>
  <error_code id="MEJEPA_OBSERVE_INTEGRITY_VIOLATION">Witness-chain replay broken; halt inference; restore from last verified checkpoint</error_code>
  <error_code id="MEJEPA_INFER_OOD_REFUSE">OOD score &gt; 0.5; refusing to predict; fail-closed</error_code>
  <error_code id="MEJEPA_INFER_DIM_MISMATCH">Panel dimensionality wrong; reject input; do not silently truncate or pad</error_code>
  <error_code id="MEJEPA_INFER_INSTRUMENT_MISSING">Required instrument unavailable for this language (e.g., Pyright not installed for Python)</error_code>
  <error_code id="MEJEPA_INFER_CONSTELLATION_VIOLATION">Gτ rejected the predicted panel; one or more embedders below threshold; persisted with violating_embedders + violating_chunks</error_code>
  <error_code id="MEJEPA_INFER_GTAU_STALE_CONSTELLATION">Constellation older than max_age; refuse without operator override</error_code>
  <error_code id="MEJEPA_RETRAIN_HOLDOUT_REGRESSION">Mode B / C did not beat Mode A on holdout; do not promote</error_code>
  <error_code id="MEJEPA_RETRAIN_QUOTA_EXCEEDED">Disk / RAM quota hit during retrain; pause; trigger eviction</error_code>
  <error_code id="MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES">&lt; N samples in conformal calibration window; refuse calibration; fall back to last-known threshold</error_code>
  <error_code id="MEJEPA_CHUNKER_PARSE_FAILED">Tree-sitter ERROR or MISSING node in source; emit byte-offset + line + node-kind diagnostics; do NOT fall back to text chunking</error_code>
  <error_code id="MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE">Language not in 11-language matrix; reject fail-closed</error_code>
  <error_code id="MEJEPA_CHUNKER_EMPTY_SOURCE">Empty source; no empty chunks emitted</error_code>
  <error_code id="MEJEPA_CHUNKER_MIXED_LANGUAGE">Embedded language detected (HTML+JS, PHP+HTML); reject for now; future work routes per-block</error_code>
  <error_code id="MEJEPA_CORPUS_INVALID_INPUT">Source has parse errors; mutation operators refuse</error_code>
  <error_code id="MEJEPA_CORPUS_OPERATOR_FAILED">Mutation site bounds invalid or original_text mismatch; refuse</error_code>
  <error_code id="CCREALITY_ENGINE_RETIRED">Retired tool surface called (reality_run_attempt etc.); fail-closed; do not spawn stale binaries</error_code>
</failure_taxonomy>

<spec_authority_order>
  When two specs conflict, resolve in this order:
  1. constitution.md (this file)
  2. doc 10 (post-pivot architecture) for inner-actor / single-loop questions
  3. doc 09 (master plan) for everything else
  4. doc 11 (UTML formula) for training math
  5. doc 12 (embedder integration) for instrument substrate
  6. doc 13 (TCT) for constellation + Gτ
  7. doc 14 (AST chunking) for chunker boundaries
  8. functional/&lt;domain&gt;.md
  9. technical/&lt;domain&gt;.md
  10. tasks/&lt;phase&gt;/&lt;TASK-DOMAIN-NNN&gt;.md
  11. CLAUDE.md (operator role + reality-loop contract)
</spec_authority_order>

</constitution>
```
