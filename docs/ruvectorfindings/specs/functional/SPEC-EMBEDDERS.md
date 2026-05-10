# SPEC-EMBEDDERS — Functional Specification

> **📊 2026-05-10 STATUS:** PARTIAL — see ../implementation-plan.md for the full Phase 1 status.

> **Level 2 (functional spec).** Defines WHAT the EMBEDDERS domain must do. For HOW, see `specs/technical/TECH-EMBEDDERS.md`. Constitution (Level 1) wins on conflict.

```xml
<functional_spec id="SPEC-EMBEDDERS" version="1.0">

<metadata>
  <title>EMBEDDERS — 21-Embedder Ensemble Wrapped Behind the Instrument Trait</title>
  <status>partial</status>
  <owner>ME-JEPA Phase 1 working group (cc-reality outer-loop optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-10</last_updated>
  <related_specs>
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>
    <spec_ref>SPEC-PANEL</spec_ref>
    <spec_ref>SPEC-PREDICTOR</spec_ref>
    <spec_ref>TECH-EMBEDDERS</spec_ref>
  </related_specs>
  <source_documents>
    <source type="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md</source>
    <source type="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md</source>
    <source type="design_doc">docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md §4</source>
    <source type="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §2.2 §3.5.5</source>
    <source type="design_doc">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md (post-pivot architecture)</source>
    <source type="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md (UTML L_step)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md</source>
    <source type="source_code">crates/context-graph-embeddings/src/types/model_id/core.rs (E1-E14)</source>
    <source type="source_code">crates/context-graph-embeddings/src/learner/mod.rs (E15-E21)</source>
    <source type="source_code">crates/context-graph-embeddings/src/training/{lora,distillation,multitask,optimizer,loss}.rs</source>
    <source type="source_code">crates/context-graph-mejepa-instruments/src/lib.rs (existing Instrument trait)</source>
  </source_documents>
</metadata>

<overview>
The EMBEDDERS domain wraps the existing 21-embedder ensemble (E1-E21) shipped in
`crates/context-graph-embeddings/` behind a unified `Instrument` trait, defines the
per-chunk-type routing table that maps `EntityType` to the active embedder set,
enforces a hard ~29 GB VRAM budget on the RTX 5090, and supplies the LoRA-based
fine-tuning surface required for Phase 5 self-healing.

**Why this exists.** Doc 09 originally specified 14 ME-JEPA instruments to be built
from scratch. Per doc 12, the shipped 21-embedder ensemble + its training infra
(LoRA, distillation, multitask, AdamW, InfoNCE) covers ~70% of that surface
already. The work shifts from "build 14 instruments" to "wrap 21 existing
embedders behind a single trait, route AST chunks to the right embedders, budget
VRAM, support LoRA on top." This domain produces the SOURCE vectors that
downstream instruments project into the 5,120-d panel; it does NOT itself own
panel slots (that is `SPEC-INSTRUMENTS`).

**Who benefits.** ME-JEPA's predictor (downstream consumer of the panel), the
TCT constellation construction (uses every embedder's centroids), the
`mejepa-shift-subscriber` (Phase 7, computes panels online), and the
self-healing loop (refreshes LoRA adapters when ΔΩ collapses) all depend on a
deterministic, fail-closed, VRAM-budgeted, version-pinned ensemble surface.

**High-level scope.** This spec covers (1) the `Instrument` trait wrapper around
each of E1-E21, (2) the per-chunk-type routing table (EntityType → active embedder
set), (3) VRAM budget enforcement and lazy-loading, (4) embedder weight loading
with SHA-256 digest verification per `SEC-MEJEPA-02`, (5) LoRA adapter management,
(6) cross-embedder coherence/spread metrics for ΔK / predictor_collapse_score,
(7) E17 AffectText on agent CoT for ΔΩ_outer. Out of scope: panel materialization
(SPEC-PANEL), predictor architecture (SPEC-PREDICTOR), constellation centroid
construction (SPEC-TCT), AST chunker boundaries (SPEC-AST-CHUNKING).
</overview>

<user_types>
  <user_type id="UT-01" name="ME-JEPA Predictor">
    <description>Downstream Phase 2-4 component that consumes panels assembled from embedder outputs.</description>
    <permissions>Read-only access to embedder outputs via `Instrument::encode` returns.</permissions>
    <goals>Receive deterministic, dim-correct, finite-valued latent vectors per slot, with byte-equal reproducibility for the same (input, weight_sha, code_version) tuple.</goals>
  </user_type>
  <user_type id="UT-02" name="Mejepa Shift Subscriber (Phase 7 §3.8)">
    <description>Tokio task that tails the cgreality shift log, recomputes panels post-edit, and feeds `mejepa.observe()`.</description>
    <permissions>Invoke the embedder ensemble in parallel batch=1 forward passes; cache outputs by file SHA.</permissions>
    <goals>Stay under the §3.8.6 latency budget (5s p50, 30s p99 per shift).</goals>
  </user_type>
  <user_type id="UT-03" name="TCT Constellation Builder">
    <description>Phase 4b component that constructs frozen centroids per (mutation_category, language, node_type, embedder).</description>
    <permissions>Batch-encode the entire 2,400-entry corpus through every embedder once.</permissions>
    <goals>Receive centroids byte-identical across re-runs given pinned weights.</goals>
  </user_type>
  <user_type id="UT-04" name="Self-Healing Loop (Phase 5)">
    <description>Background maintenance task that refreshes LoRA adapters when `ΔΩ.effective_plasticity &lt; 0.4`.</description>
    <permissions>Train LoRA adapters on E5/E11/E14 + the four custom embedders; never touches frozen base weights.</permissions>
    <goals>Restore plasticity without altering the frozen-base contract that protects the predictor's target invariance.</goals>
  </user_type>
  <user_type id="UT-05" name="Operator (Claude Code) via MCP">
    <description>Outer-loop optimizer accessing the ensemble via `mcp__cgreality__mejepa_*` MCP tools.</description>
    <permissions>Call `mejepa_load_learner_state`, `mejepa_compute_panel`, `mejepa_audit_pairwise_mi`; cannot directly mutate weights or LoRA state.</permissions>
    <goals>Surface diagnostics; trigger explicit lazy-loads of E15-E21; inspect coherence/spread.</goals>
  </user_type>
</user_types>

<user_stories>

<story id="US-EMBED-01" priority="must-have">
  <narrative>
    <as_a>ME-JEPA predictor</as_a>
    <i_want_to>encode any of the 21 embedders' outputs through a single uniform `Instrument::encode` interface</i_want_to>
    <so_that>panel slot wiring is decoupled from embedder-specific tokenization, weight loading, and forward-pass internals</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-01-01">
      <given>An EmbedderId in {E1..E14, E15..E21} and a corresponding `EmbedderInput` (text / chunk / sequence / sensor samples)</given>
      <when>The caller invokes `EmbedderInstrument::encode(&input)` on the loaded ensemble</when>
      <then>The output is a `Vec&lt;f32&gt;` of length equal to that embedder's `projected_dimension()` (or `output_dimension()` for E15-E21), all values finite, byte-equal across calls with the same input + weight_sha + code_version</then>
    </criterion>
    <criterion id="AC-EMBED-01-02">
      <given>An embedder weight file whose SHA-256 digest does not match the pin in `models_config.toml`</given>
      <when>The ensemble attempts to load that embedder</when>
      <then>Loading fails closed with `MEJEPA_EMBED_DIGEST_MISMATCH`; no fallback to alternative weights; surfaced in operator-visible error</then>
    </criterion>
    <criterion id="AC-EMBED-01-03">
      <given>A non-finite (NaN / Inf) value emerges in any embedder's forward output</given>
      <when>`encode` validates the output before returning</when>
      <then>It returns `MEJEPA_EMBED_NUMERICAL_INVARIANT` with the embedder id, input SHA, and the index of the offending element</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-02" priority="must-have">
  <narrative>
    <as_a>panel materialization pipeline</as_a>
    <i_want_to>look up the active embedder set for a given AST `EntityType` via a static routing table</i_want_to>
    <so_that>chunks are sent to only the embedders that produce useful signal for that entity type, avoiding wasted forward passes</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-02-01">
      <given>An AST chunk with `EntityType::Function { is_test: false }`</given>
      <when>The pipeline queries `route_for_entity_type(entity_type)` for the function-body subkind</when>
      <then>The returned `BTreeSet&lt;EmbedderId&gt;` contains exactly {E7, E14}</then>
    </criterion>
    <criterion id="AC-EMBED-02-02">
      <given>An AST chunk with `EntityType::Function { is_test: true }` (a pytest / cargo / JUnit test function)</given>
      <when>The pipeline queries the routing table</when>
      <then>The returned set is exactly {E1, E7}</then>
    </criterion>
    <criterion id="AC-EMBED-02-03">
      <given>A chunk classified as a class/struct definition, an import, a comment block, a docstring, a test assertion, a function signature, or a module/namespace</given>
      <when>The pipeline queries the routing table</when>
      <then>The returned set matches the table in REQ-EMBED-04 byte-equal across runs</then>
    </criterion>
    <criterion id="AC-EMBED-02-04">
      <given>A chunk whose `EntityType` is one of the four "instrument-internal" kinds (DiffHunk, AstNodeSequence, CfgBasicBlock, DefUseEdge)</given>
      <when>The routing table is queried</when>
      <then>The result is `RoutingResult::HandledByInstrument(slot)` with no embedder set returned (E_Diff/E_AST/E_CFG/E_DataFlow handle these internally per doc 14)</then>
    </criterion>
    <criterion id="AC-EMBED-02-05">
      <given>An `EntityType` variant that the table does not cover</given>
      <when>The pipeline queries</when>
      <then>The query returns `MEJEPA_EMBED_ROUTING_MISSING { entity_type, language }`; no silent default to a "catch-all" embedder set</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-03" priority="must-have">
  <narrative>
    <as_a>system operator</as_a>
    <i_want_to>be confident that the loaded ensemble + predictor + LoRA + activations stay under the RTX 5090's 29 GB usable budget</i_want_to>
    <so_that>training and inference do not OOM mid-step or fall back to CPU</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-03-01">
      <given>A cold start with all 14 content embedders + predictor + activations + AdamW + LoRA adapters configured</given>
      <when>`Ensemble::load_content_set()` completes successfully</when>
      <then>Resident VRAM (queried via cudarc) is ≤ 25 GB and the ensemble emits a `vram_budget_report` artifact listing per-component bytes</then>
    </criterion>
    <criterion id="AC-EMBED-03-02">
      <given>A request to lazy-load any E15-E21 embedder via `mejepa_load_learner_state(slot)` MCP tool</given>
      <when>Available VRAM headroom is &lt; the embedder's footprint + 512 MB safety margin</when>
      <then>The call returns `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED { needed_bytes, available_bytes }` and does not load partially</then>
    </criterion>
    <criterion id="AC-EMBED-03-03">
      <given>The ensemble is at steady state with all 21 embedders loaded</given>
      <when>VRAM is sampled for 60 seconds during a batch=64 training step</when>
      <then>Peak VRAM never exceeds 29 GB; the ensemble logs the peak as part of the per-step training certificate</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-04" priority="must-have">
  <narrative>
    <as_a>self-healing loop (Phase 5)</as_a>
    <i_want_to>refresh LoRA adapters on E5/E11/E14 (and the custom embedders) when effective plasticity drops below 0.4</i_want_to>
    <so_that>UTML §V.1 continual-backprop equivalent restores capacity without altering the frozen base contract that gives ME-JEPA its collapse-immunity</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-04-01">
      <given>The shipped `training/lora.rs` LoraAdapter is wired for embedder E5 (Causal, NomicBERT)</given>
      <when>`refresh_lora_adapter(E5, fresh_corpus_slice, rank=32, epochs=10)` is invoked</when>
      <then>Only the adapter `Var` tensors update; the base weight buffer remains byte-equal pre-/post-call (verified via SHA-256)</then>
    </criterion>
    <criterion id="AC-EMBED-04-02">
      <given>An attempt to fine-tune the base weights of any embedder directly (no LoRA wrapper, gradient flow into the frozen base)</given>
      <when>The training step is invoked</when>
      <then>The training step refuses with `MEJEPA_EMBED_FROZEN_BASE_VIOLATION`; the training certificate would have logged ΔΞ.target_collapse &gt; 0, which is a CRITICAL bug per constitution &lt;utml_v1_invariants&gt;</then>
    </criterion>
    <criterion id="AC-EMBED-04-03">
      <given>An embedder has accumulated 1 base LoRA + 4 task-specific delta-LoRAs (the cap)</given>
      <when>A 5th delta-LoRA registration is requested</when>
      <then>The oldest delta is SVD-merged into the base LoRA, then the new delta is registered, keeping the per-embedder adapter count at 5</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-05" priority="should-have">
  <narrative>
    <as_a>UTML L_step computation</as_a>
    <i_want_to>derive `embedder_coherence(panel)` and `predictor_collapse_score` from the ensemble</i_want_to>
    <so_that>ΔK and ΔΞ have a structural cross-embedder signal beyond gradient alignment alone (per doc 12 §5.1, §5.2)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-05-01">
      <given>A candidate panel with all 14 content embedders' source vectors</given>
      <when>`embedder_coherence(panel)` is computed</when>
      <then>The result is `mean(jaccard(NN_i(panel), NN_j(panel)))` over all (i,j) pairs in the content embedder set; clamped to [0,1]; ≥1,000 corpus entries required (else returns neutral 0.5 with a `low_corpus` flag)</then>
    </criterion>
    <criterion id="AC-EMBED-05-02">
      <given>A predicted panel and a target panel from ME-JEPA's frozen target readback</given>
      <when>`predictor_collapse_score(predicted, target)` is computed</when>
      <then>The result is `1 - std([cos(predicted_panel, target_panel) under E_i for i in 1..14])` clamped to [0,1]</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-06" priority="should-have">
  <narrative>
    <as_a>UTML ΔΩ_outer term</as_a>
    <i_want_to>get an agent-CoT affect signal from E17 AffectText running on Claude Code conversation transcripts</i_want_to>
    <so_that>UTML can detect frustrated / cycling outer-loop sessions whose data is low quality (per doc 12 §5.3)</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-06-01">
      <given>A conversation transcript window (last N tool-call exchanges from the current /loop turn)</given>
      <when>`agent_state_score(transcript)` is invoked</when>
      <then>The result is `E17(transcript).valence · arousal_inverted_U(E17(transcript).arousal)` clamped to [0,1]</then>
    </criterion>
    <criterion id="AC-EMBED-06-02">
      <given>E17 has not been calibrated on agent-transcript data (the held-out 100-500 sample calibration set has not run)</given>
      <when>`agent_state_score` is invoked</when>
      <then>The function returns `MEJEPA_EMBED_E17_UNCALIBRATED` until calibration is recorded; the operator must explicitly opt in via `--allow-uncalibrated-e17` for diagnostic-only use</then>
    </criterion>
  </acceptance_criteria>
</story>

<story id="US-EMBED-07" priority="must-have">
  <narrative>
    <as_a>determinism auditor</as_a>
    <i_want_to>verify that any byte-equal `(input, weight_sha, code_version)` tuple yields the same embedder output across machines</i_want_to>
    <so_that>training certs, constellation centroids, and witness-chain entries can be replay-verified</so_that>
  </narrative>
  <acceptance_criteria>
    <criterion id="AC-EMBED-07-01">
      <given>An embedder loaded with weight_sha=W and code_version=V on machine A</given>
      <when>The same (W, V) is loaded on machine B and both encode the same input</when>
      <then>The output `Vec&lt;f32&gt;` is byte-identical (subject to bf16 vs f32 reduction order — bf16 path documents tolerated 1-ULP variance and gates re-encoding through f32 reference path on disagreement)</then>
    </criterion>
    <criterion id="AC-EMBED-07-02">
      <given>An embedder's `encode` involves any stochastic operation (e.g., dropout in LoRA training mode)</given>
      <when>The instrument is constructed</when>
      <then>It accepts a `seed: u64` parameter; same seed → same output; dropout is disabled by default (inference mode) and only enabled inside the LoRA training path</then>
    </criterion>
  </acceptance_criteria>
</story>

</user_stories>

<requirements>

<requirement id="REQ-EMBED-01" story_ref="US-EMBED-01" priority="must">
  <description>Ship a Rust trait `Instrument` (already in `crates/context-graph-mejepa-instruments/src/lib.rs`) extended with three new methods: `embedder_id() -> Option&lt;EmbedderId&gt;`, `version() -> &amp;str`, `weight_sha() -> Option&lt;&amp;str&gt;`. Provide a struct `EmbedderInstrument&lt;E&gt;` that wraps any of E1-E21 and implements the trait. Each wrapper exposes the underlying embedder's deterministic forward pass and validates output dim + finiteness fail-closed.</description>
  <rationale>Per doc 12 §1, the 21 embedders are the substrate; the panel slots project them. A single trait surface decouples slot wiring from per-embedder code paths.</rationale>
  <constraints>The existing trait `Instrument` is `Input`-generic; the three new methods MUST default to `None / "unknown"` so existing E_Oracle / E_Witness / E_StaticAnalysis impls keep compiling. EmbedderInstrument MUST NOT introduce gradient flow through the base weights (constitution &lt;anti_patterns&gt; "No learnable target encoder").</constraints>
  <verification_method>FSV harness `examples/embedders_fsv.rs` instantiates one EmbedderInstrument for each of E1, E5, E7, E11, E14, E17 and asserts: (a) `output_dim() == E.projected_dimension()`, (b) byte-equal output across two calls on the same input, (c) `MEJEPA_EMBED_DIGEST_MISMATCH` on a tampered weight file, (d) `MEJEPA_EMBED_NUMERICAL_INVARIANT` on injected NaN.</verification_method>
</requirement>

<requirement id="REQ-EMBED-02" story_ref="US-EMBED-01" priority="must">
  <description>Pin every embedder weight file's SHA-256 digest in `config/models_config.toml` under a per-embedder `[E&lt;n&gt;] weight_sha256 = "..."` entry. The wrapper MUST verify the digest at load time and refuse with `MEJEPA_EMBED_DIGEST_MISMATCH` on any mismatch.</description>
  <rationale>Constitution `SEC-MEJEPA-02` mandates pinned digests. Embedding-space drift on upstream model updates would silently invalidate every persisted constellation and training cert.</rationale>
  <constraints>The digest covers the full file payload (not the HuggingFace revision string). For multi-file weights (e.g., shard-X.safetensors), digest a concatenated manifest (sorted by filename) to bind all shards atomically.</constraints>
  <verification_method>FSV: corrupt one byte of a loaded weight file, attempt re-load, assert error code. Test: enumerate every entry in `models_config.toml`, verify the digest matches the file on disk.</verification_method>
</requirement>

<requirement id="REQ-EMBED-03" story_ref="US-EMBED-01" priority="must">
  <description>The `EmbedderId` enum MUST cover all 21 slots (E1 through E21) and be serde-stable. Mapping: E1=Semantic, E2=TemporalRecent, E3=TemporalPeriodic, E4=TemporalPositional, E5=Causal, E6=Sparse, E7=Code, E8=Graph, E9=Hdc, E10=Contextual, E11=Kepler (production; legacy `Entity` MiniLM is an alias for backward-compat reads only), E12=LateInteraction, E13=Splade, E14=BgeM3Dense, E15=AffectSpeech, E16=AffectFace, E17=AffectText, E18=PPG, E19=EDA, E20=EEG, E21=EegArtifactRobust.</description>
  <rationale>Constellation centroids, MCP tool responses, training certs all key on `EmbedderId`. A stable enum is the contract.</rationale>
  <constraints>Existing `crates/context-graph-embeddings/src/types/model_id/core.rs` enumerates E1-E14 already with `production()` returning the 14 production set. The EMBEDDERS spec adds the E15-E21 namespace from `learner/mod.rs::LearnerEmbedderSlot`. The new `EmbedderId` is the union — defined in this crate, not in `context-graph-embeddings` (clean-room rule: no upstream modifications).</constraints>
  <verification_method>Unit test enumerates the 21 IDs, asserts `dimension()` matches the inventory table, asserts JSON serde round-trips byte-equal.</verification_method>
</requirement>

<requirement id="REQ-EMBED-04" story_ref="US-EMBED-02" priority="must">
  <description>Ship a static routing table `route_for_entity_type(entity_type, language) -> RoutingResult` mapping each AST `EntityType` to either (a) `RoutingResult::Embedders(BTreeSet&lt;EmbedderId&gt;)` for chunks the panel pipeline encodes directly, or (b) `RoutingResult::HandledByInstrument(InstrumentSlot)` for the four slot-internal kinds.

The MUST-COVER table:

| EntityType | Routed embedders | Handled by |
|---|---|---|
| Function (body, is_test=false) | {E7, E14} | — |
| Function (signature only) | {E1, E10} | — |
| Function (is_test=true) | {E1, E7} | — |
| Class / Struct definition | {E11, E7} | — |
| Module / Namespace | {E1, E8} | — |
| Import | {E11, E13} | — |
| Comment block | {E1, E10} | — |
| Docstring | {E1, E10} | — |
| Test assertion | {E5, E1} | — |
| Diff hunk | (none) | E_Diff |
| AST node sequence | (none) | E_AST |
| CFG basic blocks | (none) | E_CFG |
| Def-use edges | (none) | E_DataFlow |
</description>
  <rationale>Per doc 14 §4.1, full ensemble forward on every chunk is wasteful and contaminates panel signal with embedders that have no useful representation for that entity kind. The routing table is the canonical answer to "which embedders see this chunk?" Constitution &lt;tct_invariants&gt; references "per-chunk-type localization" — this table is its operational form.</rationale>
  <constraints>Table entries MUST be `const` / static — no runtime construction, no environment-variable override. Adding a new `EntityType` requires updating this spec AND the table; missing-entry queries fail closed (no implicit default). Language-specific deviations (e.g., Python-specific routes) are folded into the `language` parameter; the default table applies when language is None.</constraints>
  <verification_method>Property test: for every `(EntityType, Language)` pair, the table either returns a non-empty `Embedders` set or `HandledByInstrument`; never `None`. FSV: encode 100 chunks of each kind through the routed embedders, assert the panel slot gets exactly the routed signal.</verification_method>
</requirement>

<requirement id="REQ-EMBED-05" story_ref="US-EMBED-03" priority="must">
  <description>Implement an `Ensemble` resource manager that loads embedder weights into VRAM under a hard 29 GB total budget. Provide `load_content_set()` (loads E1-E14, the always-on set), `load_learner_state(slot: EmbedderId)` (lazy-loads one of E15-E21 on demand), and `unload(slot)` (releases VRAM). Maintain a running `vram_resident_bytes` counter; refuse loads that would exceed `29 * 2^30 - 512 * 2^20` (29 GB minus 512 MB safety margin).</description>
  <rationale>Per doc 12 §4 + constitution `&lt;performance_budgets&gt; vram_steady_state ≤ 29 GB`. Without an enforcement layer, the ensemble + predictor + activations + AdamW would silently OOM during a batch=64 step.</rationale>
  <constraints>VRAM accounting MUST go through cudarc's `cuMemGetInfo` (not Rust-side bookkeeping) to catch driver-side fragmentation. The 14-content steady-state target is ≤ 14 GB; the predictor + activations + optimizer + LoRA + KV-cache target is ≤ 13 GB. The 2 GB residual buffers headroom for E15-E21 lazy loads.</constraints>
  <verification_method>Integration test: load all 14 content embedders, assert peak VRAM ≤ 14 GB (via cudarc query). Stress test: simulate a request that would overflow; assert `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED` error and that no partial state was loaded.</verification_method>
</requirement>

<requirement id="REQ-EMBED-06" story_ref="US-EMBED-04" priority="must">
  <description>Wire the existing `crates/context-graph-embeddings/src/training/lora.rs` LoraAdapter pipeline into the EMBEDDERS domain. Expose `register_lora_adapter(EmbedderId, rank, alpha, dropout) -> AdapterId`, `refresh_lora_adapter(AdapterId, fresh_corpus_slice, epochs)`, and `merge_lora_adapter(AdapterId)` (SVD-compresses an old delta into the base LoRA). Cap each embedder at 1 base + 4 delta LoRAs = 5 adapters max.</description>
  <rationale>Per doc 12 §6 + constitution `&lt;utml_v1_invariants&gt;` UTML §V.1 effective_plasticity intervention. LoRA is the pre-shipped substitute for continual-backprop dormant-unit re-init.</rationale>
  <constraints>The LoraAdapter MUST sit OUTSIDE the frozen base weight buffer; gradient computation MUST NOT flow into the base. Verifiable by SHA-checking the base buffer pre- and post-LoRA fit; same hash required. Default rank=32, alpha=32, dropout=0.05. LoRA storage cap per embedder: 5 * (2 * hidden_size * 32) ≈ 200 KB per embedder × 21 = ~4 MB total.</constraints>
  <verification_method>FSV: register LoRA on E5, run 10 epochs of fine-tuning on a synthetic 1k-pair corpus, SHA-check the base weights pre/post (must match), encode a fixed input through the adapted embedder + raw embedder, assert outputs differ but base buffer is identical.</verification_method>
</requirement>

<requirement id="REQ-EMBED-07" story_ref="US-EMBED-05" priority="should">
  <description>Implement `embedder_coherence(panel) -> f32` as `mean(jaccard(NN_i(panel), NN_j(panel)))` over content-embedder pairs (i,j ∈ E1..E14). NN sets sized at k=20 from the corpus index. When the corpus has &lt; 1,000 entries, return 0.5 with a `low_corpus_size` flag in the result struct.</description>
  <rationale>Per doc 12 §5.1, this is the structural ΔK supplement. It needs a corpus to compute against — the ANN/HNSW index over the panel database.</rationale>
  <constraints>NN computation MUST use a HNSW-style ANN index over each embedder's source vectors persisted in the panel database. Latency target: &lt; 50 ms per panel at corpus-size 100k. Output is a single f32 in [0,1].</constraints>
  <verification_method>Test: at corpus &lt; 1000 entries, assert `0.5` with flag; at corpus ≥ 1000, assert the result matches a hand-computed reference on a 50-entry mini-corpus.</verification_method>
</requirement>

<requirement id="REQ-EMBED-08" story_ref="US-EMBED-05" priority="should">
  <description>Implement `predictor_collapse_score(predicted_panel, target_panel) -> f32` as `1 - std([cos(predicted_panel, target_panel) under E_i for i in E1..E14])`. Std uses sample-std (n-1) denominator. Result clamped to [0,1].</description>
  <rationale>Per doc 12 §5.2, cross-embedder spread is the structural anti-collapse signal beyond pairwise MI auditing.</rationale>
  <constraints>Computed on the panel slice corresponding to each embedder's source contribution. Healthy predictor produces low score (high spread); collapsed predictor produces high score (uniform agreement). The threshold for "collapsed" is set by the ΔΞ.predictor_redundancy term in UTML; this requirement only specifies the score, not the threshold.</constraints>
  <verification_method>Synthetic test: a constant-output predictor produces score ≈ 1.0; a diverse predictor whose 14 cosines are uniformly distributed in [0,1] produces score &lt; 0.7.</verification_method>
</requirement>

<requirement id="REQ-EMBED-09" story_ref="US-EMBED-06" priority="should">
  <description>Implement `agent_state_score(transcript_window) -> f32` using E17 AffectText. Default to a 4096-token rolling window of the current /loop turn's transcript. Return `valence · arousal_inverted_U(arousal)` where `arousal_inverted_U(a) = 4 * a * (1 - a)` (Yerkes-Dodson inverted-U mapping). Result clamped to [0,1].</description>
  <rationale>Per doc 12 §5.3, this is the ΔΩ_outer agent-state factor in UTML.</rationale>
  <constraints>Until calibrated on agent-transcript data, return `MEJEPA_EMBED_E17_UNCALIBRATED`. Calibration set: 100-500 outer-loop conversation samples scored by operator on (valence, arousal). The calibration certificate is logged to `memory/decisions/`.</constraints>
  <verification_method>Test: with calibration absent, assert error. With a synthetic flat calibration (always returns 0.5/0.5), assert score = 1.0.</verification_method>
</requirement>

<requirement id="REQ-EMBED-10" story_ref="US-EMBED-07" priority="must">
  <description>Every `Instrument::encode` call MUST be deterministic given (input, weight_sha, code_version, seed). Stochastic operations (LoRA dropout in training mode) MUST accept an explicit `seed: u64`. Inference mode disables dropout. The default `seed` value is 0 in inference paths.</description>
  <rationale>Constitution &lt;anti_patterns&gt; "No random initialization in instrument outputs. Same input + same frozen weights + same code_version = same output, byte-equal." Required for replayable training certs and witness-chain integrity.</rationale>
  <constraints>The bf16 GPU path may exhibit 1-ULP non-determinism due to non-associative reduction order on different driver versions. The wrapper MUST log a `precision_class: bf16 | fp32` field on every encode and provide a `--strict-fp32-replay` mode that re-runs through fp32 for byte-equal verification.</constraints>
  <verification_method>FSV: encode the same input 100 times in inference mode; assert all outputs byte-equal. Encode in LoRA training mode with seed=42 twice; assert byte-equal. With seed=43; assert different.</verification_method>
</requirement>

<requirement id="REQ-EMBED-11" story_ref="US-EMBED-03" priority="must">
  <description>Expose three MCP tools under the `mcp__cgreality__mejepa_*` namespace: (1) `mejepa_load_learner_state {slot: EmbedderId}` — lazy-loads one of E15-E21; (2) `mejepa_compute_panel {file_sha, chunk_filter}` — runs the ensemble on a file's chunks and returns a panel; (3) `mejepa_audit_pairwise_mi {corpus_id}` — computes the pairwise mutual-information matrix across the 14 content embedders.</description>
  <rationale>Per doc 12 §10 + constitution &lt;naming_conventions&gt; mcp_tool_namespace_lock. The operator needs surface to inspect, lazy-load, and audit the ensemble.</rationale>
  <constraints>All tools fail closed on dim mismatch, digest mismatch, VRAM budget exceeded, or chunk parser failure. Tool responses include `embedder_versions: BTreeMap&lt;EmbedderId, String&gt;` for replay binding.</constraints>
  <verification_method>MCP integration test: invoke each tool against a fixture panel database, assert response schema matches the typed contract in `TECH-EMBEDDERS &lt;api_contracts&gt;`.</verification_method>
</requirement>

<requirement id="REQ-EMBED-12" story_ref="US-EMBED-03" priority="must">
  <description>Performance budget: panel materialization across the routed-active embedder set MUST hit p50 &lt; 100 ms with cached instruments and p99 &lt; 500 ms cold (full AST + CFG + DataFlow + TypeGraph rebuild). Per-embedder forward pass at batch=1, RTX 5090, bf16 MUST stay within doc 12 §9 latency budget (each &lt; 5 ms; ensemble of 7-9 active embedders &lt; 50 ms).</description>
  <rationale>Constitution `&lt;performance_budgets&gt;` panel_materialization_p50 / p99. Phase 7 §3.8 shift subscriber depends on this latency.</rationale>
  <constraints>Cold path includes weight load + tokenizer init + first-batch warmup. Cached path includes only forward pass. Cache key is `(file_sha, chunk_id, embedder_id, weight_sha)`.</constraints>
  <verification_method>Benchmark FSV: 10,000 panel materializations over the corpus, p50 / p99 latency reported in `vram_budget_report.json`. Test: at corpus N=1000, p50 ≤ 100 ms.</verification_method>
</requirement>

</requirements>

<business_rules>

<rule id="BR-EMBED-01" req_ref="REQ-EMBED-01">
  <condition>An embedder weight file is being loaded.</condition>
  <action>Verify SHA-256 digest against `models_config.toml`. Refuse if mismatched.</action>
  <exception>None. Operator override requires editing `models_config.toml` and is auditable in git.</exception>
</rule>

<rule id="BR-EMBED-02" req_ref="REQ-EMBED-04">
  <condition>An AST chunk's `EntityType` does not appear in the routing table.</condition>
  <action>Fail closed with `MEJEPA_EMBED_ROUTING_MISSING`; the spec must be updated and the table extended.</action>
  <exception>None.</exception>
</rule>

<rule id="BR-EMBED-03" req_ref="REQ-EMBED-05">
  <condition>VRAM resident exceeds the 29 GB - 512 MB safety threshold during runtime.</condition>
  <action>The next embedder load is refused with `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED`. The supervisor SHOULD trigger `unload(E15..E21)` for any lazy-loaded learner-state embedder before retrying.</action>
  <exception>Operator can pass `--vram-budget-gb=N` to raise the budget on a different host (constitution-pinned default of 29 holds for the RTX 5090).</exception>
</rule>

<rule id="BR-EMBED-04" req_ref="REQ-EMBED-06">
  <condition>A LoRA adapter fit step is requested.</condition>
  <action>Compute base-weight SHA-256 pre-fit, run the fit, recompute SHA-256 post-fit, assert equality. If unequal, fail closed with `MEJEPA_EMBED_FROZEN_BASE_VIOLATION` and roll back.</action>
  <exception>None — base immutability is constitutional.</exception>
</rule>

<rule id="BR-EMBED-05" req_ref="REQ-EMBED-09">
  <condition>`agent_state_score` is invoked but E17 has not been calibrated on agent-transcript data.</condition>
  <action>Return `MEJEPA_EMBED_E17_UNCALIBRATED` unless the caller passes `allow_uncalibrated=true` (diagnostic only — value is logged but not gated into ΔΩ_outer).</action>
  <exception>Calibration certificate at `memory/decisions/agent-XXX--e17-agent-transcript-calibration.md` enables production use.</exception>
</rule>

<rule id="BR-EMBED-06" req_ref="REQ-EMBED-10">
  <condition>A bf16 GPU encode disagrees byte-equal with another machine's bf16 encode of the same input.</condition>
  <action>Re-run through fp32 reference path (`--strict-fp32-replay`); if fp32 outputs match, log a `bf16_drift` warning with the 1-ULP delta. If fp32 outputs disagree, fail closed with `MEJEPA_EMBED_DETERMINISM_VIOLATION`.</action>
  <exception>None.</exception>
</rule>

<rule id="BR-EMBED-07" req_ref="REQ-EMBED-06">
  <condition>An embedder accumulates 5 LoRA adapters (the cap).</condition>
  <action>SVD-merge the oldest delta into the base LoRA; register the new delta to maintain the count.</action>
  <exception>None.</exception>
</rule>

</business_rules>

<edge_cases>

<edge_case id="EC-EMBED-01" req_ref="REQ-EMBED-01">
  <scenario>Empty input string passed to a text-based embedder (E1, E5, E7, E10, E11, E14, E17).</scenario>
  <expected_behavior>Return `MEJEPA_EMBED_INVALID_INPUT { reason: "empty input" }`. Do NOT return a zero vector.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-EMBED-02" req_ref="REQ-EMBED-01">
  <scenario>Input exceeds the embedder's `max_tokens` (e.g., 10K-token input to E1's 512-token limit).</scenario>
  <expected_behavior>Truncate to `max_tokens` from the head, log a `truncated_input` flag in the encode result. Per-embedder truncation strategy: E1/E5/E10/E11 → head; E7/E14 → head (8K / 8192-token contexts); custom embedders (E2/E3/E4/E9) → no token concept, no truncation.</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-EMBED-03" req_ref="REQ-EMBED-04">
  <scenario>An AST chunk straddles two `EntityType` kinds (e.g., a function with an embedded class definition that the AST chunker emits as a single chunk).</scenario>
  <expected_behavior>The chunker is responsible for emitting one chunk per atomic entity; the routing table assumes single-EntityType chunks. If a multi-kind chunk is observed, fail closed with `MEJEPA_CHUNKER_INVARIANT_VIOLATION` (chunker bug, not embedders bug).</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-EMBED-04" req_ref="REQ-EMBED-05">
  <scenario>Cold start: VRAM is plentiful but the first encode call is in flight; a concurrent second encode arrives before the embedder's CUDA stream is initialized.</scenario>
  <expected_behavior>The Ensemble manager serializes initialization with a `tokio::sync::Mutex`; the second call waits for the first; both succeed. Latency for the second call may exceed the cached p50 (acceptable; counted under p99 cold).</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-EMBED-05" req_ref="REQ-EMBED-06">
  <scenario>LoRA fit is interrupted (process killed) mid-step.</scenario>
  <expected_behavior>The base weight buffer is unaffected (verified by SHA on next load). Adapter state is rolled back to the last checkpointed version. Partial training certificate marked `aborted: true`.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-EMBED-06" req_ref="REQ-EMBED-07">
  <scenario>Corpus has exactly 999 entries when `embedder_coherence` is queried.</scenario>
  <expected_behavior>Return 0.5 with `low_corpus_size: true` (strict ≥ 1,000 cutoff per doc 12 §10 risk #3).</expected_behavior>
  <priority>low</priority>
</edge_case>

<edge_case id="EC-EMBED-07" req_ref="REQ-EMBED-09">
  <scenario>A conversation transcript window contains MCP tool-call JSON only (no natural-language prose).</scenario>
  <expected_behavior>E17 still runs (tokenizes the JSON as text); the calibration set must include such samples. Until calibrated, `MEJEPA_EMBED_E17_UNCALIBRATED` applies.</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-EMBED-08" req_ref="REQ-EMBED-10">
  <scenario>An embedder's tokenizer normalizes Unicode differently across libtokenizers versions (e.g., NFKC vs NFC).</scenario>
  <expected_behavior>The `code_version` in the byte-equal contract MUST include the libtokenizers crate version (frozen in `Cargo.lock`). Different `code_version` → different byte-equal class; replay verification accounts for this.</expected_behavior>
  <priority>high</priority>
</edge_case>

<edge_case id="EC-EMBED-09" req_ref="REQ-EMBED-11">
  <scenario>`mejepa_compute_panel` is called with a `file_sha` not in the corpus.</scenario>
  <expected_behavior>Compute the panel on demand (no precomputed artifact); cache the result for the next call. If the file is unparseable, propagate `MEJEPA_CHUNKER_PARSE_FAILED` (per constitution `&lt;failure_taxonomy&gt;`).</expected_behavior>
  <priority>medium</priority>
</edge_case>

<edge_case id="EC-EMBED-10" req_ref="REQ-EMBED-05">
  <scenario>cudarc reports VRAM headroom that is silently inaccurate due to driver-side fragmentation (allocation fails despite reported headroom).</scenario>
  <expected_behavior>Fall back to a `cudaMalloc` probe-then-free pre-flight check before each `load_learner_state`. If the probe fails, treat as `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED` even if reported headroom suggests otherwise.</expected_behavior>
  <priority>medium</priority>
</edge_case>

</edge_cases>

<error_states>

<error id="ERR-EMBED-01" http_code="400" req_ref="REQ-EMBED-01">
  <condition>Caller passes an `EmbedderInput` whose runtime variant doesn't match the embedder's expected input (e.g., `LearnerEmbedderInput::Samples` to a text-only embedder).</condition>
  <user_message>"The {embedder_id} expects input of type {expected_kind}; received {actual_kind}."</user_message>
  <internal_message>MEJEPA_EMBED_INVALID_INPUT {embedder_id, expected_kind, actual_kind, input_preview_sha}</internal_message>
  <recovery_action>Caller routes the input through the correct embedder per the routing table; check upstream chunk classification.</recovery_action>
</error>

<error id="ERR-EMBED-02" http_code="500" req_ref="REQ-EMBED-02">
  <condition>Embedder weight file SHA-256 mismatches `models_config.toml`.</condition>
  <user_message>"Embedder {embedder_id} weight file digest mismatch. Expected {expected_sha}, got {actual_sha}. Refusing to load."</user_message>
  <internal_message>MEJEPA_EMBED_DIGEST_MISMATCH {embedder_id, expected_sha, actual_sha, file_path}</internal_message>
  <recovery_action>Operator inspects `config/models_config.toml` and the on-disk file; if the model was deliberately updated, update the digest pin via PR (auditable). Constitution &lt;security_requirements&gt; SEC-MEJEPA-02.</recovery_action>
</error>

<error id="ERR-EMBED-03" http_code="500" req_ref="REQ-EMBED-01">
  <condition>An embedder's forward pass produces NaN or Inf in the output.</condition>
  <user_message>"Embedder {embedder_id} produced non-finite output at index {idx}. Refusing to use."</user_message>
  <internal_message>MEJEPA_EMBED_NUMERICAL_INVARIANT {embedder_id, idx, value, input_sha}</internal_message>
  <recovery_action>Roll back to last-known checkpoint; surface to operator. If reproducible across multiple inputs, the embedder weight may be corrupted — re-verify digest.</recovery_action>
</error>

<error id="ERR-EMBED-04" http_code="503" req_ref="REQ-EMBED-05">
  <condition>VRAM budget would be exceeded by the next load.</condition>
  <user_message>"Cannot load {embedder_id}: needed {needed_bytes} bytes, only {available_bytes} bytes free under budget."</user_message>
  <internal_message>MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED {embedder_id, needed_bytes, available_bytes, currently_loaded: Vec&lt;EmbedderId&gt;}</internal_message>
  <recovery_action>Caller invokes `unload(slot)` on idle E15-E21 embedders; retry. Operator can raise the budget if running on a larger host.</recovery_action>
</error>

<error id="ERR-EMBED-05" http_code="500" req_ref="REQ-EMBED-06">
  <condition>A LoRA fit step accidentally backpropagates into the frozen base weights (post-fit base SHA differs from pre-fit base SHA).</condition>
  <user_message>"Frozen-base contract violated: embedder {embedder_id} base weights changed during LoRA fit."</user_message>
  <internal_message>MEJEPA_EMBED_FROZEN_BASE_VIOLATION {embedder_id, pre_sha, post_sha, adapter_id}</internal_message>
  <recovery_action>This is a CRITICAL bug per constitution &lt;utml_v1_invariants&gt;. Rollback is automatic; the failing adapter is dropped; operator inspects the LoRA wrapper code path.</recovery_action>
</error>

<error id="ERR-EMBED-06" http_code="404" req_ref="REQ-EMBED-04">
  <condition>An `EntityType` queried in the routing table is not covered.</condition>
  <user_message>"Routing table has no entry for entity_type={entity_type}, language={language}. The spec must be extended."</user_message>
  <internal_message>MEJEPA_EMBED_ROUTING_MISSING {entity_type, language, chunk_sha}</internal_message>
  <recovery_action>Add the entry to REQ-EMBED-04, update the static table, ship a regression test.</recovery_action>
</error>

<error id="ERR-EMBED-07" http_code="412" req_ref="REQ-EMBED-09">
  <condition>`agent_state_score` is invoked but E17 has not been calibrated on agent transcripts.</condition>
  <user_message>"E17 AffectText calibration on agent transcripts is required before agent_state_score can be trusted. See memory/decisions/agent-XXX--e17-agent-transcript-calibration.md for the calibration certificate template."</user_message>
  <internal_message>MEJEPA_EMBED_E17_UNCALIBRATED {window_size, allow_uncalibrated_flag}</internal_message>
  <recovery_action>Run the calibration set (100-500 samples); record certificate in memory/decisions/.</recovery_action>
</error>

<error id="ERR-EMBED-08" http_code="500" req_ref="REQ-EMBED-10">
  <condition>fp32 reference replay disagrees with bf16 GPU output beyond 1-ULP per element.</condition>
  <user_message>"Embedder {embedder_id} determinism violation: bf16 vs fp32 disagreement &gt; 1-ULP."</user_message>
  <internal_message>MEJEPA_EMBED_DETERMINISM_VIOLATION {embedder_id, max_delta, input_sha, code_version}</internal_message>
  <recovery_action>Halt training; verify weight digests; check for driver / cudarc version drift since last replay.</recovery_action>
</error>

</error_states>

<data_requirements>

<entity name="EmbedderId">
  <field name="variant" type="enum(E1..E21)" required="true">
    <description>Stable serde-tagged variant identifier.</description>
    <constraints>Must be one of the 21 documented embedders. Legacy `Entity` (MiniLM-384D) is a deserialization alias for E11=Kepler reads only.</constraints>
  </field>
</entity>

<entity name="EmbedderInput">
  <field name="text_content" type="string" required="false">
    <description>Text input for E1, E5, E6, E7, E8, E10, E11, E12, E13, E14, E17.</description>
    <constraints>Must be UTF-8; non-empty unless the embedder explicitly supports empty.</constraints>
  </field>
  <field name="temporal_context" type="TemporalContext" required="false">
    <description>Input for E2/E3/E4 — wall-clock timestamp + decay constants.</description>
    <constraints>Wall-clock must be a valid Unix timestamp (positive i64).</constraints>
  </field>
  <field name="hdc_input" type="HdcInput" required="false">
    <description>Input for E9 — text-or-bytes hashed into 10K-bit holographic vector.</description>
    <constraints>Input non-empty.</constraints>
  </field>
  <field name="learner_input" type="LearnerEmbedderInput" required="false">
    <description>Input for E15-E21 — Text / Samples / Features per `learner/mod.rs`.</description>
    <constraints>Modality matches the embedder's declared modality or fails closed.</constraints>
  </field>
</entity>

<entity name="EmbedderOutput">
  <field name="vector" type="Vec&lt;f32&gt;" required="true">
    <description>The latent vector. Length equals `EmbedderId::projected_dimension()`.</description>
    <constraints>All values finite. No `unwrap_or` defaults.</constraints>
  </field>
  <field name="scalars" type="BTreeMap&lt;string, f32&gt;" required="false">
    <description>For E15-E21: scalar heads (valence, arousal, hrv_coherence, etc.).</description>
    <constraints>Empty map for E1-E14.</constraints>
  </field>
  <field name="embedder_version" type="string" required="true">
    <description>Frozen version tag (e.g., "e5-large-v2-2024-11-15").</description>
    <constraints>Non-empty; recorded in every artifact that consumes this output.</constraints>
  </field>
  <field name="weight_sha" type="string" required="true">
    <description>SHA-256 of the weight file(s) used to produce this output.</description>
    <constraints>64 hex chars.</constraints>
  </field>
  <field name="latency_ms" type="u32" required="true">
    <description>Forward-pass wall-clock latency in milliseconds.</description>
    <constraints>Bounded — 0 to 60_000.</constraints>
  </field>
  <field name="precision_class" type="enum(bf16, fp32)" required="true">
    <description>The numerical precision the forward used.</description>
    <constraints>Default bf16 on CUDA path; fp32 on `--strict-fp32-replay`.</constraints>
  </field>
  <field name="truncated_input" type="bool" required="true">
    <description>Whether the input was truncated to fit `max_tokens`.</description>
    <constraints>—</constraints>
  </field>
</entity>

<entity name="RoutingResult">
  <field name="embedders" type="Option&lt;BTreeSet&lt;EmbedderId&gt;&gt;" required="false">
    <description>The active embedder set for this entity type, or None when handled internally.</description>
    <constraints>If Some, must be non-empty.</constraints>
  </field>
  <field name="handled_by_instrument" type="Option&lt;InstrumentSlot&gt;" required="false">
    <description>The panel slot's instrument that handles this entity internally.</description>
    <constraints>Either embedders OR handled_by_instrument is Some, never both.</constraints>
  </field>
</entity>

<entity name="VramBudgetReport">
  <field name="resident_total_bytes" type="u64" required="true">
    <description>Total VRAM resident across the ensemble + predictor + activations.</description>
    <constraints>≤ 29 * 2^30.</constraints>
  </field>
  <field name="per_embedder_bytes" type="BTreeMap&lt;EmbedderId, u64&gt;" required="true">
    <description>Per-embedder VRAM footprint.</description>
    <constraints>Sum ≤ resident_total_bytes.</constraints>
  </field>
  <field name="lora_adapters_bytes" type="u64" required="true">
    <description>Total LoRA adapter VRAM.</description>
    <constraints>—</constraints>
  </field>
  <field name="cuda_driver_overhead_bytes" type="u64" required="true">
    <description>Driver / system overhead reported by cuMemGetInfo.</description>
    <constraints>—</constraints>
  </field>
</entity>

</data_requirements>

<non_functional_requirements>

<nfr id="NFR-EMBED-01" category="performance">
  <description>Panel materialization across the routed-active embedder set.</description>
  <metric>p50 &lt; 100 ms with cached instruments; p99 &lt; 500 ms cold (per constitution &lt;performance_budgets&gt;).</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-02" category="performance">
  <description>Per-embedder forward pass at batch=1, RTX 5090, bf16.</description>
  <metric>Each &lt; 5 ms; ensemble of 7-9 active embedders &lt; 50 ms (per doc 12 §9).</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-03" category="performance">
  <description>VRAM steady-state with 21 embedders + predictor + LoRA + activations + AdamW + KV-cache.</description>
  <metric>≤ 29 GB on RTX 5090 (per constitution).</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-04" category="reliability">
  <description>Determinism under fixed (input, weight_sha, code_version, seed).</description>
  <metric>Byte-equal output 100 / 100 trials in inference mode; 1-ULP tolerance on bf16 GPU path.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-05" category="security">
  <description>Weight digest pinning per SEC-MEJEPA-02.</description>
  <metric>100% of embedder weight loads verified against SHA-256 in `models_config.toml`; mismatches refused.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-06" category="security">
  <description>Frozen-base immutability under LoRA fitting.</description>
  <metric>Base-weight SHA-256 byte-equal pre- and post-fit, 1,000 / 1,000 trials.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-07" category="reliability">
  <description>Fail-closed boundary errors.</description>
  <metric>0% of error paths silently fall back to a "default" embedder, "neutral" coherence, or zero vector. All errors surface a SCREAMING_SNAKE error code.</metric>
  <priority>must</priority>
</nfr>

<nfr id="NFR-EMBED-08" category="reliability">
  <description>LoRA adapter cap.</description>
  <metric>Per-embedder adapter count ≤ 5 (1 base + 4 deltas); SVD-merge enforces the cap.</metric>
  <priority>should</priority>
</nfr>

<nfr id="NFR-EMBED-09" category="usability">
  <description>MCP tool diagnostics.</description>
  <metric>Every error response includes embedder_id, weight_sha, code_version, input_sha for replay.</metric>
  <priority>must</priority>
</nfr>

</non_functional_requirements>

<dependencies>

<dependency type="internal">
  <name>context-graph-embeddings</name>
  <description>The shipped 21-embedder ensemble + training infrastructure. EMBEDDERS spec wraps it; does NOT modify it (clean-room rule on upstream-reference-only is internal here, but the spec keeps the wrapping crate cleanly separated).</description>
  <impact>If the ensemble's API signatures shift, the wrappers update; the trait surface remains stable.</impact>
</dependency>

<dependency type="internal">
  <name>context-graph-mejepa-instruments</name>
  <description>The Instrument trait + InstrumentSlot enum + Panel / PanelBuilder. EMBEDDERS extends the trait with embedder_id / version / weight_sha methods.</description>
  <impact>Existing E_Oracle / E_Witness / E_StaticAnalysis impls compile unchanged via default method bodies returning None.</impact>
</dependency>

<dependency type="internal">
  <name>context-graph-core (AST chunker)</name>
  <description>The 11-language AST chunker emits `EntityType`-tagged chunks that the routing table consumes.</description>
  <impact>Routing table breaks if `EntityType` enum changes without updating REQ-EMBED-04.</impact>
</dependency>

<dependency type="external">
  <name>candle 0.9.x + cudarc</name>
  <description>The pure-Rust ML framework + CUDA bindings hosting embedder forward passes on the RTX 5090.</description>
  <impact>VRAM accounting goes through cudarc's `cuMemGetInfo`. Driver / cudarc updates require re-running determinism FSV.</impact>
</dependency>

<dependency type="external">
  <name>HuggingFace tokenizers 0.21.x</name>
  <description>Tokenization for E1/E5/E7/E10/E11/E12/E13/E14/E17.</description>
  <impact>Tokenizer NFC/NFKC normalization changes between versions affect determinism — `code_version` MUST include the tokenizers crate version.</impact>
</dependency>

<dependency type="external">
  <name>RTX 5090 GPU + 32 GB VRAM</name>
  <description>The deployment target.</description>
  <impact>VRAM budget tuned to this hardware. Smaller GPUs require lazy-loading more aggressively or dropping E15-E21 entirely.</impact>
</dependency>

</dependencies>

<out_of_scope>

<item>Panel slot dim layout and PanelBuilder validation logic — owned by SPEC-INSTRUMENTS / TECH-INSTRUMENTS (existing `context-graph-mejepa-instruments` crate).</item>
<item>The TCT constellation centroid construction (per (mutation_category, language, node_type, embedder)) — owned by SPEC-TCT.</item>
<item>The ME-JEPA predictor architecture and target readback — owned by SPEC-PREDICTOR.</item>
<item>The 8 mutation operators that generate the Phase 0 corpus — owned by SPEC-CORPUS (already shipped via context-graph-mejepa-corpus).</item>
<item>The `mejepa-shift-subscriber` Tokio task — owned by SPEC-SHIFT-SUBSCRIBER (Phase 7).</item>
<item>The 11 tree-sitter parsers and chunker boundaries — owned by SPEC-AST-CHUNKING (doc 14).</item>
<item>Multi-GPU sharding — single-GPU (RTX 5090) is the deployment target.</item>
<item>Quantization beyond bf16 (e.g., int8 / Q4) — not needed under the current VRAM budget; revisit if budget tightens.</item>
<item>Live operator sensor wiring for E15/E16/E18-E21 — these embedders ship the instrument surface, but live sensor integration is out of scope; only E17 (text-only) is wired into ΔΩ_outer in this spec.</item>
<item>Constellation provenance metadata (corpus_sha, frozen_at, max_age) — owned by SPEC-TCT.</item>

</out_of_scope>

<test_plan>

<test_case id="TC-EMBED-01" type="unit" req_ref="REQ-EMBED-01">
  <description>EmbedderInstrument dim contract: every wrapper's `output_dim()` equals `EmbedderId::projected_dimension()`.</description>
  <preconditions>21 embedders loaded.</preconditions>
  <inputs>For each EmbedderId, a synthetic valid input of the embedder's expected kind.</inputs>
  <expected_result>encode succeeds; output length matches projected_dimension; finite values.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-02" type="unit" req_ref="REQ-EMBED-02">
  <description>Digest mismatch refuses to load.</description>
  <preconditions>A weight file on disk with SHA differing from `models_config.toml` pin.</preconditions>
  <inputs>Attempt `Ensemble::load_content_set()`.</inputs>
  <expected_result>`MEJEPA_EMBED_DIGEST_MISMATCH`; ensemble state remains pre-load.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-03" type="unit" req_ref="REQ-EMBED-04">
  <description>Routing table coverage.</description>
  <preconditions>Static routing table compiled in.</preconditions>
  <inputs>For each (EntityType variant × Language variant), call `route_for_entity_type(et, lang)`.</inputs>
  <expected_result>Either a non-empty `Embedders` set or a `HandledByInstrument(slot)`. The Function/Class/Test/etc. cases match the table in REQ-EMBED-04.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-04" type="integration" req_ref="REQ-EMBED-05">
  <description>VRAM budget enforcement under load.</description>
  <preconditions>Cold ensemble; 32 GB GPU.</preconditions>
  <inputs>Sequentially load E1-E14, then iteratively load E15-E21 until budget exceeded.</inputs>
  <expected_result>The first 14 succeed; loading enough learner-state to push past 28.5 GB returns `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED`; resident bytes match cudarc query.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-05" type="integration" req_ref="REQ-EMBED-06">
  <description>LoRA fit preserves base-weight integrity.</description>
  <preconditions>E5 loaded; synthetic 1k-pair fine-tune corpus.</preconditions>
  <inputs>SHA-256 of E5 base buffer; `register_lora_adapter(E5, rank=32, alpha=32, dropout=0.05)`; `refresh_lora_adapter(adapter_id, corpus, epochs=10)`.</inputs>
  <expected_result>Pre/post base-weight SHA byte-equal; adapter Vars updated; encode output differs between adapted and raw.</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-06" type="unit" req_ref="REQ-EMBED-07">
  <description>embedder_coherence on a small corpus returns neutral with flag.</description>
  <preconditions>Corpus &lt; 1,000 entries.</preconditions>
  <inputs>A panel; query coherence.</inputs>
  <expected_result>0.5 with `low_corpus_size: true`.</expected_result>
  <priority>medium</priority>
</test_case>

<test_case id="TC-EMBED-07" type="unit" req_ref="REQ-EMBED-08">
  <description>predictor_collapse_score on a constant predictor.</description>
  <preconditions>Synthetic constant-output predictor.</preconditions>
  <inputs>predicted = constant vector; target = real panel.</inputs>
  <expected_result>Score ≈ 1.0 (uniform agreement across the 14 cosines = std → 0).</expected_result>
  <priority>medium</priority>
</test_case>

<test_case id="TC-EMBED-08" type="integration" req_ref="REQ-EMBED-09">
  <description>agent_state_score requires E17 calibration.</description>
  <preconditions>No calibration certificate in memory/decisions/.</preconditions>
  <inputs>Conversation transcript window; `agent_state_score(window)`.</inputs>
  <expected_result>`MEJEPA_EMBED_E17_UNCALIBRATED`.</expected_result>
  <priority>medium</priority>
</test_case>

<test_case id="TC-EMBED-09" type="integration" req_ref="REQ-EMBED-10">
  <description>Determinism: byte-equal output across 100 calls.</description>
  <preconditions>E1 loaded.</preconditions>
  <inputs>The string "test input"; encode 100 times in inference mode.</inputs>
  <expected_result>All 100 outputs byte-equal (or 1-ULP tolerated on bf16, then fp32-replay byte-equal).</expected_result>
  <priority>critical</priority>
</test_case>

<test_case id="TC-EMBED-10" type="integration" req_ref="REQ-EMBED-11">
  <description>MCP tool round-trip.</description>
  <preconditions>Ensemble loaded; corpus database populated with at least 100 panels.</preconditions>
  <inputs>Invoke `mejepa_compute_panel`, `mejepa_load_learner_state(E17)`, `mejepa_audit_pairwise_mi`.</inputs>
  <expected_result>All return well-formed JSON-RPC responses; `embedder_versions` populated; no fallback to defaults.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-EMBED-11" type="e2e" req_ref="REQ-EMBED-12">
  <description>Panel materialization latency benchmark.</description>
  <preconditions>Ensemble loaded; 10,000 representative chunks across 11 languages.</preconditions>
  <inputs>Materialize a panel for each chunk; record per-chunk wall-clock.</inputs>
  <expected_result>p50 &lt; 100 ms warm; p99 &lt; 500 ms cold; latency report serialized to FSV evidence.</expected_result>
  <priority>high</priority>
</test_case>

<test_case id="TC-EMBED-12" type="e2e" req_ref="REQ-EMBED-04">
  <description>Per-language routing coverage across the 11-language matrix.</description>
  <preconditions>AST chunker shipped for all 11 languages.</preconditions>
  <inputs>Chunk a representative file in each language; for each chunk, look up routing.</inputs>
  <expected_result>Every chunk's EntityType resolves; no MEJEPA_EMBED_ROUTING_MISSING; per-language deviations (if any) match the spec.</expected_result>
  <priority>high</priority>
</test_case>

</test_plan>

<open_questions>

<question id="Q-EMBED-01" status="open" assignee="operator">
  <text>Should the routing table be const-static or env-overrideable for experiments?</text>
  <context>REQ-EMBED-04 specifies const-static. An env override would speed iteration but breaks determinism contracts and replay verification. Operator decision needed before Phase 1 ships.</context>
  <resolution>(pending)</resolution>
</question>

<question id="Q-EMBED-02" status="open" assignee="operator">
  <text>How should the routing table handle Python-specific routes (e.g., decorators, async functions, dataclasses)? Per-language deviation tables, or expanded EntityType variants?</text>
  <context>Doc 14 §3.1 lists per-language node-kind tables. The cleanest design is per-language EntityType extensions; a per-language routing override map is simpler but harder to reason about.</context>
  <resolution>(pending — defer to Phase 1 chunker generalization)</resolution>
</question>

<question id="Q-EMBED-03" status="open" assignee="operator">
  <text>What is the calibration corpus for E17 AffectText on agent transcripts? 100 samples, 500, more?</text>
  <context>REQ-EMBED-09. Doc 12 §10 risk #4 says "100-500 outer-loop conversation samples." A specific size + scoring rubric needs to be locked before E17 enters production ΔΩ_outer.</context>
  <resolution>(pending — proposed 250 samples with operator-scored valence/arousal in {-1, -0.5, 0, +0.5, +1})</resolution>
</question>

<question id="Q-EMBED-04" status="open" assignee="operator">
  <text>For multi-shard weight files (e.g., E14 BGE-M3 sharded safetensors), is the digest the manifest SHA or per-shard SHAs?</text>
  <context>REQ-EMBED-02 currently specifies "concatenated manifest sorted by filename." Confirm before pinning all 21 digests in `models_config.toml`.</context>
  <resolution>(pending — proposed: per-shard SHAs in the toml + a derived top-level SHA over the concatenated sorted manifest)</resolution>
</question>

<question id="Q-EMBED-05" status="open" assignee="operator">
  <text>Should the bf16 / fp32 strict-replay path run on every encode (high overhead) or only on operator-triggered audit?</text>
  <context>REQ-EMBED-10 + BR-EMBED-06. Per-encode fp32 verification adds ~3× latency. Audit-only is much cheaper but defers detection.</context>
  <resolution>(pending — proposed: audit-only via `mejepa_audit_determinism` MCP tool, scheduled weekly + on every weight-digest change)</resolution>
</question>

<question id="Q-EMBED-06" status="open" assignee="operator">
  <text>Should `mejepa_audit_pairwise_mi` cache its result or recompute on every invocation?</text>
  <context>REQ-EMBED-11. Pairwise MI over 14 embedders × N corpus entries is O(N · 14²) — not cheap on a 100k corpus. Caching with corpus_sha as key is sensible but invalidates on any corpus mutation.</context>
  <resolution>(pending — proposed: cache keyed on (corpus_sha, embedder_versions), invalidate on either change)</resolution>
</question>

</open_questions>

<glossary>

<term name="EmbedderId">The serde-stable enum identifying one of E1-E21.</term>
<term name="Instrument trait">Per `crates/context-graph-mejepa-instruments/src/lib.rs`: a typed contract for "thing that produces a panel-slot vector." Extended in this spec with `embedder_id / version / weight_sha`.</term>
<term name="EmbedderInstrument&lt;E&gt;">A wrapper that adapts any of the 21 embedders to the Instrument trait surface.</term>
<term name="Routing table">Static map: `EntityType × Language → BTreeSet&lt;EmbedderId&gt; | InstrumentSlot`. Defines which embedders see which chunks.</term>
<term name="VRAM budget">29 GB hard cap on RTX 5090 — sum of resident embedder weights + predictor + activations + AdamW state + LoRA adapters + KV-cache.</term>
<term name="Frozen base">An embedder's base weight buffer that LoRA fitting MUST NOT alter (verified by SHA-256 pre/post fit).</term>
<term name="LoRA adapter">A low-rank `(A: hidden×rank, B: rank×hidden)` pair attached to query/key/value projections; trainable while base is frozen.</term>
<term name="embedder_coherence">`mean(jaccard(NN_i(panel), NN_j(panel)))` over content-embedder pairs — the structural ΔK supplement.</term>
<term name="predictor_collapse_score">`1 - std([cos(predicted, target) under E_i])` — the structural anti-collapse signal beyond pairwise MI.</term>
<term name="agent_state_score">`E17(transcript).valence · arousal_inverted_U(E17(transcript).arousal)` — the ΔΩ_outer agent factor.</term>
<term name="Code version">A string binding the libtokenizers + candle + cudarc crate versions for byte-equal determinism contracts.</term>
<term name="Weight SHA">SHA-256 of the embedder weight file (or sorted-manifest digest for multi-shard).</term>
<term name="EntityType">The AST chunker's atomic-entity tag: Function / Method / Class / Struct / Module / Impl / Comment / Docstring / Import / etc. Per doc 14.</term>
<term name="Inverted-U arousal">`arousal_inverted_U(a) = 4·a·(1-a)` — Yerkes-Dodson optimal-arousal mapping. Peaks at a=0.5.</term>

</glossary>

</functional_spec>
```
