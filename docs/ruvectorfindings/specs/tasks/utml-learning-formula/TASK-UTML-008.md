# TASK-UTML-008 — EmbedderCoherence (cross-embedder Jaccard NN-set agreement) + pairwise-MI audit job

```xml
<task_spec id="TASK-UTML-008" version="1.0">

<metadata>
  <title>compute_embedder_coherence (Jaccard agreement of NN sets across E1-E14) + pairwise_mi_audit (writes pairwise_mi_matrix.csv) + bootstrap_neutral fallback for &lt; 1000 corpus entries</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-UTML-03">embedder_coherence term in ΔK</item>
    <item ref="AC-UTML-03-03">Jaccard NN-set agreement across 14 content embedders</item>
    <item ref="AC-UTML-05-03">predictor_redundancy &gt; 0.5 → trigger pairwise-MI audit</item>
    <item ref="EC-UTML-11">First 1,000 corpus entries: bootstrap neutral 0.5</item>
    <item ref="component:EmbedderCoherence">crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 created</estimated_files>
</metadata>

<context>
embedder_coherence (doc 12 §5.1) is one of the four ΔK sub-components. Across 14 frozen content embedders (E1-E14), each panel has a top-K nearest-neighbor set under each embedder's metric. High mean pairwise Jaccard agreement = panel is recognized in all embedder feature spaces (low OOD risk). For the first 1,000 corpus entries the NN sets are not statistically meaningful, so we return a neutral 0.5 (NEVER 1.0 — would over-credit ΔK). Pairwise MI audit is the periodic job that detects panel-slot redundancy &gt; 0.5 and triggers VICReg coefficient bumps in Phase 5.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-03; AC-UTML-03-03; EC-UTML-11; AC-UTML-05-03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component EmbedderCoherence)</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §5.1 (cross-embedder Jaccard)</file>
  <file purpose="instrument_panel">crates/context-graph-mejepa-instruments/src/lib.rs (Panel struct)</file>
  <file purpose="prior_tasks">TASK-UTML-001..007</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-007 complete (sampling_weight + L_step shipped)</check>
  <check>Phase 1 instruments shipped (Panel + InstrumentSlot)</check>
  <check>21-embedder ensemble loadable (Phase 1c partial — 14 content embedders)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/embedder_coherence.rs` with:
      - `pub trait ContentEmbedder { fn nn_indices(&amp;self, panel: &amp;Panel, k: usize) -&gt; Result&lt;Vec&lt;usize&gt;, UtmlError&gt;; }`
      - `pub fn compute_embedder_coherence(panel: &amp;Panel, content_embedders: &amp;[Box&lt;dyn ContentEmbedder&gt;; 14], corpus_size: usize, bootstrap_threshold: usize, k_nn: usize) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt;`
      - jaccard helper
    - Create `src/learning_signal/pairwise_mi.rs` with:
      - `pub struct PairwiseMiSummary { pub mean_normalized_mi: f32, pub max_normalized_mi: f32, pub matrix_csv_path: PathBuf }`
      - `pub fn pairwise_mi_audit(panels: &amp;[Panel], output_dir: &amp;Path) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt;` writing `pairwise_mi_matrix.csv` (15×15 matrix per InstrumentSlot)
      - `pub fn predictor_redundancy_from_summary(summary: &amp;PairwiseMiSummary) -&gt; f32` (returns mean_normalized_mi clamped to [0,1])
    - Bootstrap fallback: if `corpus_size &lt; bootstrap_threshold` (default 1,000) → return `(0.5, "bootstrap_neutral_n_lt_1000")` immediately
    - Default k_nn = 50; configurable
    - Compute 91 pairwise Jaccards (14 choose 2 = 91); coherence = mean
    - Pairwise MI: use a deterministic histogram-based estimator over slot activations (1D MI between slot pairs); store full 15×15 symmetric matrix in CSV with InstrumentSlot::slug() row/column labels
    - CSV output mode 0o600 per SEC-MEJEPA-07
    - Unit tests with stub embedders: full agreement → 1.0; full disagreement (disjoint NN sets) → 0.0; bootstrap → 0.5; MI matrix size 15×15; symmetric matrix invariant
  </in_scope>
  <out_of_scope>
    - Real E1-E14 forward passes (Phase 1c neural backends)
    - VICReg coefficient bump intervention (Phase 5)
    - Constellation Gτ rate (Phase 4b)
    - Cert chain emission (TASK-TRAIN-009)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs">
use context_graph_mejepa_instruments::Panel;
use crate::error::UtmlError;

pub const DEFAULT_BOOTSTRAP_THRESHOLD: usize = 1000;
pub const DEFAULT_K_NN: usize = 50;
pub const NUM_CONTENT_EMBEDDERS: usize = 14;

pub trait ContentEmbedder: Send + Sync {
    fn nn_indices(&amp;self, panel: &amp;Panel, k: usize) -&gt; Result&lt;Vec&lt;usize&gt;, UtmlError&gt;;
    fn slug(&amp;self) -&gt; &amp;'static str; // "e1".."e14"
}

pub fn compute_embedder_coherence(
    panel: &amp;Panel,
    content_embedders: &amp;[Box&lt;dyn ContentEmbedder&gt;], // length must equal NUM_CONTENT_EMBEDDERS
    corpus_size: usize,
    bootstrap_threshold: usize,
    k_nn: usize,
) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt;;

fn jaccard(a: &amp;[usize], b: &amp;[usize]) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi.rs">
use std::path::{Path, PathBuf};
use crate::error::UtmlError;
use context_graph_mejepa_instruments::Panel;

#[derive(Debug, Clone)]
pub struct PairwiseMiSummary {
    pub mean_normalized_mi: f32,
    pub max_normalized_mi: f32,
    pub matrix_csv_path: PathBuf,
}

pub fn pairwise_mi_audit(
    panels: &amp;[Panel],
    output_dir: &amp;Path,
) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt;;

pub fn predictor_redundancy_from_summary(summary: &amp;PairwiseMiSummary) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - 14 content embedders: validate slice length; UtmlFactorOutOfRange on mismatch
    - corpus_size, k_nn validated &gt; 0
    - Jaccard = |A ∩ B| / |A ∪ B|; both empty → 0.0
    - Pairwise MI estimator: deterministic histogram bins (e.g., 32 bins per slot); normalized by min(H(X), H(Y))
    - CSV format: header row "slot,e_ast,e_cfg,...,scalars\n" + 15 data rows
    - File mode 0o600 on the CSV
    - Files &lt; 400 lines each
  </constraints>
  <verification>
    - cargo test shows ≥ 8 tests pass: bootstrap fallback, full agreement, full disagreement, jaccard symmetry, MI matrix shape 15×15, MI symmetric, CSV file mode 0o600
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_embedder_coherence(panel, embs, corpus, threshold, k) -&gt; Result&lt;(f32, &amp;'static str), UtmlError&gt; {
    if corpus &lt; threshold { return Ok((0.5, "bootstrap_neutral_n_lt_1000")); }
    if embs.len() != NUM_CONTENT_EMBEDDERS { return Err(UtmlFactorOutOfRange); }
    let mut nn = Vec::with_capacity(NUM_CONTENT_EMBEDDERS);
    for e in embs { nn.push(e.nn_indices(panel, k)?); }
    let mut sum = 0.0; let mut pairs = 0;
    for i in 0..NUM_CONTENT_EMBEDDERS {
        for j in (i+1)..NUM_CONTENT_EMBEDDERS {
            sum += jaccard(&amp;nn[i], &amp;nn[j]); pairs += 1;
        }
    }
    Ok(((sum / pairs as f32).clamp(0.0, 1.0), "computed"))
}

fn pairwise_mi_audit(panels, out_dir) -&gt; Result&lt;PairwiseMiSummary, UtmlError&gt; {
    let slots = InstrumentSlot::all(); // 15 slots
    let n = slots.len();
    let mut mat = vec![vec![0f32; n]; n];
    for i in 0..n { for j in i..n {
        let mi = histogram_mi(panels, slots[i], slots[j], 32)?;
        mat[i][j] = mi; mat[j][i] = mi;
    }}
    write_csv_with_mode_600(out_dir.join("pairwise_mi_matrix.csv"), &amp;slots, &amp;mat)?;
    let mean = mat.iter().flatten().sum::&lt;f32&gt;() / (n*n) as f32;
    let max = mat.iter().flatten().copied().fold(0.0, f32::max);
    Ok(PairwiseMiSummary { mean_normalized_mi: mean.clamp(0.0,1.0), max_normalized_mi: max.clamp(0.0,1.0), matrix_csv_path: ... })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/embedder_coherence.rs">compute_embedder_coherence + ContentEmbedder trait + jaccard + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/pairwise_mi.rs">pairwise_mi_audit + PairwiseMiSummary + CSV writer + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">re-export new modules</file>
</files_to_modify>

<validation_criteria>
  <criterion>Bootstrap fallback returns (0.5, "bootstrap_neutral_n_lt_1000") for corpus_size &lt; 1000</criterion>
  <criterion>14-embedder constraint validated</criterion>
  <criterion>Full-agreement NN sets → coherence 1.0; disjoint → 0.0</criterion>
  <criterion>pairwise_mi_matrix.csv is 15×15, symmetric, mode 0o600</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib embedder_coherence</command>
  <command>cargo test -p context-graph-mejepa-train --lib pairwise_mi</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
