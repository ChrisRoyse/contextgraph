# TASK-EMBED-022 — Diagnostics: embedder_coherence + predictor_collapse_score

```xml
<task_spec id="TASK-EMBED-022" version="1.0">
<metadata>
  <title>diagnostics::embedder_coherence (jaccard NN over content embedders) + predictor_collapse_score (cross-embedder std)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>22</sequence>
  <implements>
    <item ref="REQ-EMBED-07">embedder_coherence(panel)</item>
    <item ref="REQ-EMBED-08">predictor_collapse_score(predicted, target)</item>
    <item ref="EC-EMBED-06">corpus &lt; 1000 returns 0.5 + low_corpus flag</item>
    <item ref="TECH-EMBEDDERS::component_contracts::DiagnosticsModule">diagnostics.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-010</task_ref>
    <task_ref>TASK-EMBED-015</task_ref>
    <task_ref>TASK-EMBED-016</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Two cross-embedder structural metrics feeding ΔK / ΔΞ in UTML. `embedder_coherence` measures the average Jaccard overlap of k-NN sets across content embedders for the same panel — high coherence means the panel's slots agree about which corpus entries are nearest. `predictor_collapse_score = 1 - std(cosines)` flags collapsed predictors whose 14 cosines uniformly agree (high score) vs. healthy predictors whose cosines spread (low score).
</context>

<input_context_files>
  <file purpose="diagnostics_spec">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#api_contracts (diagnostics)</file>
  <file purpose="instrument_panel">crates/context-graph-mejepa-instruments/src/lib.rs</file>
  <file purpose="embedder_id">crates/context-graph-mejepa-embedders/src/embedder_id.rs</file>
  <file purpose="solver_helpers">crates/context-graph-solver/src/lib.rs (cosine helpers if available)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-010 / 015 / 016 complete (content embedders functional)</check>
  <check>Panel + InstrumentSlot from upstream context-graph-mejepa-instruments</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub trait CorpusAnnIndex { fn entry_count(&self) -> usize; fn knn(&self, query: &[f32], k: usize, slot: InstrumentSlot) -> EmbedResult<Vec<u64>>; }` — abstract index trait for testing
    - `pub struct CoherenceResult { pub score: f32, pub low_corpus_size: bool, pub pair_count: usize, pub computed_at: DateTime<Utc> }`
    - `pub fn embedder_coherence(panel: &Panel, corpus_index: &dyn CorpusAnnIndex) -> EmbedResult<CoherenceResult>`:
      1. If `corpus_index.entry_count() < 1000` → return `CoherenceResult { score: 0.5, low_corpus_size: true, pair_count: 0 }`
      2. For each (i,j) pair in content-embedder slot list, i < j:
         a. Pull panel slot for E_i (or projected) → query NN k=20
         b. Pull panel slot for E_j → query NN k=20
         c. j_ij = jaccard(NN_i, NN_j) ∈ [0,1]
      3. score = mean(j_ij over all pairs); clamp [0,1]
      4. Return CoherenceResult
    - `pub fn predictor_collapse_score(predicted: &Panel, target: &Panel) -> EmbedResult<f32>`:
      1. For each E_i ∈ E1..E14 with a corresponding panel slot:
         a. cos_i = cosine(predicted.slot_for(E_i), target.slot_for(E_i))
      2. s = sample_std([cos_1..cos_14]) (n-1 denominator)
      3. Return clamp(1.0 - s, 0.0, 1.0)
    - `pub fn cosine(a: &[f32], b: &[f32]) -> EmbedResult<f32>` — fail-closed on zero-norm or mismatched len; returns raw cosine in [-1,1]
    - `pub fn sample_std(xs: &[f32]) -> EmbedResult<f32>` — n-1 denominator; fail-closed if n < 2
    - `pub fn jaccard(a: &[u64], b: &[u64]) -> f32` — symmetric Jaccard over deduped sorted vecs; returns 0.0 on both-empty
    - Tests:
      - corpus_index < 1000 → score = 0.5, low_corpus_size = true
      - synthetic 50-entry mini-corpus + canned NN sets → coherence matches hand-computed reference
      - constant-output predictor → collapse_score ≈ 1.0
      - diverse predictor with spread cosines → collapse_score < 0.7
      - jaccard happy-path + edge cases (both empty, no overlap, full overlap)
      - cosine zero-norm → InvalidInput (fail-closed)
      - sample_std on n=1 → InvalidInput
  </in_scope>
  <out_of_scope>
    - Real HNSW index integration (TASK-EMBED-022 ships the trait + tests; production `CorpusAnnIndex` impl ships in INSTRUMENTS / shift-subscriber domain)
    - Mutual-information audit (`mejepa_audit_pairwise_mi` MCP tool; out of scope for this task — see TASK-EMBED-023 which only ships 3 simpler MCP tools)
    - Per-embedder slot mapping E_i → panel slot (slot ownership is in INSTRUMENTS domain; this task uses `panel.slot_for(InstrumentSlot::EProblem)` proxy and a const map `embedder_to_panel_slot` for E1-E14)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-embedders/src/diagnostics.rs">
      use chrono::{DateTime, Utc};
      use context_graph_mejepa_instruments::{Panel, InstrumentSlot};

      use crate::EmbedderId;
      use crate::error::{EmbedError, EmbedResult};

      pub trait CorpusAnnIndex: Send + Sync {
          fn entry_count(&amp;self) -&gt; usize;
          fn knn(&amp;self, query: &amp;[f32], k: usize, slot: InstrumentSlot) -&gt; EmbedResult&lt;Vec&lt;u64&gt;&gt;;
      }

      #[derive(Debug, Clone, PartialEq)]
      pub struct CoherenceResult {
          pub score: f32,
          pub low_corpus_size: bool,
          pub pair_count: usize,
          pub computed_at: DateTime&lt;Utc&gt;,
      }

      pub const COHERENCE_MIN_CORPUS: usize = 1000;
      pub const COHERENCE_K: usize = 20;

      pub fn embedder_coherence(
          panel: &amp;Panel,
          corpus_index: &amp;dyn CorpusAnnIndex,
      ) -&gt; EmbedResult&lt;CoherenceResult&gt;;

      pub fn predictor_collapse_score(predicted: &amp;Panel, target: &amp;Panel) -&gt; EmbedResult&lt;f32&gt;;

      pub fn cosine(a: &amp;[f32], b: &amp;[f32]) -&gt; EmbedResult&lt;f32&gt;;
      pub fn sample_std(xs: &amp;[f32]) -&gt; EmbedResult&lt;f32&gt;;
      pub fn jaccard(a: &amp;[u64], b: &amp;[u64]) -&gt; f32;

      /// Static map from EmbedderId to its representative InstrumentSlot for diagnostics.
      pub fn embedder_to_panel_slot(id: EmbedderId) -&gt; Option&lt;InstrumentSlot&gt;;
    </signature>
  </signatures>

  <constraints>
    - File &lt; 500 lines.
    - corpus_index.entry_count() &lt; 1000 → score = 0.5 hardcoded with low_corpus_size = true (per EC-EMBED-06).
    - cosine fail-closed on zero-norm (NO `unwrap_or(0.0)` per constitution `<error_handling>`).
    - sample_std n-1 denominator; fail-closed on n &lt; 2.
    - jaccard sorted-dedup union/intersection; both-empty returns 0.0 (no NaN).
    - All math finite; validate per output before returning.
    - Latency target NFR-EMBED-01: &lt; 50 ms per panel at corpus-size 100k (architectural target; tests use small synthetic).
  </constraints>

  <verification>
    - corpus &lt; 1000 → 0.5 + low_corpus_size flag
    - 50-entry mini-corpus reference matches hand-computed
    - constant-output predictor → collapse_score ≈ 1.0
    - diverse predictor → collapse_score &lt; 0.7
    - jaccard / cosine / sample_std edge cases pass
    - cargo test diagnostics passes
    - cargo clippy clean
  </verification>
</definition_of_done>

<pseudo_code>
diagnostics.rs:
  - embedder_coherence: gather slot vectors per content embedder; for each (i,j) pair, run knn + jaccard; mean.
  - predictor_collapse_score: 14 cosines; sample_std; 1 - s.
  - cosine, sample_std, jaccard helpers as documented.
  - embedder_to_panel_slot const-match: E1→EProblem, E5→EReasoning, E7→EAst, E10→ECommitMsg, E11→EAst (or similar; pick representative slot per spec).
  - Tests with stub CorpusAnnIndex implementation.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-embedders/src/diagnostics.rs">embedder_coherence + predictor_collapse_score + helpers + tests</file>
  <file path="crates/context-graph-mejepa-embedders/tests/diagnostics_integration.rs">Integration test wiring stub corpus index + predictor synthetic panels</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-embedders/src/lib.rs">Add `pub mod diagnostics;` + re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>corpus &lt; 1000 → score = 0.5 with low_corpus_size flag</criterion>
  <criterion>50-entry reference matches hand-computed coherence</criterion>
  <criterion>constant predictor → collapse_score ≈ 1.0</criterion>
  <criterion>diverse predictor → collapse_score &lt; 0.7</criterion>
  <criterion>cosine zero-norm fail-closed (no unwrap_or)</criterion>
  <criterion>sample_std n &lt; 2 fail-closed</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mejepa-embedders diagnostics</command>
  <command>cargo clippy -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
