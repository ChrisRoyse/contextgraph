# TASK-EMBED-019 — E15-E21 learner-state lazy-load wrappers (one task, all 7)

```xml
<task_spec id="TASK-EMBED-019" version="1.0">
<metadata>
  <title>EmbedderInstrument wrappers + lazy-load factories for E15 AffectSpeech, E16 AffectFace, E17 AffectText, E18 PPG, E19 EDA, E20 EEG, E21 EegArtifactRobust</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <item ref="REQ-EMBED-05">load_learner_state lazy-load surface</item>
    <item ref="REQ-EMBED-09">E17 forward path used by agent_state_score</item>
    <item ref="REQ-EMBED-11">mejepa_load_learner_state MCP tool backing</item>
    <item ref="EC-EMBED-07">JSON-only transcripts encode through E17</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-009</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The 7 learner-state embedders (E15-E21). All lazy-loaded — NOT loaded by `Ensemble::load_content_set()`. Triggered explicitly via `mejepa_load_learner_state(slot)` MCP tool (TASK-EMBED-023) or auto-loaded on first observation of a learner-state input. E17 (AffectText) is the only one wired into ΔΩ_outer (TASK-EMBED-021); E15/E16/E18-E21 ship the instrument surface but live sensor wiring is out of scope for Phase 1b. Modalities: speech (Wav2Vec2 MSP-DIM, 1024-d), face (OpenFace 3.0 + MTL, 512-d), text (MiniLM + EmoBank, 384-d), PPG (PaPaGei, 512-d), EDA (WESAD CNN, 64-d), EEG (LaBraM, 768-d), EEG-artifact-robust (EEGPT, 768-d). Output dimensions match upstream `LearnerEmbedderSlot::output_dimension()`.
</context>

<input_context_files>
  <file purpose="learner_pattern">crates/context-graph-embeddings/src/learner/mod.rs</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="vram_table">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#vram_budget_table</file>
  <file purpose="e1_template">crates/context-graph-mejepa-embedders/src/embedders/e1_semantic.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-009 complete</check>
  <check>Upstream `LearnerEmbedderInput { Text { text }, Samples { samples }, Features { features } }` available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub struct E15AffectSpeechForward, E16AffectFaceForward, E17AffectTextForward, E18PpgForward, E19EdaForward, E20EegForward, E21EegArtifactRobustForward` — 7 forward structs
    - Each `impl EmbedderForward` validates `EmbedderInput::Learner { input }` (else InvalidInput); validates input modality matches embedder; runs forward; returns dim per upstream `output_dimension()`
    - **E17 specifics**: produces `scalars: { "valence": f32 in [-1,+1], "arousal": f32 in [0,1] }` IN ADDITION to the 384-d vector — used by `agent_state_score` (TASK-EMBED-021)
    - Each wrapper records `vram_estimate_bytes` per the VRAM table (E15 ~315 MB, E16 ~330 MB, E17 ~470 MB, E18 ~12 MB, E19 ~50 KB, E20 ~12 MB, E21 ~50 MB)
    - 7 Factories with `lazy = true` flag (TODO: extend `EmbedderForwardFactory` trait or use a `LazyEmbedderForwardFactory` subtrait that opts-in to lazy loading)
    - `Ensemble::load_learner_state(slot)` (already specified in TASK-EMBED-009 skeleton) calls these factories
    - `Ensemble::auto_unload_idle_learner_state(idle_threshold: Duration)` background helper — releases E15-E21 if idle > 5 min and VRAM headroom < 1 GB
    - 7 Instrument structs; impl Instrument with informational slot per spec
    - Tests:
      - E15-E21 NOT loaded by load_content_set (verified by inspecting ensemble.loaded after load_content_set)
      - load_learner_state(E17) succeeds; subsequent encode returns valid scalars
      - load_learner_state(E15-E21) under VRAM-tight stub returns VramBudgetExceeded
      - encode wrong modality input → InvalidInput
      - auto_unload_idle releases E17 after fake clock advance
  </in_scope>
  <out_of_scope>
    - Real audio / face / EEG pipelines — synthetic test inputs only
    - E17 calibration certificate verifier (TASK-EMBED-021)
    - Live sensor integrations (out of Phase 1b scope per SPEC-EMBEDDERS &lt;out_of_scope&gt;)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/embedders/learner_state.rs">
      pub const E15_VERSION: &amp;str = "wav2vec2-msp-dim";
      pub const E16_VERSION: &amp;str = "openface-3-mtl";
      pub const E17_VERSION: &amp;str = "minilm-emobank";
      pub const E18_VERSION: &amp;str = "papagei-ppg";
      pub const E19_VERSION: &amp;str = "wesad-eda-cnn";
      pub const E20_VERSION: &amp;str = "labram-eeg";
      pub const E21_VERSION: &amp;str = "eegpt-artifact-robust";

      pub const E15_PROJECTED_DIM: usize = 1024;
      pub const E16_PROJECTED_DIM: usize = 512;
      pub const E17_PROJECTED_DIM: usize = 384;
      pub const E18_PROJECTED_DIM: usize = 512;
      pub const E19_PROJECTED_DIM: usize = 64;
      pub const E20_PROJECTED_DIM: usize = 768;
      pub const E21_PROJECTED_DIM: usize = 768;

      // Per VRAM table:
      pub const E15_VRAM_BYTES: u64 = 330 * (1 &lt;&lt; 20);
      pub const E16_VRAM_BYTES: u64 = 346 * (1 &lt;&lt; 20);
      pub const E17_VRAM_BYTES: u64 = 493 * (1 &lt;&lt; 20);
      pub const E18_VRAM_BYTES: u64 = 12  * (1 &lt;&lt; 20);
      pub const E19_VRAM_BYTES: u64 = 50  * (1 &lt;&lt; 10);
      pub const E20_VRAM_BYTES: u64 = 12  * (1 &lt;&lt; 20);
      pub const E21_VRAM_BYTES: u64 = 50  * (1 &lt;&lt; 20);

      pub struct E15AffectSpeechForward { /* ... */ }
      pub struct E16AffectFaceForward   { /* ... */ }
      pub struct E17AffectTextForward   { /* ... */ }
      pub struct E18PpgForward          { /* ... */ }
      pub struct E19EdaForward          { /* ... */ }
      pub struct E20EegForward          { /* ... */ }
      pub struct E21EegArtifactRobustForward { /* ... */ }

      impl EmbedderForward for E15AffectSpeechForward { /* ... */ }
      impl EmbedderForward for E16AffectFaceForward { /* ... */ }
      impl EmbedderForward for E17AffectTextForward {
          /// Output: 384-d vector + scalars { "valence", "arousal" }.
          fn forward(&amp;self, input: &amp;EmbedderInput) -&gt; EmbedResult&lt;EmbedderOutput&gt;;
      }
      impl EmbedderForward for E18PpgForward { /* ... */ }
      impl EmbedderForward for E19EdaForward { /* ... */ }
      impl EmbedderForward for E20EegForward { /* ... */ }
      impl EmbedderForward for E21EegArtifactRobustForward { /* ... */ }

      pub struct E15AffectSpeechFactory;
      pub struct E16AffectFaceFactory;
      pub struct E17AffectTextFactory;
      pub struct E18PpgFactory;
      pub struct E19EdaFactory;
      pub struct E20EegFactory;
      pub struct E21EegArtifactRobustFactory;

      impl EmbedderForwardFactory for E15AffectSpeechFactory { /* lazy = true */ }
      // ... 6 more identical patterns

      pub fn register_all_lazy();

      // 7 Instrument structs:
      pub struct E15AffectSpeechInstrument { ensemble: Arc&lt;Ensemble&gt; }
      // ... E16..E21Instrument

      impl Instrument for E17AffectTextInstrument {
          type Input = EmbedderInput;
          fn slot(&amp;self) -&gt; InstrumentSlot { InstrumentSlot::Scalars } // E17 contributes valence/arousal scalars to ΔΩ_outer
          fn output_dim(&amp;self) -&gt; usize { E17_PROJECTED_DIM }
          /* ... */
      }
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines — split into multiple files if necessary (`learner_state/{e15.rs, e16.rs, ...}` and `mod.rs` re-exports).
    - All 7 NOT loaded by `Ensemble::load_content_set()` (regression test verifies this).
    - `register_all_lazy()` registers factories with a lazy flag; `Ensemble::load_one(E15..E21)` works but is gated by this flag (load_content_set skips lazy embedders).
    - E17 forward populates scalars `{ "valence": f32, "arousal": f32 }` non-empty.
    - E15-E21 input MUST match `LearnerEmbedderInput::modality()` per upstream contract; mismatch → InvalidInput.
    - All E1-pattern constraints apply (provenance, no mocks-while-broken, no unwrap, validate_finite + validate_dim).
    - VRAM constants from the spec's vram_budget_table (E17 ~470 MB rounded up to E17_VRAM_BYTES = 493 MiB for safety margin).
    - `auto_unload_idle_learner_state` checks `last_accessed` timestamp on each LoadedEmbedder; if past threshold AND VRAM headroom < 1 GB → release. v1 ships only the helper API; the tokio scheduler that calls it is out of scope.
  </constraints>

  <verification>
    - load_content_set leaves E15-E21 unloaded
    - load_learner_state(E17) loads E17; encode returns 384-d vector + scalars
    - encode wrong modality → InvalidInput
    - VRAM-tight stub → VramBudgetExceeded
    - cargo test learner_state passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
learner_state.rs (or split into per-embedder files):
  - 7 forwards + 7 factories + 7 instruments.
  - Lazy flag in factory: `fn is_lazy(&self) -> bool { true }` — Ensemble::load_content_set filters them out.
  - E17 forward returns BTreeMap with valence/arousal scalars.
  - register_all_lazy() registers all 7.
  - Tests verify content-set load skips lazy.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/learner_state.rs">All 7 wrappers + register_all_lazy + tests (split if >500 lines)</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/embedders/mod.rs">Add `pub mod learner_state;`</file>
  <file path="crates/context-graph-mejepa-embedders/src/ensemble.rs">Extend EmbedderForwardFactory with `fn is_lazy(&self) -> bool { false }`; load_content_set skips lazy factories</file>
</files_to_modify>

<validation_criteria>
  <criterion>load_content_set leaves E15-E21 unloaded</criterion>
  <criterion>load_learner_state(E17) loads E17 and encode returns valid output</criterion>
  <criterion>E17 scalars include valence + arousal</criterion>
  <criterion>Wrong modality input rejected with InvalidInput</criterion>
  <criterion>VRAM-tight scenario returns VramBudgetExceeded</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders learner_state</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
