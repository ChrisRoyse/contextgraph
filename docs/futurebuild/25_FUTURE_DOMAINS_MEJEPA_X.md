# 25 — Future Domains: ME-JEPA-Voice / -Image / -X

> **Purpose.** ME-JEPA-Code is the code-domain instance of TCT (per `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §6`). The architecture is general; future domain packs follow the same template. This doc specifies the template for adapting to a new modality.
>
> **Scope.** This is **deferred** per `17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md §O4`. The doc exists so future operators can extend the system without re-deriving the architecture.

---

## 1. The TCT template (single source of truth)

Per `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §6`, the universal template:

> Pick N frozen embedders covering the modality, build constellations from real working examples, train a predictor with constellation-attraction loss, gate inference with `Gτ`.

For any new modality M, the steps are:

1. **Define instruments.** N frozen externally-defined functions that map a typed event in domain M to a structured feature output. For ME-JEPA-Voice: E_Spectrogram, E_Phoneme, E_Prosody, E_Speaker, E_Acoustic, E_Transcript, etc. For ME-JEPA-Image: E_Pixel, E_Depth, E_Object, E_Semantic, E_Caption, etc.
2. **Define the panel.** Concatenation of all instrument outputs to a fixed-dim panel.
3. **Define the predictor.** Same architecture: 6-layer transformer trunk + per-head Linear projections.
4. **Define the heads.** Domain-specific (e.g., voice quality, deepfake-likelihood, transcription accuracy, prosody appropriateness).
5. **Define the corpus.** Mutation operators specific to M (e.g., for voice: add noise, shift pitch, scrub by 100ms, replace one phoneme; for image: cutout, color-jitter, add adversarial perturbation).
6. **Define the oracle.** What's the deterministic verifier in M? For voice: speaker-verification system + transcription accuracy. For image: object detection ground truth + classification benchmark.
7. **Train + calibrate + gate.** Same as code domain.

---

## 2. Plugin architecture (D8)

To support multiple domains in the same daemon:

```rust
// crates/context-graph-mejepa-domain/src/lib.rs (NEW for future)
pub trait Domain: Send + Sync + 'static {
    type Event;
    type Panel;
    type Verdict;
    
    fn instruments(&self) -> &[Box<dyn Instrument<Self::Event>>];
    fn predictor(&self) -> &MeJepaPredictor;
    fn constellation(&self) -> &TctConstellation;
    fn corpus_root(&self) -> &Path;
    fn domain_name(&self) -> &str;  // "code", "voice", "image", ...
    
    fn verify(&self, input: &Self::Event, context: &DomainContext) -> Self::Verdict;
    fn observe(&self, event: &Self::Event, ground_truth: &Self::Verdict) -> Result<()>;
}
```

Domain implementations live in their own crates:
- `context-graph-mejepa-code` (current; this is what the futurebuild plan ships)
- `context-graph-mejepa-voice` (future)
- `context-graph-mejepa-image` (future)
- `context-graph-mejepa-x-template` (template for new domains)

The daemon loads a configurable set of domains from `~/.config/mejepa/domains.toml`. Each domain has its own CFs (prefixed `CF_<DOMAIN>_*`), its own MCP tool subset (`mejepa_<domain>_verify` etc.), its own scheduler workload.

Storage: each domain claims a budget under `/mnt/d/contextgraph/<domain>/`.

---

## 3. Resource sharing across domains

Domains share:
- The CUDA device (queued via the existing CUDA queue manager)
- The witness chain (one chain per machine)
- The operator (single operator manages all domains)

Domains do NOT share:
- Predictor weights
- Constellations
- Calibrations
- Patch / event corpora
- Active-learning queues

Operator-side dashboards aggregate across domains for a single weekly report.

---

## 4. Specific domain templates

### 4.1 ME-JEPA-Voice (sketch)

| Element | Voice instantiation |
|---|---|
| Event | (audio_clip, target_text, speaker_id) |
| Panel slots | E_Spectrogram (2048-d), E_Phoneme (512-d), E_Prosody (256-d), E_Speaker (256-d), E_Acoustic (512-d), E_Transcript (1024-d), E_Witness, E_Oracle, E_Reasoning, ... |
| Predictor target | post-encoding panel (e.g., predicted transcript correctness, predicted speaker verifiability) |
| Heads | transcript_accuracy, speaker_verifiability, deepfake_likelihood, prosody_appropriateness, audio_quality |
| Mutation operators | add_noise, pitch_shift, time_stretch, swap_phoneme, splice, add_silence, codec_compress, replace_word |
| Oracle | Whisper-class transcription system + speaker-verification system |
| Storage budget | similar to code domain (~30-50 GB) |

### 4.2 ME-JEPA-Image (sketch)

| Element | Image instantiation |
|---|---|
| Event | (image, target_label, target_caption) |
| Panel slots | E_Pixel (CLIP-class, 1024-d), E_Depth (256-d), E_Object (1024-d), E_Semantic (1024-d), E_Caption (1024-d), E_Style (256-d), E_Witness, E_Oracle, ... |
| Predictor target | post-encoding panel |
| Heads | classification_accuracy, object_detection_recall, caption_quality, adversarial_perturbation_likelihood |
| Mutation operators | random_crop, color_jitter, adversarial_perturbation, swap_object, blur, occlusion |
| Oracle | object detection benchmark + classification benchmark |
| Storage budget | larger (~100-200 GB; images are big) |

---

## 5. Cross-domain transfer

Some signals transfer:
- The witness chain primitive transfers unchanged
- The conformal predictor primitive transfers unchanged
- The TCT Gτ guard transfers (per-domain centroids)
- The active-learning queue transfers
- The continuous self-optimization scheduler transfers

Some signals don't transfer:
- Specific instruments (voice's E_Spectrogram makes no sense for code)
- Specific mutation operators (`SubtleFlip` for voice would be silly)
- Specific heads (transcription_accuracy is voice-specific)

The futurebuild plan's primitives in `crates/context-graph-mejepa{,-cf,-train,-tct,-shift-subscriber,-hygiene}` are domain-agnostic and stay shared. Only the `crates/context-graph-mejepa-code` (or analogous) per-domain crate is replaced.

---

## 6. Integration with reality

For ME-JEPA-Code, reality is "Docker test outcome." For other domains:

- **ME-JEPA-Voice**: reality is "human listener verdict + automated transcription accuracy"
- **ME-JEPA-Image**: reality is "human classifier verdict + automated object detection"
- **ME-JEPA-X**: reality is whatever deterministic verifier exists for X

The shared property: **reality is something other than the agent itself**. The system is structurally bound to a verifier the agent doesn't control.

---

## 7. What this commits us to

This doc is **deferred work**. No tasks are added in the immediate plan. The doc exists so:

- Operators considering ME-JEPA expansion know the cost shape (build the domain crate; reuse the substrate).
- Future development can use this template without re-deriving from `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`.
- The `Domain` trait extraction (per §2) becomes a refactoring task IF and WHEN a second domain is funded.

When it's time:
- `TASK-DOMAIN-001` — extract `Domain` trait from `mejepa-code`-specific code
- `TASK-DOMAIN-002` — voice domain crate scaffold (per §4.1)
- `TASK-DOMAIN-003` — image domain crate scaffold (per §4.2)

These are weeks of work each. Out of scope for v1.
