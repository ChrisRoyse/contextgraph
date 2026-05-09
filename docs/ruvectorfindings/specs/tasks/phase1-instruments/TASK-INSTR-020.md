# TASK-INSTR-020 — `materialize_panel` helper (TimeStep::{T0, T1, T2} rules)

```xml
<task_spec id="TASK-INSTR-020" version="1.0">

<metadata>
  <title>materialize_panel(time_step, AttemptContext) — build a Panel for one of three time steps per doc 09 §2.3</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>20</sequence>
  <implements>
    <item ref="REQ-INSTR-17">materialize_panel helper</item>
    <item ref="component:materialize_panel">fn materialize_panel(time_step, &amp;AttemptContext) -&gt; InstrumentResult&lt;Panel&gt;</item>
    <item ref="BR-INSTR-06">filled_mask reflects only explicitly-set slots (not zero defaults)</item>
    <item ref="AC-INSTR-02-01">t=0 populates only structural slots</item>
    <item ref="AC-INSTR-02-02">t=2 populates all 15 slots including E_Oracle</item>
    <item ref="AC-INSTR-02-03">missing toolchain (E_TypeGraph) -&gt; MEJEPA_INFER_INSTRUMENT_MISSING; failure recorded in CF_MEJEPA_PANEL_META</item>
    <item ref="REQ-INSTR-22">panel_materialization_p50 &lt; 100 ms with cached instruments; p99 &lt; 500 ms cold</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-005</task_ref>
    <task_ref>TASK-INSTR-006</task_ref>
    <task_ref>TASK-INSTR-007</task_ref>
    <task_ref>TASK-INSTR-008</task_ref>
    <task_ref>TASK-INSTR-009</task_ref>
    <task_ref>TASK-INSTR-010</task_ref>
    <task_ref>TASK-INSTR-011</task_ref>
    <task_ref>TASK-INSTR-012</task_ref>
    <task_ref>TASK-INSTR-013</task_ref>
    <task_ref>TASK-INSTR-014</task_ref>
    <task_ref>TASK-INSTR-015</task_ref>
    <task_ref>TASK-INSTR-016</task_ref>
    <task_ref>TASK-INSTR-017</task_ref>
    <task_ref>TASK-INSTR-018</task_ref>
    <task_ref>TASK-INSTR-019</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file created, 1 file modified</estimated_files>
</metadata>

<context>
Per doc 09 §2.3 + REQ-INSTR-17, every attempt produces 3 panels:

- **t=0 (pre-patch):** populates `{EAst, ECfg, EDataFlow, ETypeGraph, ETest, EProblem}` over the
  pre-patch code; `{EDiff, ETrace, EWitness, EOracle, ECommitMsg, EStaticAnalysis, ERuntime,
  EReasoning, Scalars}` are by-construction zero (not in filled_mask).
- **t=1 (post-patch pre-execution):** the t=0 slots re-evaluated on patched code, plus
  `{EDiff, ECommitMsg}`, plus `EWitness` with patch-application-only entries; `ETrace`, `EOracle`
  remain zero. (`EStaticAnalysis`, `ERuntime`, `EReasoning`, `Scalars` not yet populated; t=2 adds
  them.)
- **t=2 (post-execution):** every slot populated; filled_mask == VALID_FILLED_MASK.

The helper takes a typed `AttemptContext` carrying all per-attempt inputs (chunks, witness chain,
oracle verdict, runtime aggregates, scalars, problem text, commit message text, reasoning events,
diff edges) and dispatches to the right instruments per time step.

Per BR-INSTR-06, filled_mask reflects only explicitly-set slots — the predictor uses this to
distinguish "explicitly zero by time-step rule" from "missing data".
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-17, BR-INSTR-06, AC-INSTR-02-01/02/03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts materialize_panel; time_step_protocol_diagram)</file>
  <file purpose="design_doc_time_steps">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.3</file>
  <file purpose="shipped_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel, PanelBuilder)</file>
  <file purpose="instruments">crates/context-graph-mejepa-instruments/src/{e_oracle,e_witness,e_static_analysis,e_code,e_text,e_trace,e_runtime,e_reasoning,e_scalars}.rs</file>
</input_context_files>

<prerequisites>
  <check>All instrument tasks (005..019) done OR shipped-deterministic encoders behind same trait</check>
  <check>TASK-INSTR-003 done (Panel/PanelBuilder verified)</check>
</prerequisites>

<scope>
  <in_scope>
    - New module `crates/context-graph-mejepa-instruments/src/materialize.rs`
    - `pub enum TimeStep { T0, T1, T2 }`
    - `pub struct AttemptContext` with typed inputs for every instrument
    - `pub fn materialize_panel(time_step: TimeStep, ctx: &amp;AttemptContext) -&gt; InstrumentResult&lt;Panel&gt;`
    - Per-time-step dispatch logic (which instruments to invoke)
    - Failure recording: when E_TypeGraph fails MEJEPA_INFER_INSTRUMENT_MISSING, record in returned `MaterializationResult { panel: Option, failures: Vec }` so the caller can persist to CF_MEJEPA_PANEL_META
    - Latency profiling: instrument-by-instrument breakdown
    - Unit tests for each time step × each combination of instruments-present/missing
    - Cross-time-step determinism: same AttemptContext + same code_version + same embedder_versions -&gt; byte-equal Panel
    - Re-export `materialize_panel`, `TimeStep`, `AttemptContext`, `MaterializationResult` from lib.rs
  </in_scope>
  <out_of_scope>
    - JSON serialization (TASK-INSTR-022)
    - Persistence to RocksDB (TASK-INSTR-022)
    - Linear-probe gate (TASK-INSTR-021)
    - Phase 7 shift-subscriber wiring (Phase 7 spec)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/materialize.rs">
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeStep { T0, T1, T2 }

#[derive(Debug, Clone)]
pub struct AttemptContext {
    pub attempt_id: String,
    pub language: ChunkerLanguage,
    pub pre_patch_chunks: Vec&lt;AstPerfectChunk&gt;,
    pub post_patch_chunks: Vec&lt;AstPerfectChunk&gt;,
    pub problem_text: String,
    pub commit_msg_text: Option&lt;String&gt;,
    pub diff_edges: Vec&lt;DiffEdge&gt;,
    pub witness_chain_partial_t1: Option&lt;WitnessChainInput&gt;,
    pub witness_chain_full_t2: Option&lt;WitnessChainInput&gt;,
    pub trace_input: Option&lt;TraceInput&gt;,
    pub oracle_verdict: Option&lt;OracleVerdict&gt;,
    pub static_analysis: Option&lt;StaticAnalysisInput&gt;,
    pub runtime_input: Option&lt;RuntimeInput&gt;,
    pub reasoning_input: Option&lt;ReasoningInput&gt;,
    pub scalar_input: Option&lt;ScalarInput&gt;,
}

pub struct InstrumentRegistry {
    pub e_ast: Box&lt;dyn Instrument&lt;Input = CodeInstrumentInput&gt;&gt;,
    pub e_cfg: Box&lt;dyn Instrument&lt;Input = CodeInstrumentInput&gt;&gt;,
    // ... one per instrument
}

#[derive(Debug)]
pub struct MaterializationResult {
    pub panel: Panel,
    pub failures: Vec&lt;(InstrumentSlot, InstrumentError)&gt;,  // E_TypeGraph etc. that failed gracefully
    pub latency_breakdown_ns: HashMap&lt;InstrumentSlot, u64&gt;,
}

pub fn materialize_panel(
    time_step: TimeStep,
    ctx: &amp;AttemptContext,
    registry: &amp;InstrumentRegistry,
) -&gt; InstrumentResult&lt;MaterializationResult&gt;;
    </signature>
  </signatures>
  <constraints>
    - Per time-step rules per doc 09 §2.3
    - Empty AttemptContext is rejected (no panels can be built)
    - filled_mask reflects only explicitly-set slots (BR-INSTR-06)
    - Same input + same code_version + same embedder_versions -&gt; byte-equal Panel
    - Latency budget: p50 &lt; 100 ms (warm cache), p99 &lt; 500 ms (cold) per NFR-INSTR-01
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments materialize::
    - Per-time-step test (4 boundary cases × 3 time steps × 11 languages)
    - Determinism test
    - Latency profile written to /tmp/contextgraph-mejepa-instruments-fsv/latency_p50_p99.json
  </verification>
</definition_of_done>

<pseudo_code>
materialize.rs:

  pub fn materialize_panel(time_step, ctx, registry) -&gt; InstrumentResult&lt;MaterializationResult&gt; {
      let mut builder = PanelBuilder::new();
      let mut failures: Vec&lt;_&gt; = vec![];
      let mut latency: HashMap&lt;InstrumentSlot, u64&gt; = HashMap::new();
      match time_step {
          TimeStep::T0 =&gt; {
              encode_or_record(&amp;mut builder, &amp;mut failures, &amp;mut latency, registry.e_ast.as_ref(),
                  &amp;CodeInstrumentInput { language: ctx.language, chunks: ctx.pre_patch_chunks.clone() })?;
              encode_or_record(&amp;mut builder, ..., registry.e_cfg.as_ref(), ...)?;
              encode_or_record(&amp;mut builder, ..., registry.e_data_flow.as_ref(), ...)?;
              encode_or_record(&amp;mut builder, ..., registry.e_type_graph.as_ref(), ...);  // toolchain may be missing -&gt; failures
              encode_or_record(&amp;mut builder, ..., registry.e_test.as_ref(), ...)?;
              encode_or_record(&amp;mut builder, ..., registry.e_problem.as_ref(),
                  &amp;TextInstrumentInput { text: ctx.problem_text.clone() })?;
              // Other slots intentionally NOT set
          }
          TimeStep::T1 =&gt; {
              // T0 slots over post_patch_chunks
              encode_or_record(..., registry.e_ast.as_ref(),
                  &amp;CodeInstrumentInput { language: ctx.language, chunks: ctx.post_patch_chunks.clone() })?;
              ... (e_cfg, e_data_flow, e_type_graph, e_test, e_problem on post-patch)
              // Plus EDiff, ECommitMsg
              encode_or_record(..., registry.e_diff.as_ref(), &amp;DiffInstrumentInput { ... })?;
              if let Some(msg) = &amp;ctx.commit_msg_text {
                  encode_or_record(..., registry.e_commit_msg.as_ref(), &amp;TextInstrumentInput { text: msg.clone() })?;
              }
              // Partial witness (patch-application entries only)
              if let Some(wc) = &amp;ctx.witness_chain_partial_t1 {
                  encode_or_record(..., registry.e_witness.as_ref(), wc)?;
              }
          }
          TimeStep::T2 =&gt; {
              // ALL 15 slots; require_slots(InstrumentSlot::all())
              ... (T1 slots over post_patch + EDiff + ECommitMsg)
              // Plus ETrace, EOracle, ERuntime, EReasoning, EStaticAnalysis, EWitness (full), Scalars
              encode_or_record_required(..., registry.e_trace.as_ref(), ctx.trace_input.as_ref())?;
              encode_or_record_required(..., registry.e_oracle.as_ref(), ctx.oracle_verdict.as_ref())?;
              ... etc
              builder.require_slots(&amp;InstrumentSlot::all())?;
          }
      }
      let panel = builder.build()?;
      Ok(MaterializationResult { panel, failures, latency_breakdown_ns: latency })
  }

  fn encode_or_record&lt;I, In&gt;(builder, failures, latency, instrument: &amp;I, input: &amp;In) -&gt; InstrumentResult&lt;()&gt;
  where I: Instrument&lt;Input = In&gt; {
      let t0 = std::time::Instant::now();
      match builder.set_from_instrument(instrument, input) {
          Ok(_) =&gt; {
              latency.insert(instrument.slot(), t0.elapsed().as_nanos() as u64);
              Ok(())
          }
          Err(e) if is_recoverable(&amp;e) =&gt; {
              // E_TypeGraph toolchain-missing is recoverable: record + continue
              failures.push((instrument.slot(), e));
              Ok(())
          }
          Err(e) =&gt; Err(e),  // unrecoverable: bubble up
      }
  }

tests:
  - materialize_t0_populates_structural_slots_only
  - materialize_t1_populates_t0_plus_diff_commit_witness_partial
  - materialize_t2_populates_all_15_slots
  - materialize_t0_filled_mask_correct
  - materialize_byte_equal_across_calls
  - materialize_e_type_graph_missing_recorded_as_failure_not_panic
  - materialize_oracle_missing_at_t2_rejected
  - materialize_per_language_python_rust_etc (11 languages)
  - latency_breakdown_recorded
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-instruments/src/materialize.rs">TimeStep enum, AttemptContext struct, InstrumentRegistry, MaterializationResult, materialize_panel function, 9 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub mod materialize;` and re-export {TimeStep, AttemptContext, InstrumentRegistry, MaterializationResult, materialize_panel}</file>
</files_to_modify>

<validation_criteria>
  <criterion>t=0 populates exactly {EAst, ECfg, EDataFlow, ETypeGraph, ETest, EProblem}; other slots not in filled_mask</criterion>
  <criterion>t=1 populates above + {EDiff, ECommitMsg, EWitness (partial)}; ETrace/EOracle still not in filled_mask</criterion>
  <criterion>t=2 populates ALL 15 slots; filled_mask == VALID_FILLED_MASK</criterion>
  <criterion>materialize is deterministic (byte-equal across calls)</criterion>
  <criterion>E_TypeGraph toolchain-missing recorded in `failures` not raised</criterion>
  <criterion>Empty AttemptContext rejected</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>p50 latency &lt; 100 ms warm; p99 &lt; 500 ms cold (NFR-INSTR-01)</criterion>
  <criterion>file under 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments materialize::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments materialize_per_language</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
