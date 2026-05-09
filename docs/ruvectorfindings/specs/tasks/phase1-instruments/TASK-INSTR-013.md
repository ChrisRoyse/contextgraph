# TASK-INSTR-013 — ETraceInstrument neural backing (sequence transformer over E2/E3/E4-encoded events)

```xml
<task_spec id="TASK-INSTR-013" version="1.0">

<metadata>
  <title>ETraceInstrument neural backing — frozen 6-layer sequence transformer (hidden 256, 8 heads) over E2/E3/E4-encoded trace events</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-INSTR-06">ETraceInstrument 512-d (E2/E3/E4 + sequence transformer)</item>
    <item ref="component:ETraceInstrument">encode(TraceInput) -&gt; 512 f32</item>
    <item ref="EC-INSTR-08">events &gt; 1M rejected fail-closed</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
Replaces shipped deterministic ETraceInstrument with neural backing per REQ-INSTR-06: encode each
TraceEvent as (kind one-hot, fn_id hashed bin, line_no normalized, val_hash fold, timestamp Δ
from sequence head) using temporal embedders E2/E3/E4 (multi-scale exponential decay), then run a
6-layer / hidden 256 / 8 heads frozen sequence transformer; mean-pool over event embeddings -&gt;
512-d.

Per EC-INSTR-08, sequences &gt; 1M events are rejected fail-closed (`MEJEPA_INSTRUMENTS_INVALID_INPUT`).
Per `MAX_TRACE_EVENTS = 1_000_000` constant.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-06, EC-INSTR-08)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ETraceInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_trace.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §3 row E_Trace</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>E2/E3/E4 temporal embedders pinned with SHA-256</check>
  <check>6-layer sequence transformer weights frozen + checked into operator-controlled keyring</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace ETraceInstrument deterministic encode with neural backing
    - Per-event embedding: (kind one-hot 5-d, fn_id hash bin 8-d, line_no normalized 1-d, val_hash 8-d, timestamp_ns delta via E2/E3/E4 temporal positional encoding)
    - 6-layer sequence transformer (hidden 256, 8 heads) frozen forward
    - Mean-pool over event embeddings -&gt; 512-d
    - `ETraceInstrument::new_with_weights(temporal: &amp;E2E3E4, transformer: &amp;SequenceTransformer6L, guard: &amp;mut FrozenGuard)` constructor
    - Fail-closed when events.len() &gt; MAX_TRACE_EVENTS (1M)
    - Fail-closed when events is empty (no silent zero-fill)
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Training transformer or temporal embedders
    - Trace collection (Phase 5 self-healing's job)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_trace.rs">
pub struct ETraceInstrument {
    temporal: Arc&lt;E2E3E4&gt;,                  // E2/E3/E4 frozen
    transformer: Arc&lt;SequenceTransformer6L&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl ETraceInstrument {
    pub fn new_with_weights(
        temporal: Arc&lt;E2E3E4&gt;,
        transformer: Arc&lt;SequenceTransformer6L&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for ETraceInstrument {
    type Input = TraceInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ETrace }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 512 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 512 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Reject events.len() &gt; MAX_TRACE_EVENTS (1M)
    - Reject empty events
    - Latency budget: &lt; 50 ms cold per NFR-INSTR-02 (sequence transformer over up-to-1M events; in practice batched)
    - Deterministic
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_trace::
    - Frozen-gradient test
    - Boundary: 0 events / 1 event / MAX events / MAX+1 events
  </verification>
</definition_of_done>

<pseudo_code>
e_trace.rs:

  pub struct ETraceInstrument { temporal, transformer, embedder_versions }

  impl Instrument for ETraceInstrument {
      fn encode(&amp;self, input: &amp;TraceInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.events.is_empty() { return Err(InstrumentError::invalid("input.events", "empty", "...")); }
          if input.events.len() &gt; MAX_TRACE_EVENTS {
              return Err(InstrumentError::invalid("input.events", format!("{} &gt; {}", input.events.len(), MAX_TRACE_EVENTS), "..."));
          }
          let head_ts = input.events[0].timestamp_ns;
          let event_embeddings: Vec&lt;Vec&lt;f32&gt;&gt; = input.events.iter().map(|e| {
              let kind_oh = encode_kind_one_hot(e.kind);  // 5-d
              let fn_bin = hash_fn_id(e.fn_id, 8);         // 8-d
              let line_norm = (e.line_no as f32) / 65536.0; // 1-d
              let val_hash = encode_val_hash(e.val_hash);   // 8-d
              let dt = e.timestamp_ns.saturating_sub(head_ts);
              let pos = self.temporal.encode_no_grad(dt as f32)?; // E2/E3/E4 multi-scale, e.g. 32-d
              concat(kind_oh, fn_bin, line_norm, val_hash, pos)
          }).collect::&lt;Result&lt;_, _&gt;&gt;()?;
          let pooled = self.transformer.forward_no_grad(&amp;event_embeddings)?;  // 512-d
          validate_finite_output("e_trace.output", &amp;pooled)?;
          Ok(pooled)
      }
  }

tests:
  - encode_empty_events_rejected
  - encode_one_event_succeeds
  - encode_at_max_events_succeeds
  - encode_above_max_events_rejected
  - encode_byte_equal_across_calls
  - encode_output_finite_and_dim_512
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_trace.rs">Replace deterministic encoder with neural backing; sequence transformer; FrozenGuard wiring</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 512 finite f32</criterion>
  <criterion>events.len() &gt; 1M rejected with MEJEPA_INSTRUMENTS_INVALID_INPUT</criterion>
  <criterion>Empty events rejected</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency profile: &lt; 50 ms cold</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_trace::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
