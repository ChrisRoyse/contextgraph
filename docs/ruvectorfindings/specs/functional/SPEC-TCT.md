# SPEC-TCT — Teleological Constellation Training + Gτ Guard

> **Level 2 — Functional Specification.** Defines WHAT the TCT-CONSTELLATION domain (Phase 4b cross-cutting) must do, not HOW. Implementation details live in `TECH-TCT.md`. Conflicts with the constitution resolve to the constitution. Conflicts with `SPEC-UTML.md` (which consumes `constellation_violation_rate` as a ΔΞ component) resolve in favor of this spec for **what** the rate is and how Gτ produces it; SPEC-UTML retains authority over how the rate is consumed inside the L_step formula.

```xml
<functional_spec id="SPEC-TCT" version="1.0">

<metadata>
  <title>Teleological Constellation Training + Gτ Guard (Phase 4b cross-cutting)</title>
  <status>draft</status>
  <owner>Phase 4b ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-09</last_updated>
  <related_specs>
    <spec_ref>SPEC-CORPUS</spec_ref>            <!-- Phase 0 mutation corpus + Docker oracle bridge; supplies labeled panel triples for centroid construction -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>      <!-- 11-language AST-perfect chunker; supplies (entity_type, language, parent_chain_hash, line_start, line_end, file_sha) per chunk -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>         <!-- 21 frozen embedders that serve all three TCT roles -->
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>       <!-- 5,120-d Panel + 15-slot layout that TCT centroids are computed over -->
    <spec_ref>SPEC-UTML</spec_ref>              <!-- Consumer of constellation_violation_rate inside ΔΞ; consumer of Gτ-flagged chunks for Max aggregator selection -->
    <spec_ref>SPEC-PHASE2-PREDICTOR</spec_ref>  <!-- Predictor whose outputs are guarded by Gτ -->
    <spec_ref>SPEC-PHASE3-TRAINING</spec_ref>   <!-- Training loop hosting L_constellation_centroid -->
    <spec_ref>SPEC-PHASE4-INFERENCE</spec_ref>  <!-- verify() call site that invokes Gτ as the fourth gate -->
    <spec_ref>SPEC-WITNESS</spec_ref>           <!-- Provenance signing of frozen constellation artifacts -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md (FULL — primary reference; §1 DDA math, §2 three-phase protocol, §3 ΔΞ diagnostic #3, §5 reward formula, §7 implementation roadmap, §10 risks)</source>
    <source type="design">docs/ruvectorfindings/13_UNIVERSAL_AGENT_VERIFICATION.md (capability synthesis: agent-output verification flow, reward-signal density per claim)</source>
    <source type="design">docs/ruvectorfindings/14_AST_EMBEDDER_ATOMIC_CHUNKING.md §5 (per-chunk-type centroids; cross-chunk constellation), §8.1 (per-chunk localization for GuardRejected)</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.3 (predictor-side ΔΞ defenses; constellation guard as 3rd diagnostic)</source>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7 (multi-objective loss surface that L_constellation_centroid extends)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (&lt;tct_invariants&gt;, &lt;failure_taxonomy&gt; rules MEJEPA_INFER_CONSTELLATION_VIOLATION + MEJEPA_INFER_GTAU_STALE_CONSTELLATION, &lt;security_requirements&gt; rule SEC-MEJEPA-04, &lt;performance_budgets&gt; single_prediction_latency_p50)</source>
    <source type="paper">Royse, C. (April 2026). *Teleological Constellation Training: Multi-Modal Embedding Decomposition as Meaning Compression for Identity-Locked Generation and a Third Path Around the Data Wall.* ResearchGate DOI 10.13140/RG.2.2.24370.57288. Operator's published research; ContextGraph cited as one of two reference implementations.</source>
    <source type="prdtospec">docs2/prdtospec.md §3.3 (functional spec template), §3.4 (technical spec template)</source>
  </source_documents>
</metadata>

<overview>
The TCT-CONSTELLATION domain operationalizes the operator's published Teleological Constellation
Training framework (Royse 2026c, DOI 10.13140/RG.2.2.24370.57288) as the **per-output verifiability
substrate** of ME-JEPA-Code. It defines a frozen multi-modal centroid set CP, a deterministic
geometric guard Gτ that rejects predictions falling outside the constellation, and a
constellation-attraction loss term `L_constellation_centroid` that pushes the predictor toward the
right region of every embedder's latent space simultaneously. The domain is **cross-cutting**: it
defines the data + Gτ test logic; the training loss plumbing lives in SPEC-PHASE3-TRAINING; the
inference call site lives in SPEC-PHASE4-INFERENCE; the consumed `constellation_violation_rate`
ΔΞ component lives in SPEC-UTML.

The architectural move is the **same 21 frozen embedders serving three roles** without any
parameter update: labeller (Phase 1 centroid construction), loss target (Phase 3
`L_constellation_centroid`), and runtime guard (Phase 4 Gτ test at verify()). This is the
operational form of the constitution's `&lt;tct_invariants&gt;` rule that ME-JEPA's
`ΔΞ.target_collapse = 0 by construction` (per doc 11 §5.2). No instrument is updated across roles;
gradient flow through any centroid is a CRITICAL bug, not a feature.

The constellation comprises 1,512 panel-level cells (8 mutation categories × 9 languages × 21
embedders) **plus** 24,024 per-chunk-type cells (8 × 13 entity types × 11 languages × 21
embedders), with Bayesian shrinkage collapsing cells whose supporting sample count N &lt; 50 to
language-aggregate or entity-aggregate centroids. Per-embedder thresholds τ_m are calibrated on a
held-out validation slice to bound the Gτ false-positive rate at ≤ 5% — an explicit calibration
gate that fails closed when met empirically. Per-chunk-type thresholds sharpen Gτ when sample
support is sufficient; otherwise the panel-level threshold applies.

A **`Verdict::GuardRejected`** variant added to ME-JEPA's verdict enum carries
`violating_embedders: Vec&lt;GtauViolation&gt;` and `violating_chunks: Vec&lt;ChunkId&gt;` so that a
patch-level rejection localizes to a specific function/method/class/impl-block — per doc 14 §8.1.
The agent (or any operator reviewing the agent's output) can then **know precisely what failed**
(which chunk, which embedder, by how much), **re-verify independently** (Gτ is deterministic
O(N · d_max) on the persisted chunk panel), and **repair surgically** (next outer-loop turn
targets only the violating chunks). This is the per-output property scalar-reward alignment
methods cannot provide structurally.

Beneficiaries: (1) the Phase 3 training loop consumes `L_constellation_centroid` to push the
predictor onto the constellation; (2) the Phase 4 verify() API runs Gτ as the fourth gate
alongside conformal interval / OOD score / L_step health; (3) the UTML L_step formula consumes
`constellation_violation_rate` as a ΔΞ.predictor-side defense (per doc 11 §5.3 diagnostic #3);
(4) the UTML §IX coaching cycle classifier reads sustained Gτ rejection rate to trigger the
**Latent-Collapsing** intervention (refit centroids on fresh deterministic-walk data); (5) the
operator's reward formula upgrade `Reward = 0.4·calibrated_confidence + 0.3·mean(L_step·ΔΞ) +
0.3·gtau_pass_rate` per doc 13 §3.1 reads `gtau_pass_rate` from this domain.

This spec depends on Phase 0 corpus completion (SPEC-CORPUS): without ≥ 2,000 labeled mutation
entries spanning all 8 categories and supported languages, centroid construction in cells with
adequate sample support is not feasible.
</overview>

<user_types>

  <user_type id="UT-01" name="TCT Constellation Builder">
    <description>The Phase 4b batch process that walks the Phase 0 mutation corpus, collects (panel, mutation_category, oracle_outcome, language, entity_type) tuples, computes L2-normalized per-(category × language × embedder) and per-(mutation × entity_type × language × embedder) centroids, applies Bayesian shrinkage on sparse cells, calibrates per-embedder thresholds τ_m, and persists the frozen `TctConstellation` to RocksDB CF_MEJEPA_CONSTELLATION.</description>
    <permissions>READ Phase 0 corpus panels and oracle verdicts; READ frozen embedder + instrument outputs; WRITE one immutable `TctConstellation` artifact per construction run keyed by `version_id + corpus_provenance`; CANNOT mutate embedder weights; CANNOT modify a frozen constellation (must produce a new version_id).</permissions>
    <goals>Produce a deterministically replayable constellation artifact that is byte-equal across re-runs given the same inputs; meet the ≤ 5% false-positive-rate calibration target on held-out KnownGood examples per language; emit FSV evidence at /tmp/contextgraph-mejepa-tct-fsv/ that any third party can re-read + re-verify.</goals>
  </user_type>

  <user_type id="UT-02" name="Phase 4 Inference Verifier (Gτ caller)">
    <description>The Phase 4 verify() API host that, given a predicted panel `ŷ` from the predictor, runs the deterministic Gτ test against the loaded constellation and returns a Verdict variant — Pass / Fail / OutOfDistribution / Abstain / GuardRejected. Per doc 13 §3.3, Gτ is the fourth gate alongside conformal interval, OOD score, and L_step health.</description>
    <permissions>READ the loaded `TctConstellation`; READ predicted panel and per-chunk panels; WRITE Verdict structures to CF_MEJEPA_GUARD_DECISIONS; READ embedder versions to verify SHA-pin match; CANNOT modify the constellation.</permissions>
    <goals>Reject any prediction whose embedding is below threshold τ_m for any embedder against the (predicted_class, entity_type, language) centroid; produce per-embedder + per-chunk diagnostic detail in every rejection; complete in &lt; 10 ms per output with all 21 embedders cached.</goals>
  </user_type>

  <user_type id="UT-03" name="Phase 3 Training Loop (L_constellation_centroid host)">
    <description>The Phase 3 training process that adds `L_constellation_centroid` to the multi-objective loss surface and computes margin-based per-embedder geometric loss against the predicted-class centroid for each batch. The loss-term implementation lives in SPEC-PHASE3-TRAINING; this domain provides the centroid lookup contract.</description>
    <permissions>READ the frozen constellation per batch; READ predicted panel slot for each embedder; CANNOT write to the constellation; CANNOT update centroids during training.</permissions>
    <goals>Push the predictor's per-embedder slot output above τ_m for the predicted-class centroid across all 21 embedders simultaneously, defending against single-embedder Goodhart; observe non-trivial gradient (verifiable in training certs that record per-embedder hinge loss component).</goals>
  </user_type>

  <user_type id="UT-04" name="UTML L_step Computer (ΔΞ component consumer)">
    <description>The UTML L_step computation host (SPEC-UTML) that consumes `constellation_violation_rate_t = (recent guard rejections) / (total recent verifications)` over a rolling window as the third diagnostic in ΔΞ.predictor-side defenses (per doc 11 §5.3).</description>
    <permissions>READ Gτ guard decisions over a rolling window; READ Gτ-flagged chunk identifiers for Max aggregator selection; CANNOT mutate the underlying decision log; CANNOT modify Gτ thresholds.</permissions>
    <goals>Receive a deterministic float in [0, 1] every UTML L_step computation; receive Gτ-flagged chunk identifiers when per-chunk Max aggregator should be selected over the default Mean.</goals>
  </user_type>

  <user_type id="UT-05" name="Phase 5 Self-Healing Controller (Refit Trigger)">
    <description>The Phase 5 controller that watches `constellation_violation_rate` over a rolling window and, when it exceeds operator-configured X% (default 5%) sustained for the configured duration (default 2 hours), invokes the constellation refit on fresh deterministic-walk data. Implements UTML §IX "Latent-Collapsing" intervention.</description>
    <permissions>READ rolling-window guard rejection rate; READ live mutation corpus accumulating new examples; INVOKE constellation refit which produces a new immutable version_id; CANNOT modify a frozen constellation in place.</permissions>
    <goals>Detect centroid staleness and trigger refit before false-positive rejection rate degrades the predictor's training signal; preserve previous version_id for rollback if refit regresses on holdout.</goals>
  </user_type>

  <user_type id="UT-06" name="Operator (human) + External Verifier">
    <description>The human operator running the system, OR any third party with the frozen embedders + the persisted constellation, who re-runs Gτ on a persisted panel to independently verify a verdict. Per Royse 2026c §1, per-output verifiability via Gτ is a structural property: the test is deterministic O(N · d_max), no learned model, replayable.</description>
    <permissions>READ all FSV evidence; READ the constellation (with provenance metadata: corpus_sha, embedder_versions, frozen_at); RE-RUN Gτ on any persisted panel; APPROVE explicit refit/replacement of a frozen constellation; CANNOT silently mutate centroids.</permissions>
    <goals>Verify ME-JEPA's verdict is geometrically grounded; spot-check threshold calibration on a sample of past verdicts; audit constellation provenance before trusting the system's output; re-run Gτ if a verdict is suspicious.</goals>
  </user_type>

  <user_type id="UT-07" name="External AI Agent (Universal Verifier consumer)">
    <description>Any AI agent (Claude Code, GPT-5, Gemini, Cursor, Aider, internal automation) whose output is being verified by ME-JEPA. The agent emits a code patch + claim graph; ME-JEPA returns a Verdict including `gtau_satisfied: bool`, `gtau_violations: Vec&lt;GtauViolation&gt;`, and `violating_chunks: Vec&lt;ChunkId&gt;` (per doc 13 §4.2 universal-agent claim).</description>
    <permissions>READ the verdict structure including chunk-level localization; CANNOT read or mutate the constellation; CANNOT mutate the verdict.</permissions>
    <goals>Receive a structured rejection that names specific chunks + embedders so the next attempt can target them surgically; not be required to retrain ME-JEPA or its embedders to be verified.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-TCT-01" priority="must-have">
    <narrative>
      <as_a>TCT Constellation Builder (UT-01)</as_a>
      <i_want_to>Construct a frozen `TctConstellation` from the Phase 0 mutation corpus with per-(mutation_category × language × embedder), per-(language × mutation_category × embedder), per-(oracle_outcome × embedder), and per-(mutation × entity_type × language × embedder) centroids — totalling 1,512 panel-level cells + 24,024 per-chunk-type cells with Bayesian shrinkage on N &lt; 50 cells</i_want_to>
      <so_that>the same 21 frozen embedders can simultaneously serve all three TCT roles (labeller, loss target, runtime guard) without any parameter update, per doc 13 §3.4 + constitution &lt;tct_invariants&gt;</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-01-01">
        <given>Phase 0 corpus with ≥ 2,000 labeled (panel, mutation_category, oracle_outcome, language, entity_type) tuples</given>
        <when>The builder runs end-to-end</when>
        <then>It emits a `TctConstellation` with per_category_centroids covering all 8 mutation categories × 21 embedders; per_language_centroids covering all observed (language, mutation_category) pairs × 21 embedders; outcome_centroids covering all 4 oracle outcome classes × 21 embedders; per_chunk_type_centroids covering all (mutation, entity_type, language, embedder) cells where N ≥ 5 (post-shrinkage), with cells where 5 ≤ N &lt; 50 collapsed to language-aggregate or entity-aggregate centroid; corpus_provenance recording corpus_sha (SHA-256 of source corpus snapshot) + embedder_versions (per-embedder weight SHA-256) + frozen_at (ISO 8601 UTC).</then>
      </criterion>
      <criterion id="AC-TCT-01-02">
        <given>A constructed `TctConstellation`</given>
        <when>Two independent builds run on the same corpus snapshot with the same code_version</when>
        <then>The resulting artifacts are byte-equal: SHA-256 of the deterministically-serialized constellation matches across the two builds; centroids are L2-normalized to within f32 ULP tolerance; sort order of BTreeMap keys is canonical (per Rust BTreeMap ordering on &lt;EmbedderId, MutationCategory, Language, EntityType, OracleOutcome&gt; tuples).</then>
      </criterion>
      <criterion id="AC-TCT-01-03">
        <given>A cell whose supporting sample count N falls below 5 even after Bayesian shrinkage to language-aggregate AND entity-aggregate</given>
        <when>The builder attempts to construct that cell</when>
        <then>The build fails closed with `MEJEPA_TCT_INSUFFICIENT_SAMPLES` carrying the cell coordinate (mutation, entity_type, language, embedder) and the observed sample count; the build does NOT silently fall back to a global centroid; the operator must add corpus entries or explicitly mark the cell as out-of-distribution before re-running.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-02" priority="must-have">
    <narrative>
      <as_a>TCT Constellation Builder (UT-01)</as_a>
      <i_want_to>Calibrate per-embedder threshold τ_m on a held-out validation slice of KnownGood mutations such that the Gτ false-positive rate (rejecting a true-Pass patch) is ≤ 5% per embedder per language</i_want_to>
      <so_that>the runtime Gτ guard does not over-reject legitimate predictions; per doc 13 §3 + constitution &lt;testing_requirements&gt; "Constellation Gτ false-positive rate validation: ≤5% on held-out KnownGood examples per language"</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-02-01">
        <given>A held-out validation slice of KnownGood mutations per language with ≥ 30 samples per language</given>
        <when>The builder calibrates `thresholds: BTreeMap&lt;EmbedderId, f32&gt;` (panel-level τ_m) and `per_chunk_type_thresholds: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;`</when>
        <then>The empirical Gτ false-positive rate on the held-out slice is ≤ 0.05 for every (embedder, language) pair where sample support is sufficient; the calibration procedure records the empirical FPR, the chosen τ_m, and the sample count per cell; the thresholds are persisted in the constellation artifact alongside the centroids.</then>
      </criterion>
      <criterion id="AC-TCT-02-02">
        <given>A calibration run where any (embedder, language) cell exceeds 5% empirical FPR after threshold sweep</given>
        <when>The builder finalizes the constellation</when>
        <then>The build fails closed with `MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL` listing the cells that failed calibration; the constellation is NOT persisted; the operator must investigate (insufficient samples, embedder drift, corpus mismatch) before retrying.</then>
      </criterion>
      <criterion id="AC-TCT-02-03">
        <given>A per-chunk-type cell where sample support is sufficient (N ≥ 50 even before shrinkage)</given>
        <when>The builder calibrates `per_chunk_type_thresholds`</when>
        <then>The chunk-level τ_m is sharper (typically tighter) than the panel-level τ_m for the same embedder; chunk-level Gτ uses the chunk-level threshold when present and falls back to panel-level otherwise.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-03" priority="must-have">
    <narrative>
      <as_a>Phase 4 Inference Verifier (UT-02)</as_a>
      <i_want_to>Run Gτ as the fourth gate at verify() — a deterministic O(N · d_max) test that returns true iff cos(φ_m(ŷ), c_m) ≥ τ_m for every embedder m</i_want_to>
      <so_that>every verdict carries a deterministic geometric proof the operator (or any third party) can independently replay on the persisted panel — per doc 13 §3.3 and Royse 2026c §1's "per-output verifiability" structural property</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-03-01">
        <given>A predicted panel ŷ output by the Phase 2 predictor, a loaded `TctConstellation`, and the predicted class (mutation_category) inferred by the predictor's classification head</given>
        <when>verify() invokes Gτ</when>
        <then>The function returns `gtau_satisfied: bool` and `gtau_violations: Vec&lt;GtauViolation&gt;` where each violation records (embedder, observed_cos: f32, threshold: f32, violating_chunk: Option&lt;ChunkId&gt;); when `gtau_satisfied == false`, the verdict is `Verdict::GuardRejected` carrying violating_embedders + predictor_predicted + violating_chunks; the call completes in &lt; 10 ms p50 with all 21 embedders cached (per constitution &lt;performance_budgets&gt; single_prediction_latency_p50).</then>
      </criterion>
      <criterion id="AC-TCT-03-02">
        <given>A `Verdict::GuardRejected` produced by Gτ</given>
        <when>An operator (or any third party with the frozen embedders + the persisted constellation) re-runs the Gτ test on the persisted panel</when>
        <then>The result is identical (gtau_satisfied flag and all per-violation metadata byte-equal) — Gτ is fully deterministic; no learned model, no RNG, no nondeterministic floating-point ordering.</then>
      </criterion>
      <criterion id="AC-TCT-03-03">
        <given>A Gτ rejection where multiple chunks fail their per-chunk-type centroid</given>
        <when>verify() builds the GuardRejected variant</when>
        <then>`violating_chunks: Vec&lt;ChunkId&gt;` is populated with each (file_sha, language, entity_type, line_start, line_end, parent_chain_hash) tuple; the operator can map each entry back to a specific function/method/class/impl-block; the agent (UT-07) can target only those chunks on the next attempt — per doc 14 §8.1 chunk-level localization.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-04" priority="must-have">
    <narrative>
      <as_a>Phase 4 Inference Verifier (UT-02)</as_a>
      <i_want_to>Refuse to use a constellation older than the configured max-age (default 90 days) without explicit operator override</i_want_to>
      <so_that>stale centroids do not silently produce false rejections that degrade the predictor's training signal — per constitution &lt;security_requirements&gt; SEC-MEJEPA-04</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-04-01">
        <given>A loaded constellation whose `frozen_at` timestamp is older than `MEJEPA_TCT_MAX_AGE_DAYS` (default 90)</given>
        <when>verify() attempts to invoke Gτ</when>
        <then>The function returns `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` carrying frozen_at + age_days + max_age_days; verify() does NOT silently fall back to the predictor-only verdict; the operator must either refresh the constellation or set `MEJEPA_TCT_ALLOW_STALE = true` (audit-logged override) to proceed.</then>
      </criterion>
      <criterion id="AC-TCT-04-02">
        <given>A constellation whose `corpus_provenance.embedder_versions` contains a SHA-256 that does NOT match the runtime-loaded embedder weight digest</given>
        <when>verify() attempts to load the constellation</when>
        <then>The function returns `MEJEPA_TCT_PROVENANCE_MISMATCH` carrying the mismatched embedder + expected_sha + observed_sha; the constellation is NOT used; the operator must rebuild the constellation against the new embedder version.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-05" priority="must-have">
    <narrative>
      <as_a>Phase 3 Training Loop (UT-03)</as_a>
      <i_want_to>Compute `L_constellation_centroid = Σ_m=1^N max(0, τ_m − cos(g_φ(panel)_m, c_m_{predicted_class}))` as a margin-based per-embedder geometric loss term</i_want_to>
      <so_that>the predictor cannot Goodhart against any one embedder without being penalized in the others — per doc 13 §2.2's stated structural advantage of TCT Phase 2 loss over single-embedder cosine</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-05-01">
        <given>A batch of training tuples with predicted panel `g_φ(panel)` and predicted-class label produced by the Phase 2 classification head</given>
        <when>The training loop computes `L_constellation_centroid`</when>
        <then>The term is summed over all 21 embedders' panel slots; for each embedder m, the hinge term `max(0, τ_m − cos(g_φ(panel)_m, c_m_{predicted_class}))` is computed against the (predicted_class, language, entity_type) centroid via constellation lookup; the per-embedder hinge components are persisted in the per-step training certificate (per constitution &lt;utml_v1_invariants&gt; rule on training certs); zero gradient flows through the centroid c_m or the embedder weights during backprop.</then>
      </criterion>
      <criterion id="AC-TCT-05-02">
        <given>A constellation-attraction step where the predictor produces a panel slot at exactly cos == τ_m for some embedder</given>
        <when>The hinge is evaluated</when>
        <then>The contribution is exactly 0.0 (boundary condition); no spurious gradient is generated; this is verifiable via FSV unit test where two predicted slots straddle τ_m by ±1 ULP.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-06" priority="must-have">
    <narrative>
      <as_a>UTML L_step Computer (UT-04)</as_a>
      <i_want_to>Read `constellation_violation_rate = (recent guard rejections) / (total recent verifications)` over a configurable rolling window for use as the third ΔΞ.predictor-side defense diagnostic</i_want_to>
      <so_that>UTML's L_step factor ΔΞ correctly reflects predictor-side latent integrity per doc 11 §5.3 + SPEC-UTML &lt;requirement REQ-UTML-12&gt; consumer contract</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-06-01">
        <given>The rolling window of recent verify() calls (default 1,000 calls or 24 hours, whichever is smaller)</given>
        <when>SPEC-UTML's compute_delta_xi reads `constellation_violation_rate`</when>
        <then>The returned float is in [0, 1]; exactly equals (count of GuardRejected verdicts in window) / (total count of verdicts in window); when total count &lt; 30, the function returns Option::None (insufficient samples) and SPEC-UTML's compute_delta_xi falls back to its insufficient-samples policy.</then>
      </criterion>
      <criterion id="AC-TCT-06-02">
        <given>A sustained constellation_violation_rate &gt; 0.05 over a 2-hour rolling window</given>
        <when>The Phase 5 self-healing controller (UT-05) reads the rate</when>
        <then>The controller emits a "Latent-Collapsing" UTML §IX intervention signal that triggers constellation refit on fresh deterministic-walk data; per doc 13 §10 risk #1 mitigation; the previous constellation version_id is preserved for rollback if the refit regresses on holdout.</then>
      </criterion>
      <criterion id="AC-TCT-06-03">
        <given>A Gτ rejection that flagged a specific chunk as the violating chunk</given>
        <when>SPEC-UTML's per-chunk ΔP aggregator selection inspects the GuardRejected verdict</when>
        <then>The Max aggregator (over per-chunk losses) is selected for that L_step (per SPEC-UTML &lt;edge_case EC-UTML-04&gt;); the standard Mean aggregator is used otherwise.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-07" priority="must-have">
    <narrative>
      <as_a>Operator + External Verifier (UT-06)</as_a>
      <i_want_to>Re-run Gτ independently on a persisted panel + chunk panels using only the frozen embedders and the persisted constellation</i_want_to>
      <so_that>per-output verifiability is structurally defensible: any third party can recompute the verdict without retraining, without proprietary models, and without privileged access — per Royse 2026c §1 + §7's "structurally different from RLHF/DPO/Constitutional AI" claim</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-07-01">
        <given>A persisted Verdict + its associated panel + chunk panels and the constellation referenced by the verdict's version_id</given>
        <when>An external verifier re-loads embedder weights matching the constellation's `corpus_provenance.embedder_versions` and re-runs Gτ</when>
        <then>The reproduced gtau_satisfied flag and per-embedder cosines match the persisted verdict to within f32 ULP; verdict reproducibility ≥ 99.9% (per doc 13 §11 falsifier #8); any divergence &gt; 0.1% is a critical determinism bug.</then>
      </criterion>
      <criterion id="AC-TCT-07-02">
        <given>The shipped FSV harness binary for TCT</given>
        <when>The harness runs end-to-end</when>
        <then>It writes /tmp/contextgraph-mejepa-tct-fsv/{constellation-evidence.json, gtau-evidence.json, threshold-calibration-evidence.json, chunk-level-rejection-evidence.json}; re-reads each file; asserts byte-equal Gτ recomputation, threshold-calibration FPR ≤ 5%, chunk-level localization metadata round-trips through serde, and artifact provenance matches; reports pass/fail per file.</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-TCT-08" priority="should-have">
    <narrative>
      <as_a>External AI Agent (UT-07)</as_a>
      <i_want_to>Receive a structured GuardRejected verdict naming specific chunks + embedders that failed</i_want_to>
      <so_that>I can target only the violating chunks on my next attempt — surgical repair, not whole-patch regeneration — per doc 14 §8.1 + doc 13 §4.2 universal-agent claim</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-TCT-08-01">
        <given>An agent's patch that triggers Gτ rejection on a single chunk</given>
        <when>verify() returns Verdict::GuardRejected</when>
        <then>The verdict's `violating_chunks` contains exactly that one ChunkId with sufficient localization metadata (file_sha, language, entity_type, line_start, line_end, parent_chain_hash) for the agent to deterministically locate the chunk in the post-edit source; other chunks in the patch are NOT listed as violating; the agent can produce a follow-up patch that touches only the violating chunk's lines.</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <!-- ============ TCT Phase 1: Construct frozen multi-modal centroid set CP ============ -->

  <requirement id="REQ-TCT-01" story_ref="US-TCT-01" priority="must">
    <description>
      The crate `context-graph-mejepa-tct` SHALL define a `TctConstellation` data type with the following fields: `per_category_centroids: BTreeMap&lt;MutationCategory, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;` (8 × 21 = 168 panel-level centroids); `per_language_centroids: BTreeMap&lt;(Language, MutationCategory), BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;` (up to 9 × 8 × 21 = 1,512 cells); `outcome_centroids: BTreeMap&lt;OracleOutcome, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;` (4 × 21 = 84 cells); `per_chunk_type_centroids: BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), Vec&lt;f32&gt;&gt;` (up to 8 × 13 × 11 × 21 = 24,024 cells with Bayesian shrinkage on N &lt; 50); `thresholds: BTreeMap&lt;EmbedderId, f32&gt;`; `per_chunk_type_thresholds: BTreeMap&lt;(EntityType, EmbedderId), f32&gt;`; `corpus_provenance: CorpusProvenance` (corpus_sha + per-embedder weight SHA-256 + frozen_at ISO 8601 + code_version git SHA); `frozen_at: SystemTime`. All centroids SHALL be L2-normalized.
    </description>
    <rationale>Per doc 13 §2.1 (Phase 1 protocol) + constitution &lt;tct_invariants&gt; rule on centroid frozenness + cell math. The struct literal layout is the contract the Builder + Gτ + L_constellation_centroid + UTML consumers all depend on.</rationale>
    <constraints>
      - Every centroid is L2-normalized to unit length to within f32 ULP
      - Every centroid is finite (no NaN/Inf)
      - Each `Vec&lt;f32&gt;` length equals the dimensionality of the corresponding embedder
      - BTreeMap ordering is canonical (Rust default ordering on keys)
      - The struct is immutable post-construction; only loadable, not mutable
    </constraints>
    <verification_method>FSV at /tmp/contextgraph-mejepa-tct-fsv/constellation-evidence.json round-trips through serde; centroid L2-norm assertions pass on every loaded cell; cell counts match the cell-math table.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-02" story_ref="US-TCT-01" priority="must">
    <description>
      The crate SHALL expose a `ConstellationBuilder` that, given a Phase 0 corpus iterator yielding (panel, mutation_category, oracle_outcome, language, entity_type, chunk_panels) tuples, computes per-cell centroids by averaging panels in each cell and L2-normalizing. The builder SHALL accept a corpus snapshot SHA-256 + embedder version map + frozen_at timestamp as construction inputs.
    </description>
    <rationale>Per doc 13 §2.1 + §7 step 2 (per-class centroid computation in ~2 days).</rationale>
    <constraints>
      - Centroid computation is deterministic given the same inputs
      - Same code_version + same corpus_sha + same embedder_versions = byte-equal `TctConstellation`
      - The builder NEVER mutates an already-frozen constellation; produces a new artifact
      - Sample-count tracking per cell is recorded for FSV evidence
    </constraints>
    <verification_method>Two independent builds on the same inputs yield byte-equal SHA-256-of-serialized output; FSV evidence records sample counts per cell.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-03" story_ref="US-TCT-01" priority="must">
    <description>
      The builder SHALL apply Bayesian shrinkage on cells with supporting sample count `N &lt; 50`, collapsing them to language-aggregate centroid (when language-aggregate has ≥ 50 samples) or entity-aggregate centroid (when entity-aggregate has ≥ 50 samples). Cells with N &lt; 5 SHALL fail closed with `MEJEPA_TCT_INSUFFICIENT_SAMPLES`. The shrinkage policy SHALL be deterministic (preferring language-aggregate over entity-aggregate when both are available).
    </description>
    <rationale>Per doc 13 §10 risk #5 (mitigation: Bayesian shrinkage when per-cell support is below threshold) + constitution &lt;tct_invariants&gt; rule on per-chunk-type cell math.</rationale>
    <constraints>
      - Shrinkage threshold: N &lt; 50 triggers collapse; N &lt; 5 fails closed
      - Aggregation order: language-aggregate first, then entity-aggregate, then per-mutation-category global; failure if all are insufficient
      - The shrinkage record (which cell collapsed to which aggregate) is persisted in FSV evidence
      - Shrinkage does NOT modify centroid L2-normalization
    </constraints>
    <verification_method>FSV evidence file `constellation-evidence.json` records each shrinkage decision; cells with N &lt; 5 produce MEJEPA_TCT_INSUFFICIENT_SAMPLES that the FSV harness asserts on.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-04" story_ref="US-TCT-02" priority="must">
    <description>
      The builder SHALL calibrate per-embedder thresholds `τ_m` (panel-level) and per-(entity_type, embedder) thresholds (chunk-level) via a sweep over a held-out validation slice of KnownGood mutations such that the empirical Gτ false-positive rate is ≤ 5% per (embedder, language) cell with sufficient sample support (≥ 30 samples per language).
    </description>
    <rationale>Per doc 13 §3 + constitution &lt;testing_requirements&gt; "Constellation Gτ false-positive rate validation: ≤5% on held-out KnownGood examples per language" + risk #2 mitigation (re-calibrate τ_m on live-attempt holdout slice during Phase 5 self-healing).</rationale>
    <constraints>
      - Calibration uses ONLY held-out KnownGood mutations (never seen by centroid construction)
      - Empirical FPR ≤ 0.05 per cell with ≥ 30 samples
      - Calibration that exceeds 5% FPR fails closed with `MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL`
      - Threshold sweep is bounded (e.g., 100 candidates between observed min and max cosine)
      - Per-chunk-type thresholds are sharper than panel-level when N ≥ 50 supporting samples
    </constraints>
    <verification_method>FSV at /tmp/contextgraph-mejepa-tct-fsv/threshold-calibration-evidence.json records per-cell empirical FPR + chosen τ_m + sample counts; harness asserts FPR ≤ 0.05 on every cell with sufficient samples.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-05" story_ref="US-TCT-01" priority="must">
    <description>
      The frozen `TctConstellation` SHALL be persisted to RocksDB column family `CF_MEJEPA_CONSTELLATION` keyed by `(version_id: [u8; 32], corpus_provenance.corpus_sha: [u8; 32])` where version_id = SHA-256(deterministic-serialization). The constellation SHALL be loaded into memory at process start and held entirely in RAM (~30 MB after Bayesian shrinkage per task brief budget); reload triggered only by explicit operator action or Phase 5 refit.
    </description>
    <rationale>Per doc 13 §2.1 (FROZEN gate) + §7 step 1 + constitution &lt;directory_structure&gt; CF naming convention + &lt;performance_budgets&gt; vram_steady_state allowance.</rationale>
    <constraints>
      - CF name: `CF_MEJEPA_CONSTELLATION` (constitution rocksdb_cfs convention)
      - Key: (version_id, corpus_sha) pair as 64-byte composite key
      - Value: deterministic serde_cbor or bincode2 of `TctConstellation`
      - Frozen-at-rest: writes are append-only; updates produce a new version_id
      - Loaded-in-RAM: ~30 MB total after Bayesian shrinkage
    </constraints>
    <verification_method>RocksDB integration test writes + reads back a constellation; SHA-256 of value byte-equals on round-trip.</verification_method>
  </requirement>

  <!-- ============ TCT Phase 2: L_constellation_centroid loss term contract ============ -->

  <requirement id="REQ-TCT-06" story_ref="US-TCT-05" priority="must">
    <description>
      The crate SHALL expose a `lookup_centroid(predicted_class, language, entity_type, embedder_id) -&gt; Option&lt;&amp;[f32]&gt;` function that returns the best-matching centroid for the requested cell, applying shrinkage fallback (chunk-type → entity-aggregate → language-aggregate → category-aggregate) silently when the requested cell was shrunk during construction.
    </description>
    <rationale>Per doc 13 §2.2 (L_constellation_centroid formula); the loss-term host (SPEC-PHASE3-TRAINING) reads centroids only via this contract — never directly indexes the BTreeMap — so shrinkage fallback is encapsulated.</rationale>
    <constraints>
      - Returns Option::None ONLY when no suitable centroid exists at any aggregate level (post-shrinkage)
      - Returns the shrinkage record alongside (which level the centroid came from) for FSV evidence
      - Lookup is O(log N) on the BTreeMap; caller is expected to cache lookups within a batch
      - The returned centroid slice is borrowed from the loaded constellation (zero-copy)
    </constraints>
    <verification_method>Unit tests with synthesized constellations at varied shrinkage levels; lookup precedence verified.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-07" story_ref="US-TCT-05" priority="must">
    <description>
      The crate SHALL provide a centroid-attraction primitive `constellation_hinge(predicted_panel: &amp;Panel, predicted_class: MutationCategory, language: Language, entity_type: EntityType, constellation: &amp;TctConstellation) -&gt; ConstellationHingeOutput` returning per-embedder hinge values `max(0, τ_m − cos(predicted_panel.slot_for(embedder_m), c_m))` summed over all 21 embedders. The primitive SHALL produce **zero gradient through any centroid or embedder weight** (centroids are constants from the autograd graph's perspective).
    </description>
    <rationale>Per doc 13 §2.2 + constitution &lt;anti_patterns&gt; rule "No learnable target encoder" + &lt;tct_invariants&gt; rule on the same 21 frozen embedders serving three roles without parameter update. The training-loop integration of the L_constellation_centroid term lives in SPEC-PHASE3-TRAINING; this primitive is the mathematical contract.</rationale>
    <constraints>
      - Output: per-embedder hinge value (21 floats) + summed total + chosen centroid origin (post-shrinkage cell coordinate)
      - Gradient flow: ONLY through the predictor's panel slot output; centroids are detached
      - Latency: &lt; 1 ms per batch element with cached embedder slots and centroids
      - Hinge boundary case: when cos == τ_m exactly, contribution is exactly 0.0
    </constraints>
    <verification_method>Unit tests for boundary case; gradient-flow detection asserts zero gradient through centroids; FSV in SPEC-PHASE3-TRAINING training-cert evidence validates per-embedder hinge components are recorded.</verification_method>
  </requirement>

  <!-- ============ TCT Phase 3: Gτ deterministic geometric guard ============ -->

  <requirement id="REQ-TCT-08" story_ref="US-TCT-03" priority="must">
    <description>
      The crate SHALL expose a `gtau_check(predicted_panel: &amp;Panel, predicted_class: MutationCategory, language: Language, entity_type: EntityType, constellation: &amp;TctConstellation) -&gt; GtauOutput` function computing `Gτ(ŷ; CP) = ⟦∀m : cos(φ_m(ŷ), c_m) ≥ τ_m⟧` deterministically. The function SHALL return `gtau_satisfied: bool` and `violations: Vec&lt;GtauViolation&gt;` where each violation records the offending embedder, observed cosine, threshold, and (when chunk-level inputs are provided) the violating ChunkId.
    </description>
    <rationale>Per doc 13 §2.3 (Phase 3 protocol) + §3.3 (deterministic O(N · d_max), no learned model) + constitution &lt;tct_invariants&gt; rule "Gτ test is deterministic O(N · d_max), no learned model, re-runnable by anyone".</rationale>
    <constraints>
      - Cost: O(N × d_max) where N = 21 embedders, d_max = max embedder dim (~1536 for E7)
      - Determinism: same predicted_panel + same constellation = byte-equal output
      - No learned model in the test path
      - All 21 embedders evaluated even if early-exit could save cost (preserves diagnostic completeness)
      - Latency: &lt; 10 ms p50 with embedders cached (per constitution &lt;performance_budgets&gt; single_prediction_latency_p50)
    </constraints>
    <verification_method>FSV at /tmp/contextgraph-mejepa-tct-fsv/gtau-evidence.json with synthesized true-Pass and true-Fail panels; deterministic re-run verification; latency profiling.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-09" story_ref="US-TCT-03" priority="must">
    <description>
      The crate SHALL expose a chunk-level Gτ variant `gtau_check_chunks(chunk_panels: &amp;[(ChunkId, Panel)], predicted_class: MutationCategory, constellation: &amp;TctConstellation) -&gt; ChunkGtauOutput` returning per-chunk Gτ verdicts with chunk-level localization. Each chunk's panel is checked against its (predicted_class, chunk.entity_type, chunk.language) per-chunk-type centroid using `per_chunk_type_thresholds[(entity_type, embedder)]` when present, falling back to panel-level `thresholds[embedder]` otherwise.
    </description>
    <rationale>Per doc 14 §5 (per-chunk-type centroids) + §8.1 (per-chunk localization) + constitution &lt;tct_invariants&gt; rule "Per-chunk-type localization: GuardRejected verdicts include violating_chunks: Vec&lt;ChunkId&gt;".</rationale>
    <constraints>
      - Each chunk evaluated independently; rejection is per-chunk, not per-patch
      - Chunk-level thresholds preferred when sample support was sufficient; panel-level threshold used as fallback
      - violating_chunks: Vec&lt;ChunkId&gt; lists every chunk that failed Gτ for at least one embedder
      - Determinism: same chunks + same constellation = byte-equal output
    </constraints>
    <verification_method>FSV at /tmp/contextgraph-mejepa-tct-fsv/chunk-level-rejection-evidence.json verifies chunk-level Gτ produces correct per-chunk localization and falls back to panel-level threshold when per-chunk-type cell was shrunk.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-10" story_ref="US-TCT-03" priority="must">
    <description>
      The crate SHALL define a `Verdict::GuardRejected` variant carrying `violating_embedders: Vec&lt;GtauViolation&gt;`, `predictor_predicted: PredictorOutput`, and `violating_chunks: Vec&lt;ChunkId&gt;`. The verify() API host (SPEC-PHASE4-INFERENCE) SHALL emit `Verdict::GuardRejected` whenever Gτ rejects, persisting the verdict to `CF_MEJEPA_GUARD_DECISIONS` for audit + reproducibility + downstream `constellation_violation_rate` consumption.
    </description>
    <rationale>Per doc 13 §2.3 Verdict::GuardRejected definition + doc 14 §8.1 chunk localization. The verdict variant + persistence is defined here; the verify() call site lives in SPEC-PHASE4-INFERENCE.</rationale>
    <constraints>
      - violating_embedders.len() ≥ 1 always (otherwise Gτ would not reject)
      - violating_chunks: Vec&lt;ChunkId&gt; populated when chunk-level Gτ ran; empty when only panel-level Gτ ran
      - predictor_predicted carries enough state for an external verifier to recompute Gτ from scratch
      - Persisted to CF_MEJEPA_GUARD_DECISIONS keyed by (verdict_id, version_id) for byte-equal reproduction
    </constraints>
    <verification_method>Round-trip serde verification through CF_MEJEPA_GUARD_DECISIONS; external-verifier replay test in FSV harness reproduces gtau_satisfied + per-violation cosines to within ULP.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-11" story_ref="US-TCT-04" priority="must">
    <description>
      The crate SHALL enforce a constellation max-age policy: when `frozen_at` is older than `MEJEPA_TCT_MAX_AGE_DAYS` (default 90 days; configurable via env var) at verify() invocation, the load function SHALL return `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` carrying frozen_at + observed_age + max_age. An audit-logged operator override `MEJEPA_TCT_ALLOW_STALE = true` is the only way to proceed without refresh.
    </description>
    <rationale>Per constitution &lt;security_requirements&gt; SEC-MEJEPA-04 + &lt;failure_taxonomy&gt; rule MEJEPA_INFER_GTAU_STALE_CONSTELLATION + doc 13 §10 risk #1 (centroid staleness mitigation).</rationale>
    <constraints>
      - Default max-age: 90 days
      - Override is audit-logged to the SMAP `./memory/journal/` namespace and the witness chain
      - Stale constellation does NOT silently fall back to predictor-only verdict
    </constraints>
    <verification_method>Integration test loads a synthesized stale constellation; verify() returns MEJEPA_INFER_GTAU_STALE_CONSTELLATION; audit-log entry verified.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-12" story_ref="US-TCT-04" priority="must">
    <description>
      The crate SHALL refuse to load a constellation whose `corpus_provenance.embedder_versions` SHA-256 digests do NOT match the runtime-loaded embedder weight digests. The load function SHALL return `MEJEPA_TCT_PROVENANCE_MISMATCH` carrying the mismatched embedder + expected_sha + observed_sha.
    </description>
    <rationale>Per doc 13 §10 risk #4 (embedder version skew mitigation) + constitution &lt;security_requirements&gt; SEC-MEJEPA-02 (refuse to load if digest mismatches).</rationale>
    <constraints>
      - All 21 embedder digests checked at constellation load time
      - Mismatch on ANY embedder fails closed; no partial-load acceptance
      - Recovery: rebuild constellation against the new embedder weights
    </constraints>
    <verification_method>Integration test with mocked embedder digest mismatch; load function returns MEJEPA_TCT_PROVENANCE_MISMATCH; constellation NOT used.</verification_method>
  </requirement>

  <!-- ============ Cross-cutting consumer contracts ============ -->

  <requirement id="REQ-TCT-13" story_ref="US-TCT-06" priority="must">
    <description>
      The crate SHALL expose `constellation_violation_rate(window: RollingWindow) -&gt; Option&lt;f32&gt;` returning the fraction of recent verify() calls whose Gτ rejected the predicted panel over the configured rolling window (default 1,000 calls or 24 hours, whichever is smaller). The function SHALL return Option::None when total verdicts in window &lt; 30 (insufficient samples); the SPEC-UTML compute_delta_xi consumer is contractually responsible for the insufficient-samples policy.
    </description>
    <rationale>Per doc 11 §5.3 (ΔΞ.predictor-side defense diagnostic #3) + SPEC-UTML &lt;consumer contract REQ-UTML-12&gt; cross-spec dependency declaration.</rationale>
    <constraints>
      - Returned float ∈ [0, 1] always (or Option::None on insufficient samples)
      - Reads from CF_MEJEPA_GUARD_DECISIONS rolling window
      - Window is configurable via `MEJEPA_TCT_RATE_WINDOW_SIZE` and `MEJEPA_TCT_RATE_WINDOW_HOURS` env vars
    </constraints>
    <verification_method>Unit tests with synthesized verdict streams; assertion that rate equals (rejected count) / (total count); insufficient-samples test returns Option::None.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-14" story_ref="US-TCT-07" priority="must">
    <description>
      The crate SHALL ship a Forensic Source Verification (FSV) example binary that writes evidence to /tmp/contextgraph-mejepa-tct-fsv/{constellation-evidence.json, gtau-evidence.json, threshold-calibration-evidence.json, chunk-level-rejection-evidence.json}. The harness SHALL re-read each file and assert: (a) constellation round-trips through serde; (b) Gτ recomputation reproduces persisted verdicts byte-equal; (c) threshold-calibration FPR ≤ 0.05; (d) chunk-level localization metadata round-trips through serde and maps back to source-of-truth chunks.
    </description>
    <rationale>Per constitution &lt;coding_standards.fsv_protocol&gt; rule and &lt;testing_requirements.fsv_evidence_paths&gt; declaration of /tmp/contextgraph-mejepa-tct-fsv/.</rationale>
    <constraints>
      - FSV files created with mode 0o600 (per SEC-MEJEPA-07)
      - Tests use real synthesized panel data (not mocks)
      - Harness exits non-zero on any assertion failure
    </constraints>
    <verification_method>Manual review of FSV harness output; CI runs the FSV harness as part of test suite.</verification_method>
  </requirement>

  <requirement id="REQ-TCT-15" story_ref="US-TCT-08" priority="should">
    <description>
      The crate SHALL emit a `mcp__cgreality__mejepa_constellation_inspect` MCP tool that returns the loaded constellation's metadata (version_id, corpus_provenance, frozen_at, cell counts per category × language × entity_type, sample-support histogram, calibrated threshold table) for operator inspection. The tool SHALL NOT return centroid byte arrays directly — only summary statistics.
    </description>
    <rationale>Per doc 13 §7 step 10 (TCT-aware MCP tool) + constitution &lt;coding_standards.naming_conventions&gt; mcp_tool_namespace_lock requiring `mcp__cgreality__mejepa_*` prefix.</rationale>
    <constraints>
      - Output is JSON-serializable summary; no raw centroid vectors
      - Tool is read-only; cannot mutate the constellation
      - MCP namespace: `mcp__cgreality__mejepa_constellation_inspect`
    </constraints>
    <verification_method>Integration test exercises the MCP tool; output schema matches contract.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-TCT-01" req_ref="REQ-TCT-01">
    <condition>Constellation construction over Phase 0 mutation corpus</condition>
    <action>L2-normalize every per-modality centroid; record corpus_sha + per-embedder weight SHA-256 + frozen_at; persist as immutable artifact keyed by version_id</action>
    <exception>None — construction always produces a new immutable version_id</exception>
  </rule>

  <rule id="BR-TCT-02" req_ref="REQ-TCT-03">
    <condition>Per-chunk-type cell with 5 ≤ N &lt; 50 supporting examples</condition>
    <action>Apply Bayesian shrinkage: collapse to language-aggregate centroid first; if still N &lt; 50, collapse to entity-aggregate; if still N &lt; 50, fall back to per-mutation-category global centroid; record the shrinkage decision in FSV evidence</action>
    <exception>Cells with N &lt; 5 fail closed with MEJEPA_TCT_INSUFFICIENT_SAMPLES — no silent fallback</exception>
  </rule>

  <rule id="BR-TCT-03" req_ref="REQ-TCT-04">
    <condition>Threshold calibration on held-out KnownGood validation slice</condition>
    <action>Sweep candidate τ_m values; pick the smallest τ_m such that empirical Gτ FPR on held-out slice ≤ 0.05 per (embedder, language) cell with ≥ 30 samples</action>
    <exception>If no τ_m yields ≤ 5% FPR, fail with MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL — operator must investigate (insufficient samples, embedder drift, corpus mismatch)</exception>
  </rule>

  <rule id="BR-TCT-04" req_ref="REQ-TCT-08">
    <condition>verify() invokes Gτ on a predicted panel</condition>
    <action>Compute cos(φ_m(ŷ), c_m) for every embedder m; compare to per-embedder threshold τ_m; gtau_satisfied = (∀m: cos ≥ τ_m); populate violations otherwise</action>
    <exception>If constellation is stale (frozen_at older than MEJEPA_TCT_MAX_AGE_DAYS), refuse with MEJEPA_INFER_GTAU_STALE_CONSTELLATION; if embedder digests mismatch provenance, refuse with MEJEPA_TCT_PROVENANCE_MISMATCH</exception>
  </rule>

  <rule id="BR-TCT-05" req_ref="REQ-TCT-09">
    <condition>Chunk-level Gτ on a multi-chunk patch</condition>
    <action>Evaluate Gτ per chunk independently; populate violating_chunks: Vec&lt;ChunkId&gt; with every chunk that failed for at least one embedder; per-chunk-type threshold preferred over panel-level when chunk-type cell had ≥ 50 samples</action>
    <exception>Chunks whose entity_type is unsupported in the constellation map to the closest supported type via SPEC-AST-CHUNKING's mapping rules</exception>
  </rule>

  <rule id="BR-TCT-06" req_ref="REQ-TCT-10">
    <condition>Gτ rejection at verify()</condition>
    <action>Emit Verdict::GuardRejected carrying violating_embedders + predictor_predicted + violating_chunks; persist to CF_MEJEPA_GUARD_DECISIONS for audit + downstream consumption</action>
    <exception>None — every rejection is persisted</exception>
  </rule>

  <rule id="BR-TCT-07" req_ref="REQ-TCT-13">
    <condition>UTML compute_delta_xi consumer reads constellation_violation_rate</condition>
    <action>Compute (count of GuardRejected verdicts in rolling window) / (total verdicts in window); return Option::None when total &lt; 30; SPEC-UTML's compute_delta_xi handles the None case via insufficient-samples policy</action>
    <exception>None</exception>
  </rule>

  <rule id="BR-TCT-08" req_ref="REQ-TCT-13">
    <condition>Sustained constellation_violation_rate &gt; 0.05 over 2-hour rolling window</condition>
    <action>Phase 5 self-healing controller emits "Latent-Collapsing" UTML §IX intervention; triggers constellation refit on fresh deterministic-walk data; previous version_id preserved for rollback</action>
    <exception>If refit regresses on holdout, the previous version_id is reactivated (per constitution &lt;failure_taxonomy&gt; MEJEPA_RETRAIN_HOLDOUT_REGRESSION)</exception>
  </rule>

  <rule id="BR-TCT-09" req_ref="REQ-TCT-07">
    <condition>L_constellation_centroid backward pass</condition>
    <action>Allow gradient ONLY through the predictor's panel slot output; centroids are detached; embedder weights are detached; per-step training cert records per-embedder hinge components</action>
    <exception>Any nonzero gradient detected on a centroid or embedder weight is a CRITICAL bug per constitution &lt;anti_patterns&gt; "No learnable target encoder" — fail-closed</exception>
  </rule>

  <rule id="BR-TCT-10" req_ref="REQ-TCT-15">
    <condition>Operator invokes mcp__cgreality__mejepa_constellation_inspect</condition>
    <action>Return constellation metadata + cell counts + sample-support histogram + threshold table; never return raw centroid byte arrays</action>
    <exception>Tool is read-only; cannot mutate</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-TCT-01" req_ref="REQ-TCT-03">
    <scenario>A per-chunk-type cell has exactly N = 50 samples (boundary between full centroid and Bayesian shrinkage)</scenario>
    <expected_behavior>Cell keeps its own centroid (the shrinkage threshold is N &lt; 50, so 50 is sufficient); FSV records the sample count and the decision; no shrinkage applied</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-TCT-02" req_ref="REQ-TCT-03">
    <scenario>A per-chunk-type cell has N = 4 samples (below the hard fail threshold of 5)</scenario>
    <expected_behavior>Build fails closed with MEJEPA_TCT_INSUFFICIENT_SAMPLES carrying cell coordinate + sample count; constellation NOT persisted; operator must add corpus entries for this cell or mark it as out-of-distribution before retrying</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-TCT-03" req_ref="REQ-TCT-04">
    <scenario>Calibration on a (embedder, language) cell with exactly 29 held-out samples (below the 30-sample minimum)</scenario>
    <expected_behavior>Calibration emits a warning + skips strict FPR ≤ 5% enforcement for that cell; threshold falls back to a conservative default (panel-level threshold for the embedder); FSV records the skip</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-TCT-04" req_ref="REQ-TCT-08">
    <scenario>Predicted panel has exactly cos == τ_m for some embedder (boundary)</scenario>
    <expected_behavior>Gτ accepts (the inequality is `cos ≥ τ_m`, inclusive); no violation generated; verdict proceeds based on other gates</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-TCT-05" req_ref="REQ-TCT-08">
    <scenario>Predicted class is one the predictor has never seen during training (unseen mutation_category)</scenario>
    <expected_behavior>Gτ falls back to category-aggregate centroid; if that is also unavailable, the verdict is Verdict::OutOfDistribution (not GuardRejected) and OOD reasoning takes over</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-TCT-06" req_ref="REQ-TCT-09">
    <scenario>A patch with zero chunks (empty patch)</scenario>
    <expected_behavior>Chunk-level Gτ emits an empty violations vector; panel-level Gτ takes over; if panel-level Gτ rejects, verdict carries violating_embedders but empty violating_chunks</expected_behavior>
    <priority>low</priority>
  </edge_case>

  <edge_case id="EC-TCT-07" req_ref="REQ-TCT-09">
    <scenario>A patch with chunks of EntityType not in the (KnownGood, EntityType, Language) constellation map (e.g., a new C++ template specialization not seen during construction)</scenario>
    <expected_behavior>Per SPEC-AST-CHUNKING mapping rules, chunk maps to closest supported EntityType; if no mapping exists, chunk is flagged in violations as `entity_type: Unknown` and the panel-level threshold applies</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-TCT-08" req_ref="REQ-TCT-11">
    <scenario>Constellation is exactly 89 days old at verify() invocation (boundary just under default 90-day max-age)</scenario>
    <expected_behavior>verify() proceeds; no MEJEPA_INFER_GTAU_STALE_CONSTELLATION; constellation is still considered fresh</expected_behavior>
    <priority>low</priority>
  </edge_case>

  <edge_case id="EC-TCT-09" req_ref="REQ-TCT-11">
    <scenario>Constellation is exactly 91 days old at verify() invocation (boundary just over default 90-day max-age)</scenario>
    <expected_behavior>verify() returns MEJEPA_INFER_GTAU_STALE_CONSTELLATION; operator override required (audit-logged)</expected_behavior>
    <priority>high</priority>
  </edge_case>

  <edge_case id="EC-TCT-10" req_ref="REQ-TCT-12">
    <scenario>One of 21 embedder weight digests changes (e.g., E11 updated to a new SHA) but the constellation is not rebuilt</scenario>
    <expected_behavior>verify() returns MEJEPA_TCT_PROVENANCE_MISMATCH for the mismatched embedder; constellation NOT used; rebuild required</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-TCT-11" req_ref="REQ-TCT-13">
    <scenario>Rolling window has fewer than 30 verdicts (system just started; not enough data)</scenario>
    <expected_behavior>constellation_violation_rate returns Option::None; SPEC-UTML's compute_delta_xi treats predictor-side defense as inactive (defaults to 0.0 rate, ΔΞ shrinks accordingly per its insufficient-samples policy)</expected_behavior>
    <priority>medium</priority>
  </edge_case>

  <edge_case id="EC-TCT-12" req_ref="REQ-TCT-08">
    <scenario>Predicted panel contains a NaN/Inf in some embedder slot (instrument upstream bug)</scenario>
    <expected_behavior>Gτ rejects with MEJEPA_INFER_DIM_MISMATCH (per constitution &lt;failure_taxonomy&gt;) — NaN cosines fail the comparison fail-closed; the bug is escalated upstream to the instrument; verify() does NOT silently substitute zeros</expected_behavior>
    <priority>critical</priority>
  </edge_case>

  <edge_case id="EC-TCT-13" req_ref="REQ-TCT-07">
    <scenario>Predictor early in training where most predictions fail Gτ (no usable training signal due to over-rejection)</scenario>
    <expected_behavior>Per doc 13 §10 risk #6: anneal τ_m from 0 (permissive) to calibrated value over first 10K training steps; UTML §IX classifier suppresses "Latent-Collapsing" diagnosis until annealing completes; FSV records the anneal schedule</expected_behavior>
    <priority>high</priority>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-TCT-01" req_ref="REQ-TCT-08">
    <condition>Gτ rejected the predicted panel; one or more embedders below threshold</condition>
    <user_message>"Constellation guard rejected this prediction. Failed embedders: {list}. Failed chunks: {list}."</user_message>
    <internal_message>"MEJEPA_INFER_CONSTELLATION_VIOLATION embedder=E_m observed_cos=X threshold=Y (chunk=Function:apply_filter@tests/test_units.py:124-167)"</internal_message>
    <recovery_action>Operator (or agent) inspects violating_embedders + violating_chunks; targets repair on those chunks specifically; re-runs verify() on the repaired patch</recovery_action>
  </error>

  <error id="ERR-TCT-02" req_ref="REQ-TCT-11">
    <condition>Constellation is older than configured max-age</condition>
    <user_message>"Constellation is {age_days} days old (max {max_age_days}). Refusing to use stale centroids without operator override."</user_message>
    <internal_message>"MEJEPA_INFER_GTAU_STALE_CONSTELLATION frozen_at={iso8601} age_days={N} max_age_days={M}"</internal_message>
    <recovery_action>Trigger constellation refit OR set MEJEPA_TCT_ALLOW_STALE=true (audit-logged) to proceed</recovery_action>
  </error>

  <error id="ERR-TCT-03" req_ref="REQ-TCT-03">
    <condition>Cell has &lt; 5 supporting samples even after Bayesian shrinkage</condition>
    <user_message>"Insufficient samples for cell ({mutation}, {entity_type}, {language}, {embedder}): N={N}. Add corpus entries before retrying."</user_message>
    <internal_message>"MEJEPA_TCT_INSUFFICIENT_SAMPLES cell=({mutation}, {entity_type}, {language}, {embedder}) N={N} aggregate_attempted=({language|entity|category})"</internal_message>
    <recovery_action>Add corpus entries for the cell OR mark cell as out-of-distribution + skip during construction</recovery_action>
  </error>

  <error id="ERR-TCT-04" req_ref="REQ-TCT-04">
    <condition>Threshold calibration FPR &gt; 5% on held-out slice for some (embedder, language) cell with sufficient samples</condition>
    <user_message>"Threshold calibration failed: {cell} empirical FPR = {observed_fpr} exceeds target {target_fpr}."</user_message>
    <internal_message>"MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL cells_failed=[{list}] observed_fpr=[{list}] target_fpr=0.05"</internal_message>
    <recovery_action>Investigate: insufficient validation samples, embedder drift, corpus mismatch; rebuild constellation with cleaner data</recovery_action>
  </error>

  <error id="ERR-TCT-05" req_ref="REQ-TCT-12">
    <condition>Constellation embedder version SHA-256 does not match runtime-loaded embedder</condition>
    <user_message>"Constellation provenance mismatch: embedder {E_m} expected SHA={A} but runtime loaded SHA={B}. Rebuild constellation."</user_message>
    <internal_message>"MEJEPA_TCT_PROVENANCE_MISMATCH embedder={E_m} expected_sha={A} observed_sha={B}"</internal_message>
    <recovery_action>Rebuild constellation against current embedder weight digests OR roll back the embedder weights</recovery_action>
  </error>

  <error id="ERR-TCT-06" req_ref="REQ-TCT-08">
    <condition>Predicted panel contains NaN/Inf in some embedder slot</condition>
    <user_message>"Predicted panel has non-finite values in slot {slot}. Cannot run Gτ."</user_message>
    <internal_message>"MEJEPA_INFER_DIM_MISMATCH non_finite_panel slot={slot} embedder={E_m}"</internal_message>
    <recovery_action>Fix upstream instrument bug; do not silently substitute zeros</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="TctConstellation">
    <field name="per_category_centroids" type="BTreeMap&lt;MutationCategory, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;" required="true">
      <description>L2-normalized panel-level centroid per (mutation_category × embedder); 8 × 21 = 168 cells</description>
      <constraints>Every Vec&lt;f32&gt; L2-normalized to unit length within ULP; finite; length matches embedder dim</constraints>
    </field>
    <field name="per_language_centroids" type="BTreeMap&lt;(Language, MutationCategory), BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;" required="true">
      <description>L2-normalized cross-language centroid; up to 9 × 8 × 21 = 1,512 cells</description>
      <constraints>L2-normalized, finite</constraints>
    </field>
    <field name="outcome_centroids" type="BTreeMap&lt;OracleOutcome, BTreeMap&lt;EmbedderId, Vec&lt;f32&gt;&gt;&gt;" required="true">
      <description>L2-normalized centroid per (oracle_outcome × embedder); 4 × 21 = 84 cells. Pass-centroid is the predicted-reality target ME-JEPA's predictor optimizes toward (per doc 13 §2.1)</description>
      <constraints>L2-normalized, finite</constraints>
    </field>
    <field name="per_chunk_type_centroids" type="BTreeMap&lt;(MutationCategory, EntityType, Language, EmbedderId), Vec&lt;f32&gt;&gt;" required="true">
      <description>L2-normalized per-chunk-type centroid; up to 8 × 13 × 11 × 21 = 24,024 cells with Bayesian shrinkage on N &lt; 50</description>
      <constraints>L2-normalized, finite; cells with N &lt; 50 collapsed via shrinkage; cells with N &lt; 5 fail closed</constraints>
    </field>
    <field name="thresholds" type="BTreeMap&lt;EmbedderId, f32&gt;" required="true">
      <description>Panel-level τ_m calibrated to ≤ 5% FPR on held-out KnownGood</description>
      <constraints>21 keys; each value ∈ [-1, 1]</constraints>
    </field>
    <field name="per_chunk_type_thresholds" type="BTreeMap&lt;(EntityType, EmbedderId), f32&gt;" required="true">
      <description>Per-chunk-type τ_m calibrated when (entity_type, embedder) cell has ≥ 50 samples</description>
      <constraints>Sharper than panel-level when present</constraints>
    </field>
    <field name="corpus_provenance" type="CorpusProvenance" required="true">
      <description>SHA-256 of source corpus + per-embedder weight SHA-256 + frozen_at ISO 8601 + code_version git SHA</description>
      <constraints>All SHAs are 32-byte arrays (64 hex)</constraints>
    </field>
    <field name="frozen_at" type="SystemTime" required="true">
      <description>UTC instant when this constellation was finalized</description>
      <constraints>ISO 8601 serialization; cannot be in the future</constraints>
    </field>
    <field name="version_id" type="[u8; 32]" required="true">
      <description>SHA-256 of deterministic-serialization of all the above fields; primary key in CF_MEJEPA_CONSTELLATION</description>
      <constraints>Computed at construction; immutable post-construction</constraints>
    </field>
  </entity>

  <entity name="ChunkId">
    <field name="file_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the post-edit file containing the chunk</description>
      <constraints>32 bytes</constraints>
    </field>
    <field name="language" type="Language" required="true">
      <description>One of: rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php</description>
      <constraints>Must be in the 11-language set</constraints>
    </field>
    <field name="entity_type" type="EntityType" required="true">
      <description>Function | Method | Class | Struct | Module | Impl | Trait | Interface | Macro | Enum | Union | Namespace | TopLevelStmt</description>
      <constraints>13 variants per SPEC-AST-CHUNKING</constraints>
    </field>
    <field name="line_start" type="u32" required="true">
      <description>1-indexed line number where chunk begins</description>
      <constraints>≥ 1</constraints>
    </field>
    <field name="line_end" type="u32" required="true">
      <description>1-indexed line number where chunk ends (inclusive)</description>
      <constraints>≥ line_start</constraints>
    </field>
    <field name="parent_chain_hash" type="[u8; 16]" required="true">
      <description>BLAKE3-128 hash of the chunk's parent_chain (per SPEC-AST-CHUNKING: enclosing module/class/impl block path)</description>
      <constraints>16 bytes; deterministic from parent_chain</constraints>
    </field>
  </entity>

  <entity name="GtauViolation">
    <field name="embedder" type="EmbedderId" required="true">
      <description>Which of E1-E21 failed</description>
      <constraints>One of 21 enum variants</constraints>
    </field>
    <field name="observed_cos" type="f32" required="true">
      <description>Observed cosine to centroid</description>
      <constraints>∈ [-1, 1]; finite</constraints>
    </field>
    <field name="threshold" type="f32" required="true">
      <description>Calibrated τ_m for this (embedder, entity_type, language) cell</description>
      <constraints>∈ [-1, 1]; finite</constraints>
    </field>
    <field name="violating_chunk" type="Option&lt;ChunkId&gt;" required="false">
      <description>The specific chunk that failed; None when violation is panel-level only (no chunk decomposition was performed)</description>
      <constraints>Some when chunk-level Gτ ran; None when only panel-level Gτ ran</constraints>
    </field>
  </entity>

  <entity name="VerdictGuardRejected">
    <field name="violating_embedders" type="Vec&lt;GtauViolation&gt;" required="true">
      <description>Every embedder that fell below threshold</description>
      <constraints>Non-empty (otherwise Gτ would not have rejected)</constraints>
    </field>
    <field name="predictor_predicted" type="PredictorOutput" required="true">
      <description>The predictor's verdict before Gτ ran (predicted oracle outcome + per-test probabilities + classification head label)</description>
      <constraints>Captures the predictor's "answer" so the rejection is fully diagnosable</constraints>
    </field>
    <field name="violating_chunks" type="Vec&lt;ChunkId&gt;" required="true">
      <description>Per-chunk localization (per doc 14 §8.1)</description>
      <constraints>Possibly empty when only panel-level Gτ ran; populated when chunk-level Gτ ran</constraints>
    </field>
  </entity>

  <entity name="CorpusProvenance">
    <field name="corpus_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the deterministic-serialization of the source corpus snapshot used at construction</description>
      <constraints>32 bytes</constraints>
    </field>
    <field name="embedder_versions" type="BTreeMap&lt;EmbedderId, [u8; 32]&gt;" required="true">
      <description>Per-embedder weight SHA-256 digest at construction time; checked at constellation load</description>
      <constraints>21 keys; each value 32 bytes</constraints>
    </field>
    <field name="frozen_at" type="SystemTime" required="true">
      <description>ISO 8601 UTC instant</description>
      <constraints>Cannot be in the future</constraints>
    </field>
    <field name="code_version" type="String" required="true">
      <description>Git SHA of `context-graph-mejepa-tct` crate at construction time</description>
      <constraints>40-hex git commit hash</constraints>
    </field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-TCT-01" category="performance">
    <description>Gτ test latency p50 with all 21 embedders cached</description>
    <metric>&lt; 10 ms per output (per task brief budget; aligns with constitution &lt;performance_budgets&gt; single_prediction_latency_p50)</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-02" category="performance">
    <description>Constellation construction wall-clock</description>
    <metric>O(corpus_size · 21 embedders); persists ~30 MB after Bayesian shrinkage on Phase 0 corpus</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-03" category="performance">
    <description>Constellation in-RAM footprint at inference</description>
    <metric>~30 MB total loaded (centroids + thresholds + provenance metadata); fits within constitution &lt;performance_budgets&gt; vram_steady_state allowance</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-04" category="reliability">
    <description>Determinism: same predicted_panel + same constellation = byte-equal Gτ output</description>
    <metric>Byte-equal across 1,000 repeated invocations on identical inputs; verdict reproducibility ≥ 99.9% per doc 13 §11 falsifier #8; nondeterminism &gt; 0.1% is a critical bug</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-05" category="reliability">
    <description>External-verifier reproducibility</description>
    <metric>A third party with the frozen embedders + the persisted constellation reproduces gtau_satisfied + per-violation cosines to within f32 ULP</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-06" category="security">
    <description>Constellation provenance enforcement</description>
    <metric>0 occurrences of constellation load with mismatched embedder digests in production logs; failure to refuse is a critical bug</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-07" category="security">
    <description>FSV evidence file permissions</description>
    <metric>Mode 0o600 on every /tmp/contextgraph-mejepa-tct-fsv/* file (per SEC-MEJEPA-07)</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-08" category="reliability">
    <description>Threshold-calibration FPR bound</description>
    <metric>≤ 5% empirical Gτ FPR per (embedder, language) cell with ≥ 30 held-out KnownGood samples; exceeded cells fail closed via MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-09" category="usability">
    <description>Per-chunk localization granularity</description>
    <metric>Every GuardRejected verdict includes violating_chunks: Vec&lt;ChunkId&gt; with sufficient metadata (file_sha, language, entity_type, line_start, line_end, parent_chain_hash) for surgical agent re-attempts; per doc 14 §8.1</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-TCT-10" category="reliability">
    <description>Frozen-instrument invariant during constellation construction + Gτ</description>
    <metric>Zero gradient on any embedder weight or centroid during construction OR L_constellation_centroid backward; any nonzero gradient is a CRITICAL bug per constitution &lt;tct_invariants&gt; rule 1 + &lt;anti_patterns&gt; "No learnable target encoder"</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>SPEC-CORPUS</name>
    <description>Phase 0 mutation corpus producing labeled (panel, mutation_category, oracle_outcome, language, entity_type) tuples; ≥ 2,400 entries (8 × 300 SWE-bench Lite tasks) at completion</description>
    <impact>Without ≥ 2,000 labeled entries, centroid construction in cells with adequate sample support is not feasible — TCT Phase 1 blocked</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-AST-CHUNKING</name>
    <description>11-language AST-perfect chunker producing (entity_type, parent_chain, line_start, line_end, file_sha) for every chunk</description>
    <impact>Without AST-perfect chunks, per-chunk-type centroids cannot be constructed and chunk-level Gτ localization is impossible</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-EMBEDDERS</name>
    <description>21 frozen embedders with SHA-pinned weight digests in models_config.toml</description>
    <impact>The whole TCT machinery rests on the frozen-embedder invariant; without SHA pins, provenance enforcement (REQ-TCT-12) is impossible</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-INSTRUMENTS</name>
    <description>5,120-d Panel + 15-slot layout that TCT centroids are computed over</description>
    <impact>Centroid dimensionality + slot-extent contract must match Panel layout; mismatch is a critical determinism bug</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PHASE2-PREDICTOR</name>
    <description>The predictor that outputs the predicted panel ŷ being guarded by Gτ + the predicted-class label feeding centroid lookup</description>
    <impact>Without a predicted_class output, Gτ cannot pick the right centroid; without a predicted panel, there is nothing to guard</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PHASE3-TRAINING</name>
    <description>Hosts the L_constellation_centroid loss term implementation; TCT-CONSTELLATION provides the centroid lookup + hinge primitive but the loss-surface integration lives there</description>
    <impact>Without Phase 3 integration, the predictor never sees gradient pushing it onto the constellation</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PHASE4-INFERENCE</name>
    <description>Hosts the verify() API call site that invokes Gτ as the fourth gate</description>
    <impact>Without verify() integration, Gτ never runs at inference; the runtime guard role is inactive</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-UTML</name>
    <description>Consumer of constellation_violation_rate (ΔΞ component) + Gτ-flagged chunks (Max aggregator selection)</description>
    <impact>Without UTML consumption, sustained guard rejection does not feed back into L_step training signal — the predictor-side defense diagnostic is inactive</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-WITNESS</name>
    <description>Provenance signing of frozen constellation artifacts via ed25519 + SHAKE-256 chain</description>
    <impact>Without witness signing, constellation tampering between construction and verify() is undetectable</impact>
  </dependency>

  <dependency type="external">
    <name>RocksDB column family CF_MEJEPA_CONSTELLATION</name>
    <description>Persists the frozen constellation artifact</description>
    <impact>Without the CF, constellation cannot persist across process restart; in-RAM only (acceptable for prototyping but not production)</impact>
  </dependency>

  <dependency type="external">
    <name>tree-sitter (11 grammars)</name>
    <description>Per constitution &lt;tech_stack&gt;: rust, python, javascript, typescript, go, java, c, cpp, c-sharp, ruby, php parsers wired through SPEC-AST-CHUNKING</description>
    <impact>Without all 11 parsers, per-language centroids are missing for unparsable languages — cross-language coverage degrades</impact>
  </dependency>

</dependencies>

<out_of_scope>

  <item>L_constellation_centroid loss surface integration into the multi-objective training loss — handled by SPEC-PHASE3-TRAINING; this spec defines the lookup + hinge primitive only</item>
  <item>verify() API call site invoking Gτ as the fourth gate — handled by SPEC-PHASE4-INFERENCE; this spec defines the gtau_check + gtau_check_chunks function contracts only</item>
  <item>The full Verdict enum (Pass / Fail / OutOfDistribution / Abstain / GuardRejected) — handled by SPEC-PHASE4-INFERENCE; this spec defines only the GuardRejected variant payload</item>
  <item>The conformal-interval / OOD-score / L_step-health gates — handled by SPEC-PHASE4-INFERENCE; Gτ is one of four parallel gates</item>
  <item>The Phase 5 self-healing controller's trigger logic for refit on sustained Gτ rejection — handled by SPEC-PHASE5-SELF-HEALING; this spec defines the constellation_violation_rate consumer contract only</item>
  <item>The reward formula upgrade `Reward = 0.4·calibrated_confidence + 0.3·mean(L_step·ΔΞ) + 0.3·gtau_pass_rate` — handled by SPEC-PHASE5-SELF-HEALING (or future SPEC-REWARD-FORMULA); this spec exposes gtau_pass_rate as a queryable metric only</item>
  <item>The 21-embedder substrate construction, frozenness enforcement, and SHA-pinned weight loading — handled by SPEC-EMBEDDERS; this spec consumes them as a contract</item>
  <item>The 11-language AST chunker construction — handled by SPEC-AST-CHUNKING; this spec consumes ChunkId metadata as a contract</item>
  <item>The 5,120-d Panel + 15-slot layout itself — handled by SPEC-INSTRUMENTS; this spec consumes Panel slot offsets as a contract</item>
  <item>The Phase 0 mutation corpus generation (8 categories × 300 tasks × 9 languages) — handled by SPEC-CORPUS; this spec consumes corpus tuples as a contract</item>
  <item>The cross-embedder pairwise MI / Jaccard signals (210 pair signals at N=21) — these are SPEC-INSTRUMENTS / SPEC-UTML signal-supply territory, not constellation territory</item>

</out_of_scope>

<test_plan>

  <test_case id="TC-TCT-01" type="unit" req_ref="REQ-TCT-01">
    <description>TctConstellation struct round-trips through serde with byte-equal SHA-256 on two independent serializations</description>
    <preconditions>Synthesized 168-cell constellation with deterministic centroid values</preconditions>
    <inputs>Synthesized TctConstellation with corpus_provenance.code_version = "test"</inputs>
    <expected_result>Two serde_cbor (or bincode2) serializations produce byte-equal output; SHA-256 matches</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-02" type="integration" req_ref="REQ-TCT-02">
    <description>Two independent ConstellationBuilder runs on the same corpus yield byte-equal constellation</description>
    <preconditions>Synthesized corpus with deterministic panel values; same code_version + same embedder_versions</preconditions>
    <inputs>500 corpus entries spanning 8 categories × 5 languages × 5 entity types</inputs>
    <expected_result>Build A and Build B produce byte-equal SHA-256-of-serialized constellation</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-03" type="unit" req_ref="REQ-TCT-03">
    <description>Bayesian shrinkage on a per-chunk-type cell with N = 30 collapses to language-aggregate</description>
    <preconditions>Synthesized cell with N = 30 (below 50 threshold) but language-aggregate has N = 200</preconditions>
    <inputs>Cell coordinate (SubtleFlip, Function, Python, E11) with N = 30; language-aggregate (SubtleFlip, *, Python, E11) with N = 200</inputs>
    <expected_result>Cell centroid replaced with language-aggregate centroid; FSV record notes the shrinkage decision</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-04" type="unit" req_ref="REQ-TCT-03">
    <description>Cell with N = 4 fails closed with MEJEPA_TCT_INSUFFICIENT_SAMPLES</description>
    <preconditions>Synthesized cell with N = 4 (below hard threshold of 5); language-aggregate also insufficient</preconditions>
    <inputs>Cell coordinate (CompileError, Macro, Ruby, E12) with N = 4</inputs>
    <expected_result>Build returns MEJEPA_TCT_INSUFFICIENT_SAMPLES; constellation NOT persisted</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-05" type="integration" req_ref="REQ-TCT-04">
    <description>Threshold calibration achieves ≤ 5% empirical FPR on held-out KnownGood</description>
    <preconditions>Constellation constructed; 30 held-out KnownGood mutations per language</preconditions>
    <inputs>Held-out validation slice; threshold sweep over 100 candidates per (embedder, language) cell</inputs>
    <expected_result>For every (embedder, language) cell with ≥ 30 samples, empirical FPR ≤ 0.05; thresholds persisted</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-06" type="integration" req_ref="REQ-TCT-04">
    <description>Threshold calibration that exceeds 5% FPR fails closed</description>
    <preconditions>Synthesized validation slice where some (embedder, language) cell has unreachable target FPR</preconditions>
    <inputs>Validation slice with high noise on (E5, Python)</inputs>
    <expected_result>Build returns MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL listing failing cells; constellation NOT persisted</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-07" type="unit" req_ref="REQ-TCT-08">
    <description>Gτ accepts a panel that meets τ_m on every embedder</description>
    <preconditions>Synthesized panel with cos(slot, centroid) = τ_m + 0.01 on every embedder</preconditions>
    <inputs>Predicted panel + KnownGood centroid set + τ_m = 0.7 across embedders</inputs>
    <expected_result>gtau_satisfied = true; violations is empty</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-08" type="unit" req_ref="REQ-TCT-08">
    <description>Gτ rejects a panel that fails τ_m on at least one embedder</description>
    <preconditions>Synthesized panel with cos(slot_E11, centroid_E11) = τ_E11 - 0.05; all other embedders pass</preconditions>
    <inputs>Predicted panel + KnownGood centroid set + τ_m = 0.7 across embedders</inputs>
    <expected_result>gtau_satisfied = false; violations contains exactly one entry for E11 with observed_cos = 0.65, threshold = 0.70</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-09" type="unit" req_ref="REQ-TCT-08">
    <description>Gτ boundary condition: cos == τ_m exactly accepts (inequality is inclusive)</description>
    <preconditions>Synthesized panel with cos(slot, centroid) = τ_m exactly on every embedder</preconditions>
    <inputs>Predicted panel + KnownGood centroid set</inputs>
    <expected_result>gtau_satisfied = true (boundary inclusive)</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-TCT-10" type="integration" req_ref="REQ-TCT-09">
    <description>Chunk-level Gτ localizes rejection to a specific chunk</description>
    <preconditions>Synthesized 3-chunk patch where chunk_2 (Function:apply_filter) fails for E11</preconditions>
    <inputs>3-chunk patch + per-chunk panels + per-chunk-type centroid set with thresholds</inputs>
    <expected_result>violating_chunks contains exactly chunk_2's ChunkId; chunk_1 and chunk_3 not flagged</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-11" type="integration" req_ref="REQ-TCT-10">
    <description>Verdict::GuardRejected round-trips through CF_MEJEPA_GUARD_DECISIONS</description>
    <preconditions>Verdict::GuardRejected with violating_embedders + violating_chunks populated</preconditions>
    <inputs>Synthesized verdict</inputs>
    <expected_result>RocksDB write + read returns byte-equal verdict; per-violation observed_cos within f32 ULP</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-12" type="integration" req_ref="REQ-TCT-11">
    <description>verify() refuses stale constellation</description>
    <preconditions>Synthesized constellation with frozen_at = 91 days ago</preconditions>
    <inputs>verify() call with default MEJEPA_TCT_MAX_AGE_DAYS = 90</inputs>
    <expected_result>verify() returns MEJEPA_INFER_GTAU_STALE_CONSTELLATION; verdict NOT produced</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-13" type="integration" req_ref="REQ-TCT-11">
    <description>verify() accepts stale constellation under audit-logged operator override</description>
    <preconditions>Synthesized constellation with frozen_at = 91 days ago; MEJEPA_TCT_ALLOW_STALE = true</preconditions>
    <inputs>verify() call with override</inputs>
    <expected_result>verify() proceeds; audit-log entry written to ./memory/journal/ + witness chain</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-TCT-14" type="integration" req_ref="REQ-TCT-12">
    <description>Constellation load fails with provenance mismatch</description>
    <preconditions>Synthesized constellation with embedder_versions[E11] = SHA_A; runtime loads E11 with SHA_B</preconditions>
    <inputs>Constellation load attempt</inputs>
    <expected_result>Load returns MEJEPA_TCT_PROVENANCE_MISMATCH carrying expected SHA_A + observed SHA_B</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-15" type="unit" req_ref="REQ-TCT-07">
    <description>L_constellation_centroid hinge boundary: cos == τ_m yields exact 0.0 contribution</description>
    <preconditions>Synthesized predicted_panel where cos(slot_E1, centroid_E1) = τ_E1 exactly (within f32 ULP)</preconditions>
    <inputs>predicted_panel + (predicted_class, language, entity_type) tuple</inputs>
    <expected_result>Per-embedder hinge for E1 is 0.0 exactly; no spurious gradient generated</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-TCT-16" type="unit" req_ref="REQ-TCT-07">
    <description>L_constellation_centroid backward pass: zero gradient through centroids and embedder weights</description>
    <preconditions>Differentiable predicted_panel with requires_grad = true; centroid + embedder weights detached</preconditions>
    <inputs>Forward pass + backward pass through constellation_hinge primitive</inputs>
    <expected_result>Gradient on predicted_panel is non-zero; gradient on centroid and embedder weights is exactly 0.0; if any nonzero is detected, fail with MEJEPA_INSTR_FROZEN_VIOLATION (per SPEC-INSTRUMENTS AC-INSTR-03-01)</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-17" type="integration" req_ref="REQ-TCT-13">
    <description>constellation_violation_rate computation over rolling window</description>
    <preconditions>CF_MEJEPA_GUARD_DECISIONS populated with 100 verdicts: 10 GuardRejected + 90 Pass/Fail/OOD</preconditions>
    <inputs>RollingWindow = 100 calls</inputs>
    <expected_result>Returned rate = 0.10; UTML's ΔΞ.constellation_violation_rate consumer reads 0.10</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-18" type="integration" req_ref="REQ-TCT-13">
    <description>constellation_violation_rate returns Option::None when window has &lt; 30 verdicts</description>
    <preconditions>CF_MEJEPA_GUARD_DECISIONS populated with 25 verdicts</preconditions>
    <inputs>RollingWindow = 100 calls (only 25 actual)</inputs>
    <expected_result>Returned Option::None; SPEC-UTML's compute_delta_xi treats predictor-side defense as inactive (handles None per its insufficient-samples policy)</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-TCT-19" type="e2e" req_ref="REQ-TCT-14">
    <description>FSV harness end-to-end produces all four evidence files and asserts on each</description>
    <preconditions>Synthesized Phase 0 corpus with 500 entries</preconditions>
    <inputs>Run the FSV harness binary</inputs>
    <expected_result>/tmp/contextgraph-mejepa-tct-fsv/{constellation-evidence.json, gtau-evidence.json, threshold-calibration-evidence.json, chunk-level-rejection-evidence.json} all present, mode 0o600, round-trip through serde, assertions pass</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-TCT-20" type="e2e" req_ref="REQ-TCT-15">
    <description>mcp__cgreality__mejepa_constellation_inspect returns metadata summary</description>
    <preconditions>Loaded constellation with 168 panel-level + 24,024 per-chunk-type cells (post-shrinkage)</preconditions>
    <inputs>MCP tool invocation</inputs>
    <expected_result>JSON output contains version_id, corpus_provenance, frozen_at, cell counts per (category × language × entity_type), sample-support histogram, threshold table; no raw centroid byte arrays</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-TCT-21" type="e2e" req_ref="REQ-TCT-08">
    <description>External-verifier reproducibility on persisted Verdict::GuardRejected</description>
    <preconditions>Persisted Verdict::GuardRejected + matching constellation in CF_MEJEPA_CONSTELLATION + matching embedder weights</preconditions>
    <inputs>Cold-start third-party verification: load embedders, load constellation, recompute Gτ on persisted panel</inputs>
    <expected_result>Reproduced gtau_satisfied + per-violation cosines byte-equal to persisted values; verdict reproducibility ≥ 99.9%</expected_result>
    <priority>critical</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-TCT-01" status="open" assignee="operator">
    <text>What is the canonical EntityType enum cardinality? Doc 14 §2.2 lists 6 in the embedded enum example (Function, Method, Class, Struct, Module, Impl) but the task brief and constitution &lt;tct_invariants&gt; reference 13 entity types in cell math (8 × 13 × 11 × 21 = 24,024). Should the canonical enum extend to {Function, Method, Class, Struct, Impl, Module, Trait, Interface, Macro, Enum, Union, Namespace, TopLevelStmt} = 13 variants?</text>
    <context>Cell-count math depends on this. SPEC-AST-CHUNKING is the authoritative owner; resolution feeds into REQ-TCT-09 (chunk-level Gτ falls back via SPEC-AST-CHUNKING mapping rules).</context>
    <resolution>Pending operator confirmation; provisional answer in this spec is 13 variants per the constitution &lt;tct_invariants&gt; cell math.</resolution>
  </question>

  <question id="Q-TCT-02" status="open" assignee="operator">
    <text>What is the canonical rolling window for constellation_violation_rate computation? Doc 13 §3 says "rolling window" without specifying duration; the task brief mentions "rolling window" but no defaults; SPEC-UTML's REQ-UTML-12 consumer expects a defined contract.</text>
    <context>The default window affects how quickly "Latent-Collapsing" interventions trigger. Too short = false alarms; too long = slow response.</context>
    <resolution>Pending; provisional defaults 1,000 calls or 24 hours (whichever is smaller); configurable via env vars MEJEPA_TCT_RATE_WINDOW_SIZE + MEJEPA_TCT_RATE_WINDOW_HOURS.</resolution>
  </question>

  <question id="Q-TCT-03" status="open" assignee="operator">
    <text>Should Bayesian shrinkage's aggregate hierarchy be (chunk-type → entity-aggregate → language-aggregate → category-aggregate) or (chunk-type → language-aggregate → entity-aggregate → category-aggregate)? Doc 13 §10 risk #5 says "language-aggregate or entity-aggregate" without ordering.</text>
    <context>Order affects which centroid a sparse cell falls back to; this shapes Gτ behavior on edge-case patches.</context>
    <resolution>Pending; provisional rule (per BR-TCT-02): language-aggregate first, then entity-aggregate, then per-mutation-category global.</resolution>
  </question>

  <question id="Q-TCT-04" status="open" assignee="operator">
    <text>How does Gτ handle a multi-language patch (e.g., a Python file plus a Rust file in the same patch)? Per-language constellations apply per-chunk; what about cross-language coherence checks?</text>
    <context>Doc 13 §4.3 frames cross-language as "constellation-set-selection problems, not retraining problems." Multi-language patches are a real case in cross-language refactors.</context>
    <resolution>Pending; provisional answer: Gτ per chunk uses chunk.language; multi-language patch is just a sequence of single-language chunk verdicts.</resolution>
  </question>

  <question id="Q-TCT-05" status="open" assignee="operator">
    <text>Should the constellation refit on Phase 5 trigger re-calibrate τ_m on a fresh held-out slice, or reuse the previous τ_m? Doc 13 §10 risk #2 mitigation says "re-calibrate τ_m on live-attempt holdout slice during Phase 5 self-healing."</text>
    <context>Re-calibration cost: every refit re-runs threshold sweep on fresh data.</context>
    <resolution>Pending; provisional answer: yes, re-calibrate on fresh held-out slice (fail-closed if FPR &gt; 5% on the new slice via MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL).</resolution>
  </question>

</open_questions>

<glossary>
  <term name="TCT">Teleological Constellation Training. Operator-published framework (Royse 2026c, DOI 10.13140/RG.2.2.24370.57288) for using frozen multi-modal embedder ensembles in three simultaneous roles: labeller (centroid construction), loss target (constellation-attraction loss), and runtime guard (Gτ test).</term>
  <term name="CP">The frozen multi-modal centroid set produced by TCT Phase 1 — the per-(category × language × entity_type × embedder) collection of L2-normalized centroid vectors.</term>
  <term name="Gτ">The deterministic geometric guard `Gτ(ŷ; CP) = ⟦∀m : cos(φ_m(ŷ), c_m) ≥ τ_m⟧`. Returns true iff the predicted panel embeds within cosine threshold of the (predicted-class, language, entity_type) centroid in EVERY embedder space.</term>
  <term name="τ_m">The per-embedder calibrated cosine threshold below which Gτ rejects. Calibrated on held-out KnownGood mutations to bound the false-positive rate at ≤ 5%.</term>
  <term name="L_constellation_centroid">The margin-based per-embedder geometric loss term added to ME-JEPA's multi-objective loss surface in Phase 3 training. `Σ_m max(0, τ_m − cos(g_φ(panel)_m, c_m_{predicted_class}))` summed across all 21 embedders.</term>
  <term name="ChunkId">The localization tuple (file_sha, language, entity_type, line_start, line_end, parent_chain_hash) that identifies a specific AST-perfect chunk for chunk-level Gτ rejection.</term>
  <term name="GuardRejected">The Verdict variant emitted when Gτ rejects a predicted panel. Carries violating_embedders, predictor_predicted, and violating_chunks for surgical agent re-attempts.</term>
  <term name="constellation_violation_rate">The rolling fraction of recent verify() calls whose Gτ rejected the predicted panel. Consumed by SPEC-UTML's compute_delta_xi as the third predictor-side defense diagnostic.</term>
  <term name="Bayesian shrinkage">The technique that, when a per-(mutation × entity_type × language × embedder) cell has insufficient sample support (N &lt; 50), collapses the cell's centroid to a higher-aggregate centroid (language-aggregate or entity-aggregate) per BR-TCT-02.</term>
  <term name="frozen embedder">An embedder whose weights are loaded once with SHA-pinned digest, never updated, and whose gradient is detached from the autograd graph. The architectural property that makes ME-JEPA's `ΔΞ.target_collapse = 0 by construction` (per doc 11 §5.2).</term>
  <term name="three-role property">The TCT architectural move (per doc 13 §3.4) of using the same 21 frozen embedders simultaneously as: (1) labeller during Phase 1 centroid construction, (2) loss target during Phase 3 L_constellation_centroid, (3) runtime guard during Phase 4 Gτ — without any parameter update across roles.</term>
  <term name="DDA">Dimensional Decomposition Algorithm. Per Royse 2026c §3.1, the counting identity `n raw inputs × N frozen embedders → up to n · (N + (N choose 2)) structured supervisory signals`. At N=21, every input yields up to 462 structured signals.</term>
  <term name="meaning compression">Per Royse 2026c §3.2, structured signal density per unit of raw data (signals-per-raw-byte). Orthogonal to bit compression; multiplicative with it.</term>
  <term name="Latent-Collapsing">UTML §IX coaching cycle classifier verdict triggered when constellation_violation_rate sustains above threshold over the rolling window. Indicates centroids are stale; refit is required.</term>
  <term name="per-output verifiability">The structural property that every Gτ rejection is independently re-verifiable by anyone with the frozen embedders + the persisted constellation, in O(N · d_max) time, with no learned model in the test path. Per Royse 2026c §1 + §7, this is the structural advantage over scalar-reward alignment (RLHF/DPO/Constitutional AI).</term>
</glossary>

</functional_spec>
```
