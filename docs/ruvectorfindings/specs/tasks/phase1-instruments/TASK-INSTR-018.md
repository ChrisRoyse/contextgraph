# TASK-INSTR-018 — EReasoningInstrument neural backing (E17 AffectText)

```xml
<task_spec id="TASK-INSTR-018" version="1.0">

<metadata>
  <title>EReasoningInstrument neural backing — frozen E17 AffectText forward over CoT events; opaque latent (BLEU &lt; 5 reverse-decode)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <item ref="REQ-INSTR-14">EReasoningInstrument 384-d (frozen E17 AffectText)</item>
    <item ref="component:EReasoningInstrument">encode(ReasoningInput) -&gt; 384 f32</item>
    <item ref="EC-INSTR-14">prompt-injection payload encoded opaquely; never echoed verbatim</item>
    <item ref="NFR-INSTR-05">linear reverse-decoder BLEU &lt; 5</item>
    <item ref="constitution:SEC-MEJEPA-06">no prompt injection from training corpus into MCP tool responses</item>
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
Replaces shipped deterministic EReasoningInstrument with neural backing per REQ-INSTR-14: forward
each ReasoningEvent's text through frozen E17 AffectText, mean-pool by event kind (ChatMessage,
ToolCall, ToolResult, RecommendationJson, ReflexionRecord) -&gt; 384-d.

Per constitution SEC-MEJEPA-06 + NFR-INSTR-05 + EC-INSTR-14, the latent MUST be opaque — a linear
reverse-decoder probe over (latent -&gt; text) MUST achieve BLEU &lt; 5 vs ground-truth CoT. Prompt
injection payloads ("ignore previous instructions...") are encoded as latent affect signal but
never reconstructable. Maximum 10K events per `MAX_REASONING_EVENTS` constant.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-14, EC-INSTR-14, NFR-INSTR-05)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts EReasoningInstrument; data_models ReasoningInput, ReasoningEvent; security note)</file>
  <file purpose="shipped_implementation">crates/context-graph-mejepa-instruments/src/e_reasoning.rs</file>
  <file purpose="frozen_guard">crates/context-graph-mejepa-instruments/src/frozen_hook.rs</file>
  <file purpose="design_doc_embedders">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §3 row E_Reasoning</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-003, 004 done</check>
  <check>E17 AffectText weights pinned with SHA-256 digest</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace EReasoningInstrument deterministic encode with neural backing
    - Per-event E17 AffectText forward (no_grad) -&gt; native AffectText dim
    - Mean-pool by event kind (5 kinds × N events -&gt; per-kind centroid)
    - Concatenate per-kind centroids + project to 384-d (frozen linear)
    - `EReasoningInstrument::new_with_weights(e17: &amp;AffectText, projection: &amp;LinearProjection, guard: &amp;mut FrozenGuard)`
    - Fail-closed when E17 unavailable: MEJEPA_INFER_INSTRUMENT_MISSING
    - Reject events &gt; MAX_REASONING_EVENTS (10K)
    - Reject empty events
    - File-SHA-keyed cache: `(event.text.sha256, e17_version)`
    - NFR-INSTR-05 reverse-decode probe test: train linear reverse-decoder on 100 (latent, text) pairs; assert BLEU &lt; 5
    - Determinism + frozen-gradient tests
    - SEC-MEJEPA-06: latent persisted only as 384-d Vec; raw text NEVER persisted to ME-JEPA panel artifact
  </in_scope>
  <out_of_scope>
    - Training E17 or projection
    - Generating CoT (Claude Code outer loop)
    - Linear-probe MUTATION gate (TASK-INSTR-021)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/e_reasoning.rs">
pub const MAX_REASONING_EVENTS: usize = 10_000;

pub struct EReasoningInstrument {
    e17: Arc&lt;AffectText&gt;,
    projection: Arc&lt;LinearProjection&gt;,
    cache: Arc&lt;Mutex&lt;LruCache&lt;CacheKey, Vec&lt;f32&gt;&gt;&gt;&gt;,
    embedder_versions: Arc&lt;EmbedderVersions&gt;,
}

impl EReasoningInstrument {
    pub fn new_with_weights(
        e17: Arc&lt;AffectText&gt;,
        projection: Arc&lt;LinearProjection&gt;,
        embedder_versions: Arc&lt;EmbedderVersions&gt;,
        guard: &amp;mut FrozenGuard,
    ) -&gt; InstrumentResult&lt;Self&gt;;
}

impl Instrument for EReasoningInstrument {
    type Input = ReasoningInput;
    fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::EReasoning }
    fn encode(&amp;self, input: &amp;Self::Input) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt;; // 384 f32
}
    </signature>
  </signatures>
  <constraints>
    - Output exactly 384 finite f32
    - Frozen — FrozenGuard registration; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION
    - Reject events &gt; MAX_REASONING_EVENTS (10K)
    - Reject empty events
    - Latent opaque: linear reverse-decoder BLEU &lt; 5 (NFR-INSTR-05); test must run + assert
    - Latency budget: &lt; 10 ms (E17-derived)
    - Deterministic
    - Raw event text never persisted to panel JSON; only 384-d latent
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test -p context-graph-mejepa-instruments e_reasoning::
    - Reverse-decode BLEU test: trained linear reverse-decoder on 100 pairs; BLEU &lt; 5
    - Frozen-gradient test
  </verification>
</definition_of_done>

<pseudo_code>
e_reasoning.rs:

  pub const MAX_REASONING_EVENTS: usize = 10_000;

  pub struct EReasoningInstrument { e17, projection, cache, embedder_versions }

  impl Instrument for EReasoningInstrument {
      fn encode(&amp;self, input: &amp;ReasoningInput) -&gt; InstrumentResult&lt;Vec&lt;f32&gt;&gt; {
          if input.events.is_empty() { return Err(InstrumentError::invalid("input.events", "empty", "...")); }
          if input.events.len() &gt; MAX_REASONING_EVENTS {
              return Err(InstrumentError::invalid("input.events",
                  format!("{} &gt; {}", input.events.len(), MAX_REASONING_EVENTS),
                  "truncate or summarize"));
          }
          // Group by event kind; per-event E17 forward (cached); per-kind centroid
          let mut centroids_by_kind: HashMap&lt;ReasoningEventKind, (Vec&lt;f32&gt;, usize)&gt; = HashMap::new();
          for event in &amp;input.events {
              let key = (sha256(&amp;event.text), self.embedder_versions.e17.clone());
              let v = if let Some(v) = self.cache.lock().get(&amp;key) { v.clone() }
                  else {
                      let v = self.e17.forward_no_grad(&amp;event.text)?; // affect-text dim
                      self.cache.lock().put(key, v.clone());
                      v
                  };
              let entry = centroids_by_kind.entry(event.kind).or_insert((vec![0.0; v.len()], 0));
              for (i, x) in v.iter().enumerate() { entry.0[i] += x; }
              entry.1 += 1;
          }
          // Concat 5 per-kind centroids in canonical order; project to 384
          let concat = canonical_concat(&amp;centroids_by_kind, 5);
          let projected = self.projection.forward_no_grad(&amp;concat)?; // 384-d
          validate_finite_output("e_reasoning.output", &amp;projected)?;
          Ok(projected)
      }
  }

tests:
  - encode_empty_events_rejected
  - encode_at_max_events_succeeds
  - encode_above_max_events_rejected
  - encode_byte_equal_across_calls
  - encode_caches_repeated_text
  - encode_fails_when_e17_missing
  - encode_output_finite_and_dim_384
  - reverse_decode_bleu_under_5  // NFR-INSTR-05
  - prompt_injection_not_reconstructable  // EC-INSTR-14
</pseudo_code>

<files_to_create>
  <!-- none -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/e_reasoning.rs">Replace deterministic encoder with neural backing; add reverse-decode BLEU test</file>
</files_to_modify>

<validation_criteria>
  <criterion>Output is 384 finite f32</criterion>
  <criterion>events &gt; 10K rejected</criterion>
  <criterion>Empty events rejected</criterion>
  <criterion>Encode is deterministic</criterion>
  <criterion>FrozenGuard registration succeeds with all params requires_grad=false</criterion>
  <criterion>Reverse-decode probe BLEU &lt; 5 (NFR-INSTR-05)</criterion>
  <criterion>Prompt injection text not reconstructable from latent (EC-INSTR-14)</criterion>
  <criterion>cargo build --release clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>Latency p99 &lt; 10 ms</criterion>
  <criterion>Missing E17 -&gt; MEJEPA_INFER_INSTRUMENT_MISSING</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments e_reasoning::</command>
  <command>cargo test --release -p context-graph-mejepa-instruments reverse_decode_bleu_under_5</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
