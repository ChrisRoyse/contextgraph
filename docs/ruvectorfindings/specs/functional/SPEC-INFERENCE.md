# SPEC-INFERENCE — ME-JEPA-Code Inference, Conformal Calibration, OOD Detection

> **Level 2 — Functional Specification.** Defines WHAT the inference domain (Phase 4a) must do, not HOW. Implementation details live in `TECH-INFERENCE.md`. Conflicts with the constitution resolve to the constitution.

```xml
<functional_spec id="SPEC-INFERENCE" version="1.0">

<metadata>
  <title>ME-JEPA-Code Inference + Conformal Calibration + OOD Detection (`MeJepaCompiler::compile/verify`)</title>
  <status>draft</status>
  <owner>Phase 4a ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>      <!-- Phase 1 panel substrate -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>     <!-- 11-language chunker -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>        <!-- 21-embedder substrate -->
    <spec_ref>SPEC-CORPUS</spec_ref>           <!-- Phase 0 corpus + calibration split -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>        <!-- Phase 2 predictor (deferred; required dependency) -->
    <spec_ref>SPEC-TRAINING</spec_ref>         <!-- Phase 3 training loop (deferred; required dependency) -->
    <spec_ref>SPEC-TCT</spec_ref>              <!-- Phase 4b constellation Gτ guard (cross-cutting) -->
    <spec_ref>SPEC-WITNESS</spec_ref>          <!-- 73-byte SHAKE-256 chain (shipped) -->
    <spec_ref>SPEC-SELF-HEALING</spec_ref>     <!-- Phase 5 observe() (delegated) -->
    <spec_ref>SPEC-UTML</spec_ref>             <!-- ΔΩ × ΔΞ multiplicative gating of confidence -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §1.3 (compilation success criteria + RealityPrediction shape), §4.1 (compile/verify call shape), §4.2 (conformal calibration), §4.3 (OOD detection), §7 Phase 4 (DoD + verification command)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §B (calibrated_confidence fail-closed gating), §F (operator priorities #2, #6, #7)</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §12 (calibrated_confidence formula; reward integrand)</source>
    <source type="design">docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §3 (Gτ guard; cross-cutting with this spec via FailedGate::ConstellationGuardRejected)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy: MEJEPA_INFER_OOD_REFUSE, MEJEPA_INFER_DIM_MISMATCH, MEJEPA_INFER_INSTRUMENT_MISSING, MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES; performance_budgets single_prediction_latency_p50 &lt; 100 ms; ood_separation_target AUC &gt; 0.85; conformal_coverage_target ± 2%; anti_patterns: no silent OOD fallback, no `unwrap_or(0.5)` on confidence intervals)</source>
    <source type="memory">memory/decisions/2026-05-09--single-loop-claude-code-actor-mejepa-data-factory.md (verify() is THE primary inference path; no LLM-gate framing)</source>
  </source_documents>
</metadata>

<overview>
The inference domain is the user-facing surface of ME-JEPA-Code. It answers the three operator
questions per doc 09 §1.3 deterministically for any (`PatchBundle`, `TaskContext`) pair:
**Q1 does the claim exist** (witness-chain integrity + source-SHA pinning), **Q2 does it work**
(`predicted_oracle_pass` with conformal interval), **Q3 how does it impact reality**
(`predicted_side_effect_set` plus `predicted_runtime_trace_summary`). Two API entry points are
exposed: `MeJepaCompiler::compile()` returns the raw `RealityPrediction`; `MeJepaCompiler::verify()`
runs the seven-gate verification ladder and returns `VerifyVerdict::Approve` (no human review
needed) or `VerifyVerdict::EscalateToHuman { failed_gate, reality_prediction }`. The `observe()`
method is exposed on the same impl block for surface symmetry but its body is specified by
SPEC-SELF-HEALING (Phase 5); this spec specifies only its trait signature and error contract.

Three statistical machineries underlie the API. (1) **Conformal calibration** computes a
non-conformity threshold on the 10% calibration split such that the actual oracle outcome lands
inside the prediction set with probability 1 − α (default α = 0.10 → 90% coverage; empirical
coverage on the calibration split must measure 88–92%). (2) **Out-of-distribution detection**
flags inputs the predictor does not recognise via the residual norm
`ood_score(panel) = 1 − exp(−‖g_φ(panel) − f_ξ(panel)‖² / σ²)` with σ tuned so calibration examples
score under 0.30; any inference-time `ood_score > 0.5` short-circuits to refusal regardless of
other gates. (3) **ΔΩ × ΔΞ multiplicative gating of `calibrated_confidence`** per doc 11 §12
deflates the conformal interval whenever the predictor itself is in a degraded state (collapsed
latent or unstable optimizer), so the verdict fails-closed when training health is poor.

A patch passes `verify()` and earns `Approve` only when **all** of: (i) `out_of_distribution_score
< 0.5`; (ii) `predicted_oracle_pass > 0.95`; (iii) `confidence.outcome_set.len() ≤ 2` (sharp
prediction, low entropy); (iv) every cited skill's Granger p-value below the rejection threshold
(default 0.01, configurable per task class); (v) the witness-chain segment in `PatchBundle` replays
without break; (vi) the claimed pre/post source SHAs in `PatchBundle` match disk; (vii) the TCT
Gτ guard accepts the predicted panel (cross-cutting with SPEC-TCT). When any single gate fails,
`VerifyVerdict::EscalateToHuman` carries the `FailedGate` variant identifying exactly which gate
fired and the underlying `RealityPrediction` so the human reviewer can see what the model
predicted before deciding. This is the "no code review needed" North Star contract per doc 09 §1.3.

Beneficiaries: (1) the Phase 7 MCP tool `mcp__cgreality__mejepa_verify` exposes `verify()` to any
agent or human reviewer; (2) the Phase 5 self-healing loop reads `RealityPrediction.confidence`
when computing rolling-window empirical coverage for drift detection; (3) the outer reality loop
consumes `EscalateToHuman` to drive operator-attention work; (4) external integrations (CI bots,
PR review tooling) can call `verify()` over MCP without trusting the network, since OOD score and
witness-chain replay structurally protect against adversarial inputs.
</overview>

<user_types>

  <user_type id="UT-01" name="External AI agent (patch producer)">
    <description>Any AI agent (Claude Code in the outer reality loop, an external coding agent over MCP, or eventually ME-JEPA itself by gradient ascent) that proposes code patches and wants a verdict on "would this work in reality."</description>
    <permissions>READ-only over MCP: `mcp__cgreality__mejepa_verify(patch, context, [include_provenance])`; cannot mutate weights; cannot call `observe()` directly.</permissions>
    <goals>Receive `VerifyVerdict::Approve { reality_prediction, gates_passed: 7 }` for in-distribution sharp-confidence patches and an unambiguous `FailedGate` variant otherwise.</goals>
  </user_type>

  <user_type id="UT-02" name="Operator (human reviewer)">
    <description>The human running the system. Reviews escalated verdicts, signs off on calibration version bumps, decides whether OOD-flagged inputs justify expanding the corpus.</description>
    <permissions>READ all `RealityPrediction` fields including provenance bundle; READ FSV evidence at /tmp/contextgraph-mejepa-infer-fsv/; APPROVE calibration version promotions; APPROVE max-age overrides per SEC-MEJEPA-04; cannot silently mutate the conformal threshold or σ bandwidth.</permissions>
    <goals>Verify that `Approve` verdicts are statistically defensible (calibration coverage on calibration split within ±2 % of stated; OOD AUC &gt; 0.85 between in-distribution and synthetic OOD inputs).</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 7 mejepa-shift-subscriber">
    <description>Per doc 09 §3.8 the Tokio task that consumes shift-log + witness-chain entries. After every Claude Code edit/test cycle it materializes the post-execution panel and may call `verify()` to attach a verdict to the resulting training tuple.</description>
    <permissions>READ-only inference path; WRITE `RealityPrediction` records into CF_MEJEPA_LIVE_PREDICTIONS keyed by session_id + offset.</permissions>
    <goals>Latency budget: a single `verify()` call completes within the subscriber's overall &lt; 5 s p50 / &lt; 30 s p99 budget per shift event (instruments cached path).</goals>
  </user_type>

  <user_type id="UT-04" name="Phase 5 Self-Healing Loop">
    <description>The drift-detection + A/B/C retrain machinery. Reads the rolling 1,000-attempt window of `(predicted_set, actual_oracle)` pairs to compute empirical conformal coverage.</description>
    <permissions>READ `RealityPrediction.confidence.outcome_set`; READ `RealityPrediction.out_of_distribution_score`; cannot directly mutate weights through this surface (Phase 5 owns its own `observe()` body).</permissions>
    <goals>Detect coverage degradation within 1,000 observations of the rolling window per doc 09 §5.2.</goals>
  </user_type>

  <user_type id="UT-05" name="Phase 4b TCT Constellation Guard">
    <description>The cross-cutting Gτ guard machinery per doc 13 §3. Consumes the predicted panel from `compile()` and decides whether the per-(mutation_category × entity_type × language × embedder) cell centroids agree with the prediction. Specified in SPEC-TCT.</description>
    <permissions>READ predicted panel via internal API (not over MCP); RETURN `Approve` or `Reject{violating_embedders, violating_chunks}`. This spec specifies only the surface contract — `FailedGate::ConstellationGuardRejected` — that ties Gτ rejection into `verify()`.</permissions>
    <goals>Enforce per-chunk typological consistency without learning; deterministic O(N · d_max) on frozen instruments.</goals>
  </user_type>

  <user_type id="UT-06" name="Reality-Loop Optimizer (Claude Code, this agent)">
    <description>The outer-loop driver per CLAUDE.md §11.2 that produces the `PatchBundle.witness_chain_segment` and `PatchBundle.claim_graph` artifacts that enter `verify()`.</description>
    <permissions>WRITE `cgreality-shift-log` entries (consumed); WRITE recommendation JSONs (entering `claim_graph`); calls `verify()` only via MCP, never via direct in-process FFI.</permissions>
    <goals>Produce well-formed `PatchBundle`s with valid SHA-256 pre/post pins, well-formed witness segments, and well-formed claim graphs so the verify() ladder can run without spurious gate failures.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-INFER-01" priority="must-have">
    <narrative>
      <as_a>External AI agent (UT-01)</as_a>
      <i_want_to>Call `verify(patch, context)` over MCP and receive a single binary verdict ("safe to merge" or "human review needed") with the failing gate identified</i_want_to>
      <so_that>I can ship sharp in-distribution patches without a human in the loop, while every uncertain or out-of-distribution patch is correctly escalated</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-01-01">
        <given>A `PatchBundle` whose pre/post SHAs match disk, whose witness segment replays clean, and whose `panel[t=2]` predicted by `compile()` has `predicted_oracle_pass &gt; 0.95`, `confidence.outcome_set.len() ≤ 2`, `out_of_distribution_score &lt; 0.5`, all cited skills' Granger p-values below threshold, and Gτ guard returning Approve</given>
        <when>The agent invokes `mcp__cgreality__mejepa_verify(patch, context, include_provenance: false)`</when>
        <then>The response is `VerifyVerdict::Approve { reality_prediction, gates_passed: 7 }`; the embedded `RealityPrediction` carries the conformal set, OOD score, Granger p-values, and witness_chain_offset pinning the prediction to a chain entry</then>
      </criterion>
      <criterion id="AC-INFER-01-02">
        <given>A `PatchBundle` whose post-patch source SHA does not match the bytes on disk for the claimed file path</given>
        <when>The agent invokes `verify(...)`</when>
        <then>The response is `VerifyVerdict::EscalateToHuman { failed_gate: SourceShaDrift { path }, reality_prediction }`; no other gates are run after the SHA-pin gate fails (short-circuit)</then>
      </criterion>
      <criterion id="AC-INFER-01-03">
        <given>An input `panel` whose `ood_score` exceeds 0.5 (e.g., the agent submitted a Haskell patch and the corpus has not seen Haskell)</given>
        <when>The agent invokes `compile(...)` (NOT verify)</when>
        <then>`compile()` returns `Err(MejepaInferError::OodRefuse { score, panel_sha })` with `.code() == "MEJEPA_INFER_OOD_REFUSE"`; no silent fallback prediction is emitted; the partial panel materialization is logged with the input shapes and file paths per constitution §error_handling</then>
      </criterion>
      <criterion id="AC-INFER-01-04">
        <given>A well-formed `PatchBundle` and `TaskContext` for an in-distribution Python patch, instrument cache hit on all 15 panel slots</given>
        <when>The agent invokes `verify(...)` 100 times in sequence on the same bundle</when>
        <then>p50 end-to-end latency &lt; 100 ms; p99 &lt; 500 ms; every response is byte-equal (deterministic given frozen weights and cached panels)</then>
      </criterion>
      <criterion id="AC-INFER-01-05">
        <given>`include_provenance: true` in the MCP call</given>
        <when>The agent invokes `verify(...)`</when>
        <then>The response carries a provenance bundle: panel SHA-256, embedder versions per E1–E21, calibration_version (pointing at the active conformal threshold record), training_cert_chain_offset, constellation_version (Phase 4b), and code_version</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-02" priority="must-have">
    <narrative>
      <as_a>Operator (UT-02)</as_a>
      <i_want_to>Verify on the calibration split that empirical coverage stays within ±2 % of the stated 1 − α coverage and that OOD AUC between corpus and synthetic OOD inputs exceeds 0.85</i_want_to>
      <so_that>I can sign off on calibration before promoting it to production, per Phase 4 DoD</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-02-01">
        <given>A trained checkpoint and the 10 % calibration split (per SPEC-CORPUS, stratified by language)</given>
        <when>The operator runs `cargo run --release -p context-graph-mejepa -- infer-test --calibration ./data/mejepa-corpus/calibration --alpha 0.1`</when>
        <then>The CLI reports `stated_coverage: 0.90`, `empirical_coverage` in [0.88, 0.92], `prediction_latency_p50 &lt; 100 ms`, `ood_separation_auc &gt; 0.85`; FSV evidence written to /tmp/contextgraph-mejepa-infer-fsv/{verify-evidence.json, conformal-calibration-evidence.json, ood-evidence.json} with mode 0o600 per SEC-MEJEPA-07</then>
      </criterion>
      <criterion id="AC-INFER-02-02">
        <given>Empirical coverage on the calibration split lands outside [0.88, 0.92]</given>
        <when>The CLI completes</when>
        <then>The CLI exits with non-zero status; emits `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` if N &lt; configured minimum, or `MEJEPA_INFER_CALIBRATION_STALE` if the calibration record is older than 30 days; recovery action is documented in stderr per constitution §error_handling</then>
      </criterion>
      <criterion id="AC-INFER-02-03">
        <given>The calibration set has fewer than the minimum required samples per language stratum (default 50 per language per mutation category)</given>
        <when>Calibration is requested</when>
        <then>Calibration refuses with `MEJEPA_CONFORMAL_INSUFFICIENT_SAMPLES`; falls back to the last-known calibration threshold loaded from CF_MEJEPA_CALIBRATION_HISTORY and surfaces `degraded` status on every prediction made under that threshold per doc 10 §D ("MUST NOT mean silent fallback")</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-03" priority="must-have">
    <narrative>
      <as_a>Phase 7 mejepa-shift-subscriber (UT-03)</as_a>
      <i_want_to>Call `verify()` for every shift event so the resulting `RealityPrediction` is durably attached to the training tuple</i_want_to>
      <so_that>The Phase 5 drift-detection rolling window has populated `(predicted_set, actual_oracle)` pairs to compute empirical coverage</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-03-01">
        <given>A shift-log entry consumed by the subscriber; the corresponding panels at t=0/t=1 already in the instrument cache</given>
        <when>The subscriber calls `verify(...)`</when>
        <then>End-to-end latency p50 &lt; 100 ms (instruments cached); the resulting `RealityPrediction` is written to CF_MEJEPA_LIVE_PREDICTIONS keyed by `(session_id, witness_chain_offset)` for replay</then>
      </criterion>
      <criterion id="AC-INFER-03-02">
        <given>The witness chain at the offset embedded in `PatchBundle.witness_chain_segment` fails the integrity replay (corrupted hash chain)</given>
        <when>`verify(...)` runs</when>
        <then>Returns `VerifyVerdict::EscalateToHuman { failed_gate: WitnessChainBroken, reality_prediction }`; emits a journal entry to memory/blockers/ with severity:high per CLAUDE.md §6 SMAP rules</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-04" priority="must-have">
    <narrative>
      <as_a>Phase 5 self-healing loop (UT-04)</as_a>
      <i_want_to>Read `RealityPrediction.confidence.outcome_set` and `out_of_distribution_score` from the rolling 1,000-attempt window</i_want_to>
      <so_that>I can compute empirical conformal coverage and trigger drift severity transitions per doc 09 §5.2</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-04-01">
        <given>1,000 prior `RealityPrediction` records in CF_MEJEPA_LIVE_PREDICTIONS each with `outcome_set` and the corresponding actual oracle outcome from CF_MEJEPA_ORACLE_VERDICTS</given>
        <when>The self-healing loop reads the window</when>
        <then>Each `RealityPrediction` exposes `outcome_set`, `alpha`, and `out_of_distribution_score` as plain f32/Vec fields with no hidden state; the structure is byte-stable across schema versions or schema migration is gated by a constitution change per anti_patterns BackwardsCompat exception</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-05" priority="must-have">
    <narrative>
      <as_a>Phase 4b TCT Constellation Guard (UT-05)</as_a>
      <i_want_to>Have `verify()` consult the Gτ guard on the predicted panel and surface `FailedGate::ConstellationGuardRejected { violating_embedders, violating_chunks }` when the guard rejects</i_want_to>
      <so_that>Per-chunk typological inconsistencies (e.g., a class's E_AST distribution far from any known centroid) are caught before `Approve`</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-05-01">
        <given>A predicted panel whose E_Diff slot's distance to every constructed centroid exceeds the d_max threshold for the (mutation_category, entity_type, language) cell</given>
        <when>`verify(...)` runs Gate 7 (TCT guard)</when>
        <then>Returns `VerifyVerdict::EscalateToHuman { failed_gate: ConstellationGuardRejected { violating_embedders: vec![EmbedderId::E7], violating_chunks: vec![ChunkId(...)] }, reality_prediction }`</then>
      </criterion>
      <criterion id="AC-INFER-05-02">
        <given>The active constellation_version is older than `max_age` (default 90 days per SEC-MEJEPA-04) and no operator override is recorded</given>
        <when>`verify(...)` is called</when>
        <then>Returns `Err(MejepaInferError::GtauStaleConstellation)` with `.code() == "MEJEPA_INFER_GTAU_STALE_CONSTELLATION"`; do not silently fall back to a stale guard</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-06" priority="must-have">
    <narrative>
      <as_a>Operator (UT-02)</as_a>
      <i_want_to>The conformal interval reported in `RealityPrediction.confidence` to be multiplicatively deflated by ΔΩ_mean × ΔΞ_mean per doc 11 §12 whenever the predictor itself is in a degraded state</i_want_to>
      <so_that>Confidence fails-closed when training health is poor (collapsed latent or unstable optimizer), even if the raw conformal coverage looks fine</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-06-01">
        <given>A trained checkpoint whose runtime-observed `ΔΩ_mean = 0.30` and `ΔΞ_mean = 0.40` (both under the 0.4 health floor)</given>
        <when>`compile(...)` is invoked</when>
        <then>The reported `calibrated_confidence` (used to dilate the conformal outcome_set) is multiplied by `ΔΩ_mean · ΔΞ_mean = 0.12` and clamped to [0.10, 0.95] per doc 11 §12; the resulting `outcome_set` is wider than the raw conformal set; the `RealityPrediction.degraded_status` flag is true</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-INFER-07" priority="should-have">
    <narrative>
      <as_a>External AI agent (UT-01)</as_a>
      <i_want_to>Receive a `MejepaInferError::InstrumentMissing { instrument, language }` error rather than a silent zero-vector when an instrument is unavailable for the input language (e.g., Pyright not installed for Python)</i_want_to>
      <so_that>I do not consume confidently wrong predictions when the instrument substrate is incomplete</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-INFER-07-01">
        <given>A Python `PatchBundle` and a deployment where `pyright` is not on PATH</given>
        <when>`compile(...)` requests E_TypeGraph slot encoding</when>
        <then>Returns `Err(MejepaInferError::InstrumentMissing { instrument: "E_TypeGraph", language: Python })` with `.code() == "MEJEPA_INFER_INSTRUMENT_MISSING"`; no zero-padded fallback; logs the missing toolchain path before returning</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-INFER-01" story_ref="US-INFER-01" priority="must">
    <description>The `MeJepaCompiler` impl block exposes three public methods with the exact signatures: `compile(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext) -&gt; Result&lt;RealityPrediction, MejepaInferError&gt;`, `verify(&amp;self, patch: &amp;PatchBundle, context: &amp;TaskContext) -&gt; Result&lt;VerifyVerdict, MejepaInferError&gt;`, and `observe(&amp;mut self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome) -&gt; Result&lt;(), MejepaInferError&gt;`. The `observe()` body is specified in SPEC-SELF-HEALING; this spec specifies only its trait signature and error contract.</description>
    <rationale>Per doc 09 §4.1 the surface is fixed; per doc 10 §F priority #7 the `verify()` method is the primary inference path; per Phase 4 DoD it must be MCP-callable.</rationale>
    <constraints>No `&amp;mut self` on `compile`/`verify` (they are pure inference paths and must be safely shareable across Tokio tasks). `observe()` requires `&amp;mut self` (online weight update). All three return `Result&lt;_, MejepaInferError&gt;` whose variants have `.code()` SCREAMING_SNAKE tags per constitution §coding_standards.</constraints>
    <verification_method>Unit tests assert signature stability; integration test calls all three methods through MCP shim and asserts the response shapes match the canonical schema.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-02" story_ref="US-INFER-01" priority="must">
    <description>The `RealityPrediction` struct carries exactly these fields: `predicted_oracle_pass: f32 ∈ [0,1]`, `predicted_failing_tests: Vec&lt;TestId&gt;`, `predicted_runtime_trace_summary: Vec&lt;f32&gt;` (latent), `predicted_side_effect_set: Vec&lt;SideEffect&gt;`, `confidence: ConformalSet { alpha: f32, outcome_set: Vec&lt;OracleOutcome&gt; }`, `out_of_distribution_score: f32 ∈ [0,1]`, `granger_attestations: HashMap&lt;SkillId, f32&gt;` (p-values per cited skill), `witness_chain_offset: u64`, plus runtime fields `degraded_status: bool`, `delta_omega_mean: f32`, `delta_xi_mean: f32` for ΔΩ × ΔΞ gating per doc 11 §12.</description>
    <rationale>Per doc 09 §1.3 the field shape is the contract; per doc 11 §12 the degraded-status fields are required to fail-closed when training health is poor.</rationale>
    <constraints>All floats must be finite at construction (`!is_nan() &amp;&amp; !is_infinite()`); `predicted_oracle_pass` and `out_of_distribution_score` must be clamped to [0, 1] at construction (constructor returns `Err(MejepaInferError::NanDetected)` if any field violates).</constraints>
    <verification_method>Unit test passes `f32::NAN` into each f32 field and asserts the constructor returns the appropriate error; FSV harness verify-evidence.json shows all-finite serialized output.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-03" story_ref="US-INFER-01" priority="must">
    <description>The `VerifyVerdict` enum has exactly two variants — `Approve { reality_prediction, gates_passed: u8 }` and `EscalateToHuman { failed_gate: FailedGate, reality_prediction }` — and `gates_passed` is in [0, 7] reflecting how many of the 7 gates passed before short-circuit.</description>
    <rationale>Per doc 09 §4.1 the enum has exactly two variants; per the seven-gate composition (REQ-INFER-09 through REQ-INFER-15) `gates_passed` provides debug visibility.</rationale>
    <constraints>An `Approve` verdict requires `gates_passed == 7` (all gates passed). `EscalateToHuman` carries the underlying `RealityPrediction` so the human reviewer sees what the model predicted before deciding.</constraints>
    <verification_method>Unit test exhaustively constructs each `FailedGate` variant; FSV harness writes one example of each to verify-evidence.json.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-04" story_ref="US-INFER-01" priority="must">
    <description>The `FailedGate` enum carries seven variants: `OutOfDistribution { score: f32 }`, `LowConfidence { entropy: f32 }`, `PredictedFailure { failing_tests: Vec&lt;TestId&gt; }`, `GrangerRejection { skill: SkillId, p_value: f32 }`, `WitnessChainBroken`, `SourceShaDrift { path: PathBuf }`, and `ConstellationGuardRejected { violating_embedders: Vec&lt;EmbedderId&gt;, violating_chunks: Vec&lt;ChunkId&gt; }`. The TCT variant is cross-cutting with SPEC-TCT.</description>
    <rationale>Per doc 09 §4.1 plus the pivot's seven-gate composition; the TCT variant is the integration surface with SPEC-TCT per US-INFER-05.</rationale>
    <constraints>`ConstellationGuardRejected.violating_chunks` localizes rejection to specific function/class/impl/module-decl, not the whole patch (per constitution tct_invariants rule 4); `GrangerRejection.p_value` is in [0, 1].</constraints>
    <verification_method>Unit test exhaustively pattern-matches each variant; ConstellationGuardRejected tested via integration with mocked Gτ guard until SPEC-TCT ships, then real Gτ.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-05" story_ref="US-INFER-01" priority="must">
    <description>The `PatchBundle` struct carries `pre_patch_sha: [u8; 32]`, `post_patch_sha: [u8; 32]`, `ast_diff: AstDiff`, `witness_chain_segment: WitnessSegment`, `claim_graph: ClaimGraph` (what the AI said it did). The `TaskContext` struct carries `task_id: TaskId`, `problem_statement: String`, `test_stack: Vec&lt;TestId&gt;`, `environment: TaskEnvironment`, `language: Language`.</description>
    <rationale>Per doc 09 §4.1 the inputs to `compile`/`verify` carry exactly these fields; the SHAs enable the SourceShaDrift gate; the witness segment enables the WitnessChainBroken gate; the claim graph enables Q1 (does what the AI said it did exist) per doc 10 three-questions.</rationale>
    <constraints>`Language` is one of the 11 supported languages per constitution &lt;tech_stack&gt; (rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php); any other language returns `Err(MejepaInferError::InstrumentMissing)`. SHAs are validated against disk in Gate 5 (SourceShaDrift).</constraints>
    <verification_method>Unit tests exercise each `Language` variant; FSV harness writes an example PatchBundle for each supported language.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-06" story_ref="US-INFER-02" priority="must">
    <description>Conformal calibration: on the 10 % calibration split (stratified by language per SPEC-CORPUS), per calibration example compute the predicted E_Oracle slice and the actual E_Oracle slice; compute the per-example non-conformity score `s = max_test |predicted_test_pass[t] − actual_test_pass[t]|`; sort calibration scores; pick the (1 − α) quantile as the conformal threshold τ. At inference time, the prediction set is `{outcomes : non_conformity_score(outcome) ≤ τ}`. Default α = 0.10 → coverage 90 %.</description>
    <rationale>Per doc 09 §4.2 verbatim. The non-conformity score is the per-test L_∞ residual, which is sharper than per-test L_1 averaging on multi-test tasks.</rationale>
    <constraints>The quantile picker uses `(⌈(N+1)(1−α)⌉ / N)`-quantile (the conservative split-conformal correction) so the marginal coverage guarantee `P(actual ∈ set) ≥ 1 − α` holds in the small-sample regime. Calibration record is persisted with `corpus_sha`, `embedder_versions`, `frozen_at` per SEC-MEJEPA-04 provenance rules.</constraints>
    <verification_method>FSV harness conformal-calibration-evidence.json measures empirical coverage on the calibration split itself (sanity check: must equal stated coverage by construction); integration test against held-out subset measures empirical coverage on out-of-fold examples and asserts within ±2 % of stated.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-07" story_ref="US-INFER-01" priority="must">
    <description>Out-of-distribution detection: compute `ood_score(panel) = 1 − exp(−‖g_φ(panel) − f_ξ(panel)‖² / σ²)` where `g_φ` is the predictor and `f_ξ` is the frozen-target reconstruction. Bandwidth σ² is tuned on the calibration set such that calibration examples score &lt; 0.30. At inference time, `ood_score &gt; 0.5` short-circuits to `EscalateToHuman { failed_gate: OutOfDistribution { score } }` when called via `verify()`, or `Err(MejepaInferError::OodRefuse)` when called via `compile()`. No silent fallback (constitution &lt;anti_patterns&gt;).</description>
    <rationale>Per doc 09 §4.3 verbatim. Critical for refusing to predict on novel libraries / language features the corpus did not cover.</rationale>
    <constraints>σ² is persisted in CF_MEJEPA_CALIBRATION_HISTORY alongside τ; refusing to load if older than `max_age` (default 30 days) per MEJEPA_INFER_CALIBRATION_STALE. AUC between in-distribution mutation corpus and synthetic OOD inputs (random AST diffs over unrelated repos) must be &gt; 0.85 per Phase 4 DoD.</constraints>
    <verification_method>FSV harness ood-evidence.json computes ROC AUC on a 50/50 mix of corpus panels (positive class: in-distribution) vs synthetic OOD panels (negative class: random AST diffs from a held-out repo); asserts AUC &gt; 0.85.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-08" story_ref="US-INFER-06" priority="must">
    <description>The conformal interval reported in `RealityPrediction.confidence` is multiplied by ΔΩ_mean × ΔΞ_mean per doc 11 §12, where `calibrated_confidence = raw_conf · conv_rate · strategy_agreement · evidence_factor · ΔΩ_mean · ΔΞ_mean` and `evidence_factor = 0.5 + 0.5 · min(1.0, num_paths / threshold)`, clamped to [0.10, 0.95]. When ΔΩ_mean &lt; 0.4 OR ΔΞ_mean &lt; 0.4 the `degraded_status` flag on `RealityPrediction` is set to true.</description>
    <rationale>Per doc 11 §12: "for ME-JEPA-Code, the conformal interval reported in `Verdict::Pass{confidence_interval}` is multiplied by runtime-observed ΔΩ_mean × ΔΞ_mean to fail-closed when model is in degraded state." Per doc 10 §B this is locked.</rationale>
    <constraints>Both ΔΩ_mean and ΔΞ_mean read from the most recent N=128-step window of training certificates (CF_MEJEPA_TRAIN_CERTS); fall-back to last-good values if window unavailable but emit `degraded_status=true`; never silently drop the multiplicative gate.</constraints>
    <verification_method>Unit test injects ΔΩ=0.3, ΔΞ=0.4 into the training-cert stream and asserts the resulting `outcome_set` widens proportionally; FSV evidence captures one degraded-state example.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-09" story_ref="US-INFER-01" priority="must">
    <description>Gate 1 — OutOfDistribution: `out_of_distribution_score &lt; 0.5`. Implemented per REQ-INFER-07. Short-circuits when violated.</description>
    <rationale>Per doc 09 §1.3 the four passing conditions and the seven-gate composition.</rationale>
    <verification_method>Unit test injects an OOD panel and asserts `failed_gate: OutOfDistribution { score: &gt; 0.5 }`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-10" story_ref="US-INFER-01" priority="must">
    <description>Gate 2 — PredictedFailure: `predicted_oracle_pass &gt; 0.95`. Tests with predicted pass probability below per-test threshold (default 0.80) are surfaced in `failing_tests`.</description>
    <rationale>Per doc 09 §1.3 second passing condition.</rationale>
    <verification_method>Unit test at `predicted_oracle_pass = 0.94` asserts `failed_gate: PredictedFailure { failing_tests: [...] }`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-11" story_ref="US-INFER-01" priority="must">
    <description>Gate 3 — LowConfidence: `confidence.outcome_set.len() ≤ 2`. Above 2 implies entropy is too high for the prediction to be sharp.</description>
    <rationale>Per doc 09 §1.3 third passing condition. Larger sets indicate the model is uncertain across multiple oracle outcomes.</rationale>
    <verification_method>Unit test with calibrated threshold producing |outcome_set| = 3 asserts `failed_gate: LowConfidence { entropy }`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-12" story_ref="US-INFER-01" priority="must">
    <description>Gate 4 — GrangerRejection: every cited skill in `claim_graph` has `granger_attestations[skill] ≤ p_threshold` (default 0.01, configurable via `mcp__cgreality__mejepa_verify`'s context). Citations are extracted from the `claim_graph` field of `PatchBundle`.</description>
    <rationale>Per doc 09 §1.3 fourth passing condition; Granger F-test machinery is shipped in context-graph-solver/granger.rs.</rationale>
    <constraints>p_threshold may be tightened (e.g., 0.001 for safety-critical tasks) via TaskContext; cannot be loosened above 0.05 per anti-overfit policy.</constraints>
    <verification_method>Integration test with synthetic claim graph citing a skill whose Granger p-value = 0.03 asserts `failed_gate: GrangerRejection { skill, p_value: 0.03 }`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-13" story_ref="US-INFER-03" priority="must">
    <description>Gate 5 — WitnessChainBroken: `PatchBundle.witness_chain_segment` replays clean against the persisted chain (`SHAKE-256` chain integrity verified end-to-end on the segment plus the prefix of the durable chain it references).</description>
    <rationale>Per doc 09 §1.3 plus §5.6 witness-chain integrity check; clean-room shipped in context-graph-witness.</rationale>
    <verification_method>Integration test corrupts a single byte in the segment and asserts `failed_gate: WitnessChainBroken`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-14" story_ref="US-INFER-01" priority="must">
    <description>Gate 6 — SourceShaDrift: every file referenced in `PatchBundle.ast_diff` has its claimed `pre_patch_sha` and `post_patch_sha` matching the SHA-256 of the bytes on disk at the path. Gate runs early to short-circuit before instrument re-evaluation.</description>
    <rationale>Per doc 10 three-questions Q1 ("does what the AI said it did EXIST"); SHA pinning is the cheapest gate so it runs first after OOD.</rationale>
    <constraints>Path canonicalization per SEC-MEJEPA-01 (no traversal `../`, no symlinks pointing outside repo root); reject malformed paths with `MEJEPA_INFER_SOURCE_SHA_DRIFT`.</constraints>
    <verification_method>Integration test asserts gate fires on (a) path drift (file moved), (b) content drift (bytes changed), (c) malformed path (path traversal); each yields `failed_gate: SourceShaDrift { path }`.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-15" story_ref="US-INFER-05" priority="must">
    <description>Gate 7 — ConstellationGuardRejected: the predicted panel is consulted against the active TCT constellation per SPEC-TCT; on rejection, the `FailedGate::ConstellationGuardRejected { violating_embedders, violating_chunks }` is returned, where `violating_chunks` localizes to specific function/class/impl/module-decl per constitution tct_invariants rule 4.</description>
    <rationale>Per doc 09 §1.3 fifth passing condition (cross-cutting); per SPEC-TCT this gate is the integration surface.</rationale>
    <constraints>This spec specifies only the surface contract; the Gτ test logic is owned by SPEC-TCT. If `constellation_version` is missing or stale (`max_age` per SEC-MEJEPA-04), `Err(MejepaInferError::GtauStaleConstellation)` returns rather than silently bypassing the gate.</constraints>
    <verification_method>Integration test mocks the TCT guard to reject; asserts the exact `FailedGate` variant carries `violating_embedders` and `violating_chunks` fields.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-16" story_ref="US-INFER-01" priority="must">
    <description>The MCP tool `mcp__cgreality__mejepa_verify(patch, context, [include_provenance: bool = false])` is the primary inference path per doc 10 §F priority #7. Parameters: `patch: PatchBundle`, `context: TaskContext`, `include_provenance: bool`. Response: `VerifyVerdict` plus optional provenance bundle (panel SHA-256, embedder versions, calibration version, training cert chain offset, constellation version, code_version).</description>
    <rationale>Per doc 10 §F priority #7: "Make `mcp__cgreality__mejepa_verify` the primary inference path so any agent or human can ask 'does this patch work in reality?' and get a calibrated answer."</rationale>
    <constraints>Tool name follows `mcp_tool_namespace_lock` per constitution. JSON schema enforces `additionalProperties: false` per the audit-9 lesson. The retired `mcp__cgreality__reality_run_attempt` MUST remain fail-closed with `CCREALITY_ENGINE_RETIRED` per constitution &lt;anti_patterns&gt;.</constraints>
    <verification_method>Integration test invokes the MCP tool over JSON-RPC and asserts (a) schema compliance, (b) round-trip equivalence with direct in-process call, (c) `include_provenance: true` returns the provenance bundle.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-17" story_ref="US-INFER-01" priority="must">
    <description>The fail-closed error taxonomy includes (per constitution &lt;failure_taxonomy&gt;): `MEJEPA_INFER_OOD_REFUSE`, `MEJEPA_INFER_DIM_MISMATCH`, `MEJEPA_INFER_INSTRUMENT_MISSING`, `MEJEPA_INFER_NAN_DETECTED`, `MEJEPA_INFER_CALIBRATION_STALE`, `MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES`, `MEJEPA_INFER_WITNESS_CHAIN_BROKEN`, `MEJEPA_INFER_SOURCE_SHA_DRIFT`, `MEJEPA_INFER_GTAU_STALE_CONSTELLATION`. Each variant carries a structured payload (path, score, dimensionality, etc.) and is logged with input shapes + file paths before returning.</description>
    <rationale>Per constitution &lt;error_handling&gt; "robust error logging" and &lt;failure_taxonomy&gt; verbatim; per doc 10 §D ("MUST NOT mean silent fallback").</rationale>
    <verification_method>Unit test for each variant asserts `.code()` returns the expected SCREAMING_SNAKE tag; FSV harness writes one example of each error case.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-18" story_ref="US-INFER-01" priority="must">
    <description>Performance budgets per constitution &lt;performance_budgets&gt;: `single_prediction_latency_p50 &lt; 100 ms` (instruments cached); `single_prediction_latency_p99 &lt; 500 ms` (cold instruments full re-eval); `conformal_coverage_target` empirical within ±2 % of stated 1−α; `ood_separation_target` AUC &gt; 0.85.</description>
    <rationale>Per Phase 4 DoD (doc 09 §7).</rationale>
    <verification_method>FSV harness runs 1,000 verify() calls in sequence and reports p50/p95/p99 latencies; calibration coverage measured on calibration split; OOD AUC measured per REQ-INFER-07.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-19" story_ref="US-INFER-04" priority="must">
    <description>Every `RealityPrediction` produced is durably persisted to CF_MEJEPA_LIVE_PREDICTIONS keyed by `(session_id, witness_chain_offset)` so the Phase 5 self-healing rolling window can read it.</description>
    <rationale>Per doc 09 §5.2 the rolling 1,000-attempt window requires persisted predictions; per the artifact layout in CLAUDE.md §11.13.</rationale>
    <constraints>Per-prediction JSON ≤ 16 KiB; older entries beyond the rolling-window budget are pruned by Phase 6 hygiene per SPEC-HYGIENE.</constraints>
    <verification_method>Integration test calls verify() N times and reads back the CF; asserts byte-equality and key uniqueness.</verification_method>
  </requirement>

  <requirement id="REQ-INFER-20" story_ref="US-INFER-02" priority="must">
    <description>Phase 4 DoD verification command: `cargo run --release -p context-graph-mejepa -- infer-test --calibration ./data/mejepa-corpus/calibration --alpha 0.1` reports `stated_coverage 0.90`, `empirical_coverage` in [0.88, 0.92], `prediction_latency_p50 &lt; 100 ms`, `ood_separation_auc &gt; 0.85`.</description>
    <rationale>Per doc 09 §7 Phase 4 DoD verbatim.</rationale>
    <verification_method>The command is itself the verification; FSV evidence at /tmp/contextgraph-mejepa-infer-fsv/ captures the output.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-INFER-01" req_ref="REQ-INFER-01">
    <condition>Any caller invokes `compile()` or `verify()` from outside the workspace</condition>
    <action>The call MUST go through the MCP tool surface (`mcp__cgreality__mejepa_verify`); direct in-process FFI is reserved for the workspace's own crates.</action>
    <exception>None. Network surface is exclusively the MCP server per CLAUDE.md §11.9.</exception>
  </rule>

  <rule id="BR-INFER-02" req_ref="REQ-INFER-07">
    <condition>OOD score exceeds 0.5 at inference time</condition>
    <action>`compile()` returns `Err(MejepaInferError::OodRefuse)`; `verify()` short-circuits Gate 1 to `EscalateToHuman { failed_gate: OutOfDistribution { score } }`.</action>
    <exception>None. There is no operator override that allows a confident prediction in OOD territory; the operator must instead expand the corpus.</exception>
  </rule>

  <rule id="BR-INFER-03" req_ref="REQ-INFER-08">
    <condition>ΔΩ_mean &lt; 0.4 OR ΔΞ_mean &lt; 0.4</condition>
    <action>`RealityPrediction.degraded_status = true`; the conformal `outcome_set` is widened per doc 11 §12 multiplicative gating; downstream consumers MUST treat the verdict as advisory.</action>
    <exception>None. Per doc 10 §D self-healing must not mean silent fallback; degraded predictions stay flagged.</exception>
  </rule>

  <rule id="BR-INFER-04" req_ref="REQ-INFER-06">
    <condition>Conformal calibration record older than `max_age` (default 30 days)</condition>
    <action>Inference returns `Err(MejepaInferError::CalibrationStale)` until operator runs continuous-calibration sliding window per doc 09 §5.5 or signs off on a fresh calibration.</action>
    <exception>SPEC-SELF-HEALING's continuous calibration auto-refreshes every 1,000 attempts; only deployments with idle traffic ever hit this rule in practice.</exception>
  </rule>

  <rule id="BR-INFER-05" req_ref="REQ-INFER-15">
    <condition>TCT constellation version older than `max_age` (default 90 days per SEC-MEJEPA-04) and no operator override is recorded</condition>
    <action>Return `Err(MejepaInferError::GtauStaleConstellation)` with `.code() == "MEJEPA_INFER_GTAU_STALE_CONSTELLATION"`; do not silently bypass Gate 7.</action>
    <exception>Operator may write `memory/decisions/&lt;agent&gt;--mejepa-constellation-staleness-override-&lt;timestamp&gt;.md` with locked status to extend max_age temporarily; the override is itself recorded in the witness chain.</exception>
  </rule>

  <rule id="BR-INFER-06" req_ref="REQ-INFER-16">
    <condition>An MCP caller invokes the retired `mcp__cgreality__reality_run_attempt` or any other retired pre-pivot tool</condition>
    <action>The MCP server returns `CCREALITY_ENGINE_RETIRED` with a fail-closed message per constitution &lt;mcp_tool_namespace_lock&gt;.</action>
    <exception>None. The retired tool MUST NOT be reintroduced as a compatibility shim.</exception>
  </rule>

  <rule id="BR-INFER-07" req_ref="REQ-INFER-04">
    <condition>Multiple gates fail simultaneously</condition>
    <action>The first gate to fire (in order: 1=OOD, 2=PredictedFailure, 3=LowConfidence, 4=GrangerRejection, 5=WitnessChainBroken, 6=SourceShaDrift, 7=ConstellationGuardRejected) is reported via `EscalateToHuman.failed_gate`; remaining gates are NOT evaluated (short-circuit).</action>
    <exception>None. Cheap gates (OOD, SHA pin) intentionally run first to avoid wasted compute on doomed inputs.</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-INFER-01" req_ref="REQ-INFER-02" priority="critical">
    <scenario>The predictor returns a NaN or Infinity in `predicted_oracle_pass`, `out_of_distribution_score`, or any `outcome_set` element.</scenario>
    <expected_behavior>`RealityPrediction::try_new` returns `Err(MejepaInferError::NanDetected { field, witness_chain_offset })`; the partial computation is logged with input shapes per &lt;error_handling&gt;; no truncation, no clamp-to-zero fallback.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-02" req_ref="REQ-INFER-05" priority="high">
    <scenario>`PatchBundle.ast_diff` references a path outside the repo root (e.g., `../../etc/passwd`).</scenario>
    <expected_behavior>Path canonicalization at the MCP boundary rejects with `MEJEPA_INFER_SOURCE_SHA_DRIFT` (categorized as a path-traversal violation per SEC-MEJEPA-01).</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-03" req_ref="REQ-INFER-06" priority="high">
    <scenario>The calibration split contains fewer than the minimum required samples per language stratum (e.g., a new language was just added).</scenario>
    <expected_behavior>Calibration refuses with `MEJEPA_CONFORMAL_INSUFFICIENT_SAMPLES`; falls back to last-known calibration if available, else hard-fails. Predictions made under fall-back surface `degraded_status = true`.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-04" req_ref="REQ-INFER-07" priority="high">
    <scenario>OOD score is exactly 0.5 (boundary).</scenario>
    <expected_behavior>The strict inequality `&gt; 0.5` is enforced (so 0.5 itself does not trigger refusal). Documented in the FSV evidence.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-05" req_ref="REQ-INFER-12" priority="medium">
    <scenario>`PatchBundle.claim_graph` is empty (the AI made no specific skill claims).</scenario>
    <expected_behavior>Gate 4 (GrangerRejection) trivially passes (no skills to attest); `granger_attestations` is empty; logged at debug level.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-06" req_ref="REQ-INFER-13" priority="high">
    <scenario>`PatchBundle.witness_chain_segment` length is not a multiple of `WITNESS_ENTRY_SIZE` (73 bytes).</scenario>
    <expected_behavior>Gate 5 (WitnessChainBroken) fires immediately; `failed_gate: WitnessChainBroken`; do not attempt parse-and-recover.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-07" req_ref="REQ-INFER-14" priority="medium">
    <scenario>A file referenced in `ast_diff` does not exist on disk (the agent claimed to edit a file that was never created).</scenario>
    <expected_behavior>Gate 6 (SourceShaDrift) fires with `path = &lt;missing path&gt;` and a structured error indicating "file not found" rather than a generic IO error; surfaces Q1 ("does what the AI said it did exist") cleanly to the operator.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-08" req_ref="REQ-INFER-15" priority="medium">
    <scenario>The TCT constellation has cells with `N &lt; 50` supporting examples (per constitution tct_invariants rule 5 Bayesian shrinkage).</scenario>
    <expected_behavior>The shrinkage to language-aggregate or entity-aggregate centroid is performed by SPEC-TCT (this spec is a passive consumer); `verify()` does not see the difference. Documented in the SPEC-TCT cross-reference.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-09" req_ref="REQ-INFER-08" priority="high">
    <scenario>No training certificates exist yet (Phase 3 has not run); ΔΩ_mean / ΔΞ_mean cannot be computed.</scenario>
    <expected_behavior>Set `degraded_status = true` and use ΔΩ_mean = ΔΞ_mean = 0.5 as the bootstrap default (yielding a clamped multiplier of 0.25); surface `MEJEPA_INFER_CALIBRATION_STALE` if no calibration record exists.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-10" req_ref="REQ-INFER-16" priority="medium">
    <scenario>An MCP caller passes `include_provenance: true` but the calibration_version or constellation_version is unloadable.</scenario>
    <expected_behavior>Return the provenance bundle with `Option::None` for unloadable components plus an explicit warning array `provenance_warnings: ["calibration_version unloadable", ...]`; do not silently drop the field.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-11" req_ref="REQ-INFER-05" priority="medium">
    <scenario>`PatchBundle` claims a `Language` not in the 11-language matrix (e.g., Haskell, OCaml).</scenario>
    <expected_behavior>Return `Err(MejepaInferError::InstrumentMissing { instrument: "&lt;all-language-dependent&gt;", language })`; recommend operator expand the corpus before attempting again.</expected_behavior>
  </edge_case>

  <edge_case id="EC-INFER-12" req_ref="REQ-INFER-18" priority="low">
    <scenario>1,000 sequential calls in a tight loop with `&amp;self`-only methods on a multi-Tokio-task setup.</scenario>
    <expected_behavior>No internal mutex contention on the inference path; latency p50 &lt; 100 ms holds; weights are immutable during inference.</expected_behavior>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-INFER-01" http_code="422" req_ref="REQ-INFER-07">
    <condition>OOD score &gt; 0.5</condition>
    <user_message>The patch contains code patterns the model has not seen during training; refusing to predict. Please expand the training corpus or escalate to human review.</user_message>
    <internal_message>MEJEPA_INFER_OOD_REFUSE: ood_score={score}, panel_sha={sha}, language={lang}</internal_message>
    <recovery_action>Operator: extend Phase 0 corpus to cover the language/library family; re-train; re-calibrate.</recovery_action>
  </error>

  <error id="ERR-INFER-02" http_code="400" req_ref="REQ-INFER-02">
    <condition>Panel dim mismatch — internal `f_target` returned a Vec whose length is not 5,120.</condition>
    <user_message>Internal panel construction failure; this is a bug. Please file an issue with the request id.</user_message>
    <internal_message>MEJEPA_INFER_DIM_MISMATCH: expected={5120}, actual={observed}, slot_layout_audit={...}</internal_message>
    <recovery_action>None silent. Halt; surface to operator.</recovery_action>
  </error>

  <error id="ERR-INFER-03" http_code="503" req_ref="REQ-INFER-05">
    <condition>An instrument is unavailable for the input language (e.g., pyright not installed for Python).</condition>
    <user_message>The model cannot evaluate this patch on the current deployment because a required toolchain is missing. Operator action required.</user_message>
    <internal_message>MEJEPA_INFER_INSTRUMENT_MISSING: instrument={E_TypeGraph}, language={Python}, expected_path={pyright}</internal_message>
    <recovery_action>Install missing toolchain; verify with `mejepa-instruments selftest --language={lang}`; retry.</recovery_action>
  </error>

  <error id="ERR-INFER-04" http_code="500" req_ref="REQ-INFER-02">
    <condition>NaN/Inf detected in any RealityPrediction f32 field at construction.</condition>
    <user_message>Internal numerical error; refusing to return potentially poisoned prediction.</user_message>
    <internal_message>MEJEPA_INFER_NAN_DETECTED: field={...}, witness_chain_offset={...}</internal_message>
    <recovery_action>Halt; rollback to last known-good checkpoint; surface to operator with the offending input panel SHA.</recovery_action>
  </error>

  <error id="ERR-INFER-05" http_code="503" req_ref="REQ-INFER-06">
    <condition>Calibration record older than max_age (default 30 days).</condition>
    <user_message>Conformal calibration is stale. Re-running calibration is required before inference can resume.</user_message>
    <internal_message>MEJEPA_INFER_CALIBRATION_STALE: calibration_version={...}, frozen_at={...}, max_age_days={30}</internal_message>
    <recovery_action>Run continuous-calibration sliding window per doc 09 §5.5; or operator runs `mejepa infer-test --calibration ...` to refresh.</recovery_action>
  </error>

  <error id="ERR-INFER-06" http_code="503" req_ref="REQ-INFER-06">
    <condition>Fewer than minimum samples in conformal calibration window.</condition>
    <user_message>Insufficient calibration samples for the requested language stratum; falling back to last-known threshold (degraded mode).</user_message>
    <internal_message>MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES: required={N}, observed={observed}, language={lang}</internal_message>
    <recovery_action>Operator: expand corpus for under-represented language; re-calibrate.</recovery_action>
  </error>

  <error id="ERR-INFER-07" http_code="409" req_ref="REQ-INFER-13">
    <condition>Witness chain replay broken at the segment boundary in PatchBundle.</condition>
    <user_message>Patch witness chain integrity check failed. The patch's claimed history is corrupted or tampered.</user_message>
    <internal_message>MEJEPA_INFER_WITNESS_CHAIN_BROKEN: offset={...}, expected_hash={...}, observed_hash={...}</internal_message>
    <recovery_action>Operator inspect the witness chain via `mcp__cgreality__witness_chain_inspect`; restore from last verified checkpoint.</recovery_action>
  </error>

  <error id="ERR-INFER-08" http_code="409" req_ref="REQ-INFER-14">
    <condition>Claimed pre-patch or post-patch SHA does not match disk.</condition>
    <user_message>The patch claims to modify {path}, but the bytes on disk do not match. Patch was generated against a different version of the file.</user_message>
    <internal_message>MEJEPA_INFER_SOURCE_SHA_DRIFT: path={path}, claimed_sha={...}, observed_sha={...}</internal_message>
    <recovery_action>Re-generate the patch against current HEAD; verify SHAs match before retrying.</recovery_action>
  </error>

  <error id="ERR-INFER-09" http_code="503" req_ref="REQ-INFER-15">
    <condition>Active TCT constellation is older than max_age and no operator override recorded.</condition>
    <user_message>TCT constellation is stale; refusing inference until refreshed.</user_message>
    <internal_message>MEJEPA_INFER_GTAU_STALE_CONSTELLATION: constellation_version={...}, frozen_at={...}, max_age_days={90}</internal_message>
    <recovery_action>Operator: refit constellation centroids on fresh deterministic-walk data per doc 13; or write override decision per BR-INFER-05 exception.</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="RealityPrediction">
    <field name="predicted_oracle_pass" type="f32" required="true">
      <description>Probability the patch will pass the SWE-bench Docker oracle, in [0, 1].</description>
      <constraints>Finite; not NaN; clamped to [0, 1] at construction.</constraints>
    </field>
    <field name="predicted_failing_tests" type="Vec&lt;TestId&gt;" required="true">
      <description>Subset of `TaskContext.test_stack` with predicted per-test pass probability below per-test threshold (default 0.80).</description>
      <constraints>Subset of `context.test_stack`; ordered by descending failure probability.</constraints>
    </field>
    <field name="predicted_runtime_trace_summary" type="Vec&lt;f32&gt;" required="true">
      <description>Latent summary of expected execution trace (panel slot E_Trace projection, length 32).</description>
      <constraints>All elements finite; length exactly 32 (E_Trace summary projection dim).</constraints>
    </field>
    <field name="predicted_side_effect_set" type="Vec&lt;SideEffect&gt;" required="true">
      <description>Set of predicted reality impacts (file modifications outside the patch, network effects, env-var changes); answers Q3 "how is it actually impacting reality".</description>
      <constraints>Each SideEffect carries a `category: enum {FileWrite, NetworkSend, EnvSet, ProcessSpawn, DbWrite}` and `severity: enum {Low, Medium, High}`.</constraints>
    </field>
    <field name="confidence" type="ConformalSet" required="true">
      <description>Calibrated outcome set (alpha + outcome_set).</description>
      <constraints>`alpha ∈ [0.01, 0.50]`; `outcome_set` non-empty; outcome elements distinct.</constraints>
    </field>
    <field name="out_of_distribution_score" type="f32" required="true">
      <description>OOD score per doc 09 §4.3, in [0, 1]. &gt; 0.5 → escalate.</description>
      <constraints>Finite; clamped to [0, 1].</constraints>
    </field>
    <field name="granger_attestations" type="HashMap&lt;SkillId, f32&gt;" required="true">
      <description>P-values per cited skill in `claim_graph`. Skills with `p_value ≤ p_threshold` are accepted.</description>
      <constraints>Each p-value finite, in [0, 1]; SkillId drawn from a closed enum.</constraints>
    </field>
    <field name="witness_chain_offset" type="u64" required="true">
      <description>Witness chain entry index that pins this prediction. Enables replay verification per doc 09 §5.6.</description>
      <constraints>Less than current chain length at construction.</constraints>
    </field>
    <field name="degraded_status" type="bool" required="true">
      <description>True when ΔΩ_mean &lt; 0.4 OR ΔΞ_mean &lt; 0.4 OR calibration is stale OR fall-back path was taken. Per doc 11 §12 multiplicative gating.</description>
    </field>
    <field name="delta_omega_mean" type="f32" required="true">
      <description>Most-recent N-step training ΔΩ mean. Used for confidence multiplicative gate.</description>
      <constraints>Finite; clamped to [0, 1].</constraints>
    </field>
    <field name="delta_xi_mean" type="f32" required="true">
      <description>Most-recent N-step training ΔΞ mean. Used for confidence multiplicative gate.</description>
      <constraints>Finite; clamped to [0, 1].</constraints>
    </field>
  </entity>

  <entity name="ConformalSet">
    <field name="alpha" type="f32" required="true">
      <description>Stated coverage parameter; coverage = 1 − α.</description>
      <constraints>In [0.01, 0.50]; default 0.10.</constraints>
    </field>
    <field name="outcome_set" type="Vec&lt;OracleOutcome&gt;" required="true">
      <description>Set of oracle outcomes the actual oracle is predicted to land in with probability 1 − α.</description>
      <constraints>Non-empty; all distinct; `len ≤ 2` for `Approve` (per Gate 3).</constraints>
    </field>
  </entity>

  <entity name="OracleOutcome">
    <field name="resolved" type="bool" required="true">
      <description>Whether SWE-bench oracle marked the instance resolved per swebench/harness/grading.py FAIL_TO_PASS / PASS_TO_PASS semantics.</description>
    </field>
    <field name="per_test_outcomes" type="Vec&lt;PerTestOutcome&gt;" required="true">
      <description>Per-test pass/fail/skip/error vector. Encoded into E_Oracle slot per SPEC-INSTRUMENTS.</description>
    </field>
    <field name="exception_class" type="Option&lt;ExceptionClass&gt;" required="false">
      <description>If execution crashed before tests started.</description>
    </field>
  </entity>

  <entity name="VerifyVerdict">
    <field name="variant" type="enum" required="true">
      <description>One of {Approve { reality_prediction, gates_passed }, EscalateToHuman { failed_gate, reality_prediction }}.</description>
      <constraints>Approve requires gates_passed == 7.</constraints>
    </field>
  </entity>

  <entity name="FailedGate">
    <field name="variant" type="enum" required="true">
      <description>One of {OutOfDistribution { score }, LowConfidence { entropy }, PredictedFailure { failing_tests }, GrangerRejection { skill, p_value }, WitnessChainBroken, SourceShaDrift { path }, ConstellationGuardRejected { violating_embedders, violating_chunks }}.</description>
    </field>
  </entity>

  <entity name="PatchBundle">
    <field name="pre_patch_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the pre-patch source bytes for each file mentioned in ast_diff.</description>
    </field>
    <field name="post_patch_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the post-patch source bytes.</description>
    </field>
    <field name="ast_diff" type="AstDiff" required="true">
      <description>Per-file AST diff; node-level adds/removes/modifies.</description>
    </field>
    <field name="witness_chain_segment" type="WitnessSegment" required="true">
      <description>Slice of the witness chain bytes covering the patch's edit + test events.</description>
      <constraints>Length multiple of WITNESS_ENTRY_SIZE (73 bytes); replayable against the persisted chain.</constraints>
    </field>
    <field name="claim_graph" type="ClaimGraph" required="true">
      <description>What the AI agent said it did: a graph of (skill cited, file touched, expected outcome) tuples. Drives Gate 4 GrangerRejection.</description>
    </field>
  </entity>

  <entity name="TaskContext">
    <field name="task_id" type="TaskId" required="true">
      <description>SWE-bench Lite task ID or synthesized analog.</description>
    </field>
    <field name="problem_statement" type="String" required="true">
      <description>Task description; encoded into E_Problem slot.</description>
      <constraints>UTF-8; bounded to 64 KiB; longer truncated with explicit warning.</constraints>
    </field>
    <field name="test_stack" type="Vec&lt;TestId&gt;" required="true">
      <description>Test IDs to evaluate against.</description>
    </field>
    <field name="environment" type="TaskEnvironment" required="true">
      <description>Docker image, env vars, language version.</description>
    </field>
    <field name="language" type="Language" required="true">
      <description>One of the 11 supported languages.</description>
    </field>
  </entity>

  <entity name="ProvenanceBundle">
    <field name="panel_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the materialized panel bytes.</description>
    </field>
    <field name="embedder_versions" type="HashMap&lt;EmbedderId, String&gt;" required="true">
      <description>Pinned versions for each E1-E21 embedder per SEC-MEJEPA-02.</description>
    </field>
    <field name="calibration_version" type="String" required="true">
      <description>Conformal calibration record id (CF_MEJEPA_CALIBRATION_HISTORY key).</description>
    </field>
    <field name="training_cert_chain_offset" type="u64" required="true">
      <description>Pointer into CF_MEJEPA_TRAIN_CERTS at the most recent training cert that fed ΔΩ_mean / ΔΞ_mean.</description>
    </field>
    <field name="constellation_version" type="String" required="true">
      <description>Active TCT constellation id (CF_MEJEPA_CONSTELLATION key).</description>
    </field>
    <field name="code_version" type="String" required="true">
      <description>Repo SHA + workspace tag (`git describe --always --dirty`).</description>
    </field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-INFER-01" category="performance">
    <description>Single-prediction latency p50 &lt; 100 ms with cached instruments.</description>
    <metric>p50 latency on 1,000-call FSV stress test &lt; 100 ms.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-02" category="performance">
    <description>Single-prediction latency p99 &lt; 500 ms cold (full instrument re-eval).</description>
    <metric>p99 latency on 1,000-call FSV stress test &lt; 500 ms.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-03" category="reliability">
    <description>Calibration coverage empirical within ±2 % of stated 1−α.</description>
    <metric>Empirical coverage on calibration split lands in [0.88, 0.92] for α=0.10.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-04" category="reliability">
    <description>OOD AUC &gt; 0.85 between in-distribution mutation corpus and synthetic OOD inputs (random AST diffs over unrelated repos).</description>
    <metric>ROC AUC computed by FSV harness ood-evidence.json &gt; 0.85.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-05" category="security">
    <description>FSV evidence files at /tmp/contextgraph-mejepa-infer-fsv/ are mode 0o600 per SEC-MEJEPA-07.</description>
    <metric>`stat -c '%a'` on every FSV file == 600.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-06" category="security">
    <description>MCP tool input validation rejects malformed paths (path traversal `../`), control characters, non-UTF8 problem_statement.</description>
    <metric>100 % of malformed inputs in fuzz suite rejected with structured errors per SEC-MEJEPA-01.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-07" category="usability">
    <description>Every error variant carries a structured payload (path, score, dimensionality) plus a SCREAMING_SNAKE `.code()` tag; logged with input shapes and file paths before returning per &lt;error_handling&gt;.</description>
    <metric>Code review confirms no `unwrap_or(0.5)`, no silent fallback paths.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-08" category="reliability">
    <description>Inference is deterministic: same input + same frozen weights + same code_version + same calibration version + same constellation version = byte-equal RealityPrediction.</description>
    <metric>10× repeated `verify()` calls on same input produce byte-equal serialized verdicts.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-INFER-09" category="reliability">
    <description>Inference is `&amp;self`-only (no internal mutation on the verify path); safe to call from N concurrent Tokio tasks.</description>
    <metric>Concurrent stress test (16 concurrent verify() calls) completes without lock contention; total throughput scales linearly.</metric>
    <priority>should</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>SPEC-INSTRUMENTS / TECH-INSTRUMENTS</name>
    <description>The 5,120-d Panel substrate. `compile()` materializes panels by calling the Instrument trait.</description>
    <impact>Without complete instruments, `compile()` fails closed with `MEJEPA_INFER_INSTRUMENT_MISSING`. Phase 1 must be at least partially complete before Phase 4 ships.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PREDICTOR / TECH-PREDICTOR (Phase 2)</name>
    <description>The trained predictor `g_φ` and frozen target `f_ξ`. Required for the OOD residual norm and for `compile()`'s forward pass.</description>
    <impact>Cannot ship inference without a trained predictor.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-TRAINING / TECH-TRAINING (Phase 3)</name>
    <description>Per-step training certificates feed ΔΩ_mean / ΔΞ_mean for the multiplicative confidence gate.</description>
    <impact>Without trained checkpoints + cert stream, ΔΩ/ΔΞ default to bootstrap values and `degraded_status = true`.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-CORPUS / TECH-CORPUS (Phase 0)</name>
    <description>The 10 % calibration split, stratified by language per doc 10 §C.</description>
    <impact>Without sufficient samples per language, `MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES`.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-TCT / TECH-TCT (Phase 4b, cross-cutting)</name>
    <description>Gate 7 ConstellationGuardRejected. This spec specifies only the surface contract; SPEC-TCT owns the Gτ logic.</description>
    <impact>Without TCT shipped, Gate 7 is mocked-pass; verify() emits a warning that ConstellationGuard is unavailable until SPEC-TCT lands.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-WITNESS</name>
    <description>Witness-chain replay for Gate 5; shipped clean-room in context-graph-witness.</description>
    <impact>Hard dependency; chain integrity is the substrate.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-SELF-HEALING (Phase 5, delegated)</name>
    <description>The body of `observe()`. This spec exposes its trait signature only.</description>
    <impact>Without Phase 5, `observe()` is a no-op stub; predictions stay frozen at initial calibration.</impact>
  </dependency>

  <dependency type="external">
    <name>swebench/harness/grading.py</name>
    <description>OracleOutcome semantics (FAIL_TO_PASS / PASS_TO_PASS / FAIL_TO_FAIL / PASS_TO_FAIL) per the upstream evaluator.</description>
    <impact>Read-only reference; never modified per &lt;anti_patterns&gt; reality-boundary rule.</impact>
  </dependency>

  <dependency type="external">
    <name>tree-sitter grammars (11 languages)</name>
    <description>Used by the AST chunker that feeds instruments.</description>
    <impact>Without the grammar for the patch's language, `MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE` propagates up as `MEJEPA_INFER_INSTRUMENT_MISSING`.</impact>
  </dependency>

  <dependency type="external">
    <name>candle 0.9.x + cudarc</name>
    <description>Inference forward pass on RTX 5090 sm_120a.</description>
    <impact>Without GPU available, fall back to CPU inference (latency budgets break; surface degraded_status).</impact>
  </dependency>

</dependencies>

<out_of_scope>
  <item>Body of `MeJepaCompiler::observe()` (online learning + EWC++ + drift detection) — handled by SPEC-SELF-HEALING (Phase 5). This spec specifies only the trait signature.</item>
  <item>Per-step training certificate generation (`L_step = ΔP × ΔK × ΔΩ × ΔΞ` accounting) — handled by SPEC-TRAINING (Phase 3) and SPEC-UTML.</item>
  <item>TCT constellation centroid construction + Gτ test math — handled by SPEC-TCT (Phase 4b). This spec specifies only the surface contract `FailedGate::ConstellationGuardRejected`.</item>
  <item>Tier compression of stored RealityPrediction records — handled by SPEC-HYGIENE (Phase 6) per CF_MEJEPA_LIVE_PREDICTIONS retention policy.</item>
  <item>The MCP shift-log subscriber that calls `verify()` per shift — handled by SPEC-SHIFT-SUBSCRIBER (Phase 7 §3.8).</item>
  <item>Active-learning loop that re-routes uncertain predictions to Docker oracle re-runs — handled by SPEC-SELF-HEALING (doc 09 §3.7.4).</item>
  <item>Predictor training from scratch — handled by SPEC-TRAINING.</item>
  <item>Cross-language calibration set construction — handled by SPEC-CORPUS (the calibration set is consumed here, not produced).</item>
  <item>Witness-chain entry creation — handled by SPEC-WITNESS; this spec only consumes (replays) chain entries.</item>
</out_of_scope>

<test_plan>

  <test_case id="TC-INFER-01" type="unit" req_ref="REQ-INFER-01">
    <description>API signature stability: `compile`, `verify`, `observe` exist with the documented signatures and return `Result&lt;_, MejepaInferError&gt;`.</description>
    <preconditions>Crate context-graph-mejepa builds.</preconditions>
    <inputs>None (signature check via type system).</inputs>
    <expected_result>Compile-time assertion via static `fn _assert_signatures()` confirms the impl block.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-02" type="unit" req_ref="REQ-INFER-02">
    <description>RealityPrediction NaN / Inf rejection at construction.</description>
    <preconditions>None.</preconditions>
    <inputs>predicted_oracle_pass = NaN | Inf | -1.0 | 1.5 | finite valid</inputs>
    <expected_result>Invalid: returns `Err(MejepaInferError::NanDetected { field })`. Valid: returns `Ok(_)`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-03" type="unit" req_ref="REQ-INFER-04">
    <description>Each FailedGate variant constructible and exhaustively pattern-matchable.</description>
    <preconditions>None.</preconditions>
    <inputs>One construction per variant.</inputs>
    <expected_result>Match arms exhaustive; each variant carries its documented payload.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-04" type="integration" req_ref="REQ-INFER-06">
    <description>Conformal calibration on the 10 % calibration split.</description>
    <preconditions>Trained predictor checkpoint loaded; calibration split available; α=0.10.</preconditions>
    <inputs>Calibration split (~240 examples for SWE-bench Lite).</inputs>
    <expected_result>Empirical coverage ∈ [0.88, 0.92]; FSV writes conformal-calibration-evidence.json.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-05" type="integration" req_ref="REQ-INFER-07">
    <description>OOD AUC between in-distribution corpus and synthetic OOD inputs (random AST diffs over unrelated repos).</description>
    <preconditions>Trained checkpoint; calibration σ tuned.</preconditions>
    <inputs>50 in-distribution panels + 50 OOD panels.</inputs>
    <expected_result>ROC AUC &gt; 0.85; FSV writes ood-evidence.json with the ROC curve points.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-06" type="integration" req_ref="REQ-INFER-09 REQ-INFER-10 REQ-INFER-11 REQ-INFER-12 REQ-INFER-13 REQ-INFER-14 REQ-INFER-15">
    <description>Seven-gate composition: synthetic inputs that fire each gate in turn.</description>
    <preconditions>Trained predictor + calibration + (mocked) constellation.</preconditions>
    <inputs>Seven crafted PatchBundle / TaskContext pairs, one per gate.</inputs>
    <expected_result>Each pair returns the expected `failed_gate` variant; gates_passed counter accurate; FSV writes verify-evidence.json with all seven examples.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-07" type="integration" req_ref="REQ-INFER-08">
    <description>ΔΩ × ΔΞ multiplicative gating widens outcome_set when training health is poor.</description>
    <preconditions>Trained checkpoint; CF_MEJEPA_TRAIN_CERTS populated with ΔΩ=0.30, ΔΞ=0.40 over last 128 steps.</preconditions>
    <inputs>One in-distribution PatchBundle.</inputs>
    <expected_result>`degraded_status = true`; `outcome_set` is wider than under healthy ΔΩ=ΔΞ=0.9; conformal interval reflects 0.12 multiplier clamped to 0.10.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INFER-08" type="integration" req_ref="REQ-INFER-16">
    <description>MCP tool round-trip: `mcp__cgreality__mejepa_verify` parity with direct in-process call.</description>
    <preconditions>MCP server running; test fixtures.</preconditions>
    <inputs>Same PatchBundle / TaskContext invoked both ways.</inputs>
    <expected_result>Byte-equal verdict; provenance bundle present when include_provenance: true.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-09" type="integration" req_ref="REQ-INFER-18 NFR-INFER-01 NFR-INFER-02">
    <description>Latency budget on 1,000 sequential verify() calls.</description>
    <preconditions>Instruments cached; trained predictor.</preconditions>
    <inputs>1,000 in-distribution PatchBundles.</inputs>
    <expected_result>p50 &lt; 100 ms; p99 &lt; 500 ms; FSV writes latency histogram.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INFER-10" type="integration" req_ref="REQ-INFER-19">
    <description>Live predictions persisted to CF_MEJEPA_LIVE_PREDICTIONS keyed by (session_id, witness_chain_offset).</description>
    <preconditions>RocksDB store opened; CF created.</preconditions>
    <inputs>10 verify() calls.</inputs>
    <expected_result>10 distinct keys; readback equality for each.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INFER-11" type="e2e" req_ref="REQ-INFER-20">
    <description>Phase 4 DoD verification command end-to-end.</description>
    <preconditions>Full Phase 0 corpus + Phase 3 trained checkpoint.</preconditions>
    <inputs>Calibration directory; α=0.10.</inputs>
    <expected_result>CLI reports stated_coverage 0.90; empirical ∈ [0.88, 0.92]; latency p50 &lt; 100 ms; OOD AUC &gt; 0.85; exits 0.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-12" type="unit" req_ref="REQ-INFER-17 ERR-INFER-01..ERR-INFER-09">
    <description>Each error variant carries SCREAMING_SNAKE `.code()` and structured payload.</description>
    <preconditions>None.</preconditions>
    <inputs>One instance per error variant.</inputs>
    <expected_result>`.code()` matches the canonical tag; payload populated; logged with input shape.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-13" type="integration" req_ref="EC-INFER-02 NFR-INFER-06">
    <description>Path-traversal rejection at MCP boundary.</description>
    <preconditions>MCP tool active.</preconditions>
    <inputs>PatchBundle with `ast_diff` referencing `../../etc/passwd`.</inputs>
    <expected_result>Returns `MEJEPA_INFER_SOURCE_SHA_DRIFT`; no file IO performed.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-14" type="integration" req_ref="EC-INFER-09">
    <description>Bootstrap-default ΔΩ/ΔΞ when no training certs exist.</description>
    <preconditions>CF_MEJEPA_TRAIN_CERTS empty.</preconditions>
    <inputs>verify() call.</inputs>
    <expected_result>`delta_omega_mean = delta_xi_mean = 0.5`; `degraded_status = true`; `MEJEPA_INFER_CALIBRATION_STALE` if no calibration record either.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-INFER-15" type="integration" req_ref="NFR-INFER-08">
    <description>Determinism: 10 sequential verify() calls on same input return byte-equal verdicts.</description>
    <preconditions>Frozen weights; pinned calibration / constellation versions.</preconditions>
    <inputs>10× same PatchBundle / TaskContext.</inputs>
    <expected_result>SHA-256 of serialized verdict equal across all 10.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-INFER-16" type="manual" req_ref="NFR-INFER-05">
    <description>FSV evidence files mode 0o600.</description>
    <preconditions>Run FSV harness.</preconditions>
    <inputs>None.</inputs>
    <expected_result>`stat -c '%a'` returns 600 for every file in /tmp/contextgraph-mejepa-infer-fsv/.</expected_result>
    <priority>medium</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-INFER-01" status="open" assignee="operator">
    <text>Should `p_threshold` for Granger rejection be tunable per-task-class, or globally fixed at 0.01?</text>
    <context>Per REQ-INFER-12, default is 0.01; allowing per-class tuning lets safety-critical tasks tighten to 0.001 but introduces a configuration surface.</context>
    <resolution>Default 0.01; opt-in tightening via TaskContext for safety-critical tasks; cannot be loosened above 0.05.</resolution>
  </question>

  <question id="Q-INFER-02" status="open" assignee="operator">
    <text>What is the minimum N for `MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES`?</text>
    <context>Per doc 10 §C, calibration must stratify by language. With 11 supported languages and 8 mutation categories, default 50 per cell yields 4,400 minimum which is feasible but tight on the 240-task calibration split.</context>
    <resolution>Default minimum 30 per (language × mutation_category) cell; below 30 invokes Bayesian shrinkage to language-aggregate or category-aggregate per constitution tct_invariants rule 5 (parallel TCT pattern).</resolution>
  </question>

  <question id="Q-INFER-03" status="open" assignee="operator">
    <text>Should Gate 7 (ConstellationGuardRejected) be opt-out for early-Phase-4 deployments before SPEC-TCT ships?</text>
    <context>SPEC-TCT is Phase 4b. If Phase 4a ships first, Gate 7 mocks-pass with a warning. Operator must accept that early deployments lack the constellation guarantee.</context>
    <resolution>Phase 4a ships with Gate 7 mocked-pass + a `provenance_warnings: ["constellation_unavailable"]` field; once Phase 4b ships, the gate becomes mandatory and the warning disappears. No silent flip.</resolution>
  </question>

  <question id="Q-INFER-04" status="open" assignee="operator">
    <text>Should `predicted_runtime_trace_summary` length (32) be a constitution constant or a per-version field?</text>
    <context>The 32-d projection is an arbitrary embedding dimension; if SPEC-INSTRUMENTS' E_Trace projection changes, this length must follow.</context>
    <resolution>Persist length in `ProvenanceBundle.embedder_versions[E_Trace]`; clients must check before deserializing.</resolution>
  </question>

</open_questions>

<glossary>
  <term name="PatchBundle">The input to compile/verify carrying pre/post SHAs, AST diff, witness segment, and claim graph. Per doc 09 §4.1.</term>
  <term name="TaskContext">The input to compile/verify carrying task_id, problem statement, test stack, environment, and language. Per doc 09 §4.1.</term>
  <term name="RealityPrediction">The output of compile() carrying predicted oracle pass probability, failing tests, runtime trace summary, side-effect set, conformal set, OOD score, Granger attestations, and witness chain offset. Per doc 09 §1.3.</term>
  <term name="VerifyVerdict">The output of verify(): two-variant enum {Approve, EscalateToHuman}. Per doc 09 §4.1.</term>
  <term name="FailedGate">Seven-variant enum identifying which gate fired in verify(). Per doc 09 §4.1 + this spec's REQ-INFER-04.</term>
  <term name="ConformalSet">Calibrated prediction set: { alpha: f32, outcome_set: Vec&lt;OracleOutcome&gt; }. Coverage = 1 − α; outcome_set built from the (1 − α)-quantile of calibration non-conformity scores.</term>
  <term name="Non-conformity score">`max_test |predicted_test_pass[t] − actual_test_pass[t]|`. The L_∞ residual per calibration example. Per doc 09 §4.2.</term>
  <term name="OOD score">`1 − exp(−‖g_φ(panel) − f_ξ(panel)‖² / σ²)`; in [0, 1]. Per doc 09 §4.3.</term>
  <term name="OracleOutcome">An element of `confidence.outcome_set`; carries `resolved: bool`, `per_test_outcomes`, `exception_class`. Per SPEC-INSTRUMENTS E_Oracle slot encoding.</term>
  <term name="Granger attestation">P-value from the Granger F-test (clean-room shipped in context-graph-solver/granger.rs) per cited skill. Used by Gate 4.</term>
  <term name="Skill">An entry in the claim_graph identifying what the AI said it did (e.g., "fix off-by-one", "add type annotation"). Closed enum drawn from a curated taxonomy.</term>
  <term name="Witness chain offset">u64 index into the durable 73-byte SHAKE-256 chain that pins this prediction. Per SPEC-WITNESS.</term>
  <term name="ΔΩ_mean / ΔΞ_mean">Most-recent-N-step training factors per doc 11 §1; ΔΩ = optimizer-state health, ΔΞ = representational non-collapse. Used to multiplicatively gate calibrated_confidence per doc 11 §12.</term>
  <term name="degraded_status">Boolean flag on RealityPrediction set true when ΔΩ_mean &lt; 0.4 OR ΔΞ_mean &lt; 0.4 OR calibration is stale. Per BR-INFER-03.</term>
  <term name="Calibration version">Identifier (CF_MEJEPA_CALIBRATION_HISTORY key) of the active conformal threshold τ + bandwidth σ. Per REQ-INFER-06.</term>
  <term name="Constellation version">Identifier (CF_MEJEPA_CONSTELLATION key) of the active TCT centroid set. Per SPEC-TCT.</term>
  <term name="Gate">One of 7 verification checks in verify(). In firing order: 1=OOD, 2=PredictedFailure, 3=LowConfidence, 4=GrangerRejection, 5=WitnessChainBroken, 6=SourceShaDrift, 7=ConstellationGuardRejected.</term>
  <term name="ProvenanceBundle">Optional payload returned when MCP caller passes include_provenance: true. Carries panel SHA, embedder versions, calibration version, training cert chain offset, constellation version, code version. Per REQ-INFER-16.</term>
</glossary>

</functional_spec>
```
