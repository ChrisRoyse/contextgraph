# TASK-INFER-004 — MeJepaCompiler::verify (7-gate ladder + short-circuit)

```xml
<task_spec id="TASK-INFER-004" version="1.0">

<metadata>
  <title>MeJepaCompiler::verify() seven-gate ladder in `crates/context-graph-mejepa/src/compiler.rs` + `gates.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-INFER-01">Public verify signature</item>
    <item ref="REQ-INFER-03">VerifyVerdict {Approve, EscalateToHuman} composition</item>
    <item ref="REQ-INFER-04">FailedGate 7-variant population</item>
    <item ref="REQ-INFER-09">Gate 1 OOD short-circuit + OodRefuse → EscalateToHuman conversion</item>
    <item ref="REQ-INFER-10">Gate 2 PredictedFailure</item>
    <item ref="REQ-INFER-11">Gate 3 LowConfidence (entropy + outcome_set.len() > outcome_set_max)</item>
    <item ref="REQ-INFER-12">Gate 4 GrangerRejection (any p_value > p_threshold)</item>
    <item ref="REQ-INFER-13">Gate 5 WitnessChainBroken</item>
    <item ref="REQ-INFER-14">Gate 6 SourceShaDrift</item>
    <item ref="REQ-INFER-15">Gate 7 ConstellationGuardRejected (mock until TCT ships)</item>
    <item ref="REQ-INFER-19">Persist RealityPrediction to CF_MEJEPA_LIVE_PREDICTIONS on Approve</item>
    <item ref="component:MeJepaCompiler.verify">behavior steps 1-4 in TECH-INFERENCE</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-003</task_ref>
    <task_ref>TASK-INFER-009</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>1 created (gates.rs); compiler.rs modified to add verify</estimated_files>
</metadata>

<context>
This task lands the seven-gate ladder. Order of evaluation per TECH-INFERENCE: Gate 6
(SourceShaDrift) and Gate 5 (WitnessChainBroken) run BEFORE compile() because they are cheapest and
most likely-to-fail-on-bogus-input. After compile(), Gates 1-4 + 7 evaluate the resulting
RealityPrediction. First failing gate short-circuits — remaining gates NOT evaluated (BR-INFER-07).
verify() never propagates OodRefuse as Err — it converts to EscalateToHuman { OutOfDistribution }.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-09..15; BR-INFER-07; AC-INFER-01-01/02; AC-INFER-03-02; AC-INFER-05-01)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts MeJepaCompiler::verify behavior; verify_sequence_diagram; gate_ordering)</file>
  <file purpose="upstream_compile">crates/context-graph-mejepa/src/compiler.rs (TASK-INFER-003)</file>
  <file purpose="upstream_gates">crates/context-graph-mejepa/src/gates.rs (TASK-INFER-009 — SourceShaDrift + WitnessChainBroken helpers)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/types.rs (TASK-INFER-001 — VerifyVerdict, FailedGate)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-003 complete; compile() returns RealityPrediction</check>
  <check>TASK-INFER-009 complete; check_source_sha_drift + replay_witness_segment exist</check>
  <check>ConstellationGuard trait defined in compiler.rs (TASK-INFER-003)</check>
</prerequisites>

<scope>
  <in_scope>
    - Add `pub fn verify(&self, patch: &PatchBundle, context: &TaskContext) -> Result<VerifyVerdict, MejepaInferError>` to `MeJepaCompiler` impl in `compiler.rs`
    - Implement gate ladder per TECH-INFERENCE behavior:
      - **Gate 6 SourceShaDrift** first (cheapest IO): call `check_source_sha_drift(patch)` from gates.rs
      - **Gate 5 WitnessChainBroken**: call `replay_witness_segment(&patch.witness_chain_segment)` from gates.rs
      - Now call `self.compile(patch, context)`; catch `OodRefuse { score, .. }` and convert to `EscalateToHuman { failed_gate: OutOfDistribution { score } }` with synthesized RealityPrediction (or with empty placeholder when compile failed before producing one — per AC-INFER-01-03 partial materialization is logged)
      - **Gate 1 OOD**: re-check `reality_prediction.out_of_distribution_score > config.ood_refuse_threshold` (defense in depth — compile may have allowed score == 0.5 boundary)
      - **Gate 2 PredictedFailure**: `reality_prediction.predicted_oracle_pass <= 0.95` → EscalateToHuman { PredictedFailure { failing_tests } }
      - **Gate 3 LowConfidence**: `confidence.outcome_set.len() > config.outcome_set_max` → EscalateToHuman { LowConfidence { entropy: shannon_entropy_of_outcome_set } }
      - **Gate 4 GrangerRejection**: any `granger_attestations[skill] > config.p_threshold` → EscalateToHuman { GrangerRejection { skill, p_value } }; pick FIRST violating skill (canonical iteration order via BTreeMap)
      - **Gate 7 ConstellationGuardRejected**: `constellation.gtau_test(&panel_pred_t2)`; on Reject return EscalateToHuman { ConstellationGuardRejected { violating_embedders, violating_chunks } }; on stale return Err(GtauStaleConstellation) (NOT EscalateToHuman per BR-INFER-05)
    - On all 7 gates pass: persist `reality_prediction` to CF_MEJEPA_LIVE_PREDICTIONS via `storage.write_live_prediction((session_id, witness_chain_offset), &prediction)`; return `Approve { reality_prediction, gates_passed: 7 }`
    - Track `gates_passed: u8` for partial-pass diagnostic (used in EscalateToHuman responses for human reviewer; actually 0..7 inclusive)
    - Verify-internal helper `verify_after_compile(prediction, panel_pred_t2) -> VerifyVerdict` so MockedConstellationGuard tests can short-circuit Gate 7 deterministically
    - The compile() call needs panel_pred_t2 for Gate 7. Refactor compile to optionally return `(RealityPrediction, Panel)` via a private `compile_with_panel` (keep the public `compile` returning only RealityPrediction)
    - Add unit tests with mocks: (a) all 7 gates pass → Approve with gates_passed==7; (b) SHA mismatch on a touched_path → SourceShaDrift; (c) corrupted witness segment → WitnessChainBroken; (d) OOD panel → OutOfDistribution (not OodRefuse Err); (e) predicted_oracle_pass=0.94 → PredictedFailure; (f) outcome_set.len()=3 → LowConfidence; (g) Granger p=0.03 with p_threshold=0.01 → GrangerRejection; (h) MockedConstellationGuard rejecting → ConstellationGuardRejected; (i) stale constellation → Err GtauStaleConstellation (not EscalateToHuman); (j) gate ordering test: simultaneous SHA drift + OOD fires SHA drift first (Gate 6 before Gate 1)
  </in_scope>
  <out_of_scope>
    - Real TCT Gτ test (cross-cutting; lives in `crates/context-graph-mejepa-tct/`; this task uses MockedAlwaysApprove + MockedAlwaysReject for tests)
    - MCP tool wiring (TASK-INFER-011)
    - CF_MEJEPA_LIVE_PREDICTIONS column-family registration (handled by Phase 6 SPEC-HYGIENE; this task only calls storage.write_live_prediction)
    - Session_id derivation (passed as input via context.task_id.0[..16]; or via storage.session_id() if added later)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/compiler.rs">
impl MeJepaCompiler {
    pub fn verify(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext)
        -&gt; Result&lt;VerifyVerdict, MejepaInferError&gt;;

    fn compile_with_panel(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext)
        -&gt; Result&lt;(RealityPrediction, Panel), MejepaInferError&gt;;

    fn convert_ood_refuse_to_verdict(&amp;self, score: f32, panel_sha: [u8; 32])
        -&gt; VerifyVerdict;
}
    </signature>
  </signatures>
  <constraints>
    - Gate evaluation order EXACTLY: Gate 6 (SHA), Gate 5 (Witness), compile(), Gate 1 (OOD), Gate 2 (PredFail), Gate 3 (LowConf), Gate 4 (Granger), Gate 7 (TCT). First gate to fire short-circuits per BR-INFER-07.
    - `OodRefuse` from compile MUST be converted to `EscalateToHuman { OutOfDistribution }`; verify never returns OodRefuse as Err
    - `GtauStaleConstellation` (Err) propagates per BR-INFER-05 — does NOT silently bypass Gate 7
    - `Approve` requires `gates_passed == 7` exactly
    - `EscalateToHuman` always carries `reality_prediction` so human reviewer sees the model's prediction
    - LowConfidence entropy = shannon_entropy_of_distribution where each outcome has uniform weight 1/N
    - First-violating-skill in Gate 4 picked via deterministic iteration (HashMap → sort by SkillId discriminant)
    - Per `BR-INFER-01`, verify() is callable from outside the workspace ONLY via MCP (TASK-INFER-011); direct in-process FFI is workspace-internal
    - Tests use `MockedAlwaysApprove` / `MockedAlwaysReject` for ConstellationGuard
    - All `?` sites wrap `e.log_context(file!())` per &lt;error_handling&gt;
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa verify` shows ≥ 10 passing tests covering each FailedGate variant + Approve + ordering
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `verify_all_gates_pass_returns_approve` passes with gates_passed=7
    - Test `verify_sha_drift_short_circuits_before_compile` passes (no panel materialization called)
    - Test `verify_ood_refuse_converted_to_escalate` passes (compile returns OodRefuse, verify returns EscalateToHuman)
    - Test `verify_stale_constellation_returns_err_not_escalate` passes
    - Test `verify_gate_ordering_sha_before_ood` passes (synthesize bundle that fails both → Gate 6 fires)
  </verification>
</definition_of_done>

<pseudo_code>
fn verify(&self, patch, context) -> Result<VerifyVerdict, MejepaInferError> {
    // Gate 6 (cheapest first)
    if let Some(fg) = check_source_sha_drift(patch)? {
        let placeholder = empty_reality_prediction(self.witness_chain.current_offset());
        return Ok(VerifyVerdict::EscalateToHuman { failed_gate: fg, reality_prediction: placeholder });
    }
    // Gate 5
    if let Err(e) = self.witness_chain.replay_segment(&patch.witness_chain_segment) {
        if matches!(e, MejepaInferError::WitnessChainBroken { .. }) {
            let placeholder = empty_reality_prediction(self.witness_chain.current_offset());
            return Ok(VerifyVerdict::EscalateToHuman { failed_gate: FailedGate::WitnessChainBroken, reality_prediction: placeholder });
        }
        return Err(e);
    }

    // compile
    let (prediction, panel_pred_t2) = match self.compile_with_panel(patch, context) {
        Ok(p) => p,
        Err(MejepaInferError::OodRefuse { score, .. }) => {
            // Convert to EscalateToHuman per AC-INFER-01-03 / verify never propagates OodRefuse
            let placeholder = empty_reality_prediction_with_ood(score, self.witness_chain.current_offset());
            return Ok(VerifyVerdict::EscalateToHuman {
                failed_gate: FailedGate::OutOfDistribution { score },
                reality_prediction: placeholder,
            });
        },
        Err(e) => return Err(e), // CalibrationStale etc.
    };

    // Gate 1 (defense in depth)
    if prediction.out_of_distribution_score > self.config.ood_refuse_threshold {
        return Ok(VerifyVerdict::EscalateToHuman {
            failed_gate: FailedGate::OutOfDistribution { score: prediction.out_of_distribution_score },
            reality_prediction: prediction,
        });
    }
    // Gate 2
    if prediction.predicted_oracle_pass <= 0.95 {
        return Ok(VerifyVerdict::EscalateToHuman {
            failed_gate: FailedGate::PredictedFailure { failing_tests: prediction.predicted_failing_tests.clone() },
            reality_prediction: prediction,
        });
    }
    // Gate 3
    if prediction.confidence.outcome_set.len() > self.config.outcome_set_max {
        let entropy = shannon_entropy(&prediction.confidence.outcome_set);
        return Ok(VerifyVerdict::EscalateToHuman {
            failed_gate: FailedGate::LowConfidence { entropy },
            reality_prediction: prediction,
        });
    }
    // Gate 4
    let mut sorted: Vec<_> = prediction.granger_attestations.iter().collect();
    sorted.sort_by_key(|(s, _)| *s);
    for (skill, p) in sorted {
        if *p > self.config.p_threshold {
            return Ok(VerifyVerdict::EscalateToHuman {
                failed_gate: FailedGate::GrangerRejection { skill: *skill, p_value: *p },
                reality_prediction: prediction,
            });
        }
    }
    // Gate 7
    let gtau = self.constellation.gtau_test(&panel_pred_t2);
    if !gtau.satisfied {
        return Ok(VerifyVerdict::EscalateToHuman {
            failed_gate: FailedGate::ConstellationGuardRejected {
                violating_embedders: gtau.violating_embedders,
                violating_chunks: gtau.violating_chunks,
            },
            reality_prediction: prediction,
        });
    }

    // All passed; persist + Approve
    let session_id: [u8; 16] = context.task_id.0;
    self.storage.write_live_prediction((session_id, prediction.witness_chain_offset), &prediction)?;
    Ok(VerifyVerdict::Approve { reality_prediction: prediction, gates_passed: 7 })
}

fn shannon_entropy(set: &[OracleOutcome]) -> f32 {
    let n = set.len() as f32;
    if n <= 1.0 { return 0.0; }
    let p = 1.0_f32 / n;
    -n * p * p.ln() / std::f32::consts::LN_2
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/verify_helpers.rs">empty_reality_prediction stubs + shannon_entropy + MockedAlwaysApprove/MockedAlwaysReject ConstellationGuard test impls (under #[cfg(test)])</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/compiler.rs">add `verify`, `compile_with_panel`, `convert_ood_refuse_to_verdict` methods on `impl MeJepaCompiler`</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod verify_helpers;` (test helpers re-exported under cfg test)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa verify shows ≥ 10 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>verify_all_gates_pass_returns_approve_with_gates_passed_7 passes</criterion>
  <criterion>verify_sha_drift_short_circuits passes</criterion>
  <criterion>verify_ood_refuse_converted_to_escalate passes</criterion>
  <criterion>verify_stale_constellation_returns_err_not_escalate passes</criterion>
  <criterion>verify_gate_ordering_sha_before_ood passes</criterion>
  <criterion>verify never returns OodRefuse Err</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa verify</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
