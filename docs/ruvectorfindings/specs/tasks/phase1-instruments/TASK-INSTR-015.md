# TASK-INSTR-015 — EProblemInstrument neural backing (E1 e5-large-v2 1024-d direct)

```xml
<task_spec id="TASK-INSTR-015" version="1.0">

<metadata>
  <title>EProblemInstrument neural backing — frozen E1 e5-large-v2 forward, 1024-d native (no projection)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <item ref="REQ-INSTR-10">EProblemInstrument 1024-d (frozen E1)</item>
    <item ref="component:EProblemInstrument">encode(TextInstrumentInput) -&gt; 1024 f32</item>
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
Replaces shipped deterministic EProblemInstrument with neural backing per REQ-INSTR-10: forward
SWE-bench problem text (or generic problem text) through frozen E1 (e5-large-v2, 1024-d). No
projection — the panel slot is exactly 1024-d (E1's native output dim). Frozen invariant.

Per constitution `<security_requirements>` SEC-MEJEPA-02, E1 weights load from pinned SHA-256
digest. Mismatch -&gt; refuse to load.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-10)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EProblemInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_text.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>E1 e5-large-v2 weights pinned with SHA-256 digest</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace EProblemInstrument deterministic encode with neural backing
    - Forward problem text through frozen E1 (no_grad) -&gt; 1024-d
    - No projection (slot is 1024-d native)
    - `EProblemInstrument::new_with_weights(e1: &amp;E1Embedder, guard: &amp;mut FrozenGuard)` constructor
    - Fail-closed when E1 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - Fail-closed on text &gt; 16 KB (per data_model TextInstrumentInput constraint)
    - File-SHA-keyed cache: `(text.sha256, e1_version)`
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Training E1
    - Sentence segmentation / chunking (problem is one text)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_text.rs">
pub struct EProblemInstrument {
    e1: Arc&lt;E1Embedder&gt;,                  // E1 e5-large-v2 frozen
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl EProblemInstrument {
    pub fn new_with_weights(
        e1: Arc&lt;E1Embedder&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for EProblemInstrument {
    type Input = TextInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EProblem }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 1024 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 1024 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Reject text &gt; 16 KB (TextInstrumentInput max)
    - Reject empty text
    - Latency budget: &lt; 5 ms (E1-derived)
    - Deterministic
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_problem::
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_text.rs:

  pub struct EProblemInstrument { e1, cache, embedder_versions }

  impl Instrument for EProblemInstrument {
      fn encode(&amp;self, input: &amp;TextInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.text.is_empty() {
              return Err(InstrumentError::invalid("input.text", "empty", "supply non-empty problem text"));
          }
          if input.text.len() &gt; 16 * 1024 {
              return Err(InstrumentError::invalid("input.text", "&gt; 16 KB", "truncate or summarize problem text"));
          }
          let key = (sha256(&amp;input.text), self.embedder_versions.e1.clone());
          let v = if let Some(v) = self.cache.lock().get(&amp;key) { v.clone() }
              else {
                  let v = self.e1.forward_no_grad("search_document: ", &amp;input.text)?;  // 1024-d
                  self.cache.lock().put(key, v.clone());
                  v
              };
          if v.len() != 1024 { return Err(InstrumentError::invariant("e_problem.output", "dim mismatch", "...")); }
          validate_finite_output("e_problem.output", &amp;v)?;
          Ok(v)
      }
  }

tests:
  - encode_empty_text_rejected
  - encode_oversize_text_rejected
  - encode_byte_equal_across_calls
  - encode_caches_repeated_text
  - encode_fails_when_e1_missing
  - encode_output_finite_and_dim_1024
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_text.rs">Replace EProblemInstrument deterministic encode with neural backing</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 1024 finite f32</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 5 ms</criterion>
  <criterion>Empty / oversize text rejected</criterion>
  <criterion>Missing E1 weights -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_problem::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
