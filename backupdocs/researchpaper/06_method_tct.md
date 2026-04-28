# 6. Teleological Constellation Training (TCT)

## 6.1 Motivation

The Derived Data Abundance argument of Section 3 is a data-enrichment principle: given a fixed corpus and $N$ independent embedders, one can produce $N + \binom{N}{2}$ labeled training signals per data point. But labeling alone does not explain how to *train with* the decomposition in a way that yields verifiable per-output guarantees at inference time.

The method we propose is **Teleological Constellation Training (TCT)**. The name is chosen deliberately: the Greek *telos* (purpose / end-state) captures the structural property that distinguishes the method from conventional scalar-reward training. In causal loss formulations — next-token prediction, next-frame prediction, reward-model optimisation — the objective is backward-looking: minimise error relative to past observations. In TCT, the objective is forward-looking: minimise distance from a frozen, purpose-defining set of centroids. The model's architecture, training loss, and runtime validator are all organised around the same end-state, which is the definition of teleological design.

## 6.2 Definition

A **Teleological Constellation** for a target $P$ is the set
$$
C_P = \{(m, c_m) : m \in \mathcal{M}\}
$$
where $\mathcal{M}$ is a set of perceptual modalities and $c_m = \mathrm{normalize}\left(\mathrm{mean}\left(\{\mathrm{normalize}(\phi_m(x_i))\}_{i=1}^{n_m}\right)\right)$ is the L2-normalised centroid of all L2-normalised embeddings extracted from a reference set $\{x_i\}$ of $P$ in modality $m$.

Optionally, centroids can be sub-typed by behavioural state $s$, in which case the constellation extends to $C_P = \{(m, s, c_{m,s})\}$ with each centroid computed only over reference embeddings classified into state $s$.

The constellation is **frozen** once constructed. It does not move during training or inference. It is the target against which both training and inference are oriented. Loss functions pull the model toward it; attention layers condition on it; runtime guards reject outputs that fall outside its neighbourhood.

## 6.3 The four-level expression hierarchy

For audiovisual applications, TCT derives not only identity centroids but also a behavioural vocabulary, organised into four levels.

**Level 0 — Action Units.** Seventeen FLAME expression coefficient indices (Li et al., 2017) mapped to FACS Action Units (Ekman & Friesen, 1978). This is the anatomical ground truth: AU12 (lip-corner pull), AU26 (jaw open), AU4 (brow lower), etc. Seventeen AUs cover the primary muscle groups responsible for the seven basic emotions and most secondary expressions. Mapping published as `FLAME_TO_AU` in `expression_skills.py`.

**Level 1 — Micro-Expression Groups.** Forty K-Means clusters (default $k=40$) over the subject's FLAME expression coefficients. Each cluster centroid is a discovered *personally distinctive* facial configuration — not generic expressions, but *this subject's* expressions: the specific way their brow lifts when surprised, the specific way their jaw tenses when emphatic. Active AUs within a cluster are identified by thresholding deviation from the neutral (mean) expression at 0.5, with a fallback to the top three by magnitude. The clustering captures the individuality of facial behaviour without requiring manual labelling.

**Level 2 — Expression Skills.** Thirty-four named behavioural primitives each with three temporal phases (onset, peak, offset) and a natural-language prompt for the downstream generator. Examples include `genuine_laugh`, `warm_smile`, `thoughtful_pause`, `gaze_down`, `voice_waver`, `brave_smile`, `deep_breath`, `eyes_wide`, `animated_gesture`, `emphatic_nod`, `brow_furrow`, `soft_eyes`, `slow_nod`, `lip_press`, `head_tilt`, `gaze_up`, `slight_frown`, `pause`, `slow_blink`, `chin_rest`, `rapid_speech`, `lean_forward`, `hand_gesture`, `steady_gaze`, `slow_speech`, `minimal_movement`, `slight_nod`, `eye_twinkle`, `half_smile`, `slight_pause`, `warm_chuckle`, `slight_gesture`. Skills are built from temporal clustering: 10-frame sliding windows of cluster labels are histogrammed and re-clustered into skill clusters (default $k=25$). Temporal phases are 0.2 s onset, 0.5 s peak, 0.3 s offset, totalling one second per execution. Intensity modulation applies prompt prefixes: $<0.3$ prepends "very subtly"; $<0.6$ prepends "slightly"; $>0.9$ prepends "intensely".

**Level 3 — Behavioural Constellations.** Eight emotional states expressed as cycling sequences of skills (roughly one skill every 2 s). The eight production constellations used in Case 2 are:

| Name | Emotion blend | Skills |
|---|---|---|
| `warm_conversational` | joy + trust | `warm_smile, gentle_nod, eye_contact, slight_gesture` |
| `emotional_recall` | sadness + trust | `thoughtful_pause, gaze_down, voice_waver, brave_smile, deep_breath` |
| `happy_storytelling` | joy + anticipation | `genuine_laugh, eyes_wide, animated_gesture, warm_smile, emphatic_nod` |
| `empathetic_concern` | sadness + trust | `brow_furrow, soft_eyes, slow_nod, lip_press, head_tilt` |
| `thoughtful_reflection` | anticipation + trust | `gaze_up, slight_frown, pause, slow_blink, chin_rest` |
| `excited_sharing` | joy + surprise | `eyes_wide, rapid_speech, lean_forward, hand_gesture, genuine_laugh` |
| `solemn_gravity` | trust + sadness | `steady_gaze, slow_speech, minimal_movement, slight_nod, deep_breath` |
| `gentle_humor` | joy + surprise | `eye_twinkle, half_smile, slight_pause, warm_chuckle` |

All four levels are derived automatically from the decomposed data. No manual labelling is required. The behavioural vocabulary is a function of the subject's own recorded material, and new subjects produce different vocabularies without any additional annotation work.

## 6.4 The three phases

### Phase 1 — Constellation Construction

From the reference corpus, run every frozen embedder $\phi_m$ and accumulate per-modality L2-normalised samples. Compute each centroid $c_m$ as the normalised mean of the normalised samples. For audiovisual applications, also fit FLAME parameters per frame (at ~82 fps on RTX 5090) and run the K-Means clustering that produces Level 1 groups. For behavioural-state-conditioned constellations, first classify each reference sample into a state (e.g., "warm conversational", "emotional recall") via transcript-content cues, prosody features, or explicit timeline labels, then compute per-state centroids.

Context Graph's `compile_constellation` MCP tool and ClipCannon's equivalent expose Phase 1 as a one-call operation parametrised by a `ConstellationSelector` (topic, session, tag, time range, or explicit UUID list). The compiler runs in two passes: pass 1 observes each member fingerprint, feeding its per-embedder vectors into `WelfordVector` online-mean accumulators and reservoir samplers; pass 2 walks the buffered observations to compute per-member cosines against the now-known centroids and finalises spread statistics (L2 mean/stddev, cosine p50/p95, min/max, coverage).

The resulting artifact is a single bincode `Constellation` record (written to RocksDB CF `constellations`, plus a selector-index entry in `constellation_by_selector`) containing:

- 13 (or 7) `EmbedderStats` entries: centroid, per-embedder coverage, cosine p50/p95, L2 mean/stddev;
- a 13D topic-profile centroid (or 7D group centroid);
- a 78D cross-correlation centroid;
- aggregate `coherence` (median E1 / visual cosine of members to centroid) and `purity` (for Topic selectors only);
- the list of member IDs.

### Phase 2 — Constellation-Conditioned Training

A generative model (we use EchoMimicV3 + LoRA for Case 2 in Section 8, Qwen3-TTS via ICL for Case 3 in Section 9, and Gemma-class + LoRA + DPO for Case 1 in Section 7) is fine-tuned with losses that explicitly incorporate distance from the constellation centroids. For audio-conditioned video diffusion, the training loss is flow-matching with an additional constellation-proximity term; for LLM fine-tuning, the training loss is standard cross-entropy with rejection-sampling against constellation membership at candidate selection time.

The critical loss-level observation in Case 2 was that flow matching in the EchoMimicV3 / Wan2.1-Fun family uses a velocity convention in which
$$
\text{target} = \text{noise} - \text{latent}
$$
and a sign flip in the loss formulation causes the model to learn to anti-denoise — that is, to move toward the noise distribution rather than toward clean video. The sign bug is discussed in detail in Section 8.4. For the purposes of the TCT framework, the point is that the *sign* and *weighting* of the flow-matching loss terms are where constellation proximity must be preserved: mouth-weighted loss (10× mouth, 2× face, 1× background) redirects gradient mass toward the regions where the visual constellation is most perceptually sensitive to drift.

### Phase 3 — Runtime Constellation Guard

At inference, every generated output is re-embedded through the same frozen models used to build the constellation and compared against each centroid via cosine similarity. A guard module (`constellation_guard.py` in Phoenix) accepts or rejects each output based on per-modality thresholds.

For the Santa video clone (Section 8) the thresholds are:

| Modality | Threshold | Severity tiers (visual only) |
|---|---|---|
| Visual | 0.70 | WARN at 0.80, STRONG at 0.90 |
| Emotion | 0.60 | — |
| Speaker | 0.80 | — |
| Voice | 0.85 | — |

These are conservative starting points, configurable per deployment. Speaker and voice thresholds are higher than visual because vocal identity is a more sensitive signal (a face can change with lighting and angle; a voice cannot). Rejected outputs are either regenerated with a different seed, replaced with the previous chunk's last frame, or escalated to a human operator, depending on deployment policy.

The guard's architecture is:

```
generated frame/chunk
    → re-embed through frozen SigLIP-SO400M (visual) and/or frozen speaker encoder (audio)
    → cosine similarity vs constellation centroid
    → compare to per-modality threshold
    → accept or reject (with `rejection_reason`)
```

The same encoder used to build the centroid is used to embed every generated frame. This is the guarantee that makes cosine similarity meaningful: it answers the literal question "does this frame, when embedded through the same encoder that produced the centroid, fall within distance $D$?" If yes, the frame is in distribution with the identity; if no, it has drifted. The guard runs continuously on the output stream. If a sustained drift is detected (a stretch of frames all below threshold), the system pages the operator.

## 6.5 Geometric properties

The constellation has three properties relevant to its use as an alignment target:

1. **Density.** With 4,044 dimensions across seven modalities (Case 2) or 11,008+ across thirteen (Context Graph), the manifold is dense enough that drift in any one modality is detectable. The seven modalities impose independent constraints; satisfying one does not satisfy the others. A model that looks like the target but sounds different fails; a model that sounds right but has the wrong emotional valence fails. The per-modality checks cannot trade off against each other.
2. **Stability.** The centroid is a frozen point. It does not move during training or inference. The alignment target is therefore invariant — there is no drift in the target itself, only in the model's outputs relative to it. This is in structural contrast to scalar-reward alignment, where the reward model is itself updated and can drift.
3. **Decomposability.** Each modality contributes an independent constraint. Adding a new modality to the constellation extends the manifold in a new direction without changing the existing constraints. Removing a modality relaxes the constraint in that direction without reshaping the others. The architecture is open-ended: as new embedders become available (V-JEPA-style visual representations, touch embeddings, gesture embeddings, gaze embeddings), they can be appended to the constellation without retraining the base model.

## 6.6 Runtime control — the ConstellationController

TCT's generative side uses a runtime orchestrator we call the *ConstellationController* (Phoenix `constellation_controller.py`). It exposes a minimal public API:

| Method | Purpose |
|---|---|
| `set_emotional_state(constellation, intensity)` | Activate a Level 3 behavioural constellation with a continuous intensity |
| `queue_expression(skill, duration, at_time)` | Schedule a specific Level 2 expression skill at a specific time |
| `get_prompt_for_frame(frame_idx, fps=25)` | Return the natural-language prompt for a specific frame |
| `get_prompt_sequence(duration_s, fps=25)` | Return a sequence of (frame_idx, prompt) tuples for a full duration |
| `blend_constellations(a, b, ratio)` | Blend two constellations (0.0 pure *a* → 1.0 pure *b*) |
| `transition_to(new_constellation, duration_s=0.5)` | Schedule a smooth transition between constellations |

Per-frame prompt resolution follows a strict priority order:

1. Queued expressions take precedence (with onset/peak/offset intensity ramps).
2. Active transitions blend the old and new constellation prompts.
3. The current constellation's cycling skill sequence fills the remainder.

A one-shot expression queued for a specific moment overrides the background cycle for its duration and returns to the cycle on completion — producing continuous but controllable behavioural conditioning.

The MCP tool `clipcannon_expression_sequence` exposes `get_prompt_sequence` so an AI assistant can *preview* the per-frame prompts that will drive a generative pass before committing to the full (expensive) video generation.

## 6.7 Integration with the generator

In Case 2 (Section 8), the generation flow is:

```
1. ConstellationController.set_emotional_state("warm_conversational", 0.8)
2. ConstellationController.get_prompt_sequence(duration_s=10.0, fps=25)
   → 250 (frame_idx, prompt) tuples
3. EchoMimicV3 inference per chunk (10 s, 768×768):
   - reference image: subject portrait, face-centred crop
   - audio: 10 s, synthesised via Qwen3-TTS in the cloned voice
   - LoRA weights: subject-specific fine-tune
   - text prompt: per-frame prompt from the controller
   - sampler: Flow_DPM++; 20 steps
4. ConstellationGuard.validate_batch(generated_frames)
   → If any frame fails visual threshold, regenerate the chunk
5. Audio remux
6. Output video with cosine similarity ≥ 0.70 (visual) and ≥ 0.80 (speaker)
```

For videos longer than the model's native 10-second window, the system uses multi-chunk generation with 0.5 s audio overlap and pixel crossfade. Each chunk uses the *original reference image* rather than the previous chunk's last frame — empirically the latter produces quality degradation chains.

## 6.8 Provenance and the ConstellationGuard API

The ConstellationGuard exposes (Phoenix `constellation_guard.py`):

| Method | Purpose |
|---|---|
| `load_visual_encoder()` | Load frozen SigLIP (~2 GB VRAM) via `gpu_guard.check_vram`; requires `HF_HUB_OFFLINE=1` |
| `validate_frame(frame_rgb)` | Single-frame validation: embed and compare |
| `validate_batch(frames)` | Batched validation in a single forward pass |
| `validate_audio(audio_embedding, modality)` | Audio validation against speaker/voice centroids |
| `get_constellation_stats()` | Centroid dimensions, sample counts, spread, thresholds |
| `unload()` | Free SigLIP and GPU memory |

Each call returns a `ValidationResult(valid: bool, visual_similarity: float, details: Dict[modality, float], rejection_reason: Optional[str])`.

The offline-mode enforcement is a deliberate rule (documented as `feedback_no_model_downloads.md` in the system): `HF_HUB_OFFLINE=1` and `TRANSFORMERS_OFFLINE=1` prevent silent runtime downloads. In production, runtime model fetches are a common failure mode and a real attack surface; the guard either has the model already cached or it fails loudly.

## 6.9 Alignment: scalar reward vs. geometric constraint

The section-long argument of Section 2.6 comes into focus here. TCT redefines alignment as proximity to a point in embedding space, with the following consequences:

| Property | Scalar reward (RLHF/DPO/Constitutional) | Geometric constraint (TCT) |
|---|---|---|
| Target | Learned preference model (proxy) | Frozen centroid (direct definition of identity) |
| Drift | Possible (reward hacking, distribution shift in the reward model) | Bounded by threshold |
| Verifiability | Indirect (must measure preference across many samples) | Direct (per-output cosine similarity) |
| Human feedback | Required ongoingly to refresh the reward model | Required only for centroid construction |
| Failure mode | Goodharting, mode collapse toward reward-gaming outputs | Frame rejection, regeneration |
| Generalisation | Implicit | Explicit: the centroid is what generalises |

The geometric constraint approach is not a universal replacement. It works well for problems whose alignment target is *definable* in embedding terms — identity-locked generation, style control, safety profiles expressible as classifier regions, content-moderation constellations built from reference "acceptable behaviour" examples. It does not obviously apply to open-ended "be helpful and harmless" objectives where the target is a capability rather than a measurable attribute.

For the subclass of problems where the target is definable, TCT provides a property no reward model can match: **every output is individually verified against a fixed criterion before release**. There is no batch evaluation, no held-out set, no offline measurement — just a per-output cosine check that runs at inference time and produces a boolean result with a human-readable rejection reason. This is the property that makes TCT suitable for autonomous deployment (meeting bots, unsupervised content pipelines, long-running avatar agents).

## 6.10 Relationship to the wider DDA argument

TCT is to DDA as supervised learning is to labelled data: it is the training method that uses the labels DDA produces. Without DDA, TCT has no multi-modal target to aim at; without TCT, DDA is just an enrichment procedure whose outputs sit in storage.

The three-case empirical validation in Sections 7–9 is therefore a validation of both claims at once. If TCT produces verifiable identity-locked outputs from small subject-specific datasets, that is evidence both (i) that DDA's multi-modal labels carry enough training signal to enable sample-efficient training of identity-locked generators, and (ii) that constellation-as-alignment-target is a workable alternative to scalar reward for the subclass of problems where the target is embedding-definable.

Section 7 describes the Shakespeare case study. Section 8 describes the Santa LoRA case study. Section 9 describes the voice-cloning case study.
