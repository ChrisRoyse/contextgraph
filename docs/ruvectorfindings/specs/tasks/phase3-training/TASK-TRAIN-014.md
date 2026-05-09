# TASK-TRAIN-014 — FSV: training-cert-chain.json + utml-l-step-evidence.json (cross-phase) + holdout-eval-evidence.json + cert-chain replay verification

```xml
<task_spec id="TASK-TRAIN-014" version="1.0">

<metadata>
  <title>Forensic Source Verification harness for the Phase 3 trainer: 100-step smoke run, 100,000-step synthetic chain replay, holdout eval round-trip; writes 4 evidence files mode 0o600 to /tmp/contextgraph-mejepa-train-fsv/ and re-reads + asserts</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-TRAIN-01..12">end-to-end FSV per &lt;fsv_protocol&gt;</item>
    <item ref="NFR-TRAIN-02">100,000-step replay &lt; 60s</item>
    <item ref="NFR-TRAIN-08">SEC-MEJEPA-07 mode 0o600 on FSV files</item>
    <item ref="constitution.fsv_evidence_paths">/tmp/contextgraph-mejepa-train-fsv/{training-cert-chain.json, utml-l-step-evidence.json, holdout-eval-evidence.json, training-smoke-evidence.json}</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-012</task_ref>
    <task_ref>TASK-TRAIN-013</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>4 created</estimated_files>
</metadata>

<context>
The Phase 3 FSV harness is the integration acceptance test. It exercises the full Trainer with stub predictor + stub frozen target + stub aux + stub oracle head; runs 100 steps; emits 100 certs; replays the chain; runs a synthetic holdout evaluation; and writes structured evidence to /tmp/contextgraph-mejepa-train-fsv/ which are then re-read and validated. Separately, a 100,000-step synthetic chain stress test verifies the replay performance NFR. UTML L_step evidence is produced by the cross-phase TASK-UTML-010 (FSV harness in utml-learning-formula); this task only ensures the file is present + readable.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (fsv_protocol; testing_requirements 100k-replay; security_requirements SEC-MEJEPA-07)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (NFR-TRAIN-01..08; test_plan)</file>
  <file purpose="prior_tasks">TASK-TRAIN-012 (Trainer + Checkpointer), TASK-TRAIN-013 (CLI), ../utml-learning-formula/TASK-UTML-010 (UTML FSV)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-012 + TASK-TRAIN-013 + UTML-010 complete</check>
  <check>/tmp writable</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-train/examples/train_smoke_fsv.rs`:
      - Stub predictor + stub frozen target + stub aux + stub oracle head + stub embedder coherence
      - 100-step training run on 200 synthetic examples (10 force-includes, 10 perfect-prediction drops, 180 regular)
      - Asserts: 100 certs persisted to CF_MEJEPA_TRAIN_CERTS; chain integrity verified; target_collapse==0 for all; per-cert delta_p/k/omega/xi all in [0,1]; cert.training_mode populated
      - Writes `training-smoke-evidence.json` with per-step cert summary + chain verification report
    - Create `crates/context-graph-mejepa-train/examples/train_cert_chain_fsv.rs`:
      - Generates 100,000 synthetic certs (no actual training; just direct cert build + emit)
      - Writes to a tmpfs RocksDB
      - Calls verify_chain(rocksdb, "CF_MEJEPA_TRAIN_CERTS", 0, 99_999); asserts replay completes &lt; 60s and verified == 100,000
      - Writes `training-cert-chain.json` with timing + verification summary
    - Create `crates/context-graph-mejepa-train/examples/train_holdout_eval_fsv.rs`:
      - Synthetic 240-example holdout dataset spanning 8 mutation categories × 11 languages (uneven distribution okay)
      - Stub predictor + stub oracle head; runs HoldoutEvaluator::evaluate
      - Asserts: agreement computed; per_category map has ≤ 8 keys; per_language map has ≤ 11 keys; generic_only_warning fires on synthetic skewed dataset; promote_best path produces best.safetensors atomically (using a stub safetensors blob)
      - Writes `holdout-eval-evidence.json`
    - Create `crates/context-graph-mejepa-train/examples/train_full_fsv.rs` (top-level integration):
      - Runs all 3 sub-FSVs in sequence
      - Verifies presence of `utml-l-step-evidence.json` (produced by UTML-010); reads + sanity-checks `target_collapse: 0.0` for all entries
      - Aggregates a top-level `training-fsv-summary.json` (optional)
      - Exit 0 on all-pass; exit 1 with stderr JSON on any failure
    - All output files mode 0o600 (re-stat verify); use `std::os::unix::fs::PermissionsExt`
    - Deterministic seed (e.g., 0xCAFEBABEDEADBEEF) for reproducibility
    - Re-reading: parse JSON; validate round-trip equality; assert specific invariants
    - Total walltime budget: smoke &lt; 10s; chain stress &lt; 90s (60s replay + 30s setup); holdout &lt; 5s
  </in_scope>
  <out_of_scope>
    - Real predictor / aux / oracle (Phase 2 dep)
    - Real corpus loading (Phase 0)
    - Real ed25519 keyring (operator-side; FSV uses ephemeral SigningKey::generate)
    - CI integration (operator-managed)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/examples/train_smoke_fsv.rs">
//! 100-step trainer smoke FSV.
//! Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room).
const FSV_DIR: &amp;str = "/tmp/contextgraph-mejepa-train-fsv";
const SEED: u64 = 0xCAFEBABEDEADBEEF;

fn main() -&gt; std::process::ExitCode { ... }
    </signature>
    <signature file="crates/context-graph-mejepa-train/examples/train_cert_chain_fsv.rs">
//! 100,000-step synthetic cert chain replay FSV (NFR-TRAIN-02).
fn main() -&gt; std::process::ExitCode { ... }
    </signature>
    <signature file="crates/context-graph-mejepa-train/examples/train_holdout_eval_fsv.rs">
//! 240-example holdout eval FSV with synthetic per-category / per-language distribution.
fn main() -&gt; std::process::ExitCode { ... }
    </signature>
    <signature file="crates/context-graph-mejepa-train/examples/train_full_fsv.rs">
//! Top-level Phase 3 FSV: smoke + chain + holdout + UTML cross-check.
fn main() -&gt; std::process::ExitCode { ... }
    </signature>
  </signatures>
  <constraints>
    - All FSV files mode 0o600 verified after write
    - Deterministic seed; re-running produces byte-equal evidence
    - 100k-step chain replay &lt; 60s (assert in code)
    - target_collapse == 0.0 invariant scan over ALL persisted certs (smoke + chain stress)
    - utml-l-step-evidence.json existence verified (TASK-UTML-010 must have run first); training_full_fsv reads it but does not regenerate
    - All examples are `#[tokio::main]` if needed (or sync); use `tempfile` crate for tmp RocksDB
    - Files &lt; 500 lines each
    - Provenance comment per `<provenance>`
  </constraints>
  <verification>
    - `cargo run --release -p context-graph-mejepa-train --example train_smoke_fsv` exits 0
    - `cargo run --release -p context-graph-mejepa-train --example train_cert_chain_fsv` exits 0
    - `cargo run --release -p context-graph-mejepa-train --example train_holdout_eval_fsv` exits 0
    - `cargo run --release -p context-graph-mejepa-train --example train_full_fsv` exits 0
    - 4 files exist at /tmp/contextgraph-mejepa-train-fsv/ with mode 0o600
    - JSON files parse and round-trip
    - Re-running produces byte-equal evidence (deterministic)
  </verification>
</definition_of_done>

<pseudo_code>
fn main() -&gt; ExitCode {
    let dir = Path::new(FSV_DIR);
    fs::create_dir_all(dir).unwrap();
    let rocksdb = open_tmp_rocksdb()?;
    let mut trainer = build_stub_trainer(rocksdb.clone(), SEED)?;
    let dataset = build_synthetic_dataset(200);
    for step in 0..100 {
        trainer.train_one_step(&amp;dataset, step)?;
    }
    let report = verify_chain(&amp;rocksdb, "CF_MEJEPA_TRAIN_CERTS", 0, 99)?;
    let invariant_violations = scan_target_collapse_invariant(&amp;rocksdb, "CF_MEJEPA_TRAIN_CERTS", 0, 99)?;
    assert_eq!(invariant_violations.len(), 0, "target_collapse non-zero observed");
    let evidence = json!({
        "steps": 100,
        "chain_verification": report,
        "target_collapse_violations": invariant_violations,
        "all_in_range": true,
    });
    write_with_mode_600(dir.join("training-smoke-evidence.json"), serde_json::to_vec_pretty(&amp;evidence)?)?;
    let reread: serde_json::Value = serde_json::from_slice(&amp;fs::read(dir.join("training-smoke-evidence.json"))?)?;
    assert_eq!(reread, evidence);
    println!("smoke FSV PASS: 100 steps, chain verified, target_collapse=0 always");
    ExitCode::from(0)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/examples/train_smoke_fsv.rs">100-step smoke FSV</file>
  <file path="crates/context-graph-mejepa-train/examples/train_cert_chain_fsv.rs">100,000-step chain replay stress FSV</file>
  <file path="crates/context-graph-mejepa-train/examples/train_holdout_eval_fsv.rs">240-example holdout eval FSV</file>
  <file path="crates/context-graph-mejepa-train/examples/train_full_fsv.rs">orchestrator FSV</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add tempfile dev-dep; declare 4 [[example]] entries</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 4 example binaries build + run + exit 0</criterion>
  <criterion>4 evidence files exist at /tmp/contextgraph-mejepa-train-fsv/ with mode 0o600</criterion>
  <criterion>100k-step chain replay &lt; 60s</criterion>
  <criterion>target_collapse == 0.0 verified over every persisted cert (smoke + stress)</criterion>
  <criterion>utml-l-step-evidence.json (from UTML-010) exists and is referenced by train_full_fsv</criterion>
  <criterion>Re-running with same seed produces byte-equal evidence</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train --examples</command>
  <command>cargo run --release -p context-graph-mejepa-train --example train_smoke_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-train --example train_cert_chain_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-train --example train_holdout_eval_fsv</command>
  <command>cargo run --release -p context-graph-mejepa-train --example train_full_fsv</command>
  <command>ls -l /tmp/contextgraph-mejepa-train-fsv/</command>
  <command>stat -c '%a' /tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json</command>
</test_commands>

</task_spec>
```
