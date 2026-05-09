# TASK-UTML-010 — FSV harness: 100 synthetic examples spanning corner cases (L_step envelope + target_collapse always 0 + sampling histogram)

```xml
<task_spec id="TASK-UTML-010" version="1.0">

<metadata>
  <title>FSV example binary utml_l_step_fsv: 100 synthetic examples (perfect prediction, total miss, high-drift, low-grad, collapsed latent, OOD, operator gold) writing utml-l-step-evidence.json + utml-classifier-evidence.json + utml-pairwise-mi-matrix.csv to /tmp/contextgraph-mejepa-train-fsv/ with mode 0o600 and re-reading + asserting envelope</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-UTML-01..12">All UTML primitives shipped end-to-end</item>
    <item ref="NFR-UTML-06">FSV evidence write protocol per doc 11 §11.1</item>
    <item ref="TC-UTML-13">100 synthetic examples spanning corner cases</item>
    <item ref="TC-UTML-05">5-regime classifier evidence</item>
    <item ref="TC-UTML-06">1-epoch dry-train sampling weight verification</item>
    <item ref="SEC-MEJEPA-07">FSV files mode 0o600</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-007</task_ref>
    <task_ref>TASK-UTML-008</task_ref>
    <task_ref>TASK-UTML-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
The FSV (Forensic Source Verification) harness exercises every UTML primitive end-to-end against 100 synthetic panels and 5 synthetic classifier regimes, writes evidence to `/tmp/contextgraph-mejepa-train-fsv/`, re-reads the evidence, and asserts: (a) `delta_xi_components.target_collapse == 0.0` for every example, (b) L_step matches expected envelope per category, (c) sampling-histogram shows high-L_step over-represented and operator_override force-included, (d) classifier identifies each of 5 regimes correctly, (e) all files mode 0o600.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; security_requirements SEC-MEJEPA-07)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (NFR-UTML-06; test_plan TC-UTML-05, TC-UTML-06, TC-UTML-13)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §11.1 (FSV protocol)</file>
  <file purpose="prior_tasks">TASK-UTML-007.md, TASK-UTML-008.md, TASK-UTML-009.md (consumed primitives)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-007..009 complete</check>
  <check>/tmp writable</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-train/examples/utml_l_step_fsv.rs` binary with `fn main()` that:
      1. Constructs 100 synthetic panels covering 7 corner-case categories (10×perfect_prediction, 30×high_quality, 50×average, 5×low_grad/SNR_below_floor, 3×collapsed_latent_synthetic, 2×OOD, ... + 10×operator_override gold)
      2. For each: synthesizes (predicted_latent, target_latent) tensors + SNR + exploration_bonus + ECE + embedder_coherence + plasticity + landscape + stability + agent_state + redundancy + violation
      3. Calls compute_delta_p/k/omega/xi/l_step/sampling_weight; collects results
      4. Asserts target_collapse == 0.0 for all 100; assertions on L_step envelope per category (perfect ≈ 0; high_quality ∈ [0.5, 0.8]; average ∈ [0.2, 0.4]; etc.)
      5. Builds 5 synthetic LStepWindow regimes (Stuck, OptimizerDysregulated, LatentCollapsing, Optimal, Dissipating) and runs `classify` on each; asserts state matches expected
      6. Constructs a sampling distribution: 100 examples × 1 epoch, partitions into drop/regular/force using sampling_weight; asserts force-include count == 10 (operator_override examples), drop count == 10 (perfect_prediction examples), regular count == 80
      7. Runs pairwise_mi_audit on a synthetic 50-panel batch; asserts CSV file exists with shape 15×15 and mode 0o600
      8. Writes:
         - `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` (100 examples + envelope assertions PASSED)
         - `/tmp/contextgraph-mejepa-train-fsv/utml-classifier-evidence.json` (5 regimes + verdicts)
         - `/tmp/contextgraph-mejepa-train-fsv/utml-sampling-histogram.json` (drop/regular/force counts)
         - `/tmp/contextgraph-mejepa-train-fsv/pairwise_mi_matrix.csv` (15×15 from pairwise_mi_audit)
      9. All files written with mode 0o600 (`std::os::unix::fs::PermissionsExt`)
      10. Re-reads each file, parses JSON/CSV, validates round-trip equality + assertions
      11. Prints SHA-256 of each file + total examples + total assertions passed; exits 0 on success, 1 on any assertion failure
    - Use `serde_json` for JSON; `csv` crate optional for the matrix file (or hand-written)
    - Deterministic synthetic-data generation: seed all RNG with a known constant (e.g., 0xDEADBEEFCAFEBABE)
    - Document each corner-case category in a top-of-file comment block (purpose, expected envelope)
  </in_scope>
  <out_of_scope>
    - Real predictor / aux head forward passes (Phase 2 dep)
    - Real corpus loading (Phase 0 dep)
    - Trainer integration (TASK-TRAIN-014)
    - Cert chain hash verification (TASK-TRAIN-009 / TASK-TRAIN-014)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/examples/utml_l_step_fsv.rs">
//! FSV: 100 synthetic examples spanning UTML corner cases.
//! Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room).
//!
//! Categories (100 total):
//!   10× perfect_prediction (ΔP_real==0 → L_step==0; expected drop)
//!   30× high_quality (ΔP ∈ [0.5,0.8]; healthy substrate)
//!   50× average (ΔP ∈ [0.2,0.4])
//!   5× low_grad_or_snr_floor (SNR below 0.05 → fail-closed UTML_SNR_BELOW_FLOOR)
//!   3× collapsed_latent_synthetic (predictor_redundancy &gt; 0.5)
//!   2× ood (constellation_violation_rate &gt; 0.5)
//!   10× operator_override gold (sampling_weight &gt; 1.0 → force-include)

use context_graph_mejepa_train::{compute_delta_p, compute_delta_k, compute_delta_omega, compute_delta_xi, compute_l_step, sampling_weight, ...};

const FSV_DIR: &amp;str = "/tmp/contextgraph-mejepa-train-fsv";
const FSV_SEED: u64 = 0xDEADBEEFCAFEBABE;

fn main() -&gt; std::process::ExitCode { ... }
fn write_with_mode_600(path: &amp;Path, content: &amp;[u8]) -&gt; std::io::Result&lt;()&gt; { ... }
    </signature>
  </signatures>
  <constraints>
    - All FSV files have mode 0o600 verified after write (re-stat the file; assert mode bits)
    - All JSON outputs are round-trip-stable (sorted keys; deterministic float formatting)
    - Asserts target_collapse == 0.0 for all 100 examples
    - Asserts force-include count == count of operator_override-true examples
    - Asserts drop count == count of perfect-prediction examples
    - Classifier asserts: 5 regimes → 5 expected ClassifierVerdict.state values
    - Print summary table: per-category mean L_step, count
    - Exit 0 on all assertions pass; exit 1 with structured stderr message on first failure
    - File &lt; 500 lines
    - Provenance comment per `<provenance>`
  </constraints>
  <verification>
    - `cargo run --release -p context-graph-mejepa-train --example utml_l_step_fsv` exits 0
    - Files exist at the listed paths with mode 0o600 (verify with `ls -l`)
    - JSON files parse + round-trip
    - CSV file is 15×15 with InstrumentSlot::slug() row/column labels
    - Re-running with same seed produces byte-equal evidence
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    let dir = Path::new(FSV_DIR);
    fs::create_dir_all(dir).unwrap();
    let mut rng = SplitMix64::seeded(FSV_SEED);
    let mut evidence = Vec::with_capacity(100);
    let mut force_count = 0; let mut drop_count = 0;
    for category in &amp;[Perfect(10), HighQuality(30), Average(50), LowSnr(5), Collapsed(3), Ood(2), Override(10)] {
        for _ in 0..category.count {
            let (pred, tgt) = synth_panels(&amp;mut rng, category);
            let dp = compute_delta_p(&amp;pred, &amp;tgt, snr_for(category), explore_for(category), 0.7, None, 0.05);
            // ... compute dk, do, dx, l, w
            // assert target_collapse==0
            evidence.push(ExampleEvidence { ... });
            if w &gt; 1.0 { force_count += 1; }
            if w &lt; 0.05 { drop_count += 1; }
        }
    }
    write_with_mode_600(dir.join("utml-l-step-evidence.json"), &amp;serde_json::to_vec_pretty(&amp;evidence)?);
    // 5 classifier regimes
    let regimes = synth_regimes();
    let mut verdicts = vec![];
    for r in regimes { verdicts.push(classify(&amp;r.window, r.gain_ratio, 100, 0.7)?); }
    write_with_mode_600(dir.join("utml-classifier-evidence.json"), ...);
    // sampling histogram
    write_with_mode_600(dir.join("utml-sampling-histogram.json"),
        json!({"drop": drop_count, "force": force_count, "regular": 100 - drop_count - force_count}));
    // pairwise MI
    pairwise_mi_audit(&amp;synth_panels_50(&amp;mut rng), dir)?;
    // re-read and assert
    let reread: Vec&lt;ExampleEvidence&gt; = serde_json::from_slice(&amp;fs::read(...)?)?;
    assert_eq!(reread, evidence);
    for ev in reread { assert_eq!(ev.delta_xi_components.target_collapse, 0.0); }
    println!("FSV PASS: 100 examples, target_collapse always 0, force={force_count}, drop={drop_count}");
    ExitCode::from(0)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/examples/utml_l_step_fsv.rs">FSV harness binary</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add `[[example]] name = "utml_l_step_fsv"` if needed</file>
</files_to_modify>

<validation_criteria>
  <criterion>Exit code 0 on success</criterion>
  <criterion>4 evidence files exist with mode 0o600</criterion>
  <criterion>target_collapse == 0.0 asserted for all 100 examples</criterion>
  <criterion>force_count == 10 (override examples)</criterion>
  <criterion>drop_count == 10 (perfect prediction examples)</criterion>
  <criterion>5 classifier regimes correctly identified</criterion>
  <criterion>pairwise_mi_matrix.csv 15×15 with correct row/column labels</criterion>
  <criterion>Re-running with same seed produces byte-equal evidence</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train --example utml_l_step_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-train --example utml_l_step_fsv</command>
  <command>ls -l /tmp/contextgraph-mejepa-train-fsv/</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json</command>
</test_commands>

</task_spec>
```
