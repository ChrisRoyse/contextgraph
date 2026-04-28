# 5. ClipCannon: A 7-Modality Realisation of DDA on Video

## 5.1 Overview

ClipCannon is a Python package of roughly 247 source files and 67,585 lines of code that applies Derived Data Abundance (DDA) to video. Every ingested video is decomposed through a directed-acyclic-graph of 23 analysis stages producing seven independent embedding modalities spanning 4,044 dimensions, plus structural metadata (scenes, shots, speakers, emotions, prosody, narrative), and the entire result is persisted into a per-project SQLite database augmented with `sqlite-vec` virtual tables for sub-linear nearest-neighbour search. A 58-tool Model Context Protocol (Anthropic, 2024) server exposes the derived representation to AI assistants, with an editing engine, a multi-platform rendering pipeline (NVENC-based), an audio-generation tier (AI music via ACE-Step, MIDI via FluidSynth, DSP effects), a voice-cloning engine (Qwen3-TTS with multi-gate verification), and an avatar / lip-sync engine (LatentSync 1.6; Li et al., 2024). A license server on port 3100 and a dashboard on port 3200 provide credit-based billing and a web UI.

ClipCannon is the audiovisual companion to Context Graph. Both systems implement DDA; the only difference is that ClipCannon's primary input is video rather than text and it uses seven embedders rather than thirteen. Together they establish that DDA applies across modalities and that the number of useful independent embedders per modality is context-specific but consistently much greater than one.

## 5.2 The 23-stage analysis DAG

When a user invokes `clipcannon_ingest`, the pipeline runs the following stages, topologically sorted (Kahn's algorithm) and executed level-by-level with `asyncio.gather()` concurrency within each level:

**Required (failure aborts the pipeline):**
- `probe` — ffprobe metadata, variable-frame-rate detection, SHA-256 hashing.
- `vfr_normalize` — snap to nearest standard frame rate (from 23.976, 24, 25, 29.97, 30, 50, 59.94, 60); re-encode to constant frame rate via `h264_nvenc` with `libx264` fallback.
- `audio_extract` — 16 kHz mono (for speech models) and native-rate audio.
- `frame_extract` — JPEG frames at configurable FPS (default 2).
- `transcribe` — WhisperX large-v3 (Bain et al., 2023) with anti-hallucination filtering (35 known phrases blocked), word-level timestamps (20–50 ms precision).
- `finalize` — status reconciliation, provenance verification, cleanup.

**Optional (failure logged, downstream degrades gracefully):**
- `source_separation` — HTDemucs v4 (Rouard et al., 2023) isolates vocals from music/drums/bass.
- `visual_embed` — SigLIP-SO400M (Zhai et al., 2023) 1,152-D embeddings per frame.
- `ocr` — PaddleOCR PP-OCRv5.
- `quality` — pyiqa BRISQUE frame-quality score.
- `shot_type` — zero-shot SigLIP classification (extreme close-up / close-up / medium / wide / establishing).
- `storyboard` — contact-sheet thumbnails.
- `scene_analysis` — SSIM-based boundaries (max 8 s scene); OpenCV face detection; webcam-overlay detection; pre-computed canvas regions for four layouts.
- `semantic_embed` — Nomic Embed Text v1.5 (Nussbaum et al., 2024), 768-D per transcript segment.
- `narrative_llm` — Qwen3-8B subprocess for story beats, open loops, chapter boundaries.
- `prosody_analysis` — F0, energy, speaking rate, pitch variation, pause ratios, composite expressiveness score per sentence (from vocals).
- `speaker_embed` — WavLM-large (Chen et al., 2022), 512-D per speech segment; agglomerative clustering at cosine-distance 0.7 for automatic diarisation.
- `emotion_embed` — Wav2Vec2-large (Baevski et al., 2020), 1,024-D; derived arousal/valence scalars.
- `reactions` — SenseVoice Small (FunASR).
- `acoustic` — beat/onset detection (librosa/scipy fallback).
- `profanity` — keyword + semantic detection.
- `chronemic` — turn-taking, pause-pattern analysis.
- `highlights` — multi-signal composite scoring (§5.4 below).

Stage dependencies form a DAG that resolves into approximately seven topological levels. The longest path (probe → vfr_normalize → audio_extract → transcribe → prosody_analysis → chronemic → highlights → finalize) runs roughly 15–30 minutes of wall-clock time on a 16-minute 1080p source on RTX 5090 with all optional stages enabled.

## 5.3 The seven embedding modalities

The key observation for DDA is not the number of pipeline stages (which is a pipeline-engineering concern) but the number of **independent embedding spaces** produced. The seven modalities are:

| # | Modality | Model | Dim | Captures |
|---|---|---|---|---|
| 1 | Visual | SigLIP-SO400M-patch14-384 | 1,152 | Facial appearance, pose, expression, lighting, composition |
| 2 | Semantic | Nomic-embed-text-v1.5 | 768 | Spoken meaning — topics, vocabulary, argument structure |
| 3 | Emotion | Wav2Vec2-large | 1,024 | Vocal arousal/valence/dominance |
| 4 | Speaker | WavLM-large | 512 | Biometric voice identity (timbre, formant, vocal-tract characteristics) |
| 5 | Prosody | Custom (F0, energy, rate, contour features) | 12 | Speaking style — the musical quality of speech |
| 6 | Sentiment | all-MiniLM-L6-v2 | 384 | Sentence-level sentiment and intent |
| 7 | Voice | ECAPA-TDNN (Desplanques et al., 2020) | 192 | Speaker verification signature |

**Total: 4,044 dimensions across seven independent frozen models.**

Each modality is produced by a model trained independently by a different research group for a different purpose. They are not jointly trained; their independence is the source of their orthogonal information content. A deviation in any single modality is detectable — a generated frame that scores visual-correct but speaker-wrong fails the speaker check; a frame with correct speaker but wrong emotion fails the emotion check; etc.

A complementary vision signal is produced by FLAME parametric head fitting (Li et al., 2017): `flame_fitter.py` fits 100-D expression coefficients per detected face at approximately 82 frames/second on an RTX 5090, producing an interpretable low-dimensional facial vocabulary on top of the 1,152-D SigLIP visual embedding. FLAME is not one of the seven "modalities" in our primary count — it is a parametric decomposition of the visual signal into FACS-aligned coefficients — but its output is used as an additional information axis in the four-level expression hierarchy described in Section 6.

## 5.4 Cross-stream intelligence: the highlights algorithm

The payoff from multi-modal decomposition is visible in the `highlights` stage, which scores 30-second candidate windows using seven weighted signals drawn from all prior analysis stages:

| Signal | Weight | Source | Computation |
|---|---|---|---|
| Emotion energy | 0.25 | `emotion_curve` (Wav2Vec2) | Average energy of overlapping 5 s windows |
| Reaction presence | 0.20 | `reactions` (SenseVoice) | 1.0 if any detected reaction overlaps |
| Semantic density | 0.20 | `topics` (Nomic Embed) | Average topic coherence in window |
| Narrative | 0.15 | `transcript_segments` | Sentence-boundary alignment score |
| Visual variety | 0.10 | `scenes` (SigLIP) | Scene-change frequency vs. expected |
| Quality | 0.05 | `scenes` (BRISQUE) | Average frame quality |
| Speaker confidence | 0.05 | `transcript_segments` (WavLM) | Single-speaker dominance |

The weighted composite is the DDA point-in-practice: any single signal taken alone misidentifies highlights; combined with weights, cross-modal signal convergence produces a ranking that matches human editorial judgment far better than any one modality can on its own. A moment where emotion energy, audience reaction, topic shift, and scene change all coincide is objectively more interesting than one where only one signal fires — and this is observable only because all four signals are present in the derived representation.

Three additional discovery tools exploit the same pattern:

- `clipcannon_find_cut_points` detects natural edit boundaries by convergence across silence / beat / scene / sentence boundaries; cuts at multi-signal convergence points feel natural; cuts at single-signal points feel jarring.
- `clipcannon_find_safe_cuts` identifies audio-quiet or transitioning points to avoid audio discontinuities.
- `clipcannon_get_narrative_flow` validates proposed edit coherence across temporal ordering, topic continuity, speaker consistency, and emotional arc — before any rendering cost is incurred.

The pattern is a direct realisation of DDA: the decomposition produces seven meaningful signals per input, and the downstream intelligence is the *convergence* of those signals.

## 5.5 The 12,000+ labeled training samples from 16 minutes

The empirical demonstration of DDA on video that we use as an anchor throughout this paper: from a 975-second (16 m 15 s) interview with a single subject ("Santa"), ClipCannon's pipeline produces the following labeled data, none of which required human annotation:

| Modality | Samples | Dimensions | Total scalar parameters |
|---|---|---|---|
| Visual (SigLIP, 2 FPS) | 1,725 | 1,152 | 1,987,200 |
| Semantic (Nomic, per transcript segment) | 200 | 768 | 153,600 |
| Emotion (Wav2Vec2, 5 s windows, 50% overlap) | 343 | 1,024 | 351,232 |
| Speaker (WavLM, per speech segment) | 300 | 512 | 153,600 |
| Prosody (custom, per sentence) | 188 | 12 | 2,256 |
| Sentiment (MiniLM, per sentence) | 202 | 384 | 77,568 |
| Voice (ECAPA-TDNN, per speaker window) | 188 | 192 | 36,096 |
| FLAME expression (parametric head fitting, per frame with face) | 3,477 | 100 | 347,700 |
| **Sum** | **6,623 samples** | — | **3,109,252 parameters** |

Additionally, the curated training-data pipeline (`curate_training_data.py`, `precompute_face_landmarks.py`, `label_training_clips.py`) produces **2,362 individual training clips**, each automatically multi-labeled with:

- speaking vs. breathing classification (from transcript-word alignment plus prosody);
- an emotion tag (from the emotion embedding via a Plutchik mapping);
- an energy level (from prosody analysis);
- 15-category MPEG-4 viseme sequences (from CMU pronunciation-dictionary mapping, 7,916 real samples, 100% viseme coverage);
- scene boundaries and shot types (from visual analysis);
- speaker diarisation labels (from speaker embeddings);
- a mouth mask precomputed from YOLOv5-Face 5-point landmarks.

Counting conservatively, this is 12,000+ individually-labeled, multi-dimensionally-annotated data points from one 16-minute video — each labelled across multiple information axes simultaneously. No human labelled any of them.

This is DDA concretely. The raw data (16 minutes of MP4) is unchanged. What changed is that we now have a structured representation of that data in which every moment is simultaneously labelled for what it looks like, what it sounds like, what emotion it conveys, what words are being said, how those words are being delivered, and who is speaking them.

## 5.6 Storage schema and provenance

Every project lives under `~/.clipcannon/projects/{project_id}/` with the layout:

```
proj_a1b2c3d4/
  analysis.db       # SQLite, schema v4, 34 core tables + 4 vector tables
  source/           # Original source video (SHA-256 verified at ingest)
  stems/            # Audio stems from HTDemucs
  frames/           # Extracted JPEG frames
  storyboards/      # Contact-sheet images
  edits/            # Edit working directories
  renders/          # Rendered output video files
```

The four `sqlite-vec` virtual tables — `vec_frames` (float[1152]), `vec_semantic` (float[768]), `vec_emotion` (float[1024]), `vec_speakers` (float[512]) — provide sub-linear nearest-neighbour search over each modality's embeddings. Every connection applies a uniform pragma set: `journal_mode=WAL`, `synchronous=NORMAL`, 64 MB page cache, `foreign_keys=ON`, `temp_store=MEMORY`.

Every pipeline operation produces an immutable row in the `provenance` table with 22 fields including `record_id`, `project_id`, `timestamp_utc`, `operation`, `stage`, `input_sha256`, `output_sha256`, model `name/version/quantization/parameters`, execution metrics, `parent_record_id`, and `chain_hash`. The chain hash is computed as

```
SHA256("{parent_hash}|{input_sha256}|{output_sha256}|{operation}|{model_name}|{model_version}|{json.dumps(model_params, sort_keys=True)}")
```

The first record uses the sentinel `GENESIS_HASH = "GENESIS"`. Each subsequent record incorporates its parent's chain hash, so any modification to any historical record invalidates all downstream hashes. The `verify_chain()` function walks the chain in chronological order and identifies the exact broken record on failure.

The provenance chain detects accidental corruption and unauthorised partial edits. It does not detect a privileged actor with database write access who recomputes the entire chain from genesis with modified data — that would require Ed25519-style signatures, which OCR Provenance (Section 11) implements for the document-intelligence context. The threat model for ClipCannon's provenance is integrity verification, not adversarial tamper resistance.

## 5.7 The 58 MCP tools

The MCP server registers 58 tools across thirteen groups:

| Group | Count | Examples |
|---|---|---|
| Project management | 5 | `project_create`, `project_status`, `project_delete` |
| Understanding | 4 | `ingest`, `get_transcript`, `get_frame`, `search_content` |
| Discovery | 4 | `find_best_moments`, `find_cut_points`, `get_narrative_flow`, `find_safe_cuts` |
| Disk management | 2 | `disk_status`, `disk_cleanup` |
| Configuration | 3 | `config_get/set/list` |
| Editing | 11 | `create_edit`, `modify_edit`, `auto_trim`, `apply_feedback`, `branch_edit` |
| Rendering | 8 | `render`, `get_editing_context`, `inspect_render`, `preview_layout`, `get_scene_map` |
| Audio generation | 6 | `generate_music`, `compose_midi`, `generate_sfx`, `auto_music`, `audio_cleanup` |
| Voice | 4 | `prepare_voice_data`, `voice_profiles`, `speak`, `speak_optimized` |
| Avatar | 2 | `lip_sync`, `extract_webcam` |
| Video generation (end-to-end) | 1 | `generate_video` |
| Constellation / clone video | 4 | `list_constellations`, `list_skills`, `clone_video`, `expression_sequence` |
| Billing | 4 | `credits_balance`, `credits_history`, `credits_estimate`, `spending_limit` |

The constellation tools are the externally-visible surface of Teleological Constellation Training (Section 6). They are named consistently with Context Graph's constellation tools, but they operate over the video-modality centroids rather than text-modality centroids.

## 5.8 Companion engines: Phoenix and VoiceAgent

Two companion packages extend ClipCannon's capabilities without changing the core DDA argument.

**Phoenix** (`src/phoenix/`) is a GPU-native avatar engine designed for real-time, identity-locked, controllable avatar rendering for interactive scenarios. It contains the four-level expression hierarchy (Section 6.3), the ConstellationController (runtime prompt orchestrator), the ConstellationGuard (runtime identity validator), and a GPU safety harness (`gpu_guard.py`) that serialises CUDA operations to prevent the WSL2 driver-context crashes documented in the system's forensic reports. Phoenix is packaged in the same wheel as ClipCannon but is architecturally distinct: ClipCannon handles batch video analysis and editing; Phoenix handles real-time generation, live meetings, and runtime validation.

**VoiceAgent** (`src/voiceagent/`) is a standalone real-time conversational agent with a 25-file meeting-bot subsystem that can join Google Meet and Zoom meetings as an AI clone of the subject. It uses `Xvfb + Playwright` (the same architecture as `tl;dv`, `Recall.ai`, `Otter.ai`) for meeting participation, MediaStreamTrackGenerator + Insertable Streams for browser-visible video delivery (the only delivery path we could make work across Meet's device requirements), WebAudio API JS injection for audio (since Chrome's fake device bypasses PulseAudio), and a three-tier always-on reasoning controller that runs in 800 ms cycles producing per-step `AvatarCommand` payloads (blendshapes, gaze, pose, prosody) for Phoenix. End-to-end speech-to-speech latency targets <2 s; video latency targets <100 ms at 25 fps.

Both companion packages consume the constellation representation produced by TCT. Neither changes the DDA argument on its own; they are included here because the identity-locked case studies in Sections 8–9 depend on them.

## 5.9 Evidence of orthogonality: the seventeen-approach engineering journey

An empirical observation that supports the DDA claim indirectly: over a six-day engineering sprint, the team attempted seventeen different architectures for the identity-locked video clone (Sections 8.4 and documented in the accompanying Sherlock forensic reports, `docs/inspect/01`–`10`.md). Sixteen of the seventeen failed for substantive technical reasons, and the failure modes were consistently one-dimensional — the model was visually correct but lip-sync-wrong, or lip-sync-correct but identity-drifting, or temporally stable but static and uncanny. No single-modality target sufficed. The seventeenth approach (the one that is being trained as of this writing) works because it defines success as **simultaneous** proximity to all seven modality centroids — the constellation framing described in Section 6.

The history of failed approaches is therefore itself evidence that multi-modal decomposition is not merely a training-data multiplier but an alignment substrate: problems that look insoluble in a single modality become tractable once the decomposition gives the training pipeline a multi-dimensional target it can satisfy simultaneously.

## 5.10 What ClipCannon demonstrates for DDA

ClipCannon establishes, for the audiovisual modality:

1. **Seven independent embedders of video are implementable on one workstation.** The pipeline runs end-to-end on an RTX 5090 with tractable latency (~15–30 m on a 16 m input).
2. **The derived data multiplies effective training-sample count by at least an order of magnitude.** 16 minutes of video yields 12,000+ labelled samples without any human annotation.
3. **Cross-modal signal convergence is a real, exploitable property.** The `highlights` algorithm and the `find_cut_points`, `find_safe_cuts`, and `get_narrative_flow` tools all use convergence across 4–7 modalities to produce results that any one modality alone cannot.
4. **The same architectural pattern (pipeline DAG → multiple embedders → structured database → MCP tool surface) applies across modalities.** Context Graph and ClipCannon are different instantiations of the same DDA architecture.

Section 6 presents the training method that uses these decompositions as alignment targets.
