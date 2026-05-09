# SPEC-INSTRUMENTS — ME-JEPA-Code Instruments + 5,120-d Panel

> **Level 2 — Functional Specification.** Defines WHAT the instruments domain must do, not HOW. Implementation details live in `TECH-INSTRUMENTS.md`. Conflicts with the constitution resolve to the constitution.

```xml
<functional_spec id="SPEC-INSTRUMENTS" version="1.0">

<metadata>
  <title>ME-JEPA-Code Instruments + 5,120-d Panel</title>
  <status>draft</status>
  <owner>Phase 1 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-CORPUS</spec_ref>            <!-- Phase 0 mutation corpus + oracle bridge -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>      <!-- 11-language AST-perfect chunker (doc 14) -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>         <!-- 21-embedder ensemble (doc 12) -->
    <spec_ref>SPEC-WITNESS</spec_ref>           <!-- 73-byte SHAKE-256 chain (shipped) -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>         <!-- Phase 2 panel→panel predictor (deferred) -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2 (instrument inventory), §2.3 (panel time-step protocol), §3.5 (E_StaticAnalysis / E_Runtime / E_Reasoning), §3.5.5 (5,120-d panel layout)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §A (single-loop architecture; no inner-actor LLM)</source>
    <source type="design">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §3 (instrument-embedder mapping)</source>
    <source type="design">docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §3.4 (frozen-instrument three-role property)</source>
    <source type="design">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §3.2, §4 (AST-perfect chunk consumption)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants, anti_patterns frozen-instrument rule, failure_taxonomy)</source>
    <source type="shipped_code">crates/context-graph-mejepa-instruments/src/{lib.rs, e_oracle.rs, e_witness.rs, e_static_analysis.rs}</source>
    <source type="empirical_fsv">/tmp/contextgraph-mejepa-instruments-fsv/{panel.json, e_oracle_*.json, e_witness_*.json, e_static_analysis_*.json}</source>
    <source type="memory">memory/project_phase0_mejepa_mutation_corpus.md (Phase 1 scaffolding + E_Oracle shipped 2026-05-09d; FSV totals)</source>
  </source_documents>
</metadata>

<overview>
The instruments domain produces the **5,120-d panel** — the joint embedding that ME-JEPA-Code's
predictor consumes as both input (panel[t=0], panel[t=1]) and target (panel[t=2]). The panel is
the IR (intermediate representation) of the reality compiler analogy in doc 09 §1.1: source code
goes in, an instrument-by-instrument latent representation comes out, and the predictor learns
to forecast the post-execution panel from the pre-execution context.

This spec defines **15 panel slots** (14 named instruments + a 128-d scalar feature aggregator)
with byte-precise offsets and dimensions summing to exactly 5,120. Three slots are already
shipped clean-room (E_Oracle 128-d, E_Witness 256-d, E_StaticAnalysis 256-d); the remaining
twelve slots are deferred Phase 1 deliverables. Each instrument is independently testable, frozen
during ME-JEPA training (per doc 11 §5.2: target-side collapse is structurally prevented), and
required to honor the same fail-closed `Instrument` trait contract.

The panel is materialized at **three time steps per attempt** per doc 09 §2.3:
- `panel[t=0]` — pre-patch state (only structural slots populated; E_Diff / E_Trace / E_Witness
  partial / E_Oracle / E_CommitMsg are zero by construction).
- `panel[t=1]` — post-patch, pre-execution (E_Diff and E_CommitMsg populated; E_Trace zero;
  E_Witness has the patch-application entries; E_Oracle still zero).
- `panel[t=2]` — post-execution (every slot populated, including E_Oracle with the verdict).

Beneficiaries: (1) the Phase 2 predictor learns from `(panel[t=0], panel[t=1]) -> panel[t=2]`;
(2) the Phase 4b TCT constellation Gτ guard reads frozen instrument outputs to enforce per-chunk
typological consistency; (3) the Phase 7 `mejepa-shift-subscriber` materializes panels in real
time from outer-loop edit/test cycles. All three uses depend on the same panel contract defined
here.
</overview>

<user_types>

  <user_type id="UT-01" name="ME-JEPA Predictor">
    <description>The Phase 2 6-layer transformer trained to map (panel[t=0], panel[t=1]) -> panel[t=2]. Reads instrument outputs as frozen targets.</description>
    <permissions>READ panel slots; READ frozen instrument trait outputs; CANNOT write panels (predictor outputs are not stored as ground-truth panels).</permissions>
    <goals>Receive deterministic, byte-equal panel reads across calls so VICReg loss does not regress to noise.</goals>
  </user_type>

  <user_type id="UT-02" name="TCT Constellation Builder">
    <description>The Phase 4b machinery that constructs centroids per (mutation_category × entity_type × language × embedder_id) and runs Gτ guard at inference time per doc 13 §3.</description>
    <permissions>READ instrument outputs in batch over the corpus; READ panel slots for held-out evaluations; CANNOT mutate instrument weights.</permissions>
    <goals>Use frozen instrument outputs as both labellers (centroid construction) and runtime guard (per-chunk Gτ test).</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 7 Shift-Subscriber">
    <description>The Tokio task that watches the harness shift log + witness chain and materializes panels in real time per doc 09 §3.8.</description>
    <permissions>READ instrument trait; READ shift-log entries and chain bytes; WRITE materialized Panels into RocksDB CF_MEJEPA_PANELS.</permissions>
    <goals>Materialize panels at &lt; 5 s p50 / &lt; 30 s p99 latency from shift event.</goals>
  </user_type>

  <user_type id="UT-04" name="Phase 0/1 Corpus Materializer">
    <description>The batch process that walks the 2,400-example mutation corpus and emits (panel[t=0], panel[t=1], panel[t=2]) tuples per (task, mutation_category).</description>
    <permissions>READ instrument trait; READ AST-perfect chunks; READ Docker oracle verdicts; WRITE CF_MEJEPA_PANELS.</permissions>
    <goals>Materialize the full corpus deterministically — same input + same frozen weights + same code_version = byte-equal panel.</goals>
  </user_type>

  <user_type id="UT-05" name="Reality-Loop Optimizer (Claude Code)">
    <description>The outer-loop agent producing patches, transcripts, recommendation JSONs, and reflexion records that feed E_Reasoning and E_CommitMsg.</description>
    <permissions>WRITE shift-log entries (consumed downstream); WRITE recommendation JSONs (consumed by E_Reasoning); never directly invokes instruments — the subscriber does.</permissions>
    <goals>Produce durable artifacts that the instruments can encode without ambiguity.</goals>
  </user_type>

  <user_type id="UT-06" name="Operator (human)">
    <description>The human running the system; reviews instrument FSV evidence; makes promote/rollback calls; triggers retrain modes.</description>
    <permissions>READ all FSV evidence; READ shipped instrument source; APPROVE additions of new slots (constitution change required); CANNOT silently mutate frozen weights.</permissions>
    <goals>Verify instrument outputs are deterministic, dim-correct, and finite before any predictor read.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-INSTR-01" priority="must-have">
    <narrative>
      <as_a>ME-JEPA Predictor (UT-01)</as_a>
      <i_want_to>Read a 5,120-d Panel built from 15 slot-typed instrument outputs with byte-precise offsets</i_want_to>
      <so_that>VICReg L_predict / L_var / L_cov / L_inv compute over a stable IR without alignment drift</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-01-01">
        <given>A Panel built via PanelBuilder with all 15 slots filled</given>
        <when>The predictor reads `panel.data()`</when>
        <then>Length == 5,120; every value is finite; slot extents per `InstrumentSlot::layout()` match the canonical table (E_AST 0..384, E_CFG 384..640, E_DataFlow 640..896, E_TypeGraph 896..1152, E_Test 1152..1536, E_Trace 1536..2048, E_Diff 2048..2304, E_Witness 2304..2560, E_Oracle 2560..2688, E_Problem 2688..3712, E_CommitMsg 3712..4096, E_StaticAnalysis 4096..4352, E_Runtime 4352..4608, E_Reasoning 4608..4992, Scalars 4992..5120)</then>
      </criterion>
      <criterion id="AC-INSTR-01-02">
        <given>Two materializations of the same (task_id, mutation_category, time_step) input</given>
        <when>Each is encoded with the same code_version and the same frozen embedder_versions</when>
        <then>The resulting Panel.data() byte arrays are byte-equal (SHA-256 identical)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INSTR-02" priority="must-have">
    <narrative>
      <as_a>Phase 0/1 Corpus Materializer (UT-04)</as_a>
      <i_want_to>Materialize panel[t=0], panel[t=1], panel[t=2] for each of the 2,400 corpus entries</i_want_to>
      <so_that>The Phase 2 predictor has training tuples (context → target) with the E_Oracle ground-truth verdict in panel[t=2]</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-02-01">
        <given>A corpus entry (task_id, mutation_category, mutated_source, oracle_verdict)</given>
        <when>The materializer requests panel[t=0]</when>
        <then>E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Problem are populated; E_Diff, E_Trace, E_Witness, E_Oracle, E_CommitMsg are zero by construction; filled_mask reflects exactly which slots were explicitly set</then>
      </criterion>
      <criterion id="AC-INSTR-02-02">
        <given>A corpus entry where the Docker oracle verdict reports a category-8 (compile-error) outcome</given>
        <when>The materializer requests panel[t=2]</when>
        <then>E_Oracle's `has_exception` flag is 1.0; one of the 10 exception-class one-hot dims is 1.0; total test count dim is set; the panel passes Panel::try_new validation</then>
      </criterion>
      <criterion id="AC-INSTR-02-03">
        <given>An attempt that fails on a language with missing toolchain (e.g., Pyright not installed for Python; rust-analyzer absent for Rust)</given>
        <when>E_TypeGraph is invoked</when>
        <then>The instrument fails closed with `MEJEPA_INFER_INSTRUMENT_MISSING`; the materializer records the failure in CF_MEJEPA_PANEL_META and does NOT silently substitute zeros</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INSTR-03" priority="must-have">
    <narrative>
      <as_a>TCT Constellation Builder (UT-02)</as_a>
      <i_want_to>Use frozen instrument outputs as labellers for centroid construction AND as runtime Gτ guard targets</i_want_to>
      <so_that>The same 21 frozen embedders serve three roles (per doc 13 §3.4) without parameter update</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-03-01">
        <given>A code-domain instrument (E_AST / E_CFG / E_DataFlow / E_TypeGraph / E_Diff / E_Test) that internally calls one of the 21 embedders</given>
        <when>The instrument is invoked during training (under autograd) AND during constellation construction (no autograd) AND during Gτ guard at inference</when>
        <then>The embedder's parameters carry zero gradient in all three contexts; any gradient detected through a frozen embedder is reported as `MEJEPA_INSTR_FROZEN_VIOLATION` (CRITICAL bug per doc 11 §5.2 / constitution utml_v1_invariants rule 2)</then>
      </criterion>
      <criterion id="AC-INSTR-03-02">
        <given>Two runs of the same instrument on the same input separated by 30 days of training</given>
        <when>The instrument is invoked</when>
        <then>The output is byte-equal (frozen instruments do not drift over training)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INSTR-04" priority="must-have">
    <narrative>
      <as_a>Phase 7 Shift-Subscriber (UT-03)</as_a>
      <i_want_to>Materialize panels from real-time outer-loop shift events</i_want_to>
      <so_that>Every Claude Code edit/test cycle becomes labeled training data within 5 s p50</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-04-01">
        <given>A shift-log entry recording a code edit + a subsequent test outcome</given>
        <when>The subscriber materializes panel[t=2]</when>
        <then>End-to-end materialization (instrument cache hit + AST chunk reuse + E_Oracle population) completes in &lt; 5 s p50 / &lt; 30 s p99 (per constitution performance_budgets)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INSTR-05" priority="must-have">
    <narrative>
      <as_a>Operator (UT-06)</as_a>
      <i_want_to>Verify each instrument's output is deterministic, dim-correct, and finite via FSV evidence</i_want_to>
      <so_that>I can promote a new instrument from "shipped" to "production" without trusting tests alone</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-05-01">
        <given>A newly shipped instrument E_X</given>
        <when>The FSV harness binary runs</when>
        <then>It writes evidence to /tmp/contextgraph-mejepa-instruments-fsv/e_x_{all_pass,mixed,crash_assertion_error,crash_type_error}_{input,encoded}.json (4 boundary cases × 2 = 8 files); re-reads each file; asserts dim==slot.dim, finite, deterministic across two encodes; reports pass/fail per file</then>
      </criterion>
      <criterion id="AC-INSTR-05-02">
        <given>The full panel FSV /tmp/contextgraph-mejepa-instruments-fsv/panel.json</given>
        <when>An operator inspects it</when>
        <then>The file deserializes via `serde_json::from_str::<Panel>`; the resulting Panel passes Panel::try_new validation; total_dim == 5,120; every slot's offset+dim matches the canonical layout table</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INSTR-06" priority="should-have">
    <narrative>
      <as_a>ME-JEPA Predictor (UT-01)</as_a>
      <i_want_to>Distinguish the 8 mutation categories using only the latents of a single instrument</i_want_to>
      <so_that>Phase 1 Definition-of-Done can be empirically demonstrated (per doc 09 §7 Phase 1 DoD)</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INSTR-06-01">
        <given>100 mutation-corpus examples spanning all 8 categories</given>
        <when>A simple linear probe is trained on a single instrument's output to predict the category</when>
        <then>Top-1 accuracy on a holdout split ≥ 70% (per the Phase 1 DoD round-trip requirement in the task brief)</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <!-- ============ Per-instrument requirements ============ -->

  <requirement id="REQ-INSTR-01" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate `context-graph-mejepa-instruments` SHALL expose an `EAstInstrument` that consumes
      AST-perfect atomic chunks from `crates/context-graph-core/src/memory/ast_chunker.rs` (any of
      the 11 supported tree-sitter languages: rust, python, javascript, typescript, go, java, c,
      cpp, c-sharp, ruby, php) and emits a fixed 384-d latent. The instrument SHALL be backed by
      embedder E7 (Qodo-Embed-1-1.5B, 1536-d) projected 1536→384, OR an alternative AST-token
      transformer reading the ast_chunker's `(language, entity_type, parent_chain, line_start,
      line_end, sha256)` metadata. The instrument SHALL be FROZEN (zero trainable parameters).
    </description>
    <rationale>
      Per doc 12 §3 row E_AST, doc 14 §3.2, constitution anti_patterns rule
      "no raw-text chunking for code-domain instruments", and constitution utml_v1_invariants
      rule 2. AST-perfect chunking is the precondition for E_AST signal-vs-noise.
    </rationale>
    <constraints>
      - Input: a list of AST-perfect chunks (NOT raw source text)
      - Output dim: exactly 384 f32; finite
      - Latency budget: &lt; 20 ms cold per chunk (per task brief budgets section)
      - Frozen: zero trainable parameters; gradient-detection asserts
      - Languages: any of the 11 wired tree-sitter languages
    </constraints>
    <verification_method>
      Shipped FSV writes /tmp/contextgraph-mejepa-instruments-fsv/e_ast_{all_pass,mixed,crash_assertion_error,crash_type_error}_{input,encoded}.json across all 11 languages; round-trip linear-probe on 100-example corpus ≥ 70% top-1 accuracy.
    </verification_method>
  </requirement>

  <requirement id="REQ-INSTR-02" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ECfgInstrument` that consumes AST-perfect chunks of patched
      functions, extracts a control-flow graph via tree-sitter (basic blocks + edges), and emits a
      fixed 256-d latent. Backed by embedder E8 (Graph) plus a graph-attention head over basic
      blocks. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_CFG; doc 09 §2.2 row 2.</rationale>
    <constraints>
      - Input: AST-perfect chunks bounded by function/method/impl entities
      - Output dim: exactly 256 f32; finite
      - Latency budget: &lt; 20 ms cold; &lt; 5 ms warm
      - Frozen
    </constraints>
    <verification_method>FSV evidence per AC-INSTR-05-01; graph-attention determinism check (same CFG → same latent).</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-03" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EDataFlowInstrument` that consumes AST-perfect chunks plus
      def-use edges (extracted by language-specific dataflow analyzers), and emits a fixed 256-d
      latent. Backed by embedder E11 (Kepler RoBERTa+TransE) plus graph-attention over variable
      lifetimes. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_DataFlow; doc 09 §2.2 row 3.</rationale>
    <constraints>
      - Input: AST-perfect chunks + def-use edges
      - Output dim: exactly 256 f32; finite
      - Latency budget: &lt; 20 ms cold
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-04" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ETypeGraphInstrument` that consumes AST-perfect chunks plus type
      diagnostics from the language toolchain (Pyright/mypy for Python; rust-analyzer for Rust;
      javac for Java; dotnet build for C#; tsc for TypeScript; etc.) and emits a fixed 256-d
      latent. Backed by E11 Kepler. The instrument SHALL be FROZEN. The instrument SHALL fail
      closed with `MEJEPA_INFER_INSTRUMENT_MISSING` when the language toolchain is unavailable.
    </description>
    <rationale>doc 12 §3 row E_TypeGraph; constitution failure_taxonomy rule MEJEPA_INFER_INSTRUMENT_MISSING; doc 09 §2.2 row 4.</rationale>
    <constraints>
      - Input: AST-perfect chunks + type diagnostics from real toolchain
      - Output dim: exactly 256 f32; finite
      - Latency budget: &lt; 20 ms cold (excluding toolchain invocation)
      - Frozen
      - Fails closed on missing toolchain (no parser-only fallback)
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01; per-language toolchain-missing test triggers MEJEPA_INFER_INSTRUMENT_MISSING.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-05" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ETestInstrument` that consumes AST-perfect chunks of test
      functions plus their natural-language descriptions (docstrings, names, assertion messages)
      and emits a fixed 384-d latent. Backed by embedder E1 (e5-large-v2). The instrument SHALL
      be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_Test; doc 09 §2.2 row 5.</rationale>
    <constraints>
      - Input: AST-perfect chunks bounded by test functions
      - Output dim: exactly 384 f32
      - Latency budget: &lt; 5 ms (E1-derived)
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-06" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ETraceInstrument` that consumes a sequence of execution-trace
      events `(fn_id, line_no, val_hash, timestamp_ns)` and emits a fixed 512-d latent. Backed by
      temporal embedders E2/E3/E4 plus a sequence transformer over trace events. The instrument
      SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_Trace; doc 09 §2.2 row 6; doc 09 §3.5.2 (runtime signals extension).</rationale>
    <constraints>
      - Input: typed sequence of TraceEvent records (max 1M events; reject larger fail-closed)
      - Output dim: exactly 512 f32
      - Latency budget: &lt; 50 ms cold (sequence transformer over up-to-1M events)
      - Frozen
      - Empty trace input is fail-closed (no silent zero-fill)
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01; empty-trace boundary case yields MEJEPA_INSTRUMENTS_INVALID_INPUT.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-07" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EDiffInstrument` that consumes a (pre-patch chunk set, post-patch
      chunk set) pair plus computed AST-diff edges (added/removed/modified node events) and emits
      a fixed 256-d latent. Backed by embedder E7 (Qodo) plus a diff-edge encoder. The instrument
      SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_Diff; doc 09 §2.2 row 7.</rationale>
    <constraints>
      - Input: AST-perfect (before, after) chunk pair + diff edge set
      - Output dim: exactly 256 f32
      - Latency budget: &lt; 20 ms cold
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-08" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EWitnessInstrument` (✅ shipped at
      `crates/context-graph-mejepa-instruments/src/e_witness.rs`) that consumes verified 73-byte
      SHAKE-256 witness chain entries via `WitnessChainInput` and emits a fixed 256-d latent. The
      instrument SHALL fail closed if `verify_chain_bytes` rejects the input chain. The
      instrument SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_Witness; doc 09 §2.2 row 8; constitution failure_taxonomy MEJEPA_OBSERVE_INTEGRITY_VIOLATION + format-version invariant from doc 09 §3.6 blindspot 17.</rationale>
    <constraints>
      - Input: WitnessChainInput { format_version: u8, chain_bytes: Vec&lt;u8&gt; }
      - Output dim: exactly 256 f32
      - Format version: rejects any version != CANONICAL_WITNESS_FORMAT_VERSION
      - Latency budget: &lt; 10 ms warm
      - Frozen
    </constraints>
    <verification_method>Already FSV-covered: /tmp/contextgraph-mejepa-instruments-fsv/e_witness_*.json (4 boundary cases shipped 2026-05-09d per memory/project_phase0_mejepa_mutation_corpus.md).</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-09" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EOracleInstrument` (✅ shipped at
      `crates/context-graph-mejepa-instruments/src/e_oracle.rs`) that consumes an `OracleVerdict`
      (per-test outcomes + optional top-level exception class) and emits a fixed 128-d latent
      with the layout: dims 0..4 = pass/fail/skip/error fractions, dim 4 = total test count,
      dim 5 = has_exception flag, dims 6..16 = 10-class one-hot exception class, dims 16..128 =
      reserved zero-padding. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 09 §2.2 row 9; doc 09 §3.5.2 stack-trace embedding extension reserved in dims 16..128.</rationale>
    <constraints>
      - Input: OracleVerdict
      - Output dim: exactly 128 f32
      - Latency budget: &lt; 1 ms (scalar aggregator)
      - Frozen
    </constraints>
    <verification_method>Already FSV-covered (4 boundary cases shipped 2026-05-09d).</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-10" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EProblemInstrument` that consumes the natural-language SWE-bench
      problem statement (or generic problem text) and emits a fixed 1024-d latent. Backed by E1.
      The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_Problem; doc 09 §2.2 row 10.</rationale>
    <constraints>
      - Input: TextInstrumentInput containing the problem statement
      - Output dim: exactly 1024 f32
      - Latency budget: &lt; 5 ms (E1)
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-11" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ECommitMsgInstrument` that consumes the candidate fix's commit
      message text and emits a fixed 384-d latent. Backed by E1. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 12 §3 row E_CommitMsg; doc 09 §2.2 row 11.</rationale>
    <constraints>
      - Input: TextInstrumentInput containing commit message
      - Output dim: exactly 384 f32
      - Latency budget: &lt; 5 ms (E1)
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-12" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EStaticAnalysisInstrument` (✅ shipped at
      `crates/context-graph-mejepa-instruments/src/e_static_analysis.rs`) that consumes a
      `StaticAnalysisInput` (source text + Vec&lt;Diagnostic&gt; from any language toolchain +
      churn_30d) and emits a fixed 256-d latent encoding linter/typecheck/source-shape signals
      per doc 09 §3.5.1 (linter warnings, type completeness, complexity, BFS depth, blame age,
      symbol churn, docstring coverage). The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 09 §3.5.1; doc 12 §3 row E_StaticAnalysis.</rationale>
    <constraints>
      - Input: StaticAnalysisInput (max 100,000 diagnostics; reject larger)
      - Output dim: exactly 256 f32
      - Latency budget: &lt; 1 ms (scalar aggregator)
      - Frozen
    </constraints>
    <verification_method>Already FSV-covered (shipped 2026-05-09d).</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-13" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `ERuntimeInstrument` that consumes runtime aggregates (walltime,
      peak RSS, allocation count, syscall histogram, coverage Δ, filesystem-effects count,
      network-call count, exit codes per test, determinism flag from 3 reruns) and emits a fixed
      256-d latent per doc 09 §3.5.2. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 09 §3.5.2; doc 12 §3 row E_Runtime.</rationale>
    <constraints>
      - Input: RuntimeInput (typed scalar aggregate struct)
      - Output dim: exactly 256 f32
      - Latency budget: &lt; 1 ms (scalar aggregator)
      - Frozen
      - Stochastic signals (walltime, RSS) flagged via determinism check
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-14" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose an `EReasoningInstrument` that consumes the outer-loop agent's CoT
      transcript (Claude Code conversation transcript + recommendation JSONs + reflexion records)
      and emits a fixed 384-d latent. Backed by embedder E17 (AffectText). The instrument SHALL
      be FROZEN. Per constitution SEC-MEJEPA-06, CoT artifacts are encoded as opaque embeddings,
      never echoed verbatim.
    </description>
    <rationale>doc 09 §3.5.3; doc 12 §3 row E_Reasoning; constitution SEC-MEJEPA-06 (no prompt injection from training corpus into MCP tool responses).</rationale>
    <constraints>
      - Input: ReasoningInput (Vec&lt;ReasoningEvent&gt;; max 10,000 events; reject larger)
      - Output dim: exactly 384 f32
      - Latency budget: &lt; 10 ms (E17-derived)
      - Frozen
      - Output is opaque — must not allow CoT text reconstruction from the latent
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <!-- ============ Aggregation requirements ============ -->

  <requirement id="REQ-INSTR-15" story_ref="US-INSTR-01" priority="must">
    <description>
      The crate SHALL expose `Panel` and `PanelBuilder` (✅ shipped at
      `crates/context-graph-mejepa-instruments/src/lib.rs`). PanelBuilder SHALL fail-closed on:
      (a) vector length != slot.dim, (b) any non-finite element in a slot vector, (c) the slot
      already filled (double-fill), (d) building with zero filled slots, (e) malformed persisted
      state (panel.data.len() != PANEL_DIM, unknown slot bits in filled_mask, NaN/Inf in
      panel.data). Panel SHALL serialize to JSON via the `try_from = "PanelSerde"` invariant
      gate, so deserialization always re-runs full validation. PANEL_DIM SHALL be exactly 5,120
      and SHALL match the sum of all 15 slot dims.
    </description>
    <rationale>doc 09 §3.5.5 panel layout; constitution failure_taxonomy MEJEPA_INFER_DIM_MISMATCH.</rationale>
    <constraints>
      - PANEL_DIM = 5,120 (compile-time constant)
      - 15 slots in canonical order (per InstrumentSlot::layout())
      - filled_mask is u16; bits indexed by InstrumentSlot::all() position
      - try_from gate runs validate_panel_parts on every JSON deserialize
    </constraints>
    <verification_method>27 tests already pass on shipped Panel/PanelBuilder (per memory/project_phase0_mejepa_mutation_corpus.md). Continued: full panel.json FSV deserializes round-trip.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-16" story_ref="US-INSTR-01" priority="must">
    <description>
      The crate SHALL expose a `ScalarsInstrument` that consumes a typed `ScalarInput` aggregate
      (BFS depth from touched files, blame age per touched line, symbol churn, cyclomatic
      complexity Δ, coverage Δ, repo-health metrics: commit cadence + CI green-rate +
      contributor count) and emits a fixed 128-d latent. The instrument SHALL be FROZEN.
    </description>
    <rationale>doc 09 §3.5.5 row 5 (Scalar feature aggregates 128-d); doc 09 §3.5.1 (BFS depth, blame age, churn, complexity, coverage Δ).</rationale>
    <constraints>
      - Input: ScalarInput (typed scalar struct)
      - Output dim: exactly 128 f32
      - Latency budget: &lt; 1 ms (scalar aggregator)
      - Frozen
    </constraints>
    <verification_method>FSV per AC-INSTR-05-01.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-17" story_ref="US-INSTR-02" priority="must">
    <description>
      The crate SHALL expose a `materialize_panel(time_step, attempt_id, ...) -> Panel` operation
      that builds a panel for one of three time steps {t=0, t=1, t=2} with the following
      population rules per doc 09 §2.3:
      - t=0: E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Problem populated; others zero.
      - t=1: t=0 slots re-evaluated on patched code; E_Diff, E_CommitMsg populated; E_Witness has
        patch-application entries; E_Trace, E_Oracle remain zero.
      - t=2: every slot populated.
    </description>
    <rationale>doc 09 §2.3 (panel time-step protocol).</rationale>
    <constraints>
      - filled_mask reflects exactly which slots were explicitly set per the time-step rules
      - Same input + same code_version + same embedder_versions → byte-equal Panel
    </constraints>
    <verification_method>Round-trip panel.json FSV across all three time steps for each of the 4 boundary cases.</verification_method>
  </requirement>

  <!-- ============ Cross-cutting requirements ============ -->

  <requirement id="REQ-INSTR-18" story_ref="US-INSTR-03" priority="must">
    <description>
      Every concrete instrument struct SHALL implement the `Instrument` trait with associated
      type `Input` and a single fallible method `encode(&self, input: &Self::Input) ->
      InstrumentResult&lt;Vec&lt;f32&gt;&gt;`. Implementations SHALL contain ZERO trainable parameters.
      Any gradient detected flowing through any instrument's parameters SHALL be reported as
      `MEJEPA_INSTR_FROZEN_VIOLATION` (CRITICAL bug per constitution utml_v1_invariants rule 2 +
      anti_patterns frozen-instrument rule).
    </description>
    <rationale>doc 11 §5.2 target-side collapse-immunity; constitution utml_v1_invariants; constitution anti_patterns.</rationale>
    <constraints>
      - Trait signature exactly matches the shipped definition in lib.rs
      - Frozen verification: training-loop static analysis OR runtime gradient hook detects flow
      - Encode is deterministic: same (input, weights, code_version) → byte-equal output
    </constraints>
    <verification_method>Phase 3 training loop test injects a gradient hook and asserts zero gradient through every instrument's parameters; on violation, error code = MEJEPA_INSTR_FROZEN_VIOLATION.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-19" story_ref="US-INSTR-02" priority="must">
    <description>
      Every code-domain instrument (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test) SHALL
      consume AST-perfect atomic chunks produced by `crates/context-graph-core/src/memory/ast_chunker.rs`.
      Raw text or fixed-size window chunking SHALL be rejected fail-closed.
    </description>
    <rationale>doc 14 §3.2; constitution anti_patterns "no raw-text chunking for code-domain instruments".</rationale>
    <constraints>
      - Input type for code-domain instruments must be a typed AST-chunk struct, not String
      - Languages supported: rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php
    </constraints>
    <verification_method>Per-language FSV across all 11 supported languages; rejection test feeds raw String → MEJEPA_INSTRUMENTS_INVALID_INPUT.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-20" story_ref="US-INSTR-05" priority="must">
    <description>
      Every shipped instrument SHALL ship with a Forensic Source Verification (FSV) example
      binary that writes evidence to /tmp/contextgraph-mejepa-instruments-fsv/ for at minimum 4
      boundary cases (all_pass, mixed, crash_assertion_error, crash_type_error), then re-reads
      and asserts each evidence file before reporting success. FSV files SHALL be created with
      mode 0o600 per constitution SEC-MEJEPA-07.
    </description>
    <rationale>constitution fsv_protocol; constitution SEC-MEJEPA-07; CLAUDE.md §11 reality-shift principle.</rationale>
    <constraints>
      - Per-boundary-case files: e_x_{case}_input.json, e_x_{case}_encoded.json
      - Aggregate: panel.json showing the full 5,120-d serialized panel
      - File permissions: 0o600
    </constraints>
    <verification_method>FSV harness `cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv` produces evidence; re-reads each file and asserts deserialize success + dim correctness.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-21" story_ref="US-INSTR-06" priority="should">
    <description>
      For each newly shipped instrument, a Phase 1 Definition-of-Done test SHALL train a simple
      linear probe on 100 mutation-corpus examples using ONLY that instrument's output as
      features and SHALL achieve top-1 mutation-category accuracy ≥ 70% on a holdout split.
      Failures below this threshold SHALL be reported as evidence the instrument is producing
      noise rather than signal.
    </description>
    <rationale>doc 09 §7 Phase 1 DoD; round-trip requirement in task brief.</rationale>
    <constraints>
      - 100 examples spanning all 8 mutation categories
      - Linear probe: single sklearn-equivalent / candle linear layer + softmax
      - Holdout: ≥ 20% of examples
      - Threshold: top-1 accuracy ≥ 70%
    </constraints>
    <verification_method>Per-instrument linear-probe binary writes /tmp/contextgraph-mejepa-instruments-fsv/linear_probe_e_x.json with confusion matrix + accuracy.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-22" story_ref="US-INSTR-04" priority="must">
    <description>
      Panel materialization SHALL meet performance budgets per constitution: panel_materialization_p50
      &lt; 100 ms with cached instruments (file-sha keyed cache); panel_materialization_p99
      &lt; 500 ms cold (full AST + CFG + DataFlow + TypeGraph rebuild). Shift-subscriber-driven
      materialization SHALL meet shift_subscriber_observe_latency_p50 &lt; 5 s, p99 &lt; 30 s.
    </description>
    <rationale>constitution performance_budgets.</rationale>
    <constraints>Per-budget metrics measured from FSV traces.</constraints>
    <verification_method>Latency profiling FSV; histogram persisted to /tmp/contextgraph-mejepa-instruments-fsv/latency_p50_p99.json.</verification_method>
  </requirement>

  <requirement id="REQ-INSTR-23" story_ref="US-INSTR-01" priority="must">
    <description>
      Every persisted Panel SHALL carry provenance metadata: `code_version`, `embedder_versions`
      (map: embedder_id → SHA-256 digest), `frozen_at` timestamp, `corpus_sha`. The Panel JSON
      schema SHALL include these fields and SHALL fail-closed on missing/malformed metadata.
    </description>
    <rationale>constitution provenance rule; constitution SEC-MEJEPA-02 (pinned weight digests).</rationale>
    <constraints>
      Schema example:
      {
        "version": "1.0.0",
        "code_version": "&lt;git-sha&gt;",
        "embedder_versions": {"E1": "&lt;sha&gt;", "E5": "&lt;sha&gt;", ...},
        "time_step": 0|1|2,
        "attempt_id": "&lt;uuid&gt;",
        "slots": {"E_AST": [384 f32], "E_CFG": [256 f32], ...},
        "scalars": [128 f32],
        "total_dim": 5120,
        "sha256": "&lt;digest of slots+scalars&gt;"
      }
    </constraints>
    <verification_method>Round-trip panel.json deserialization rejects missing code_version / embedder_versions.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-INSTR-01" req_ref="REQ-INSTR-18">
    <condition>An instrument's encode() is called during Phase 3 training under autograd</condition>
    <action>Static analysis (training-loop guard) AND runtime gradient hook MUST verify zero gradient through the instrument's parameter tensors</action>
    <exception>None — frozen-instrument is a hard structural property of ME-JEPA per doc 11 §5.2</exception>
  </rule>

  <rule id="BR-INSTR-02" req_ref="REQ-INSTR-04">
    <condition>E_TypeGraph is invoked on a language whose toolchain is unavailable</condition>
    <action>Return MEJEPA_INFER_INSTRUMENT_MISSING fail-closed</action>
    <exception>None — parser-only fallback is forbidden per per-language harness rules in doc 09 §3.1 (current C / C# / Java / TypeScript / Ruby states)</exception>
  </rule>

  <rule id="BR-INSTR-03" req_ref="REQ-INSTR-15">
    <condition>PanelBuilder.set_slot is called for the same slot twice</condition>
    <action>Return InstrumentError::InvalidInput with message "slot {slot:?} already filled"</action>
    <exception>None</exception>
  </rule>

  <rule id="BR-INSTR-04" req_ref="REQ-INSTR-15">
    <condition>Panel::try_new is called with data containing NaN or Inf</condition>
    <action>Return InstrumentError::NumericalInvariant with code MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT</action>
    <exception>None — non-finite values are forbidden in any persisted panel</exception>
  </rule>

  <rule id="BR-INSTR-05" req_ref="REQ-INSTR-08">
    <condition>EWitnessInstrument receives chain_bytes that fail verify_chain_bytes</condition>
    <action>Return InstrumentError::InvalidInput; remediation = "repair the witness chain at the source of truth before encoding"</action>
    <exception>None — broken witness provenance must not silently produce a latent</exception>
  </rule>

  <rule id="BR-INSTR-06" req_ref="REQ-INSTR-17">
    <condition>materialize_panel(t=0) is called; E_Diff, E_Trace, E_Witness, E_Oracle, E_CommitMsg slots are by-construction zero</condition>
    <action>Those slots' bits in filled_mask MUST remain unset (filled_mask reflects only explicitly-set slots, not zero defaults)</action>
    <exception>None — the predictor uses filled_mask to distinguish "explicitly zero by time-step rule" from "missing data"</exception>
  </rule>

  <rule id="BR-INSTR-07" req_ref="REQ-INSTR-19">
    <condition>A code-domain instrument receives raw String input instead of typed AST-chunk struct</condition>
    <action>Refuse fail-closed at the type level (compile-time error) OR runtime guard returns MEJEPA_INSTRUMENTS_INVALID_INPUT</action>
    <exception>None — raw text chunking would defeat AST-perfect signal preservation</exception>
  </rule>

  <rule id="BR-INSTR-08" req_ref="REQ-INSTR-23">
    <condition>An instrument's underlying embedder has been retrained or its weights file SHA changes</condition>
    <action>The instrument's embedder_versions entry must update; any persisted Panel with stale embedder_versions must be rebuilt or rejected at predictor read time</action>
    <exception>None — Panel determinism depends on (input, weights, code_version) byte-equality</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-INSTR-01" req_ref="REQ-INSTR-15">
    <scenario>PanelBuilder builds with zero filled slots</scenario>
    <expected_behavior>Panel::try_new returns InstrumentError::InvalidInput with field "panel.filled_mask" — an empty panel is indistinguishable from "missing instrument evidence"</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-INSTR-02" req_ref="REQ-INSTR-15">
    <scenario>Panel JSON deserializes with filled_mask containing bits outside the 15-slot mask (bit 15 or higher)</scenario>
    <expected_behavior>try_from gate rejects with InstrumentError::InvalidInput; "filled_mask contains unknown slot bits"</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-INSTR-03" req_ref="REQ-INSTR-09">
    <scenario>EOracleInstrument receives an OracleVerdict with total_test_count == 0</scenario>
    <expected_behavior>Dims 0..4 (pass/fail/skip/error fractions) all 0.0; dim 4 (total) = 0.0; dim 5 (has_exception) = 0.0 or 1.0 per input; one-hot exception class set per input; encode succeeds</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-INSTR-04" req_ref="REQ-INSTR-09">
    <scenario>EOracleInstrument receives an OracleVerdict with total_test_count exceeding 1e6</scenario>
    <expected_behavior>Total bounded to 1e6 (per shipped behavior in e_oracle.rs); pathological runner outputs cannot allocate unbounded panel work</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-INSTR-05" req_ref="REQ-INSTR-08">
    <scenario>EWitnessInstrument receives WitnessChainInput with empty chain_bytes</scenario>
    <expected_behavior>verify_chain_bytes refuses the empty chain; instrument fails closed with InvalidInput; no all-zero slot is silently emitted</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-INSTR-06" req_ref="REQ-INSTR-08">
    <scenario>EWitnessInstrument receives WitnessChainInput with format_version != CANONICAL_WITNESS_FORMAT_VERSION</scenario>
    <expected_behavior>Reject fail-closed; per doc 09 §3.6 blindspot 17 — mixed-version batches forbidden without explicit conversion</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-INSTR-07" req_ref="REQ-INSTR-04">
    <scenario>E_TypeGraph invoked on Python source where Pyright is installed but mypy is not (or vice versa)</scenario>
    <expected_behavior>Instrument uses whichever toolchain is available; if BOTH are missing, fail closed with MEJEPA_INFER_INSTRUMENT_MISSING; the diagnostic structure records which tool produced each diagnostic</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-INSTR-08" req_ref="REQ-INSTR-06">
    <scenario>E_Trace receives a sequence of 1,000,001 events</scenario>
    <expected_behavior>Reject fail-closed (max 1M); the predictor must not see truncated traces silently</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-INSTR-09" req_ref="REQ-INSTR-19">
    <scenario>AST chunker produces a parse with tree-sitter ERROR or MISSING nodes</scenario>
    <expected_behavior>The chunker fails closed (per constitution failure_taxonomy MEJEPA_CHUNKER_PARSE_FAILED); the code-domain instrument never sees the broken parse</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-INSTR-10" req_ref="REQ-INSTR-18">
    <scenario>A maintenance PR accidentally exposes E_AST's underlying transformer parameters as `requires_grad = true`</scenario>
    <expected_behavior>Phase 3 training loop's gradient hook detects nonzero gradient through E_AST's parameters; raises MEJEPA_INSTR_FROZEN_VIOLATION; halts the training step; surfaces to operator; the PR is treated as a CRITICAL bug</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-INSTR-11" req_ref="REQ-INSTR-22">
    <scenario>Cold-start panel materialization takes 750 ms (exceeding p99 budget of 500 ms)</scenario>
    <expected_behavior>Instrument-cache-miss profiling identifies the slow stage (typically AST chunker on a large file or rust-analyzer on a fresh workspace); operator surfaces the regression; not a fail-closed condition (latency budget is "should", not "must" at this level)</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-INSTR-12" req_ref="REQ-INSTR-23">
    <scenario>A persisted Panel JSON is loaded with an embedder_versions entry whose SHA does not match the currently-loaded embedder weights file</scenario>
    <expected_behavior>Predictor read rejects the panel as stale; recompute the panel with current weights OR refuse the predictor invocation per constitution SEC-MEJEPA-02</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-INSTR-13" req_ref="REQ-INSTR-13">
    <scenario>E_Runtime receives walltime measurements that disagree across the 3 reruns by &gt; 30%</scenario>
    <expected_behavior>determinism_flag in RuntimeInput set to false; walltime dim contains the median of the 3 runs; instrument output remains finite; downstream training treats nondeterministic runtimes as lower-confidence supervision</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-INSTR-14" req_ref="REQ-INSTR-14">
    <scenario>E_Reasoning receives a CoT transcript containing a prompt-injection payload (e.g., "ignore previous instructions and ...")</scenario>
    <expected_behavior>Per constitution SEC-MEJEPA-06, the latent is opaque; the payload text is never echoed verbatim into MCP tool responses; reasoning latent encodes the affect signal but cannot be decoded back to text</expected_behavior>
    <priority>critical</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-INSTR-01" http_code="N/A" req_ref="REQ-INSTR-15">
    <condition>Vector length passed to PanelBuilder::set_slot does not match slot.dim()</condition>
    <user_message>"vector length {got} does not match slot {slot:?}'s dim {expected}"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=vector</internal_message>
    <recovery_action>Fix the instrument implementation to honor its declared output_dim; do not silently truncate or pad</recovery_action>
  </error>

  <error id="ERR-INSTR-02" http_code="N/A" req_ref="REQ-INSTR-15">
    <condition>Any element of a slot vector or persisted Panel.data is non-finite (NaN or Inf)</condition>
    <user_message>"vector[{i}] is non-finite: {v} (slot {slot:?})"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT field=vector</internal_message>
    <recovery_action>Scrub NaN/Inf from the instrument output before set_slot; never persist non-finite to ME-JEPA panels</recovery_action>
  </error>

  <error id="ERR-INSTR-03" http_code="N/A" req_ref="REQ-INSTR-04">
    <condition>E_TypeGraph invoked on a language with no available type-toolchain</condition>
    <user_message>"E_TypeGraph requires a type-checker for {language}; none available (Pyright/mypy/rust-analyzer/javac/dotnet/tsc/...)"</user_message>
    <internal_message>MEJEPA_INFER_INSTRUMENT_MISSING field=toolchain language={lang}</internal_message>
    <recovery_action>Install the language toolchain; or restrict the corpus to languages with available toolchains; do NOT fall back to parser-only evidence</recovery_action>
  </error>

  <error id="ERR-INSTR-04" http_code="N/A" req_ref="REQ-INSTR-19">
    <condition>A code-domain instrument receives raw text instead of AST-perfect chunks</condition>
    <user_message>"code-domain instrument {slot:?} requires typed AST-chunk input, not raw String"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=input</internal_message>
    <recovery_action>Run the AST chunker first; pass the chunks; never feed raw source text to a code-domain instrument</recovery_action>
  </error>

  <error id="ERR-INSTR-05" http_code="N/A" req_ref="REQ-INSTR-18">
    <condition>Gradient detected flowing through frozen instrument parameters during training</condition>
    <user_message>"frozen instrument {slot:?} has nonzero gradient; this is a CRITICAL bug per doc 11 §5.2"</user_message>
    <internal_message>MEJEPA_INSTR_FROZEN_VIOLATION slot={slot}</internal_message>
    <recovery_action>Revert the PR that exposed the parameters as trainable; assert torch/candle .requires_grad == false on every instrument's parameter tensor; halt training</recovery_action>
  </error>

  <error id="ERR-INSTR-06" http_code="N/A" req_ref="REQ-INSTR-08">
    <condition>EWitnessInstrument receives chain_bytes whose verify_chain_bytes fails</condition>
    <user_message>"witness chain verification failed: {err}; code={code}"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=input.chain_bytes</internal_message>
    <recovery_action>Repair the witness chain at the source of truth (using witness_chain_repair.rs tooling); never encode an unverified chain</recovery_action>
  </error>

  <error id="ERR-INSTR-07" http_code="N/A" req_ref="REQ-INSTR-08">
    <condition>WitnessChainInput.format_version != CANONICAL_WITNESS_FORMAT_VERSION</condition>
    <user_message>"witness chain format_version {got} not equal to canonical {expected}"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=input.format_version</internal_message>
    <recovery_action>Migrate the chain via witness_chain_legacy.rs reconciliation before encoding</recovery_action>
  </error>

  <error id="ERR-INSTR-08" http_code="N/A" req_ref="REQ-INSTR-15">
    <condition>Panel deserialized from JSON has data.len() != PANEL_DIM</condition>
    <user_message>"panel data length {got} does not match PANEL_DIM 5120"</user_message>
    <internal_message>MEJEPA_INFER_DIM_MISMATCH field=panel.data</internal_message>
    <recovery_action>Persist exactly PANEL_DIM f32 values; reject malformed panels at the deserialization boundary</recovery_action>
  </error>

  <error id="ERR-INSTR-09" http_code="N/A" req_ref="REQ-INSTR-15">
    <condition>PanelBuilder::set_slot called twice for the same slot</condition>
    <user_message>"slot {slot:?} already filled"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=slot</internal_message>
    <recovery_action>Refactor caller to call set_slot at most once per slot per panel</recovery_action>
  </error>

  <error id="ERR-INSTR-10" http_code="N/A" req_ref="REQ-INSTR-23">
    <condition>Panel JSON missing code_version or embedder_versions provenance fields</condition>
    <user_message>"panel missing required provenance metadata: {field}"</user_message>
    <internal_message>MEJEPA_INSTRUMENTS_INVALID_INPUT field=metadata</internal_message>
    <recovery_action>Re-materialize the panel with current code_version + embedder_versions; do not load partial-provenance panels</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="InstrumentSlot">
    <field name="variant" type="enum" required="true">
      <description>One of the 15 named slots: EAst, ECfg, EDataFlow, ETypeGraph, ETest, ETrace, EDiff, EWitness, EOracle, EProblem, ECommitMsg, EStaticAnalysis, ERuntime, EReasoning, Scalars</description>
      <constraints>Variant order MUST match InstrumentSlot::layout() and InstrumentSlot::all(); slug must be lowercase snake_case</constraints>
    </field>
  </entity>

  <entity name="InstrumentLayoutEntry">
    <field name="slot" type="InstrumentSlot" required="true"><description>Which slot</description></field>
    <field name="offset" type="usize" required="true"><description>Byte-precise offset in the 5,120-d panel</description><constraints>0 ≤ offset &lt; 5120</constraints></field>
    <field name="dim" type="usize" required="true"><description>Number of f32 elements owned by this slot</description><constraints>Sum of all 15 dims == 5,120</constraints></field>
  </entity>

  <entity name="Panel">
    <field name="data" type="Vec&lt;f32&gt;" required="true"><description>Full 5,120-d panel data</description><constraints>data.len() == 5,120; every element finite</constraints></field>
    <field name="filled_mask" type="u16" required="true"><description>Bitmask: bit i set iff slot i (in InstrumentSlot::all() order) was explicitly filled</description><constraints>filled_mask &amp; !VALID_FILLED_MASK == 0; filled_mask != 0 (no zero-slot panels)</constraints></field>
  </entity>

  <entity name="PanelJSON">
    <field name="version" type="string" required="true"><description>Schema version</description><constraints>Currently "1.0.0"</constraints></field>
    <field name="code_version" type="string" required="true"><description>Git SHA of the codebase that produced this panel</description></field>
    <field name="embedder_versions" type="map&lt;string, string&gt;" required="true"><description>Map from embedder_id (E1, E5, ..., E21) to weights file SHA-256</description></field>
    <field name="time_step" type="enum {0, 1, 2}" required="true"><description>Which of the three panels per attempt this is</description></field>
    <field name="attempt_id" type="string" required="true"><description>Stable identifier for the (task, mutation_category) attempt</description></field>
    <field name="slots" type="map&lt;string, Vec&lt;f32&gt;&gt;" required="true"><description>Per-slot vectors keyed by slug</description></field>
    <field name="scalars" type="Vec&lt;f32&gt;" required="true"><description>The Scalars slot's 128-d vector</description><constraints>length == 128</constraints></field>
    <field name="total_dim" type="usize" required="true"><description>Total panel dimension</description><constraints>== 5,120</constraints></field>
    <field name="sha256" type="string" required="true"><description>SHA-256 digest of concatenated slots+scalars</description></field>
  </entity>

  <entity name="OracleVerdict">
    <field name="per_test_outcomes" type="Vec&lt;PerTestOutcome&gt;" required="true"><description>One entry per test that ran</description></field>
    <field name="exception_class" type="Option&lt;ExceptionClass&gt;" required="false"><description>Top-level exception if execution crashed</description></field>
    <field name="total_test_count" type="u32" required="true"><description>Total tests attempted</description><constraints>bounded to 1e6</constraints></field>
  </entity>

  <entity name="WitnessChainInput">
    <field name="format_version" type="u8" required="true"><description>Witness chain canonical format version</description><constraints>== CANONICAL_WITNESS_FORMAT_VERSION (1)</constraints></field>
    <field name="chain_bytes" type="Vec&lt;u8&gt;" required="true"><description>73-byte SHAKE-256 entries serialized contiguously</description><constraints>length is multiple of WITNESS_ENTRY_SIZE (73); ≤ MAX_WITNESS_ENTRIES (65,536) entries</constraints></field>
  </entity>

  <entity name="StaticAnalysisInput">
    <field name="source_text" type="String" required="true"><description>Source text the diagnostics refer to</description></field>
    <field name="diagnostics" type="Vec&lt;Diagnostic&gt;" required="true"><description>Linter/typechecker output</description><constraints>≤ MAX_DIAGNOSTICS (100,000)</constraints></field>
    <field name="churn_30d" type="Option&lt;u32&gt;" required="false"><description>Number of commits touching this file in last 30 days</description></field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-INSTR-01" category="performance">
    <description>Panel materialization with cached instruments must complete within budget</description>
    <metric>p50 &lt; 100 ms; p99 &lt; 500 ms cold (per constitution performance_budgets)</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-02" category="performance">
    <description>Per-instrument latency budgets</description>
    <metric>E1-derived (E_Test, E_Problem, E_CommitMsg) &lt; 5 ms; E11/Kepler-derived (E_DataFlow, E_TypeGraph) &lt; 10 ms; E_AST/E_CFG/E_Diff &lt; 20 ms cold; scalar aggregators (E_Oracle, E_StaticAnalysis, E_Runtime, Scalars) &lt; 1 ms</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-INSTR-03" category="reliability">
    <description>Determinism: same input + same code_version + same embedder_versions → byte-equal output</description>
    <metric>Determinism check: 100 random inputs encoded twice; SHA-256 of outputs must match in 100/100 cases</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-04" category="security">
    <description>FSV evidence files created with mode 0o600 per constitution SEC-MEJEPA-07; never world-readable</description>
    <metric>stat() on every /tmp/contextgraph-mejepa-instruments-fsv/* file shows mode 0o600</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-05" category="security">
    <description>E_Reasoning latent must not allow CoT text reconstruction; opaque encoding only (constitution SEC-MEJEPA-06)</description>
    <metric>Linear reverse-decoder probe on E_Reasoning latent → text must achieve &lt; 5 BLEU vs ground-truth CoT</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-06" category="reliability">
    <description>Frozen-instrument invariant: zero trainable parameters in any instrument</description>
    <metric>Phase 3 gradient hook reports zero ∂L/∂θ for every instrument's parameter tensors across 1,000 training steps</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-07" category="usability">
    <description>FSV evidence is human-readable and re-readable</description>
    <metric>Every /tmp/contextgraph-mejepa-instruments-fsv/*.json file deserializes via serde_json without manual edits; the FSV harness re-reads and asserts before reporting success</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-08" category="reliability">
    <description>Test coverage on shipped instrument code</description>
    <metric>≥ 80% line coverage per constitution testing_requirements; current shipped E_Oracle/E_Witness/E_StaticAnalysis ship with 27 unit tests (per memory/project_phase0_mejepa_mutation_corpus.md 2026-05-09d entry); total ME-JEPA-foundation crates tests at 251</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INSTR-09" category="reliability">
    <description>Phase 1 Definition-of-Done: linear-probe top-1 accuracy on mutation-category prediction</description>
    <metric>≥ 70% top-1 accuracy on a held-out 100-example linear probe per instrument (per task brief round-trip requirement)</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-INSTR-10" category="reliability">
    <description>Multi-language coverage: code-domain instruments work across all 11 wired languages</description>
    <metric>Per-language FSV (Python, Rust, C, C++, Java, C#, JS, TS, Ruby, Go, PHP) verifies parser correctness against language toolchain output (py_compile, rustc, gcc, javac, dotnet build, node, tsc, ruby -c, go build, php -l) per constitution fsv_protocol</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>context-graph-core (ast_chunker)</name>
    <description>Multi-language AST-perfect chunker (Rust shipped; 10 more languages Phase 1)</description>
    <impact>Code-domain instruments (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Diff, E_Test) cannot run without it; fail closed with MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-embeddings (E1-E21 embedder ensemble)</name>
    <description>21 frozen embedders providing the substrate for 8 of the 14 instruments per doc 12</description>
    <impact>Instruments backed by E1/E7/E8/E10/E11/E17 cannot run without their respective embedder; fail closed with MEJEPA_INFER_INSTRUMENT_MISSING</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-witness</name>
    <description>73-byte SHAKE-256 chain primitives (verify_chain_bytes, WitnessEntry, WITNESS_ENTRY_SIZE)</description>
    <impact>E_Witness fails closed without it</impact>
  </dependency>

  <dependency type="internal">
    <name>context-graph-mejepa-corpus</name>
    <description>Phase 0 mutation corpus + SWE-bench Docker oracle bridge providing OracleVerdict</description>
    <impact>E_Oracle requires real Docker oracle output; mock OracleVerdicts violate constitution anti_patterns "no mock data in tests"</impact>
  </dependency>

  <dependency type="external">
    <name>tree-sitter (rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php)</name>
    <description>11 grammars wired</description>
    <impact>Code-domain instruments depend on these for AST parsing</impact>
  </dependency>

  <dependency type="external">
    <name>Language toolchains (Pyright, mypy, rust-analyzer, javac, dotnet, tsc, gcc, ruby, go, php-cli)</name>
    <description>Required for E_TypeGraph and E_StaticAnalysis to produce real diagnostics</description>
    <impact>Toolchain absence triggers MEJEPA_INFER_INSTRUMENT_MISSING; no parser-only fallback per doc 09 §3.1 per-language harness rules</impact>
  </dependency>

  <dependency type="external">
    <name>candle (0.9.x, sm_120a CUDA via cudarc)</name>
    <description>ML framework for E1/E7/E8/E10/E11/E17 forward passes</description>
    <impact>Neural-backed instruments cannot run without it; pure-Rust scalar instruments (E_Oracle, E_StaticAnalysis, E_Runtime, Scalars) are not affected</impact>
  </dependency>

</dependencies>

<out_of_scope>

  <item>Predictor architecture (Phase 2) — handled by SPEC-PREDICTOR (deferred)</item>
  <item>VICReg loss + Oracle head (Phase 2) — SPEC-PREDICTOR</item>
  <item>UTML L_step training formula (Phase 3) — SPEC-TRAINING</item>
  <item>TCT constellation construction + Gτ guard (Phase 4b) — SPEC-TCT</item>
  <item>Conformal calibration + OOD detection (Phase 4) — SPEC-INFERENCE</item>
  <item>EWC++ memory protection + drift detection (Phase 5) — SPEC-SELF-HEALING</item>
  <item>RocksDB column family persistence layer for CF_MEJEPA_PANELS — SPEC-STORAGE (the data model is ours; the persistence is theirs)</item>
  <item>MCP tool surface (mcp__cgreality__mejepa_predict_latest etc., Phase 7) — SPEC-MCP-TOOLS</item>
  <item>21-embedder ensemble itself — SPEC-EMBEDDERS (we consume them frozen; we do not train them)</item>
  <item>The AST chunker — SPEC-AST-CHUNKING (we consume its output)</item>
  <item>Phase 0 mutation corpus generator — SPEC-CORPUS (we consume its OracleVerdicts)</item>

</out_of_scope>

<test_plan>

  <test_case id="TC-INSTR-01" type="unit" req_ref="REQ-INSTR-15">
    <description>InstrumentSlot::layout() sums to PANEL_DIM (5,120) and matches the canonical doc-09 §3.5.5 table</description>
    <preconditions>Crate compiles</preconditions>
    <inputs>None</inputs>
    <expected_result>Sum of all 15 slot dims == 5,120; offsets are contiguous and non-overlapping</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-02" type="unit" req_ref="REQ-INSTR-15">
    <description>Panel JSON round-trip preserves data and filled_mask byte-for-byte</description>
    <preconditions>Build a Panel with all 15 slots filled with deterministic values</preconditions>
    <inputs>Random-seeded f32 vectors per slot</inputs>
    <expected_result>serde_json::to_string then serde_json::from_str returns a Panel that compares == to the original via PartialEq</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-03" type="unit" req_ref="REQ-INSTR-15">
    <description>PanelBuilder::set_slot rejects double-fill</description>
    <preconditions>None</preconditions>
    <inputs>Same slot, two valid vectors</inputs>
    <expected_result>Second call returns InstrumentError::InvalidInput field=slot</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-04" type="unit" req_ref="REQ-INSTR-15">
    <description>PanelBuilder::set_slot rejects non-finite</description>
    <preconditions>None</preconditions>
    <inputs>A slot vector with one NaN element</inputs>
    <expected_result>InstrumentError::NumericalInvariant field=vector</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-05" type="unit" req_ref="REQ-INSTR-09">
    <description>EOracleInstrument all-pass boundary</description>
    <preconditions>None</preconditions>
    <inputs>OracleVerdict { per_test_outcomes: 100x Pass, exception_class: None, total: 100 }</inputs>
    <expected_result>encode returns 128-d Vec; dims 0..4 == [1.0, 0.0, 0.0, 0.0]; dim 5 (has_exception) == 0.0; dims 6..16 all 0.0</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-06" type="unit" req_ref="REQ-INSTR-09">
    <description>EOracleInstrument crash_assertion_error boundary</description>
    <preconditions>None</preconditions>
    <inputs>OracleVerdict { per_test_outcomes: empty, exception_class: AssertionError, total: 0 }</inputs>
    <expected_result>encode returns 128-d Vec; dim 5 == 1.0; dim 6 (AssertionError one-hot) == 1.0; dims 7..16 all 0.0</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-07" type="unit" req_ref="REQ-INSTR-08">
    <description>EWitnessInstrument rejects empty chain</description>
    <preconditions>None</preconditions>
    <inputs>WitnessChainInput { format_version: 1, chain_bytes: vec![] }</inputs>
    <expected_result>InstrumentError::InvalidInput; verify_chain_bytes refuses empty</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-08" type="unit" req_ref="REQ-INSTR-08">
    <description>EWitnessInstrument rejects mismatched format_version</description>
    <preconditions>None</preconditions>
    <inputs>WitnessChainInput { format_version: 99, chain_bytes: valid_73_byte_entry }</inputs>
    <expected_result>InstrumentError::InvalidInput field=format_version</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-09" type="integration" req_ref="REQ-INSTR-19">
    <description>E_AST consumes AST-perfect chunks across all 11 languages</description>
    <preconditions>tree-sitter grammars wired; ast_chunker present; sample sources for each language</preconditions>
    <inputs>One real source file per language (rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php)</inputs>
    <expected_result>encode succeeds for each language; output dim == 384; finite; deterministic across two runs</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-10" type="integration" req_ref="REQ-INSTR-04">
    <description>E_TypeGraph fails closed when toolchain missing</description>
    <preconditions>Pyright NOT installed; mypy NOT installed</preconditions>
    <inputs>Python AST-perfect chunks</inputs>
    <expected_result>MEJEPA_INFER_INSTRUMENT_MISSING; no zero-fallback latent emitted</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-11" type="integration" req_ref="REQ-INSTR-17">
    <description>materialize_panel(t=0) populates only the structural slots</description>
    <preconditions>Mutation corpus entry available</preconditions>
    <inputs>(task_id, mutation_category, time_step=0)</inputs>
    <expected_result>Panel.is_filled returns true for {EAst, ECfg, EDataFlow, ETypeGraph, ETest, EProblem}; false for {EDiff, ETrace, EWitness, EOracle, ECommitMsg}; filled_mask reflects this</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-12" type="integration" req_ref="REQ-INSTR-17">
    <description>materialize_panel(t=2) populates all 15 slots</description>
    <preconditions>Mutation corpus entry with Docker oracle verdict</preconditions>
    <inputs>(task_id, mutation_category, time_step=2)</inputs>
    <expected_result>All 15 slots filled; filled_mask == VALID_FILLED_MASK; panel.data length == 5,120; finite</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-13" type="e2e" req_ref="REQ-INSTR-20">
    <description>Phase 1 extended FSV harness writes evidence for all 4 boundary cases</description>
    <preconditions>Crate built in release mode; /tmp writable</preconditions>
    <inputs>cargo run --release -p context-graph-mejepa-instruments --example phase1_extended_fsv</inputs>
    <expected_result>/tmp/contextgraph-mejepa-instruments-fsv/{all_pass,mixed,crash_assertion_error,crash_type_error}_{input,encoded}.json + panel.json (~46 KB) all present, mode 0o600, deserialize cleanly, dim correctness asserted</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-14" type="integration" req_ref="REQ-INSTR-18">
    <description>Frozen-instrument gradient hook detects zero gradient through every instrument's parameters</description>
    <preconditions>Phase 3 training step set up</preconditions>
    <inputs>1,000 forward+backward passes through the predictor with random panels</inputs>
    <expected_result>For every instrument's parameter tensor θ_E, |∂L/∂θ_E| == 0 across all 1,000 steps; any nonzero gradient triggers MEJEPA_INSTR_FROZEN_VIOLATION</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INSTR-15" type="integration" req_ref="NFR-INSTR-09" priority="should">
    <description>Linear probe on E_Diff achieves ≥ 70% mutation-category top-1 accuracy</description>
    <preconditions>100 mutation-corpus examples spanning all 8 categories, with 20% holdout</preconditions>
    <inputs>E_Diff outputs as features; mutation_category as label</inputs>
    <expected_result>Top-1 accuracy on holdout ≥ 70% (per Phase 1 DoD)</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-INSTR-16" type="integration" req_ref="NFR-INSTR-01">
    <description>Panel materialization latency profiling</description>
    <preconditions>Cache populated for prior runs; 100 random corpus entries</preconditions>
    <inputs>Materialize 100 panels in sequence; measure wall-clock</inputs>
    <expected_result>p50 &lt; 100 ms warm; p99 &lt; 500 ms cold; histogram persisted to /tmp/contextgraph-mejepa-instruments-fsv/latency_p50_p99.json</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-INSTR-17" type="integration" req_ref="REQ-INSTR-23">
    <description>Stale embedder_versions metadata triggers panel rejection</description>
    <preconditions>Persisted Panel with embedder_versions: {E1: "&lt;old-sha&gt;"}</preconditions>
    <inputs>Load panel after embedder weights file is replaced (new SHA)</inputs>
    <expected_result>Predictor read rejects the panel as stale; recomputed panel passes; rejection logs MEJEPA_INSTRUMENTS_INVALID_INPUT</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INSTR-18" type="integration" req_ref="REQ-INSTR-14">
    <description>E_Reasoning latent does not allow CoT text reconstruction</description>
    <preconditions>E_Reasoning shipped; 100 paired (CoT_text, latent) examples</preconditions>
    <inputs>Train a linear reverse-decoder from latent → text; compare against ground-truth CoT</inputs>
    <expected_result>BLEU score &lt; 5; latent is operationally opaque per constitution SEC-MEJEPA-06</expected_result>
    <priority>high</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-INSTR-01" status="open" assignee="operator">
    <text>Which embedder backs E_AST: a 1536→384 linear projection of E7 Qodo, or a small AST-token transformer trained on top of E7? Doc 12 §3 row E_AST lists both as candidates.</text>
    <context>The choice affects the FROZEN definition: a linear projection has frozen-trivially-by-construction parameters; a small transformer fine-tuned on E7's outputs has parameters that must be frozen post-training. Both satisfy REQ-INSTR-18 if applied correctly.</context>
    <resolution></resolution>
  </question>

  <question id="Q-INSTR-02" status="open" assignee="operator">
    <text>Is the deterministic shipped E_Witness encoder (no neural component) acceptable as the frozen production target, or must it be replaced with the planned E10-Contextual-backed sequence transformer described in plan_e_witness.md?</text>
    <context>The current shipped EWitnessInstrument is a hand-crafted scalar aggregator; doc 12 §3 row E_Witness mentions a neural replacement is tracked in specs. Both are FROZEN per REQ-INSTR-18.</context>
    <resolution></resolution>
  </question>

  <question id="Q-INSTR-03" status="open" assignee="operator">
    <text>For E_TypeGraph on Python: should the instrument use Pyright (microsoft, more strict) or mypy (more widespread, less strict) when both are available? Or merge both diagnostic streams?</text>
    <context>Affects the categorical distribution of diagnostic.tool in the encoded latent.</context>
    <resolution></resolution>
  </question>

  <question id="Q-INSTR-04" status="open" assignee="operator">
    <text>For E_Reasoning, the CoT input includes the agent's full conversation transcript including operator messages. Are operator messages PII or sensitive? Should they be redacted before encoding?</text>
    <context>Constitution SEC-MEJEPA-06 says encoded as opaque embeddings; constitution security_requirements does not mention PII redaction. NFR-INSTR-05 mandates BLEU &lt; 5 reverse decoding, but a determined attacker might still extract patterns.</context>
    <resolution></resolution>
  </question>

  <question id="Q-INSTR-05" status="open" assignee="operator">
    <text>Should the deterministic shipped instruments (E_Oracle, E_Witness, E_StaticAnalysis, plus the Phase 1 typed encoders for E_AST/E_CFG/E_DataFlow/E_TypeGraph/E_Diff/E_Test/E_Trace/E_Problem/E_CommitMsg/E_Reasoning/E_Runtime/Scalars) remain in the production panel even after neural replacements ship? Or are they transitional?</text>
    <context>The shipped state from 2026-05-09 is a deterministic typed encoder for every slot. Neural backings are Phase 1 deliverables; the typed encoders give us a working panel today. Both could co-exist behind a config flag.</context>
    <resolution></resolution>
  </question>

</open_questions>

<glossary>

  <term name="Panel">A 5,120-d joint embedding produced by concatenating the 14 instrument outputs + the Scalars aggregator at byte-precise offsets. The IR of the reality-compiler analogy.</term>
  <term name="PanelBuilder">Stateful builder that fills slots one at a time, fail-closed on dim mismatch / non-finite / double-fill / zero-filled.</term>
  <term name="Slot">A named region of the panel owned by exactly one instrument. 15 slots total: 14 named instruments + Scalars.</term>
  <term name="Instrument">A frozen function `(typed_input) -> Vec&lt;f32&gt;` with declared output_dim. Implements the `Instrument` trait. Zero trainable parameters.</term>
  <term name="FROZEN">An instrument's parameters carry zero gradient under autograd; same input + same weights + same code_version → byte-equal output. Structurally prevents target-side collapse per doc 11 §5.2.</term>
  <term name="AST-perfect chunk">A code segment bounded by a syntactic entity (function, class, impl, module-decl), produced by the multi-language ast_chunker, ~500 non-whitespace chars per Qodo / cAST methodology, never splits a statement.</term>
  <term name="time_step">Which of three panels per attempt: t=0 (pre-patch), t=1 (post-patch pre-execution), t=2 (post-execution).</term>
  <term name="filled_mask">u16 bitmask: bit i set iff slot i (in InstrumentSlot::all() order) was explicitly set; lets the predictor distinguish "explicitly zero" from "not yet set".</term>
  <term name="OracleVerdict">Structured representation of the SWE-bench Docker oracle's per-test outcome and optional top-level exception.</term>
  <term name="WitnessChainInput">Verified 73-byte SHAKE-256 chain entry sequence + format_version.</term>
  <term name="FSV (Forensic Source Verification)">Per constitution fsv_protocol: every shipped primitive ships with an example binary that writes evidence to /tmp/contextgraph-&lt;crate&gt;-fsv/, re-reads it, and asserts before reporting success.</term>
  <term name="Linear probe (round-trip)">Train a single linear classifier on instrument outputs to predict mutation_category; ≥ 70% top-1 accuracy is the Phase 1 DoD.</term>
  <term name="MEJEPA_INSTR_FROZEN_VIOLATION">Critical error: a gradient was detected flowing through a frozen instrument; the system halts and surfaces to operator.</term>

</glossary>

</functional_spec>
```
