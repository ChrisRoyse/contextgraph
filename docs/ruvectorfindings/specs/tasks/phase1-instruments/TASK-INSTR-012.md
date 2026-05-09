# TASK-INSTR-012 — ETestInstrument neural backing (E1 e5-large-v2 + 1024→384 projection)

```xml
<task_spec id="TASK-INSTR-012" version="1.0">

<metadata>
  <title>ETestInstrument neural backing — frozen E1 e5-large-v2 forward + 1024→384 projection over test descriptions</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-INSTR-05">ETestInstrument 384-d (frozen E1)</item>
    <item ref="REQ-INSTR-19">AST-perfect chunk consumption</item>
    <item ref="component:ETestInstrument">encode(TextInstrumentInput) -&gt; 384 f32 (or per neural plan: encode(CodeInstrumentInput))</item>
    <item ref="constitution:utml_v1_invariants rule 2">Frozen</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-003</task_ref>
    <task_ref>TASK-INSTR-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file modified, 0 created</estimated_files>
</metadata>

<context>
The shipped ETestInstrument is a deterministic typed encoder over test descriptions. This task
replaces it with neural backing per REQ-INSTR-05: forward through frozen E1 (e5-large-v2, 1024-d)
on AST-perfect chunks bounded by test functions + their natural-language descriptions
(docstrings, names, assertion messages), then project 1024→384 with a frozen linear layer.

Per REQ-INSTR-19, the input MUST be AST-chunk-typed (not raw String). The chunker bounds chunks
by test functions (entity_type ∈ {Function, Method} with test naming heuristics OR explicit
test framework markers per-language: pytest, unittest, JUnit, NUnit, xUnit, mocha, jest, rspec,
go test).
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-05, REQ-INSTR-19)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ETestInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_text.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §3 row E_Test</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>E1 e5-large-v2 weights pinned with SHA-256 digest</check>
  <check>1024-&gt;384 projection weights frozen + checked into operator-controlled keyring</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace ETestInstrument deterministic encode with neural backing
    - Forward each test-function chunk through frozen E1 (no_grad) -&gt; 1024-d
    - Mean-pool across chunks weighted by description length
    - Project 1024 -&gt; 384 with frozen linear layer
    - `ETestInstrument::new_with_weights(e1: &amp;E1Embedder, projection: &amp;LinearProjection, guard: &amp;mut FrozenGuard)` constructor
    - Fail-closed when E1 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - File-SHA-keyed cache: `(chunk.sha256, embedder_version)`
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Training E1 or projection weights
    - Test runner integration (E_Oracle's job)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_text.rs">
pub struct ETestInstrument {
    e1: Arc&lt;E1Embedder&gt;,                  // E1 e5-large-v2 frozen
    projection: Arc&lt;LinearProjection&gt;,    // 1024-&gt;384 frozen
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl ETestInstrument {
    pub fn new_with_weights(
        e1: Arc&lt;E1Embedder&gt;,
        projection: Arc&lt;LinearProjection&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for ETestInstrument {
    type Input = CodeInstrumentInput;       // typed, not raw String
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ETest }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 384 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 384 finite f32
    - Frozen: requires_grad=false; FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Latency budget: &lt; 5 ms (E1-derived per NFR-INSTR-02)
    - Deterministic
    - Empty test chunks rejected
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_test::
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_text.rs:

  pub struct ETestInstrument { e1, projection, cache, embedder_versions }

  impl Instrument for ETestInstrument {
      fn encode(&amp;self, input: &amp;CodeInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          let test_chunks: Vec&lt;_&gt; = input.chunks.iter().filter(|c| c.is_test_function(input.language)).collect();
          if test_chunks.is_empty() {
              return Err(InstrumentError::invalid("input.chunks", "no test functions found", "..."));
          }
          let mut acc = vec![0.0_f32; 1024];
          let mut total_weight = 0.0_f32;
          for chunk in &amp;test_chunks {
              let key = (chunk.sha256, self.embedder_versions.e1.clone());
              let v = if let Some(v) = self.cache.lock().get(&amp;key) { v.clone() }
                  else {
                      let v = self.e1.forward_no_grad("search_document: ", &amp;chunk.test_description_text())?;
                      self.cache.lock().put(key, v.clone());
                      v
                  };
              let weight = chunk.description_len_chars() as f32;
              for (i, x) in v.iter().enumerate() { acc[i] += weight * x; }
              total_weight += weight;
          }
          for x in &amp;mut acc { *x /= total_weight; }
          let projected = self.projection.forward_no_grad(&amp;acc)?;  // 384-d
          validate_finite_output("e_test.output", &amp;projected)?;
          Ok(projected)
      }
  }

tests:
  - encode_empty_chunks_rejected
  - encode_no_test_functions_rejected
  - encode_byte_equal_across_calls
  - encode_caches_repeated_chunk
  - encode_fails_when_e1_missing
  - encode_output_finite_and_dim_384
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_text.rs">Replace ETestInstrument deterministic encode with neural backing</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 384 finite f32</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 5 ms</criterion>
  <criterion>No-test-function input rejected</criterion>
  <criterion>Missing E1 weights -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_test::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
