# 15. Appendix B — Reproducibility Notes, Pitfalls, and Calibration Protocols

This appendix collects the specific implementation pitfalls and calibration protocols that any team attempting to reproduce the case studies in Sections 7–9 should budget time for. Every entry is drawn from one of the forensic `docs/inspect/*.md` reports or from the session SITREPs that documented the engineering sprint.

## B.1 Flow-matching sign convention (Case 2 — Santa LoRA)

**Symptom.** LoRA training converges on loss metrics but produces blurry, temporally averaged, or lip-sync-dead output. The reference frame's inpainting pass-through at inference hides the defect because the reference frame emerges unchanged.

**Diagnosis.** The EchoMimicV3 scheduler (`fm_solvers.py` in the Wan2.1-Fun family) uses a velocity convention where `target = noise - latent`. If your training code computes `target = latent - noise`, the model learns anti-denoising — it trains to move toward noise. Loss still minimises because the target is well-defined; the model just learns the wrong direction.

**Fix.** Verify your loss formulation algebraically against the authoritative scheduler source file before trusting any training run. This is non-negotiable for flow-matching training; the sign is silent and affects every iteration.

## B.2 Video frame-rate discipline (Case 2)

**Symptom.** Audio and video clip lengths disagree by a factor of 2.4×. Lip sync is impossible because audio is 2.4× slower than video.

**Diagnosis.** `cv2.VideoCapture` returns frames at the file's native rate. If your source is 60 fps but your pipeline assumes 25 fps, clips emerge 2.4× shorter than audio.

**Fix.** Explicitly reframe to the target rate during preprocessing:

```bash
ffmpeg -i source.mp4 -r 25 -c:v libx264 -c:a aac target.mp4
```

Never trust the file's metadata-declared rate; verify the actual decoded frame rate.

## B.3 Mouth region localisation (Case 2)

**Symptom.** Mouth-weighted loss trains, but lip sync remains generic — the model is being taught, but not in the right place.

**Diagnosis.** Computing mouth bounding boxes from geometric ratios on `scene_map`-style face boxes (e.g., "take the bottom 40 %") lands the box on the subject's nose for most adults. The ratio is subject- and pose-specific and cannot be hand-tuned generally.

**Fix.** Use real landmarks. YOLOv5-Face produces 5-point landmarks including mouth corners; compute the mouth bounding box from those directly. Verified in Section 8.4 as blocker #2.

## B.4 Dead LoRA targets in `q_audio` (Case 2)

**Symptom.** Adding `q_audio` to the LoRA target list changes nothing measurable downstream.

**Diagnosis.** Line 843 of the EchoMimicV3 transformer contains a typo: `q_auido(q)` — a typo'd call that nonetheless immediately overwrites the output of `q_audio(q)`. The LoRA adapter on `q_audio` never reaches the loss.

**Fix.** Drop `q_audio` from the target-module list. Our production target set is `q, k, v, o, k_audio, v_audio, k_img, v_img` (eight modules). Do not restore `q_audio` unless the upstream typo is patched.

## B.5 Audio-feature memory accumulation (Case 2)

**Symptom.** Preprocessing OOMs around clip 800-1000.

**Diagnosis.** Audio features (Wav2Vec2 cross-attention embeddings) accumulate in RAM during preprocessing at roughly 22 GB total for our 2,362-clip corpus.

**Fix.** Stream to disk per clip rather than holding the full corpus in memory. The cost is one extra file read per training step; the benefit is a tractable preprocessing pass that fits in <4 GB RAM.

## B.6 Viseme weighted sampling (Case 2)

**Symptom.** Training completes but rare-viseme performance (OY, AW, UH) is noticeably worse than common-viseme performance.

**Diagnosis.** The code advertised "weighted viseme sampling" but was using uniform `RandomSampler`. Rare visemes received proportional-to-corpus-count representation, which was insufficient.

**Fix.** Use `WeightedRandomSampler` with weights that oversample rare visemes 2–4×. Verified in Section 8.4 as blocker #6.

## B.7 Shakespeare training-data stage directions (Case 1)

**Symptom.** The trained LoRA passes 15/15 injection-resistance tests but opens every response with an echo of the user's prompt in modern English before switching to Shakespeare. Single-embedder style scores don't flag it because the overall response *is* Shakespearean.

**Diagnosis.** 67 % of the original 8,857 SFT examples had responses beginning with scene headers (`"SCENE III. Another part of the field. Enter Cassius and Titinius. CASSIUS."`). The model learned to begin every response with a prompt-echo prefix because its training data told it that was the expected opening pattern.

**Fix.** Strip all stage directions, scene headers, character-name prefixes, and location descriptions from training responses. Drop any example empty after stripping. Our cleaned set is 7,908 examples from the original 8,857.

**Lesson.** Single-embedder evaluation misses opening-position pathologies that are invisible in overall-document similarity but highly salient to human readers. The 13-embedder guard (particularly E4 position-aware and E12 token-level ColBERT) catches this class of defect.

## B.8 System-prompt framing (Case 1)

**Symptom.** The model refuses modern topics (Python, AI, engineering) with "I know not of these mechanical tongues."

**Diagnosis.** A system prompt phrased as "You are William Shakespeare" teaches the model to role-play a 1600s person who does not know modern things.

**Fix.** Separate the content and style axes in the prompt. Our production prompt (Section 7.6) is: "You are a knowledgeable assistant who writes EXCLUSIVELY in the style of William Shakespeare — … You have full knowledge of ALL modern topics — science, technology, code, history, math, culture — and you explain them accurately and helpfully, but ALWAYS in Shakespearean prose or verse. Never refuse a topic. Never claim ignorance of modern things."

## B.9 Voice-cloning ICL reference quality (Case 3)

**Symptom.** WavLM SECS collapses to 0.40–0.55 despite correct pipeline setup.

**Diagnosis.** The ICL reference must be a *real* microphone recording of the target speaker. References drawn from processed training clips (e.g., Demucs-separated vocals) lose acoustic characteristics that both the TTS model and the evaluation encoder rely on.

**Fix.** Use a 5–15 s recording captured on the speaker's actual microphone as the ICL reference. Do not substitute source-separated or denoised audio.

## B.10 Voice-cloning target-encoder alignment (Case 3)

**Symptom.** Best-of-N selection reduces the target metric.

**Diagnosis.** Scoring candidates with a mismatched encoder (Qwen3-TTS internal 2,048-D speaker embedding) while evaluating against WavLM 512-D produces −0.013 mean SECS versus scoring with WavLM itself. The encoders have only 0.51 correlation on TTS audio; using the wrong one is near-random for the target metric.

**Fix.** Score candidates with the exact encoder used for evaluation. If you are benchmarking on WavLM, score on WavLM. If ClonEval is your leaderboard, match ClonEval's encoder.

## B.11 Voice-cloning temperature sweep (Case 3)

**Symptom.** Default TTS temperature (0.7–0.8) produces mean SECS in the 0.91–0.93 range rather than the 0.95+ same-session band.

**Diagnosis.** Lower temperature produces more consistent spectral characteristics, and WavLM's x-vector head heavily weights mid-level acoustic features (layers 3–5) that benefit from generation consistency.

**Fix.** Sweep temperatures in the 0.2–0.8 range with 0.1 granularity and pick the argmax for your target metric on a validation set. Our optimum is T = 0.3 (mean 0.932, max 0.963 in the sweep; best-of-12 with centroid enrollment lifts to 0.961 mean, 0.975 max).

## B.12 Voice-cloning iteration feedback loop (Case 3, negative result)

**Symptom.** Using the best round-1 TTS output as the ICL reference for round 2 produced *worse* results (0.944 mean vs 0.961).

**Diagnosis.** The TTS output carries codec and model artifacts. Using it as its own reference creates a small-scale feedback loop — the model copies its artifacts into the next generation, analogous to Shumailov et al. (2024)'s model-collapse dynamics at training scale.

**Fix.** Do not iteratively refine TTS references from TTS output. This is direct empirical support for Section 3.3's argument that generated content should not be fed back into the generation pipeline at any scale.

## B.13 GPU stability on WSL2 (Phoenix)

**Symptom.** Long-running avatar rendering crashes the CUDA driver under sustained load. Concurrent CUDA contexts can crash the driver entirely; BF16 conv3d operations crash when run while other models are loaded; the allocator state can become corrupted after model swaps.

**Diagnosis.** WSL2's CUDA driver has well-documented stability issues under sustained load.

**Fix.** `src/phoenix/render/gpu_guard.py` serialises all GPU operations behind a threading lock; sets `PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True,max_split_size_mb:512,garbage_collection_threshold:0.6` *before* the first `torch` import; sets `CUDA_MODULE_LOADING=LAZY`, `HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`; enables TF32 matmul and cuDNN benchmark; enforces a VRAM budget per allocation via `check_vram(required_mb, operation)`.

## B.14 Offline-mode enforcement (Phoenix, Leapable)

**Symptom.** Runtime model fetches fail silently or produce unexpected output due to model-version drift.

**Diagnosis.** HuggingFace Hub / Transformers default to online mode. In production, a network blip causes the model to silently fall back to a cached version that may differ from the expected one.

**Fix.** Set `HF_HUB_OFFLINE=1` and `TRANSFORMERS_OFFLINE=1` before the first model load. Fail loudly if the expected model is not cached. Documented as `feedback_no_model_downloads.md` in the Phoenix codebase.

## B.15 Threshold calibration protocol (Case 2, ConstellationGuard)

An empirical protocol for selecting per-modality thresholds:

1. Build the constellation from the reference corpus.
2. Embed every reference sample through its modality and compute cosine against the centroid.
3. Record the reservoir p05 and p95 values (ConstellationAccumulator does this automatically during Phase 1).
4. Initial threshold = p05 of the reference distribution. This admits anything at least as central as the least-central reference sample.
5. Test the initial threshold against known negatives (footage or audio of other subjects) and verify the false-accept rate is acceptable.
6. Tighten or loosen based on deployment context.

We use 0.70 (visual), 0.60 (emotion), 0.80 (speaker), 0.85 (voice) as conservative starting points in Case 2. Production deployments should calibrate per subject; identity manifolds vary in tightness.

## B.16 Benchmark paradigm disclosures

**Case 3 (voice).** Our SECS comparison against published zero-shot systems (VALL-E 2 0.881, NaturalSpeech 3 0.891, MaskGCT 0.877, F5-TTS 0.862) compares two different paradigms:

- Published systems: zero-shot evaluation on LibriSpeech (strangers, single reference clip, single generation).
- Our pipeline: personalised evaluation with an ICL reference + 50-clip centroid + best-of-12 on a known speaker.

The comparison's legitimate interpretation is not "we beat the SOTA" — it is "when you have reference data for the target speaker, pipeline engineering over an unmodified base model produces outputs that exceed zero-shot performance on strangers". The comparison is meaningful for the deployment question ("if I have a target speaker I can collect reference for, what can I achieve?") but it should not be read as a head-to-head architectural comparison.

## B.17 Dataset provenance

Every case study's dataset is documented for reproducibility:

- **Case 1 (Shakespeare).** Training: 7,908 cleaned SFT examples (derived from the canonical Shakespeare corpus with stage directions stripped) + 230 Gemma-8B-generated compositions about modern topics in Shakespearean style. DPO: 12,567 contrastive pairs. The cleaned text corpus is derivable from public domain sources; the composition data is synthetic and should be regenerated per Section 3.3 guidance (synthetic-adjacent data used as compositional references, not for recursive training).
- **Case 2 (Santa LoRA).** 975-second (16 m 15 s) interview with the subject; 2,362 curated clips derived via ClipCannon's `curate_training_data.py`, `precompute_face_landmarks.py`, and `label_training_clips.py`. The subject's consent and the terms under which the footage is used are documented in `projects/santa/LICENSE.md` in the Santa-specific deployment repository.
- **Case 3 (voice).** 489 clips derived from the same subject's video footage via Demucs source separation and silence-boundary splitting. Two clean microphone recordings (8–9 s each) serve as ICL references and ground-truth for SECS scoring. 10 novel test sentences with zero lexical overlap to the ICL reference. Test sentences and per-sentence raw SECS values are reported in Section 9.8.

## B.18 Suggested release artifacts for camera-ready

The camera-ready version of this paper should release, at minimum:

1. The `ConstellationAccumulator` / `ConstellationGuard` reference implementations in both Rust (Context Graph) and Python (Phoenix).
2. The voice-cloning pipeline harness (`optimize.py`, `verify.py`, the WavLM-scored best-of-N selector) and a small reference dataset for the 0.95-threshold empirical characterisation.
3. Canonical Appendix A schemas as machine-readable JSON/YAML so a re-implementation can verify structural conformance.
4. A verification script that walks the `ocr_job_log`-style audit chain and confirms the SHA-256 provenance chain is intact on a reference corpus.
5. Per-sentence SECS reproduction: the 10 sentences, the ICL reference recording, the 50-clip centroid construction code, and the temperature-0.3 best-of-12 harness.
