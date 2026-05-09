# TASK-INSTR-016 — ECommitMsgInstrument neural backing (E1 e5-large-v2 + 1024→384 projection)

```xml
<task_spec id="TASK-INSTR-016" version="1.0">

<metadata>
  <title>ECommitMsgInstrument neural backing — frozen E1 forward + 1024→384 linear projection over commit message text</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements>
    <item ref="REQ-INSTR-11">ECommitMsgInstrument 384-d (frozen E1 + 1024-&gt;384 projection)</item>
    <item ref="component:ECommitMsgInstrument">encode(TextInstrumentInput) -&gt; 384 f32</item>
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
Replaces shipped deterministic ECommitMsgInstrument with neural backing per REQ-INSTR-11: forward
commit message text through frozen E1 (e5-large-v2, 1024-d), then project 1024-&gt;384 with frozen
linear layer. Frozen invariant.

Reuses E1 forward path / cache from EProblemInstrument and ETestInstrument.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-11)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts ECommitMsgInstrument)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_text.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>E1 e5-large-v2 weights pinned</check>
  <check>1024-&gt;384 projection weights frozen + checked into operator-controlled keyring</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace ECommitMsgInstrument deterministic encode with neural backing
    - Forward commit message text through frozen E1 (no_grad) -&gt; 1024-d
    - Project 1024-&gt;384 with frozen linear layer
    - `ECommitMsgInstrument::new_with_weights(e1: &amp;E1Embedder, projection: &amp;LinearProjection, guard: &amp;mut FrozenGuard)`
    - Fail-closed when E1 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - Reject text &gt; 16 KB / empty
    - File-SHA-keyed cache (shared with EProblem/ETest)
    - Determinism + frozen-gradient tests
  </in_scope>
  <out_of_scope>
    - Commit message generation (out of ME-JEPA scope)
    - Linear-probe gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_text.rs">
pub struct ECommitMsgInstrument {
    e1: Arc&lt;E1Embedder&gt;,
    projection: Arc&lt;LinearProjection&gt;,    // 1024-&gt;384 frozen
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl ECommitMsgInstrument {
    pub fn new_with_weights(
        e1: Arc&lt;E1Embedder&gt;,
        projection: Arc&lt;LinearProjection&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for ECommitMsgInstrument {
    type Input = TextInstrumentInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::ECommitMsg }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 384 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 384 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Reject text &gt; 16 KB / empty
    - Latency budget: &lt; 5 ms (E1-derived)
    - Deterministic
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_commit_msg::
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_text.rs:

  pub struct ECommitMsgInstrument { e1, projection, cache, embedder_versions }

  impl Instrument for ECommitMsgInstrument {
      fn encode(&amp;self, input: &amp;TextInstrumentInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.text.is_empty() { return Err(InstrumentError::invalid("input.text", "empty", "...")); }
          if input.text.len() &gt; 16 * 1024 { return Err(InstrumentError::invalid("input.text", "&gt; 16 KB", "...")); }
          let key = (sha256(&amp;input.text), self.embedder_versions.e1.clone());
          let raw_1024 = if let Some(v) = self.cache.lock().get(&amp;key) { v.clone() }
              else {
                  let v = self.e1.forward_no_grad("search_document: ", &amp;input.text)?;
                  self.cache.lock().put(key, v.clone());
                  v
              };
          let projected = self.projection.forward_no_grad(&amp;raw_1024)?; // 384-d
          validate_finite_output("e_commit_msg.output", &amp;projected)?;
          Ok(projected)
      }
  }

tests:
  - encode_empty_text_rejected
  - encode_oversize_text_rejected
  - encode_byte_equal_across_calls
  - encode_caches_repeated_text
  - encode_fails_when_e1_missing
  - encode_output_finite_and_dim_384
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_text.rs">Replace ECommitMsgInstrument deterministic encode with neural backing</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 384 finite f32</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 5 ms</criterion>
  <criterion>Empty / oversize text rejected</criterion>
  <criterion>Missing E1 -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_commit_msg::</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
