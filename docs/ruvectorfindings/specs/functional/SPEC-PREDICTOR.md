# SPEC-PREDICTOR — ME-JEPA-Code Predictor + Frozen Target + VICReg + Oracle Head

> **Level 2 — Functional Specification.** Defines WHAT the Phase 2 predictor domain must do, not HOW. Implementation details live in `TECH-PREDICTOR.md`. Conflicts with the constitution resolve to the constitution; conflicts with `<utml_v1_invariants>` rule 2 (`ΔΞ.target_collapse = 0 by construction`) resolve to that rule.

```xml
<functional_spec id="SPEC-PREDICTOR" version="1.0">

<metadata>
  <title>ME-JEPA-Code Predictor + Frozen Target Readback + VICReg Loss + Oracle Head</title>
  <status>draft</status>
  <owner>Phase 2 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>      <!-- supplies the 5,120-d Panel + frozen Instrument trait -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>        <!-- 21-embedder ensemble, frozen during predictor training -->
    <spec_ref>SPEC-CORPUS</spec_ref>           <!-- 8-category × 300-task mutation corpus = source of (panel[t=0..2]) tuples -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>     <!-- AST-perfect chunks consumed by code-domain instruments upstream -->
    <spec_ref>SPEC-UTML</spec_ref>             <!-- the predictor's L_predict feeds into compute_delta_p (UTML §3) -->
    <spec_ref>SPEC-WITNESS</spec_ref>          <!-- per-step training cert chain (Phase 3, downstream of this spec) -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.4 (predictor architecture: 6-layer transformer, hidden 1024, 8 heads, GeLU, residual+LN, AdamW lr 5e-4 cosine, weight decay 0.04, batch 64, λ_var=25, λ_cov=1, λ_inv=25, γ=1)</source>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.5 (Oracle head: Linear[panel_dim → num_tests + 1], softmax → per-test pass probability)</source>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.5.5 (5,120-d panel layout — concat input dim 10,240, output dim 5,120)</source>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §7 Phase 2 deliverables + DoD</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2 (target-side structural collapse-immunity — frozen externally-defined instruments; ΔΞ.target_collapse = 0 by construction)</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.3 (predictor-side defenses: VICReg + pairwise MI audit + constellation Gτ + negative-action ablation)</source>
    <source type="design">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §4 (RTX 5090 VRAM budget table: 14 GB embedders + 3 GB predictor + 5 GB activations + 2 GB AdamW + 1 GB KV/scratch ≈ 28-29 GB)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; anti_patterns "no learnable target encoder"; failure_taxonomy MEJEPA_OBSERVE_BATCH_NAN; performance_budgets predictor_forward_p50 &lt; 50 ms)</source>
    <source type="prd">user request 2026-05-09: write SPEC-PREDICTOR + TECH-PREDICTOR for the trainable transformer + frozen target adapter + VICReg loss + Oracle head</source>
  </source_documents>
</metadata>

<overview>
The predictor domain is the **only trainable component** of ME-JEPA-Code. It is a 6-layer transformer
that takes the joint embedding of two consecutive time steps `(panel[t=0], panel[t=1])` — a 10,240-d
concatenation produced by the upstream Instruments domain — and outputs a 5,120-d predicted
`panel[t=2]`, the post-execution panel that an actual Docker oracle run would have produced. On top
of the predicted panel sits a small **Oracle head**, a Linear projection to `num_tests + 1` logits
that, after softmax, gives the per-test pass probability + a single "exception" probability. The
predicted panel + Oracle head together are the model output of doc 09's `mejepa.predict()` API.

Training supervises the predictor's output against a **frozen target panel** read back from the
upstream instruments evaluated at `panel[t=2]`. The target adapter has **zero trainable
parameters** by construction (per doc 11 §5.2 and the constitution `<anti_patterns>` "no learnable
target encoder" rule). This is the architectural property that makes ME-JEPA structurally
collapse-immune on the target side: there is no gradient path from the loss into any instrument
weight, and `ΔΞ.target_collapse = 0` is locked at 1.0 in the per-step UTML training certificate
for every step. Any code path that breaks this property — accidentally registering an instrument's
parameters with the optimizer, or omitting the gradient hook that asserts `‖dL/dθ_instrument‖ == 0`
— is a CRITICAL bug, not a feature regression: it must be fixed at the trait level before merge.

Predictor-side collapse is a separate concern. The trainable transformer can still degenerate to a
constant output, ignore the patch context, or learn a low-rank shortcut. This spec defends against
that with the VICReg variance + covariance + invariance terms (per doc 09 §2.4 default
λ_var=25, λ_cov=1, λ_inv=25, γ=1) and a runtime variance-floor diagnostic that flags
`MEJEPA_PRED_VICREG_DEGENERATE` if more than 50% of the predictor's output dimensions fall below γ
for two consecutive validation passes.

Beneficiaries: (1) the Phase 3 training loop calls `predictor.forward()` + `vicreg_loss()` per
batch and writes the four-term loss decomposition into the per-step training certificate; (2) the
Phase 4 inference path calls `predictor.forward()` + `oracle_head.predict_probabilities()` to
produce the calibrated `RealityPrediction` returned by `mejepa.verify()`; (3) the Phase 7
shift-subscriber materializes panels from outer-loop edits and uses the predictor to emit live
predictions at &lt; 5 s p50 latency.

Phase 2 ships this domain end-to-end with FSV evidence at
`/tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json, vicreg-loss-evidence.json}`,
satisfying the doc 09 §7 Phase 2 Definition of Done: forward pass on a 64-sample batch in &lt; 50 ms
on RTX 5090 (candle-cuda, bf16), finite loss on a single-batch dry-run with no NaN/Inf, VRAM
steady state &lt; 8 GB at batch 64.
</overview>

<user_types>

  <user_type id="UT-01" name="Phase 3 Training Loop (mejepa-train)">
    <description>The crate-binary that drives the full corpus training run: AdamW + cosine LR + per-step training-cert emission + holdout evaluation every 1,000 steps + witness-chain commits per epoch.</description>
    <permissions>READ panel triplets `(panel[t=0], panel[t=1], panel[t=2])` from CF_MEJEPA_PANELS; WRITE updated predictor weights to CF_MEJEPA_WEIGHT_BLOBS keyed by `theta_sha`; INVOKE `predictor.forward()` + `vicreg_loss()` + backward + `optimizer.step()`. NEVER mutates the FrozenTargetAdapter or any underlying Instrument.</permissions>
    <goals>One AdamW step per call: forward(panel_t0, panel_t1) → predicted; frozen_target_adapter.encode(panel_t2) → target; vicreg_loss(predicted, target) → scalar L_total + four-component breakdown for the training cert.</goals>
  </user_type>

  <user_type id="UT-02" name="Phase 4 Inference Caller (mejepa.verify)">
    <description>The Phase 4 `MeJepaCompiler::verify()` MCP tool that, given an attempt's pre-execution panels, returns a calibrated RealityPrediction (per-test pass probability + conformal interval + OOD score).</description>
    <permissions>READ pinned weights from CF_MEJEPA_WEIGHT_BLOBS; INVOKE `predictor.forward()` and `oracle_head.predict_probabilities()` in inference mode (no_grad, eval mode, KV-cache where applicable). NEVER trains.</permissions>
    <goals>Single-prediction latency &lt; 100 ms end-to-end including instrument re-evaluation; refuses with `MEJEPA_INFER_OOD_REFUSE` when OOD score &gt; 0.5.</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 7 Shift-Subscriber (mejepa-shift-subscriber)">
    <description>The Tokio subscriber that observes harness shift-log + witness-chain entries and materializes live predictions per doc 09 §3.8.</description>
    <permissions>INVOKE `predictor.forward()` + `oracle_head.predict_probabilities()` in inference mode; WRITE live RealityPrediction entries to CF_MEJEPA_LIVE_PREDICTIONS; never trains.</permissions>
    <goals>Emit a live prediction per shift event at &lt; 5 s p50 / &lt; 30 s p99 latency.</goals>
  </user_type>

  <user_type id="UT-04" name="Operator (human)">
    <description>The human running the system; reviews FSV evidence; promotes / rolls back checkpoints; triggers retrain modes; reads per-step training certificates.</description>
    <permissions>READ FSV at `/tmp/contextgraph-mejepa-predictor-fsv/`; READ CF_MEJEPA_TRAIN_CERTS; APPROVE λ defaults changes (constitution amendment required); CANNOT silently make an instrument trainable.</permissions>
    <goals>Verify forward/loss FSV is reproducible, dim-correct, finite, and that gradient hooks fire `MEJEPA_PRED_FROZEN_TARGET_GRAD` on adversarial dry-runs.</goals>
  </user_type>

  <user_type id="UT-05" name="ME-JEPA Predictor (the trained artifact, indirect)">
    <description>The 6-layer transformer + Oracle head + FrozenTargetAdapter as a single library object. Not an actor; consumes panels and emits PredictedPanel + OracleLogits. Listed for completeness of the user-types matrix.</description>
    <permissions>OWNS its trainable weights (only the transformer + Oracle head); CANNOT modify FrozenTargetAdapter (the trait API forbids `&amp;mut self` modification of instruments).</permissions>
    <goals>Honor the dim contract (10,240 → 5,120), produce finite tensors, refuse forward on dim-mismatched inputs.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-PRED-01" priority="must-have">
    <narrative>
      <as_a>Phase 3 Training Loop (UT-01)</as_a>
      <i_want_to>Call `predictor.forward(panel_t0, panel_t1)` on a batch of 64 panel pairs and receive a finite 5,120-d predicted panel batch in under 50 ms on RTX 5090</i_want_to>
      <so_that>I can complete one AdamW step (forward + loss + backward + step + cert-emit) within the &lt; 12 hour walltime budget for 10 epochs on the 2,400-example corpus</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-01-01">
        <given>A batch of 64 panel pairs each shaped `(5120, 5120)` (i.e., panel_t0 + panel_t1), bf16 dtype, on the RTX 5090 candle-cuda device</given>
        <when>The training loop calls `predictor.forward(panel_t0_batch, panel_t1_batch)`</when>
        <then>A `PredictedPanel` batch of shape `(64, 5120)` is returned in &lt; 50 ms (p50 over 100 warm calls); every element is finite (no NaN, no Inf); concat-dim assertion fires on shape `(64, 10240)` after the input concatenation step</then>
      </criterion>
      <criterion id="AC-PRED-01-02">
        <given>Two forward passes on the same batch with the same predictor weights</given>
        <when>The loop calls `predictor.forward()` twice in eval mode (no dropout, no_grad)</when>
        <then>The two PredictedPanel tensors are byte-equal (SHA-256 identical); the predictor is deterministic with respect to (input bytes, weight bytes, code_version)</then>
      </criterion>
      <criterion id="AC-PRED-01-03">
        <given>An input where `panel_t0.shape != (batch, 5120)` or `panel_t1.shape != (batch, 5120)`</given>
        <when>The loop calls `predictor.forward()`</when>
        <then>The call fails closed with `MEJEPA_PRED_DIM_MISMATCH`; no partial computation; no silent zero-pad or truncation</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-PRED-02" priority="must-have">
    <narrative>
      <as_a>Phase 3 Training Loop (UT-01)</as_a>
      <i_want_to>Compute VICReg loss (`L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance`) on a batch of (predicted, target) panels and recover the four-term decomposition for the training cert</i_want_to>
      <so_that>The per-step UTML training certificate (`CF_MEJEPA_TRAIN_CERTS`) records every loss component used to compute ΔP, and the predictor cannot collapse to a constant output without the variance / covariance terms exposing it</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-02-01">
        <given>A `PredictedPanel` batch shape `(64, 5120)` and a `TargetPanel` batch shape `(64, 5120)`, both bf16, on RTX 5090, with default `VicregLambdas { var: 25.0, cov: 1.0, inv: 25.0, gamma: 1.0 }`</given>
        <when>The loop calls `vicreg_loss(predicted, target, lambdas)`</when>
        <then>A `LossOutputs` is returned with: `l_predict` (Huber δ=1 over the residual), `l_variance` (mean over dims of `max(0, γ − std(z_dim))`), `l_covariance` (sum of off-diagonal squared covariances divided by panel_dim), `l_invariance` (MSE between context and target latents per VICReg §3.3), `l_total = l_predict + 25·l_variance + 1·l_covariance + 25·l_invariance`; every component finite; all components ≥ 0</then>
      </criterion>
      <criterion id="AC-PRED-02-02">
        <given>A degenerate predicted batch where every row is identically zero (predictor has collapsed)</given>
        <when>The loop computes `vicreg_loss`</when>
        <then>`l_variance ≥ γ · panel_dim / panel_dim = 1.0` (per-dim std == 0 hits the `max(0, γ - 0)` ceiling at γ); `l_covariance` is well-defined and zero (zero variance ⇒ zero covariance); the total loss surfaces the collapse via the variance term, NOT via NaN; calling code can use `l_variance &gt; 0.5 · γ` as a collapse early-warning</then>
      </criterion>
      <criterion id="AC-PRED-02-03">
        <given>Inputs that contain a NaN or Inf (e.g., from an upstream instrument bug)</given>
        <when>The loop calls `vicreg_loss`</when>
        <then>The function fails closed with `MEJEPA_PRED_NAN_DETECTED` BEFORE returning any tensor; no silent NaN propagation through the optimizer</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-PRED-03" priority="must-have">
    <narrative>
      <as_a>Operator (UT-04)</as_a>
      <i_want_to>The frozen target adapter to provably never receive a gradient through any of its underlying instruments, regardless of training-loop misconfiguration</i_want_to>
      <so_that>The architectural property "ME-JEPA-Code is structurally collapse-immune on the target side" (constitution `<utml_v1_invariants>` rule 2 + anti-pattern rule "no learnable target encoder") is enforced by the type system + a runtime gradient-hook assertion, not by code review</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-03-01">
        <given>The full predictor + frozen target adapter pipeline at training startup</given>
        <when>The training loop calls `optimizer.parameters()` to register trainable parameters</when>
        <then>Only `MeJepaPredictor::trainable_parameters()` (transformer body + Oracle head) is enumerated; `FrozenTargetAdapter::parameters()` is `&amp;[]` by construction (no instruments expose `&amp;Tensor` or `&amp;Var` to AdamW); a startup assertion `assert!(frozen_target_adapter.parameters().is_empty())` is required and FSV-verified</then>
      </criterion>
      <criterion id="AC-PRED-03-02">
        <given>A standard forward + backward pass over a batch where someone has accidentally let a gradient flow through an instrument</given>
        <when>The training loop calls `loss.backward()` and the runtime gradient-hook traverses every instrument's potential weight references</when>
        <then>The hook detects `‖dL/dθ_instrument‖ != 0` and immediately raises `MEJEPA_PRED_FROZEN_TARGET_GRAD` (CRITICAL severity); the AdamW step is aborted; the step is NOT logged to `CF_MEJEPA_TRAIN_CERTS` (no poisoned cert); the operator is alerted via the recommendation channel and the implicated instrument's trait impl is flagged for trait-level fix</then>
      </criterion>
      <criterion id="AC-PRED-03-03">
        <given>The FrozenTargetAdapter encodes a `panel[t=2]` from instrument outputs, and the same `panel[t=2]` is encoded twice with the same code_version + embedder_versions</given>
        <when>The two encodings are SHA-256-hashed</when>
        <then>The hashes are byte-equal — frozen target output is deterministic with respect to (input, code_version, embedder_versions)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-PRED-04" priority="must-have">
    <narrative>
      <as_a>Phase 4 Inference Caller (UT-02)</as_a>
      <i_want_to>Read the predictor's predicted panel through an Oracle head that maps to per-test pass probabilities + a single "exception" probability</i_want_to>
      <so_that>The end-user MCP tool `mejepa_predict(patch, context) -> RealityPrediction` returns a calibrated probability per test (after Phase 4 conformal calibration consumes these logits)</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-04-01">
        <given>A predicted panel of shape `(batch, 5120)` and an Oracle head sized for `num_tests = T`</given>
        <when>The caller calls `oracle_head.predict_logits(predicted_panel)`</when>
        <then>A logits tensor of shape `(batch, T + 1)` is returned; finite; the `T + 1`-th index is the dedicated "exception" probability (per doc 09 §2.5 "+1 for exception")</then>
      </criterion>
      <criterion id="AC-PRED-04-02">
        <given>The same logits</given>
        <when>The caller calls `oracle_head.predict_probabilities(logits)` (softmax over the per-test+exception axis)</when>
        <then>The returned tensor has the same shape, every row sums to 1.0 ± 1e-5, every value ∈ [0, 1]</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-PRED-05" priority="must-have">
    <narrative>
      <as_a>Operator (UT-04)</as_a>
      <i_want_to>The predictor's VRAM footprint at batch 64 to be ≤ 8 GB steady-state on RTX 5090, with batch 128 / gradient-checkpointing / a 12-layer extended predictor reachable when the embedder ensemble lazy-loads E15-E21</i_want_to>
      <so_that>The doc 12 §4 VRAM budget table ("14 GB embedders + 3 GB predictor + 5 GB activations + 2 GB AdamW + 1 GB scratch ≈ 28-29 GB total fitting in the 32 GB RTX 5090") holds with operating headroom for future extension</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-05-01">
        <given>The predictor + frozen target adapter loaded with batch 64, bf16 weights + activations, candle-cuda on RTX 5090</given>
        <when>A full training step (forward + VICReg loss + backward + AdamW step) is executed and `cudaMemGetInfo` is queried at steady state</when>
        <then>Resident allocation is ≤ 8 GB (3 GB predictor weights + 2 GB AdamW state + 2 GB activations at batch 64 + 1 GB scratch / KV); the doc 12 §4 budget line for "ME-JEPA predictor" is satisfied</then>
      </criterion>
      <criterion id="AC-PRED-05-02">
        <given>VRAM allocation crosses the 9 GB warning threshold (e.g., a misbehaving instrument leaks tensors into the predictor's allocator pool)</given>
        <when>The next step's `cudaMemGetInfo` poll is taken</when>
        <then>The predictor raises `MEJEPA_PRED_VRAM_EXCEEDED`; the training loop pauses; the operator is notified to either reduce batch size, enable gradient checkpointing, or evict E15-E21 from VRAM</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-PRED-06" priority="must-have">
    <narrative>
      <as_a>Operator (UT-04)</as_a>
      <i_want_to>The Phase 2 dry-run command `cargo run --release -p context-graph-mejepa -- train --dry-run --batches 1 --batch-size 64` to report a finite loss of order 10² with forward latency &lt; 50 ms and VRAM &lt; 8 GB</i_want_to>
      <so_that>The doc 09 §7 Phase 2 Definition of Done can be verified by a single command that closes the FSV loop</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-PRED-06-01">
        <given>A fresh checkout on RTX 5090 with cuda 13.2 + sm_120a, no pre-existing CF_MEJEPA_PANELS contents, the corpus split's first 64 examples available</given>
        <when>The operator runs the dry-run command</when>
        <then>The command prints structured output containing `forward_latency_ms_p50` &lt; 50, `loss_total` finite ∈ [10, 1000], `loss_predict` finite, `loss_variance` finite, `loss_covariance` finite, `loss_invariance` finite, `vram_resident_bytes` &lt; 8 × 2³⁰; exit code 0; FSV evidence at `/tmp/contextgraph-mejepa-predictor-fsv/forward-pass-evidence.json` and `vicreg-loss-evidence.json` is written and re-readable</then>
      </criterion>
      <criterion id="AC-PRED-06-02">
        <given>Any of the above metrics fails the threshold</given>
        <when>The command runs</when>
        <then>The dry-run reports `dod_pass: false` with the failing metric and remediation; exit code 2; FSV evidence is still written so the operator can inspect</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-PRED-01" story_ref="US-PRED-01" priority="must">
    <description>The predictor SHALL accept inputs `(panel_t0: Tensor, panel_t1: Tensor)` each of shape `(batch, 5120)` (per SPEC-INSTRUMENTS panel layout) and produce a predicted panel of shape `(batch, 5120)` (the predicted post-execution panel).</description>
    <rationale>Per doc 09 §2.4 + §3.5.5: input is concat of two consecutive 5,120-d panels = 10,240-d; output is the third panel = 5,120-d. The SPEC-INSTRUMENTS panel layout (15 slots summing to 5,120) is the upstream contract; this spec inherits the dim and refuses to change it without a constitution amendment.</rationale>
    <constraints>Input dims fixed at 5120 per panel. No silent zero-pad. No silent truncation. dtype bf16 on candle-cuda.</constraints>
    <verification_method>Unit test asserting dim-mismatch raises `MEJEPA_PRED_DIM_MISMATCH`; FSV evidence at `/tmp/contextgraph-mejepa-predictor-fsv/forward-pass-evidence.json` records `input_shape`, `output_shape`, `panel_dim_constant`.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-02" story_ref="US-PRED-01" priority="must">
    <description>The predictor architecture SHALL be a 6-layer transformer with hidden dim 1,024, 8 attention heads, GeLU activations, residual connections + layer norm, per doc 09 §2.4. Headroom for 12-layer extension SHALL be reachable by a single config change without code refactor.</description>
    <rationale>doc 09 §2.4 fixes the architecture; extension headroom is required by AC-PRED-05 + the doc 12 §4 budget when E15-E21 lazy-load.</rationale>
    <constraints>Layer count = 6 (default). Hidden = 1024. Heads = 8. dropout = 0 (dropout is regularization for training; production uses no dropout per CLAUDE.md "no silent fallback" rule). Activation = GeLU (not ReLU, not Swish — fixed by doc 09).</constraints>
    <verification_method>Architecture summary printed at startup; FSV evidence enumerates layers / hidden / heads / activations.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-03" story_ref="US-PRED-01" priority="must">
    <description>The predictor's forward pass at batch 64 on RTX 5090 (candle-cuda, sm_120a, bf16) SHALL complete in &lt; 50 ms p50 over 100 warm calls.</description>
    <rationale>Constitution `<performance_budgets>` `predictor_forward_p50` &lt; 50 ms is the binding constraint; per-prediction end-to-end latency budget &lt; 100 ms requires the forward pass to consume &lt; 50 ms.</rationale>
    <verification_method>FSV evidence records `forward_latency_ms_p50`, `_p99` over 100 warm calls; failure is `dod_pass: false`.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-04" story_ref="US-PRED-01,US-PRED-06" priority="must">
    <description>Every output tensor produced by the predictor's forward pass SHALL be finite (no NaN, no Inf). On detection of NaN or Inf in any layer's output, the call SHALL fail closed with `MEJEPA_PRED_NAN_DETECTED` BEFORE returning to the caller.</description>
    <rationale>Constitution `<error_handling>` "fail-closed at boundaries". Constitution `<failure_taxonomy>` `MEJEPA_OBSERVE_BATCH_NAN` rolls back to a prior checkpoint on NaN; this requirement is the upstream sentinel.</rationale>
    <verification_method>FSV adversarial dry-run: inject a NaN into panel_t0, confirm the call raises `MEJEPA_PRED_NAN_DETECTED` instead of silent propagation.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-05" story_ref="US-PRED-01" priority="must">
    <description>The predictor's forward pass SHALL be deterministic: same (input bytes, weight bytes, code_version, eval mode) ⇒ byte-equal output tensors. Stochastic operations (e.g., training-mode dropout) SHALL be disabled in eval mode.</description>
    <rationale>Constitution `<anti_patterns>` "no random initialization in instrument outputs"; the same property extends to predictor inference. Determinism is required by Phase 7 §3.8 subscriber idempotency.</rationale>
    <verification_method>FSV evidence: SHA-256 of two consecutive eval-mode forward passes is byte-equal.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-06" story_ref="US-PRED-02" priority="must">
    <description>The VICReg loss function SHALL implement L_total = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance with the doc 09 §2.4 defaults λ_var=25, λ_cov=1, λ_inv=25, γ=1. L_predict uses Huber loss δ=1 over the (predicted, target) residual.</description>
    <rationale>doc 09 §2.4 fixes the loss formulation. Defaults are constitutionally locked; changes require operator amendment.</rationale>
    <constraints>Huber δ=1 (not L1, not L2). Variance term per VICReg Bardes 2022: `L_var = mean_d max(0, γ − std(z_d))`. Covariance term: `L_cov = sum_{i ≠ j} C(z)[i, j]² / d`. Invariance term: MSE between context and target latents.</constraints>
    <verification_method>Unit tests verifying each term's formula on known synthetic inputs (zero-vector → l_var = γ; constant non-zero → l_var = γ; identity-cov → l_cov = 0); FSV evidence at `vicreg-loss-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-07" story_ref="US-PRED-02" priority="must">
    <description>The four-term loss decomposition (`l_predict`, `l_variance`, `l_covariance`, `l_invariance`) plus the total `l_total` SHALL be returned as a structured `LossOutputs` value, NOT collapsed to a single scalar. The training loop must read all four values to write them into the per-step `CF_MEJEPA_TRAIN_CERTS` cert.</description>
    <rationale>UTML §11 (per-step cert records) requires all four loss components. SPEC-UTML's compute_delta_p consumes l_predict directly.</rationale>
    <verification_method>Type signature: `vicreg_loss(...) -> Result<LossOutputs, LossError>` where `LossOutputs { l_predict, l_variance, l_covariance, l_invariance, l_total }`. Unit test verifies all five fields are populated.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-08" story_ref="US-PRED-02" priority="must">
    <description>The runtime variance-floor diagnostic SHALL flag `MEJEPA_PRED_VICREG_DEGENERATE` if more than 50% of the predictor's output dimensions have per-dim std &lt; γ for two consecutive validation passes.</description>
    <rationale>doc 11 §5.3 predictor-side defense #1 (VICReg variance term as collapse early-warning). Two-consecutive-pass debounce avoids spurious flags from noise in a single validation batch.</rationale>
    <verification_method>FSV adversarial dry-run: train predictor to a constant output for 2 batches, confirm flag fires.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-09" story_ref="US-PRED-03" priority="must">
    <description>The `FrozenTargetAdapter` SHALL expose ZERO trainable parameters: `FrozenTargetAdapter::parameters()` returns `&amp;[]` by construction. The trait API forbids `&amp;mut self` methods that modify instrument weights.</description>
    <rationale>Constitution `<utml_v1_invariants>` rule 2: ΔΞ.target_collapse = 0 by construction. Constitution `<anti_patterns>` "no learnable target encoder". doc 11 §5.2: target-side structural collapse-immunity is the architectural property.</rationale>
    <constraints>The Rust type system enforces this: instruments are held by `Box<dyn Instrument>` where `Instrument` has no `&amp;mut self` methods that touch weights. AdamW is constructed with `predictor.trainable_parameters()` only — never with adapter state.</constraints>
    <verification_method>Compile-time: trait API review. Runtime: startup assertion `assert!(frozen_target_adapter.parameters().is_empty())`. FSV evidence: snapshot of `optimizer.parameters().len()` matches `predictor.trainable_parameters().len()`.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-10" story_ref="US-PRED-03" priority="must">
    <description>A runtime gradient-hook SHALL detect any non-zero gradient flowing into an instrument's underlying tensors and raise `MEJEPA_PRED_FROZEN_TARGET_GRAD` (CRITICAL severity). The training step SHALL be aborted; no per-step training cert SHALL be written for the poisoned step.</description>
    <rationale>Defense-in-depth: the type-system enforcement (REQ-PRED-09) prevents accidental registration; the gradient hook catches the case where someone reaches around the trait to tap a tensor reference.</rationale>
    <constraints>Hook fires on `‖dL/dθ_instrument‖ &gt; 1e-12` (numerical-noise floor). Detection MUST happen before `optimizer.step()` is called. The implicated instrument id is logged + persisted to `memory/blockers/` so the operator can fix at the trait level.</constraints>
    <verification_method>FSV adversarial dry-run: register an instrument's tensor with the optimizer pool (synthetic violation), confirm the hook fires before step() runs. The cert chain MUST not contain a record for the poisoned step.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-11" story_ref="US-PRED-03" priority="must">
    <description>The FrozenTargetAdapter SHALL produce byte-equal target panels across calls with the same (input, code_version, embedder_versions). All upstream instruments (per SPEC-INSTRUMENTS) honor the determinism contract; the adapter does not introduce any non-determinism (no shuffling, no random sampling, no temperature).</description>
    <rationale>Constitution `<anti_patterns>` "no random initialization in instrument outputs". Phase 7 subscriber idempotency depends on this.</rationale>
    <verification_method>FSV: same `panel[t=2]` input encoded twice ⇒ byte-equal target. SHA-256 hash recorded.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-12" story_ref="US-PRED-04" priority="must">
    <description>The Oracle head SHALL be a single Linear projection from the 5,120-d predicted panel to `num_tests + 1` logits, where the `+1` slot is the dedicated "exception" probability per doc 09 §2.5.</description>
    <rationale>doc 09 §2.5 fixes the architecture: one Linear layer; softmax at inference; conformal calibration consumes the logits in Phase 4.</rationale>
    <constraints>No bias-only models. No multi-layer head (the simplicity is intentional — let the predictor learn the representation, the head is a thin projection).</constraints>
    <verification_method>Unit test: `oracle_head.predict_logits(panel).shape == (batch, num_tests + 1)`. Probability test: softmax row-sum = 1.0 ± 1e-5.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-13" story_ref="US-PRED-05" priority="must">
    <description>Steady-state VRAM consumption of the predictor + frozen target adapter (loaded weights, AdamW state, activations at batch 64, scratch + KV cache) SHALL be ≤ 8 GB on RTX 5090 with bf16.</description>
    <rationale>Constitution `<performance_budgets>` vram_steady_state ≤ 29 GB; doc 12 §4 budget line for the predictor is 3 + 5 + 2 + (small) ≈ ~10 GB peak; the spec budgets 8 GB steady-state with headroom for activation peaks.</rationale>
    <constraints>Measurement via `cudaMemGetInfo` between training steps (steady, not peak). Excludes embedder ensemble (E1-E14 + E15-E21 are accounted for separately in doc 12 §4).</constraints>
    <verification_method>FSV records `vram_resident_bytes` at end of dry-run; threshold 8 × 2³⁰ bytes.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-14" story_ref="US-PRED-05" priority="must">
    <description>VRAM allocations crossing the 9 GB warning threshold SHALL trigger `MEJEPA_PRED_VRAM_EXCEEDED`. The training loop SHALL pause and surface the recommendation channel; the operator chooses among (a) reduce batch size, (b) enable gradient checkpointing, (c) evict E15-E21 from VRAM.</description>
    <rationale>doc 12 §4 risk #2 (VRAM allocation contention) explicitly calls out lazy-load of E15-E21 as the headroom mechanism.</rationale>
    <constraints>Threshold 9 × 2³⁰ bytes. Polled between steps, not in the hot path. Auto-eviction is NOT applied silently — the operator decides.</constraints>
    <verification_method>FSV adversarial: synthetic over-allocation (e.g., batch 256), confirm the flag fires.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-15" story_ref="US-PRED-06" priority="must">
    <description>The `cargo run --release -p context-graph-mejepa -- train --dry-run --batches 1 --batch-size 64` command SHALL exit 0 with FSV evidence at `/tmp/contextgraph-mejepa-predictor-fsv/{forward-pass-evidence.json, vicreg-loss-evidence.json}` written, re-read, and asserted to contain finite metrics meeting REQ-PRED-03 (latency &lt; 50 ms), REQ-PRED-04 (no NaN/Inf), REQ-PRED-13 (VRAM &lt; 8 GB).</description>
    <rationale>doc 09 §7 Phase 2 Definition of Done. Constitution `<fsv_protocol>` requires re-read + assertion before reporting success.</rationale>
    <verification_method>The dry-run binary itself executes the assertion loop (write → re-read → assert). FSV files are mode 0o600 per SEC-MEJEPA-07.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-16" story_ref="US-PRED-02" priority="must">
    <description>The L_predict scalar SHALL be exposed as a public field of `LossOutputs` so the Phase 3 SPEC-UTML compute_delta_p can read it directly. The predictor SHALL not collapse the four-term loss into a private aggregate.</description>
    <rationale>SPEC-UTML cross-reference: ΔP = f(L_predict). doc 11 §3 ΔP formula consumes L_predict per training step.</rationale>
    <verification_method>Public field on `LossOutputs::l_predict`. SPEC-UTML technical spec reads it via the public path.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-17" story_ref="US-PRED-01" priority="should">
    <description>The predictor SHALL support a configuration flag enabling gradient checkpointing on transformer layers when batch size or model depth exceeds the 8 GB steady-state budget.</description>
    <rationale>Headroom for batch=128 / 12-layer extension per doc 12 §4 risk #2.</rationale>
    <constraints>Default OFF in Phase 2; opt-in via `PredictorConfig { gradient_checkpointing: true, ... }`.</constraints>
    <verification_method>Unit test confirms enabling the flag reduces measured peak activation memory at batch 128.</verification_method>
  </requirement>

  <requirement id="REQ-PRED-18" story_ref="US-PRED-01,US-PRED-04" priority="must">
    <description>All inputs (panel tensors, weight blobs, hyperparameters) SHALL be validated at MCP / library boundaries: dim-correct, finite, dtype = bf16 on candle-cuda. Malformed inputs are rejected with `MEJEPA_PRED_DIM_MISMATCH` or `MEJEPA_PRED_NAN_DETECTED`; never silently truncated, padded, or cast.</description>
    <rationale>Constitution `<security_requirements>` SEC-MEJEPA-01 (boundary validation); `<error_handling>` "fail-closed at boundaries".</rationale>
    <verification_method>Unit tests for each rejection condition; FSV records the rejected-inputs path in adversarial dry-runs.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-PRED-01" req_ref="REQ-PRED-09,REQ-PRED-10">
    <condition>Any code path produces a non-zero gradient on an instrument's underlying tensor</condition>
    <action>Abort the training step; raise `MEJEPA_PRED_FROZEN_TARGET_GRAD` (CRITICAL); do NOT write a training cert for the poisoned step; persist a `memory/blockers/` entry naming the implicated instrument; alert the operator</action>
    <exception>None — this rule has no exception. Per constitution `<anti_patterns>` "no learnable target encoder", target-side gradients are a CRITICAL bug class with no tolerated escape hatch</exception>
  </rule>

  <rule id="BR-PRED-02" req_ref="REQ-PRED-04">
    <condition>NaN or Inf appears in any predictor layer output</condition>
    <action>Abort the forward pass before returning; raise `MEJEPA_PRED_NAN_DETECTED`; the upstream caller (training loop or inference path) handles rollback per its own contract (training: `MEJEPA_OBSERVE_BATCH_NAN` rollback; inference: refuse with `MEJEPA_INFER_OOD_REFUSE` if recurring)</action>
    <exception>None</exception>
  </rule>

  <rule id="BR-PRED-03" req_ref="REQ-PRED-13,REQ-PRED-14">
    <condition>Steady-state VRAM resident bytes exceed 8 × 2³⁰ AND fall below 9 × 2³⁰</condition>
    <action>Continue (within budget); record the elevated reading in the FSV / training log so operator can monitor</action>
    <exception>If peak (not steady) crosses 8 GB during a single forward+backward, allow it provided the next steady-state poll returns under 8 GB; sustained operation above 8 GB triggers BR-PRED-04</exception>
  </rule>

  <rule id="BR-PRED-04" req_ref="REQ-PRED-14">
    <condition>Steady-state VRAM resident bytes ≥ 9 × 2³⁰</condition>
    <action>Pause training; raise `MEJEPA_PRED_VRAM_EXCEEDED`; surface options: reduce batch, enable gradient checkpointing, evict E15-E21</action>
    <exception>None — auto-eviction would silently degrade the embedder ensemble; the operator must choose</exception>
  </rule>

  <rule id="BR-PRED-05" req_ref="REQ-PRED-08">
    <condition>More than 50% of predictor output dims have per-dim std &lt; γ for two consecutive validation passes</condition>
    <action>Raise `MEJEPA_PRED_VICREG_DEGENERATE` (predictor-side collapse early warning); record in cert; surface to operator; consider reducing λ_var weighting on the next epoch via constitution-amendment process</action>
    <exception>Single-pass spike below threshold debounces — only consecutive passes trigger the flag</exception>
  </rule>

  <rule id="BR-PRED-06" req_ref="REQ-PRED-06">
    <condition>VICReg lambdas (λ_var, λ_cov, λ_inv) or γ are configured to non-default values</condition>
    <action>The configuration is rejected as a constitutional change unless the constitution `<utml_v1_invariants>` block has been amended via operator approval; until then, the defaults λ_var=25, λ_cov=1, λ_inv=25, γ=1 are immutable</action>
    <exception>FSV / dry-run binaries may explicitly set non-defaults to verify the formula's response surface; production training uses defaults only</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-PRED-01" req_ref="REQ-PRED-01">
    <scenario>The two input panels have inconsistent batch dimensions (e.g., panel_t0 has batch 64, panel_t1 has batch 32)</scenario>
    <expected_behavior>Forward fails closed with `MEJEPA_PRED_DIM_MISMATCH` reporting both observed batch sizes</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-PRED-02" req_ref="REQ-PRED-01">
    <scenario>An input panel contains the structural zero pattern from `panel[t=0]` (only structural slots populated; E_Diff/E_Trace/E_Witness partial/E_Oracle/E_CommitMsg are zero by SPEC-INSTRUMENTS construction)</scenario>
    <expected_behavior>Forward succeeds; the predictor learns to handle structural zeros as part of its training distribution; no special-casing is needed at this layer</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-PRED-03" req_ref="REQ-PRED-04">
    <scenario>The first forward pass on a freshly randomly-initialized predictor produces an Inf in attention softmax (numerical issue in scaled-dot-product before LayerNorm warms up)</scenario>
    <expected_behavior>Detect the Inf, raise `MEJEPA_PRED_NAN_DETECTED`; the operator's remediation is to verify weight init scale or enable attention-scaling stabilizers</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-PRED-04" req_ref="REQ-PRED-09,REQ-PRED-10">
    <scenario>An instrument's encode() internally uses a frozen embedder weight tensor that was loaded via `Tensor::detach()` but a downstream caller accidentally calls `.requires_grad_(true)` on the same allocation</scenario>
    <expected_behavior>Gradient hook detects the non-zero gradient on instrument tensor; `MEJEPA_PRED_FROZEN_TARGET_GRAD` fires; training step aborted; operator fixes at the trait level by removing the `.detach()` escape hatch</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-PRED-05" req_ref="REQ-PRED-06">
    <scenario>The covariance matrix computation `C(z) = (z - z.mean()).T @ (z - z.mean()) / (batch - 1)` has batch=1 (degenerate batch in a misconfigured dry-run)</scenario>
    <expected_behavior>L_covariance is well-defined as 0 (single-row batch has zero variance); a warning is logged that batch=1 makes VICReg meaningless; the dry-run completes but the FSV evidence flags it</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-PRED-06" req_ref="REQ-PRED-08">
    <scenario>A specific subset of dims (say E_Oracle's exception one-hot dims) consistently has near-zero variance across training because the corpus is class-imbalanced, BUT the predictor as a whole is healthy</scenario>
    <expected_behavior>The 50% threshold debounce of REQ-PRED-08 prevents this from firing as collapse — only when more than half of all 5,120 dims fall below γ does the flag trigger; per-slot variance is logged separately for diagnosis without false collapse alarms</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-PRED-07" req_ref="REQ-PRED-12">
    <scenario>`num_tests` differs across SWE-bench tasks (one task has 12 tests, another has 1,247). A single Oracle head trained on the union of all tests would have ~10⁵ output dims</scenario>
    <expected_behavior>The Oracle head is sized to the maximum test count + 1 across the corpus (per-instance subsetting at inference time selects the relevant logits). Total Oracle head parameter count = 5120 × (max_num_tests + 1) — bounded by the corpus's max test count, not by the union of all tests across all tasks</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-PRED-08" req_ref="REQ-PRED-13">
    <scenario>The dry-run command runs on a host with sm_120a unavailable (CPU-only or older GPU)</scenario>
    <expected_behavior>The command fails closed with `MEJEPA_PRED_VRAM_EXCEEDED` or a more specific "candle-cuda sm_120a unavailable" error; CPU fallback is NOT silently applied (per CLAUDE.md "no silent fallback" rule). Operator chooses to either provision RTX 5090 hardware or build a CPU-debug variant explicitly</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-PRED-09" req_ref="REQ-PRED-05">
    <scenario>Two forward passes on the same input give different bytes because the operator inadvertently left training-mode dropout on in the eval path</scenario>
    <expected_behavior>The determinism FSV check fails; the test reports `determinism: false`; the binary exits 2; the operator switches to eval mode</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-PRED-10" req_ref="REQ-PRED-02">
    <scenario>An operator wants to extend to 12 layers + batch 128 to test capacity headroom</scenario>
    <expected_behavior>Config flag `PredictorConfig { num_layers: 12, batch_size_at_load: 128, gradient_checkpointing: true, evict_e15_e21_when_loaded: true, ... }`; the loaded predictor occupies ≤ 8 GB steady-state with E15-E21 evicted; behavioral correctness verified by re-running the FSV dry-run with the extended config</expected_behavior>
    <priority>nice-to-have</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-PRED-01" http_code="400" req_ref="REQ-PRED-01,REQ-PRED-18">
    <condition>Input panels have wrong shape (not (batch, 5120)) or inconsistent batch dims between panel_t0 and panel_t1</condition>
    <user_message>"Predictor input dimensionality mismatch: expected panel shape (batch, 5120) for both inputs."</user_message>
    <internal_message>"MEJEPA_PRED_DIM_MISMATCH: panel_t0.shape={panel_t0.shape:?}, panel_t1.shape={panel_t1.shape:?}, expected_panel_dim={PANEL_DIM}"</internal_message>
    <recovery_action>Caller verifies input pipeline (SPEC-INSTRUMENTS panel materialization) produces 5120-d panels; do not silent-pad/truncate</recovery_action>
  </error>

  <error id="ERR-PRED-02" http_code="500" req_ref="REQ-PRED-04">
    <condition>NaN or Inf detected in any forward-pass intermediate tensor</condition>
    <user_message>"Predictor produced a non-finite tensor; aborting forward pass."</user_message>
    <internal_message>"MEJEPA_PRED_NAN_DETECTED: layer={layer_id}, tensor={tensor_name}, sample_idx={first_nan_idx}"</internal_message>
    <recovery_action>Training loop: rollback to prior checkpoint per `MEJEPA_OBSERVE_BATCH_NAN`. Inference: refuse with `MEJEPA_INFER_OOD_REFUSE` if recurring</recovery_action>
  </error>

  <error id="ERR-PRED-03" http_code="507" req_ref="REQ-PRED-14">
    <condition>Steady-state VRAM ≥ 9 × 2³⁰ bytes</condition>
    <user_message>"Predictor VRAM budget exceeded; pausing training."</user_message>
    <internal_message>"MEJEPA_PRED_VRAM_EXCEEDED: vram_resident_bytes={bytes}, threshold_bytes={9 * (1 &lt;&lt; 30)}, action_options=[reduce_batch, enable_grad_ckpt, evict_e15_e21]"</internal_message>
    <recovery_action>Operator selects mitigation; auto-eviction is NOT applied silently per BR-PRED-04</recovery_action>
  </error>

  <error id="ERR-PRED-04" http_code="500" req_ref="REQ-PRED-09,REQ-PRED-10">
    <condition>Gradient detected on a frozen instrument's underlying tensor (CRITICAL bug — target-side collapse-immunity violated)</condition>
    <user_message>"Frozen target adapter received a gradient; this is a critical architectural bug. Training step aborted."</user_message>
    <internal_message>"MEJEPA_PRED_FROZEN_TARGET_GRAD: instrument_id={instrument_id}, grad_norm={grad_norm:.6e}, threshold={1e-12}, fix_at=src/instruments/{instrument_id}.rs"</internal_message>
    <recovery_action>Operator fixes at the trait level — verify the instrument does not expose `&amp;Var`, `&amp;mut Tensor`, or detach-then-reattach paths; rebuild + re-run FSV gradient-hook adversarial test</recovery_action>
  </error>

  <error id="ERR-PRED-05" http_code="422" req_ref="REQ-PRED-08">
    <condition>More than 50% of predictor output dims have per-dim std &lt; γ for 2 consecutive validation passes (predictor-side collapse early warning)</condition>
    <user_message>"VICReg variance term flags potential predictor collapse."</user_message>
    <internal_message>"MEJEPA_PRED_VICREG_DEGENERATE: low_variance_dim_count={n}, total_dims={5120}, gamma={gamma}, consecutive_passes={2}"</internal_message>
    <recovery_action>Operator inspects the per-step training cert chain for the affected steps; consider increasing λ_var via constitutional amendment; verify the corpus has not regressed to a degenerate distribution</recovery_action>
  </error>

  <error id="ERR-PRED-06" http_code="400" req_ref="REQ-PRED-04,REQ-PRED-18">
    <condition>NaN/Inf in input tensor (upstream instrument bug)</condition>
    <user_message>"Predictor input contains non-finite values; cannot proceed."</user_message>
    <internal_message>"MEJEPA_PRED_NAN_DETECTED: source=input, panel={t0|t1}, dim_indices={[i, j, k]}"</internal_message>
    <recovery_action>Caller validates upstream instrument output; this is a SPEC-INSTRUMENTS contract violation, not a predictor bug</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="PredictedPanel">
    <field name="data" type="Tensor" required="true">
      <description>The predictor's output predicted panel</description>
      <constraints>shape == (batch, PANEL_DIM = 5120); dtype bf16 on candle-cuda; every element finite</constraints>
    </field>
    <field name="batch_size" type="usize" required="true">
      <description>The batch dimension; mirrors the input batch</description>
      <constraints>≥ 1</constraints>
    </field>
  </entity>

  <entity name="TargetPanel">
    <field name="data" type="Tensor" required="true">
      <description>The frozen target panel: the actual panel[t=2] read back from frozen instruments</description>
      <constraints>shape == (batch, PANEL_DIM); dtype bf16; finite; produced with no gradient flow into instruments</constraints>
    </field>
    <field name="provenance" type="TargetProvenance" required="true">
      <description>code_version + embedder_versions for byte-equality verification per REQ-PRED-11</description>
      <constraints>code_version is git SHA hex; embedder_versions maps E1..E21 → SHA-256 hex</constraints>
    </field>
  </entity>

  <entity name="OracleLogits">
    <field name="logits" type="Tensor" required="true">
      <description>Pre-softmax logits per test + 1 for exception</description>
      <constraints>shape == (batch, num_tests + 1); finite</constraints>
    </field>
    <field name="num_tests" type="usize" required="true">
      <description>Number of tests in the task being predicted; determined by the per-task SWE-bench harness output</description>
      <constraints>1 ≤ num_tests ≤ corpus_max_test_count (10,000 hard ceiling)</constraints>
    </field>
  </entity>

  <entity name="VicregLambdas">
    <field name="var" type="f32" required="true">
      <description>λ_var; weight on L_variance term</description>
      <constraints>default 25.0; immutable absent constitutional amendment</constraints>
    </field>
    <field name="cov" type="f32" required="true">
      <description>λ_cov; weight on L_covariance term</description>
      <constraints>default 1.0</constraints>
    </field>
    <field name="inv" type="f32" required="true">
      <description>λ_inv; weight on L_invariance term</description>
      <constraints>default 25.0</constraints>
    </field>
    <field name="gamma" type="f32" required="true">
      <description>γ; per-dim std floor for the variance term</description>
      <constraints>default 1.0</constraints>
    </field>
  </entity>

  <entity name="LossOutputs">
    <field name="l_predict" type="f32" required="true">
      <description>Huber loss δ=1 over the (predicted, target) residual</description>
      <constraints>finite; ≥ 0; consumed by SPEC-UTML compute_delta_p</constraints>
    </field>
    <field name="l_variance" type="f32" required="true">
      <description>VICReg variance term: mean over dims of max(0, γ − std(z_dim))</description>
      <constraints>finite; ≥ 0; ≤ γ</constraints>
    </field>
    <field name="l_covariance" type="f32" required="true">
      <description>VICReg covariance term: sum over off-diagonal C(z)² divided by panel_dim</description>
      <constraints>finite; ≥ 0</constraints>
    </field>
    <field name="l_invariance" type="f32" required="true">
      <description>MSE between context and target latents</description>
      <constraints>finite; ≥ 0</constraints>
    </field>
    <field name="l_total" type="f32" required="true">
      <description>l_predict + λ_var · l_variance + λ_cov · l_covariance + λ_inv · l_invariance</description>
      <constraints>finite; ≥ 0</constraints>
    </field>
  </entity>

  <entity name="PredictorConfig">
    <field name="num_layers" type="u8" required="true">
      <description>Transformer depth</description>
      <constraints>default 6; range [1, 12]; exceeding 12 triggers BR-PRED-04 VRAM check</constraints>
    </field>
    <field name="hidden_dim" type="u32" required="true">
      <description>Hidden dimension</description>
      <constraints>default 1024; only multiples of num_heads supported</constraints>
    </field>
    <field name="num_heads" type="u8" required="true">
      <description>Number of attention heads</description>
      <constraints>default 8</constraints>
    </field>
    <field name="activation" type="ActivationKind" required="true">
      <description>Activation function</description>
      <constraints>{ GeLU } per doc 09 §2.4 — no other variant supported in Phase 2</constraints>
    </field>
    <field name="gradient_checkpointing" type="bool" required="true">
      <description>Enable gradient checkpointing on transformer layers</description>
      <constraints>default false; opt-in for batch≥128 / depth≥9</constraints>
    </field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-PRED-01" category="performance">
    <description>Forward pass latency at batch 64 on RTX 5090 (candle-cuda, sm_120a, bf16)</description>
    <metric>p50 &lt; 50 ms over 100 warm calls; p99 &lt; 100 ms</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-02" category="performance">
    <description>VRAM steady-state at batch 64</description>
    <metric>≤ 8 × 2³⁰ bytes (8 GB)</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-03" category="performance">
    <description>Single-prediction end-to-end latency in inference (predictor.forward + oracle_head.predict_probabilities; instruments cached)</description>
    <metric>&lt; 100 ms (constitution `<performance_budgets>` `single_prediction_latency_p50`)</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-04" category="reliability">
    <description>Forward-pass determinism in eval mode</description>
    <metric>SHA-256 of two consecutive eval-mode forwards is byte-equal</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-05" category="reliability">
    <description>Loss finiteness on dry-run</description>
    <metric>L_total ∈ [10, 1000] on a single-batch dry-run with random-init weights; no NaN/Inf in any term</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-06" category="security">
    <description>Frozen-target gradient isolation (target-side structural collapse-immunity)</description>
    <metric>‖dL/dθ_instrument‖ &lt; 1e-12 for ALL instrument tensors during ALL training steps; zero exceptions; CRITICAL severity on violation</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-07" category="security">
    <description>Input boundary validation</description>
    <metric>100% of malformed inputs (dim wrong, NaN, Inf, dtype wrong) rejected with structured error; no silent truncation/padding/cast</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-PRED-08" category="reliability">
    <description>Predictor-side variance floor (collapse early-warning)</description>
    <metric>&lt; 50% of dims below γ on any healthy training run; threshold-crossing surfaces `MEJEPA_PRED_VICREG_DEGENERATE`</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-PRED-09" category="usability">
    <description>FSV evidence reproducibility</description>
    <metric>Re-running the FSV dry-run on the same hardware + same code_version produces byte-equal evidence files (modulo timestamps)</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>SPEC-INSTRUMENTS / context-graph-mejepa-instruments</name>
    <description>Phase 1 instrument trait + 5,120-d Panel + PanelBuilder + InstrumentSlot enum. The predictor reads panels through this contract; the FrozenTargetAdapter consumes the same instruments at panel[t=2]</description>
    <impact>Phase 2 cannot start until Phase 1 ships at least the deterministic skeletons of all 15 slots (currently shipped: E_Oracle, E_Witness, E_StaticAnalysis, Panel; pending: 12 of 15)</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-EMBEDDERS / context-graph-embeddings</name>
    <description>The 21-embedder ensemble (E1-E21) loads pinned weight digests; the FrozenTargetAdapter never trains them</description>
    <impact>Embedder weights must be loaded with `requires_grad = false` and `.detach()`'d before being held by any instrument; failure to do so causes `MEJEPA_PRED_FROZEN_TARGET_GRAD`</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-CORPUS / context-graph-mejepa-corpus</name>
    <description>The 8-category × 300-task mutation corpus + holdout split + Docker oracle bridge supply (panel[t=0..2], oracle_outcome) tuples for training</description>
    <impact>Phase 2 dry-run uses synthetic / pre-materialized panels; Phase 3 real training requires the full 2,400-example corpus from SPEC-CORPUS</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-UTML / utml-learning-formula</name>
    <description>UTML's compute_delta_p reads the predictor's L_predict scalar per training step</description>
    <impact>Public access to `LossOutputs.l_predict` is a contract; refactoring it private would break the SPEC-UTML technical spec</impact>
  </dependency>

  <dependency type="external">
    <name>candle (candle-core, candle-nn, candle-transformers v0.9.x)</name>
    <description>Pure-Rust ML framework; provides Tensor, Var, AdamW, and LayerNorm/Linear/MultiHeadAttention building blocks</description>
    <impact>If candle-cuda sm_120a support regresses, the predictor cannot run on RTX 5090; CPU fallback is forbidden by CLAUDE.md "no silent fallback"</impact>
  </dependency>

  <dependency type="external">
    <name>cudarc + CUDA 13.2 + sm_120a (RTX 5090)</name>
    <description>The GPU kernel substrate; bf16 throughput / VRAM polling depends on this stack</description>
    <impact>Without sm_120a, the predictor refuses to run; operator provisions hardware</impact>
  </dependency>

  <dependency type="internal">
    <name>RocksDB CF_MEJEPA_WEIGHT_BLOBS</name>
    <description>Persists predictor weights keyed by theta_sha for cross-session resume + Phase 4 inference</description>
    <impact>Phase 2 dry-run does not require this; Phase 3 training does</impact>
  </dependency>

</dependencies>

<out_of_scope>

  <item>The Phase 3 training driver itself (AdamW step loop + cosine LR schedule + per-step training cert emission + holdout eval scheduler) — handled by SPEC-MEJEPA-TRAIN and the `context-graph-mejepa-train` crate</item>
  <item>Per-step training certificate format + Merkle root + UTML L_step computation — defined in SPEC-UTML and `crates/context-graph-witness` cert.rs (shipped Tier 2)</item>
  <item>Conformal calibration + OOD scoring + RealityPrediction conformal interval — Phase 4 deliverable, separate spec</item>
  <item>EWC++ Fisher snapshots + drift detection + A/B/C retrain — Phase 5 deliverable, separate spec</item>
  <item>Constellation Gτ guard at inference + per-chunk-type localization — Phase 4b deliverable in SPEC-TCT (deferred)</item>
  <item>The MCP tool surface `mcp__cgreality__mejepa_predict` and `mejepa_verify` — Phase 7 deliverable</item>
  <item>Predictor's behavior on languages outside the 11-language matrix — handled upstream by SPEC-AST-CHUNKING `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE`</item>
  <item>Replacement of the 6-layer transformer with alternative architectures (RNN, MoE, etc.) — out of scope for Phase 2 per doc 09 §2.4 fixed architecture</item>
  <item>Distributed / multi-GPU training — operator hardware is single RTX 5090; no distributed plan</item>

</out_of_scope>

<test_plan>

  <test_case id="TC-PRED-01" type="unit" req_ref="REQ-PRED-01">
    <description>predictor.forward() with shape-correct inputs returns shape-correct output</description>
    <preconditions>Predictor instantiated with default config; bf16 inputs (4, 5120) on candle-cuda</preconditions>
    <inputs>panel_t0 = randn(4, 5120, bf16, cuda); panel_t1 = randn(4, 5120, bf16, cuda)</inputs>
    <expected_result>output.shape() == (4, 5120); every element finite</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-02" type="unit" req_ref="REQ-PRED-01,REQ-PRED-18">
    <description>predictor.forward() rejects shape-wrong input with MEJEPA_PRED_DIM_MISMATCH</description>
    <preconditions>Predictor instantiated; intentionally wrong-shape inputs</preconditions>
    <inputs>panel_t0 = randn(4, 4096); panel_t1 = randn(4, 5120)</inputs>
    <expected_result>Err(MEJEPA_PRED_DIM_MISMATCH); panel_t0_dim=4096 reported in error message</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-03" type="unit" req_ref="REQ-PRED-04">
    <description>predictor.forward() rejects NaN-containing input with MEJEPA_PRED_NAN_DETECTED</description>
    <preconditions>Predictor instantiated; one NaN injected into panel_t0[0, 0]</preconditions>
    <inputs>panel_t0 with single NaN; panel_t1 finite</inputs>
    <expected_result>Err(MEJEPA_PRED_NAN_DETECTED); source=input reported</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-04" type="unit" req_ref="REQ-PRED-05">
    <description>Two eval-mode forwards on the same input give byte-equal outputs</description>
    <preconditions>Predictor in eval mode; fixed input</preconditions>
    <inputs>panel_t0 = fixed_seed_randn(4, 5120); panel_t1 = fixed_seed_randn(4, 5120)</inputs>
    <expected_result>SHA-256(output_1) == SHA-256(output_2); determinism: true</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-PRED-05" type="unit" req_ref="REQ-PRED-06,REQ-PRED-07">
    <description>vicreg_loss returns the four-term decomposition</description>
    <preconditions>Predicted + target panels of shape (16, 5120)</preconditions>
    <inputs>predicted = randn(16, 5120); target = predicted + 0.01 * randn(16, 5120); lambdas = default</inputs>
    <expected_result>LossOutputs { l_predict, l_variance, l_covariance, l_invariance, l_total }; l_total == l_predict + 25*l_variance + 1*l_covariance + 25*l_invariance ± 1e-3; all five fields finite ≥ 0</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-06" type="unit" req_ref="REQ-PRED-06">
    <description>vicreg_loss collapse case: zero predicted batch surfaces variance term at γ</description>
    <preconditions>Predicted = zeros(16, 5120); target = randn(16, 5120)</preconditions>
    <inputs>predicted = zeros, target = randn; default lambdas</inputs>
    <expected_result>l_variance ≈ γ = 1.0; l_total dominated by 25 * l_variance = 25; no NaN</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-PRED-07" type="unit" req_ref="REQ-PRED-09">
    <description>FrozenTargetAdapter exposes zero trainable parameters</description>
    <preconditions>FrozenTargetAdapter instantiated with all 15 instruments</preconditions>
    <inputs>frozen_target_adapter</inputs>
    <expected_result>frozen_target_adapter.parameters().is_empty() == true; AdamW.parameters() does not include any adapter state</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-08" type="integration" req_ref="REQ-PRED-10">
    <description>Adversarial: synthetic registration of an instrument tensor with the optimizer triggers MEJEPA_PRED_FROZEN_TARGET_GRAD</description>
    <preconditions>Manually inject a Var tensor into the optimizer pool that aliases an instrument's weight</preconditions>
    <inputs>poisoned optimizer state; one batch step</inputs>
    <expected_result>backward() detects gradient on instrument; MEJEPA_PRED_FROZEN_TARGET_GRAD raised; optimizer.step() is NOT called; no training cert written</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-09" type="unit" req_ref="REQ-PRED-12">
    <description>OracleHead produces (batch, num_tests + 1) logits; softmax row-sums are 1.0</description>
    <preconditions>OracleHead with num_tests = 30; predicted_panel = randn(8, 5120)</preconditions>
    <inputs>predicted_panel</inputs>
    <expected_result>oracle_head.predict_logits(predicted_panel).shape() == (8, 31); softmax row sums == 1.0 ± 1e-5; all values ∈ [0, 1]</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-10" type="integration" req_ref="REQ-PRED-03,REQ-PRED-13,REQ-PRED-15">
    <description>Phase 2 DoD dry-run: forward latency, loss finiteness, VRAM budget</description>
    <preconditions>Fresh checkout on RTX 5090; sm_120a available; corpus's first 64 examples available</preconditions>
    <inputs>cargo run --release -p context-graph-mejepa -- train --dry-run --batches 1 --batch-size 64</inputs>
    <expected_result>exit 0; FSV files written + re-readable; forward_latency_ms_p50 &lt; 50; vram_resident_bytes &lt; 8 × 2³⁰; l_total ∈ [10, 1000]; no NaN/Inf</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-11" type="integration" req_ref="REQ-PRED-08">
    <description>Predictor-side collapse early warning: collapsed predictor for two passes triggers MEJEPA_PRED_VICREG_DEGENERATE</description>
    <preconditions>Synthetic predictor that returns zeros; run 2 validation batches</preconditions>
    <inputs>two batches with zero-output predictor</inputs>
    <expected_result>after second pass: MEJEPA_PRED_VICREG_DEGENERATE raised; low_variance_dim_count &gt; 0.5 * 5120</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-PRED-12" type="e2e" req_ref="REQ-PRED-15">
    <description>FSV harness: forward-pass-evidence.json + vicreg-loss-evidence.json round-trip</description>
    <preconditions>Phase 2 FSV harness binary `examples/predictor_fsv.rs`</preconditions>
    <inputs>cargo run --release -p context-graph-mejepa --example predictor_fsv</inputs>
    <expected_result>/tmp/contextgraph-mejepa-predictor-fsv/forward-pass-evidence.json written, mode 0o600, re-readable, dim/finite/latency assertions pass; same for vicreg-loss-evidence.json</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-13" type="unit" req_ref="REQ-PRED-11">
    <description>FrozenTargetAdapter determinism: encode same panel[t=2] twice → byte-equal</description>
    <preconditions>FrozenTargetAdapter with all 15 instruments, fixed embedder versions</preconditions>
    <inputs>panel_t2 input encoded twice</inputs>
    <expected_result>SHA-256(target_1) == SHA-256(target_2); determinism: true</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-PRED-14" type="integration" req_ref="REQ-PRED-14">
    <description>VRAM exceeded: synthetic batch 256 surfaces MEJEPA_PRED_VRAM_EXCEEDED</description>
    <preconditions>Predictor loaded; synthetic over-batch</preconditions>
    <inputs>batch=256; default config (no gradient checkpointing)</inputs>
    <expected_result>MEJEPA_PRED_VRAM_EXCEEDED raised at next steady-state poll; training paused</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-PRED-15" type="unit" req_ref="REQ-PRED-02">
    <description>Architecture summary: predictor reports 6 layers, hidden 1024, 8 heads, GeLU activations</description>
    <preconditions>Default predictor config</preconditions>
    <inputs>predictor.architecture_summary()</inputs>
    <expected_result>{ num_layers: 6, hidden_dim: 1024, num_heads: 8, activation: GeLU, residual: true, layer_norm: true }</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-PRED-16" type="integration" req_ref="REQ-PRED-17">
    <description>Gradient checkpointing on extended config (12 layers, batch 128) keeps VRAM under budget</description>
    <preconditions>Predictor config { num_layers: 12, gradient_checkpointing: true }; E15-E21 evicted</preconditions>
    <inputs>One forward+backward step at batch 128</inputs>
    <expected_result>VRAM steady-state still ≤ 8 × 2³⁰ bytes; latency budget relaxed (no longer required &lt; 50 ms)</expected_result>
    <priority>nice-to-have</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-PRED-01" status="open" assignee="operator">
    <text>Should the Oracle head's `num_tests + 1` ceiling be sized to the corpus-max test count or the union of tests across all 300 SWE-bench Lite tasks?</text>
    <context>EC-PRED-07 surfaces this: a single Oracle head trained on the union could grow to ~10⁵ output dims. The proposed answer is corpus-max + per-instance subsetting at inference time, but this needs confirmation.</context>
    <resolution>(pending)</resolution>
  </question>

  <question id="Q-PRED-02" status="open" assignee="operator">
    <text>For the gradient-hook (REQ-PRED-10), what is the canonical numerical-noise floor for "‖dL/dθ_instrument‖ == 0"?</text>
    <context>1e-12 is proposed as a conservative bf16 noise floor; in practice instruments use frozen weights, so this should be exactly 0 modulo numerical roundoff. Empirical calibration during Phase 2 FSV will confirm.</context>
    <resolution>(pending)</resolution>
  </question>

  <question id="Q-PRED-03" status="open" assignee="operator">
    <text>Does the Phase 2 dry-run binary `cargo run --release -p context-graph-mejepa -- train --dry-run` need its own in-tree `bin/` entrypoint, or can it be an `examples/predictor_fsv.rs`?</text>
    <context>Phase 2 DoD specifies a `bin` invocation; the existing FSV pattern uses `examples/`. Either works; the spec is silent. Suggest `bin/mejepa.rs` with a `train` subcommand that gates on `--dry-run`.</context>
    <resolution>(pending)</resolution>
  </question>

  <question id="Q-PRED-04" status="open" assignee="operator">
    <text>Should the FrozenTargetAdapter be a single `Box<dyn Instrument>`-wrapping struct or a thin `&amp;dyn Instrument` slice over the underlying `Panel`?</text>
    <context>The doc 11 §5.2 contract is "frozen externally-defined instruments". The simplest mapping is a vector of trait objects. Whether the predictor crate owns the FrozenTargetAdapter or borrows it from `context-graph-mejepa-instruments` is a small architecture choice for the technical spec.</context>
    <resolution>(pending — TECH-PREDICTOR proposes ownership in the predictor crate, with the underlying `Box<dyn Instrument>` instances loaded from the instruments crate)</resolution>
  </question>

</open_questions>

<glossary>

  <term name="Panel">A 5,120-d joint embedding produced by SPEC-INSTRUMENTS' 15-slot panel layout. Three panels per attempt: panel[t=0], panel[t=1], panel[t=2].</term>
  <term name="Predictor">The 6-layer transformer trained to map (panel[t=0], panel[t=1]) → predicted panel[t=2]. The only trainable component of ME-JEPA-Code.</term>
  <term name="FrozenTargetAdapter">The wrapper around the SPEC-INSTRUMENTS panel that produces target panels for training. Has zero trainable parameters by construction.</term>
  <term name="Oracle head">A Linear projection from the predicted panel to (num_tests + 1) logits; softmax → per-test pass probability + exception probability.</term>
  <term name="VICReg">Variance-Invariance-Covariance Regularization (Bardes et al. 2022). Loss decomposition: L_total = L_predict + λ_var·L_variance + λ_cov·L_covariance + λ_inv·L_invariance.</term>
  <term name="L_predict">The Huber-loss term over the (predicted, target) residual; consumed by SPEC-UTML compute_delta_p per training step.</term>
  <term name="γ (gamma)">VICReg per-dim std floor; default 1.0. The variance term penalizes dims whose std falls below γ.</term>
  <term name="Target-side collapse">A collapse pathology where the target encoder learns a trivial constant output. ME-JEPA-Code has zero target-side trainable parameters, so this is structurally impossible (per doc 11 §5.2).</term>
  <term name="Predictor-side collapse">A collapse pathology where the trainable predictor degenerates to a constant or low-rank output. Defended by VICReg + variance-floor diagnostic + Phase 4b constellation Gτ.</term>
  <term name="Frozen instrument">An Instrument trait impl that holds embedder weights with `requires_grad = false` and exposes no `&mut` weight-mutating methods. Per the constitution, breaking this is a CRITICAL bug.</term>
  <term name="bf16">Brain-float 16: 1 sign + 8 exponent + 7 mantissa bits. Native on RTX 5090 sm_120a; halves VRAM vs fp32 for weights/activations.</term>
  <term name="sm_120a">CUDA compute capability of the RTX 5090. Required by candle-cuda v0.9.x for native bf16 throughput.</term>
  <term name="theta_sha">SHA-256 hex of the predictor weight blob; key into CF_MEJEPA_WEIGHT_BLOBS.</term>

</glossary>

</functional_spec>
```
