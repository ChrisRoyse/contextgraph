# TASK-INSTR-021 — Per-instrument linear-probe quality gate (≥ 70% top-1 mutation-category accuracy)

```xml
<task_spec id="TASK-INSTR-021" version="1.0">

<metadata>
  <title>Per-instrument linear-probe DoD — ≥ 70% top-1 mutation-category accuracy on 100-example holdout</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>21</sequence>
  <implements>
    <item ref="REQ-INSTR-21">Linear-probe DoD (≥ 70% top-1)</item>
    <item ref="NFR-INSTR-09">Per-instrument linear-probe ≥ 70% top-1 on 100-example holdout</item>
    <item ref="AC-INSTR-06-01">Top-1 ≥ 70% on Phase 1 DoD round-trip</item>
    <item ref="TC-INSTR-15">Linear probe on E_Diff achieves ≥ 70%</item>
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
    <task_ref>TASK-CORPUS-023</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file created (probe harness) + 15 example binaries (one per instrument)</estimated_files>
</metadata>

<context>
Per REQ-INSTR-21 + NFR-INSTR-09 + AC-INSTR-06-01, the Phase 1 Definition-of-Done for any instrument
is: train a single linear classifier on its output as features and predict the mutation category
(8 classes per Phase 0 corpus). Holdout split ≥ 20%. Top-1 accuracy ≥ 70% on the holdout.

This task ships:

1. A reusable probe harness module `linear_probe.rs` with a deterministic SGD trainer (no
   external candle deps; pure Rust f32 matrix multiply for portability + determinism).
2. 15 example binaries (one per slot — including Scalars), each loads 100 mutation-corpus
   examples spanning all 8 categories, encodes via the corresponding instrument, trains a linear
   probe, evaluates on holdout, and writes evidence to
   `/tmp/contextgraph-mejepa-instruments-fsv/linear_probe_e_*.json` with confusion matrix + accuracy.
3. A pass/fail gate per instrument: assert top-1 ≥ 70%; failure raises with full evidence path.

When an instrument is below 70%, surface the failure as evidence the instrument is producing
noise rather than signal — DO NOT silently pad accuracy.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-21, NFR-INSTR-09, AC-INSTR-06-01, TC-INSTR-15)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (test_strategy round_trip)</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §7 Phase 1 DoD</file>
  <file purpose="corpus">crates/context-graph-mejepa-corpus/src/lib.rs (mutation corpus generator + 8 categories)</file>
  <file purpose="instruments">crates/context-graph-mejepa-instruments/src/{e_oracle,e_witness,e_static_analysis,e_code,e_text,e_trace,e_runtime,e_reasoning,e_scalars}.rs</file>
</input_context_files>

<prerequisites>
  <check>All instruments shipped (TASK-INSTR-005..019)</check>
  <check>Phase 0 corpus generator shipped (TASK-CORPUS-023) AND a 100-example dev corpus available at /tmp/contextgraph-mejepa-corpus-fsv/dev100/ OR generated on demand</check>
</prerequisites>

<scope>
  <in_scope>
    - New module `crates/context-graph-mejepa-instruments/src/linear_probe.rs`
    - `pub struct LinearProbe { weights: Vec&lt;Vec&lt;f32&gt;&gt;, biases: Vec&lt;f32&gt; }` (8 classes)
    - `pub fn train_linear_probe(features: &amp;[Vec&lt;f32&gt;], labels: &amp;[u8], holdout_frac: f32, seed: u64) -&gt; ProbeReport`
    - `pub struct ProbeReport { top_1_acc: f32, confusion_matrix: [[u32; 8]; 8], holdout_size: usize, train_size: usize, evidence_path: PathBuf }`
    - Deterministic seeded SGD: SplitMix64 for shuffle; learning rate decay; max 200 epochs
    - 15 example binaries: `examples/linear_probe_e_ast.rs`, ..., `examples/linear_probe_scalars.rs`
    - Each binary writes evidence to `/tmp/contextgraph-mejepa-instruments-fsv/linear_probe_e_x.json` with mode 0o600 (per SEC-MEJEPA-07)
    - Pass/fail assertion: top-1 ≥ 70%; failure prints evidence path + accuracy
    - Aggregate report: `examples/linear_probe_all.rs` writes `/tmp/contextgraph-mejepa-instruments-fsv/linear_probe_summary.json` with per-instrument accuracy
    - cargo run binaries build cleanly in --release
  </in_scope>
  <out_of_scope>
    - Training instruments themselves (frozen)
    - Generating the corpus (Phase 0)
    - Evaluation gate enforcement during PR merge (operator's job)
    - Linear probe on the FULL panel (deferred — that's the Phase 2 predictor's job)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/linear_probe.rs">
pub const LINEAR_PROBE_MIN_ACCURACY: f32 = 0.70;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub instrument_slot: InstrumentSlot,
    pub top_1_accuracy: f32,
    pub confusion_matrix: Vec&lt;Vec&lt;u32&gt;&gt;, // 8x8
    pub train_size: usize,
    pub holdout_size: usize,
    pub seed: u64,
    pub epochs_trained: u32,
    pub final_loss: f32,
}

pub fn train_linear_probe(
    features: &amp;[Vec&lt;f32&gt;],
    labels: &amp;[u8],
    holdout_frac: f32,
    seed: u64,
    slot: InstrumentSlot,
) -&gt; InstrumentResult&lt;ProbeReport&gt;;

pub fn write_probe_evidence(
    report: &amp;ProbeReport,
    fsv_dir: &amp;Path,
) -&gt; InstrumentResult&lt;PathBuf&gt;;

pub fn assert_probe_passes(report: &amp;ProbeReport) -&gt; InstrumentResult&lt;()&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-instruments/examples/linear_probe_e_ast.rs">
fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt; {
    // 1. load 100 corpus examples
    // 2. encode via EAstInstrument
    // 3. train_linear_probe(features, labels, 0.20, 42, EAst)
    // 4. write_probe_evidence to /tmp/contextgraph-mejepa-instruments-fsv/
    // 5. assert_probe_passes (panic with evidence path on &lt; 70%)
}
    </signature>
  </signatures>
  <constraints>
    - Pure-Rust deterministic SGD (no candle dep — for portability + determinism)
    - SplitMix64 seeded shuffling (already in workspace)
    - 100 examples spanning all 8 mutation categories (corpus pre-built)
    - holdout_frac default 0.20 (20 examples holdout, 80 train)
    - Threshold: top_1 ≥ 0.70; fail with InstrumentError + evidence path
    - FSV files mode 0o600 (SEC-MEJEPA-07)
    - Determinism: same seed + same features + same labels -&gt; byte-equal ProbeReport
    - 15 example binaries (one per slot, including Scalars)
    - File-by-file under 500 lines
  </constraints>
  <verification>
    - cargo run --release -p context-graph-mejepa-instruments --example linear_probe_e_ast (and 14 others)
    - cargo run --release -p context-graph-mejepa-instruments --example linear_probe_all (aggregate)
    - Evidence files at /tmp/contextgraph-mejepa-instruments-fsv/linear_probe_e_*.json
    - Each instrument's top_1 ≥ 0.70 (failure flags noise vs signal)
  </verification>
</definition_of_done>

<pseudo_code>
linear_probe.rs:

  pub fn train_linear_probe(features, labels, holdout_frac, seed, slot) -&gt; InstrumentResult&lt;ProbeReport&gt; {
      // Validate: features.len() == labels.len(); ≥ 50 examples; 8 classes covered
      let n = features.len();
      let mut indices: Vec&lt;usize&gt; = (0..n).collect();
      shuffle_split_mix_64(&amp;mut indices, seed);
      let holdout_n = ((n as f32) * holdout_frac).max(20.0) as usize;
      let (holdout, train) = indices.split_at(holdout_n);
      let d = features[0].len();
      let mut weights = vec![vec![0.0_f32; d]; 8];
      let mut biases = vec![0.0_f32; 8];
      for epoch in 0..200 {
          let lr = 0.1 / (1.0 + (epoch as f32) * 0.01);
          for &amp;i in train {
              let logits = forward(&amp;weights, &amp;biases, &amp;features[i]);
              let probs = softmax(&amp;logits);
              let true_class = labels[i] as usize;
              for c in 0..8 {
                  let dz = probs[c] - if c == true_class { 1.0 } else { 0.0 };
                  for k in 0..d { weights[c][k] -= lr * dz * features[i][k]; }
                  biases[c] -= lr * dz;
              }
          }
      }
      // Evaluate on holdout
      let mut confusion = vec![vec![0_u32; 8]; 8];
      let mut correct = 0;
      for &amp;i in holdout {
          let pred = argmax(&amp;forward(&amp;weights, &amp;biases, &amp;features[i]));
          confusion[labels[i] as usize][pred] += 1;
          if pred == labels[i] as usize { correct += 1; }
      }
      let top_1 = correct as f32 / holdout.len() as f32;
      Ok(ProbeReport { instrument_slot: slot, top_1_accuracy: top_1, confusion_matrix: confusion, ... })
  }

  pub fn write_probe_evidence(report, fsv_dir) -&gt; InstrumentResult&lt;PathBuf&gt; {
      let path = fsv_dir.join(format!("linear_probe_{}.json", report.instrument_slot.slug()));
      let mut opts = std::fs::OpenOptions::new();
      opts.create(true).write(true).truncate(true);
      #[cfg(unix)] opts.mode(0o600);
      let file = opts.open(&amp;path)?;
      serde_json::to_writer_pretty(file, report)?;
      Ok(path)
  }

  pub fn assert_probe_passes(report) -&gt; InstrumentResult&lt;()&gt; {
      if report.top_1_accuracy &lt; LINEAR_PROBE_MIN_ACCURACY {
          return Err(InstrumentError::invalid("probe.top_1_accuracy",
              format!("instrument {:?} top_1={:.3} &lt; threshold {:.3}", report.instrument_slot, report.top_1_accuracy, LINEAR_PROBE_MIN_ACCURACY),
              "review instrument signal vs noise; do not pad accuracy"));
      }
      Ok(())
  }

examples/linear_probe_e_ast.rs:
  fn main() -&gt; Result&lt;(), Box&lt;dyn std::error::Error&gt;&gt; {
      let corpus = load_dev_corpus_100()?;       // 100 (mutated_source, mutation_category) pairs
      let chunker = AstChunkerFacade::new();
      let instrument = EAstInstrument::new_with_weights(...)?;  // OR deterministic shipped fallback
      let mut features = vec![];
      let mut labels = vec![];
      for entry in corpus {
          let chunks = chunker.chunk(entry.source.as_bytes(), entry.language)?;
          let v = instrument.encode(&amp;CodeInstrumentInput { language: entry.language, chunks })?;
          features.push(v);
          labels.push(entry.mutation_category as u8);
      }
      let report = train_linear_probe(&amp;features, &amp;labels, 0.20, 42, InstrumentSlot::EAst)?;
      let path = write_probe_evidence(&amp;report, Path::new("/tmp/contextgraph-mejepa-instruments-fsv"))?;
      println!("evidence: {}", path.display());
      assert_probe_passes(&amp;report)?;
      Ok(())
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-instruments/src/linear_probe.rs">LinearProbe + train_linear_probe + write_probe_evidence + assert_probe_passes; SplitMix64 SGD; tests</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_ast.rs">Probe binary for E_AST</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_cfg.rs">Probe binary for E_CFG</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_data_flow.rs">Probe binary for E_DataFlow</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_type_graph.rs">Probe binary for E_TypeGraph</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_test.rs">Probe binary for E_Test</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_trace.rs">Probe binary for E_Trace</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_diff.rs">Probe binary for E_Diff</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_witness.rs">Probe binary for E_Witness</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_oracle.rs">Probe binary for E_Oracle</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_problem.rs">Probe binary for E_Problem</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_commit_msg.rs">Probe binary for E_CommitMsg</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_static_analysis.rs">Probe binary for E_StaticAnalysis</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_runtime.rs">Probe binary for E_Runtime</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_e_reasoning.rs">Probe binary for E_Reasoning</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_scalars.rs">Probe binary for Scalars</file>
  <file path="crates/context-graph-mejepa-instruments/examples/linear_probe_all.rs">Aggregate report runner</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub mod linear_probe;` re-export {LinearProbe, ProbeReport, train_linear_probe, write_probe_evidence, assert_probe_passes, LINEAR_PROBE_MIN_ACCURACY}</file>
  <file path="crates/context-graph-mejepa-instruments/Cargo.toml">Add example bins under [[example]]</file>
</files_to_modify>

<validation_criteria>
  <criterion>15 probe binaries build in cargo build --release --examples</criterion>
  <criterion>Each probe writes evidence to /tmp/contextgraph-mejepa-instruments-fsv/linear_probe_e_*.json</criterion>
  <criterion>Evidence files have mode 0o600</criterion>
  <criterion>Each instrument top_1 ≥ 0.70 OR failure surfaces evidence path + accuracy (no silent padding)</criterion>
  <criterion>Probe is deterministic (same seed + same data -&gt; byte-equal report)</criterion>
  <criterion>linear_probe_all.rs aggregates all 15 reports into linear_probe_summary.json</criterion>
  <criterion>cargo build --release --examples clean</criterion>
  <criterion>cargo clippy -- -D warnings clean</criterion>
  <criterion>file under 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments --examples</command>
  <command>cargo test -p context-graph-mejepa-instruments linear_probe::</command>
  <command>cargo run --release -p context-graph-mejepa-instruments --example linear_probe_e_oracle</command>
  <command>cargo run --release -p context-graph-mejepa-instruments --example linear_probe_all</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
